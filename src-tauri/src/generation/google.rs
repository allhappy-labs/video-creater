use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    decode_base64_bounded, download_url_to_atomic_file, read_response_bounded,
    write_bytes_atomically, DownloadNetworkPolicy, ScopedHeader, AUDIO_ENVELOPE_LIMIT,
    AUDIO_OUTPUT_LIMIT, JSON_RESPONSE_LIMIT, VIDEO_OUTPUT_LIMIT,
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
use std::thread::sleep;
use std::time::Duration;
use thiserror::Error;

pub const GOOGLE_PROVIDER: &str = "google";
pub const GOOGLE_VEO_31_FAST_MODEL_ID: &str = "veo3.1-fast";
pub const GOOGLE_VEO_31_FAST_API_MODEL_ID: &str = "veo-3.1-fast-generate-preview";
pub const GOOGLE_GEMINI_TTS_MODEL_ID: &str = "gemini-3.1-flash-tts-preview";
pub const GOOGLE_LYRIA_3_PRO_MODEL_ID: &str = "lyria3-pro";
pub const GOOGLE_LYRIA_3_PRO_API_MODEL_ID: &str = "lyria-3-pro-preview";
pub const GOOGLE_GEMINI_TTS_DEFAULT_VOICE: &str = "Kore";
pub const GOOGLE_GEMINI_TTS_VOICES: &[&str] = &[
    "Zephyr",
    "Puck",
    "Charon",
    "Kore",
    "Fenrir",
    "Leda",
    "Orus",
    "Aoede",
    "Callirrhoe",
    "Autonoe",
    "Enceladus",
    "Iapetus",
    "Umbriel",
    "Algieba",
    "Despina",
    "Erinome",
    "Algenib",
    "Rasalgethi",
    "Laomedeia",
    "Achernar",
    "Alnilam",
    "Schedar",
    "Gacrux",
    "Pulcherrima",
    "Achird",
    "Zubenelgenubi",
    "Vindemiatrix",
    "Sadachbia",
    "Sadaltager",
    "Sulafat",
];
pub const GOOGLE_GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
pub const VIDEO_CREATER_GOOGLE_API_BASE_URL_ENV_VAR: &str = "VIDEO_CREATER_GOOGLE_API_BASE_URL";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleVeoGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub api_model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleGeminiTtsGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleLyriaGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub api_model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoogleVeoStatusKind {
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoogleVeoGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoogleVeoGenerationRun {
    pub request_id: String,
    pub status: GoogleVeoStatusKind,
    pub completion: GoogleVeoGenerationCompletion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoogleGeminiTtsGenerationRun {
    pub request_id: String,
    pub status: GoogleVeoStatusKind,
    pub completion: GoogleVeoGenerationCompletion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoogleLyriaGenerationRun {
    pub request_id: String,
    pub status: GoogleVeoStatusKind,
    pub completion: GoogleVeoGenerationCompletion,
}

#[derive(Debug, Error, PartialEq)]
pub enum GoogleGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported google generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("google veo result is missing {0}")]
    MissingResultField(String),
    #[error("generated asset cannot be completed by google worker: {0}")]
    NotPending(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum GoogleGenerationError {
    #[error("unsupported google credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("google credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("google generation request failed: {message}")]
    RequestFailed { message: String },
    #[error("google generation returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("google generation response could not be decoded: {message}")]
    DecodeResponse { message: String },
    #[error("google generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("google generated output write failed: {message}")]
    WriteOutputFailed { message: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum GoogleGenerationWorkerError {
    #[error("google generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] GoogleGenerationProviderError),
    #[error(transparent)]
    Request(#[from] GoogleGenerationError),
}

#[derive(Debug, Deserialize)]
struct GoogleOperationStartResponse {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GoogleOperationStatusResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    response: Option<GoogleOperationResponse>,
    #[serde(default)]
    error: Option<GoogleOperationError>,
}

#[derive(Debug, Deserialize)]
struct GoogleOperationError {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleOperationResponse {
    generate_video_response: GoogleGenerateVideoResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleGenerateVideoResponse {
    generated_samples: Vec<GoogleGeneratedSample>,
}

#[derive(Debug, Deserialize)]
struct GoogleGeneratedSample {
    video: GoogleGeneratedVideo,
}

#[derive(Debug, Deserialize)]
struct GoogleGeneratedVideo {
    uri: String,
}

#[derive(Debug, Deserialize)]
struct GoogleGeminiTtsResponse {
    #[serde(default, alias = "outputAudio")]
    output_audio: Option<GoogleGeminiTtsOutputAudio>,
}

#[derive(Debug, Deserialize)]
struct GoogleGeminiTtsOutputAudio {
    data: String,
    #[serde(default, alias = "mimeType")]
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleLyriaResponse {
    #[serde(default, alias = "outputAudio")]
    output_audio: Option<GoogleGeminiTtsOutputAudio>,
    #[serde(default)]
    steps: Vec<GoogleLyriaStep>,
}

#[derive(Debug, Deserialize)]
struct GoogleLyriaStep {
    #[serde(default, rename = "type")]
    step_type: Option<String>,
    #[serde(default)]
    content: Vec<GoogleLyriaContentBlock>,
}

#[derive(Debug, Deserialize)]
struct GoogleLyriaContentBlock {
    #[serde(default, rename = "type")]
    block_type: Option<String>,
    #[serde(default)]
    data: Option<String>,
}

pub fn build_google_veo_generation_submission(
    asset: &GeneratedAsset,
) -> Result<GoogleVeoGenerationSubmission, GoogleGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(GoogleGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != GOOGLE_PROVIDER || model_id != GOOGLE_VEO_31_FAST_MODEL_ID {
        return Err(GoogleGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    Ok(GoogleVeoGenerationSubmission {
        method: "POST".to_string(),
        url: format!(
            "{GOOGLE_GEMINI_API_BASE_URL}/models/{GOOGLE_VEO_31_FAST_API_MODEL_ID}:predictLongRunning"
        ),
        model: GOOGLE_VEO_31_FAST_MODEL_ID.to_string(),
        api_model: GOOGLE_VEO_31_FAST_API_MODEL_ID.to_string(),
        provider: GOOGLE_PROVIDER.to_string(),
        input: google_veo_generation_input(asset, prompt),
    })
}

pub fn build_google_gemini_tts_generation_submission(
    asset: &GeneratedAsset,
) -> Result<GoogleGeminiTtsGenerationSubmission, GoogleGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(GoogleGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != GOOGLE_PROVIDER || model_id != GOOGLE_GEMINI_TTS_MODEL_ID {
        return Err(GoogleGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    Ok(GoogleGeminiTtsGenerationSubmission {
        method: "POST".to_string(),
        url: format!("{GOOGLE_GEMINI_API_BASE_URL}/interactions"),
        model: GOOGLE_GEMINI_TTS_MODEL_ID.to_string(),
        provider: GOOGLE_PROVIDER.to_string(),
        input: json!({
            "model": GOOGLE_GEMINI_TTS_MODEL_ID,
            "input": google_gemini_tts_input(asset, prompt),
            "response_format": {
                "type": "audio"
            },
            "generation_config": {
                "speech_config": [
                    {
                        "voice": google_gemini_tts_voice(asset)
                    }
                ]
            }
        }),
    })
}

pub fn build_google_lyria_generation_submission(
    asset: &GeneratedAsset,
) -> Result<GoogleLyriaGenerationSubmission, GoogleGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(GoogleGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != GOOGLE_PROVIDER || model_id != GOOGLE_LYRIA_3_PRO_MODEL_ID {
        return Err(GoogleGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    Ok(GoogleLyriaGenerationSubmission {
        method: "POST".to_string(),
        url: format!("{GOOGLE_GEMINI_API_BASE_URL}/interactions"),
        model: GOOGLE_LYRIA_3_PRO_MODEL_ID.to_string(),
        api_model: GOOGLE_LYRIA_3_PRO_API_MODEL_ID.to_string(),
        provider: GOOGLE_PROVIDER.to_string(),
        input: json!({
            "model": GOOGLE_LYRIA_3_PRO_API_MODEL_ID,
            "input": google_lyria_input(asset, prompt),
            "response_format": {
                "type": "audio"
            }
        }),
    })
}

fn google_gemini_tts_input(asset: &GeneratedAsset, prompt: &str) -> String {
    let Some(style_instructions) = asset
        .settings
        .style_instructions
        .as_deref()
        .map(str::trim)
        .filter(|instructions| !instructions.is_empty())
    else {
        return prompt.to_string();
    };
    format!("{style_instructions}\n\n{prompt}")
}

fn google_gemini_tts_voice(asset: &GeneratedAsset) -> &str {
    asset
        .settings
        .voice
        .as_deref()
        .map(str::trim)
        .filter(|voice| !voice.is_empty())
        .unwrap_or(GOOGLE_GEMINI_TTS_DEFAULT_VOICE)
}

fn google_lyria_input(asset: &GeneratedAsset, prompt: &str) -> String {
    let mut parts = vec![prompt.trim().to_string()];
    if let Some(lyrics) = asset
        .settings
        .lyrics
        .as_deref()
        .map(str::trim)
        .filter(|lyrics| !lyrics.is_empty())
    {
        parts.push(format!("Lyrics:\n{lyrics}"));
    }
    if let Some(style) = asset
        .settings
        .style_instructions
        .as_deref()
        .map(str::trim)
        .filter(|style| !style.is_empty())
    {
        parts.push(format!("Style: {style}"));
    }
    if asset.settings.instrumental.unwrap_or(false) {
        parts.push("Instrumental only, no vocals.".to_string());
    }
    if let Some(duration) = asset.settings.duration_seconds.and_then(|duration| {
        if duration.is_finite() && duration > 0.0 {
            Some(duration.round() as i64)
        } else {
            None
        }
    }) {
        parts.push(format!("Target duration: {duration} seconds."));
    }
    parts.join("\n\n")
}

fn google_veo_generation_input(asset: &GeneratedAsset, prompt: &str) -> Value {
    let mut parameters = serde_json::Map::new();
    if let Some(duration_seconds) = google_veo_duration_seconds(asset) {
        parameters.insert("durationSeconds".to_string(), json!(duration_seconds));
    }
    if let Some(aspect_ratio) = asset
        .settings
        .aspect_ratio
        .as_deref()
        .map(str::trim)
        .filter(|aspect_ratio| !aspect_ratio.is_empty())
    {
        parameters.insert("aspectRatio".to_string(), json!(aspect_ratio));
    }
    if let Some(resolution) = asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| !resolution.is_empty())
    {
        parameters.insert("resolution".to_string(), json!(resolution));
    }
    parameters.insert(
        "generateAudio".to_string(),
        json!(asset.settings.generate_audio.unwrap_or(true)),
    );

    let mut input_url_index = 0usize;
    let mut instance = serde_json::Map::new();
    instance.insert("prompt".to_string(), json!(prompt));
    if let Some(first_frame) = asset
        .references
        .first_frame_media_id
        .as_ref()
        .and_then(|_| google_veo_next_inline_image(asset, &mut input_url_index))
    {
        instance.insert("image".to_string(), first_frame);
    }
    if let Some(last_frame) = asset
        .references
        .last_frame_media_id
        .as_ref()
        .and_then(|_| google_veo_next_inline_image(asset, &mut input_url_index))
    {
        instance.insert(
            "lastFrame".to_string(),
            json!({
                "image": last_frame
            }),
        );
    }
    let reference_images = asset
        .references
        .reference_image_media_refs
        .iter()
        .filter_map(|_| google_veo_next_inline_image(asset, &mut input_url_index))
        .map(|image| {
            json!({
                "image": image,
                "referenceType": "asset"
            })
        })
        .collect::<Vec<_>>();
    if !reference_images.is_empty() {
        instance.insert("referenceImages".to_string(), json!(reference_images));
    }

    json!({
        "instances": [Value::Object(instance)],
        "parameters": parameters
    })
}

fn google_veo_next_inline_image(asset: &GeneratedAsset, index: &mut usize) -> Option<Value> {
    while let Some(input) = asset.references.provider_input_urls.get(*index) {
        *index += 1;
        if let Some(inline_image) = google_veo_inline_image_from_data_url(input) {
            return Some(inline_image);
        }
    }
    None
}

fn google_veo_inline_image_from_data_url(input: &str) -> Option<Value> {
    let trimmed = input.trim();
    let payload = trimmed.strip_prefix("data:")?;
    let (mime_type, data) = payload.split_once(";base64,")?;
    if !mime_type.starts_with("image/") || data.trim().is_empty() {
        return None;
    }
    Some(json!({
        "inlineData": {
            "mimeType": mime_type,
            "data": data
        }
    }))
}

pub fn run_google_veo_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleVeoGenerationSubmission,
    credential: &str,
) -> Result<GoogleVeoGenerationRun, GoogleGenerationWorkerError> {
    run_google_veo_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_google_veo_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleVeoGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<GoogleVeoGenerationRun, GoogleGenerationWorkerError> {
    run_google_veo_generation_submission_with_client_cancellable_and_download_policy(
        client,
        target,
        submission,
        credential,
        cancellation,
        &DownloadNetworkPolicy::public_only(),
    )
}

pub fn run_google_veo_generation_submission_with_client_cancellable_and_download_policy(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleVeoGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
    download_policy: &DownloadNetworkPolicy,
) -> Result<GoogleVeoGenerationRun, GoogleGenerationWorkerError> {
    ensure_not_cancelled(cancellation)?;
    let start = submit_google_veo_generation_with_client(client, submission, credential)?;
    ensure_not_cancelled(cancellation)?;
    let video_uri = poll_google_veo_operation_with_client(
        client,
        &submission.url,
        &start.name,
        credential,
        60,
        Duration::from_secs(10),
        cancellation,
    )?;
    ensure_not_cancelled(cancellation)?;
    let relative_path = google_veo_default_output_relative_path(target.asset)?;
    let output_path = safe_google_generated_output_path(target.project_dir, &relative_path)?;
    ensure_not_cancelled(cancellation)?;
    download_google_veo_video(
        &video_uri,
        &submission.url,
        credential,
        &output_path,
        cancellation,
        download_policy,
    )?;
    let actions = build_google_veo_generation_completion_actions(
        target.asset,
        &relative_path,
        &video_uri,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(GoogleVeoGenerationRun {
        request_id: start.name,
        status: GoogleVeoStatusKind::Completed,
        completion: GoogleVeoGenerationCompletion {
            output_path,
            actions,
        },
    })
}

pub fn run_google_gemini_tts_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleGeminiTtsGenerationSubmission,
    credential: &str,
) -> Result<GoogleGeminiTtsGenerationRun, GoogleGenerationWorkerError> {
    run_google_gemini_tts_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_google_gemini_tts_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleGeminiTtsGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<GoogleGeminiTtsGenerationRun, GoogleGenerationWorkerError> {
    ensure_not_cancelled(cancellation)?;
    let response = submit_google_gemini_tts_generation_with_client(client, submission, credential)?;
    ensure_not_cancelled(cancellation)?;
    let pcm_bytes = response
        .output_audio
        .ok_or_else(|| GoogleGenerationError::DecodeResponse {
            message: "Gemini TTS response missing output_audio".to_string(),
        })?;
    let audio_bytes = google_gemini_tts_output_bytes(&pcm_bytes)?;
    let relative_path = google_gemini_tts_default_output_relative_path(target.asset)?;
    let output_path = safe_google_generated_output_path(target.project_dir, &relative_path)?;
    ensure_not_cancelled(cancellation)?;
    write_google_output(&output_path, &audio_bytes)?;
    if let Err(error) = ensure_not_cancelled(cancellation) {
        let _ = fs::remove_file(&output_path);
        return Err(error);
    }
    let actions = build_google_gemini_tts_generation_completion_actions(
        target.asset,
        &relative_path,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(GoogleGeminiTtsGenerationRun {
        request_id: format!("google-{}", target.asset.id),
        status: GoogleVeoStatusKind::Completed,
        completion: GoogleVeoGenerationCompletion {
            output_path,
            actions,
        },
    })
}

pub fn run_google_lyria_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleLyriaGenerationSubmission,
    credential: &str,
) -> Result<GoogleLyriaGenerationRun, GoogleGenerationWorkerError> {
    run_google_lyria_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_google_lyria_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &GoogleLyriaGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<GoogleLyriaGenerationRun, GoogleGenerationWorkerError> {
    ensure_not_cancelled(cancellation)?;
    let response = submit_google_lyria_generation_with_client(client, submission, credential)?;
    ensure_not_cancelled(cancellation)?;
    let audio_bytes = google_lyria_output_bytes(&response)?;
    let relative_path = google_lyria_default_output_relative_path(target.asset)?;
    let output_path = safe_google_generated_output_path(target.project_dir, &relative_path)?;
    ensure_not_cancelled(cancellation)?;
    write_google_output(&output_path, &audio_bytes)?;
    if let Err(error) = ensure_not_cancelled(cancellation) {
        let _ = fs::remove_file(&output_path);
        return Err(error);
    }
    let actions = build_google_lyria_generation_completion_actions(
        target.asset,
        &relative_path,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;

    Ok(GoogleLyriaGenerationRun {
        request_id: format!("google-{}", target.asset.id),
        status: GoogleVeoStatusKind::Completed,
        completion: GoogleVeoGenerationCompletion {
            output_path,
            actions,
        },
    })
}

fn submit_google_veo_generation_with_client(
    client: &Client,
    submission: &GoogleVeoGenerationSubmission,
    credential: &str,
) -> Result<GoogleOperationStartResponse, GoogleGenerationError> {
    ensure_google_credential(&submission.provider, credential)?;
    let request_body = serde_json::to_vec(&submission.input).map_err(|error| {
        GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;
    let response = client
        .post(&submission.url)
        .header("x-goog-api-key", credential.trim())
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_google_json_with_limit(response, JSON_RESPONSE_LIMIT)
}

fn submit_google_gemini_tts_generation_with_client(
    client: &Client,
    submission: &GoogleGeminiTtsGenerationSubmission,
    credential: &str,
) -> Result<GoogleGeminiTtsResponse, GoogleGenerationError> {
    ensure_google_credential(&submission.provider, credential)?;
    let request_body = serde_json::to_vec(&submission.input).map_err(|error| {
        GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;
    let response = client
        .post(&submission.url)
        .header("x-goog-api-key", credential.trim())
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_google_json_with_limit(response, AUDIO_ENVELOPE_LIMIT)
}

fn submit_google_lyria_generation_with_client(
    client: &Client,
    submission: &GoogleLyriaGenerationSubmission,
    credential: &str,
) -> Result<GoogleLyriaResponse, GoogleGenerationError> {
    ensure_google_credential(&submission.provider, credential)?;
    let request_body = serde_json::to_vec(&submission.input).map_err(|error| {
        GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;
    let response = client
        .post(&submission.url)
        .header("x-goog-api-key", credential.trim())
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_google_json_with_limit(response, AUDIO_ENVELOPE_LIMIT)
}

fn google_gemini_tts_output_bytes(
    output_audio: &GoogleGeminiTtsOutputAudio,
) -> Result<Vec<u8>, GoogleGenerationError> {
    let decoded = decode_base64_bounded(
        output_audio.data.trim(),
        AUDIO_ENVELOPE_LIMIT,
        AUDIO_OUTPUT_LIMIT.saturating_sub(44),
    )
    .map_err(|error| GoogleGenerationError::DecodeResponse {
        message: format!("Gemini TTS output_audio.data could not be decoded: {error}"),
    })?;
    let mime_type = output_audio
        .mime_type
        .as_deref()
        .unwrap_or("audio/pcm")
        .to_ascii_lowercase();
    if mime_type.contains("wav") || decoded.starts_with(b"RIFF") {
        return Ok(decoded);
    }
    wav_from_pcm_24khz_mono_i16(&decoded)
}

fn google_lyria_output_bytes(
    response: &GoogleLyriaResponse,
) -> Result<Vec<u8>, GoogleGenerationError> {
    if let Some(output_audio) = &response.output_audio {
        return decode_base64_bounded(
            output_audio.data.trim(),
            AUDIO_ENVELOPE_LIMIT,
            AUDIO_OUTPUT_LIMIT,
        )
        .map_err(|error| GoogleGenerationError::DecodeResponse {
            message: format!("Lyria output_audio.data is not valid base64: {error}"),
        });
    }
    for step in &response.steps {
        if step
            .step_type
            .as_deref()
            .is_some_and(|step_type| step_type != "model_output")
        {
            continue;
        }
        for block in &step.content {
            if block
                .block_type
                .as_deref()
                .is_some_and(|block_type| block_type != "audio")
            {
                continue;
            }
            if let Some(data) = block.data.as_deref() {
                return decode_base64_bounded(
                    data.trim(),
                    AUDIO_ENVELOPE_LIMIT,
                    AUDIO_OUTPUT_LIMIT,
                )
                .map_err(|error| GoogleGenerationError::DecodeResponse {
                    message: format!("Lyria audio block data is not valid base64: {error}"),
                });
            }
        }
    }
    Err(GoogleGenerationError::DecodeResponse {
        message: "Lyria response missing audio data".to_string(),
    })
}

fn wav_from_pcm_24khz_mono_i16(pcm: &[u8]) -> Result<Vec<u8>, GoogleGenerationError> {
    const SAMPLE_RATE: u32 = 24_000;
    const CHANNELS: u16 = 1;
    const BITS_PER_SAMPLE: u16 = 16;
    let byte_rate = SAMPLE_RATE * u32::from(CHANNELS) * u32::from(BITS_PER_SAMPLE) / 8;
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    let data_len = u32::try_from(pcm.len()).map_err(|_| GoogleGenerationError::DecodeResponse {
        message: "PCM audio exceeds WAV format limits".into(),
    })?;
    let riff_len =
        36_u32
            .checked_add(data_len)
            .ok_or_else(|| GoogleGenerationError::DecodeResponse {
                message: "PCM audio exceeds WAV format limits".into(),
            })?;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&CHANNELS.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    Ok(wav)
}

fn poll_google_veo_operation_with_client(
    client: &Client,
    submission_url: &str,
    operation_name: &str,
    credential: &str,
    max_status_polls: usize,
    poll_interval: Duration,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<String, GoogleGenerationWorkerError> {
    let status_url = google_operation_status_url(submission_url, operation_name)?;
    let max_status_polls = max_status_polls.max(1);
    for attempt in 0..max_status_polls {
        ensure_not_cancelled(cancellation)?;
        let response = client
            .get(&status_url)
            .header("x-goog-api-key", credential.trim())
            .header(ACCEPT, "application/json")
            .send()
            .map_err(|error| GoogleGenerationError::RequestFailed {
                message: error.to_string(),
            })?;
        ensure_not_cancelled(cancellation)?;
        let status: GoogleOperationStatusResponse =
            decode_google_json_with_limit(response, JSON_RESPONSE_LIMIT)?;
        if let Some(error) = status.error {
            return Err(GoogleGenerationError::RequestFailed {
                message: error
                    .message
                    .unwrap_or_else(|| "operation failed".to_string()),
            }
            .into());
        }
        if !status.done {
            if attempt + 1 < max_status_polls {
                wait_for_poll_interval(cancellation, poll_interval)?;
            }
            continue;
        }
        return Ok(google_video_uri_from_status(status)?);
    }
    Err(GoogleGenerationError::RequestFailed {
        message: format!(
            "google veo operation {operation_name} did not complete after {max_status_polls} polls"
        ),
    }
    .into())
}

fn ensure_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), GoogleGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        return Err(GoogleGenerationWorkerError::Cancelled);
    }
    Ok(())
}

fn wait_for_poll_interval(
    cancellation: Option<&GenerationCancellationToken>,
    poll_interval: Duration,
) -> Result<(), GoogleGenerationWorkerError> {
    if let Some(cancellation) = cancellation {
        if cancellation.wait_timeout(poll_interval) {
            return Err(GoogleGenerationWorkerError::Cancelled);
        }
    } else if poll_interval > Duration::ZERO {
        sleep(poll_interval);
    }
    Ok(())
}

fn google_video_uri_from_status(
    status: GoogleOperationStatusResponse,
) -> Result<String, GoogleGenerationError> {
    let response = status
        .response
        .ok_or_else(|| GoogleGenerationError::DecodeResponse {
            message: format!(
                "operation {} completed without response",
                status.name.unwrap_or_else(|| "<unknown>".to_string())
            ),
        })?;
    response
        .generate_video_response
        .generated_samples
        .into_iter()
        .next()
        .map(|sample| sample.video.uri)
        .filter(|uri| !uri.trim().is_empty())
        .ok_or_else(|| GoogleGenerationError::DecodeResponse {
            message: "operation response missing generated video uri".to_string(),
        })
}

fn download_google_veo_video(
    video_uri: &str,
    submission_url: &str,
    credential: &str,
    output_path: &Path,
    cancellation: Option<&GenerationCancellationToken>,
    policy: &DownloadNetworkPolicy,
) -> Result<(), GoogleGenerationWorkerError> {
    let header = ScopedHeader::new("x-goog-api-key", credential.trim(), submission_url).map_err(
        |error| GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        },
    )?;
    download_url_to_atomic_file(
        video_uri,
        output_path,
        VIDEO_OUTPUT_LIMIT,
        || cancellation.is_some_and(GenerationCancellationToken::is_cancelled),
        policy,
        Some(header),
    )
    .map(|_| ())
    .map_err(|error| match error {
        crate::generation::download::DownloadError::Cancelled => {
            GoogleGenerationWorkerError::Cancelled
        }
        crate::generation::download::DownloadError::HttpStatus { status } => {
            GoogleGenerationError::HttpStatus { status }.into()
        }
        other => GoogleGenerationError::RequestFailed {
            message: other.to_string(),
        }
        .into(),
    })
}

fn decode_google_json_with_limit<T: for<'de> Deserialize<'de>>(
    response: reqwest::blocking::Response,
    limit: u64,
) -> Result<T, GoogleGenerationError> {
    let status = response.status();
    if !status.is_success() {
        return Err(GoogleGenerationError::HttpStatus {
            status: status.as_u16(),
        });
    }
    let body = read_response_bounded(response, limit, || false).map_err(|error| {
        GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })?;
    serde_json::from_slice(&body).map_err(|error| GoogleGenerationError::DecodeResponse {
        message: error.to_string(),
    })
}

pub fn google_generation_client() -> Result<Client, GoogleGenerationError> {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| GoogleGenerationError::RequestFailed {
            message: error.to_string(),
        })
}

fn ensure_google_credential(provider: &str, credential: &str) -> Result<(), GoogleGenerationError> {
    if provider.trim() != GOOGLE_PROVIDER {
        return Err(GoogleGenerationError::UnsupportedCredentialProvider {
            provider: provider.to_string(),
        });
    }
    if credential.trim().is_empty() {
        return Err(GoogleGenerationError::EmptyCredential {
            provider: GOOGLE_PROVIDER.to_string(),
        });
    }
    Ok(())
}

fn google_operation_status_url(
    submission_url: &str,
    operation_name: &str,
) -> Result<String, GoogleGenerationError> {
    let base = submission_url
        .split("/models/")
        .next()
        .filter(|base| !base.trim().is_empty())
        .ok_or_else(|| GoogleGenerationError::RequestFailed {
            message: "google submission URL is missing /models/".to_string(),
        })?;
    Ok(format!(
        "{}/{}",
        base.trim_end_matches('/'),
        operation_name.trim_start_matches('/')
    ))
}

pub fn build_google_veo_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    source_url: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, GoogleGenerationProviderError> {
    if asset.status != GeneratedAssetStatus::Queued && asset.status != GeneratedAssetStatus::Running
    {
        return Err(GoogleGenerationProviderError::NotPending(format!(
            "{:?}",
            asset.status
        )));
    }

    let output = ProjectActionGeneratedAssetOutput {
        media_id: google_veo_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: Some(source_url.to_string()),
        width: asset.settings.width.unwrap_or(0),
        height: asset.settings.height.unwrap_or(0),
        duration_seconds: asset.settings.duration_seconds.unwrap_or(8.0),
        fps: asset.settings.fps.unwrap_or(24.0),
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

pub fn build_google_gemini_tts_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, GoogleGenerationProviderError> {
    if asset.status != GeneratedAssetStatus::Queued && asset.status != GeneratedAssetStatus::Running
    {
        return Err(GoogleGenerationProviderError::NotPending(format!(
            "{:?}",
            asset.status
        )));
    }

    let output = ProjectActionGeneratedAssetOutput {
        media_id: google_gemini_tts_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: None,
        width: 1,
        height: 1,
        duration_seconds: asset.settings.duration_seconds.unwrap_or(12.0).max(0.0),
        fps: 1.0,
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

pub fn build_google_lyria_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, GoogleGenerationProviderError> {
    if asset.status != GeneratedAssetStatus::Queued && asset.status != GeneratedAssetStatus::Running
    {
        return Err(GoogleGenerationProviderError::NotPending(format!(
            "{:?}",
            asset.status
        )));
    }

    let output = ProjectActionGeneratedAssetOutput {
        media_id: google_lyria_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: None,
        width: 1,
        height: 1,
        duration_seconds: asset.settings.duration_seconds.unwrap_or(120.0).max(0.0),
        fps: 1.0,
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

fn google_veo_duration_seconds(asset: &GeneratedAsset) -> Option<i64> {
    asset.settings.duration_seconds.and_then(|duration| {
        if duration.is_finite() && duration > 0.0 {
            Some(duration.round() as i64)
        } else {
            None
        }
    })
}

fn google_veo_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-google-veo-output", asset.id)
}

fn google_gemini_tts_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-google-gemini-tts-output", asset.id)
}

fn google_lyria_output_media_id(asset: &GeneratedAsset) -> String {
    format!("{}-google-lyria-output", asset.id)
}

fn google_veo_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, GoogleGenerationError> {
    Ok(format!(
        "generated/{}/google-veo-output.mp4",
        sanitize_google_path_component(&asset.id)
    ))
}

fn google_gemini_tts_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, GoogleGenerationError> {
    Ok(format!(
        "generated/{}/google-gemini-tts-output.wav",
        sanitize_google_path_component(&asset.id)
    ))
}

fn google_lyria_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, GoogleGenerationError> {
    Ok(format!(
        "generated/{}/google-lyria-output.mp3",
        sanitize_google_path_component(&asset.id)
    ))
}

fn safe_google_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, GoogleGenerationError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        || !relative_path.starts_with("generated/")
    {
        return Err(GoogleGenerationError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }
    Ok(project_dir.join(relative))
}

fn sanitize_google_path_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.trim_matches('-').is_empty() {
        "asset".to_string()
    } else {
        sanitized
    }
}

fn write_google_output(path: &Path, bytes: &[u8]) -> Result<(), GoogleGenerationError> {
    write_bytes_atomically(path, bytes).map_err(|error| GoogleGenerationError::WriteOutputFailed {
        message: error.to_string(),
    })
}

#[cfg(test)]
mod download_policy_tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn same_origin_google_key_scope_does_not_authorize_private_resolution() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let dir = tempfile::tempdir().unwrap();

        let error = download_google_veo_video(
            &format!("{origin}/video.mp4"),
            &format!("{origin}/v1beta/models/veo:predictLongRunning"),
            "secret",
            &dir.path().join("video.mp4"),
            None,
            &DownloadNetworkPolicy::public_only(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("public downloads require HTTPS"));
        assert!(matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }
}
