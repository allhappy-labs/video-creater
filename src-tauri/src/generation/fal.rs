use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    output_limit_for_path, read_response_body_bounded, read_response_bounded,
    stream_response_to_atomic_file, JSON_RESPONSE_LIMIT,
};
use crate::generation::GenerationTarget;
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const FAL_PROVIDER: &str = "fal.ai";
pub const FAL_FLUX_SCHNELL_MODEL_ID: &str = "fal-ai/flux/schnell";
pub const FAL_KREA_2_TURBO_MODEL_ID: &str = "fal-ai/krea-2/turbo";
pub const FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID: &str = "fal-ai/recraft/v3/text-to-image";
pub const FAL_WAN_TEXT_TO_VIDEO_MODEL_ID: &str = "fal-ai/wan-25-preview/text-to-video";
pub const FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID: &str = "fal-ai/wan/v2.7/image-to-video";
pub const FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID: &str = "fal-ai/wan/v2.7/reference-to-video";
pub const FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID: &str = "fal-ai/wan/v2.2-a14b/video-to-video";
pub const FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID: &str =
    "fal-ai/kling-video/v3/standard/text-to-video";
pub const FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID: &str =
    "fal-ai/kling-video/v3/pro/image-to-video";
pub const FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID: &str =
    "fal-ai/kling-video/v3/pro/motion-control";
