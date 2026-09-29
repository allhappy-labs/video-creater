use reqwest::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
};
use video_creater_lib::generation::fal::{
    build_fal_generated_output_import, build_fal_generation_completion_actions,
    build_fal_generation_request, build_fal_queue_submission, cancel_fal_queue_request,
    download_fal_generated_output, download_fal_generated_output_with_client_cancellable,
    download_fal_generation_result_and_build_completion_actions, fetch_fal_queue_result,
    fetch_fal_queue_status, run_fal_generation_submission_with_client,
    run_fal_generation_submission_with_client_cancellable, submit_fal_queue_submission,
    upload_fal_local_file_to_cdn_with_client, FalGenerationProviderError, FalGenerationRunOptions,
    FalGenerationWorkerError, FalQueueCancelStatus, FalQueueStatusKind, FalQueueStatusResponse,
    FalQueueSubmitError, FalQueueSubmitResponse, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
    FAL_KREA_2_TURBO_MODEL_ID, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
    FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
    FAL_SEED_AUDIO_MODEL_ID, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
    FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
};
use video_creater_lib::generation::mock::mock_generation_completion_actions;
use video_creater_lib::generation::xai::{XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_PROVIDER};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::action::apply_project_action;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, JobStatus, MediaKind,
};
use video_creater_lib::workflows::{temporal_job_summary, TemporalWorkflowKind};

fn generated_asset(model_id: &str, settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: "  floating product shot with crisp rim light  ".to_string(),
        model: GenerationModel {
            provider: "fal.ai".to_string(),
            id: model_id.to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: Vec::new(),
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings,
        outputs: Vec::new(),
        created_at: "2026-06-23T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

#[test]
fn flux_schnell_asset_builds_text_to_image_queue_request() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build flux request");

    assert_eq!(request.endpoint, "fal-ai/flux/schnell");
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "image_size": {
                "width": 1024,
                "height": 1024
            },
            "num_images": 1,
            "output_format": "png",
            "enable_safety_checker": true
        })
    );
}

#[test]
fn flux_schnell_asset_builds_multi_image_queue_request() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            num_images: Some(3),
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build multi-image flux request");

    assert_eq!(request.endpoint, "fal-ai/flux/schnell");
    assert_eq!(request.input["num_images"], json!(3));
}

#[test]
fn krea_2_turbo_asset_builds_text_to_image_queue_request() {
    let asset = generated_asset(
        FAL_KREA_2_TURBO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(576),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build krea request");

    assert_eq!(request.endpoint, FAL_KREA_2_TURBO_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "image_size": {
                "width": 1024,
                "height": 576
            },
            "num_images": 1,
            "output_format": "png",
            "enable_safety_checker": true
        })
    );
}

#[test]
fn recraft_v3_asset_builds_text_to_image_queue_request() {
    let asset = generated_asset(
        FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("1280x720".to_string()),
            quality: Some("vector_illustration".to_string()),
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build recraft request");

    assert_eq!(request.endpoint, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "image_size": {
                "width": 1280,
                "height": 720
            },
            "style": "vector_illustration",
            "enable_safety_checker": true
        })
    );
}

#[test]
fn wan_text_to_video_asset_builds_normalized_queue_request() {
    let asset = generated_asset(
        "fal-ai/wan-25-preview/text-to-video",
        GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(8.0),
            fps: Some(24.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build wan request");

    assert_eq!(request.endpoint, "fal-ai/wan-25-preview/text-to-video");
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "aspect_ratio": "9:16",
            "resolution": "1080p",
            "duration": "10",
            "generate_audio": true,
            "enable_prompt_expansion": true,
            "enable_safety_checker": true
        })
    );
}

#[test]
fn wan_image_to_video_request_maps_ordered_provider_input_urls_to_typed_references() {
    let mut asset = generated_asset(
        "fal-ai/wan/v2.7/image-to-video",
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(false),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec![
            "first-frame".to_string(),
            "last-frame".to_string(),
            "audio-ref".to_string(),
        ],
        first_frame_media_id: Some("first-frame".to_string()),
        last_frame_media_id: Some("last-frame".to_string()),
        reference_audio_media_refs: vec!["audio-ref".to_string()],
        provider_input_urls: vec![
            "https://fal.media/uploads/first.png".to_string(),
            "https://fal.media/uploads/last.png".to_string(),
            "https://fal.media/uploads/audio-ref.wav".to_string(),
        ],
        ..Default::default()
    };

    let request =
        build_fal_generation_request(&asset).expect("build wan image-to-video request with refs");

    assert_eq!(request.endpoint, "fal-ai/wan/v2.7/image-to-video");
    assert_eq!(
        request.input["image_url"],
        json!("https://fal.media/uploads/first.png")
    );
    assert_eq!(
        request.input["end_image_url"],
        json!("https://fal.media/uploads/last.png")
    );
    assert_eq!(
        request.input["audio_url"],
        json!("https://fal.media/uploads/audio-ref.wav")
    );
}

#[test]
fn wan_reference_to_video_request_maps_provider_urls_to_reference_arrays() {
    let mut asset = generated_asset(
        "fal-ai/wan/v2.7/reference-to-video",
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(6.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(false),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["style-ref".to_string(), "motion-ref".to_string()],
        reference_image_media_refs: vec!["style-ref".to_string()],
        reference_video_media_refs: vec!["motion-ref".to_string()],
        provider_input_urls: vec![
            "https://fal.media/uploads/style.png".to_string(),
            "https://fal.media/uploads/motion.mp4".to_string(),
        ],
        ..Default::default()
    };

    let request =
        build_fal_generation_request(&asset).expect("build wan reference-to-video request");

    assert_eq!(request.endpoint, "fal-ai/wan/v2.7/reference-to-video");
    assert_eq!(
        request.input["reference_image_urls"],
        json!(["https://fal.media/uploads/style.png"])
    );
    assert_eq!(
        request.input["reference_video_urls"],
        json!(["https://fal.media/uploads/motion.mp4"])
    );
    assert_eq!(request.input["duration"], json!(6));
    assert_eq!(request.input["resolution"], json!("720p"));
    assert_eq!(request.input["aspect_ratio"], json!("16:9"));
}

#[test]
fn wan_video_to_video_request_maps_source_video_url_to_official_rest_input() {
    let mut asset = generated_asset(
        FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(false),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["source-video".to_string()],
        source_video_media_ref: Some("source-video".to_string()),
        provider_input_urls: vec!["https://fal.media/uploads/source.mp4".to_string()],
        ..Default::default()
    };

    let request =
        build_fal_generation_request(&asset).expect("build wan video-to-video request with source");

    assert_eq!(request.endpoint, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "video_url": "https://fal.media/uploads/source.mp4",
            "prompt": "floating product shot with crisp rim light",
            "strength": 0.9,
            "num_frames": 96,
            "frames_per_second": 24,
            "resolution": "720p",
            "aspect_ratio": "16:9",
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
        })
    );
}

#[test]
fn wan_video_to_video_requires_source_video_provider_url() {
    let asset = generated_asset(
        FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let error = build_fal_generation_request(&asset).expect_err("missing source video url");

    assert!(matches!(
        error,
        FalGenerationProviderError::MissingProviderInputUrl { model_id }
            if model_id == FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID
    ));
}

#[test]
fn kling_motion_control_request_maps_source_video_and_reference_image_urls() {
    let mut asset = generated_asset(
        FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
        GeneratedAssetSettings {
            duration_seconds: Some(8.0),
            generate_audio: Some(false),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["source-video".to_string(), "character-ref".to_string()],
        source_video_media_ref: Some("source-video".to_string()),
        reference_image_media_refs: vec!["character-ref".to_string()],
        provider_input_urls: vec![
            "https://fal.media/uploads/source.mp4".to_string(),
            "https://fal.media/uploads/character.png".to_string(),
        ],
        ..Default::default()
    };

    let request = build_fal_generation_request(&asset).expect("build Kling motion-control request");

    assert_eq!(request.endpoint, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "video_url": "https://fal.media/uploads/source.mp4",
            "image_url": "https://fal.media/uploads/character.png",
            "keep_original_sound": false,
            "character_orientation": "image"
        })
    );
}

