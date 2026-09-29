use serde::{Deserialize, Serialize};

use super::error::ServiceError;

const DURATION_TOLERANCE_SECONDS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderReviewEvidence {
    pub expected_duration_seconds: f64,
    pub actual_duration_seconds: f64,
    pub has_video_stream: bool,
    pub has_audio_stream: bool,
    pub captions_aligned: bool,
    pub overlays_aligned: bool,
    pub artifact_id: Option<String>,
    pub log_reference: Option<String>,
}

pub struct ExportService;

impl ExportService {
    pub fn validate_render_review(evidence: &RenderReviewEvidence) -> Result<(), ServiceError> {
        if !evidence.expected_duration_seconds.is_finite()
            || !evidence.actual_duration_seconds.is_finite()
            || evidence.expected_duration_seconds <= 0.0
            || (evidence.actual_duration_seconds - evidence.expected_duration_seconds).abs()
                > DURATION_TOLERANCE_SECONDS
        {
            return Err(ServiceError::invalid_input(
                "render duration does not match the approved timeline",
            ));
        }
        if !evidence.has_video_stream || !evidence.has_audio_stream {
            return Err(ServiceError::invalid_input(
                "render must contain video and audio streams",
            ));
        }
        if !evidence.captions_aligned || !evidence.overlays_aligned {
            return Err(ServiceError::invalid_input(
                "render caption or overlay timing is not aligned",
            ));
        }
        if evidence.artifact_id.as_deref().is_none_or(str::is_empty)
            || evidence.log_reference.as_deref().is_none_or(str::is_empty)
        {
            return Err(ServiceError::invalid_input(
                "render artifact and log references are required",
            ));
        }
        Ok(())
    }
}
