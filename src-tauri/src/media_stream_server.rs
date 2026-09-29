//! Loopback HTTP server for webview media on Linux.
//!
//! WebKitGTK never hands custom URI schemes such as `asset://` to its media player, so
//! `<video>`/`<audio>` elements cannot play project media through Tauri's asset protocol.
//! HTTP URLs are loaded by WebKit's own network stack and support seeking through byte ranges.
//! This server binds to 127.0.0.1 only, requires a per-launch random token in every path, and
//! delegates path authorization to the caller (the Tauri asset-protocol scope).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const COPY_BUFFER_BYTES: usize = 256 * 1024;

pub type MediaPathAuthorizer = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

#[derive(Debug, Clone)]
pub struct MediaStreamServer {
    address: SocketAddr,
    token: String,
}

impl MediaStreamServer {
    /// Base URL for media paths; append the percent-encoded absolute file path.
    pub fn base_url(&self) -> String {
        format!("http://{}/media/{}", self.address, self.token)
    }

    /// Script that exposes the base URL to the frontend before any page script runs.
    pub fn initialization_script(&self) -> String {
        format!(
            "Object.defineProperty(window, '__VIDEO_CREATER_MEDIA_STREAM_BASE__', {{ value: {}, writable: false, configurable: false }});",
            serde_json::to_string(&self.base_url()).unwrap_or_else(|_| "null".to_string())
        )
    }
}

pub fn start_media_stream_server(authorize: MediaPathAuthorizer) -> io::Result<MediaStreamServer> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let address = listener.local_addr()?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let server = MediaStreamServer {
        address,
        token: token.clone(),
    };
    thread::Builder::new()
        .name("media-stream-accept".to_string())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let token = token.clone();
                let authorize = Arc::clone(&authorize);
                let _ = thread::Builder::new()
                    .name("media-stream-connection".to_string())
                    .spawn(move || {
                        let _ = serve_connection(stream, &token, authorize.as_ref());
                    });
            }
        })?;
    Ok(server)
}

#[derive(Debug, PartialEq, Eq)]
struct RequestHead {
    method: String,
    target: String,
    range: Option<String>,
}

fn serve_connection(
    stream: TcpStream,
    token: &str,
    authorize: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> io::Result<()> {
    stream.set_read_timeout(Some(CONNECTION_TIMEOUT))?;
    stream.set_write_timeout(Some(CONNECTION_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let head = match read_request_head(&mut reader) {
        Ok(head) => head,
        Err(_) => return write_status(&mut writer, 400, "Bad Request"),
    };
    let head_only = match head.method.as_str() {
        "GET" => false,
        "HEAD" => true,
        _ => return write_status(&mut writer, 405, "Method Not Allowed"),
    };
    let Some(path) = authorized_media_path(&head.target, token, authorize) else {
        return write_status(&mut writer, 404, "Not Found");
    };
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => return write_status(&mut writer, 404, "Not Found"),
    };
    let size = file.metadata()?.len();
    let range = match head
        .range
        .as_deref()
        .map(|range| parse_byte_range(range, size))
    {
        None => None,
        Some(Some(range)) => Some(range),
        Some(None) => {
            let response = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{size}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            return writer.write_all(response.as_bytes());
        }
    };
    let (status, start, length) = match range {
        Some((start, end)) => ("206 Partial Content", start, end - start + 1),
        None => ("200 OK", 0, size),
    };
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}\r\nContent-Length: {length}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n",
        content_type_for(&path)
    );
    if range.is_some() {
        response.push_str(&format!(
            "Content-Range: bytes {start}-{}/{size}\r\n",
            start + length.saturating_sub(1)
        ));
    }
    response.push_str("\r\n");
    writer.write_all(response.as_bytes())?;
    if head_only || length == 0 {
        return writer.flush();
    }
    file.seek(SeekFrom::Start(start))?;
    let mut remaining = file.take(length);
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = remaining.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        // The media element closes connections while seeking; that is not an error.
        if writer.write_all(&buffer[..read]).is_err() {
            break;
        }
    }
    writer.flush()
}

fn read_request_head(reader: &mut impl BufRead) -> io::Result<RequestHead> {
    let mut consumed = 0_usize;
    let mut line = String::new();
    let mut next_line = |line: &mut String| -> io::Result<()> {
        line.clear();
        let read = reader.read_line(line)?;
        consumed += read;
        if read == 0 || consumed > MAX_REQUEST_HEAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid request head",
            ));
        }
        Ok(())
    };
    next_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    if method.is_empty() || !target.starts_with('/') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid request line",
        ));
    }
    let mut range = None;
    loop {
        next_line(&mut line)?;
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_string());
            }
        }
    }
    Ok(RequestHead {
        method,
        target,
        range,
    })
}