pub const FAL_AURA_SR_MODEL_ID: &str = "fal-ai/aura-sr";
pub const FAL_VIDEO_UPSCALER_MODEL_ID: &str = "fal-ai/video-upscaler";
pub const FAL_NANO_BANANA_PRO_EDIT_MODEL_ID: &str = "fal-ai/nano-banana-pro/edit";
pub const FAL_SEED_AUDIO_MODEL_ID: &str = "bytedance/seed-audio-1.0";
pub const FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID: &str = "sonilo/v1.1/text-to-music";
pub const FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID: &str = "sonilo/v1.1/video-to-music";
pub const FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID: &str = "mirelo-ai/sfx-v1.5/video-to-audio";
pub const FAL_QUEUE_BASE_URL: &str = "https://queue.fal.run";
pub const FAL_REST_API_BASE_URL: &str = "https://rest.fal.ai";
pub const VIDEO_CREATER_FAL_REST_API_BASE_URL_ENV_VAR: &str = "VIDEO_CREATER_FAL_REST_API_BASE_URL";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FalGenerationRequest {
    pub endpoint: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FalQueueSubmission {
    pub method: String,
    pub url: String,
    pub endpoint: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FalCdnUpload {
    pub file_url: String,
    pub upload_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FalGeneratedOutputImport {
    pub source_url: String,
    pub output: ProjectActionGeneratedAssetOutput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FalGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FalGenerationRun {
    pub request_id: String,
    pub status: FalQueueStatusResponse,
    pub result: Value,
    pub completion: FalGenerationCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FalGenerationRunOptions {
    pub max_status_polls: usize,
    pub poll_interval: Duration,
}

impl Default for FalGenerationRunOptions {
    fn default() -> Self {
        Self {
            max_status_polls: 60,
            poll_interval: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FalQueueSubmitResponse {
    pub request_id: String,
    pub response_url: String,
    pub status_url: String,
    pub cancel_url: String,
    pub queue_position: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FalQueueCancelStatus {
    CancellationRequested,
    AlreadyCompleted,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FalQueueCancelResponse {
    pub status: FalQueueCancelStatus,
}

#[derive(Debug, Error, PartialEq)]
pub enum FalQueueSubmitError {
    #[error("unsupported fal credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("fal credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("fal queue request failed: {message}")]
    RequestFailed { message: String },
    #[error("fal queue returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("fal queue returned HTTP status {status}: {message}")]
    HttpStatusWithMessage { status: u16, message: String },
    #[error("fal queue response could not be decoded: {message}")]
    DecodeResponse { message: String },
    #[error("fal generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("fal generated output download failed: {message}")]
    DownloadFailed { message: String },
}

fn fal_http_status_with_message(response: Response) -> FalQueueSubmitError {
    let status = response.status().as_u16();
    let message = read_response_body_bounded(response, JSON_RESPONSE_LIMIT, || false)
        .map(|body| String::from_utf8_lossy(&body).into_owned())
        .unwrap_or_else(|error| format!("response body unavailable: {error}"));
    let message = message.trim();
    let message = if message.chars().count() > 2_048 {
        format!("{}...", message.chars().take(2_048).collect::<String>())
    } else if message.is_empty() {
        "provider returned an empty response body".to_string()
    } else {
        message.to_string()
    };
    FalQueueSubmitError::HttpStatusWithMessage { status, message }
}

#[derive(Debug, Deserialize)]
struct FalCdnUploadInitiateResponse {
    file_url: String,
    upload_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FalQueueStatusKind {
    InQueue,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FalQueueStatusResponse {
    pub status: FalQueueStatusKind,
    pub request_id: String,
    pub response_url: String,
    pub queue_position: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub logs: Vec<FalQueueLog>,
    pub metrics: Option<FalQueueMetrics>,
    pub error: Option<String>,
    pub error_type: Option<String>,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

impl FalQueueStatusResponse {
    pub fn is_terminal(&self) -> bool {
        self.status == FalQueueStatusKind::Completed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FalQueueLog {
    pub message: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FalQueueMetrics {
    pub inference_time: Option<f64>,
}

#[derive(Debug, Error, PartialEq)]
pub enum FalGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported fal generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("unsupported fal video aspect ratio: {0}")]
    UnsupportedAspectRatio(String),
    #[error("fal generation result is missing {0}")]
    MissingResultField(String),
    #[error("fal generation result has invalid {field}: {value}")]
    InvalidResultField { field: String, value: String },
    #[error("fal generation requires a provider input URL for {model_id}")]
    MissingProviderInputUrl { model_id: String },
    #[error("generated asset cannot be completed by fal worker: {0}")]
    NotPending(String),
}

pub fn build_fal_generation_request(
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let prompt = asset.prompt.trim();
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != FAL_PROVIDER {
        return Err(FalGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }
    if prompt.is_empty() && !fal_model_allows_empty_prompt(model_id) {
        return Err(FalGenerationProviderError::EmptyPrompt);
    }

    match model_id {
        FAL_FLUX_SCHNELL_MODEL_ID | FAL_KREA_2_TURBO_MODEL_ID => {
            Ok(build_fal_text_to_image_request(model_id, prompt, asset))
        }
        FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID => Ok(build_recraft_v3_request(prompt, asset)),
        FAL_WAN_TEXT_TO_VIDEO_MODEL_ID => build_wan_text_to_video_request(prompt, asset),
        FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID => build_wan_image_to_video_request(prompt, asset),
        FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID => build_wan_reference_to_video_request(prompt, asset),
        FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID => build_wan_video_to_video_request(prompt, asset),
        FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID => {
            build_kling_text_to_video_request(prompt, asset)
        }
        FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID => {
            build_kling_image_to_video_request(prompt, asset)
        }
        FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID => {
            build_kling_motion_control_request(prompt, asset)
        }
        FAL_AURA_SR_MODEL_ID => build_aura_sr_request(asset),
        FAL_VIDEO_UPSCALER_MODEL_ID => build_video_upscaler_request(asset),
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID => build_nano_banana_pro_edit_request(prompt, asset),
        FAL_SEED_AUDIO_MODEL_ID => Ok(build_seed_audio_request(prompt)),
        FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID => Ok(build_sonilo_text_to_music_request(prompt, asset)),
        FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID => build_sonilo_video_to_music_request(asset),
        FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID => build_mirelo_video_to_audio_request(prompt, asset),
        _ => Err(FalGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        }),
    }
}

fn fal_model_allows_empty_prompt(model_id: &str) -> bool {
    matches!(
        model_id,
        FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID
    )
}

pub fn build_fal_queue_submission(
    asset: &GeneratedAsset,
) -> Result<FalQueueSubmission, FalGenerationProviderError> {
    let request = build_fal_generation_request(asset)?;

    Ok(FalQueueSubmission {
        method: "POST".to_string(),
        url: format!("{}/{}", FAL_QUEUE_BASE_URL, request.endpoint),
        endpoint: request.endpoint,
        provider: FAL_PROVIDER.to_string(),
        input: request.input,
    })
}

pub fn upload_fal_local_file_to_cdn_with_client(
    client: &Client,
    rest_api_base_url: &str,
    source_path: &Path,
    credential: &str,
) -> Result<String, FalQueueSubmitError> {
    let credential = credential.trim();
    if credential.is_empty() {
        return Err(FalQueueSubmitError::EmptyCredential {
            provider: FAL_PROVIDER.to_string(),
        });
    }
    let filename = source_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .filter(|filename| !filename.trim().is_empty())
        .ok_or_else(|| FalQueueSubmitError::RequestFailed {
            message: "fal CDN upload source path is missing a file name".to_string(),
        })?;
    let content_type = fal_upload_content_type(source_path);
    let source_bytes =
        fs::read(source_path).map_err(|error| FalQueueSubmitError::RequestFailed {
            message: format!("fal CDN upload source could not be read: {error}"),
        })?;
    let base_url = rest_api_base_url.trim_end_matches('/');
    let initiate_url = format!("{base_url}/storage/upload/initiate?storage_type=fal-cdn-v3");
    let initiate_body = serde_json::to_vec(&json!({
        "content_type": content_type,
        "file_name": filename,
    }))
    .map_err(|error| FalQueueSubmitError::RequestFailed {
        message: error.to_string(),
    })?;

    let initiate_response = client
        .post(&initiate_url)
        .header(AUTHORIZATION, format!("Key {credential}"))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(initiate_body)
        .send()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;
    if !initiate_response.status().is_success() {
        return Err(fal_http_status_with_message(initiate_response));
    }
    let initiate_text = read_response_bounded(initiate_response, JSON_RESPONSE_LIMIT, || false)
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;
    let initiate: FalCdnUploadInitiateResponse =
        serde_json::from_slice(&initiate_text).map_err(|error| {
            FalQueueSubmitError::DecodeResponse {
                message: error.to_string(),
            }
        })?;
    if initiate.file_url.trim().is_empty() || Url::parse(&initiate.file_url).is_err() {
        return Err(FalQueueSubmitError::DecodeResponse {
            message: "fal CDN upload initiate response is missing file_url".to_string(),
        });
    }
    if initiate.upload_url.trim().is_empty() || Url::parse(&initiate.upload_url).is_err() {
        return Err(FalQueueSubmitError::DecodeResponse {
            message: "fal CDN upload initiate response is missing upload_url".to_string(),
        });
    }

    let upload_response = client
        .put(&initiate.upload_url)
        .header(CONTENT_TYPE, content_type)
        .body(source_bytes)
        .send()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;
    if !upload_response.status().is_success() {
        return Err(fal_http_status_with_message(upload_response));
    }

    Ok(initiate.file_url)
}

pub fn submit_fal_queue_submission(
    submission: &FalQueueSubmission,
    credential: &str,
) -> Result<FalQueueSubmitResponse, FalQueueSubmitError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    submit_fal_queue_submission_with_client(&client, submission, credential)
}

pub fn submit_fal_queue_submission_with_client(
    client: &Client,
    submission: &FalQueueSubmission,
    credential: &str,
) -> Result<FalQueueSubmitResponse, FalQueueSubmitError> {
    if submission.provider.trim() != FAL_PROVIDER {
        return Err(FalQueueSubmitError::UnsupportedCredentialProvider {
            provider: submission.provider.clone(),
        });
    }

    let credential = credential.trim();
    if credential.is_empty() {
        return Err(FalQueueSubmitError::EmptyCredential {
            provider: FAL_PROVIDER.to_string(),
        });
    }
    let request_body = serde_json::to_vec(&submission.input).map_err(|error| {
        FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        }
    })?;

    let response = client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Key {credential}"))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    if !response.status().is_success() {
        return Err(fal_http_status_with_message(response));
    }
    let body = read_response_bounded(response, JSON_RESPONSE_LIMIT, || false).map_err(|error| {
        FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        }
    })?;
    serde_json::from_slice(&body).map_err(|error| FalQueueSubmitError::DecodeResponse {
        message: error.to_string(),
    })
}

pub fn fetch_fal_queue_status(
    status_url: &str,
    credential: &str,
    logs: bool,
) -> Result<FalQueueStatusResponse, FalQueueSubmitError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    fetch_fal_queue_status_with_client(&client, status_url, credential, logs)
}

pub fn fetch_fal_queue_status_with_client(
    client: &Client,
    status_url: &str,
    credential: &str,
    logs: bool,
) -> Result<FalQueueStatusResponse, FalQueueSubmitError> {
    let url = fal_status_url_with_logs(status_url, logs);
    get_fal_json_with_client(client, &url, credential)
}

pub fn fetch_fal_queue_result(
    response_url: &str,
    credential: &str,
) -> Result<Value, FalQueueSubmitError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    fetch_fal_queue_result_with_client(&client, response_url, credential)
}

pub fn fetch_fal_queue_result_with_client(
    client: &Client,
    response_url: &str,
    credential: &str,
) -> Result<Value, FalQueueSubmitError> {
    get_fal_json_with_client(client, response_url, credential)
}

pub fn cancel_fal_queue_request(
    cancel_url: &str,
    credential: &str,
) -> Result<FalQueueCancelResponse, FalQueueSubmitError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    cancel_fal_queue_request_with_client(&client, cancel_url, credential)
}

pub fn cancel_fal_queue_request_with_client(
    client: &Client,
    cancel_url: &str,
    credential: &str,
) -> Result<FalQueueCancelResponse, FalQueueSubmitError> {
    put_empty_fal_json_with_client(client, cancel_url, credential)
}

pub fn download_fal_generated_output(
    project_dir: &Path,
    output_import: &FalGeneratedOutputImport,
) -> Result<PathBuf, FalQueueSubmitError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|error| FalQueueSubmitError::DownloadFailed {
            message: error.to_string(),
        })?;

    download_fal_generated_output_with_client(&client, project_dir, output_import)
}

pub fn download_fal_generated_output_with_client(
    client: &Client,
    project_dir: &Path,
    output_import: &FalGeneratedOutputImport,
) -> Result<PathBuf, FalQueueSubmitError> {
    match download_fal_generated_output_with_client_and_cancellation(
        client,
        project_dir,
        output_import,
        None,
    ) {
        Ok(path) => Ok(path),
        Err(FalGenerationWorkerError::Queue(error)) => Err(error),
        Err(error) => Err(FalQueueSubmitError::DownloadFailed {
            message: error.to_string(),
        }),
    }
}

pub fn download_fal_generated_output_with_client_cancellable(
    client: &Client,
    project_dir: &Path,
    output_import: &FalGeneratedOutputImport,
    cancellation: &GenerationCancellationToken,
) -> Result<PathBuf, FalGenerationWorkerError> {
    download_fal_generated_output_with_client_and_cancellation(
        client,
        project_dir,
        output_import,
        Some(cancellation),
    )
}

fn download_fal_generated_output_with_client_and_cancellation(
    client: &Client,
    project_dir: &Path,
    output_import: &FalGeneratedOutputImport,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<PathBuf, FalGenerationWorkerError> {
    ensure_fal_generation_not_cancelled(cancellation)?;
    let output_path =
        safe_fal_generated_output_path(project_dir, &output_import.output.relative_path)?;
    let response = client
        .get(&output_import.source_url)
        .send()
        .map_err(|error| FalQueueSubmitError::DownloadFailed {
            message: error.to_string(),
        })?;
    ensure_fal_generation_not_cancelled(cancellation)?;
    let limit = output_limit_for_path(&output_import.output.relative_path);
    stream_response_to_atomic_file(response, &output_path, limit, || {
        cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
    })
    .map_err(|error| match error {
        crate::generation::download::DownloadError::Cancelled => {
            FalGenerationWorkerError::Cancelled
        }
        crate::generation::download::DownloadError::HttpStatus { status } => {
            FalQueueSubmitError::HttpStatus { status }.into()
        }
        other => FalQueueSubmitError::DownloadFailed {
            message: other.to_string(),
        }
        .into(),
    })?;

    Ok(output_path)
}

fn get_fal_json_with_client<T>(
    client: &Client,
    url: &str,
    credential: &str,
) -> Result<T, FalQueueSubmitError>
where
    T: for<'de> Deserialize<'de>,
{
    let credential = credential.trim();
    if credential.is_empty() {
        return Err(FalQueueSubmitError::EmptyCredential {
            provider: FAL_PROVIDER.to_string(),
        });
    }

    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Key {credential}"))
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    if !response.status().is_success() {
        return Err(fal_http_status_with_message(response));
    }
    let body = read_response_bounded(response, JSON_RESPONSE_LIMIT, || false).map_err(|error| {
        FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        }
    })?;

    serde_json::from_slice(&body).map_err(|error| FalQueueSubmitError::DecodeResponse {
        message: error.to_string(),
    })
}

fn put_empty_fal_json_with_client<T>(
    client: &Client,
    url: &str,
    credential: &str,
) -> Result<T, FalQueueSubmitError>
where
    T: for<'de> Deserialize<'de>,
{
    let credential = credential.trim();
    if credential.is_empty() {
        return Err(FalQueueSubmitError::EmptyCredential {
            provider: FAL_PROVIDER.to_string(),
        });
    }

    let response = client
        .put(url)
        .header(AUTHORIZATION, format!("Key {credential}"))
        .header(ACCEPT, "application/json")
        .send()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })?;

    let status = response.status().as_u16();
    let body =
        read_response_body_bounded(response, JSON_RESPONSE_LIMIT, || false).map_err(|error| {
            FalQueueSubmitError::RequestFailed {
                message: error.to_string(),
            }
        })?;

    if let Ok(decoded) = serde_json::from_slice(&body) {
        return Ok(decoded);
    }
    if !(200..300).contains(&status) {
        let body = String::from_utf8_lossy(&body);
        let message = if body.trim().is_empty() {
            "provider returned an empty response body".to_string()
        } else {
            body.chars().take(2_048).collect()
        };
        return Err(FalQueueSubmitError::HttpStatusWithMessage { status, message });
    }
    serde_json::from_slice(&body).map_err(|error| FalQueueSubmitError::DecodeResponse {
        message: error.to_string(),
    })
}

fn safe_fal_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, FalQueueSubmitError> {
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
        return Err(FalQueueSubmitError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }

    Ok(project_dir.join(relative))
}

fn fal_status_url_with_logs(status_url: &str, logs: bool) -> String {
    if !logs {
        return status_url.to_string();
    }

    if let Ok(mut url) = Url::parse(status_url) {
        let query_pairs = url
            .query_pairs()
            .filter(|(key, _)| key != "logs")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.query_pairs_mut().clear().extend_pairs(query_pairs);
        url.query_pairs_mut().append_pair("logs", "1");
        return url.to_string();
    }

    if status_url.contains('?') {
        format!("{status_url}&logs=1")
    } else {
        format!("{status_url}?logs=1")
    }
}

pub fn build_fal_generated_output_import(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<FalGeneratedOutputImport, FalGenerationProviderError> {
    build_fal_generated_output_imports(asset, result, relative_path)?
        .into_iter()
        .next()
        .ok_or_else(|| missing_result_field("output"))
}

fn build_fal_generated_output_imports(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<Vec<FalGeneratedOutputImport>, FalGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != FAL_PROVIDER {
        return Err(FalGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    match model_id {
        FAL_FLUX_SCHNELL_MODEL_ID | FAL_KREA_2_TURBO_MODEL_ID => {
            build_flux_output_imports(asset, result, relative_path)
        }
        FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID => {
            build_fal_image_output_imports(asset, result, relative_path)
        }
        FAL_WAN_TEXT_TO_VIDEO_MODEL_ID
        | FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID
        | FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID
        | FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
        | FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID
        | FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID
        | FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        | FAL_VIDEO_UPSCALER_MODEL_ID => {
            build_fal_video_output_import(asset, result, relative_path).map(|output| vec![output])
        }
        FAL_AURA_SR_MODEL_ID => {
            build_aura_sr_output_import(asset, result, relative_path).map(|output| vec![output])
        }
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID => {
            build_fal_image_output_imports(asset, result, relative_path)
        }
        FAL_SEED_AUDIO_MODEL_ID
        | FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID
        | FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID => {
            build_fal_audio_output_import(asset, result, relative_path).map(|output| vec![output])
        }
        _ => Err(FalGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        }),
    }
}

pub fn build_fal_generation_completion_actions(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, FalGenerationProviderError> {
    if !matches!(
        asset.status,
        GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
    ) {
        return Err(FalGenerationProviderError::NotPending(asset.id.clone()));
    }

    let output_imports = build_fal_generated_output_imports(asset, result, relative_path)?;
    let generated_output_media_id = output_imports
        .first()
        .map(|output_import| output_import.output.media_id.clone())
        .ok_or_else(|| missing_result_field("output"))?;
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
    let run_id = run_id.map(str::to_string);
    let updated_at = updated_at.to_string();

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
            updated_at,
            run_id,
        },
    ])
}

pub fn download_fal_generation_result_and_build_completion_actions(
    project_dir: &Path,
    asset: &GeneratedAsset,
    result: &Value,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<FalGenerationCompletion, FalGenerationWorkerError> {
    download_fal_generation_result_and_build_completion_actions_with_cancellation(
        project_dir,
        asset,
        result,
        updated_at,
        run_id,
        replacement_item_id,
        None,
    )
}

pub fn download_fal_generation_result_and_build_completion_actions_cancellable(
    project_dir: &Path,
    asset: &GeneratedAsset,
    result: &Value,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
    cancellation: &GenerationCancellationToken,
) -> Result<FalGenerationCompletion, FalGenerationWorkerError> {
    download_fal_generation_result_and_build_completion_actions_with_cancellation(
        project_dir,
        asset,
        result,
        updated_at,
        run_id,
        replacement_item_id,
        Some(cancellation),
    )
}

fn download_fal_generation_result_and_build_completion_actions_with_cancellation(
    project_dir: &Path,
    asset: &GeneratedAsset,
    result: &Value,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<FalGenerationCompletion, FalGenerationWorkerError> {
    ensure_fal_generation_not_cancelled(cancellation)?;
    let relative_path = fal_default_output_relative_path(asset)?;
    let output_imports = build_fal_generated_output_imports(asset, result, &relative_path)?;
    let mut output_paths = Vec::new();
    for output_import in &output_imports {
        if let Err(error) = ensure_fal_generation_not_cancelled(cancellation) {
            for path in &output_paths {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        let output_path = match download_fal_generated_output_with_client_and_cancellation(
            &fal_generation_run_client()?,
            project_dir,
            output_import,
            cancellation,
        ) {
            Ok(path) => path,
            Err(error) => {
                if matches!(error, FalGenerationWorkerError::Cancelled) {
                    for path in &output_paths {
                        let _ = fs::remove_file(path);
                    }
                }
                return Err(error);
            }
        };
        output_paths.push(output_path);
    }
    if let Err(error) = ensure_fal_generation_not_cancelled(cancellation) {
        for path in &output_paths {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    let output_path = output_paths
        .first()
        .cloned()
        .ok_or_else(|| FalGenerationWorkerError::Provider(missing_result_field("output")))?;
    let actions = build_fal_generation_completion_actions(
        asset,
        result,
        &relative_path,
        updated_at,
        run_id,
        replacement_item_id,
    )?;
    if let Err(error) = ensure_fal_generation_not_cancelled(cancellation) {
        for path in &output_paths {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }

    Ok(FalGenerationCompletion {
        output_path,
        actions,
    })
}

pub fn run_fal_generation_job(
    project_dir: &Path,
    asset: &GeneratedAsset,
    credential: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
    options: FalGenerationRunOptions,
) -> Result<FalGenerationRun, FalGenerationWorkerError> {
    let client = fal_generation_run_client()?;
    let submission = build_fal_queue_submission(asset)?;
    run_fal_generation_submission_with_client(
        &client,
        GenerationTarget::new(project_dir, asset, updated_at, run_id, replacement_item_id),
        &submission,
        credential,
        options,
    )
}

pub fn run_fal_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &FalQueueSubmission,
    credential: &str,
    options: FalGenerationRunOptions,
) -> Result<FalGenerationRun, FalGenerationWorkerError> {
    run_fal_generation_submission_with_client_cancellable(
        client, target, submission, credential, options, None,
    )
}

pub fn run_fal_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &FalQueueSubmission,
    credential: &str,
    options: FalGenerationRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<FalGenerationRun, FalGenerationWorkerError> {
    run_fal_generation_submission_with_client_after_submit_cancellable(
        client,
        target,
        submission,
        credential,
        options,
        cancellation,
        |_| Ok(()),
    )
}

pub fn run_fal_generation_submission_with_client_after_submit(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &FalQueueSubmission,
    credential: &str,
    options: FalGenerationRunOptions,
    after_submit: impl FnOnce(&FalQueueSubmitResponse) -> Result<(), FalGenerationWorkerError>,
) -> Result<FalGenerationRun, FalGenerationWorkerError> {
    run_fal_generation_submission_with_client_after_submit_cancellable(
        client,
        target,
        submission,
        credential,
        options,
        None,
        after_submit,
    )
}

pub fn run_fal_generation_submission_with_client_after_submit_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &FalQueueSubmission,
    credential: &str,
    options: FalGenerationRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
    after_submit: impl FnOnce(&FalQueueSubmitResponse) -> Result<(), FalGenerationWorkerError>,
) -> Result<FalGenerationRun, FalGenerationWorkerError> {
    ensure_fal_generation_not_cancelled(cancellation)?;
    let submit_response = submit_fal_queue_submission_with_client(client, submission, credential)?;
    ensure_fal_generation_not_cancelled(cancellation)?;
    after_submit(&submit_response)?;
    ensure_fal_generation_not_cancelled(cancellation)?;
    let max_status_polls = options.max_status_polls.max(1);

    for poll_index in 0..max_status_polls {
        ensure_fal_generation_not_cancelled(cancellation)?;
        let status = match fetch_fal_queue_status_with_client(
            client,
            &submit_response.status_url,
            credential,
            true,
        ) {
            Ok(status) => status,
            Err(error)
                if retryable_fal_queue_read_error(&error) && poll_index + 1 < max_status_polls =>
            {
                if wait_for_fal_generation_cancellation(cancellation, options.poll_interval) {
                    return Err(FalGenerationWorkerError::Cancelled);
                }
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        ensure_fal_generation_not_cancelled(cancellation)?;
        if status.is_terminal() {
            if status.error.is_some() || status.error_type.is_some() {
                return Err(FalGenerationWorkerError::CompletedWithProviderError {
                    request_id: status.request_id,
                    error_type: status.error_type,
                    error: status.error,
                });
            }

            let result = match fetch_fal_queue_result_with_client(
                client,
                &status.response_url,
                credential,
            ) {
                Ok(result) => result,
                Err(error)
                    if retryable_fal_queue_read_error(&error)
                        && poll_index + 1 < max_status_polls =>
                {
                    if wait_for_fal_generation_cancellation(cancellation, options.poll_interval) {
                        return Err(FalGenerationWorkerError::Cancelled);
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            ensure_fal_generation_not_cancelled(cancellation)?;
            let completion =
                download_fal_generation_result_and_build_completion_actions_with_cancellation(
                    target.project_dir,
                    target.asset,
                    &result,
                    target.updated_at,
                    target.run_id,
                    target.replacement_item_id,
                    cancellation,
                )?;
            if let Err(error) = ensure_fal_generation_not_cancelled(cancellation) {
                cleanup_fal_generation_completion_outputs(target.project_dir, &completion);
                return Err(error);
            }

            return Ok(FalGenerationRun {
                request_id: submit_response.request_id,
                status,
                result,
                completion,
            });
        }

        if poll_index + 1 < max_status_polls
            && options.poll_interval > Duration::from_millis(0)
            && wait_for_fal_generation_cancellation(cancellation, options.poll_interval)
        {
            return Err(FalGenerationWorkerError::Cancelled);
        }
    }

    Err(FalGenerationWorkerError::StatusPollLimitExceeded {
        request_id: submit_response.request_id,
        max_status_polls,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct FalResumeRequest<'a> {
    pub request_id: &'a str,
    pub status_url: &'a str,
    pub response_url: &'a str,
}

pub fn resume_fal_generation_request_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    request: FalResumeRequest<'_>,
    credential: &str,
    options: FalGenerationRunOptions,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<FalGenerationRun, FalGenerationWorkerError> {
    ensure_fal_generation_not_cancelled(cancellation)?;
    let max_status_polls = options.max_status_polls.max(1);

    for poll_index in 0..max_status_polls {
        ensure_fal_generation_not_cancelled(cancellation)?;
        let status = match fetch_fal_queue_status_with_client(
            client,
            request.status_url,
            credential,
            true,
        ) {
            Ok(status) => status,
            Err(error)
                if retryable_fal_queue_read_error(&error) && poll_index + 1 < max_status_polls =>
            {
                if wait_for_fal_generation_cancellation(cancellation, options.poll_interval) {
                    return Err(FalGenerationWorkerError::Cancelled);
                }
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        ensure_fal_generation_not_cancelled(cancellation)?;
        if status.is_terminal() {
            if status.error.is_some() || status.error_type.is_some() {
                return Err(FalGenerationWorkerError::CompletedWithProviderError {
                    request_id: status.request_id,
                    error_type: status.error_type,
                    error: status.error,
                });
            }

            let result_url = if status.response_url.trim().is_empty() {
                request.response_url
            } else {
                &status.response_url
            };
            let result = match fetch_fal_queue_result_with_client(client, result_url, credential) {
                Ok(result) => result,
                Err(error)
                    if retryable_fal_queue_read_error(&error)
                        && poll_index + 1 < max_status_polls =>
                {
                    if wait_for_fal_generation_cancellation(cancellation, options.poll_interval) {
                        return Err(FalGenerationWorkerError::Cancelled);
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            ensure_fal_generation_not_cancelled(cancellation)?;
            let completion =
                download_fal_generation_result_and_build_completion_actions_with_cancellation(
                    target.project_dir,
                    target.asset,
                    &result,
                    target.updated_at,
                    target.run_id,
                    target.replacement_item_id,
                    cancellation,
                )?;
            if let Err(error) = ensure_fal_generation_not_cancelled(cancellation) {
                cleanup_fal_generation_completion_outputs(target.project_dir, &completion);
                return Err(error);
            }

            return Ok(FalGenerationRun {
                request_id: request.request_id.to_string(),
                status,
                result,
                completion,
            });
        }

        if poll_index + 1 < max_status_polls
            && options.poll_interval > Duration::from_millis(0)
            && wait_for_fal_generation_cancellation(cancellation, options.poll_interval)
        {
            return Err(FalGenerationWorkerError::Cancelled);
        }
    }

    Err(FalGenerationWorkerError::StatusPollLimitExceeded {
        request_id: request.request_id.to_string(),
        max_status_polls,
    })
}

fn fal_generation_run_client() -> Result<Client, FalQueueSubmitError> {
    Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|error| FalQueueSubmitError::RequestFailed {
            message: error.to_string(),
        })
}

fn retryable_fal_queue_read_error(error: &FalQueueSubmitError) -> bool {
    match error {
        FalQueueSubmitError::RequestFailed { .. } => true,
        FalQueueSubmitError::HttpStatus { status }
        | FalQueueSubmitError::HttpStatusWithMessage { status, .. } => {
            matches!(*status, 404 | 409 | 422 | 425 | 429 | 500..=599)
        }
        _ => false,
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum FalGenerationWorkerError {
    #[error("fal generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] FalGenerationProviderError),
    #[error(transparent)]
    Queue(#[from] FalQueueSubmitError),
    #[error("fal generation did not complete after {max_status_polls} status polls: {request_id}")]
    StatusPollLimitExceeded {
        request_id: String,
        max_status_polls: usize,
    },
    #[error("fal generation submit hook failed: {message}")]
    SubmitHookFailed { message: String },
    #[error("fal generation completed with provider error for {request_id}")]
    CompletedWithProviderError {
        request_id: String,
        error_type: Option<String>,
        error: Option<String>,
    },
}

fn ensure_fal_generation_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), FalGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        Err(FalGenerationWorkerError::Cancelled)
    } else {
        Ok(())
    }
}

fn wait_for_fal_generation_cancellation(
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

fn cleanup_fal_generation_completion_outputs(
    project_dir: &Path,
    completion: &FalGenerationCompletion,
) {
    for action in &completion.actions {
        if let ProjectAction::CompleteGeneratedAsset { outputs, .. } = action {
            for output in outputs {
                if let Ok(path) = safe_fal_generated_output_path(project_dir, &output.relative_path)
                {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

fn fal_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, FalGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != FAL_PROVIDER {
        return Err(FalGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let extension = match model_id {
        FAL_FLUX_SCHNELL_MODEL_ID | FAL_KREA_2_TURBO_MODEL_ID => "png",
        FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID => "webp",
        FAL_WAN_TEXT_TO_VIDEO_MODEL_ID
        | FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID
        | FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID
        | FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
        | FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID
        | FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID
        | FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID
        | FAL_VIDEO_UPSCALER_MODEL_ID => "mp4",
        FAL_AURA_SR_MODEL_ID => "png",
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID => "png",
        FAL_SEED_AUDIO_MODEL_ID => "mp3",
        FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID | FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID => "m4a",
        FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID => "wav",
        _ => {
            return Err(FalGenerationProviderError::UnsupportedModel {
                provider: provider.to_string(),
                model_id: model_id.to_string(),
            });
        }
    };

    Ok(format!("generated/{}/fal-output.{extension}", asset.id))
}

fn build_fal_text_to_image_request(
    model_id: &str,
    prompt: &str,
    asset: &GeneratedAsset,
) -> FalGenerationRequest {
    FalGenerationRequest {
        endpoint: model_id.to_string(),
        input: json!({
            "prompt": prompt,
            "image_size": flux_image_size(asset),
            "num_images": requested_image_count(asset, 4),
            "output_format": "png",
            "enable_safety_checker": true
        }),
    }
}

fn build_recraft_v3_request(prompt: &str, asset: &GeneratedAsset) -> FalGenerationRequest {
    FalGenerationRequest {
        endpoint: FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID.to_string(),
        input: json!({
            "prompt": prompt,
            "image_size": flux_image_size(asset),
            "style": generated_asset_quality(asset).unwrap_or("realistic_image"),
            "enable_safety_checker": true
        }),
    }
}

fn generated_asset_quality(asset: &GeneratedAsset) -> Option<&str> {
    asset
        .settings
        .quality
        .as_deref()
        .map(str::trim)
        .filter(|quality| !quality.is_empty())
}

fn build_wan_text_to_video_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let urls = wan_provider_input_urls(asset);
    let mut input = serde_json::Map::from_iter([
        ("prompt".to_string(), json!(prompt)),
        ("aspect_ratio".to_string(), json!(wan_aspect_ratio(asset)?)),
        ("resolution".to_string(), json!(wan_resolution(asset))),
        ("duration".to_string(), json!(wan_duration(asset))),
        (
            "generate_audio".to_string(),
            json!(asset.settings.generate_audio.unwrap_or(true)),
        ),
        ("enable_prompt_expansion".to_string(), json!(true)),
        ("enable_safety_checker".to_string(), json!(true)),
    ]);
    if let Some(url) = urls.audio_url {
        input.insert("audio_url".to_string(), json!(url));
    }

    Ok(FalGenerationRequest {
        endpoint: FAL_WAN_TEXT_TO_VIDEO_MODEL_ID.to_string(),
        input: Value::Object(input),
    })
}

fn build_wan_image_to_video_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let urls = wan_provider_input_urls(asset);
    if urls.source_video_url.is_none() && urls.first_frame_url.is_none() {
        return Err(FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
        });
    }

    let mut input = serde_json::Map::from_iter([
        ("prompt".to_string(), json!(prompt)),
        ("resolution".to_string(), json!(wan_resolution(asset))),
        ("duration".to_string(), json!(wan_image_duration(asset))),
        ("enable_prompt_expansion".to_string(), json!(true)),
        ("enable_safety_checker".to_string(), json!(true)),
    ]);
    if let Some(url) = urls.source_video_url {
        input.insert("video_url".to_string(), json!(url));
    } else if let Some(url) = urls.first_frame_url {
        input.insert("image_url".to_string(), json!(url));
        if let Some(url) = urls.last_frame_url {
            input.insert("end_image_url".to_string(), json!(url));
        }
    }
    if let Some(url) = urls.audio_url {
        input.insert("audio_url".to_string(), json!(url));
    }

    Ok(FalGenerationRequest {
        endpoint: FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
        input: Value::Object(input),
    })
}

fn build_wan_reference_to_video_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let urls = wan_reference_provider_input_urls(asset);
    if urls.image_urls.is_empty() && urls.video_urls.is_empty() {
        return Err(FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID.to_string(),
        });
    }

    let mut input = serde_json::Map::from_iter([
        ("prompt".to_string(), json!(prompt)),
        (
            "aspect_ratio".to_string(),
            json!(wan_reference_aspect_ratio(asset)?),
        ),
        (
            "resolution".to_string(),
            json!(wan_reference_resolution(asset)),
        ),
        ("duration".to_string(), json!(wan_reference_duration(asset))),
        ("enable_safety_checker".to_string(), json!(true)),
    ]);
    if !urls.image_urls.is_empty() {
        input.insert("reference_image_urls".to_string(), json!(urls.image_urls));
    }
    if !urls.video_urls.is_empty() {
        input.insert("reference_video_urls".to_string(), json!(urls.video_urls));
    }

    Ok(FalGenerationRequest {
        endpoint: FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID.to_string(),
        input: Value::Object(input),
    })
}

fn build_wan_video_to_video_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let frames_per_second = wan_video_to_video_fps(asset);
    Ok(FalGenerationRequest {
        endpoint: FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID.to_string(),
        input: json!({
            "video_url": video_to_video_input_url(asset)?,
            "prompt": prompt,
            "strength": 0.9,
            "num_frames": wan_video_to_video_num_frames(asset, frames_per_second),
            "frames_per_second": frames_per_second,
            "resolution": wan_video_to_video_resolution(asset),
            "aspect_ratio": wan_video_to_video_aspect_ratio(asset)?,
            "num_inference_steps": 27,
            "enable_safety_checker": true,
            "enable_output_safety_checker": false,
            "enable_prompt_expansion": false,
            "acceleration": "regular",
            "guidance_scale": 3.5,
            "guidance_scale_2": 4,
            "shift": 5,
            "interpolator_model": "film",
            "num_interpolated_frames": 0,
            "adjust_fps_for_interpolation": false,
            "video_quality": "high",
            "video_write_mode": "balanced",
            "resample_fps": false
        }),
    })
}

fn build_kling_text_to_video_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    Ok(FalGenerationRequest {
        endpoint: FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID.to_string(),
        input: json!({
            "prompt": prompt,
            "aspect_ratio": kling_aspect_ratio(asset)?,
            "duration": kling_duration(asset),
            "generate_audio": asset.settings.generate_audio.unwrap_or(true),
        }),
    })
}

fn build_kling_image_to_video_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let urls = wan_provider_input_urls(asset);
    let Some(start_image_url) = urls.first_frame_url else {
        return Err(FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
        });
    };
    let mut input = serde_json::Map::from_iter([
        ("prompt".to_string(), json!(prompt)),
        ("start_image_url".to_string(), json!(start_image_url)),
        ("duration".to_string(), json!(kling_duration(asset))),
        (
            "generate_audio".to_string(),
            json!(asset.settings.generate_audio.unwrap_or(true)),
        ),
    ]);
    if let Some(url) = urls.last_frame_url {
        input.insert("end_image_url".to_string(), json!(url));
    }

    Ok(FalGenerationRequest {
        endpoint: FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
        input: Value::Object(input),
    })
}

fn build_kling_motion_control_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let urls = kling_motion_control_provider_input_urls(asset);
    let Some(video_url) = urls.source_video_url else {
        return Err(FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID.to_string(),
        });
    };
    let Some(image_url) = urls.image_url else {
        return Err(FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID.to_string(),
        });
    };

    Ok(FalGenerationRequest {
        endpoint: FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID.to_string(),
        input: json!({
            "prompt": prompt,
            "video_url": video_url,
            "image_url": image_url,
            "keep_original_sound": asset.settings.generate_audio.unwrap_or(true),
            "character_orientation": "image"
        }),
    })
}

fn build_aura_sr_request(
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    Ok(FalGenerationRequest {
        endpoint: FAL_AURA_SR_MODEL_ID.to_string(),
        input: json!({
            "image_url": aura_sr_input_url(asset)?,
            "upscale_factor": 4,
            "overlapping_tiles": true,
            "checkpoint": "v2"
        }),
    })
}

fn build_video_upscaler_request(
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    Ok(FalGenerationRequest {
        endpoint: FAL_VIDEO_UPSCALER_MODEL_ID.to_string(),
        input: json!({
            "video_url": video_upscaler_input_url(asset)?,
            "scale": 2
        }),
    })
}

fn build_nano_banana_pro_edit_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let image_urls = fal_provider_input_urls(asset);
    if image_urls.is_empty() {
        return Err(FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_NANO_BANANA_PRO_EDIT_MODEL_ID.to_string(),
        });
    }

    Ok(FalGenerationRequest {
        endpoint: FAL_NANO_BANANA_PRO_EDIT_MODEL_ID.to_string(),
        input: json!({
            "prompt": prompt,
            "image_urls": image_urls,
            "num_images": requested_image_count(asset, 1),
            "aspect_ratio": nano_banana_aspect_ratio(asset),
            "resolution": nano_banana_resolution(asset),
            "output_format": "png",
            "safety_tolerance": "4"
        }),
    })
}

fn requested_image_count(asset: &GeneratedAsset, max_images: u32) -> u32 {
    asset
        .settings
        .num_images
        .unwrap_or(1)
        .clamp(1, max_images.max(1))
}

fn build_seed_audio_request(prompt: &str) -> FalGenerationRequest {
    FalGenerationRequest {
        endpoint: FAL_SEED_AUDIO_MODEL_ID.to_string(),
        input: json!({
            "prompt": prompt,
            "output_format": "mp3",
            "sample_rate": 24000,
            "speed": 1.0,
            "volume": 1.0
        }),
    }
}

fn build_sonilo_text_to_music_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> FalGenerationRequest {
    FalGenerationRequest {
        endpoint: FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID.to_string(),
        input: json!({
            "prompt": prompt,
            "duration": fal_sonilo_text_to_music_duration(asset),
            "num_samples": 1
        }),
    }
}

fn build_sonilo_video_to_music_request(
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    let mut input = Map::new();
    input.insert(
        "video_url".to_string(),
        json!(video_to_audio_input_url(
            asset,
            FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        )?),
    );
    input.insert("num_samples".to_string(), json!(1));
    if let Some(prompt) = fal_sonilo_video_to_music_prompt(asset) {
        input.insert("prompt".to_string(), json!(prompt));
    }
    if let Some(duration) = fal_sonilo_video_to_music_duration(asset) {
        input.insert("duration".to_string(), json!(duration));
    }

    Ok(FalGenerationRequest {
        endpoint: FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID.to_string(),
        input: Value::Object(input),
    })
}

fn fal_sonilo_video_to_music_prompt(asset: &GeneratedAsset) -> Option<String> {
    let prompt = asset.prompt.trim();
    let style = asset
        .settings
        .style_instructions
        .as_deref()
        .map(str::trim)
        .filter(|style| !style.is_empty());

    match (prompt.is_empty(), style) {
        (false, Some(style)) => Some(format!("{prompt}\n\nStyle: {style}")),
        (false, None) => Some(prompt.to_string()),
        (true, Some(style)) => Some(style.to_string()),
        (true, None) => None,
    }
}

fn fal_sonilo_video_to_music_duration(asset: &GeneratedAsset) -> Option<f64> {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| duration.clamp(1.0, 600.0))
}

fn build_mirelo_video_to_audio_request(
    prompt: &str,
    asset: &GeneratedAsset,
) -> Result<FalGenerationRequest, FalGenerationProviderError> {
    Ok(FalGenerationRequest {
        endpoint: FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID.to_string(),
        input: json!({
            "video_url": video_to_audio_input_url(asset, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID)?,
            "text_prompt": prompt,
            "num_samples": 1,
            "duration": fal_audio_output_duration(asset)
        }),
    })
}

fn flux_image_size(asset: &GeneratedAsset) -> Value {
    match (asset.settings.width, asset.settings.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => {
            json!({ "width": width, "height": height })
        }
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("1:1") => json!("square_hd"),
            Some("9:16") => json!("portrait_16_9"),
            Some("16:9") => json!("landscape_16_9"),
            _ => json!("landscape_16_9"),
        },
    }
}

fn wan_aspect_ratio(asset: &GeneratedAsset) -> Result<&'static str, FalGenerationProviderError> {
    match asset.settings.aspect_ratio.as_deref().map(str::trim) {
        Some("16:9") | None => Ok("16:9"),
        Some("9:16") => Ok("9:16"),
        Some("1:1") => Ok("1:1"),
        Some(value) => Err(FalGenerationProviderError::UnsupportedAspectRatio(
            value.to_string(),
        )),
    }
}

fn wan_resolution(asset: &GeneratedAsset) -> &str {
    if let Some(resolution) = asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| !resolution.is_empty())
    {
        return resolution;
    }

    let longest_edge = asset
        .settings
        .width
        .into_iter()
        .chain(asset.settings.height)
        .max()
        .unwrap_or(1920);

    if longest_edge <= 854 {
        "480p"
    } else if longest_edge <= 1280 {
        "720p"
    } else {
        "1080p"
    }
}

fn wan_reference_resolution(asset: &GeneratedAsset) -> &str {
    if let Some(resolution) = asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| matches!(*resolution, "720p" | "1080p"))
    {
        return resolution;
    }

    let longest_edge = asset
        .settings
        .width
        .into_iter()
        .chain(asset.settings.height)
        .max()
        .unwrap_or(1920);

    if longest_edge <= 1280 {
        "720p"
    } else {
        "1080p"
    }
}

fn wan_duration(asset: &GeneratedAsset) -> &'static str {
    if asset
        .settings
        .duration_seconds
        .is_some_and(|duration| duration > 5.0)
    {
        "10"
    } else {
        "5"
    }
}

fn wan_image_duration(asset: &GeneratedAsset) -> u64 {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| duration.round().clamp(2.0, 15.0) as u64)
        .unwrap_or(5)
}

fn wan_reference_duration(asset: &GeneratedAsset) -> u64 {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| duration.round().clamp(2.0, 10.0) as u64)
        .unwrap_or(5)
}

fn wan_reference_aspect_ratio(
    asset: &GeneratedAsset,
) -> Result<&'static str, FalGenerationProviderError> {
    match asset.settings.aspect_ratio.as_deref().map(str::trim) {
        Some("16:9") | None => Ok("16:9"),
        Some("9:16") => Ok("9:16"),
        Some("1:1") => Ok("1:1"),
        Some("4:3") => Ok("4:3"),
        Some("3:4") => Ok("3:4"),
        Some(value) => Err(FalGenerationProviderError::UnsupportedAspectRatio(
            value.to_string(),
        )),
    }
}

fn wan_video_to_video_aspect_ratio(
    asset: &GeneratedAsset,
) -> Result<&'static str, FalGenerationProviderError> {
    match asset.settings.aspect_ratio.as_deref().map(str::trim) {
        Some("16:9") => Ok("16:9"),
        Some("9:16") => Ok("9:16"),
        Some("1:1") => Ok("1:1"),
        Some("auto") | None => Ok("auto"),
        Some(value) => Err(FalGenerationProviderError::UnsupportedAspectRatio(
            value.to_string(),
        )),
    }
}

fn wan_video_to_video_resolution(asset: &GeneratedAsset) -> &str {
    if let Some(resolution) = asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| matches!(*resolution, "480p" | "580p" | "720p"))
    {
        return resolution;
    }

    let longest_edge = asset
        .settings
        .width
        .into_iter()
        .chain(asset.settings.height)
        .max()
        .unwrap_or(1280);

    if longest_edge <= 854 {
        "480p"
    } else if longest_edge <= 1024 {
        "580p"
    } else {
        "720p"
    }
}

fn wan_video_to_video_fps(asset: &GeneratedAsset) -> u64 {
    asset
        .settings
        .fps
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .map(|fps| fps.round().clamp(4.0, 60.0) as u64)
        .unwrap_or(16)
}

fn wan_video_to_video_num_frames(asset: &GeneratedAsset, frames_per_second: u64) -> u64 {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| {
            (duration * frames_per_second as f64)
                .round()
                .clamp(17.0, 161.0) as u64
        })
        .unwrap_or(81)
}