#[test]
fn wan_text_to_video_request_uses_explicit_resolution_and_audio_setting() {
    let asset: GeneratedAsset = serde_json::from_value(json!({
        "schemaVersion": 1,
        "id": "generated-video-explicit-settings",
        "kind": "video",
        "status": "queued",
        "prompt": "cinematic product launch",
        "model": {
            "provider": "fal.ai",
            "id": "fal-ai/wan-25-preview/text-to-video"
        },
        "references": {
            "mediaIds": [],
            "firstFrameMediaId": null,
            "lastFrameMediaId": null
        },
        "settings": {
            "width": 1280,
            "height": 720,
            "durationSeconds": 4,
            "fps": 24,
            "aspectRatio": "16:9",
            "resolution": "1080p",
            "generateAudio": false
        },
        "outputs": [],
        "createdAt": "2026-07-06T12:00:00Z",
        "parentAssetId": null,
        "retryOfAssetId": null
    }))
    .expect("deserialize generated asset with Palmier video settings");

    let request = build_fal_generation_request(&asset).expect("build wan request");

    assert_eq!(request.input["resolution"], json!("1080p"));
    assert_eq!(request.input["generate_audio"], json!(false));
}

#[test]
fn kling_text_to_video_asset_builds_v3_queue_request() {
    let asset = generated_asset(
        FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(12.0),
            fps: Some(24.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: Some(false),
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build kling text-to-video request");

    assert_eq!(
        request.endpoint,
        FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID
    );
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "aspect_ratio": "9:16",
            "duration": "12",
            "generate_audio": false
        })
    );
}

#[test]
fn kling_image_to_video_request_maps_first_and_last_frame_provider_urls() {
    let mut asset = generated_asset(
        FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["first-frame".to_string(), "last-frame".to_string()],
        first_frame_media_id: Some("first-frame".to_string()),
        last_frame_media_id: Some("last-frame".to_string()),
        provider_input_urls: vec![
            "https://fal.media/uploads/first.png".to_string(),
            "https://fal.media/uploads/last.png".to_string(),
        ],
        ..Default::default()
    };

    let request = build_fal_generation_request(&asset).expect("build kling image-to-video request");

    assert_eq!(request.endpoint, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "start_image_url": "https://fal.media/uploads/first.png",
            "end_image_url": "https://fal.media/uploads/last.png",
            "duration": "4",
            "generate_audio": true
        })
    );
}

#[test]
fn kling_image_to_video_requires_first_frame_provider_url() {
    let asset = generated_asset(
        FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );

    let error = build_fal_generation_request(&asset).expect_err("missing first frame URL");

    assert_eq!(
        error,
        FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID.to_string()
        }
    );
}

#[test]
fn kling_video_output_import_uses_video_metadata() {
    let asset = generated_asset(
        FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "video": {
                "url": "https://v3.fal.media/files/kling/output.mp4",
                "width": 1280,
                "height": 720,
                "duration": 4.0,
                "fps": 24.0
            }
        }),
        "generated/generated-shot-1/fal-output.mp4",
    )
    .expect("build kling output import");

    assert_eq!(
        output.source_url,
        "https://v3.fal.media/files/kling/output.mp4"
    );
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.mp4"
    );
    assert_eq!(output.output.width, 1280);
    assert_eq!(output.output.height, 720);
    assert_eq!(output.output.duration_seconds, 4.0);
    assert_eq!(output.output.fps, 24.0);
}

#[test]
fn kling_video_output_import_falls_back_to_asset_settings_when_metadata_is_missing() {
    let asset = generated_asset(
        FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "video": {
                "url": "https://v3.fal.media/files/kling/output.mp4"
            }
        }),
        "generated/generated-shot-1/fal-output.mp4",
    )
    .expect("build kling output import");

    assert_eq!(output.output.width, 1280);
    assert_eq!(output.output.height, 720);
    assert_eq!(output.output.duration_seconds, 4.0);
    assert_eq!(output.output.fps, 24.0);
}

#[test]
fn seed_audio_asset_builds_text_to_audio_queue_request() {
    let asset = generated_asset(
        FAL_SEED_AUDIO_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(12.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build seed audio request");

    assert_eq!(request.endpoint, FAL_SEED_AUDIO_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "output_format": "mp3",
            "sample_rate": 24000,
            "speed": 1.0,
            "volume": 1.0
        })
    );
}

#[test]
fn sonilo_text_to_music_asset_builds_prompt_duration_request() {
    let asset = generated_asset(
        FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(90.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_fal_generation_request(&asset).expect("build sonilo text-to-music request");

    assert_eq!(request.endpoint, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "duration": 90,
            "num_samples": 1
        })
    );
}

#[test]
fn nano_banana_pro_edit_asset_builds_image_reference_queue_request() {
    let mut asset = generated_asset(
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(2048),
            height: Some(2048),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: Some("2K".to_string()),
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["style-ref".to_string(), "subject-ref".to_string()],
        reference_image_media_refs: vec!["style-ref".to_string(), "subject-ref".to_string()],
        provider_input_urls: vec![
            "https://v3.fal.media/files/images/style.png".to_string(),
            "https://v3.fal.media/files/images/subject.png".to_string(),
        ],
        ..Default::default()
    };

    let request = build_fal_generation_request(&asset).expect("build nano banana edit request");

    assert_eq!(request.endpoint, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "image_urls": [
                "https://v3.fal.media/files/images/style.png",
                "https://v3.fal.media/files/images/subject.png"
            ],
            "num_images": 1,
            "aspect_ratio": "1:1",
            "resolution": "2K",
            "output_format": "png",
            "safety_tolerance": "4"
        })
    );
}

#[test]
fn nano_banana_pro_edit_requires_provider_input_urls() {
    let asset = generated_asset(
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let error = build_fal_generation_request(&asset).expect_err("missing image URLs");

    assert_eq!(
        error,
        FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_NANO_BANANA_PRO_EDIT_MODEL_ID.to_string()
        }
    );
}

#[test]
fn nano_banana_pro_edit_output_import_uses_image_metadata_fallback() {
    let asset = generated_asset(
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(2048),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("2K".to_string()),
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [
                {
                    "url": "https://v3.fal.media/files/nano/edit-output.png",
                    "content_type": "image/png"
                }
            ],
            "description": ""
        }),
        "generated/generated-shot-1/fal-output.png",
    )
    .expect("build nano banana output import");

    assert_eq!(
        output.source_url,
        "https://v3.fal.media/files/nano/edit-output.png"
    );
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.png"
    );
    assert_eq!(output.output.width, 2048);
    assert_eq!(output.output.height, 1024);
    assert_eq!(output.output.duration_seconds, 1.0);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn sonilo_video_to_music_asset_builds_prompt_style_duration_request() {
    let mut asset = generated_asset(
        FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(12.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            style_instructions: Some("bright synth pulse with tight drums".to_string()),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references.source_video_media_ref = Some("source-video".to_string());
    asset.references.provider_input_urls =
        vec!["https://v3.fal.media/files/source-video.mp4".to_string()];

    let request = build_fal_generation_request(&asset).expect("build sonilo request");

    assert_eq!(request.endpoint, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "video_url": "https://v3.fal.media/files/source-video.mp4",
            "prompt": "floating product shot with crisp rim light\n\nStyle: bright synth pulse with tight drums",
            "duration": 12.0,
            "num_samples": 1
        })
    );
}

#[test]
fn mirelo_video_to_audio_asset_builds_prompt_guided_request() {
    let mut asset = generated_asset(
        FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(9.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references.source_video_media_ref = Some("source-video".to_string());
    asset.references.provider_input_urls =
        vec!["https://v3.fal.media/files/source-video.mp4".to_string()];

    let request = build_fal_generation_request(&asset).expect("build mirelo request");

    assert_eq!(request.endpoint, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "video_url": "https://v3.fal.media/files/source-video.mp4",
            "text_prompt": "floating product shot with crisp rim light",
            "num_samples": 1,
            "duration": 9.0
        })
    );
}

#[test]
fn video_to_audio_models_require_provider_input_url() {
    let asset = generated_asset(
        FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(9.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let error = build_fal_generation_request(&asset).expect_err("missing video URL");

    assert_eq!(
        error,
        FalGenerationProviderError::MissingProviderInputUrl {
            model_id: FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID.to_string()
        }
    );
}

#[test]
fn sonilo_audio_output_import_reads_audio_object_with_duration_fallback() {
    let asset = generated_asset(
        FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(12.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "audio": {
                "url": "https://v3b.fal.media/files/music/output.m4a",
                "content_type": "audio/mp4"
            },
            "audios": []
        }),
        "generated/generated-shot-1/fal-output.m4a",
    )
    .expect("build sonilo output import");

    assert_eq!(
        output.source_url,
        "https://v3b.fal.media/files/music/output.m4a"
    );
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.m4a"
    );
    assert_eq!(output.output.duration_seconds, 12.0);
    assert_eq!(output.output.width, 1);
    assert_eq!(output.output.height, 1);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn mirelo_audio_output_import_reads_audio_array() {
    let asset = generated_asset(
        FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(7.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "audio": [
                {
                    "url": "https://v3b.fal.media/files/sfx/output.wav",
                    "content_type": "audio/wav",
                    "duration": 7.5
                }
            ]
        }),
        "generated/generated-shot-1/fal-output.wav",
    )
    .expect("build mirelo output import");

    assert_eq!(
        output.source_url,
        "https://v3b.fal.media/files/sfx/output.wav"
    );
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.wav"
    );
    assert_eq!(output.output.duration_seconds, 7.5);
}

