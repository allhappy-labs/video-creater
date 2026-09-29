use base64::prelude::*;
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderName, HeaderValue, CONTENT_LENGTH, LOCATION};
use reqwest::redirect::Policy;
use reqwest::{StatusCode, Url};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tempfile::NamedTempFile;
use thiserror::Error;

pub const JSON_RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;
pub const CATALOG_RESPONSE_LIMIT: u64 = 2 * 1024 * 1024;
pub const IMAGE_OUTPUT_LIMIT: u64 = 128 * 1024 * 1024;
pub const AUDIO_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;
pub const VIDEO_OUTPUT_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
pub const IMAGE_ENVELOPE_LIMIT: u64 = 180 * 1024 * 1024;
pub const AUDIO_ENVELOPE_LIMIT: u64 = 700 * 1024 * 1024;

pub fn output_limit_for_path(path: &str) -> u64 {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".mp4") || lower.ends_with(".mov") || lower.ends_with(".webm") {
        VIDEO_OUTPUT_LIMIT
    } else if lower.ends_with(".mp3")
        || lower.ends_with(".wav")
        || lower.ends_with(".aac")
        || lower.ends_with(".flac")
    {
        AUDIO_OUTPUT_LIMIT
    } else {
        IMAGE_OUTPUT_LIMIT
    }
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("response exceeds {limit} byte limit")]
    TooLarge { limit: u64 },
    #[error("encoded response exceeds {limit} byte limit")]
    EncodedTooLarge { limit: u64 },
    #[error("download was cancelled")]
    Cancelled,
    #[error("invalid download URL: {0}")]
    InvalidUrl(String),
    #[error("download destination is not permitted: {0}")]
    NetworkPolicy(String),
    #[error("download failed: {0}")]
    Request(String),
    #[error("download I/O failed: {0}")]
    Io(String),
    #[error("encoded media is invalid: {0}")]
    Decode(String),
}

fn ensure_success(response: &Response) -> Result<(), DownloadError> {
    if response.status().is_success() {
        Ok(())
    } else {
        Err(DownloadError::HttpStatus {
            status: response.status().as_u16(),
        })
    }
}

fn declared_length(response: &Response, limit: u64) -> Result<(), DownloadError> {
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > limit)
    {
        return Err(DownloadError::TooLarge { limit });
    }
    Ok(())
}

pub fn read_response_bounded(
    response: Response,
    limit: u64,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>, DownloadError> {
    ensure_success(&response)?;
    read_response_body_bounded(response, limit, cancelled)
}

pub fn read_response_body_bounded(
    mut response: Response,
    limit: u64,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u8>, DownloadError> {
    declared_length(&response, limit)?;
    let mut output = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(0)
            .min(limit)
            .min(64 * 1024) as usize,
    );
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let count = response
            .read(&mut buffer)
            .map_err(|_| DownloadError::Request("response body read failed".into()))?;
        if cancelled() {
            return Err(DownloadError::Cancelled);
        }
        if count == 0 {
            return Ok(output);
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or(DownloadError::TooLarge { limit })?;
        if observed > limit {
            return Err(DownloadError::TooLarge { limit });
        }
        output.extend_from_slice(&buffer[..count]);
    }
}

pub fn stream_response_to_atomic_file(
    mut response: Response,
    destination: &Path,
    limit: u64,
    cancelled: impl Fn() -> bool,
) -> Result<u64, DownloadError> {
    ensure_success(&response)?;
    declared_length(&response, limit)?;
    if cancelled() {
        return Err(DownloadError::Cancelled);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| DownloadError::Io("destination has no parent directory".into()))?;
    fs::create_dir_all(parent).map_err(|error| DownloadError::Io(error.to_string()))?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|error| DownloadError::Io(error.to_string()))?;
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let count = response
            .read(&mut buffer)
            .map_err(|_| DownloadError::Request("response body read failed".into()))?;
        if cancelled() {
            return Err(DownloadError::Cancelled);
        }
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .ok_or(DownloadError::TooLarge { limit })?;
        if observed > limit {
            return Err(DownloadError::TooLarge { limit });
        }
        temporary
            .write_all(&buffer[..count])
            .map_err(|error| DownloadError::Io(error.to_string()))?;
    }
    temporary
        .flush()
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| DownloadError::Io(error.to_string()))?;
    if cancelled() {
        return Err(DownloadError::Cancelled);
    }
    temporary
        .persist(destination)
        .map_err(|error| DownloadError::Io(error.error.to_string()))?;
    Ok(observed)
}

