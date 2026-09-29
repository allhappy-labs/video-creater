use super::safe_path_segment;
use super::visual_cache::write_json_atomically;
use crate::generation::fal::{
    cancel_fal_queue_request_with_client, fetch_fal_queue_result_with_client,
    fetch_fal_queue_status_with_client, submit_fal_queue_submission_with_client,
    upload_fal_local_file_to_cdn_with_client, FalQueueSubmission, FAL_PROVIDER, FAL_QUEUE_BASE_URL,
    FAL_REST_API_BASE_URL, VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR,
};
use crate::project::split::resolve_project_relative_path;
use crate::provider_credentials::resolve_provider_credential;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub const FAL_FLORENCE_CAPTION_MODEL_ID: &str = "fal-ai/florence-2-large/caption";
const MAX_CAPTION_BYTES: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisualCaptionReport {
    pub media_id: String,
    pub provider: String,
    pub model_id: String,
    pub captioned_frame_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualCaptionError {
    MissingSidecar(String),
    MissingFrame(String),
    InvalidSidecar(String),
    CaptionProvider(String),
    Cancelled,
    Io(String),
}

impl std::fmt::Display for VisualCaptionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSidecar(path) => {
                write!(formatter, "visual frame cache is missing: {path}")
            }
            Self::MissingFrame(path) => write!(formatter, "cached visual frame is missing: {path}"),
            Self::InvalidSidecar(message) => {
                write!(formatter, "visual frame cache is invalid: {message}")
            }
            Self::CaptionProvider(message) => {
                write!(formatter, "visual caption provider failed: {message}")
            }
            Self::Cancelled => formatter.write_str("visual captioning was cancelled"),
            Self::Io(message) => write!(formatter, "visual caption cache IO failed: {message}"),
        }
    }
}

impl std::error::Error for VisualCaptionError {}