#[test]
fn aura_sr_image_upscale_asset_builds_image_to_image_queue_request() {
    let asset: GeneratedAsset = serde_json::from_value(json!({
        "schemaVersion": 1,
        "id": "generated-upscale-1",
        "kind": "generated",
        "status": "queued",
        "prompt": "Upscale product still",
        "model": {
            "provider": "fal.ai",
            "id": "fal-ai/aura-sr"
        },
        "references": {
            "mediaIds": ["media-1"],
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "providerInputUrls": ["https://fal.media/uploads/source.png"]
        },
        "settings": {
            "width": 4096,
            "height": 4096,
            "aspectRatio": "1:1"
        },
        "outputs": [],
        "createdAt": "2026-06-23T12:00:00Z",
        "parentAssetId": null,
        "retryOfAssetId": null
    }))
    .expect("deserialize aura sr asset");

    let request = build_fal_generation_request(&asset).expect("build aura sr request");

    assert_eq!(request.endpoint, "fal-ai/aura-sr");
    assert_eq!(
        request.input,
        json!({
            "image_url": "https://fal.media/uploads/source.png",
            "upscale_factor": 4,
            "overlapping_tiles": true,
            "checkpoint": "v2"
        })
    );
}

#[test]
fn fal_video_upscale_asset_builds_video_to_video_queue_request() {
    let mut asset = generated_asset(
        FAL_VIDEO_UPSCALER_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(3840),
            height: Some(2160),
            duration_seconds: Some(12.5),
            fps: Some(30.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references.media_ids = vec!["media-video-1".to_string()];
    asset.references.provider_input_urls =
        vec!["https://fal.media/uploads/source-video.mp4".to_string()];

    let request = build_fal_generation_request(&asset).expect("build fal video upscale request");

    assert_eq!(request.endpoint, FAL_VIDEO_UPSCALER_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "video_url": "https://fal.media/uploads/source-video.mp4",
            "scale": 2
        })
    );
}

#[test]
fn fal_queue_submission_uses_async_queue_endpoint_without_credentials() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let submission = build_fal_queue_submission(&asset).expect("build fal queue submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://queue.fal.run/fal-ai/flux/schnell");
    assert_eq!(submission.endpoint, "fal-ai/flux/schnell");
    assert_eq!(submission.provider, FAL_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "image_size": {
                "width": 1024,
                "height": 1024
            },
            "num_images": 1,
            "output_format": "png",
            "enable_safety_checker": true
        })
    );
}

#[test]
fn submits_fal_queue_request_with_secret_only_in_authorization_header() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    let mut submission = build_fal_queue_submission(&asset).expect("build fal queue submission");
    let Some((base_url, request_handle)) = spawn_fal_submit_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/flux/schnell");

    let response =
        submit_fal_queue_submission(&submission, "unit-test-token").expect("submit fal queue");
    let request = request_handle.join().expect("request capture");

    assert_eq!(response.request_id, "req-123");
    assert_eq!(
        response.status_url,
        "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/status"
    );
    assert!(request.starts_with("POST /fal-ai/flux/schnell HTTP/1.1"));
    assert!(request.contains("authorization: Key unit-test-token"));
    assert!(request.contains(r#""prompt":"floating product shot with crisp rim light""#));
    assert!(!request_body(&request).contains("unit-test-token"));
}

#[test]
fn uploads_local_file_to_fal_cdn_with_rest_initiate_and_put() {
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source_path = source_dir.path().join("source.png");
    std::fs::write(&source_path, b"source-pixels").expect("source image");
    let Some((base_url, request_handle)) = spawn_fal_cdn_upload_server() else {
        return;
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let uploaded_url = upload_fal_local_file_to_cdn_with_client(
        &client,
        &base_url,
        &source_path,
        "unit-test-token",
    )
    .expect("upload local file to fal CDN");

    let requests = request_handle.join().expect("upload requests");
    assert_eq!(
        uploaded_url,
        "https://v3.fal.media/files/video-creater/source.png"
    );
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0].starts_with("POST /storage/upload/initiate?storage_type=fal-cdn-v3 HTTP/1.1")
    );
    assert!(requests[0].contains("authorization: Key unit-test-token"));
    assert!(request_body(&requests[0]).contains("\"file_name\":\"source.png\""));
    assert!(request_body(&requests[0]).contains("\"content_type\":\"image/png\""));
    assert!(requests[1].starts_with("PUT /upload/source.png HTTP/1.1"));
    assert!(requests[1].contains("content-type: image/png"));
    assert_eq!(request_body(&requests[1]).as_bytes(), b"source-pixels");
    assert!(!requests
        .iter()
        .any(|request| request.contains("fal files upload")));
}

#[test]
fn fal_queue_submission_requires_explicit_credential_and_ignores_process_environment() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    let submission = build_fal_queue_submission(&asset).expect("build fal queue submission");
    std::env::set_var("FAL_KEY", "environment-canary-must-be-ignored");

    let error = submit_fal_queue_submission(&submission, "").expect_err("missing credential");
    std::env::remove_var("FAL_KEY");

    assert_eq!(
        error,
        FalQueueSubmitError::EmptyCredential {
            provider: FAL_PROVIDER.to_string()
        }
    );
    assert!(!error
        .to_string()
        .contains("environment-canary-must-be-ignored"));
}

#[test]
fn fal_queue_submit_response_deserializes_tracking_urls() {
    let response: FalQueueSubmitResponse = serde_json::from_value(json!({
        "request_id": "req-123",
        "response_url": "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response",
        "status_url": "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/status",
        "cancel_url": "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/cancel",
        "queue_position": 0
    }))
    .expect("deserialize fal submit response");

    assert_eq!(response.request_id, "req-123");
    assert_eq!(
        response.response_url,
        "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response"
    );
    assert_eq!(
        response.status_url,
        "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/status"
    );
    assert_eq!(
        response.cancel_url,
        "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/cancel"
    );
    assert_eq!(response.queue_position, Some(0));
}

#[test]
fn fetches_fal_queue_status_with_logs_using_secret_only_in_authorization_header() {
    let body = r#"{"status":"IN_PROGRESS","request_id":"req-123","response_url":"https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response","logs":[{"message":"Generating image...","timestamp":"2026-02-17T10:30:02.456Z"}]}"#;
    let Some((base_url, request_handle)) = spawn_fal_json_server(body) else {
        return;
    };
    let status_url = format!("{base_url}/fal-ai/flux/schnell/requests/req-123/status");

    let status =
        fetch_fal_queue_status(&status_url, "unit-test-token", true).expect("fetch fal status");
    let request = request_handle.join().expect("request capture");

    assert_eq!(status.status, FalQueueStatusKind::InProgress);
    assert_eq!(status.request_id, "req-123");
    assert_eq!(status.logs[0].message, "Generating image...");
    assert!(request.starts_with("GET /fal-ai/flux/schnell/requests/req-123/status?logs=1 HTTP/1.1"));
    assert!(request.contains("authorization: Key unit-test-token"));
    assert!(!request_body(&request).contains("unit-test-token"));
}

#[test]
fn fetches_fal_queue_status_replaces_existing_logs_query_parameter() {
    let body = r#"{"status":"IN_PROGRESS","request_id":"req-123","response_url":"https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response","logs":[]}"#;
    let Some((base_url, request_handle)) = spawn_fal_json_server(body) else {
        return;
    };
    let status_url =
        format!("{base_url}/fal-ai/flux/schnell/requests/req-123/status?source=worker&logs=0");

    let status =
        fetch_fal_queue_status(&status_url, "unit-test-token", true).expect("fetch fal status");
    let request = request_handle.join().expect("request capture");

    assert_eq!(status.status, FalQueueStatusKind::InProgress);
    assert!(request
        .starts_with("GET /fal-ai/flux/schnell/requests/req-123/status?source=worker&logs=1 "));
    assert!(!request.contains("logs=0"));
}