pub fn write_bytes_atomically(destination: &Path, bytes: &[u8]) -> Result<(), DownloadError> {
    let parent = destination
        .parent()
        .ok_or_else(|| DownloadError::Io("destination has no parent directory".into()))?;
    fs::create_dir_all(parent).map_err(|error| DownloadError::Io(error.to_string()))?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|error| DownloadError::Io(error.to_string()))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.flush())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| DownloadError::Io(error.to_string()))?;
    temporary
        .persist(destination)
        .map_err(|error| DownloadError::Io(error.error.to_string()))?;
    Ok(())
}

pub fn decode_base64_bounded(
    encoded: &str,
    encoded_limit: u64,
    decoded_limit: u64,
) -> Result<Vec<u8>, DownloadError> {
    let encoded = encoded.trim();
    if encoded.len() as u64 > encoded_limit {
        return Err(DownloadError::EncodedTooLarge {
            limit: encoded_limit,
        });
    }
    let padding = encoded
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count() as u64;
    let estimated = (encoded.len() as u64 / 4)
        .saturating_mul(3)
        .saturating_sub(padding)
        .saturating_add(match encoded.len() % 4 {
            2 => 1,
            3 => 2,
            _ => 0,
        });
    if estimated > decoded_limit {
        return Err(DownloadError::TooLarge {
            limit: decoded_limit,
        });
    }
    let decoded = BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| DownloadError::Decode(error.to_string()))?;
    if decoded.len() as u64 > decoded_limit {
        return Err(DownloadError::TooLarge {
            limit: decoded_limit,
        });
    }
    Ok(decoded)
}

pub fn decode_hex_bounded(
    encoded: &str,
    encoded_limit: u64,
    decoded_limit: u64,
) -> Result<Vec<u8>, DownloadError> {
    let encoded = encoded.trim();
    if encoded.len() as u64 > encoded_limit {
        return Err(DownloadError::EncodedTooLarge {
            limit: encoded_limit,
        });
    }
    if encoded.len() as u64 / 2 > decoded_limit {
        return Err(DownloadError::TooLarge {
            limit: decoded_limit,
        });
    }
    if !encoded.len().is_multiple_of(2) {
        return Err(DownloadError::Decode("hex input has an odd length".into()));
    }
    let mut output = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let text =
            std::str::from_utf8(pair).map_err(|error| DownloadError::Decode(error.to_string()))?;
        output.push(
            u8::from_str_radix(text, 16)
                .map_err(|error| DownloadError::Decode(error.to_string()))?,
        );
    }
    Ok(output)
}

#[derive(Clone, Debug, Default)]
pub struct DownloadNetworkPolicy {
    trusted_origins: Vec<String>,
    max_redirects: usize,
}

impl DownloadNetworkPolicy {
    pub fn public_only() -> Self {
        Self {
            trusted_origins: Vec::new(),
            max_redirects: 3,
        }
    }