fn authorized_media_path(
    target: &str,
    token: &str,
    authorize: &(dyn Fn(&Path) -> bool + Send + Sync),
) -> Option<PathBuf> {
    let target = target.split(['?', '#']).next()?;
    let rest = target.strip_prefix("/media/")?;
    let (request_token, encoded_path) = rest.split_once('/')?;
    if !constant_time_eq(request_token.as_bytes(), token.as_bytes()) {
        return None;
    }
    let decoded = percent_decode(encoded_path)?;
    let path = PathBuf::from(decoded);
    if !path.is_absolute() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    (canonical.is_file() && authorize(&canonical)).then_some(canonical)
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
                decoded.push(u8::from_str_radix(hex, 16).ok()?);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    let decoded = String::from_utf8(decoded).ok()?;
    (!decoded.contains('\0')).then_some(decoded)
}

/// Parses a single `bytes=` range into an inclusive `(start, end)` within `size`.
fn parse_byte_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || size == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix = suffix.parse::<u64>().ok()?;
            if suffix == 0 {
                return None;
            }
            (size.saturating_sub(suffix), size - 1)
        }
        (start, "") => (start.parse::<u64>().ok()?, size - 1),
        (start, end) => (
            start.parse::<u64>().ok()?,
            end.parse::<u64>().ok()?.min(size - 1),
        ),
    };
    (start <= end && start < size).then_some((start, end))
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("ogv") => "video/ogg",
        Some("m4a") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("ogg" | "oga" | "opus") => "audio/ogg",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

fn write_status(writer: &mut impl Write, code: u16, reason: &str) -> io::Result<()> {
    writer.write_all(
        format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn request(server: &MediaStreamServer, target: &str, extra_headers: &str) -> (String, Vec<u8>) {
        let mut stream = TcpStream::connect(server.address).expect("connect");
        write!(
            stream,
            "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra_headers}\r\n"
        )
        .expect("write request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        let split = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("header terminator");
        (
            String::from_utf8_lossy(&response[..split]).into_owned(),
            response[split + 4..].to_vec(),
        )
    }

    fn encoded(path: &Path) -> String {
        path.to_string_lossy()
            .bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                    (byte as char).to_string()
                }
                _ => format!("%{byte:02X}"),
            })
            .collect()
    }

    #[test]
    fn serves_authorized_files_with_byte_ranges() {
        let dir = tempfile::tempdir().expect("temp dir");
        let media = dir.path().join("clip with space.mp4");
        std::fs::write(&media, b"0123456789").expect("media");
        let allowed = dir.path().canonicalize().expect("canonical");
        let server = start_media_stream_server(Arc::new(move |path| path.starts_with(&allowed)))
            .expect("server");
        let target = format!("/media/{}/{}", server.token, encoded(&media));

        let (head, body) = request(&server, &target, "");
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert!(head.contains("Content-Type: video/mp4"));
        assert_eq!(body, b"0123456789");

        let (head, body) = request(&server, &target, "Range: bytes=2-5\r\n");
        assert!(head.starts_with("HTTP/1.1 206"), "{head}");
        assert!(head.contains("Content-Range: bytes 2-5/10"));
        assert_eq!(body, b"2345");

        let (head, body) = request(&server, &target, "Range: bytes=-3\r\n");
        assert!(head.contains("Content-Range: bytes 7-9/10"));
        assert_eq!(body, b"789");

        let (head, _) = request(&server, &target, "Range: bytes=20-\r\n");
        assert!(head.starts_with("HTTP/1.1 416"), "{head}");
    }

    #[test]
    fn rejects_wrong_tokens_and_unauthorized_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        let allowed_dir = dir.path().join("project");
        std::fs::create_dir_all(&allowed_dir).expect("project dir");
        let secret = dir.path().join("secret.mp4");
        std::fs::write(&secret, b"secret").expect("secret");
        let allowed = allowed_dir.canonicalize().expect("canonical");
        let server = start_media_stream_server(Arc::new(move |path| path.starts_with(&allowed)))
            .expect("server");

        let (head, _) = request(
            &server,
            &format!("/media/{}/{}", "0".repeat(64), encoded(&secret)),
            "",
        );
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");
        let (head, _) = request(
            &server,
            &format!("/media/{}/{}", server.token, encoded(&secret)),
            "",
        );
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");
        let escape = allowed_dir.join("..").join("secret.mp4");
        let (head, _) = request(
            &server,
            &format!("/media/{}/{}", server.token, encoded(&escape)),
            "",
        );
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");
    }

    #[test]
    fn parses_request_heads_and_ranges() {
        let head = read_request_head(&mut Cursor::new(
            b"HEAD /media/t/x HTTP/1.1\r\nrange: bytes=0-\r\n\r\n".to_vec(),
        ))
        .expect("head");
        assert_eq!(head.method, "HEAD");
        assert_eq!(head.range.as_deref(), Some("bytes=0-"));
        assert!(
            read_request_head(&mut Cursor::new(b"GET relative HTTP/1.1\r\n\r\n".to_vec())).is_err()
        );
        assert_eq!(parse_byte_range("bytes=0-99", 10), Some((0, 9)));
        assert_eq!(parse_byte_range("bytes=0-1,4-5", 10), None);
        assert_eq!(parse_byte_range("bytes=-0", 10), None);
        assert_eq!(
            percent_decode("%2Ftmp%2Fa%20b"),
            Some("/tmp/a b".to_string())
        );
        assert_eq!(percent_decode("%00"), None);
        assert_eq!(percent_decode("%zz"), None);
    }
}