#[test]
fn fetches_fal_queue_status_treats_null_logs_as_empty() {
    let body = r#"{"status":"IN_PROGRESS","request_id":"req-123","response_url":"https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response","logs":null}"#;
    let Some((base_url, request_handle)) = spawn_fal_json_server(body) else {
        return;
    };
    let status_url = format!("{base_url}/fal-ai/flux/schnell/requests/req-123/status");

    let status =
        fetch_fal_queue_status(&status_url, "unit-test-token", false).expect("fetch fal status");
    request_handle.join().expect("request capture");

    assert_eq!(status.status, FalQueueStatusKind::InProgress);
    assert!(status.logs.is_empty());
}

#[test]
fn fetches_fal_queue_result_using_secret_only_in_authorization_header() {
    let body = r#"{"images":[{"url":"https://v3.fal.media/files/rabbit/abc123.png","width":1024,"height":1024,"content_type":"image/png"}],"prompt":"a sunset over mountains","seed":42,"has_nsfw_concepts":[false]}"#;
    let Some((base_url, request_handle)) = spawn_fal_json_server(body) else {
        return;
    };
    let response_url = format!("{base_url}/fal-ai/flux/schnell/requests/req-123/response");

    let result = fetch_fal_queue_result(&response_url, "unit-test-token").expect("fetch result");
    let request = request_handle.join().expect("request capture");

    assert_eq!(
        result["images"][0]["url"],
        json!("https://v3.fal.media/files/rabbit/abc123.png")
    );
    assert_eq!(result["seed"], json!(42));
    assert!(request.starts_with("GET /fal-ai/flux/schnell/requests/req-123/response HTTP/1.1"));
    assert!(request.contains("authorization: Key unit-test-token"));
    assert!(!request_body(&request).contains("unit-test-token"));
}

#[test]
fn cancels_fal_queue_request_using_secret_only_in_authorization_header() {
    let Some((base_url, request_handle)) = spawn_fal_status_response_server(
        202,
        "application/json",
        br#"{"status":"CANCELLATION_REQUESTED"}"#.to_vec(),
    ) else {
        return;
    };
    let cancel_url = format!("{base_url}/fal-ai/flux/schnell/requests/req-123/cancel");

    let response =
        cancel_fal_queue_request(&cancel_url, "unit-test-token").expect("cancel fal request");
    let request = request_handle.join().expect("request capture");

    assert_eq!(response.status, FalQueueCancelStatus::CancellationRequested);
    assert!(request.starts_with("PUT /fal-ai/flux/schnell/requests/req-123/cancel HTTP/1.1"));
    assert!(request.contains("authorization: Key unit-test-token"));
    assert!(!request_body(&request).contains("unit-test-token"));
}

#[test]
fn decodes_fal_already_completed_cancel_response_from_http_400() {
    let Some((base_url, request_handle)) = spawn_fal_status_response_server(
        400,
        "application/json",
        br#"{"status":"ALREADY_COMPLETED"}"#.to_vec(),
    ) else {
        return;
    };
    let cancel_url = format!("{base_url}/fal-ai/flux/schnell/requests/req-123/cancel");

    let response =
        cancel_fal_queue_request(&cancel_url, "unit-test-token").expect("decode terminal cancel");
    request_handle.join().expect("request capture");

    assert_eq!(response.status, FalQueueCancelStatus::AlreadyCompleted);
}

fn spawn_fal_submit_server() -> Option<(String, thread::JoinHandle<String>)> {
    spawn_fal_json_server(
        r#"{"request_id":"req-123","response_url":"https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response","status_url":"https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/status","cancel_url":"https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/cancel","queue_position":0}"#,
    )
}

fn spawn_fal_cdn_upload_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal upload server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let base_url = format!("http://{address}");
    let upload_url = format!("{base_url}/upload/source.png");
    let responses = vec![
        fal_json_response(format!(
            r#"{{"file_url":"https://v3.fal.media/files/video-creater/source.png","upload_url":"{upload_url}"}}"#
        )),
        ("application/json", b"{}".to_vec()),
    ];

    Some(spawn_fal_sequence_server(listener, responses, base_url))
}

fn spawn_fal_json_server(body: &'static str) -> Option<(String, thread::JoinHandle<String>)> {
    spawn_fal_response_server("application/json", body.as_bytes().to_vec())
}

fn spawn_fal_binary_server(body: &'static [u8]) -> Option<(String, thread::JoinHandle<String>)> {
    spawn_fal_response_server("application/octet-stream", body.to_vec())
}

fn spawn_fal_response_server(
    content_type: &'static str,
    body: Vec<u8>,
) -> Option<(String, thread::JoinHandle<String>)> {
    spawn_fal_status_response_server(200, content_type, body)
}

fn spawn_fal_status_response_server(
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
) -> Option<(String, thread::JoinHandle<String>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal response server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept fal request");
        let request = read_http_request(&mut stream);
        let response = format!(
            "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("write fal response headers");
        stream.write_all(&body).expect("write fal response body");
        request
    });

    Some((format!("http://{address}"), handle))
}

fn spawn_fal_generation_run_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal run server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let base_url = format!("http://{address}");
    let responses = vec![
        fal_json_response(format!(
            r#"{{"request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","status_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/status","cancel_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/cancel","queue_position":0}}"#
        )),
        fal_json_response(format!(
            r#"{{"status":"IN_PROGRESS","request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","logs":[{{"message":"Generating image...","timestamp":"2026-02-17T10:30:02.456Z"}}]}}"#
        )),
        fal_json_response(format!(
            r#"{{"status":"COMPLETED","request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","logs":[{{"message":"Done.","timestamp":"2026-02-17T10:30:05.789Z"}}],"metrics":{{"inference_time":3.42}}}}"#
        )),
        fal_json_response(format!(
            r#"{{"images":[{{"url":"{base_url}/generated/hero.png","width":1024,"height":1024,"content_type":"image/png"}}],"prompt":"floating product shot with crisp rim light","seed":42,"has_nsfw_concepts":[false]}}"#
        )),
        ("application/octet-stream", b"result-image-bytes".to_vec()),
    ];

    Some(spawn_fal_sequence_server(listener, responses, base_url))
}

fn spawn_fal_transient_status_run_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal transient-status server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let base_url = format!("http://{address}");
    let responses = vec![
        (
            200,
            fal_json_response(format!(
                r#"{{"request_id":"req-transient","response_url":"{base_url}/fal-ai/wan/requests/req-transient","status_url":"{base_url}/fal-ai/wan/requests/req-transient/status","cancel_url":"{base_url}/fal-ai/wan/requests/req-transient/cancel","queue_position":0}}"#
            )),
        ),
        (
            422,
            fal_json_response(r#"{"detail":"request status is not available yet"}"#.to_string()),
        ),
        (
            200,
            fal_json_response(format!(
                r#"{{"status":"COMPLETED","request_id":"req-transient","response_url":"{base_url}/fal-ai/wan/requests/req-transient","logs":[],"metrics":{{"inference_time":0.1}}}}"#
            )),
        ),
        (
            200,
            fal_json_response(format!(
                r#"{{"images":[{{"url":"{base_url}/generated/transient.png","width":1024,"height":1024,"content_type":"image/png"}}],"prompt":"transient status retry","seed":7,"has_nsfw_concepts":[false]}}"#
            )),
        ),
        (
            200,
            ("application/octet-stream", b"transient-result".to_vec()),
        ),
    ];
    let handle = thread::spawn(move || {
        let mut requests = Vec::with_capacity(responses.len());
        for (status, (content_type, body)) in responses {
            let (mut stream, _) = listener.accept().expect("accept fal request");
            requests.push(read_http_request(&mut stream));
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write headers");
            stream.write_all(&body).expect("write body");
        }
        requests
    });
    Some((base_url, handle))
}

fn spawn_fal_poll_limit_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal poll limit server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let base_url = format!("http://{address}");
    let responses = vec![
        fal_json_response(format!(
            r#"{{"request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","status_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/status","cancel_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/cancel","queue_position":0}}"#
        )),
        fal_json_response(format!(
            r#"{{"status":"IN_PROGRESS","request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","logs":[{{"message":"Still running.","timestamp":"2026-02-17T10:30:02.456Z"}}]}}"#
        )),
    ];

    Some(spawn_fal_sequence_server(listener, responses, base_url))
}

fn spawn_fal_streaming_download_server() -> Option<(String, thread::JoinHandle<()>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal streaming server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept fal download");
        let _ = read_http_request(&mut stream);
        let chunk = vec![b'x'; 64 * 1024];
        let headers = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            chunk.len() * 2,
        );
        stream.write_all(headers.as_bytes()).expect("write headers");
        stream.write_all(&chunk).expect("write first chunk");
        stream.flush().expect("flush first chunk");
        thread::sleep(Duration::from_millis(150));
        let _ = stream.write_all(&chunk);
    });
    Some((format!("http://{address}"), handle))
}