fn kling_aspect_ratio(asset: &GeneratedAsset) -> Result<&'static str, FalGenerationProviderError> {
    match asset.settings.aspect_ratio.as_deref().map(str::trim) {
        Some("16:9") | None => Ok("16:9"),
        Some("9:16") => Ok("9:16"),
        Some("1:1") => Ok("1:1"),
        Some(value) => Err(FalGenerationProviderError::UnsupportedAspectRatio(
            value.to_string(),
        )),
    }
}

fn kling_duration(asset: &GeneratedAsset) -> String {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| duration.round().clamp(3.0, 15.0) as u64)
        .unwrap_or(5)
        .to_string()
}

fn nano_banana_aspect_ratio(asset: &GeneratedAsset) -> &str {
    asset
        .settings
        .aspect_ratio
        .as_deref()
        .map(str::trim)
        .filter(|aspect_ratio| {
            matches!(
                *aspect_ratio,
                "auto"
                    | "21:9"
                    | "16:9"
                    | "3:2"
                    | "4:3"
                    | "5:4"
                    | "1:1"
                    | "4:5"
                    | "3:4"
                    | "2:3"
                    | "9:16"
            )
        })
        .unwrap_or("auto")
}

fn nano_banana_resolution(asset: &GeneratedAsset) -> &str {
    if let Some(resolution) = asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| matches!(*resolution, "1K" | "2K" | "4K"))
    {
        return resolution;
    }

    let longest_edge = asset
        .settings
        .width
        .into_iter()
        .chain(asset.settings.height)
        .max()
        .unwrap_or(1024);
    if longest_edge <= 1024 {
        "1K"
    } else if longest_edge <= 2048 {
        "2K"
    } else {
        "4K"
    }
}

