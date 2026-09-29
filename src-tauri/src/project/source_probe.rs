use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const PROTOCOL: &str = "video-creater.avfoundation-export";
const PROTOCOL_VERSION: u32 = 5;
const EXPORTER_ENV_VAR: &str = "VIDEO_CREATER_AVFOUNDATION_EXPORTER";
const MAX_PROTOCOL_BYTES: usize = 64 * 1024;
const SOURCE_PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const SOURCE_PROBE_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The first probe after install may rebuild the plugin registry.
const GSTREAMER_SOURCE_PROBE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceMediaType {
    Video,
    Audio,
    Image,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceProbeMetadata {
    pub source_path: String,
    pub media_type: SourceMediaType,
    pub duration_seconds: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

#[derive(Debug, Error)]
pub enum SourceProbeError {
    #[error("source path must identify an existing regular file")]
    InvalidSource,
    #[error("bundled source probe could not be located: {0}")]
    Unavailable(String),
    #[error("bundled source probe could not be started: {0}")]
    Start(String),
    #[error("bundled source probe request could not be written: {0}")]
    Request(String),
    #[error("bundled source probe failed: {0}")]
    Worker(String),
    #[error("bundled source probe emitted an invalid protocol response: {0}")]
    Protocol(String),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceProbeRequest<'a> {
    protocol: &'static str,
    schema_version: u32,
    request_id: &'a str,
    source_path: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum SourceProbeEvent {
    Completed {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        result: SourceProbeMetadata,
    },
    Failed {
        protocol: String,
        schema_version: u32,
        request_id: Option<String>,
        error: SourceProbeWorkerError,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceProbeWorkerError {
    code: String,
    message: String,
    field: Option<String>,
    retryable: bool,
}

/// Probes source media with the platform's reviewed media runtime: the bundled AVFoundation
/// exporter on macOS, and the GStreamer compatibility worker elsewhere. Both run out of process.
pub fn probe_source_metadata(source_path: &Path) -> Result<SourceProbeMetadata, SourceProbeError> {
    if cfg!(target_os = "macos") {
        probe_with_avfoundation_exporter(source_path)
    } else {
        probe_with_gstreamer_worker(source_path)
    }
}

fn probe_with_gstreamer_worker(
    source_path: &Path,
) -> Result<SourceProbeMetadata, SourceProbeError> {
    let source_path = source_path
        .canonicalize()
        .map_err(|_| SourceProbeError::InvalidSource)?;
    if !source_path.is_file() {
        return Err(SourceProbeError::InvalidSource);
    }
    let source_path_text = source_path
        .to_str()
        .ok_or(SourceProbeError::InvalidSource)?
        .to_string();
    let probe = crate::precompose::compatibility::probe_compatibility_source_within(
        &source_path,
        GSTREAMER_SOURCE_PROBE_TIMEOUT,
    )
    .map_err(|errors| {
        SourceProbeError::Worker(
            errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    let metadata =
        source_metadata_from_media_probe(source_path_text, is_still_image(&source_path), &probe)?;
    validate_metadata(&metadata)?;
    Ok(metadata)
}

fn is_still_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "tiff" | "tif" | "heic" | "heif" | "bmp" | "gif")
    )
}

fn source_metadata_from_media_probe(
    source_path: String,
    still_image: bool,
    probe: &video_creater_compatibility_protocol::MediaProbe,
) -> Result<SourceProbeMetadata, SourceProbeError> {
    let duration_seconds = (probe.duration_seconds.is_finite() && probe.duration_seconds > 0.0)
        .then_some(probe.duration_seconds);
    match (&probe.video, &probe.audio) {
        (Some(video), _) if still_image || video.still_image => Ok(SourceProbeMetadata {
            source_path,
            media_type: SourceMediaType::Image,
            duration_seconds: None,
            width: Some(video.width),
            height: Some(video.height),
            fps: None,
        }),
        (Some(video), _) => Ok(SourceProbeMetadata {
            source_path,
            media_type: SourceMediaType::Video,
            duration_seconds,
            width: Some(video.width),
            height: Some(video.height),
            fps: (video.fps.is_finite() && video.fps > 0.0).then_some(video.fps),
        }),
        (None, Some(_)) => Ok(SourceProbeMetadata {
            source_path,
            media_type: SourceMediaType::Audio,
            duration_seconds,
            width: None,
            height: None,
            fps: None,
        }),
        (None, None) => Err(SourceProbeError::Worker(
            "source contains no decodable audio or video stream".to_string(),
        )),
    }
}

fn probe_with_avfoundation_exporter(
    source_path: &Path,
) -> Result<SourceProbeMetadata, SourceProbeError> {
    let source_path = source_path
        .canonicalize()
        .map_err(|_| SourceProbeError::InvalidSource)?;
    if !source_path.is_file() {
        return Err(SourceProbeError::InvalidSource);
    }
    let source_path_text = source_path
        .to_str()
        .ok_or(SourceProbeError::InvalidSource)?;
    let request_id = format!("source-probe-{}", uuid::Uuid::new_v4());
    let request = SourceProbeRequest {
        protocol: PROTOCOL,
        schema_version: PROTOCOL_VERSION,
        request_id: &request_id,
        source_path: source_path_text,
    };
    let exporter = source_probe_executable()?;
    let mut child = Command::new(&exporter)
        .arg("--source-probe")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| SourceProbeError::Start(error.to_string()))?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        SourceProbeError::Request("source probe stdin was unavailable".to_string())
    })?;
    serde_json::to_writer(&mut stdin, &request)
        .map_err(|error| SourceProbeError::Request(error.to_string()))?;
    stdin
        .write_all(b"\n")
        .map_err(|error| SourceProbeError::Request(error.to_string()))?;
    drop(stdin);

    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| SourceProbeError::Start(error.to_string()))?
            .is_some()
        {
            break;
        }
        if started.elapsed() >= SOURCE_PROBE_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SourceProbeError::Worker(format!(
                "source probe timed out after {} seconds",
                SOURCE_PROBE_TIMEOUT.as_secs()
            )));
        }
        thread::sleep(SOURCE_PROBE_POLL_INTERVAL);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| SourceProbeError::Start(error.to_string()))?;
    if output.stdout.len() > MAX_PROTOCOL_BYTES || output.stderr.len() > MAX_PROTOCOL_BYTES {
        return Err(SourceProbeError::Protocol(
            "response exceeded the protocol byte budget".to_string(),
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|error| SourceProbeError::Protocol(error.to_string()))?;
    let lines = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(SourceProbeError::Protocol(format!(
            "expected one final event, received {}",
            lines.len()
        )));
    }
    let event: SourceProbeEvent = serde_json::from_str(lines[0])
        .map_err(|error| SourceProbeError::Protocol(error.to_string()))?;
    match event {
        SourceProbeEvent::Completed {
            protocol,
            schema_version,
            request_id: response_request_id,
            result,
        } if output.status.success() => {
            validate_envelope(
                &protocol,
                schema_version,
                response_request_id.as_deref(),
                &request_id,
            )?;
            if result.source_path != source_path_text {
                return Err(SourceProbeError::Protocol(
                    "result sourcePath did not match the request".to_string(),
                ));
            }
            validate_metadata(&result)?;
            Ok(result)
        }
        SourceProbeEvent::Failed {
            protocol,
            schema_version,
            request_id: response_request_id,
            error,
        } => {
            validate_envelope(
                &protocol,
                schema_version,
                response_request_id.as_deref(),
                &request_id,
            )?;
            Err(SourceProbeError::Worker(format!(
                "{}: {}{} (retryable={})",
                error.code,
                error.message,
                error
                    .field
                    .as_deref()
                    .map(|field| format!(" [{field}]"))
                    .unwrap_or_default(),
                error.retryable
            )))
        }
        SourceProbeEvent::Completed { .. } => Err(SourceProbeError::Protocol(
            "completed event returned a failing process status".to_string(),
        )),
    }
}