fn spawn_fal_provider_error_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind fal provider error server: {error}"),
    };
    let address = listener.local_addr().expect("server address");
    let base_url = format!("http://{address}");
    let responses = vec![
        fal_json_response(format!(
            r#"{{"request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","status_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/status","cancel_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/cancel","queue_position":0}}"#
        )),
        fal_json_response(format!(
            r#"{{"status":"COMPLETED","request_id":"req-123","response_url":"{base_url}/fal-ai/flux/schnell/requests/req-123/response","error_type":"provider_error","error":"provider echoed unit-test-token in diagnostic text"}}"#
        )),
    ];

    Some(spawn_fal_sequence_server(listener, responses, base_url))
}

fn fal_json_response(body: String) -> (&'static str, Vec<u8>) {
    ("application/json", body.into_bytes())
}

fn spawn_fal_sequence_server(
    listener: TcpListener,
    responses: Vec<(&'static str, Vec<u8>)>,
    base_url: String,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for (content_type, body) in responses {
            let (mut stream, _) = listener.accept().expect("accept fal request");
            let request = read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write fal response headers");
            stream.write_all(&body).expect("write fal response body");
            requests.push(request);
        }
        requests
    });

    (base_url, handle)
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        let request = String::from_utf8_lossy(&buffer);
        if let Some((headers, _)) = request.split_once("\r\n\r\n") {
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let header_len = headers.len() + 4;
            if buffer.len() >= header_len + content_length {
                break;
            }
        }
    }

    String::from_utf8(buffer).expect("request utf8")
}

fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("")
}

#[test]
fn fal_queue_status_response_deserializes_lifecycle_states() {
    let queued: FalQueueStatusResponse = serde_json::from_value(json!({
        "status": "IN_QUEUE",
        "request_id": "req-123",
        "queue_position": 2,
        "response_url": "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response"
    }))
    .expect("deserialize queued fal status");
    assert_eq!(queued.status, FalQueueStatusKind::InQueue);
    assert_eq!(queued.queue_position, Some(2));
    assert!(!queued.is_terminal());

    let in_progress: FalQueueStatusResponse = serde_json::from_value(json!({
        "status": "IN_PROGRESS",
        "request_id": "req-123",
        "response_url": "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response",
        "logs": [
            {
                "message": "Generating image...",
                "timestamp": "2026-02-17T10:30:02.456Z"
            }
        ]
    }))
    .expect("deserialize in-progress fal status");
    assert_eq!(in_progress.status, FalQueueStatusKind::InProgress);
    assert_eq!(in_progress.logs.len(), 1);
    assert_eq!(in_progress.logs[0].message, "Generating image...");
    assert_eq!(
        in_progress.logs[0].timestamp.as_deref(),
        Some("2026-02-17T10:30:02.456Z")
    );
    assert!(!in_progress.is_terminal());

    let completed: FalQueueStatusResponse = serde_json::from_value(json!({
        "status": "COMPLETED",
        "request_id": "req-123",
        "response_url": "https://queue.fal.run/fal-ai/flux/schnell/requests/req-123/response",
        "logs": [
            {
                "message": "Done.",
                "timestamp": "2026-02-17T10:30:05.789Z"
            }
        ],
        "metrics": {
            "inference_time": 3.42
        }
    }))
    .expect("deserialize completed fal status");
    assert_eq!(completed.status, FalQueueStatusKind::Completed);
    assert_eq!(
        completed.metrics.as_ref().expect("metrics").inference_time,
        Some(3.42)
    );
    assert!(completed.is_terminal());
}

#[test]
fn fal_generation_request_rejects_unsupported_models() {
    let asset = generated_asset(
        "seedance-2-fast",
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let error = build_fal_generation_request(&asset).expect_err("unsupported model");

    assert_eq!(
        error,
        FalGenerationProviderError::UnsupportedModel {
            provider: "fal.ai".to_string(),
            model_id: "seedance-2-fast".to_string()
        }
    );
}

#[test]
fn flux_schnell_result_builds_generated_output_import_metadata() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [
                {
                    "url": "https://fal.media/generated/hero.png",
                    "content_type": "image/png",
                    "width": 1024,
                    "height": 1024
                }
            ],
            "prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.png",
    )
    .expect("build output import");

    assert_eq!(output.source_url, "https://fal.media/generated/hero.png");
    assert_eq!(
        output.output.source_url.as_deref(),
        Some("https://fal.media/generated/hero.png")
    );
    assert_eq!(output.output.media_id, "generated-shot-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.png"
    );
    assert_eq!(output.output.width, 1024);
    assert_eq!(output.output.height, 1024);
    assert_eq!(output.output.duration_seconds, 1.0);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn krea_2_turbo_result_builds_generated_output_import_metadata() {
    let asset = generated_asset(
        FAL_KREA_2_TURBO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(576),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [
                {
                    "url": "https://v3b.fal.media/files/b/0a9f7455/krea-output.jpg",
                    "content_type": "image/jpeg",
                    "width": 1024,
                    "height": 576
                }
            ],
            "prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.png",
    )
    .expect("build krea output import");

    assert_eq!(
        output.source_url,
        "https://v3b.fal.media/files/b/0a9f7455/krea-output.jpg"
    );
    assert_eq!(
        output.output.source_url.as_deref(),
        Some("https://v3b.fal.media/files/b/0a9f7455/krea-output.jpg")
    );
    assert_eq!(output.output.media_id, "generated-shot-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.png"
    );
    assert_eq!(output.output.width, 1024);
    assert_eq!(output.output.height, 576);
    assert_eq!(output.output.duration_seconds, 1.0);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn recraft_v3_result_builds_generated_output_import_metadata() {
    let asset = generated_asset(
        FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("1280x720".to_string()),
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [
                {
                    "url": "https://fal.media/files/penguin/recraft-image.webp",
                    "content_type": "image/webp",
                    "width": 1280,
                    "height": 720
                }
            ]
        }),
        "generated/generated-shot-1/fal-output.webp",
    )
    .expect("build recraft output import");

    assert_eq!(
        output.source_url,
        "https://fal.media/files/penguin/recraft-image.webp"
    );
    assert_eq!(
        output.output.source_url.as_deref(),
        Some("https://fal.media/files/penguin/recraft-image.webp")
    );
    assert_eq!(output.output.media_id, "generated-shot-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.webp"
    );
    assert_eq!(output.output.width, 1280);
    assert_eq!(output.output.height, 720);
    assert_eq!(output.output.duration_seconds, 1.0);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn aura_sr_result_builds_generated_output_import_metadata() {
    let asset: GeneratedAsset = serde_json::from_value(json!({
        "schemaVersion": 1,
        "id": "generated-upscale-1",
        "kind": "generated",
        "status": "queued",
        "prompt": "Upscale product still",
        "model": {
            "provider": "fal.ai",
            "id": "fal-ai/aura-sr"
        },
        "references": {
            "mediaIds": ["media-1"],
            "firstFrameMediaId": null,
            "lastFrameMediaId": null,
            "providerInputUrls": ["https://fal.media/uploads/source.png"]
        },
        "settings": {
            "width": 4096,
            "height": 4096,
            "aspectRatio": "1:1"
        },
        "outputs": [],
        "createdAt": "2026-06-23T12:00:00Z",
        "parentAssetId": null,
        "retryOfAssetId": null
    }))
    .expect("deserialize aura sr asset");

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "image": {
                "url": "https://fal.media/generated/upscaled.png",
                "content_type": "image/png",
                "width": 4096,
                "height": 4096
            }
        }),
        "generated/generated-upscale-1/fal-output.png",
    )
    .expect("build aura sr output import");

    assert_eq!(
        output.source_url,
        "https://fal.media/generated/upscaled.png"
    );
    assert_eq!(output.output.media_id, "generated-upscale-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-upscale-1/fal-output.png"
    );
    assert_eq!(output.output.width, 4096);
    assert_eq!(output.output.height, 4096);
    assert_eq!(output.output.duration_seconds, 1.0);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn fal_video_upscale_result_builds_generated_output_import_metadata() {
    let mut asset = generated_asset(
        FAL_VIDEO_UPSCALER_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(3840),
            height: Some(2160),
            duration_seconds: Some(12.5),
            fps: Some(30.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references.media_ids = vec!["media-video-1".to_string()];
    asset.references.provider_input_urls =
        vec!["https://fal.media/uploads/source-video.mp4".to_string()];

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "video": {
                "url": "https://fal.media/generated/upscaled-video.mp4",
                "content_type": "video/mp4"
            }
        }),
        "generated/generated-upscale-video/fal-output.mp4",
    )
    .expect("build fal video upscale output import");

    assert_eq!(
        output.source_url,
        "https://fal.media/generated/upscaled-video.mp4"
    );
    assert_eq!(output.output.media_id, "generated-shot-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-upscale-video/fal-output.mp4"
    );
    assert_eq!(output.output.width, 3840);
    assert_eq!(output.output.height, 2160);
    assert_eq!(output.output.duration_seconds, 12.5);
    assert_eq!(output.output.fps, 30.0);
}

