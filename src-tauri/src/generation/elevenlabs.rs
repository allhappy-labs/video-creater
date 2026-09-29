use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    read_response_bounded, stream_response_to_atomic_file, AUDIO_OUTPUT_LIMIT,
};
use crate::generation::GenerationTarget;
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const ELEVENLABS_PROVIDER: &str = "elevenlabs";
pub const ELEVENLABS_TTS_V3_MODEL_ID: &str = "elevenlabs-tts-v3";
pub const ELEVENLABS_API_MODEL_ID: &str = "eleven_v3";
pub const ELEVENLABS_MUSIC_MODEL_ID: &str = "elevenlabs-music";
pub const ELEVENLABS_MUSIC_API_MODEL_ID: &str = "music_v2";
pub const ELEVENLABS_DEFAULT_VOICE: &str = "rachel";
pub const ELEVENLABS_DEFAULT_VOICE_ID: &str = "21m00Tcm4TlvDq8ikWAM";
pub const ELEVENLABS_API_BASE_URL: &str = "https://api.elevenlabs.io/v1";
pub const VIDEO_CREATER_ELEVENLABS_API_BASE_URL_ENV_VAR: &str =
    "VIDEO_CREATER_ELEVENLABS_API_BASE_URL";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevenLabsGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElevenLabsStatusKind {
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElevenLabsGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElevenLabsGenerationRun {
    pub request_id: String,
    pub status: ElevenLabsStatusKind,
    pub completion: ElevenLabsGenerationCompletion,
}

#[derive(Debug, Error, PartialEq)]
pub enum ElevenLabsGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported elevenlabs generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("generated asset cannot be completed by elevenlabs worker: {0}")]
    NotPending(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum ElevenLabsGenerationError {
    #[error("unsupported elevenlabs credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("elevenlabs credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("elevenlabs generation request failed: {message}")]
    RequestFailed { message: String },
    #[error("elevenlabs generation returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("elevenlabs generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("elevenlabs generated output write failed: {message}")]
    WriteOutputFailed { message: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum ElevenLabsGenerationWorkerError {
    #[error("elevenlabs generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] ElevenLabsGenerationProviderError),
    #[error(transparent)]
    Request(#[from] ElevenLabsGenerationError),
}

pub fn build_elevenlabs_generation_submission(
    asset: &GeneratedAsset,
) -> Result<ElevenLabsGenerationSubmission, ElevenLabsGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(ElevenLabsGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != ELEVENLABS_PROVIDER
        || !matches!(
            model_id,
            ELEVENLABS_TTS_V3_MODEL_ID | ELEVENLABS_MUSIC_MODEL_ID
        )
    {
        return Err(ElevenLabsGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    if model_id == ELEVENLABS_MUSIC_MODEL_ID {
        let mut input = json!({
            "prompt": elevenlabs_music_prompt(asset, prompt),
            "model_id": ELEVENLABS_MUSIC_API_MODEL_ID,
            "force_instrumental": asset.settings.instrumental.unwrap_or(false)
        });
        if let Some(duration_ms) = elevenlabs_music_duration_ms(asset) {
            input["music_length_ms"] = json!(duration_ms);
        }
        Ok(ElevenLabsGenerationSubmission {
            method: "POST".to_string(),
            url: format!("{ELEVENLABS_API_BASE_URL}/music"),
            model: ELEVENLABS_MUSIC_MODEL_ID.to_string(),
            provider: ELEVENLABS_PROVIDER.to_string(),
            input,
        })
    } else {
        let voice_id = elevenlabs_voice_id(asset);
        Ok(ElevenLabsGenerationSubmission {
            method: "POST".to_string(),
            url: format!("{ELEVENLABS_API_BASE_URL}/text-to-speech/{voice_id}"),
            model: ELEVENLABS_TTS_V3_MODEL_ID.to_string(),
            provider: ELEVENLABS_PROVIDER.to_string(),
            input: json!({
                "text": prompt,
                "model_id": ELEVENLABS_API_MODEL_ID
            }),
        })
    }
}

pub fn run_elevenlabs_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &ElevenLabsGenerationSubmission,
    credential: &str,
) -> Result<ElevenLabsGenerationRun, ElevenLabsGenerationWorkerError> {
    run_elevenlabs_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_elevenlabs_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &ElevenLabsGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<ElevenLabsGenerationRun, ElevenLabsGenerationWorkerError> {
    ensure_elevenlabs_generation_not_cancelled(cancellation)?;
    let relative_path = elevenlabs_default_output_relative_path(target.asset)?;
    let output_path = safe_elevenlabs_generated_output_path(target.project_dir, &relative_path)?;
    let response = send_elevenlabs_generation_with_client(client, submission, credential)?;
    let observed =
        stream_response_to_atomic_file(response, &output_path, AUDIO_OUTPUT_LIMIT, || {
            cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
        })
        .map_err(|error| match error {
            crate::generation::download::DownloadError::Cancelled => {
                ElevenLabsGenerationWorkerError::Cancelled
            }
            crate::generation::download::DownloadError::HttpStatus { status } => {
                ElevenLabsGenerationError::HttpStatus { status }.into()
            }
            other => ElevenLabsGenerationError::RequestFailed {
                message: other.to_string(),
            }
            .into(),
        })?;
    if observed == 0 {
        let _ = fs::remove_file(&output_path);
        return Err(ElevenLabsGenerationError::RequestFailed {
            message: "provider returned empty audio".into(),
        }
        .into());
    }
    let actions = build_elevenlabs_generation_completion_actions(
        target.asset,
        &relative_path,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(ElevenLabsGenerationRun {
        request_id: format!("elevenlabs-{}", target.asset.id),
        status: ElevenLabsStatusKind::Completed,
        completion: ElevenLabsGenerationCompletion {
            output_path,
            actions,
        },
    })
}

fn ensure_elevenlabs_generation_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), ElevenLabsGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        Err(ElevenLabsGenerationWorkerError::Cancelled)
    } else {
        Ok(())
    }
}

pub fn submit_elevenlabs_generation_with_client(
    client: &Client,
    submission: &ElevenLabsGenerationSubmission,
    credential: &str,
) -> Result<Vec<u8>, ElevenLabsGenerationError> {
    let response = send_elevenlabs_generation_with_client(client, submission, credential)?;
    decode_elevenlabs_binary(response)
}

fn send_elevenlabs_generation_with_client(
    client: &Client,
    submission: &ElevenLabsGenerationSubmission,
    credential: &str,
) -> Result<reqwest::blocking::Response, ElevenLabsGenerationError> {
    ensure_elevenlabs_credential(&submission.provider, credential)?;
    let request_body = serde_json::to_vec(&submission.input).map_err(|error| {
        ElevenLabsGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;

    client
        .post(&submission.url)
        .query(&[("output_format", elevenlabs_output_format(submission))])
        .header("xi-api-key", credential.trim())
        .header(ACCEPT, "audio/mpeg")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| ElevenLabsGenerationError::RequestFailed {
            message: error.to_string(),
        })
}

pub fn elevenlabs_generation_client() -> Result<Client, ElevenLabsGenerationError> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| ElevenLabsGenerationError::RequestFailed {
            message: error.to_string(),
        })
}

fn decode_elevenlabs_binary(
    response: reqwest::blocking::Response,
) -> Result<Vec<u8>, ElevenLabsGenerationError> {
    if !response.status().is_success() {
        return Err(ElevenLabsGenerationError::HttpStatus {
            status: response.status().as_u16(),
        });
    }
    read_response_bounded(response, AUDIO_OUTPUT_LIMIT, || false).map_err(|error| {
        ElevenLabsGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })
}

fn ensure_elevenlabs_credential(
    provider: &str,
    credential: &str,
) -> Result<(), ElevenLabsGenerationError> {
    if provider.trim() != ELEVENLABS_PROVIDER {
        return Err(ElevenLabsGenerationError::UnsupportedCredentialProvider {
            provider: provider.to_string(),
        });
    }
    if credential.trim().is_empty() {
        return Err(ElevenLabsGenerationError::EmptyCredential {
            provider: ELEVENLABS_PROVIDER.to_string(),
        });
    }
    Ok(())
}

pub fn build_elevenlabs_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, ElevenLabsGenerationProviderError> {
    if asset.status != GeneratedAssetStatus::Queued && asset.status != GeneratedAssetStatus::Running
    {
        return Err(ElevenLabsGenerationProviderError::NotPending(format!(
            "{:?}",
            asset.status
        )));
    }

    let output = ProjectActionGeneratedAssetOutput {
        media_id: elevenlabs_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: None,
        width: 0,
        height: 0,
        duration_seconds: elevenlabs_output_duration_seconds(asset),
        fps: 0.0,
    };
    let generated_output_media_id = output.media_id.clone();
    let replacement = replacement_item_id.map(|item_id| ProjectActionReplaceGeneratedOutput {
        item_id: item_id.to_string(),
        media_id: generated_output_media_id.clone(),
    });
    let completion = ProjectActionGeneratedAssetCompletion {
        generated_asset_id: asset.id.clone(),
        generated_output_media_id,
        placement_intent: asset
            .placement_intent
            .clone()
            .unwrap_or_else(|| "library".to_string()),
        references: ProjectActionGeneratedAssetReferences {
            media_ids: asset.references.media_ids.clone(),
            source_video_media_ref: asset.references.source_video_media_ref.clone(),
            first_frame_media_id: asset.references.first_frame_media_id.clone(),
            last_frame_media_id: asset.references.last_frame_media_id.clone(),
            reference_image_media_refs: asset.references.reference_image_media_refs.clone(),
            reference_video_media_refs: asset.references.reference_video_media_refs.clone(),
            reference_audio_media_refs: asset.references.reference_audio_media_refs.clone(),
            provider_input_urls: asset.references.provider_input_urls.clone(),
        },
    };

    Ok(vec![
        ProjectAction::CompleteGeneratedAsset {
            asset_id: asset.id.clone(),
            outputs: vec![output],
            completion: Some(completion),
            replacement,
        },
        ProjectAction::UpdateJobStatus {
            job_id: asset.id.clone(),
            status: JobStatus::Completed,
            updated_at: updated_at.to_string(),
            run_id: run_id.map(str::to_string),
        },
    ])
}

fn safe_elevenlabs_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, ElevenLabsGenerationError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || !relative.starts_with("generated")
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(ElevenLabsGenerationError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }
    Ok(project_dir.join(relative))
}

fn elevenlabs_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, ElevenLabsGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != ELEVENLABS_PROVIDER
        || !matches!(
            model_id,
            ELEVENLABS_TTS_V3_MODEL_ID | ELEVENLABS_MUSIC_MODEL_ID
        )
    {
        return Err(ElevenLabsGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }
    Ok(format!(
        "generated/{}/{}.mp3",
        asset.id,
        elevenlabs_output_basename(asset)
    ))
}

fn elevenlabs_voice_id(asset: &GeneratedAsset) -> String {
    let voice = asset
        .settings
        .voice
        .as_deref()
        .map(str::trim)
        .filter(|voice| !voice.is_empty())
        .unwrap_or(ELEVENLABS_DEFAULT_VOICE);
    if voice.eq_ignore_ascii_case(ELEVENLABS_DEFAULT_VOICE) {
        ELEVENLABS_DEFAULT_VOICE_ID.to_string()
    } else {
        voice.to_string()
    }
}

fn elevenlabs_output_duration_seconds(asset: &GeneratedAsset) -> f64 {
    let default_duration = if asset.model.id.trim() == ELEVENLABS_MUSIC_MODEL_ID {
        30.0
    } else {
        12.0
    };
    asset
        .settings
        .duration_seconds
        .unwrap_or(default_duration)
        .max(0.0)
}

fn elevenlabs_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-{}", asset.id, elevenlabs_output_basename(asset))
}

fn elevenlabs_output_basename(asset: &GeneratedAsset) -> &'static str {
    if asset.model.id.trim() == ELEVENLABS_MUSIC_MODEL_ID {
        "elevenlabs-music-output"
    } else {
        "elevenlabs-output"
    }
}

fn elevenlabs_output_format(submission: &ElevenLabsGenerationSubmission) -> &'static str {
    if submission.model.trim() == ELEVENLABS_MUSIC_MODEL_ID {
        "mp3_48000_192"
    } else {
        "mp3_44100_128"
    }
}

fn elevenlabs_music_duration_ms(asset: &GeneratedAsset) -> Option<u64> {
    let seconds = asset.settings.duration_seconds?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    Some((seconds * 1000.0).round() as u64)
}

fn elevenlabs_music_prompt(asset: &GeneratedAsset, prompt: &str) -> String {
    let mut parts = vec![prompt.trim().to_string()];
    if let Some(style) = asset
        .settings
        .style_instructions
        .as_deref()
        .map(str::trim)
        .filter(|style| !style.is_empty())
    {
        parts.push(format!("Style: {style}"));
    }
    if let Some(lyrics) = asset
        .settings
        .lyrics
        .as_deref()
        .map(str::trim)
        .filter(|lyrics| !lyrics.is_empty())
    {
        parts.push(format!("Lyrics:\n{lyrics}"));
    }
    parts.join("\n\n")
}
