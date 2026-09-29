use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    read_response_bounded, stream_response_to_atomic_file, IMAGE_OUTPUT_LIMIT, JSON_RESPONSE_LIMIT,
    VIDEO_OUTPUT_LIMIT,
};
use crate::generation::GenerationTarget;
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::job_progress::JobProgressReporter;
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use thiserror::Error;

pub const XAI_PROVIDER: &str = "xai";
pub const XAI_GROK_IMAGE_QUALITY_MODEL_ID: &str = "grok-imagine-image-quality";
pub const XAI_GROK_VIDEO_MODEL_ID: &str = "grok-imagine-video";
pub const XAI_API_BASE_URL: &str = "https://api.x.ai/v1";
pub const VIDEO_CREATER_XAI_API_BASE_URL_ENV_VAR: &str = "VIDEO_CREATER_XAI_API_BASE_URL";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XAiImageGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XAiImageStatusKind {
    Completed,
    Done,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XAiImageGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XAiImageGenerationRun {
    pub request_id: String,
    pub status: XAiImageStatusKind,
    pub completion: XAiImageGenerationCompletion,
}

#[derive(Debug, Error, PartialEq)]
pub enum XAiGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported xai generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("xai generation result is missing {0}")]
    MissingResultField(String),
    #[error("xai generation result has invalid {field}: {value}")]
    InvalidResultField { field: String, value: String },
    #[error("xai image edit requires at least one prepared image input")]
    MissingImageEditInput,
    #[error("xai video result status was terminal: {status}")]
    TerminalVideoStatus { status: String },
    #[error("generated asset cannot be completed by xai worker: {0}")]
    NotPending(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum XAiGenerationError {
    #[error("unsupported xai credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("xai credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("xai generation request failed: {message}")]
    RequestFailed { message: String },
    #[error("xai generation returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("xai generation response could not be decoded: {message}")]
    DecodeResponse { message: String },
    #[error("xai generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("xai generated output write failed: {message}")]
    WriteOutputFailed { message: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum XAiGenerationWorkerError {
    #[error("xai generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] XAiGenerationProviderError),
    #[error(transparent)]
    Request(#[from] XAiGenerationError),
}

#[derive(Debug, Deserialize)]
pub struct XAiImageResponse {
    data: Vec<XAiImageData>,
}

#[derive(Debug, Deserialize)]
struct XAiImageData {
    url: Option<String>,
    #[serde(default)]
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XAiVideoStartResponse {
    request_id: String,
}

#[derive(Debug, Deserialize)]
struct XAiVideoStatusResponse {
    status: String,
    #[serde(default)]
    video: Option<XAiVideoData>,
    /// Provider-reported progress, 0–100.
    #[serde(default)]
    progress: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct XAiVideoData {
    url: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

pub fn build_xai_image_generation_submission(
    asset: &GeneratedAsset,
) -> Result<XAiImageGenerationSubmission, XAiGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(XAiGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != XAI_PROVIDER || model_id != XAI_GROK_IMAGE_QUALITY_MODEL_ID {
        return Err(XAiGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let mut input = json!({
        "prompt": prompt,
        "response_format": "url",
        "n": 1
    });
    if let Some(aspect_ratio) = xai_aspect_ratio(asset) {
        input
            .as_object_mut()
            .expect("xAI input should be an object")
            .insert("aspect_ratio".to_string(), json!(aspect_ratio));
    }

    if !asset.references.provider_input_urls.is_empty() {
        let image_url = asset
            .references
            .provider_input_urls
            .iter()
            .map(|url| url.trim())
            .find(|url| !url.is_empty())
            .ok_or(XAiGenerationProviderError::MissingImageEditInput)?;
        input
            .as_object_mut()
            .expect("xAI edit input should be an object")
            .insert(
                "image".to_string(),
                json!({
                    "url": image_url,
                    "type": "image_url"
                }),
            );
        return Ok(XAiImageGenerationSubmission {
            method: "POST".to_string(),
            url: format!("{XAI_API_BASE_URL}/images/edits"),
            model: XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
            provider: XAI_PROVIDER.to_string(),
            input,
        });
    }

    Ok(XAiImageGenerationSubmission {
        method: "POST".to_string(),
        url: format!("{XAI_API_BASE_URL}/images/generations"),
        model: XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
        provider: XAI_PROVIDER.to_string(),
        input,
    })
}

pub fn build_xai_video_generation_submission(
    asset: &GeneratedAsset,
) -> Result<XAiImageGenerationSubmission, XAiGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(XAiGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != XAI_PROVIDER || model_id != XAI_GROK_VIDEO_MODEL_ID {
        return Err(XAiGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let mut url = format!("{XAI_API_BASE_URL}/videos/generations");
    let is_source_video_edit = asset.references.source_video_media_ref.is_some();
    let mut input = json!({
        "prompt": prompt
    });
    if !is_source_video_edit {
        if let Some(duration_seconds) = xai_video_duration(asset) {
            input
                .as_object_mut()
                .expect("xAI video input should be an object")
                .insert("duration".to_string(), json!(duration_seconds));
        }
        if let Some(aspect_ratio) = xai_aspect_ratio(asset) {
            input
                .as_object_mut()
                .expect("xAI video input should be an object")
                .insert("aspect_ratio".to_string(), json!(aspect_ratio));
        }
        if let Some(resolution) = asset
            .settings
            .resolution
            .as_deref()
            .map(str::trim)
            .filter(|resolution| !resolution.is_empty())
        {
            input
                .as_object_mut()
                .expect("xAI video input should be an object")
                .insert("resolution".to_string(), json!(resolution));
        }
    }

    let provider_input_urls = asset
        .references
        .provider_input_urls
        .iter()
        .map(|url| url.trim())
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();
    if let Some(provider_input_url) = provider_input_urls.first().copied() {
        let input_object = input
            .as_object_mut()
            .expect("xAI video input should be an object");
        if is_source_video_edit {
            input_object.insert(
                "video".to_string(),
                json!({
                    "url": provider_input_url
                }),
            );
            url = format!("{XAI_API_BASE_URL}/videos/edits");
        } else if !asset.references.reference_image_media_refs.is_empty() {
            input_object.insert(
                "reference_images".to_string(),
                json!(provider_input_urls
                    .iter()
                    .map(|url| json!({ "url": url }))
                    .collect::<Vec<_>>()),
            );
        } else {
            input_object.insert(
                "image".to_string(),
                json!({
                    "url": provider_input_url,
                    "type": "image_url"
                }),
            );
        }
    }

    Ok(XAiImageGenerationSubmission {
        method: "POST".to_string(),
        url,
        model: XAI_GROK_VIDEO_MODEL_ID.to_string(),
        provider: XAI_PROVIDER.to_string(),
        input,
    })
}

pub fn submit_xai_image_generation_with_client(
    client: &Client,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
) -> Result<XAiImageResponse, XAiGenerationError> {
    ensure_xai_credential(&submission.provider, credential)?;
    let mut body = submission.input.clone();
    let body_object = body
        .as_object_mut()
        .ok_or_else(|| XAiGenerationError::RequestFailed {
            message: "submission input must be a JSON object".to_string(),
        })?;
    body_object.insert("model".to_string(), Value::String(submission.model.clone()));
    let request_body =
        serde_json::to_vec(&body).map_err(|error| XAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    let response = client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| XAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_xai_json(response)
}

pub fn run_xai_image_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
) -> Result<XAiImageGenerationRun, XAiGenerationWorkerError> {
    run_xai_image_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_xai_image_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<XAiImageGenerationRun, XAiGenerationWorkerError> {
    ensure_not_cancelled(cancellation)?;
    let response = submit_xai_image_generation_with_client(client, submission, credential)?;
    ensure_not_cancelled(cancellation)?;
    let first = response
        .data
        .first()
        .ok_or_else(|| XAiGenerationProviderError::MissingResultField("data[0].url".to_string()))?;
    let source_url = first
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| XAiGenerationProviderError::MissingResultField("data[0].url".to_string()))?
        .to_string();
    ensure_not_cancelled(cancellation)?;
    let relative_path = xai_default_output_relative_path(target.asset, first.mime_type.as_deref())?;
    let output_path = safe_xai_generated_output_path(target.project_dir, &relative_path)?;
    ensure_not_cancelled(cancellation)?;
    download_xai_output_with_client(
        client,
        &source_url,
        &output_path,
        IMAGE_OUTPUT_LIMIT,
        cancellation,
    )?;
    let actions = build_xai_generation_completion_actions(
        target.asset,
        &relative_path,
        &source_url,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(XAiImageGenerationRun {
        request_id: format!("xai-{}", target.asset.id),
        status: XAiImageStatusKind::Completed,
        completion: XAiImageGenerationCompletion {
            output_path,
            actions,
        },
    })
}

pub fn run_xai_video_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
    max_status_polls: usize,
    poll_interval: Duration,
) -> Result<XAiImageGenerationRun, XAiGenerationWorkerError> {
    run_xai_video_generation_submission_with_client_cancellable(
        client,
        target,
        submission,
        credential,
        max_status_polls,
        poll_interval,
        None,
    )
}

pub fn run_xai_video_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
    max_status_polls: usize,
    poll_interval: Duration,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<XAiImageGenerationRun, XAiGenerationWorkerError> {
    run_xai_video_generation_submission_with_client_reporting(
        client,
        target,
        submission,
        credential,
        max_status_polls,
        poll_interval,
        cancellation,
        None,
    )
}

/// Runs an xAI video generation and reports the provider's pending progress (0–100) as a job
/// progress snapshot (0–1).
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the cancellable run with one optional progress reporter"
)]
pub fn run_xai_video_generation_submission_with_client_reporting(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
    max_status_polls: usize,
    poll_interval: Duration,
    cancellation: Option<&GenerationCancellationToken>,
    progress: Option<&JobProgressReporter>,
) -> Result<XAiImageGenerationRun, XAiGenerationWorkerError> {
    ensure_not_cancelled(cancellation)?;
    let start_response = submit_xai_video_generation_with_client(client, submission, credential)?;
    ensure_not_cancelled(cancellation)?;
    let request_id = start_response.request_id.trim();
    if request_id.is_empty() {
        return Err(
            XAiGenerationProviderError::MissingResultField("request_id".to_string()).into(),
        );
    }
    let status_url = xai_video_status_url(&submission.url, request_id)?;
    let mut final_status = None;
    for poll_index in 0..max_status_polls.max(1) {
        ensure_not_cancelled(cancellation)?;
        let response = get_xai_video_status_with_client(client, &status_url, credential)?;
        ensure_not_cancelled(cancellation)?;
        match response.status.trim() {
            "done" => {
                final_status = Some(response);
                break;
            }
            "pending" => {
                if let (Some(reporter), Some(percent)) = (progress, response.progress) {
                    reporter.report(percent / 100.0);
                }
                if poll_index + 1 < max_status_polls.max(1) {
                    wait_for_poll_interval(cancellation, poll_interval)?;
                }
            }
            "failed" | "expired" => {
                return Err(XAiGenerationProviderError::TerminalVideoStatus {
                    status: response.status,
                }
                .into());
            }
            status => {
                return Err(XAiGenerationProviderError::InvalidResultField {
                    field: "status".to_string(),
                    value: status.to_string(),
                }
                .into());
            }
        }
    }
    let response = final_status.ok_or_else(|| XAiGenerationProviderError::TerminalVideoStatus {
        status: "timed_out".to_string(),
    })?;
    let video = response
        .video
        .ok_or_else(|| XAiGenerationProviderError::MissingResultField("video.url".to_string()))?;
    let source_url = video
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| XAiGenerationProviderError::MissingResultField("video.url".to_string()))?
        .to_string();
    let _provider_duration_seconds = video.duration;
    ensure_not_cancelled(cancellation)?;
    let relative_path = xai_video_output_relative_path(target.asset)?;
    let output_path = safe_xai_generated_output_path(target.project_dir, &relative_path)?;
    ensure_not_cancelled(cancellation)?;
    download_xai_output_with_client(
        client,
        &source_url,
        &output_path,
        VIDEO_OUTPUT_LIMIT,
        cancellation,
    )?;
    let actions = build_xai_generation_completion_actions(
        target.asset,
        &relative_path,
        &source_url,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(XAiImageGenerationRun {
        request_id: request_id.to_string(),
        status: XAiImageStatusKind::Done,
        completion: XAiImageGenerationCompletion {
            output_path,
            actions,
        },
    })
}

fn ensure_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), XAiGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        return Err(XAiGenerationWorkerError::Cancelled);
    }
    Ok(())
}

fn wait_for_poll_interval(
    cancellation: Option<&GenerationCancellationToken>,
    poll_interval: Duration,
) -> Result<(), XAiGenerationWorkerError> {
    if let Some(cancellation) = cancellation {
        if cancellation.wait_timeout(poll_interval) {
            return Err(XAiGenerationWorkerError::Cancelled);
        }
    } else if poll_interval > Duration::ZERO {
        sleep(poll_interval);
    }
    Ok(())
}

fn submit_xai_video_generation_with_client(
    client: &Client,
    submission: &XAiImageGenerationSubmission,
    credential: &str,
) -> Result<XAiVideoStartResponse, XAiGenerationError> {
    ensure_xai_credential(&submission.provider, credential)?;
    let mut body = submission.input.clone();
    let body_object = body
        .as_object_mut()
        .ok_or_else(|| XAiGenerationError::RequestFailed {
            message: "submission input must be a JSON object".to_string(),
        })?;
    body_object.insert("model".to_string(), Value::String(submission.model.clone()));
    let request_body =
        serde_json::to_vec(&body).map_err(|error| XAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    let response = client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| XAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_xai_json(response)
}

fn get_xai_video_status_with_client(
    client: &Client,
    status_url: &str,
    credential: &str,
) -> Result<XAiVideoStatusResponse, XAiGenerationError> {
    let response = client
        .get(status_url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|error| XAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_xai_json(response)
}

pub fn build_xai_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    source_url: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, XAiGenerationProviderError> {
    if !matches!(
        asset.status,
        GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
    ) {
        return Err(XAiGenerationProviderError::NotPending(asset.id.clone()));
    }

    let output = ProjectActionGeneratedAssetOutput {
        media_id: xai_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: Some(source_url.to_string()),
        width: xai_output_width(asset),
        height: xai_output_height(asset),
        duration_seconds: xai_output_duration_seconds(asset),
        fps: xai_output_fps(asset),
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

fn download_xai_output_with_client(
    client: &Client,
    url: &str,
    output_path: &Path,
    limit: u64,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), XAiGenerationWorkerError> {
    let response = client
        .get(url)
        .send()
        .map_err(|error| XAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;
    ensure_not_cancelled(cancellation)?;
    let observed = stream_response_to_atomic_file(response, output_path, limit, || {
        cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
    })
    .map_err(|error| match error {
        crate::generation::download::DownloadError::Cancelled => {
            XAiGenerationWorkerError::Cancelled
        }
        crate::generation::download::DownloadError::HttpStatus { status } => {
            XAiGenerationError::HttpStatus { status }.into()
        }
        other => XAiGenerationError::RequestFailed {
            message: other.to_string(),
        }
        .into(),
    })?;
    if observed == 0 {
        let _ = fs::remove_file(output_path);
        return Err(XAiGenerationError::DecodeResponse {
            message: "generated output download was empty".into(),
        }
        .into());
    }
    Ok(())
}

fn decode_xai_json<T>(response: reqwest::blocking::Response) -> Result<T, XAiGenerationError>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    if !status.is_success() {
        return Err(XAiGenerationError::HttpStatus {
            status: status.as_u16(),
        });
    }

    let body = read_response_bounded(response, JSON_RESPONSE_LIMIT, || false).map_err(|error| {
        XAiGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;

    serde_json::from_slice(&body).map_err(|error| XAiGenerationError::DecodeResponse {
        message: error.to_string(),
    })
}

fn ensure_xai_credential(provider: &str, credential: &str) -> Result<(), XAiGenerationError> {
    if provider.trim() != XAI_PROVIDER {
        return Err(XAiGenerationError::UnsupportedCredentialProvider {
            provider: provider.to_string(),
        });
    }
    if credential.trim().is_empty() {
        return Err(XAiGenerationError::EmptyCredential {
            provider: XAI_PROVIDER.to_string(),
        });
    }

    Ok(())
}

fn safe_xai_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, XAiGenerationError> {
    let relative = Path::new(relative_path);
    let has_unsafe_component = relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if relative_path.trim().is_empty()
        || has_unsafe_component
        || !relative.starts_with("generated")
        || relative.file_name().is_none()
        || relative
            .file_name()
            .is_some_and(|file_name| file_name == "asset.json")
    {
        return Err(XAiGenerationError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }

    Ok(project_dir.join(relative))
}

fn xai_default_output_relative_path(
    asset: &GeneratedAsset,
    mime_type: Option<&str>,
) -> Result<String, XAiGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != XAI_PROVIDER || model_id != XAI_GROK_IMAGE_QUALITY_MODEL_ID {
        return Err(XAiGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    Ok(format!(
        "generated/{}/xai-output.{}",
        asset.id,
        xai_output_extension(mime_type)
    ))
}

fn xai_video_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, XAiGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != XAI_PROVIDER || model_id != XAI_GROK_VIDEO_MODEL_ID {
        return Err(XAiGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    Ok(format!("generated/{}/xai-output.mp4", asset.id))
}

fn xai_video_status_url(
    submission_url: &str,
    request_id: &str,
) -> Result<String, XAiGenerationError> {
    let Some((base_url, _mode)) = submission_url.rsplit_once("/videos/") else {
        return Err(XAiGenerationError::RequestFailed {
            message: format!("xai video submission URL is not under /videos/: {submission_url}"),
        });
    };
    Ok(format!("{base_url}/videos/{request_id}"))
}

fn xai_video_duration(asset: &GeneratedAsset) -> Option<u32> {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| duration.round().clamp(1.0, 15.0) as u32)
}

fn xai_aspect_ratio(asset: &GeneratedAsset) -> Option<String> {
    asset
        .settings
        .aspect_ratio
        .as_deref()
        .map(str::trim)
        .filter(|aspect_ratio| !aspect_ratio.is_empty())
        .map(str::to_string)
        .or_else(|| match (asset.settings.width, asset.settings.height) {
            (Some(width), Some(height)) if width == height && width > 0 => Some("1:1".to_string()),
            (Some(width), Some(height)) if width > height && height > 0 => Some("16:9".to_string()),
            (Some(width), Some(height)) if height > width && width > 0 => Some("9:16".to_string()),
            _ => None,
        })
}

fn xai_output_extension(mime_type: Option<&str>) -> &'static str {
    match mime_type.map(str::trim) {
        Some("image/png") => "png",
        Some("image/webp") => "webp",
        _ => "jpg",
    }
}

fn xai_output_width(asset: &GeneratedAsset) -> u32 {
    match asset.settings.width {
        Some(width) if width > 0 => width,
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => 720,
            Some("1:1") => 1024,
            _ => 1280,
        },
    }
}

fn xai_output_height(asset: &GeneratedAsset) -> u32 {
    match asset.settings.height {
        Some(height) if height > 0 => height,
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => 1280,
            Some("1:1") => 1024,
            _ => 720,
        },
    }
}

fn xai_output_duration_seconds(asset: &GeneratedAsset) -> f64 {
    if asset.model.provider == XAI_PROVIDER && asset.model.id == XAI_GROK_VIDEO_MODEL_ID {
        asset
            .settings
            .duration_seconds
            .filter(|duration| duration.is_finite() && *duration > 0.0)
            .unwrap_or(5.0)
    } else {
        0.0
    }
}

fn xai_output_fps(asset: &GeneratedAsset) -> f64 {
    if asset.model.provider == XAI_PROVIDER && asset.model.id == XAI_GROK_VIDEO_MODEL_ID {
        asset
            .settings
            .fps
            .filter(|fps| fps.is_finite() && *fps > 0.0)
            .unwrap_or(24.0)
    } else {
        0.0
    }
}

fn xai_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-xai-output", asset.id)
}