struct WanProviderInputUrls<'a> {
    source_video_url: Option<&'a str>,
    first_frame_url: Option<&'a str>,
    last_frame_url: Option<&'a str>,
    audio_url: Option<&'a str>,
}

struct WanReferenceProviderInputUrls<'a> {
    image_urls: Vec<&'a str>,
    video_urls: Vec<&'a str>,
}

struct KlingMotionControlProviderInputUrls<'a> {
    source_video_url: Option<&'a str>,
    image_url: Option<&'a str>,
}

fn wan_provider_input_urls(asset: &GeneratedAsset) -> WanProviderInputUrls<'_> {
    let mut urls = asset
        .references
        .provider_input_urls
        .iter()
        .filter_map(|url| {
            let trimmed = url.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        });
    let source_video_url = asset
        .references
        .source_video_media_ref
        .as_deref()
        .filter(|media_id| !media_id.trim().is_empty())
        .and_then(|_| urls.next());
    let first_frame_url = if source_video_url.is_none() {
        asset
            .references
            .first_frame_media_id
            .as_deref()
            .filter(|media_id| !media_id.trim().is_empty())
            .and_then(|_| urls.next())
    } else {
        None
    };
    let last_frame_url = if source_video_url.is_none() {
        asset
            .references
            .last_frame_media_id
            .as_deref()
            .filter(|media_id| !media_id.trim().is_empty())
            .and_then(|_| urls.next())
    } else {
        None
    };
    let audio_url = asset
        .references
        .reference_audio_media_refs
        .iter()
        .any(|media_id| !media_id.trim().is_empty())
        .then(|| urls.next())
        .flatten();

    WanProviderInputUrls {
        source_video_url,
        first_frame_url,
        last_frame_url,
        audio_url,
    }
}