#[test]
fn downloads_fal_generated_output_under_project_generated_directory() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    let mut output = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [
                {
                    "url": "https://fal.media/generated/hero.png",
                    "content_type": "image/png",
                    "width": 1024,
                    "height": 1024
                }
            ],
            "prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.png",
    )
    .expect("build output import");
    let Some((base_url, request_handle)) = spawn_fal_binary_server(b"fake-png-bytes") else {
        return;
    };
    output.source_url = format!("{base_url}/generated/hero.png");
    let project_dir = tempfile::tempdir().expect("project dir");

    let output_path =
        download_fal_generated_output(project_dir.path(), &output).expect("download output");
    let request = request_handle.join().expect("request capture");

    assert_eq!(
        output_path,
        project_dir
            .path()
            .join("generated/generated-shot-1/fal-output.png")
    );
    assert_eq!(
        std::fs::read(&output_path).expect("downloaded output"),
        b"fake-png-bytes"
    );
    assert!(request.starts_with("GET /generated/hero.png HTTP/1.1"));
}

#[test]
fn downloads_fal_generation_result_and_builds_completion_actions() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    let Some((base_url, request_handle)) = spawn_fal_binary_server(b"result-image-bytes") else {
        return;
    };
    let project_dir = tempfile::tempdir().expect("project dir");

    let completion = download_fal_generation_result_and_build_completion_actions(
        project_dir.path(),
        &asset,
        &json!({
            "images": [
                {
                    "url": format!("{base_url}/generated/hero.png"),
                    "content_type": "image/png",
                    "width": 1024,
                    "height": 1024
                }
            ],
            "prompt": "floating product shot with crisp rim light"
        }),
        "2026-06-23T12:05:00Z",
        Some("fal-run-123"),
        None,
    )
    .expect("download result and actions");
    let request = request_handle.join().expect("request capture");

    assert_eq!(
        completion.output_path,
        project_dir
            .path()
            .join("generated/generated-shot-1/fal-output.png")
    );
    assert_eq!(
        std::fs::read(&completion.output_path).expect("downloaded output"),
        b"result-image-bytes"
    );
    assert!(request.starts_with("GET /generated/hero.png HTTP/1.1"));
    assert_eq!(completion.actions.len(), 2);
    assert!(matches!(
        &completion.actions[0],
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            asset_id,
            outputs,
            ..
        } if asset_id == "generated-shot-1"
            && outputs[0].relative_path == "generated/generated-shot-1/fal-output.png"
    ));
    assert!(matches!(
        &completion.actions[1],
        video_creater_lib::project::action::ProjectAction::UpdateJobStatus {
            job_id,
            status,
            run_id,
            ..
        } if job_id == "generated-shot-1"
            && *status == JobStatus::Completed
            && run_id.as_deref() == Some("fal-run-123")
    ));
}

#[test]
fn runs_fal_generation_submission_until_result_is_downloaded_and_actions_are_built() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    let mut submission = build_fal_queue_submission(&asset).expect("build fal queue submission");
    let Some((base_url, request_handle)) = spawn_fal_generation_run_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/flux/schnell");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("test client");
    let project_dir = tempfile::tempdir().expect("project dir");

    let run = run_fal_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-06-23T12:05:00Z",
            Some("fal-run-123"),
            None,
        ),
        &submission,
        "unit-test-token",
        FalGenerationRunOptions {
            max_status_polls: 3,
            poll_interval: Duration::from_millis(0),
        },
    )
    .expect("run fal generation");
    let requests = request_handle.join().expect("request capture");

    assert_eq!(run.request_id, "req-123");
    assert_eq!(run.status.status, FalQueueStatusKind::Completed);
    assert_eq!(run.result["seed"], json!(42));
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-shot-1/fal-output.png")
    );
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("downloaded output"),
        b"result-image-bytes"
    );
    assert_eq!(run.completion.actions.len(), 2);
    assert!(matches!(
        &run.completion.actions[0],
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            asset_id,
            outputs,
            ..
        } if asset_id == "generated-shot-1"
            && outputs[0].relative_path == "generated/generated-shot-1/fal-output.png"
    ));
    assert!(requests[0].starts_with("POST /fal-ai/flux/schnell HTTP/1.1"));
    assert!(
        requests[1].starts_with("GET /fal-ai/flux/schnell/requests/req-123/status?logs=1 HTTP/1.1")
    );
    assert!(
        requests[2].starts_with("GET /fal-ai/flux/schnell/requests/req-123/status?logs=1 HTTP/1.1")
    );
    assert!(requests[3].starts_with("GET /fal-ai/flux/schnell/requests/req-123/response HTTP/1.1"));
    assert!(requests[4].starts_with("GET /generated/hero.png HTTP/1.1"));
    assert!(requests[..4]
        .iter()
        .all(|request| request.contains("authorization: Key unit-test-token")));
    assert!(requests
        .iter()
        .all(|request| !request_body(request).contains("unit-test-token")));
}

#[test]
fn retries_transient_fal_status_reads_without_resubmitting_the_paid_job() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            aspect_ratio: Some("1:1".to_string()),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    let mut submission = build_fal_queue_submission(&asset).expect("build submission");
    let Some((base_url, request_handle)) = spawn_fal_transient_status_run_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/flux/schnell");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("test client");
    let project_dir = tempfile::tempdir().expect("project dir");

    let run = run_fal_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-12T03:58:00Z",
            Some("fal-transient-run"),
            None,
        ),
        &submission,
        "unit-test-token",
        FalGenerationRunOptions {
            max_status_polls: 3,
            poll_interval: Duration::from_millis(0),
        },
    )
    .expect("retry transient status and finish");
    let requests = request_handle.join().expect("request capture");

    assert_eq!(run.request_id, "req-transient");
    assert_eq!(run.status.status, FalQueueStatusKind::Completed);
    assert_eq!(
        std::fs::read(run.completion.output_path).unwrap(),
        b"transient-result"
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("POST /fal-ai/flux/schnell "))
            .count(),
        1,
        "a transient status read must never resubmit a paid generation request"
    );
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("GET /fal-ai/wan/requests/req-transient/status"))
            .count(),
        2
    );
}

#[test]
fn fal_generation_cancellation_interrupts_poll_wait() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    let mut submission = build_fal_queue_submission(&asset).expect("build submission");
    let Some((base_url, request_handle)) = spawn_fal_poll_limit_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/flux/schnell");
    let project_dir = tempfile::tempdir().expect("project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let guard = register_generation_cancellation("project-fal-cancel", &asset.id)
        .expect("register cancellation");
    let token = guard.token();
    let job_id = asset.id.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        request_generation_cancellation("project-fal-cancel", &job_id)
    });
    let started = std::time::Instant::now();

    let error = run_fal_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-12T00:00:00Z",
            None,
            None,
        ),
        &submission,
        "unit-test-token",
        FalGenerationRunOptions {
            max_status_polls: 3,
            poll_interval: Duration::from_secs(5),
        },
        Some(&token),
    )
    .expect_err("cancel polling");

    assert_eq!(error, FalGenerationWorkerError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    cancel_thread.join().expect("cancel thread");
    assert_eq!(request_handle.join().expect("request capture").len(), 2);
}

#[test]
fn fal_generation_cancellation_during_download_removes_partial_file() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            ..GeneratedAssetSettings::default()
        },
    );
    let Some((base_url, server)) = spawn_fal_streaming_download_server() else {
        return;
    };
    let output_import = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [{
                "url": format!("{base_url}/output.png"),
                "width": 1024,
                "height": 1024,
                "content_type": "image/png"
            }]
        }),
        "generated/generated-shot-1/fal-output.png",
    )
    .expect("output import");
    let project_dir = tempfile::tempdir().expect("project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let guard = register_generation_cancellation("project-fal-download", &asset.id)
        .expect("register cancellation");
    let token = guard.token();
    let job_id = asset.id.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        request_generation_cancellation("project-fal-download", &job_id)
    });

    let error = download_fal_generated_output_with_client_cancellable(
        &client,
        project_dir.path(),
        &output_import,
        &token,
    )
    .expect_err("cancel download");
    let output_path = project_dir
        .path()
        .join("generated/generated-shot-1/fal-output.png");
    let part_path = output_path.with_extension("png.part");

    assert_eq!(error, FalGenerationWorkerError::Cancelled);
    assert!(!output_path.exists());
    assert!(!part_path.exists());
    cancel_thread.join().expect("cancel thread");
    server.join().expect("streaming server");
}

