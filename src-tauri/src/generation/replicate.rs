use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    output_limit_for_path, read_response_bounded, stream_response_to_atomic_file,
    JSON_RESPONSE_LIMIT,
};
use crate::generation::GenerationTarget;
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use reqwest::blocking::{multipart, Client};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const REPLICATE_PROVIDER: &str = "replicate";
pub const REPLICATE_FLUX_SCHNELL_MODEL_ID: &str = "black-forest-labs/flux-schnell";
pub const REPLICATE_FLUX_DEV_MODEL_ID: &str = "black-forest-labs/flux-dev";
pub const REPLICATE_FLUX_11_PRO_MODEL_ID: &str = "black-forest-labs/flux-1.1-pro";
pub const REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID: &str = "black-forest-labs/flux-1.1-pro-ultra";
pub const REPLICATE_SEEDANCE_20_MODEL_ID: &str = "bytedance/seedance-2.0";
pub const REPLICATE_SEEDANCE_20_FAST_MODEL_ID: &str = "bytedance/seedance-2.0-fast";
pub const REPLICATE_API_BASE_URL: &str = "https://api.replicate.com/v1";
pub const VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR: &str =
    "VIDEO_CREATER_REPLICATE_API_BASE_URL";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplicateGenerationRequest {
    pub version: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplicatePredictionSubmission {
    pub method: String,
    pub url: String,
    pub version: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplicateGeneratedOutputImport {
    pub source_url: String,
    pub output: ProjectActionGeneratedAssetOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplicateFileUpload {
    pub get_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplicateGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplicateGenerationRun {
    pub prediction_id: String,
    pub status: ReplicatePredictionResponse,
    pub completion: ReplicateGenerationCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicateGenerationRunOptions {
    pub max_status_polls: usize,
    pub poll_interval: Duration,
}

impl Default for ReplicateGenerationRunOptions {
    fn default() -> Self {
        Self {
            max_status_polls: 60,
            poll_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicatePredictionUrls {
    pub get: String,
    pub cancel: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicatePredictionStatusKind {
    Starting,
    Processing,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicatePredictionResponse {
    pub id: String,
    pub status: ReplicatePredictionStatusKind,
    pub output: Option<Value>,
    pub error: Option<Value>,
    pub urls: ReplicatePredictionUrls,
    #[serde(default)]
    pub logs: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReplicateFileUploadUrls {
    get: String,
}

#[derive(Debug, Deserialize)]
struct ReplicateFileUploadResponse {
    urls: ReplicateFileUploadUrls,
}

impl ReplicatePredictionResponse {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            ReplicatePredictionStatusKind::Succeeded
                | ReplicatePredictionStatusKind::Failed
                | ReplicatePredictionStatusKind::Canceled
        )
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ReplicateGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported replicate generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("replicate generation result is missing {0}")]
    MissingResultField(String),
    #[error("replicate generation result has invalid {field}: {value}")]
    InvalidResultField { field: String, value: String },
    #[error("generated asset cannot be completed by replicate worker: {0}")]
    NotPending(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum ReplicatePredictionError {
    #[error("unsupported replicate credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("replicate credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("replicate prediction request failed: {message}")]
    RequestFailed { message: String },
    #[error("replicate prediction returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("replicate prediction response could not be decoded: {message}")]
    DecodeResponse { message: String },
    #[error("replicate generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("replicate generated output download failed: {message}")]
    DownloadFailed { message: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum ReplicateGenerationWorkerError {
    #[error("replicate generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] ReplicateGenerationProviderError),
    #[error(transparent)]
    Prediction(#[from] ReplicatePredictionError),
    #[error("replicate prediction did not complete after {max_status_polls} status polls: {prediction_id}")]
    StatusPollLimitExceeded {
        prediction_id: String,
        max_status_polls: usize,
    },
    #[error("replicate generation submit hook failed: {message}")]
    SubmitHookFailed { message: String },
    #[error("replicate prediction completed with provider error for {prediction_id}")]
    CompletedWithProviderError {
        prediction_id: String,
        status: ReplicatePredictionStatusKind,
        error: Option<Value>,
    },
}

pub fn build_replicate_generation_request(
    asset: &GeneratedAsset,
) -> Result<ReplicateGenerationRequest, ReplicateGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(ReplicateGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != REPLICATE_PROVIDER {
        return Err(ReplicateGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    match model_id {
        id if is_supported_replicate_image_model(id) => {
            Ok(build_replicate_image_request(model_id, prompt, asset))
        }
        id if is_supported_replicate_video_model(id) => Ok(build_replicate_seedance_video_request(
            model_id, prompt, asset,
        )),
        _ => Err(ReplicateGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        }),
    }
}

pub fn build_replicate_prediction_submission(
    asset: &GeneratedAsset,
) -> Result<ReplicatePredictionSubmission, ReplicateGenerationProviderError> {
    let request = build_replicate_generation_request(asset)?;

    Ok(ReplicatePredictionSubmission {
        method: "POST".to_string(),
        url: format!("{REPLICATE_API_BASE_URL}/predictions"),
        version: request.version,
        provider: REPLICATE_PROVIDER.to_string(),
        input: request.input,
    })
}

fn build_replicate_image_request(
    model_id: &str,
    prompt: &str,
    asset: &GeneratedAsset,
) -> ReplicateGenerationRequest {
    ReplicateGenerationRequest {
        version: model_id.to_string(),
        input: json!({
            "prompt": prompt,
            "width": replicate_image_width(asset),
            "height": replicate_image_height(asset),
            "num_outputs": replicate_image_output_count(asset),
            "output_format": "png"
        }),
    }
}

fn build_replicate_seedance_video_request(
    model_id: &str,
    prompt: &str,
    asset: &GeneratedAsset,
) -> ReplicateGenerationRequest {
    let provider_urls = seedance_provider_input_urls(asset);
    let mut input = serde_json::Map::from_iter([
        ("prompt".to_string(), json!(prompt)),
        (
            "duration".to_string(),
            json!(replicate_video_duration(asset)),
        ),
        (
            "resolution".to_string(),
            json!(replicate_video_resolution(asset)),
        ),
        (
            "aspect_ratio".to_string(),
            json!(replicate_video_aspect_ratio(asset)),
        ),
        (
            "generate_audio".to_string(),
            json!(asset.settings.generate_audio.unwrap_or(true)),
        ),
    ]);

    if let Some(url) = provider_urls.first_frame_url {
        input.insert("first_frame_url".to_string(), json!(url));
    }
    if let Some(url) = provider_urls.last_frame_url {
        input.insert("last_frame_url".to_string(), json!(url));
    }
    if let Some(url) = provider_urls.source_video_url {
        input.insert("source_video_url".to_string(), json!(url));
    }
    if !provider_urls.reference_image_urls.is_empty() {
        input.insert(
            "reference_image_urls".to_string(),
            json!(provider_urls.reference_image_urls),
        );
    }
    if !provider_urls.reference_video_urls.is_empty() {
        input.insert(
            "reference_video_urls".to_string(),
            json!(provider_urls.reference_video_urls),
        );
    }
    if !provider_urls.reference_audio_urls.is_empty() {
        input.insert(
            "reference_audio_urls".to_string(),
            json!(provider_urls.reference_audio_urls),
        );
    }

    ReplicateGenerationRequest {
        version: model_id.to_string(),
        input: Value::Object(input),
    }
}

pub fn upload_replicate_local_file_with_client(
    client: &Client,
    api_base_url: &str,
    source_path: &Path,
    credential: &str,
) -> Result<String, ReplicatePredictionError> {
    ensure_replicate_credential(REPLICATE_PROVIDER, credential)?;
    let filename = source_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .filter(|filename| !filename.trim().is_empty())
        .ok_or_else(|| ReplicatePredictionError::RequestFailed {
            message: "replicate file upload source path is missing a file name".to_string(),
        })?;
    let content_type = replicate_upload_content_type(source_path);
    let source_bytes =
        fs::read(source_path).map_err(|error| ReplicatePredictionError::RequestFailed {
            message: format!("replicate file upload source could not be read: {error}"),
        })?;
    let content_part = multipart::Part::bytes(source_bytes)
        .file_name(filename.to_string())
        .mime_str(content_type)
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;
    let metadata_part = multipart::Part::text("{}")
        .mime_str("application/json")
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;
    let form = multipart::Form::new()
        .part("content", content_part)
        .part("metadata", metadata_part);
    let upload_url = format!("{}/files", api_base_url.trim_end_matches('/'));

    let response = client
        .post(upload_url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .multipart(form)
        .send()
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;
    let upload: ReplicateFileUploadResponse = decode_replicate_json(response)?;
    if upload.urls.get.trim().is_empty() || Url::parse(&upload.urls.get).is_err() {
        return Err(ReplicatePredictionError::DecodeResponse {
            message: "replicate file upload response is missing urls.get".to_string(),
        });
    }

    Ok(upload.urls.get)
}

pub fn submit_replicate_prediction_submission(
    submission: &ReplicatePredictionSubmission,
    credential: &str,
) -> Result<ReplicatePredictionResponse, ReplicatePredictionError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;

    submit_replicate_prediction_submission_with_client(&client, submission, credential)
}

pub fn submit_replicate_prediction_submission_with_client(
    client: &Client,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
) -> Result<ReplicatePredictionResponse, ReplicatePredictionError> {
    ensure_replicate_credential(&submission.provider, credential)?;
    let body = json!({
        "version": submission.version,
        "input": submission.input
    });
    let request_body =
        serde_json::to_vec(&body).map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;

    let response = client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_replicate_json(response)
}

pub fn fetch_replicate_prediction(
    prediction_url: &str,
    credential: &str,
) -> Result<ReplicatePredictionResponse, ReplicatePredictionError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;

    fetch_replicate_prediction_with_client(&client, prediction_url, credential)
}

pub fn fetch_replicate_prediction_with_client(
    client: &Client,
    prediction_url: &str,
    credential: &str,
) -> Result<ReplicatePredictionResponse, ReplicatePredictionError> {
    ensure_replicate_credential(REPLICATE_PROVIDER, credential)?;
    let response = client
        .get(prediction_url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_replicate_json(response)
}

pub fn cancel_replicate_prediction_with_client(
    client: &Client,
    cancel_url: &str,
    credential: &str,
) -> Result<(), ReplicatePredictionError> {
    ensure_replicate_credential(REPLICATE_PROVIDER, credential)?;
    let response = client
        .post(cancel_url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|error| ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(ReplicatePredictionError::HttpStatus {
            status: status.as_u16(),
        });
    }

    Ok(())
}

pub fn build_replicate_generated_output_import(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<ReplicateGeneratedOutputImport, ReplicateGenerationProviderError> {
    build_replicate_generated_output_imports(asset, result, relative_path)?
        .into_iter()
        .next()
        .ok_or_else(|| missing_result_field("output[0]"))
}

fn build_replicate_generated_output_imports(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<Vec<ReplicateGeneratedOutputImport>, ReplicateGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != REPLICATE_PROVIDER || !is_supported_replicate_model(model_id) {
        return Err(ReplicateGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let output = result.get("output").unwrap_or(result);
    let source_urls = if let Some(items) = output.as_array() {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str()
                    .ok_or_else(|| missing_result_field(&format!("output[{index}]")))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![output
            .as_str()
            .ok_or_else(|| missing_result_field("output[0]"))?]
    };
    if source_urls.is_empty() {
        return Err(missing_result_field("output[0]"));
    }

    source_urls
        .into_iter()
        .enumerate()
        .map(|(index, source_url)| {
            if source_url.trim().is_empty() {
                return Err(ReplicateGenerationProviderError::InvalidResultField {
                    field: format!("output[{index}]"),
                    value: source_url.to_string(),
                });
            }
            Ok(ReplicateGeneratedOutputImport {
                source_url: source_url.to_string(),
                output: ProjectActionGeneratedAssetOutput {
                    media_id: indexed_output_media_id(&replicate_output_media_id(asset), index),
                    relative_path: indexed_output_relative_path(relative_path, index),
                    source_url: Some(source_url.to_string()),
                    width: replicate_output_width(asset),
                    height: replicate_output_height(asset),
                    duration_seconds: replicate_output_duration_seconds(asset),
                    fps: replicate_output_fps(asset),
                },
            })
        })
        .collect()
}

pub fn build_replicate_generation_completion_actions(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, ReplicateGenerationProviderError> {
    if !matches!(
        asset.status,
        GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
    ) {
        return Err(ReplicateGenerationProviderError::NotPending(
            asset.id.clone(),
        ));
    }

    let output_imports = build_replicate_generated_output_imports(asset, result, relative_path)?;
    let generated_output_media_id = output_imports
        .first()
        .map(|output_import| output_import.output.media_id.clone())
        .ok_or_else(|| missing_result_field("output[0]"))?;
    let replacement = (output_imports.len() == 1)
        .then_some(replacement_item_id)
        .flatten()
        .map(|item_id| ProjectActionReplaceGeneratedOutput {
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
            outputs: output_imports
                .into_iter()
                .map(|output_import| output_import.output)
                .collect(),
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

pub fn run_replicate_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
    options: ReplicateGenerationRunOptions,
) -> Result<ReplicateGenerationRun, ReplicateGenerationWorkerError> {
    run_replicate_generation_submission_with_client_cancellable(
        client, target, submission, credential, options, None,
    )
}

pub fn run_replicate_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
    options: ReplicateGenerationRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<ReplicateGenerationRun, ReplicateGenerationWorkerError> {
    run_replicate_generation_submission_with_client_after_submit_cancellable(
        client,
        target,
        submission,
        credential,
        options,
        cancellation,
        |_| Ok(()),
    )
}

pub fn run_replicate_generation_submission_with_client_after_submit(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
    options: ReplicateGenerationRunOptions,
    after_submit: impl FnOnce(
        &ReplicatePredictionResponse,
    ) -> Result<(), ReplicateGenerationWorkerError>,
) -> Result<ReplicateGenerationRun, ReplicateGenerationWorkerError> {
    run_replicate_generation_submission_with_client_after_submit_cancellable(
        client,
        target,
        submission,
        credential,
        options,
        None,
        after_submit,
    )
}

pub fn run_replicate_generation_submission_with_client_after_submit_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &ReplicatePredictionSubmission,
    credential: &str,
    options: ReplicateGenerationRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
    after_submit: impl FnOnce(
        &ReplicatePredictionResponse,
    ) -> Result<(), ReplicateGenerationWorkerError>,
) -> Result<ReplicateGenerationRun, ReplicateGenerationWorkerError> {
    ensure_replicate_generation_not_cancelled(cancellation)?;
    let submitted =
        submit_replicate_prediction_submission_with_client(client, submission, credential)?;
    ensure_replicate_generation_not_cancelled(cancellation)?;
    after_submit(&submitted)?;
    ensure_replicate_generation_not_cancelled(cancellation)?;
    let max_status_polls = options.max_status_polls.max(1);

    for poll_index in 0..max_status_polls {
        ensure_replicate_generation_not_cancelled(cancellation)?;
        let status =
            fetch_replicate_prediction_with_client(client, &submitted.urls.get, credential)?;
        ensure_replicate_generation_not_cancelled(cancellation)?;
        if status.is_terminal() {
            if status.status != ReplicatePredictionStatusKind::Succeeded {
                return Err(ReplicateGenerationWorkerError::CompletedWithProviderError {
                    prediction_id: status.id,
                    status: status.status,
                    error: status.error,
                });
            }

            let result = status
                .output
                .clone()
                .ok_or_else(|| missing_result_field("output"))?;
            let completion =
                download_replicate_generation_result_and_build_completion_actions_with_cancellation(
                    client,
                    target,
                    &result,
                    cancellation,
                )?;
            if let Err(error) = ensure_replicate_generation_not_cancelled(cancellation) {
                cleanup_replicate_generation_completion_outputs(target.project_dir, &completion);
                return Err(error);
            }

            return Ok(ReplicateGenerationRun {
                prediction_id: submitted.id,
                status,
                completion,
            });
        }

        if poll_index + 1 < max_status_polls
            && options.poll_interval > Duration::from_millis(0)
            && wait_for_replicate_generation_cancellation(cancellation, options.poll_interval)
        {
            return Err(ReplicateGenerationWorkerError::Cancelled);
        }
    }

    Err(ReplicateGenerationWorkerError::StatusPollLimitExceeded {
        prediction_id: submitted.id,
        max_status_polls,
    })
}

pub fn resume_replicate_generation_request_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    prediction_id: &str,
    prediction_url: &str,
    credential: &str,
    options: ReplicateGenerationRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<ReplicateGenerationRun, ReplicateGenerationWorkerError> {
    ensure_replicate_generation_not_cancelled(cancellation)?;
    let max_status_polls = options.max_status_polls.max(1);

    for poll_index in 0..max_status_polls {
        ensure_replicate_generation_not_cancelled(cancellation)?;
        let status = fetch_replicate_prediction_with_client(client, prediction_url, credential)?;
        ensure_replicate_generation_not_cancelled(cancellation)?;
        if status.is_terminal() {
            if status.status != ReplicatePredictionStatusKind::Succeeded {
                return Err(ReplicateGenerationWorkerError::CompletedWithProviderError {
                    prediction_id: status.id,
                    status: status.status,
                    error: status.error,
                });
            }

            let result = status
                .output
                .clone()
                .ok_or_else(|| missing_result_field("output"))?;
            let completion =
                download_replicate_generation_result_and_build_completion_actions_with_cancellation(
                    client,
                    target,
                    &result,
                    cancellation,
                )?;
            if let Err(error) = ensure_replicate_generation_not_cancelled(cancellation) {
                cleanup_replicate_generation_completion_outputs(target.project_dir, &completion);
                return Err(error);
            }

            return Ok(ReplicateGenerationRun {
                prediction_id: prediction_id.to_string(),
                status,
                completion,
            });
        }

        if poll_index + 1 < max_status_polls
            && options.poll_interval > Duration::from_millis(0)
            && wait_for_replicate_generation_cancellation(cancellation, options.poll_interval)
        {
            return Err(ReplicateGenerationWorkerError::Cancelled);
        }
    }

    Err(ReplicateGenerationWorkerError::StatusPollLimitExceeded {
        prediction_id: prediction_id.to_string(),
        max_status_polls,
    })
}

fn download_replicate_generation_result_and_build_completion_actions_with_cancellation(
    client: &Client,
    target: GenerationTarget<'_>,
    result: &Value,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<ReplicateGenerationCompletion, ReplicateGenerationWorkerError> {
    ensure_replicate_generation_not_cancelled(cancellation)?;
    let relative_path = replicate_default_output_relative_path(target.asset)?;
    let output_imports =
        build_replicate_generated_output_imports(target.asset, result, &relative_path)?;
    let mut output_paths = Vec::new();
    for output_import in &output_imports {
        if let Err(error) = ensure_replicate_generation_not_cancelled(cancellation) {
            for path in &output_paths {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        let output_path = match download_replicate_generated_output_with_client_and_cancellation(
            client,
            target.project_dir,
            output_import,
            cancellation,
        ) {
            Ok(path) => path,
            Err(error) => {
                if matches!(error, ReplicateGenerationWorkerError::Cancelled) {
                    for path in &output_paths {
                        let _ = fs::remove_file(path);
                    }
                }
                return Err(error);
            }
        };
        output_paths.push(output_path);
    }
    if let Err(error) = ensure_replicate_generation_not_cancelled(cancellation) {
        for path in &output_paths {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    let output_path = output_paths.first().cloned().ok_or_else(|| {
        ReplicateGenerationWorkerError::Provider(missing_result_field("output[0]"))
    })?;
    let actions = build_replicate_generation_completion_actions(
        target.asset,
        result,
        &relative_path,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;
    if let Err(error) = ensure_replicate_generation_not_cancelled(cancellation) {
        for path in &output_paths {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }

    Ok(ReplicateGenerationCompletion {
        output_path,
        actions,
    })
}

pub fn download_replicate_generated_output_with_client_cancellable(
    client: &Client,
    project_dir: &Path,
    output_import: &ReplicateGeneratedOutputImport,
    cancellation: &GenerationCancellationToken,
) -> Result<PathBuf, ReplicateGenerationWorkerError> {
    download_replicate_generated_output_with_client_and_cancellation(
        client,
        project_dir,
        output_import,
        Some(cancellation),
    )
}

fn download_replicate_generated_output_with_client_and_cancellation(
    client: &Client,
    project_dir: &Path,
    output_import: &ReplicateGeneratedOutputImport,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<PathBuf, ReplicateGenerationWorkerError> {
    ensure_replicate_generation_not_cancelled(cancellation)?;
    let output_path =
        safe_replicate_generated_output_path(project_dir, &output_import.output.relative_path)?;
    let response = client
        .get(&output_import.source_url)
        .send()
        .map_err(|error| ReplicatePredictionError::DownloadFailed {
            message: error.to_string(),
        })?;
    ensure_replicate_generation_not_cancelled(cancellation)?;
    let limit = output_limit_for_path(&output_import.output.relative_path);
    stream_response_to_atomic_file(response, &output_path, limit, || {
        cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
    })
    .map_err(|error| match error {
        crate::generation::download::DownloadError::Cancelled => {
            ReplicateGenerationWorkerError::Cancelled
        }
        crate::generation::download::DownloadError::HttpStatus { status } => {
            ReplicatePredictionError::HttpStatus { status }.into()
        }
        other => ReplicatePredictionError::DownloadFailed {
            message: other.to_string(),
        }
        .into(),
    })?;

    Ok(output_path)
}

fn ensure_replicate_generation_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), ReplicateGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        Err(ReplicateGenerationWorkerError::Cancelled)
    } else {
        Ok(())
    }
}

fn wait_for_replicate_generation_cancellation(
    cancellation: Option<&GenerationCancellationToken>,
    duration: Duration,
) -> bool {
    if let Some(cancellation) = cancellation {
        cancellation.wait_timeout(duration)
    } else {
        std::thread::sleep(duration);
        false
    }
}

fn cleanup_replicate_generation_completion_outputs(
    project_dir: &Path,
    completion: &ReplicateGenerationCompletion,
) {
    for action in &completion.actions {
        if let ProjectAction::CompleteGeneratedAsset { outputs, .. } = action {
            for output in outputs {
                if let Ok(path) =
                    safe_replicate_generated_output_path(project_dir, &output.relative_path)
                {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

fn decode_replicate_json<T>(
    response: reqwest::blocking::Response,
) -> Result<T, ReplicatePredictionError>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    if !status.is_success() {
        return Err(ReplicatePredictionError::HttpStatus {
            status: status.as_u16(),
        });
    }

    let body = read_response_bounded(response, JSON_RESPONSE_LIMIT, || false).map_err(|error| {
        ReplicatePredictionError::RequestFailed {
            message: error.to_string(),
        }
    })?;

    serde_json::from_slice(&body).map_err(|error| ReplicatePredictionError::DecodeResponse {
        message: error.to_string(),
    })
}

fn ensure_replicate_credential(
    provider: &str,
    credential: &str,
) -> Result<(), ReplicatePredictionError> {
    if provider.trim() != REPLICATE_PROVIDER {
        return Err(ReplicatePredictionError::UnsupportedCredentialProvider {
            provider: provider.to_string(),
        });
    }
    if credential.trim().is_empty() {
        return Err(ReplicatePredictionError::EmptyCredential {
            provider: REPLICATE_PROVIDER.to_string(),
        });
    }

    Ok(())
}

fn safe_replicate_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, ReplicatePredictionError> {
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
        return Err(ReplicatePredictionError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }

    Ok(project_dir.join(relative))
}

fn replicate_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, ReplicateGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != REPLICATE_PROVIDER || !is_supported_replicate_model(model_id) {
        return Err(ReplicateGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let extension = if is_supported_replicate_video_model(model_id) {
        "mp4"
    } else {
        "png"
    };

    Ok(format!(
        "generated/{}/replicate-output.{extension}",
        asset.id
    ))
}

fn replicate_upload_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("zip") => "application/zip",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn is_supported_replicate_image_model(model_id: &str) -> bool {
    matches!(
        model_id,
        REPLICATE_FLUX_SCHNELL_MODEL_ID
            | REPLICATE_FLUX_DEV_MODEL_ID
            | REPLICATE_FLUX_11_PRO_MODEL_ID
            | REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID
    )
}

fn is_supported_replicate_video_model(model_id: &str) -> bool {
    matches!(
        model_id,
        REPLICATE_SEEDANCE_20_MODEL_ID | REPLICATE_SEEDANCE_20_FAST_MODEL_ID
    )
}

fn is_supported_replicate_model(model_id: &str) -> bool {
    is_supported_replicate_image_model(model_id) || is_supported_replicate_video_model(model_id)
}

#[derive(Debug, Default)]
struct SeedanceProviderInputUrls {
    source_video_url: Option<String>,
    first_frame_url: Option<String>,
    last_frame_url: Option<String>,
    reference_image_urls: Vec<String>,
    reference_video_urls: Vec<String>,
    reference_audio_urls: Vec<String>,
}

fn seedance_provider_input_urls(asset: &GeneratedAsset) -> SeedanceProviderInputUrls {
    let mut urls = asset
        .references
        .provider_input_urls
        .iter()
        .map(|url| url.trim())
        .filter(|url| !url.is_empty());
    let mut inputs = SeedanceProviderInputUrls::default();

    if asset
        .references
        .source_video_media_ref
        .as_deref()
        .map(str::trim)
        .is_some_and(|media_id| !media_id.is_empty())
    {
        inputs.source_video_url = urls.next().map(str::to_string);
    }
    if asset
        .references
        .first_frame_media_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|media_id| !media_id.is_empty())
    {
        inputs.first_frame_url = urls.next().map(str::to_string);
    }
    if asset
        .references
        .last_frame_media_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|media_id| !media_id.is_empty())
    {
        inputs.last_frame_url = urls.next().map(str::to_string);
    }
    inputs.reference_image_urls = urls
        .by_ref()
        .take(non_empty_count(
            &asset.references.reference_image_media_refs,
        ))
        .map(str::to_string)
        .collect();
    inputs.reference_video_urls = urls
        .by_ref()
        .take(non_empty_count(
            &asset.references.reference_video_media_refs,
        ))
        .map(str::to_string)
        .collect();
    inputs.reference_audio_urls = urls
        .by_ref()
        .take(non_empty_count(
            &asset.references.reference_audio_media_refs,
        ))
        .map(str::to_string)
        .collect();

    inputs
}

fn non_empty_count(values: &[String]) -> usize {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .count()
}

fn replicate_image_width(asset: &GeneratedAsset) -> u32 {
    match asset.settings.width {
        Some(width) if width > 0 => width,
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => 768,
            Some("1:1") => 1024,
            _ => 1024,
        },
    }
}

fn replicate_image_height(asset: &GeneratedAsset) -> u32 {
    match asset.settings.height {
        Some(height) if height > 0 => height,
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => 1344,
            Some("1:1") => 1024,
            _ => 768,
        },
    }
}

fn replicate_image_output_count(asset: &GeneratedAsset) -> u32 {
    asset.settings.num_images.unwrap_or(1).clamp(1, 4)
}

fn replicate_video_duration(asset: &GeneratedAsset) -> u64 {
    asset
        .settings
        .duration_seconds
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| seconds.round().clamp(1.0, 10.0) as u64)
        .unwrap_or(4)
}

fn replicate_video_resolution(asset: &GeneratedAsset) -> String {
    asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| !resolution.is_empty())
        .unwrap_or("720p")
        .to_string()
}

fn replicate_video_aspect_ratio(asset: &GeneratedAsset) -> String {
    asset
        .settings
        .aspect_ratio
        .as_deref()
        .map(str::trim)
        .filter(|aspect_ratio| !aspect_ratio.is_empty())
        .unwrap_or("16:9")
        .to_string()
}

fn replicate_output_width(asset: &GeneratedAsset) -> u32 {
    if is_supported_replicate_video_model(asset.model.id.trim()) {
        replicate_video_dimensions(asset).0
    } else {
        replicate_image_width(asset)
    }
}

fn replicate_output_height(asset: &GeneratedAsset) -> u32 {
    if is_supported_replicate_video_model(asset.model.id.trim()) {
        replicate_video_dimensions(asset).1
    } else {
        replicate_image_height(asset)
    }
}

fn replicate_output_duration_seconds(asset: &GeneratedAsset) -> f64 {
    if is_supported_replicate_video_model(asset.model.id.trim()) {
        replicate_video_duration(asset) as f64
    } else {
        1.0
    }
}

fn replicate_output_fps(asset: &GeneratedAsset) -> f64 {
    if is_supported_replicate_video_model(asset.model.id.trim()) {
        asset
            .settings
            .fps
            .filter(|fps| fps.is_finite() && *fps > 0.0)
            .unwrap_or(24.0)
    } else {
        1.0
    }
}

fn replicate_video_dimensions(asset: &GeneratedAsset) -> (u32, u32) {
    match (asset.settings.width, asset.settings.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => match replicate_video_aspect_ratio(asset).as_str() {
            "9:16" => (720, 1280),
            "1:1" => (1024, 1024),
            _ => (1280, 720),
        },
    }
}

fn replicate_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-replicate-output", asset.id)
}

fn indexed_output_media_id(base_media_id: &str, index: usize) -> String {
    if index == 0 {
        base_media_id.to_string()
    } else {
        format!("{base_media_id}-{}", index + 1)
    }
}

fn indexed_output_relative_path(relative_path: &str, index: usize) -> String {
    if index == 0 {
        return relative_path.to_string();
    }

    let path = Path::new(relative_path);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let (stem, extension) = file_name
        .rsplit_once('.')
        .map(|(stem, extension)| (stem, Some(extension)))
        .unwrap_or((file_name, None));
    let indexed_file_name = match extension {
        Some(extension) => format!("{stem}-{}.{extension}", index + 1),
        None => format!("{stem}-{}", index + 1),
    };

    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.join(&indexed_file_name))
        .unwrap_or_else(|| PathBuf::from(indexed_file_name))
        .to_string_lossy()
        .replace('\\', "/")
}

fn missing_result_field(field: &str) -> ReplicateGenerationProviderError {
    ReplicateGenerationProviderError::MissingResultField(field.to_string())
}