fn wan_reference_provider_input_urls(asset: &GeneratedAsset) -> WanReferenceProviderInputUrls<'_> {
    let urls = asset
        .references
        .provider_input_urls
        .iter()
        .filter_map(|url| {
            let trimmed = url.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .collect::<Vec<_>>();
    let image_count = asset
        .references
        .reference_image_media_refs
        .iter()
        .filter(|media_id| !media_id.trim().is_empty())
        .count();
    let video_count = asset
        .references
        .reference_video_media_refs
        .iter()
        .filter(|media_id| !media_id.trim().is_empty())
        .count();

    WanReferenceProviderInputUrls {
        image_urls: urls.iter().take(image_count).copied().collect(),
        video_urls: urls
            .iter()
            .skip(image_count)
            .take(video_count)
            .copied()
            .collect(),
    }
}

fn kling_motion_control_provider_input_urls(
    asset: &GeneratedAsset,
) -> KlingMotionControlProviderInputUrls<'_> {
    let mut urls = asset
        .references
        .provider_input_urls
        .iter()
        .filter_map(|url| {
            let trimmed = url.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        });
    let source_video_url = asset
        .references
        .source_video_media_ref
        .as_deref()
        .filter(|media_id| !media_id.trim().is_empty())
        .and_then(|_| urls.next());
    let image_url = asset
        .references
        .reference_image_media_refs
        .iter()
        .any(|media_id| !media_id.trim().is_empty())
        .then(|| urls.next())
        .flatten();

    KlingMotionControlProviderInputUrls {
        source_video_url,
        image_url,
    }
}

