use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::edit::preset::EditJobRequest;
use crate::project::model::{MediaKind, VideoProject};
use crate::transcription::model::ModelInstallStatus;
use crate::transcription::runtime::{NativeTranscriptionOutput, RuntimeSelection};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalTranscribeMediaWorkflowInput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub language_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalTranscribeProbeOutput {
    pub status: TemporalTranscribeProbeStatus,
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub language_mode: String,
    pub source_path: String,
    pub artifact_path: String,
    pub media_kind: MediaKind,
    pub media_relative_path: String,
    pub media_duration_seconds: f64,
    pub media_width: Option<u32>,
    pub media_height: Option<u32>,
    pub media_fps: Option<f64>,
    pub model_id: String,
    pub model_path: String,
    pub runtime_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TemporalTranscribeProbeStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalTranscribeRunOutput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub language_mode: String,
    pub model_id: String,
    pub runtime_id: String,
    pub artifact_path: String,
    pub token_count: usize,
    pub native_output: NativeTranscriptionOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalStoreTranscriptOutput {
    pub project_id: String,
    pub project_dir: String,
    pub media_id: String,
    pub job_id: String,
    pub transcript_id: String,
    pub segment_count: usize,
    pub word_count: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TranscriptionJobError {
    #[error("transcription model is not ready")]
    ModelNotReady,
    #[error("transcription runtime is unavailable")]
    RuntimeUnavailable,
}

pub fn validate_transcription_ready_for_generate_edit(
    project: &VideoProject,
    request: &EditJobRequest,
    install_status: ModelInstallStatus,
    runtime_selection: RuntimeSelection,
) -> Result<(), TranscriptionJobError> {
    if project
        .transcripts
        .iter()
        .any(|transcript| transcript.media_id == request.media_id)
    {
        return Ok(());
    }

    if install_status != ModelInstallStatus::Ready {
        return Err(TranscriptionJobError::ModelNotReady);
    }

    match runtime_selection {
        RuntimeSelection::Native => Ok(()),
        RuntimeSelection::UnsupportedPlatform | RuntimeSelection::Unavailable => {
            Err(TranscriptionJobError::RuntimeUnavailable)
        }
    }
}