#[test]
fn fal_generation_submission_reports_poll_limit_without_secret() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    let mut submission = build_fal_queue_submission(&asset).expect("build fal queue submission");
    let Some((base_url, request_handle)) = spawn_fal_poll_limit_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/flux/schnell");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("test client");
    let project_dir = tempfile::tempdir().expect("project dir");

    let error = run_fal_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-06-23T12:05:00Z",
            None,
            None,
        ),
        &submission,
        "unit-test-token",
        FalGenerationRunOptions {
            max_status_polls: 1,
            poll_interval: Duration::from_millis(0),
        },
    )
    .expect_err("poll limit exceeded");
    let requests = request_handle.join().expect("request capture");

    assert_eq!(
        error,
        FalGenerationWorkerError::StatusPollLimitExceeded {
            request_id: "req-123".to_string(),
            max_status_polls: 1,
        }
    );
    assert_eq!(requests.len(), 2);
    assert!(!error.to_string().contains("unit-test-token"));
}

#[test]
fn fal_generation_submission_provider_error_message_omits_secret() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    let mut submission = build_fal_queue_submission(&asset).expect("build fal queue submission");
    let Some((base_url, request_handle)) = spawn_fal_provider_error_server() else {
        return;
    };
    submission.url = format!("{base_url}/fal-ai/flux/schnell");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("test client");
    let project_dir = tempfile::tempdir().expect("project dir");

    let error = run_fal_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-06-23T12:05:00Z",
            None,
            None,
        ),
        &submission,
        "unit-test-token",
        FalGenerationRunOptions {
            max_status_polls: 1,
            poll_interval: Duration::from_millis(0),
        },
    )
    .expect_err("provider error");
    let requests = request_handle.join().expect("request capture");

    assert!(matches!(
        error,
        FalGenerationWorkerError::CompletedWithProviderError {
            ref request_id,
            error_type: Some(_),
            error: Some(_),
        } if request_id == "req-123"
    ));
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|request| !request_body(request).contains("unit-test-token")));
    assert!(!error.to_string().contains("unit-test-token"));
}

#[test]
fn rejects_fal_generated_output_download_outside_project_generated_directory() {
    let asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    let mut output = build_fal_generated_output_import(
        &asset,
        &json!({
            "images": [
                {
                    "url": "https://fal.media/generated/hero.png",
                    "content_type": "image/png",
                    "width": 1024,
                    "height": 1024
                }
            ],
            "prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.png",
    )
    .expect("build output import");
    output.output.relative_path = "generated/generated-shot-1/../other/output.png".to_string();
    let project_dir = tempfile::tempdir().expect("project dir");

    let error =
        download_fal_generated_output(project_dir.path(), &output).expect_err("unsafe output path");

    assert_eq!(
        error,
        FalQueueSubmitError::UnsafeOutputPath {
            relative_path: "generated/generated-shot-1/../other/output.png".to_string()
        }
    );
    assert!(!project_dir
        .path()
        .join("generated/other/output.png")
        .exists());
}

#[test]
fn wan_text_to_video_result_builds_generated_output_import_metadata() {
    let asset = generated_asset(
        "fal-ai/wan-25-preview/text-to-video",
        GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(8.0),
            fps: Some(24.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "video": {
                "url": "https://fal.media/generated/hero.mp4",
                "content_type": "video/mp4",
                "width": 1080,
                "height": 1920,
                "fps": 24.0,
                "duration": 10.0
            },
            "seed": 175932751,
            "actual_prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.mp4",
    )
    .expect("build output import");

    assert_eq!(output.source_url, "https://fal.media/generated/hero.mp4");
    assert_eq!(output.output.media_id, "generated-shot-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.mp4"
    );
    assert_eq!(output.output.width, 1080);
    assert_eq!(output.output.height, 1920);
    assert_eq!(output.output.duration_seconds, 10.0);
    assert_eq!(output.output.fps, 24.0);
}

#[test]
fn seed_audio_result_builds_generated_output_import_metadata() {
    let asset = generated_asset(
        FAL_SEED_AUDIO_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(12.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output = build_fal_generated_output_import(
        &asset,
        &json!({
            "audio": {
                "url": "https://fal.media/generated/soundtrack.mp3",
                "content_type": "audio/mpeg",
                "file_name": "soundtrack.mp3",
                "file_size": 520556,
                "duration": 12.5,
                "channels": 2,
                "sample_rate": 24000,
                "bitrate": 192000
            }
        }),
        "generated/generated-shot-1/fal-output.mp3",
    )
    .expect("build output import");

    assert_eq!(
        output.source_url,
        "https://fal.media/generated/soundtrack.mp3"
    );
    assert_eq!(output.output.media_id, "generated-shot-1-fal-output");
    assert_eq!(
        output.output.relative_path,
        "generated/generated-shot-1/fal-output.mp3"
    );
    assert_eq!(output.output.width, 1);
    assert_eq!(output.output.height, 1);
    assert_eq!(output.output.duration_seconds, 12.5);
    assert_eq!(output.output.fps, 1.0);
}

#[test]
fn fal_generation_completion_actions_complete_asset_and_job_from_result() {
    let mut project = sample_project();
    let mut asset = generated_asset(
        "fal-ai/wan-25-preview/text-to-video",
        GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(8.0),
            fps: Some(24.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["media-1".to_string()],
        first_frame_media_id: Some("media-1".to_string()),
        last_frame_media_id: None,
        provider_input_urls: Vec::new(),
        ..Default::default()
    };
    asset.status = GeneratedAssetStatus::Running;
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        &asset.id,
        JobStatus::Running,
        "2026-06-23T12:00:00Z",
    ));
    project.generated_assets.push(asset.clone());

    let actions = build_fal_generation_completion_actions(
        &asset,
        &json!({
            "video": {
                "url": "https://fal.media/generated/hero.mp4",
                "content_type": "video/mp4",
                "width": 1080,
                "height": 1920,
                "fps": 24.0,
                "duration": 10.0
            },
            "actual_prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.mp4",
        "2026-06-23T12:05:00Z",
        Some("fal-run-123"),
        None,
    )
    .expect("build fal completion actions");
    assert_eq!(actions.len(), 2);
    match &actions[0] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            completion: Some(completion),
            ..
        } => {
            assert_eq!(completion.generated_asset_id, "generated-shot-1");
            assert_eq!(
                completion.generated_output_media_id,
                "generated-shot-1-fal-output"
            );
            assert_eq!(completion.placement_intent, "library");
            assert_eq!(completion.references.media_ids, vec!["media-1".to_string()]);
            assert_eq!(
                completion.references.first_frame_media_id.as_deref(),
                Some("media-1")
            );
            assert_eq!(completion.references.last_frame_media_id, None);
        }
        action => panic!("expected completion metadata on fal action: {action:?}"),
    }

    for action in actions {
        apply_project_action(&mut project, action).expect("apply fal completion action");
    }

    let generated = project
        .generated_assets
        .iter()
        .find(|candidate| candidate.id == asset.id)
        .expect("generated asset");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);
    assert_eq!(generated.outputs.len(), 1);
    assert_eq!(
        generated.outputs[0].relative_path,
        "generated/generated-shot-1/fal-output.mp4"
    );
    assert!(project.media.iter().any(|media| {
        media.id == "generated-shot-1-fal-output" && media.kind == MediaKind::Generated
    }));

    let job = project
        .jobs
        .iter()
        .find(|candidate| candidate.id == asset.id)
        .expect("generation job");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.updated_at, "2026-06-23T12:05:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("fal-run-123")
    );
}

#[test]
fn fal_generation_completion_actions_preserve_multi_image_outputs() {
    let mut asset = generated_asset(
        "fal-ai/flux/schnell",
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            num_images: Some(2),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;

    let actions = build_fal_generation_completion_actions(
        &asset,
        &json!({
            "images": [
                {"url": "https://fal.media/generated/one.png", "width": 1024, "height": 1024},
                {"url": "https://fal.media/generated/two.png", "width": 1024, "height": 1024}
            ]
        }),
        "generated/generated-shot-1/fal-output.png",
        "2026-06-23T12:05:00Z",
        Some("fal-run-123"),
        Some("timeline-item-that-needs-explicit-output-selection"),
    )
    .expect("build fal multi-image completion actions");

    match &actions[0] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            outputs,
            completion: Some(completion),
            replacement,
            ..
        } => {
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].media_id, "generated-shot-1-fal-output");
            assert_eq!(
                outputs[0].relative_path,
                "generated/generated-shot-1/fal-output.png"
            );
            assert_eq!(
                outputs[0].source_url.as_deref(),
                Some("https://fal.media/generated/one.png")
            );
            assert_eq!(outputs[1].media_id, "generated-shot-1-fal-output-2");
            assert_eq!(
                outputs[1].relative_path,
                "generated/generated-shot-1/fal-output-2.png"
            );
            assert_eq!(
                outputs[1].source_url.as_deref(),
                Some("https://fal.media/generated/two.png")
            );
            assert_eq!(
                completion.generated_output_media_id,
                "generated-shot-1-fal-output"
            );
            assert_eq!(replacement, &None);
        }
        action => panic!("expected multi-output fal completion action: {action:?}"),
    }
}