    pub fn with_trusted_origin(mut self, origin: &str) -> Result<Self, DownloadError> {
        let url =
            Url::parse(origin).map_err(|error| DownloadError::InvalidUrl(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(DownloadError::NetworkPolicy(
                "trusted origin must be HTTP(S) without user information".into(),
            ));
        }
        self.trusted_origins.push(url_origin(&url)?);
        Ok(self)
    }

    fn is_trusted(&self, url: &Url) -> bool {
        url_origin(url).ok().is_some_and(|origin| {
            self.trusted_origins
                .iter()
                .any(|trusted| trusted == &origin)
        })
    }

    pub fn resolve_hop(&self, url: &Url) -> Result<Vec<SocketAddr>, DownloadError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(DownloadError::NetworkPolicy(
                "only HTTP(S) URLs are allowed".into(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(DownloadError::NetworkPolicy(
                "URL user information is forbidden".into(),
            ));
        }
        let trusted = self.is_trusted(url);
        if url.scheme() != "https" && !trusted {
            return Err(DownloadError::NetworkPolicy(
                "public downloads require HTTPS".into(),
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| DownloadError::InvalidUrl("URL has no host".into()))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| DownloadError::InvalidUrl("URL has no port".into()))?;
        let addresses: Vec<_> = (host, port)
            .to_socket_addrs()
            .map_err(|_| DownloadError::Request("DNS resolution failed".into()))?
            .collect();
        if addresses.is_empty() {
            return Err(DownloadError::Request(
                "DNS resolution returned no addresses".into(),
            ));
        }
        if !trusted && addresses.iter().any(|address| !is_public_ip(address.ip())) {
            return Err(DownloadError::NetworkPolicy(
                "resolved address is local or private".into(),
            ));
        }
        Ok(addresses)
    }
}

fn url_origin(url: &Url) -> Result<String, DownloadError> {
    let host = url
        .host_str()
        .ok_or_else(|| DownloadError::InvalidUrl("URL has no host".into()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| DownloadError::InvalidUrl("URL has no port".into()))?;
    Ok(format!(
        "{}://{}:{port}",
        url.scheme(),
        host.to_ascii_lowercase()
    ))
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(is_public_ipv4)
            .unwrap_or_else(|| is_public_ipv6(ip)),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xff00) == 0xff00
        || segments[0] == 0x2002
        || (segments[0] == 0x2001 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[derive(Clone, Debug)]
pub struct ScopedHeader {
    name: HeaderName,
    value: HeaderValue,
    origin: String,
}

impl ScopedHeader {
    pub fn new(name: &str, value: &str, origin_url: &str) -> Result<Self, DownloadError> {
        let origin_url =
            Url::parse(origin_url).map_err(|error| DownloadError::InvalidUrl(error.to_string()))?;
        Ok(Self {
            name: HeaderName::from_str(name)
                .map_err(|error| DownloadError::InvalidUrl(error.to_string()))?,
            value: HeaderValue::from_str(value)
                .map_err(|error| DownloadError::InvalidUrl(error.to_string()))?,
            origin: url_origin(&origin_url)?,
        })
    }
}

pub fn download_url_to_atomic_file(
    source_url: &str,
    destination: impl AsRef<Path>,
    limit: u64,
    cancelled: impl Fn() -> bool,
    network_policy: &DownloadNetworkPolicy,
    scoped_header: Option<ScopedHeader>,
) -> Result<u64, DownloadError> {
    let mut current =
        Url::parse(source_url).map_err(|error| DownloadError::InvalidUrl(error.to_string()))?;
    let mut visited = HashSet::new();
    for redirect_count in 0..=network_policy.max_redirects {
        if !visited.insert(current.as_str().to_string()) {
            return Err(DownloadError::NetworkPolicy(
                "redirect loop detected".into(),
            ));
        }
        let addresses = network_policy.resolve_hop(&current)?;
        let host = current
            .host_str()
            .ok_or_else(|| DownloadError::InvalidUrl("URL has no host".into()))?;
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(300))
            .no_proxy()
            .redirect(Policy::none());
        for address in addresses {
            builder = builder.resolve(host, address);
        }
        let client = builder
            .build()
            .map_err(|_| DownloadError::Request("download client creation failed".into()))?;
        let mut request = client.get(current.clone());
        if let Some(header) = &scoped_header {
            if url_origin(&current)? == header.origin {
                request = request.header(header.name.clone(), header.value.clone());
            }
        }
        if cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let response = request
            .send()
            .map_err(|_| DownloadError::Request("download request failed".into()))?;
        if response.status().is_redirection() {
            if redirect_count == network_policy.max_redirects {
                return Err(DownloadError::NetworkPolicy(
                    "redirect limit exceeded".into(),
                ));
            }
            let location = response
                .headers()
                .get(LOCATION)
                .ok_or_else(|| DownloadError::NetworkPolicy("redirect has no Location".into()))?
                .to_str()
                .map_err(|error| DownloadError::NetworkPolicy(error.to_string()))?;
            let next = current
                .join(location)
                .map_err(|error| DownloadError::InvalidUrl(error.to_string()))?;
            if current.scheme() == "https"
                && next.scheme() == "http"
                && !network_policy.is_trusted(&next)
            {
                return Err(DownloadError::NetworkPolicy(
                    "HTTPS downgrade is forbidden".into(),
                ));
            }
            current = next;
            continue;
        }
        if response.status() == StatusCode::NOT_MODIFIED {
            return Err(DownloadError::HttpStatus { status: 304 });
        }
        return stream_response_to_atomic_file(response, destination.as_ref(), limit, &cancelled);
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn serve(response: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            stream.write_all(response).unwrap();
        });
        format!("http://{address}")
    }

    #[test]
    fn bounded_reader_rejects_declared_and_observed_oversize() {
        let client = reqwest::blocking::Client::new();
        let declared = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nabcdef");
        let error =
            read_response_bounded(client.get(declared).send().unwrap(), 5, || false).unwrap_err();
        assert!(matches!(error, DownloadError::TooLarge { .. }));

        let chunked = serve(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n");
        let error =
            read_response_bounded(client.get(chunked).send().unwrap(), 5, || false).unwrap_err();
        assert!(matches!(error, DownloadError::TooLarge { .. }));
    }

    #[test]
    fn atomic_stream_preserves_destination_and_removes_unique_temp_on_error() {
        let client = reqwest::blocking::Client::new();
        let url = serve(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n");
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("output.bin");
        std::fs::write(&destination, b"existing").unwrap();
        let error = stream_response_to_atomic_file(
            client.get(url).send().unwrap(),
            &destination,
            5,
            || false,
        )
        .unwrap_err();
        assert!(matches!(error, DownloadError::TooLarge { .. }));
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn atomic_stream_cancellation_preserves_destination_and_removes_temp() {
        use std::cell::Cell;
        let client = reqwest::blocking::Client::new();
        let url = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nabcdef");
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("output.bin");
        std::fs::write(&destination, b"existing").unwrap();
        let checks = Cell::new(0_u8);
        let error = stream_response_to_atomic_file(
            client.get(url).send().unwrap(),
            &destination,
            10,
            || {
                let next = checks.get() + 1;
                checks.set(next);
                next >= 2
            },
        )
        .unwrap_err();
        assert!(matches!(error, DownloadError::Cancelled));
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn encoded_media_limits_apply_before_and_after_decode() {
        assert!(matches!(
            decode_base64_bounded("YWJjZA==", 7, 8),
            Err(DownloadError::EncodedTooLarge { .. })
        ));
        assert!(matches!(
            decode_base64_bounded("YWJjZA==", 8, 3),
            Err(DownloadError::TooLarge { .. })
        ));
        assert_eq!(decode_hex_bounded("616263", 6, 3).unwrap(), b"abc");
    }

    #[test]
    fn network_policy_requires_explicit_exact_origin_for_private_addresses() {
        let url = reqwest::Url::parse("http://127.0.0.1:43123/file").unwrap();
        assert!(DownloadNetworkPolicy::public_only()
            .resolve_hop(&url)
            .is_err());
        let allowed = DownloadNetworkPolicy::public_only()
            .with_trusted_origin("http://127.0.0.1:43123")
            .unwrap();
        assert!(allowed.resolve_hop(&url).is_ok());
        let other_port = reqwest::Url::parse("http://127.0.0.1:43124/file").unwrap();
        assert!(allowed.resolve_hop(&other_port).is_err());
    }

    #[test]
    fn trusted_redirect_download_never_forwards_scoped_header_cross_origin() {
        let target_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let target_address = target_listener.local_addr().unwrap();
        let target = thread::spawn(move || {
            let (mut stream, _) = target_listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let count = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
            request
        });
        let redirect_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let redirect_address = redirect_listener.local_addr().unwrap();
        let redirect = thread::spawn(move || {
            let (mut stream, _) = redirect_listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let count = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]).to_string();
            let response = format!("HTTP/1.1 302 Found\r\nLocation: http://{target_address}/final\r\nContent-Length: 0\r\n\r\n");
            stream.write_all(response.as_bytes()).unwrap();
            request
        });
        let start = format!("http://{redirect_address}/start");
        let policy = DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&format!("http://{redirect_address}"))
            .unwrap()
            .with_trusted_origin(&format!("http://{target_address}"))
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        download_url_to_atomic_file(
            &start,
            dir.path().join("out"),
            10,
            || false,
            &policy,
            Some(ScopedHeader::new("x-api-key", "secret", &start).unwrap()),
        )
        .unwrap();
        assert_eq!(std::fs::read(dir.path().join("out")).unwrap(), b"ok");
        assert!(redirect
            .join()
            .unwrap()
            .to_ascii_lowercase()
            .contains("x-api-key: secret"));
        assert!(!target
            .join()
            .unwrap()
            .to_ascii_lowercase()
            .contains("x-api-key"));
    }

    #[test]
    fn redirect_from_trusted_origin_to_untrusted_private_origin_is_rejected() {
        let redirect_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let redirect_address = redirect_listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = redirect_listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/private\r\nContent-Length: 0\r\n\r\n").unwrap();
        });
        let start = format!("http://{redirect_address}/start");
        let policy = DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&start)
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let error = download_url_to_atomic_file(
            &start,
            dir.path().join("out"),
            10,
            || false,
            &policy,
            None,
        )
        .unwrap_err();
        assert!(matches!(error, DownloadError::NetworkPolicy(_)));
        assert!(!dir.path().join("out").exists());
    }

    #[test]
    fn public_policy_rejects_special_use_and_ipv4_mapped_addresses() {
        for source in [
            "https://127.0.0.1/file",
            "https://169.254.169.254/metadata",
            "https://224.0.0.1/file",
            "https://[::]/file",
            "https://[::1]/file",
            "https://[::ffff:127.0.0.1]/file",
            "https://[2001:db8::1]/file",
            "https://[2002:7f00:1::]/file",
        ] {
            let url = Url::parse(source).unwrap();
            assert!(DownloadNetworkPolicy::public_only()
                .resolve_hop(&url)
                .is_err());
        }
    }
}
