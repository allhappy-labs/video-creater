use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    decode_hex_bounded, read_response_bounded, write_bytes_atomically, AUDIO_ENVELOPE_LIMIT,
    AUDIO_OUTPUT_LIMIT,
};
use crate::generation::GenerationTarget;
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const MINIMAX_PROVIDER: &str = "minimax";
pub const MINIMAX_MUSIC_MODEL_ID: &str = "minimax-music-v2.6";
pub const MINIMAX_MUSIC_API_MODEL_ID: &str = "music-2.6";
pub const MINIMAX_API_BASE_URL: &str = "https://api.minimax.io";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimaxGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub api_model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimaxStatusKind {
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MinimaxGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MinimaxGenerationRun {
    pub request_id: String,
    pub status: MinimaxStatusKind,
    pub completion: MinimaxGenerationCompletion,
}

#[derive(Debug, Error, PartialEq)]
pub enum MinimaxGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported minimax generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("generated asset cannot be completed by minimax worker: {0}")]
    NotPending(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum MinimaxGenerationError {
    #[error("unsupported minimax credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("minimax credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("minimax generation request failed: {message}")]
    RequestFailed { message: String },
    #[error("minimax generation returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("minimax generation response failed: {message}")]
    DecodeResponse { message: String },
    #[error("minimax generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("minimax generated output write failed: {message}")]
    WriteOutputFailed { message: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum MinimaxGenerationWorkerError {
    #[error("minimax generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] MinimaxGenerationProviderError),
    #[error(transparent)]
    Request(#[from] MinimaxGenerationError),
}

#[derive(Debug, Deserialize)]
struct MinimaxMusicResponse {
    data: Option<MinimaxMusicData>,
    #[serde(default)]
    extra_info: Option<MinimaxMusicExtraInfo>,
    #[serde(default)]
    base_resp: Option<MinimaxBaseResponse>,
}

#[derive(Debug, Deserialize)]
struct MinimaxMusicData {
    audio: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MinimaxMusicExtraInfo {
    music_duration: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct MinimaxBaseResponse {
    status_code: Option<i64>,
    status_msg: Option<String>,
}

pub fn build_minimax_generation_submission(
    asset: &GeneratedAsset,
) -> Result<MinimaxGenerationSubmission, MinimaxGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(MinimaxGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != MINIMAX_PROVIDER || model_id != MINIMAX_MUSIC_MODEL_ID {
        return Err(MinimaxGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let lyrics = trimmed_setting(asset.settings.lyrics.as_deref());
    let instrumental = asset.settings.instrumental.unwrap_or(false);
    let mut input = json!({
        "model": MINIMAX_MUSIC_API_MODEL_ID,
        "prompt": minimax_music_prompt(asset, prompt),
        "output_format": "hex",
        "lyrics_optimizer": !instrumental && lyrics.is_none(),
        "is_instrumental": instrumental,
        "audio_setting": {
            "sample_rate": 44100,
            "bitrate": 256000,
            "format": "mp3"
        }
    });
    if let Some(lyrics) = lyrics {
        input["lyrics"] = json!(lyrics);
    }

    Ok(MinimaxGenerationSubmission {
        method: "POST".to_string(),
        url: format!("{MINIMAX_API_BASE_URL}/v1/music_generation"),
        model: MINIMAX_MUSIC_MODEL_ID.to_string(),
        api_model: MINIMAX_MUSIC_API_MODEL_ID.to_string(),
        provider: MINIMAX_PROVIDER.to_string(),
        input,
    })
}

pub fn run_minimax_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &MinimaxGenerationSubmission,
    credential: &str,
) -> Result<MinimaxGenerationRun, MinimaxGenerationWorkerError> {
    run_minimax_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_minimax_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &MinimaxGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<MinimaxGenerationRun, MinimaxGenerationWorkerError> {
    ensure_minimax_generation_not_cancelled(cancellation)?;
    let response = submit_minimax_generation_with_client_cancellable(
        client,
        submission,
        credential,
        cancellation,
    )?;
    ensure_minimax_generation_not_cancelled(cancellation)?;
    let audio_bytes = minimax_music_output_bytes(&response)?;
    let duration_seconds = minimax_music_output_duration_seconds(target.asset, &response);
    let relative_path = minimax_default_output_relative_path(target.asset)?;
    let output_path = safe_minimax_generated_output_path(target.project_dir, &relative_path)?;
    write_minimax_output(&output_path, &audio_bytes)?;
    if ensure_minimax_generation_not_cancelled(cancellation).is_err() {
        let _ = fs::remove_file(&output_path);
        return Err(MinimaxGenerationWorkerError::Cancelled);
    }
    let actions = build_minimax_generation_completion_actions(
        target.asset,
        &relative_path,
        duration_seconds,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(MinimaxGenerationRun {
        request_id: format!("minimax-{}", target.asset.id),
        status: MinimaxStatusKind::Completed,
        completion: MinimaxGenerationCompletion {
            output_path,
            actions,
        },
    })
}

fn ensure_minimax_generation_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), MinimaxGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        Err(MinimaxGenerationWorkerError::Cancelled)
    } else {
        Ok(())
    }
}

fn submit_minimax_generation_with_client_cancellable(
    client: &Client,
    submission: &MinimaxGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<MinimaxMusicResponse, MinimaxGenerationWorkerError> {
    let response = send_minimax_generation_with_client(client, submission, credential)?;
    decode_minimax_json_cancellable(response, cancellation)
}

fn send_minimax_generation_with_client(
    client: &Client,
    submission: &MinimaxGenerationSubmission,
    credential: &str,
) -> Result<reqwest::blocking::Response, MinimaxGenerationError> {
    ensure_minimax_credential(&submission.provider, credential)?;
    let request_body = serde_json::to_vec(&submission.input).map_err(|error| {
        MinimaxGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;

    client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| MinimaxGenerationError::RequestFailed {
            message: error.to_string(),
        })
}

pub fn minimax_generation_client() -> Result<Client, MinimaxGenerationError> {
    Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| MinimaxGenerationError::RequestFailed {
            message: error.to_string(),
        })
}

fn decode_minimax_json_cancellable(
    response: reqwest::blocking::Response,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<MinimaxMusicResponse, MinimaxGenerationWorkerError> {
    let body = read_response_bounded(response, AUDIO_ENVELOPE_LIMIT, || {
        cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
    })
    .map_err(|error| match error {
        crate::generation::download::DownloadError::Cancelled => {
            MinimaxGenerationWorkerError::Cancelled
        }
        crate::generation::download::DownloadError::HttpStatus { status } => {
            MinimaxGenerationError::HttpStatus { status }.into()
        }
        other => MinimaxGenerationError::RequestFailed {
            message: other.to_string(),
        }
        .into(),
    })?;
    let parsed =
        serde_json::from_slice(&body).map_err(|error| MinimaxGenerationError::DecodeResponse {
            message: error.to_string(),
        })?;
    validate_minimax_response(parsed).map_err(Into::into)
}

fn validate_minimax_response(
    parsed: MinimaxMusicResponse,
) -> Result<MinimaxMusicResponse, MinimaxGenerationError> {
    if let Some(base_resp) = &parsed.base_resp {
        if base_resp.status_code.is_some_and(|status| status != 0) {
            return Err(MinimaxGenerationError::DecodeResponse {
                message: base_resp
                    .status_msg
                    .clone()
                    .unwrap_or_else(|| "MiniMax response reported failure".to_string()),
            });
        }
    }
    Ok(parsed)
}

fn minimax_music_output_bytes(
    response: &MinimaxMusicResponse,
) -> Result<Vec<u8>, MinimaxGenerationError> {
    let audio = response
        .data
        .as_ref()
        .and_then(|data| data.audio.as_deref())
        .map(str::trim)
        .filter(|audio| !audio.is_empty())
        .ok_or_else(|| MinimaxGenerationError::DecodeResponse {
            message: "MiniMax response missing data.audio".to_string(),
        })?;
    decode_hex_bounded(audio, AUDIO_ENVELOPE_LIMIT, AUDIO_OUTPUT_LIMIT).map_err(|error| {
        MinimaxGenerationError::DecodeResponse {
            message: error.to_string(),
        }
    })
}

fn ensure_minimax_credential(
    provider: &str,
    credential: &str,
) -> Result<(), MinimaxGenerationError> {
    if provider.trim() != MINIMAX_PROVIDER {
        return Err(MinimaxGenerationError::UnsupportedCredentialProvider {
            provider: provider.to_string(),
        });
    }
    if credential.trim().is_empty() {
        return Err(MinimaxGenerationError::EmptyCredential {
            provider: MINIMAX_PROVIDER.to_string(),
        });
    }
    Ok(())
}

fn minimax_music_prompt(asset: &GeneratedAsset, prompt: &str) -> String {
    if let Some(style) = trimmed_setting(asset.settings.style_instructions.as_deref()) {
        format!("{}\n\nStyle: {}", prompt.trim(), style)
    } else {
        prompt.trim().to_string()
    }
}

fn trimmed_setting(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn minimax_music_output_duration_seconds(
    asset: &GeneratedAsset,
    response: &MinimaxMusicResponse,
) -> f64 {
    response
        .extra_info
        .as_ref()
        .and_then(|extra| extra.music_duration)
        .filter(|duration_ms| duration_ms.is_finite() && *duration_ms > 0.0)
        .map(|duration_ms| duration_ms / 1000.0)
        .or(asset.settings.duration_seconds)
        .unwrap_or(120.0)
        .max(0.0)
}

pub fn build_minimax_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    duration_seconds: f64,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, MinimaxGenerationProviderError> {
    if asset.status != GeneratedAssetStatus::Queued && asset.status != GeneratedAssetStatus::Running
    {
        return Err(MinimaxGenerationProviderError::NotPending(format!(
            "{:?}",
            asset.status
        )));
    }

    let output = ProjectActionGeneratedAssetOutput {
        media_id: minimax_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: None,
        width: 0,
        height: 0,
        duration_seconds,
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
            first_frame_media_id: asset.references.first_frame_media_id.clone(),
            last_frame_media_id: asset.references.last_frame_media_id.clone(),
            source_video_media_ref: asset.references.source_video_media_ref.clone(),
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

fn minimax_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-minimax-music-output", asset.id)
}

fn minimax_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, MinimaxGenerationError> {
    Ok(format!(
        "generated/{}/minimax-music-output.mp3",
        sanitize_minimax_path_component(&asset.id)
    ))
}

fn safe_minimax_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, MinimaxGenerationError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || !relative.starts_with("generated")
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(MinimaxGenerationError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }
    Ok(project_dir.join(relative))
}

fn sanitize_minimax_path_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "asset".to_string()
    } else {
        sanitized
    }
}

fn write_minimax_output(path: &Path, bytes: &[u8]) -> Result<(), MinimaxGenerationError> {
    write_bytes_atomically(path, bytes).map_err(|error| MinimaxGenerationError::WriteOutputFailed {
        message: error.to_string(),
    })
}