#[test]
fn fal_generation_completion_actions_can_replace_timeline_item() {
    let mut project = sample_project();
    let mut asset = generated_asset(
        "fal-ai/wan-25-preview/text-to-video",
        GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(8.0),
            fps: Some(24.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        &asset.id,
        JobStatus::Running,
        "2026-06-23T12:00:00Z",
    ));
    project.generated_assets.push(asset.clone());

    let actions = build_fal_generation_completion_actions(
        &asset,
        &json!({
            "video": {
                "url": "https://fal.media/generated/hero.mp4",
                "content_type": "video/mp4",
                "width": 1080,
                "height": 1920,
                "fps": 24.0,
                "duration": 10.0
            },
            "actual_prompt": "floating product shot with crisp rim light"
        }),
        "generated/generated-shot-1/fal-output.mp4",
        "2026-06-23T12:05:00Z",
        Some("fal-run-123"),
        Some("item-1"),
    )
    .expect("build fal replacement actions");

    for action in actions {
        apply_project_action(&mut project, action).expect("apply fal replacement action");
    }

    let timeline_item = project.timeline.tracks[0]
        .items
        .iter()
        .find(|item| item.id == "item-1")
        .expect("timeline item");
    assert_eq!(
        timeline_item.source,
        video_creater_lib::project::model::TimelineSource::Media {
            media_id: "generated-shot-1-fal-output".to_string()
        }
    );
    assert_eq!(
        timeline_item.properties["generatedAssetId"],
        serde_json::json!("generated-shot-1")
    );
}

#[test]
fn mock_generation_completion_actions_complete_queued_video_asset() {
    let mut project = sample_project();
    let mut asset = generated_asset(
        "fal-ai/wan-25-preview/text-to-video",
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["media-1".to_string()],
        first_frame_media_id: Some("media-1".to_string()),
        last_frame_media_id: None,
        provider_input_urls: Vec::new(),
        ..Default::default()
    };
    project.jobs.push(temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        &asset.id,
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    ));
    project.generated_assets.push(asset.clone());

    let actions = mock_generation_completion_actions(&asset, "2026-06-23T12:05:00Z", None)
        .expect("build mock completion actions");
    assert_eq!(actions.len(), 3);
    match &actions[1] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            completion: Some(completion),
            ..
        } => {
            assert_eq!(completion.generated_asset_id, "generated-shot-1");
            assert_eq!(
                completion.generated_output_media_id,
                "generated-shot-1-mock-output"
            );
            assert_eq!(completion.placement_intent, "library");
            assert_eq!(completion.references.media_ids, vec!["media-1".to_string()]);
        }
        action => panic!("expected completion metadata on mock action: {action:?}"),
    }
    for action in actions {
        apply_project_action(&mut project, action).expect("apply mock completion action");
    }

    let generated = project
        .generated_assets
        .iter()
        .find(|candidate| candidate.id == asset.id)
        .expect("generated asset");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);
    assert_eq!(generated.outputs.len(), 1);
    assert_eq!(
        generated.outputs[0].relative_path,
        "generated/generated-shot-1/mock-output.mp4"
    );
    assert_eq!(generated.outputs[0].duration_seconds, 5.0);
    assert!(project.media.iter().any(|media| {
        media.id == "generated-shot-1-mock-output" && media.kind == MediaKind::Generated
    }));

    let job = project
        .jobs
        .iter()
        .find(|candidate| candidate.id == asset.id)
        .expect("generation job");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(job.updated_at, "2026-06-23T12:05:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("mock-generated-shot-1")
    );
}

#[test]
fn mock_generation_completion_actions_complete_source_media_edits_without_provider_urls() {
    let mut video_edit = generated_asset(
        FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(false),
            ..GeneratedAssetSettings::default()
        },
    );
    video_edit.references = GeneratedAssetReferences {
        media_ids: vec!["source-video".to_string()],
        source_video_media_ref: Some("source-video".to_string()),
        reference_video_media_refs: vec!["source-video".to_string()],
        provider_input_urls: Vec::new(),
        ..Default::default()
    };

    let video_actions =
        mock_generation_completion_actions(&video_edit, "2026-06-23T12:05:00Z", None)
            .expect("source-video edit mock completion should stay no-spend");
    match &video_actions[1] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            outputs,
            completion: Some(completion),
            ..
        } => {
            assert_eq!(
                outputs[0].relative_path,
                "generated/generated-shot-1/mock-output.mp4"
            );
            assert_eq!(
                completion.references.source_video_media_ref.as_deref(),
                Some("source-video")
            );
            assert_eq!(
                completion.references.provider_input_urls,
                Vec::<String>::new()
            );
        }
        action => panic!("expected source-video edit mock completion action: {action:?}"),
    }

    let mut image_edit = generated_asset(
        FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: Some("2K".to_string()),
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    image_edit.references = GeneratedAssetReferences {
        media_ids: vec!["style-ref".to_string(), "subject-ref".to_string()],
        reference_image_media_refs: vec!["style-ref".to_string(), "subject-ref".to_string()],
        provider_input_urls: Vec::new(),
        ..Default::default()
    };

    let image_actions =
        mock_generation_completion_actions(&image_edit, "2026-06-23T12:05:00Z", None)
            .expect("image edit mock completion should stay no-spend");
    match &image_actions[1] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            outputs,
            completion: Some(completion),
            ..
        } => {
            assert_eq!(
                outputs[0].relative_path,
                "generated/generated-shot-1/mock-output.png"
            );
            assert_eq!(
                completion.references.reference_image_media_refs,
                vec!["style-ref".to_string(), "subject-ref".to_string()]
            );
            assert_eq!(
                completion.references.provider_input_urls,
                Vec::<String>::new()
            );
        }
        action => panic!("expected image edit mock completion action: {action:?}"),
    }
}

#[test]
fn mock_generation_completion_actions_complete_sonilo_text_to_music_as_audio() {
    let asset = generated_asset(
        FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID,
        GeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(90.0),
            fps: None,
            aspect_ratio: None,
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let actions = mock_generation_completion_actions(&asset, "2026-06-23T12:05:00Z", None)
        .expect("build Sonilo text-to-music mock completion actions");

    match &actions[1] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            outputs,
            completion: Some(completion),
            ..
        } => {
            assert_eq!(completion.generated_asset_id, "generated-shot-1");
            assert_eq!(
                outputs[0].relative_path,
                "generated/generated-shot-1/mock-output.m4a"
            );
            assert_eq!(outputs[0].duration_seconds, 90.0);
            assert_eq!(outputs[0].fps, 0.0);
        }
        action => panic!("expected Sonilo text-to-music completion metadata: {action:?}"),
    }
}

#[test]
fn mock_generation_completion_actions_complete_queued_xai_image_asset() {
    let mut asset = generated_asset(
        XAI_GROK_IMAGE_QUALITY_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    asset.model.provider = XAI_PROVIDER.to_string();

    let actions = mock_generation_completion_actions(&asset, "2026-06-23T12:05:00Z", None)
        .expect("build xAI mock completion actions");

    match &actions[1] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            outputs,
            completion: Some(completion),
            ..
        } => {
            assert_eq!(completion.generated_asset_id, "generated-shot-1");
            assert_eq!(
                outputs[0].relative_path,
                "generated/generated-shot-1/mock-output.png"
            );
            assert_eq!(outputs[0].duration_seconds, 0.0);
            assert_eq!(outputs[0].fps, 0.0);
        }
        action => panic!("expected xAI image completion metadata on mock action: {action:?}"),
    }
}