fn fal_upload_content_type(path: &Path) -> &'static str {
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
        _ => "application/octet-stream",
    }
}

fn aura_sr_input_url(asset: &GeneratedAsset) -> Result<&str, FalGenerationProviderError> {
    asset
        .references
        .provider_input_urls
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|url| !url.is_empty())
        .ok_or_else(|| FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_AURA_SR_MODEL_ID.to_string(),
        })
}

fn video_upscaler_input_url(asset: &GeneratedAsset) -> Result<&str, FalGenerationProviderError> {
    asset
        .references
        .provider_input_urls
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|url| !url.is_empty())
        .ok_or_else(|| FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_VIDEO_UPSCALER_MODEL_ID.to_string(),
        })
}

fn fal_provider_input_urls(asset: &GeneratedAsset) -> Vec<&str> {
    asset
        .references
        .provider_input_urls
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .collect()
}

fn video_to_audio_input_url<'a>(
    asset: &'a GeneratedAsset,
    model_id: &str,
) -> Result<&'a str, FalGenerationProviderError> {
    asset
        .references
        .provider_input_urls
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|url| !url.is_empty())
        .ok_or_else(|| FalGenerationProviderError::MissingProviderInputUrl {
            model_id: model_id.to_string(),
        })
}

fn video_to_video_input_url(asset: &GeneratedAsset) -> Result<&str, FalGenerationProviderError> {
    asset
        .references
        .provider_input_urls
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .find(|url| !url.is_empty())
        .ok_or_else(|| FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID.to_string(),
        })
}