pub trait VisualCaptionProvider {
    fn caption_frame(
        &self,
        frame_path: &Path,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<String, VisualCaptionError>;
}

#[derive(Debug, Clone, Copy)]
pub struct FalFlorenceCaptionProvider {
    pub max_status_polls: usize,
    pub poll_interval: Duration,
}

impl Default for FalFlorenceCaptionProvider {
    fn default() -> Self {
        Self {
            max_status_polls: 60,
            poll_interval: Duration::from_secs(1),
        }
    }
}

impl VisualCaptionProvider for FalFlorenceCaptionProvider {
    fn caption_frame(
        &self,
        frame_path: &Path,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<String, VisualCaptionError> {
        if is_cancelled() {
            return Err(VisualCaptionError::Cancelled);
        }
        let credential = resolve_provider_credential(FAL_PROVIDER)
            .map_err(|error| VisualCaptionError::CaptionProvider(error.to_string()))?
            .into_secret();
        let client = Client::new();
        let rest_api_base_url = std::env::var(VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR)
            .unwrap_or_else(|_| FAL_REST_API_BASE_URL.to_string());
        let image_url = upload_fal_local_file_to_cdn_with_client(
            &client,
            &rest_api_base_url,
            frame_path,
            &credential,
        )
        .map_err(|error| VisualCaptionError::CaptionProvider(error.to_string()))?;
        let submission = FalQueueSubmission {
            method: "POST".to_string(),
            url: format!("{FAL_QUEUE_BASE_URL}/{FAL_FLORENCE_CAPTION_MODEL_ID}"),
            endpoint: FAL_FLORENCE_CAPTION_MODEL_ID.to_string(),
            provider: FAL_PROVIDER.to_string(),
            input: json!({ "image_url": image_url }),
        };
        let submitted = submit_fal_queue_submission_with_client(&client, &submission, &credential)
            .map_err(|error| VisualCaptionError::CaptionProvider(error.to_string()))?;

        for poll_index in 0..self.max_status_polls.max(1) {
            if is_cancelled() {
                let _ = cancel_fal_queue_request_with_client(
                    &client,
                    &submitted.cancel_url,
                    &credential,
                );
                return Err(VisualCaptionError::Cancelled);
            }
            let status = fetch_fal_queue_status_with_client(
                &client,
                &submitted.status_url,
                &credential,
                true,
            )
            .map_err(|error| VisualCaptionError::CaptionProvider(error.to_string()))?;
            if status.is_terminal() {
                if let Some(error) = status.error.or(status.error_type) {
                    return Err(VisualCaptionError::CaptionProvider(error));
                }
                let result =
                    fetch_fal_queue_result_with_client(&client, &status.response_url, &credential)
                        .map_err(|error| VisualCaptionError::CaptionProvider(error.to_string()))?;
                return caption_from_fal_result(&result);
            }
            if poll_index + 1 < self.max_status_polls.max(1) && !self.poll_interval.is_zero() {
                thread::sleep(self.poll_interval);
            }
        }

        Err(VisualCaptionError::CaptionProvider(format!(
            "{FAL_FLORENCE_CAPTION_MODEL_ID} did not complete after {} polls",
            self.max_status_polls.max(1)
        )))
    }
}

pub fn caption_cached_visual_frames_with_fal(
    project_dir: &Path,
    media_id: &str,
    is_cancelled: impl Fn() -> bool,
) -> Result<VisualCaptionReport, VisualCaptionError> {
    caption_cached_visual_frames_with_provider(
        project_dir,
        media_id,
        &FalFlorenceCaptionProvider::default(),
        is_cancelled,
    )
}

pub fn caption_cached_visual_frames_with_provider(
    project_dir: &Path,
    media_id: &str,
    provider: &impl VisualCaptionProvider,
    is_cancelled: impl Fn() -> bool,
) -> Result<VisualCaptionReport, VisualCaptionError> {
    let sidecar_path = project_dir
        .join("search")
        .join("visual")
        .join(safe_path_segment(media_id))
        .join("frames.json");
    let mut sidecar = fs::read_to_string(&sidecar_path)
        .map_err(|_| VisualCaptionError::MissingSidecar(sidecar_path.display().to_string()))
        .and_then(|source| {
            serde_json::from_str::<Value>(&source)
                .map_err(|error| VisualCaptionError::InvalidSidecar(error.to_string()))
        })?;
    let frame_cache_ready = sidecar
        .get("frameCache")
        .and_then(|cache| cache.get("status"))
        .and_then(Value::as_str)
        == Some("ready");
    if !frame_cache_ready {
        return Err(VisualCaptionError::InvalidSidecar(
            "extract visual frames before requesting remote captions".to_string(),
        ));
    }
    let captioned_frame_count = {
        let frames = sidecar
            .get_mut("frames")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| VisualCaptionError::InvalidSidecar("frames are missing".to_string()))?;
        if frames.is_empty() {
            return Err(VisualCaptionError::InvalidSidecar(
                "frames are empty".to_string(),
            ));
        }

        for frame in frames.iter_mut() {
            if is_cancelled() {
                return Err(VisualCaptionError::Cancelled);
            }
            let relative_frame_path = frame
                .get("relativeFramePath")
                .and_then(Value::as_str)
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| {
                    VisualCaptionError::InvalidSidecar("frame path is missing".to_string())
                })?;
            let frame_path = resolve_project_relative_path(project_dir, relative_frame_path)
                .map_err(|error| VisualCaptionError::InvalidSidecar(error.to_string()))?;
            if !frame_path.is_file() {
                return Err(VisualCaptionError::MissingFrame(
                    frame_path.display().to_string(),
                ));
            }
            let caption = provider.caption_frame(&frame_path, &is_cancelled)?;
            let caption = validated_caption(&caption)?;
            let object = frame.as_object_mut().ok_or_else(|| {
                VisualCaptionError::InvalidSidecar("frame is not an object".to_string())
            })?;
            object.insert("caption".to_string(), Value::String(caption));
            object.insert(
                "captionModel".to_string(),
                Value::String(FAL_FLORENCE_CAPTION_MODEL_ID.to_string()),
            );
        }
        frames.len()
    };
    let object = sidecar.as_object_mut().ok_or_else(|| {
        VisualCaptionError::InvalidSidecar("sidecar is not an object".to_string())
    })?;
    object.insert("status".to_string(), json!("ready"));
    object.insert("visualStatus".to_string(), json!("ready"));
    object.insert("embeddingModel".to_string(), Value::Null);
    object.insert(
        "captionModel".to_string(),
        json!(FAL_FLORENCE_CAPTION_MODEL_ID),
    );
    object.insert("captionProvider".to_string(), json!("fal.ai"));
    write_json_atomically(&sidecar_path, &sidecar)
        .map_err(|error| VisualCaptionError::Io(error.to_string()))?;

