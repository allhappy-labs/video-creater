use crate::generation::cancel::GenerationCancellationToken;
use crate::generation::download::{
    decode_base64_bounded, read_response_bounded, stream_response_to_atomic_file,
    write_bytes_atomically, AUDIO_OUTPUT_LIMIT, IMAGE_ENVELOPE_LIMIT, IMAGE_OUTPUT_LIMIT,
};
use crate::generation::GenerationTarget;
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use base64::prelude::*;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const OPENAI_PROVIDER: &str = "openai";
pub const OPENAI_GPT_IMAGE_2_MODEL_ID: &str = "gpt-image-2";
pub const OPENAI_GPT_IMAGE_EDIT_MODEL_ID: &str = "gpt-image-1.5";
pub const OPENAI_GPT_4O_MINI_TTS_MODEL_ID: &str = "gpt-4o-mini-tts";
pub const OPENAI_API_BASE_URL: &str = "https://api.openai.com/v1";
pub const VIDEO_CREATER_OPENAI_API_BASE_URL_ENV_VAR: &str = "VIDEO_CREATER_OPENAI_API_BASE_URL";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiImageGenerationSubmission {
    pub method: String,
    pub url: String,
    pub model: String,
    pub provider: String,
    pub input: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiImageStatusKind {
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiImageGenerationCompletion {
    pub output_path: PathBuf,
    pub actions: Vec<ProjectAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiImageGenerationRun {
    pub request_id: String,
    pub status: OpenAiImageStatusKind,
    pub completion: OpenAiImageGenerationCompletion,
}

#[derive(Debug, Error, PartialEq)]
pub enum OpenAiGenerationProviderError {
    #[error("generated asset prompt cannot be empty")]
    EmptyPrompt,
    #[error("unsupported openai generation model: {provider}/{model_id}")]
    UnsupportedModel { provider: String, model_id: String },
    #[error("openai generation result is missing {0}")]
    MissingResultField(String),
    #[error("openai generation result has invalid {field}: {value}")]
    InvalidResultField { field: String, value: String },
    #[error("openai image edit requires at least one prepared image input")]
    MissingImageEditInput,
    #[error("openai image edit supports at most 16 image inputs, got {count}")]
    TooManyImageEditInputs { count: usize },
    #[error("generated asset cannot be completed by openai worker: {0}")]
    NotPending(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum OpenAiGenerationError {
    #[error("unsupported openai credential provider: {provider}")]
    UnsupportedCredentialProvider { provider: String },
    #[error("openai credential is empty for provider {provider}")]
    EmptyCredential { provider: String },
    #[error("openai image generation request failed: {message}")]
    RequestFailed { message: String },
    #[error("openai image generation returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("openai image generation response could not be decoded: {message}")]
    DecodeResponse { message: String },
    #[error("openai generated output path must stay under generated/: {relative_path}")]
    UnsafeOutputPath { relative_path: String },
    #[error("openai image input cannot be encoded: {message}")]
    EncodeInput { message: String },
    #[error("openai generated output write failed: {message}")]
    WriteOutputFailed { message: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum OpenAiGenerationWorkerError {
    #[error("openai generation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Provider(#[from] OpenAiGenerationProviderError),
    #[error(transparent)]
    Request(#[from] OpenAiGenerationError),
}

#[derive(Debug, Deserialize)]
pub struct OpenAiImageResponse {
    #[serde(default)]
    id: Option<String>,
    data: Vec<OpenAiImageData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageData {
    b64_json: Option<String>,
}

pub fn build_openai_image_generation_submission(
    asset: &GeneratedAsset,
) -> Result<OpenAiImageGenerationSubmission, OpenAiGenerationProviderError> {
    let prompt = asset.prompt.trim();
    if prompt.is_empty() {
        return Err(OpenAiGenerationProviderError::EmptyPrompt);
    }

    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != OPENAI_PROVIDER
        || !matches!(
            model_id,
            OPENAI_GPT_IMAGE_2_MODEL_ID
                | OPENAI_GPT_IMAGE_EDIT_MODEL_ID
                | OPENAI_GPT_4O_MINI_TTS_MODEL_ID
        )
    {
        return Err(OpenAiGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    if model_id == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        let mut input = json!({
            "input": prompt,
            "voice": openai_tts_voice(asset),
            "response_format": "mp3"
        });
        if let Some(instructions) = asset
            .settings
            .style_instructions
            .as_deref()
            .map(str::trim)
            .filter(|instructions| !instructions.is_empty())
        {
            input
                .as_object_mut()
                .expect("OpenAI TTS input should be an object")
                .insert("instructions".to_string(), json!(instructions));
        }
        return Ok(OpenAiImageGenerationSubmission {
            method: "POST".to_string(),
            url: format!("{OPENAI_API_BASE_URL}/audio/speech"),
            model: OPENAI_GPT_4O_MINI_TTS_MODEL_ID.to_string(),
            provider: OPENAI_PROVIDER.to_string(),
            input,
        });
    }

    if model_id == OPENAI_GPT_IMAGE_EDIT_MODEL_ID {
        return Ok(OpenAiImageGenerationSubmission {
            method: "POST".to_string(),
            url: format!("{OPENAI_API_BASE_URL}/images/edits"),
            model: OPENAI_GPT_IMAGE_EDIT_MODEL_ID.to_string(),
            provider: OPENAI_PROVIDER.to_string(),
            input: json!({
                "prompt": prompt,
                "images": openai_image_edit_inputs(asset)?,
                "size": openai_image_edit_size(asset),
                "quality": openai_image_quality(asset),
                "output_format": "png",
                "n": openai_image_count(asset)
            }),
        });
    }

    Ok(OpenAiImageGenerationSubmission {
        method: "POST".to_string(),
        url: format!("{OPENAI_API_BASE_URL}/images/generations"),
        model: OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
        provider: OPENAI_PROVIDER.to_string(),
        input: json!({
            "prompt": prompt,
            "size": openai_image_size(asset),
            "quality": openai_image_quality(asset),
            "output_format": "png",
            "n": openai_image_count(asset)
        }),
    })
}

fn openai_image_count(asset: &GeneratedAsset) -> u32 {
    asset
        .settings
        .num_images
        .filter(|count| *count > 0)
        .unwrap_or(1)
}

fn openai_image_quality(asset: &GeneratedAsset) -> &str {
    asset
        .settings
        .quality
        .as_deref()
        .map(str::trim)
        .filter(|quality| !quality.is_empty())
        .unwrap_or("auto")
}

pub fn submit_openai_image_generation_with_client(
    client: &Client,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
) -> Result<OpenAiImageResponse, OpenAiGenerationError> {
    ensure_openai_credential(&submission.provider, credential)?;
    let mut body = submission.input.clone();
    let body_object = body
        .as_object_mut()
        .ok_or_else(|| OpenAiGenerationError::RequestFailed {
            message: "submission input must be a JSON object".to_string(),
        })?;
    body_object.insert("model".to_string(), Value::String(submission.model.clone()));
    let request_body =
        serde_json::to_vec(&body).map_err(|error| OpenAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    let response = client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| OpenAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    decode_openai_json(response)
}

pub fn openai_image_data_url_from_path(path: &Path) -> Result<String, OpenAiGenerationError> {
    let mime_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some(extension) => {
            return Err(OpenAiGenerationError::EncodeInput {
                message: format!("unsupported image extension: {extension}"),
            });
        }
        None => {
            return Err(OpenAiGenerationError::EncodeInput {
                message: "image input path has no extension".to_string(),
            });
        }
    };
    let bytes = fs::read(path).map_err(|error| OpenAiGenerationError::EncodeInput {
        message: error.to_string(),
    })?;
    if bytes.is_empty() {
        return Err(OpenAiGenerationError::EncodeInput {
            message: "image input file is empty".to_string(),
        });
    }

    Ok(format!(
        "data:{mime_type};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

pub fn run_openai_image_generation_submission_with_client(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
) -> Result<OpenAiImageGenerationRun, OpenAiGenerationWorkerError> {
    run_openai_image_generation_submission_with_client_cancellable(
        client, target, submission, credential, None,
    )
}

pub fn run_openai_image_generation_submission_with_client_cancellable(
    client: &Client,
    target: GenerationTarget<'_>,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<OpenAiImageGenerationRun, OpenAiGenerationWorkerError> {
    ensure_openai_generation_not_cancelled(cancellation)?;
    if submission.model == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        let relative_path = openai_default_output_relative_path(target.asset)?;
        let output_path = safe_openai_generated_output_path(target.project_dir, &relative_path)?;
        let response = send_openai_speech_generation_with_client(client, submission, credential)?;
        let observed =
            stream_response_to_atomic_file(response, &output_path, AUDIO_OUTPUT_LIMIT, || {
                cancellation.is_some_and(GenerationCancellationToken::is_cancelled)
            })
            .map_err(|error| match error {
                crate::generation::download::DownloadError::Cancelled => {
                    OpenAiGenerationWorkerError::Cancelled
                }
                crate::generation::download::DownloadError::HttpStatus { status } => {
                    OpenAiGenerationError::HttpStatus { status }.into()
                }
                other => OpenAiGenerationError::RequestFailed {
                    message: other.to_string(),
                }
                .into(),
            })?;
        if observed == 0 {
            let _ = fs::remove_file(&output_path);
            return Err(OpenAiGenerationProviderError::InvalidResultField {
                field: "audio".into(),
                value: "empty".into(),
            }
            .into());
        }
        let actions = build_openai_generation_completion_actions(
            target.asset,
            &relative_path,
            target.updated_at,
            target.run_id,
            target.replacement_item_id,
        )?;
        let completion = OpenAiImageGenerationCompletion {
            output_path,
            actions,
        };

        return Ok(OpenAiImageGenerationRun {
            request_id: format!("openai-{}", target.asset.id),
            status: OpenAiImageStatusKind::Completed,
            completion,
        });
    }

    let response = submit_openai_image_generation_with_client(client, submission, credential)?;
    ensure_openai_generation_not_cancelled(cancellation)?;
    let completion = write_openai_generation_result_and_build_completion_actions(
        target.project_dir,
        target.asset,
        &response,
        target.updated_at,
        target.run_id,
        target.replacement_item_id,
    )?;
    if ensure_openai_generation_not_cancelled(cancellation).is_err() {
        cleanup_openai_completion_outputs(target.project_dir, &completion.actions);
        return Err(OpenAiGenerationWorkerError::Cancelled);
    }

    Ok(OpenAiImageGenerationRun {
        request_id: response
            .id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("openai-{}", target.asset.id)),
        status: OpenAiImageStatusKind::Completed,
        completion,
    })
}

fn ensure_openai_generation_not_cancelled(
    cancellation: Option<&GenerationCancellationToken>,
) -> Result<(), OpenAiGenerationWorkerError> {
    if cancellation.is_some_and(GenerationCancellationToken::is_cancelled) {
        Err(OpenAiGenerationWorkerError::Cancelled)
    } else {
        Ok(())
    }
}

fn cleanup_openai_completion_outputs(project_dir: &Path, actions: &[ProjectAction]) {
    for action in actions {
        if let ProjectAction::CompleteGeneratedAsset { outputs, .. } = action {
            for output in outputs {
                if let Ok(path) =
                    safe_openai_generated_output_path(project_dir, &output.relative_path)
                {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

pub fn build_openai_generation_completion_actions(
    asset: &GeneratedAsset,
    relative_path: &str,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, OpenAiGenerationProviderError> {
    let output = ProjectActionGeneratedAssetOutput {
        media_id: openai_output_media_id(asset),
        relative_path: relative_path.to_string(),
        source_url: None,
        width: openai_output_width(asset),
        height: openai_output_height(asset),
        duration_seconds: openai_output_duration_seconds(asset),
        fps: openai_output_fps(asset),
    };
    build_openai_generation_completion_actions_for_outputs(
        asset,
        vec![output],
        updated_at,
        run_id,
        replacement_item_id,
    )
}

fn build_openai_generation_completion_actions_for_outputs(
    asset: &GeneratedAsset,
    outputs: Vec<ProjectActionGeneratedAssetOutput>,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, OpenAiGenerationProviderError> {
    if !matches!(
        asset.status,
        GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
    ) {
        return Err(OpenAiGenerationProviderError::NotPending(asset.id.clone()));
    }

    let generated_output_media_id = outputs
        .first()
        .map(|output| output.media_id.clone())
        .ok_or_else(|| {
            OpenAiGenerationProviderError::MissingResultField("data[0].b64_json".to_string())
        })?;
    let replacement = (outputs.len() == 1)
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
            outputs,
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

fn write_openai_generation_result_and_build_completion_actions(
    project_dir: &Path,
    asset: &GeneratedAsset,
    response: &OpenAiImageResponse,
    updated_at: &str,
    run_id: Option<&str>,
    replacement_item_id: Option<&str>,
) -> Result<OpenAiImageGenerationCompletion, OpenAiGenerationWorkerError> {
    if response.data.is_empty() {
        return Err(OpenAiGenerationProviderError::MissingResultField(
            "data[0].b64_json".to_string(),
        )
        .into());
    }

    let mut first_output_path = None;
    let mut outputs = Vec::new();
    for (index, image) in response.data.iter().enumerate() {
        let field_name = format!("data[{index}].b64_json");
        let b64_json = image
            .b64_json
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| OpenAiGenerationProviderError::MissingResultField(field_name.clone()))?;
        let bytes = decode_base64_bounded(b64_json, IMAGE_ENVELOPE_LIMIT, IMAGE_OUTPUT_LIMIT)
            .map_err(|error| OpenAiGenerationError::DecodeResponse {
                message: format!("{field_name} could not be decoded: {error}"),
            })?;
        if bytes.is_empty() {
            return Err(OpenAiGenerationProviderError::InvalidResultField {
                field: field_name,
                value: "empty".to_string(),
            }
            .into());
        }

        let relative_path = openai_output_relative_path(asset, index)?;
        let output_path = safe_openai_generated_output_path(project_dir, &relative_path)?;
        write_bytes_atomically(&output_path, &bytes).map_err(|error| {
            OpenAiGenerationError::WriteOutputFailed {
                message: error.to_string(),
            }
        })?;
        if first_output_path.is_none() {
            first_output_path = Some(output_path.clone());
        }
        outputs.push(ProjectActionGeneratedAssetOutput {
            media_id: openai_output_media_id_for_index(asset, index),
            relative_path,
            source_url: None,
            width: openai_output_width(asset),
            height: openai_output_height(asset),
            duration_seconds: openai_output_duration_seconds(asset),
            fps: openai_output_fps(asset),
        });
    }

    let actions = build_openai_generation_completion_actions_for_outputs(
        asset,
        outputs,
        updated_at,
        run_id,
        replacement_item_id,
    )?;

    Ok(OpenAiImageGenerationCompletion {
        output_path: first_output_path.ok_or_else(|| {
            OpenAiGenerationProviderError::MissingResultField("data[0].b64_json".to_string())
        })?,
        actions,
    })
}

pub fn submit_openai_speech_generation_with_client(
    client: &Client,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
) -> Result<Vec<u8>, OpenAiGenerationError> {
    let response = send_openai_speech_generation_with_client(client, submission, credential)?;
    decode_openai_binary(response)
}

fn send_openai_speech_generation_with_client(
    client: &Client,
    submission: &OpenAiImageGenerationSubmission,
    credential: &str,
) -> Result<reqwest::blocking::Response, OpenAiGenerationError> {
    ensure_openai_credential(&submission.provider, credential)?;
    let mut body = submission.input.clone();
    let body_object = body
        .as_object_mut()
        .ok_or_else(|| OpenAiGenerationError::RequestFailed {
            message: "submission input must be a JSON object".to_string(),
        })?;
    body_object.insert("model".to_string(), Value::String(submission.model.clone()));
    let request_body =
        serde_json::to_vec(&body).map_err(|error| OpenAiGenerationError::RequestFailed {
            message: error.to_string(),
        })?;

    client
        .post(&submission.url)
        .header(AUTHORIZATION, format!("Bearer {}", credential.trim()))
        .header(ACCEPT, "audio/mpeg")
        .header(CONTENT_TYPE, "application/json")
        .body(request_body)
        .send()
        .map_err(|error| OpenAiGenerationError::RequestFailed {
            message: error.to_string(),
        })
}

fn decode_openai_json<T>(response: reqwest::blocking::Response) -> Result<T, OpenAiGenerationError>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    if !status.is_success() {
        return Err(OpenAiGenerationError::HttpStatus {
            status: status.as_u16(),
        });
    }

    let body =
        read_response_bounded(response, IMAGE_ENVELOPE_LIMIT, || false).map_err(|error| {
            OpenAiGenerationError::RequestFailed {
                message: error.to_string(),
            }
        })?;

    serde_json::from_slice(&body).map_err(|error| OpenAiGenerationError::DecodeResponse {
        message: error.to_string(),
    })
}

fn decode_openai_binary(
    response: reqwest::blocking::Response,
) -> Result<Vec<u8>, OpenAiGenerationError> {
    if !response.status().is_success() {
        return Err(OpenAiGenerationError::HttpStatus {
            status: response.status().as_u16(),
        });
    }
    read_response_bounded(response, AUDIO_OUTPUT_LIMIT, || false).map_err(|error| {
        OpenAiGenerationError::RequestFailed {
            message: error.to_string(),
        }
    })
}

fn ensure_openai_credential(provider: &str, credential: &str) -> Result<(), OpenAiGenerationError> {
    if provider.trim() != OPENAI_PROVIDER {
        return Err(OpenAiGenerationError::UnsupportedCredentialProvider {
            provider: provider.to_string(),
        });
    }
    if credential.trim().is_empty() {
        return Err(OpenAiGenerationError::EmptyCredential {
            provider: OPENAI_PROVIDER.to_string(),
        });
    }

    Ok(())
}

fn safe_openai_generated_output_path(
    project_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, OpenAiGenerationError> {
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
        return Err(OpenAiGenerationError::UnsafeOutputPath {
            relative_path: relative_path.to_string(),
        });
    }

    Ok(project_dir.join(relative))
}

fn openai_default_output_relative_path(
    asset: &GeneratedAsset,
) -> Result<String, OpenAiGenerationProviderError> {
    openai_output_relative_path(asset, 0)
}

fn openai_output_relative_path(
    asset: &GeneratedAsset,
    index: usize,
) -> Result<String, OpenAiGenerationProviderError> {
    let provider = asset.model.provider.trim();
    let model_id = asset.model.id.trim();
    if provider != OPENAI_PROVIDER
        || !matches!(
            model_id,
            OPENAI_GPT_IMAGE_2_MODEL_ID
                | OPENAI_GPT_IMAGE_EDIT_MODEL_ID
                | OPENAI_GPT_4O_MINI_TTS_MODEL_ID
        )
    {
        return Err(OpenAiGenerationProviderError::UnsupportedModel {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        });
    }

    let suffix = if index == 0 {
        String::new()
    } else {
        format!("-{}", index + 1)
    };
    Ok(format!(
        "generated/{}/openai-output{}.{}",
        asset.id,
        suffix,
        openai_output_extension(asset)
    ))
}

fn openai_image_edit_inputs(
    asset: &GeneratedAsset,
) -> Result<Vec<Value>, OpenAiGenerationProviderError> {
    let urls = asset
        .references
        .provider_input_urls
        .iter()
        .map(|url| url.trim())
        .filter(|url| !url.is_empty())
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(OpenAiGenerationProviderError::MissingImageEditInput);
    }
    if urls.len() > 16 {
        return Err(OpenAiGenerationProviderError::TooManyImageEditInputs { count: urls.len() });
    }

    Ok(urls
        .into_iter()
        .map(|url| json!({ "image_url": url }))
        .collect())
}

fn openai_image_size(asset: &GeneratedAsset) -> String {
    asset
        .settings
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| resolution.contains('x'))
        .map(str::to_string)
        .or_else(|| match (asset.settings.width, asset.settings.height) {
            (Some(width), Some(height)) if width > 0 && height > 0 => {
                Some(format!("{width}x{height}"))
            }
            _ => None,
        })
        .unwrap_or_else(|| "auto".to_string())
}

fn openai_image_edit_size(asset: &GeneratedAsset) -> String {
    let resolution = asset.settings.resolution.as_deref().map(str::trim);
    if matches!(
        resolution,
        Some("1024x1024" | "1536x1024" | "1024x1536" | "auto")
    ) {
        return resolution.unwrap_or("auto").to_string();
    }

    match (asset.settings.width, asset.settings.height) {
        (Some(1024), Some(1024)) => "1024x1024".to_string(),
        (Some(1536), Some(1024)) => "1536x1024".to_string(),
        (Some(1024), Some(1536)) => "1024x1536".to_string(),
        _ => "auto".to_string(),
    }
}

fn openai_output_width(asset: &GeneratedAsset) -> u32 {
    if asset.model.id.trim() == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        return 1;
    }
    match asset.settings.width {
        Some(width) if width > 0 => width,
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => 1024,
            Some("1:1") => 1024,
            _ => 1536,
        },
    }
}

fn openai_output_height(asset: &GeneratedAsset) -> u32 {
    if asset.model.id.trim() == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        return 1;
    }
    match asset.settings.height {
        Some(height) if height > 0 => height,
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => 1536,
            Some("1:1") => 1024,
            _ => 864,
        },
    }
}

fn openai_output_duration_seconds(asset: &GeneratedAsset) -> f64 {
    if asset.model.id.trim() == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        return asset
            .settings
            .duration_seconds
            .filter(|duration| duration.is_finite() && *duration > 0.0)
            .unwrap_or(1.0);
    }

    1.0
}

fn openai_output_fps(asset: &GeneratedAsset) -> f64 {
    if asset.model.id.trim() == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        return 1.0;
    }

    1.0
}

fn openai_output_extension(asset: &GeneratedAsset) -> &'static str {
    if asset.model.id.trim() == OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
        "mp3"
    } else {
        "png"
    }
}

fn openai_output_media_id(asset: &GeneratedAsset) -> String {
    openai_output_media_id_for_index(asset, 0)
}

fn openai_output_media_id_for_index(asset: &GeneratedAsset, index: usize) -> String {
    if index == 0 {
        format!("{}-openai-output", asset.id)
    } else {
        format!("{}-openai-output-{}", asset.id, index + 1)
    }
}

fn openai_tts_voice(asset: &GeneratedAsset) -> &str {
    asset
        .settings
        .voice
        .as_deref()
        .map(str::trim)
        .filter(|voice| !voice.is_empty())
        .unwrap_or("alloy")
}