fn build_flux_output_imports(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<Vec<FalGeneratedOutputImport>, FalGenerationProviderError> {
    let images = result
        .get("images")
        .and_then(Value::as_array)
        .ok_or_else(|| missing_result_field("images[0]"))?;
    if images.is_empty() {
        return Err(missing_result_field("images[0]"));
    }

    images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            let source_url = required_string(image, "url")?.to_string();
            Ok(FalGeneratedOutputImport {
                source_url: source_url.clone(),
                output: ProjectActionGeneratedAssetOutput {
                    media_id: indexed_output_media_id(&fal_output_media_id(asset), index),
                    relative_path: indexed_output_relative_path(relative_path, index),
                    source_url: Some(source_url),
                    width: required_u32(image, "width")?,
                    height: required_u32(image, "height")?,
                    duration_seconds: 1.0,
                    fps: 1.0,
                },
            })
        })
        .collect()
}

fn build_fal_image_output_imports(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<Vec<FalGeneratedOutputImport>, FalGenerationProviderError> {
    let images = result
        .get("images")
        .and_then(Value::as_array)
        .ok_or_else(|| missing_result_field("images[0]"))?;
    if images.is_empty() {
        return Err(missing_result_field("images[0]"));
    }

    images
        .iter()
        .enumerate()
        .map(|(index, image)| {
            let source_url = required_string(image, "url")?.to_string();
            Ok(FalGeneratedOutputImport {
                source_url: source_url.clone(),
                output: ProjectActionGeneratedAssetOutput {
                    media_id: indexed_output_media_id(&fal_output_media_id(asset), index),
                    relative_path: indexed_output_relative_path(relative_path, index),
                    source_url: Some(source_url),
                    width: optional_u32(image, "width")
                        .unwrap_or_else(|| fal_image_output_width(asset)),
                    height: optional_u32(image, "height")
                        .unwrap_or_else(|| fal_image_output_height(asset)),
                    duration_seconds: 1.0,
                    fps: 1.0,
                },
            })
        })
        .collect()
}

fn build_aura_sr_output_import(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<FalGeneratedOutputImport, FalGenerationProviderError> {
    let image = result
        .get("image")
        .ok_or_else(|| missing_result_field("image"))?;

    let source_url = required_string(image, "url")?.to_string();

    Ok(FalGeneratedOutputImport {
        source_url: source_url.clone(),
        output: ProjectActionGeneratedAssetOutput {
            media_id: fal_output_media_id(asset),
            relative_path: relative_path.to_string(),
            source_url: Some(source_url),
            width: required_u32(image, "width")?,
            height: required_u32(image, "height")?,
            duration_seconds: 1.0,
            fps: 1.0,
        },
    })
}

fn build_fal_video_output_import(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<FalGeneratedOutputImport, FalGenerationProviderError> {
    let video = result
        .get("video")
        .ok_or_else(|| missing_result_field("video"))?;

    let source_url = required_string(video, "url")?.to_string();

    Ok(FalGeneratedOutputImport {
        source_url: source_url.clone(),
        output: ProjectActionGeneratedAssetOutput {
            media_id: fal_output_media_id(asset),
            relative_path: relative_path.to_string(),
            source_url: Some(source_url),
            width: optional_u32(video, "width").unwrap_or_else(|| fal_video_output_width(asset)),
            height: optional_u32(video, "height").unwrap_or_else(|| fal_video_output_height(asset)),
            duration_seconds: optional_positive_f64(video, "duration")
                .unwrap_or_else(|| fal_video_output_duration(asset)),
            fps: optional_positive_f64(video, "fps").unwrap_or_else(|| fal_video_output_fps(asset)),
        },
    })
}

fn build_fal_audio_output_import(
    asset: &GeneratedAsset,
    result: &Value,
    relative_path: &str,
) -> Result<FalGeneratedOutputImport, FalGenerationProviderError> {
    let audio = first_fal_audio_result(result)?;

    let source_url = required_string(audio, "url")?.to_string();

    Ok(FalGeneratedOutputImport {
        source_url: source_url.clone(),
        output: ProjectActionGeneratedAssetOutput {
            media_id: fal_output_media_id(asset),
            relative_path: relative_path.to_string(),
            source_url: Some(source_url),
            width: 1,
            height: 1,
            duration_seconds: optional_positive_f64(audio, "duration")
                .unwrap_or_else(|| fal_audio_output_duration(asset)),
            fps: 1.0,
        },
    })
}

fn first_fal_audio_result(result: &Value) -> Result<&Value, FalGenerationProviderError> {
    if let Some(audio) = result.get("audio") {
        if audio.is_object() {
            return Ok(audio);
        }
        if let Some(audio) = audio.as_array().and_then(|audios| audios.first()) {
            return Ok(audio);
        }
    }

    result
        .get("audios")
        .and_then(Value::as_array)
        .and_then(|audios| audios.first())
        .ok_or_else(|| missing_result_field("audio"))
}

fn fal_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-fal-output", asset.id)
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

fn fal_video_output_width(asset: &GeneratedAsset) -> u32 {
    asset
        .settings
        .width
        .filter(|width| *width > 0)
        .unwrap_or(1280)
}

fn fal_video_output_height(asset: &GeneratedAsset) -> u32 {
    asset
        .settings
        .height
        .filter(|height| *height > 0)
        .unwrap_or(720)
}

fn fal_video_output_duration(asset: &GeneratedAsset) -> f64 {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(5.0)
}

fn fal_audio_output_duration(asset: &GeneratedAsset) -> f64 {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(10.0)
}

fn fal_sonilo_text_to_music_duration(asset: &GeneratedAsset) -> u64 {
    asset
        .settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| duration.round().clamp(1.0, 600.0) as u64)
        .unwrap_or(90)
}

fn fal_video_output_fps(asset: &GeneratedAsset) -> f64 {
    asset
        .settings
        .fps
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(24.0)
}

fn fal_image_output_width(asset: &GeneratedAsset) -> u32 {
    asset
        .settings
        .width
        .filter(|width| *width > 0)
        .unwrap_or(1024)
}

fn fal_image_output_height(asset: &GeneratedAsset) -> u32 {
    asset
        .settings
        .height
        .filter(|height| *height > 0)
        .unwrap_or(1024)
}

fn required_string<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a str, FalGenerationProviderError> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| missing_result_field(field))?;
    if text.trim().is_empty() {
        return Err(FalGenerationProviderError::InvalidResultField {
            field: field.to_string(),
            value: text.to_string(),
        });
    }

    Ok(text)
}

fn required_u32(value: &Value, field: &'static str) -> Result<u32, FalGenerationProviderError> {
    let number = value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| missing_result_field(field))?;
    if number == 0 || number > u32::MAX as u64 {
        return Err(FalGenerationProviderError::InvalidResultField {
            field: field.to_string(),
            value: number.to_string(),
        });
    }

    Ok(number as u32)
}

fn optional_u32(value: &Value, field: &'static str) -> Option<u32> {
    let number = value.get(field)?.as_u64()?;
    (number > 0 && number <= u32::MAX as u64).then_some(number as u32)
}

fn optional_positive_f64(value: &Value, field: &'static str) -> Option<f64> {
    let number = value.get(field)?.as_f64()?;
    (number.is_finite() && number > 0.0).then_some(number)
}

fn missing_result_field(field: &str) -> FalGenerationProviderError {
    FalGenerationProviderError::MissingResultField(field.to_string())
}