    Ok(VisualCaptionReport {
        media_id: media_id.to_string(),
        provider: "fal.ai".to_string(),
        model_id: FAL_FLORENCE_CAPTION_MODEL_ID.to_string(),
        captioned_frame_count,
    })
}

fn caption_from_fal_result(result: &Value) -> Result<String, VisualCaptionError> {
    result
        .get("results")
        .and_then(Value::as_str)
        .map(validated_caption)
        .transpose()?
        .ok_or_else(|| {
            VisualCaptionError::CaptionProvider(
                "fal Florence response is missing string results".to_string(),
            )
        })
}

fn validated_caption(value: &str) -> Result<String, VisualCaptionError> {
    let caption = value.trim();
    if caption.is_empty() {
        return Err(VisualCaptionError::CaptionProvider(
            "caption response is blank".to_string(),
        ));
    }
    if caption.len() > MAX_CAPTION_BYTES {
        return Err(VisualCaptionError::CaptionProvider(format!(
            "caption exceeds {MAX_CAPTION_BYTES} bytes"
        )));
    }
    Ok(caption.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FixtureCaptionProvider {
        calls: AtomicUsize,
    }

    impl VisualCaptionProvider for FixtureCaptionProvider {
        fn caption_frame(
            &self,
            _frame_path: &Path,
            _is_cancelled: &dyn Fn() -> bool,
        ) -> Result<String, VisualCaptionError> {
            let index = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(format!("fixture caption {index}"))
        }
    }

    fn write_cached_sidecar(project_dir: &Path) {
        let frames_dir = project_dir.join("search/visual/cache-key/frames");
        fs::create_dir_all(&frames_dir).expect("frames directory");
        for index in 0..2 {
            fs::write(frames_dir.join(format!("frame-{index:06}.png")), b"frame")
                .expect("fixture frame");
        }
        let sidecar_dir = project_dir.join("search/visual/media-1");
        fs::create_dir_all(&sidecar_dir).expect("sidecar directory");
        fs::write(
            sidecar_dir.join("frames.json"),
            serde_json::to_vec_pretty(&json!({
                "status": "notInstalled",
                "visualStatus": "notInstalled",
                "frameCache": { "status": "ready" },
                "frames": [
                    { "relativeFramePath": "search/visual/cache-key/frames/frame-000000.png" },
                    { "relativeFramePath": "search/visual/cache-key/frames/frame-000001.png" }
                ]
            }))
            .expect("serialize sidecar"),
        )
        .expect("write sidecar");
    }

    #[test]
    fn captions_cached_frames_then_enables_caption_based_visual_search() {
        let temporary = tempfile::tempdir().expect("temporary project");
        write_cached_sidecar(temporary.path());
        let provider = FixtureCaptionProvider {
            calls: AtomicUsize::new(0),
        };

        let report = caption_cached_visual_frames_with_provider(
            temporary.path(),
            "media-1",
            &provider,
            || false,
        )
        .expect("caption cache");
        assert_eq!(report.captioned_frame_count, 2);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        let sidecar: Value = serde_json::from_str(
            &fs::read_to_string(temporary.path().join("search/visual/media-1/frames.json"))
                .expect("sidecar"),
        )
        .expect("sidecar JSON");
        assert_eq!(sidecar["visualStatus"], "ready");
        assert_eq!(sidecar["embeddingModel"], Value::Null);
        assert_eq!(sidecar["frames"][1]["caption"], "fixture caption 1");
    }

    #[test]
    fn cancellation_does_not_publish_partial_captions() {
        let temporary = tempfile::tempdir().expect("temporary project");
        write_cached_sidecar(temporary.path());
        let provider = FixtureCaptionProvider {
            calls: AtomicUsize::new(0),
        };

        let error = caption_cached_visual_frames_with_provider(
            temporary.path(),
            "media-1",
            &provider,
            || provider.calls.load(Ordering::SeqCst) >= 1,
        )
        .expect_err("cancel captioning");
        assert_eq!(error, VisualCaptionError::Cancelled);
        let sidecar: Value = serde_json::from_str(
            &fs::read_to_string(temporary.path().join("search/visual/media-1/frames.json"))
                .expect("sidecar"),
        )
        .expect("sidecar JSON");
        assert_eq!(sidecar["visualStatus"], "notInstalled");
        assert!(sidecar["frames"][0].get("caption").is_none());
    }
}