fn validate_envelope(
    protocol: &str,
    schema_version: u32,
    response_request_id: Option<&str>,
    request_id: &str,
) -> Result<(), SourceProbeError> {
    if protocol != PROTOCOL
        || schema_version != PROTOCOL_VERSION
        || response_request_id != Some(request_id)
    {
        return Err(SourceProbeError::Protocol(
            "protocol, schemaVersion, or requestId did not match".to_string(),
        ));
    }
    Ok(())
}

fn validate_metadata(metadata: &SourceProbeMetadata) -> Result<(), SourceProbeError> {
    if metadata
        .duration_seconds
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
        || metadata
            .fps
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        || metadata.width.is_some() != metadata.height.is_some()
        || metadata.width == Some(0)
        || metadata.height == Some(0)
    {
        return Err(SourceProbeError::Protocol(
            "metadata contained invalid numeric values".to_string(),
        ));
    }
    match metadata.media_type {
        SourceMediaType::Video
            if metadata.duration_seconds.is_none()
                || metadata.width.is_none()
                || metadata.fps.is_none() =>
        {
            Err(SourceProbeError::Protocol(
                "video metadata was incomplete".to_string(),
            ))
        }
        SourceMediaType::Audio if metadata.duration_seconds.is_none() => Err(
            SourceProbeError::Protocol("audio metadata was incomplete".to_string()),
        ),
        SourceMediaType::Image if metadata.width.is_none() => Err(SourceProbeError::Protocol(
            "image metadata was incomplete".to_string(),
        )),
        _ => Ok(()),
    }
}

fn source_probe_executable() -> Result<PathBuf, SourceProbeError> {
    if let Some(path) = std::env::var_os(EXPORTER_ENV_VAR) {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe()
        .map_err(|error| SourceProbeError::Unavailable(error.to_string()))?;
    let mut binary_dir = current.parent().unwrap_or_else(|| Path::new("."));
    if binary_dir.file_name().and_then(|value| value.to_str()) == Some("deps") {
        binary_dir = binary_dir.parent().unwrap_or(binary_dir);
    }
    Ok(binary_dir.join("video-creater-avfoundation-exporter"))
}

#[cfg(test)]
mod tests {
    use super::{probe_source_metadata, SourceMediaType};

    #[test]
    fn probes_content_detected_png_with_non_allowlisted_extension() {
        let directory = tempfile::tempdir().expect("temporary probe directory");
        let source_path = directory.path().join("system-decodable.visual");
        image::save_buffer_with_format(
            &source_path,
            &[0, 0, 0, 255],
            1,
            1,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .expect("write PNG bytes with a non-allowlisted extension");

        let metadata = probe_source_metadata(&source_path)
            .expect("source probe should use content detection rather than the extension");

        assert_eq!(metadata.media_type, SourceMediaType::Image);
        assert_eq!(metadata.width, Some(1));
        assert_eq!(metadata.height, Some(1));
    }
}
