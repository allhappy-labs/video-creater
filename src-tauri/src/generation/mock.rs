use crate::generation::fal::{
    build_fal_generation_request, FalGenerationProviderError, FAL_AURA_SR_MODEL_ID,
    FAL_FLUX_SCHNELL_MODEL_ID, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID, FAL_KREA_2_TURBO_MODEL_ID,
    FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
    FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID, FAL_SEED_AUDIO_MODEL_ID,
    FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
    FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
    FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
};
use crate::generation::openai::{
    build_openai_image_generation_submission, OpenAiGenerationProviderError,
    OPENAI_GPT_IMAGE_2_MODEL_ID,
};
use crate::generation::replicate::{
    build_replicate_generation_request, ReplicateGenerationProviderError,
    REPLICATE_FLUX_11_PRO_MODEL_ID, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
    REPLICATE_FLUX_DEV_MODEL_ID, REPLICATE_FLUX_SCHNELL_MODEL_ID,
};
use crate::generation::xai::{
    build_xai_image_generation_submission, XAiGenerationProviderError,
    XAI_GROK_IMAGE_QUALITY_MODEL_ID,
};
use crate::project::action::{
    ProjectAction, ProjectActionGeneratedAssetCompletion, ProjectActionGeneratedAssetOutput,
    ProjectActionGeneratedAssetReferences, ProjectActionReplaceGeneratedOutput,
};
use crate::project::model::{GeneratedAsset, GeneratedAssetStatus, JobStatus};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum MockGenerationError {
    #[error("generated asset cannot be completed by mock worker: {0}")]
    NotPending(String),
    #[error("fal generation request is invalid: {0}")]
    Provider(#[from] FalGenerationProviderError),
    #[error("replicate generation request is invalid: {0}")]
    ReplicateProvider(#[from] ReplicateGenerationProviderError),
    #[error("openai generation request is invalid: {0}")]
    OpenAiProvider(#[from] OpenAiGenerationProviderError),
    #[error("xai generation request is invalid: {0}")]
    XAiProvider(#[from] XAiGenerationProviderError),
}

pub fn mock_generation_completion_actions(
    asset: &GeneratedAsset,
    updated_at: &str,
    replacement_item_id: Option<&str>,
) -> Result<Vec<ProjectAction>, MockGenerationError> {
    if !matches!(
        asset.status,
        GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
    ) {
        return Err(MockGenerationError::NotPending(asset.id.clone()));
    }

    let endpoint = mock_generation_endpoint(asset)?;
    let output = mock_output_for_asset(asset, endpoint.as_str());
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
    let run_id = Some(format!("mock-{}", asset.id));
    let updated_at = updated_at.to_string();

    Ok(vec![
        ProjectAction::UpdateJobStatus {
            job_id: asset.id.clone(),
            status: JobStatus::Running,
            updated_at: updated_at.clone(),
            run_id: run_id.clone(),
        },
        ProjectAction::CompleteGeneratedAsset {
            asset_id: asset.id.clone(),
            outputs: vec![output],
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

fn mock_generation_endpoint(asset: &GeneratedAsset) -> Result<String, MockGenerationError> {
    match asset.model.provider.as_str() {
        "fal.ai" => match build_fal_generation_request(asset) {
            Ok(request) => Ok(request.endpoint),
            Err(FalGenerationProviderError::MissingProviderInputUrl { model_id })
                if fal_mock_can_complete_without_provider_inputs(asset, &model_id) =>
            {
                Ok(model_id)
            }
            Err(error) => Err(error.into()),
        },
        "replicate" => Ok(build_replicate_generation_request(asset)?.version),
        "openai" => Ok(build_openai_image_generation_submission(asset)?.model),
        "xai" => Ok(build_xai_image_generation_submission(asset)?.model),
        _ => Ok(build_fal_generation_request(asset)?.endpoint),
    }
}

fn fal_mock_can_complete_without_provider_inputs(asset: &GeneratedAsset, model_id: &str) -> bool {
    match model_id {
        FAL_AURA_SR_MODEL_ID => !asset.references.media_ids.is_empty(),
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID => {
            !asset.references.reference_image_media_refs.is_empty()
        }
        FAL_VIDEO_UPSCALER_MODEL_ID
        | FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
        | FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
        | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID => asset.references.source_video_media_ref.is_some(),
        FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID => {
            asset.references.source_video_media_ref.is_some()
                && !asset.references.reference_image_media_refs.is_empty()
        }
        FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID | FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID => {
            asset.references.first_frame_media_id.is_some()
        }
        FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID => {
            !asset.references.reference_image_media_refs.is_empty()
                && !asset.references.reference_video_media_refs.is_empty()
        }
        _ => false,
    }
}

fn mock_output_for_asset(
    asset: &GeneratedAsset,
    endpoint: &str,
) -> ProjectActionGeneratedAssetOutput {
    let extension = if endpoint == FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID {
        "webp"
    } else if endpoint == FAL_SEED_AUDIO_MODEL_ID {
        "mp3"
    } else if matches!(
        endpoint,
        FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID | FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
    ) {
        "m4a"
    } else if endpoint == FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID {
        "wav"
    } else if matches!(
        endpoint,
        FAL_FLUX_SCHNELL_MODEL_ID
            | FAL_AURA_SR_MODEL_ID
            | FAL_KREA_2_TURBO_MODEL_ID
            | FAL_NANO_BANANA_PRO_EDIT_MODEL_ID
            | REPLICATE_FLUX_SCHNELL_MODEL_ID
            | REPLICATE_FLUX_DEV_MODEL_ID
            | REPLICATE_FLUX_11_PRO_MODEL_ID
            | REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID
            | OPENAI_GPT_IMAGE_2_MODEL_ID
            | XAI_GROK_IMAGE_QUALITY_MODEL_ID
    ) {
        "png"
    } else {
        "mp4"
    };
    let (width, height) = mock_dimensions(asset);
    let (duration_seconds, fps) = if endpoint == FAL_WAN_TEXT_TO_VIDEO_MODEL_ID {
        (
            mock_video_duration(asset),
            asset.settings.fps.unwrap_or(24.0),
        )
    } else if matches!(
        endpoint,
        FAL_SEED_AUDIO_MODEL_ID
            | FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID
            | FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID
            | FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID
    ) {
        (
            asset.settings.duration_seconds.unwrap_or(1.0).max(0.001),
            0.0,
        )
    } else if endpoint == XAI_GROK_IMAGE_QUALITY_MODEL_ID {
        (0.0, 0.0)
    } else {
        (1.0, 1.0)
    };

    ProjectActionGeneratedAssetOutput {
        media_id: format!("{}-mock-output", asset.id),
        relative_path: format!("generated/{}/mock-output.{}", asset.id, extension),
        source_url: None,
        width,
        height,
        duration_seconds,
        fps,
    }
}

fn mock_dimensions(asset: &GeneratedAsset) -> (u32, u32) {
    match (asset.settings.width, asset.settings.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => match asset.settings.aspect_ratio.as_deref().map(str::trim) {
            Some("9:16") => (1080, 1920),
            Some("1:1") => (1024, 1024),
            _ => (1280, 720),
        },
    }
}

fn mock_video_duration(asset: &GeneratedAsset) -> f64 {
    if asset
        .settings
        .duration_seconds
        .is_some_and(|duration| duration > 5.0)
    {
        10.0
    } else {
        5.0
    }
}
