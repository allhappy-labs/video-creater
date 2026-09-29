use chrono::Utc;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::Duration;
use video_creater_lib::generation::elevenlabs::{
    ELEVENLABS_DEFAULT_VOICE, ELEVENLABS_MUSIC_MODEL_ID, ELEVENLABS_PROVIDER,
    ELEVENLABS_TTS_V3_MODEL_ID,
};
use video_creater_lib::generation::fal::{
    FalGenerationRunOptions, FAL_AURA_SR_MODEL_ID, FAL_FLUX_SCHNELL_MODEL_ID,
    FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
    FAL_KREA_2_TURBO_MODEL_ID, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
    FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
    FAL_SEED_AUDIO_MODEL_ID, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
    FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
    FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
};
use video_creater_lib::generation::google::{
    GOOGLE_GEMINI_TTS_DEFAULT_VOICE, GOOGLE_GEMINI_TTS_MODEL_ID, GOOGLE_LYRIA_3_PRO_MODEL_ID,
    GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID,
};
use video_creater_lib::generation::minimax::{MINIMAX_MUSIC_MODEL_ID, MINIMAX_PROVIDER};
use video_creater_lib::generation::openai::{
    OPENAI_GPT_4O_MINI_TTS_MODEL_ID, OPENAI_GPT_IMAGE_2_MODEL_ID, OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
    OPENAI_PROVIDER,
};
use video_creater_lib::generation::replicate::{
    upload_replicate_local_file_with_client, ReplicateGenerationRunOptions, REPLICATE_API_BASE_URL,
    REPLICATE_FLUX_11_PRO_MODEL_ID, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
    REPLICATE_FLUX_DEV_MODEL_ID, REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER,
    REPLICATE_SEEDANCE_20_FAST_MODEL_ID, REPLICATE_SEEDANCE_20_MODEL_ID,
    VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR,
};
use video_creater_lib::generation::xai::{
    XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
};
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, JobStatus, MediaAsset, MediaKind, TimelineItem, TimelineItemKind,
    TimelineSource, VideoProject,
};
use video_creater_lib::project::split::save_split_project;
use video_creater_lib::render_pipeline::process::SystemProcessRunner;
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::workflows::{
    temporal_generate_media_elevenlabs_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_google_gemini_tts_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_google_lyria_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_google_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_minimax_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_openai_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner,
    temporal_generate_media_provider_submission_from_project_dir,
    temporal_generate_media_replicate_run_and_attach_submission_from_project_dir_with_client,
    temporal_generate_media_start_request,
    temporal_generate_media_xai_run_and_attach_submission_from_project_dir_with_client,
    temporal_job_summary, TemporalGenerateMediaBrief, TemporalGenerateMediaProviderSubmission,
    TemporalWorkflowKind,
};

const PROVIDER_E2E_SOURCE_WIDTH: u32 = 64;
const PROVIDER_E2E_SOURCE_HEIGHT: u32 = 64;
const FAL_KEY_ENV_VAR: &str = "FAL_KEY";
const REPLICATE_API_TOKEN_ENV_VAR: &str = "REPLICATE_API_TOKEN";
const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
const XAI_API_KEY_ENV_VAR: &str = "XAI_API_KEY";
const GEMINI_API_KEY_ENV_VAR: &str = "GEMINI_API_KEY";
const ELEVENLABS_API_KEY_ENV_VAR: &str = "ELEVENLABS_API_KEY";
const MINIMAX_API_KEY_ENV_VAR: &str = "MINIMAX_API_KEY";
const ELEVENLABS_TEXT_TO_MUSIC_DEFAULT_LYRICS: &str =
    "[Verse]\nWarm synth pop in English\n[Chorus]\nBright chorus lift";
const GOOGLE_LYRIA_TEXT_TO_MUSIC_DEFAULT_LYRICS: &str =
    "[Verse]\nSoft synth pulse in English\n[Chorus]\nConfident vocal lift";
const MINIMAX_TEXT_TO_MUSIC_DEFAULT_LYRICS: &str =
    "[Verse]\nBright product launch\n[Chorus]\nLoopable brand hook";

#[derive(Debug)]
struct ProviderE2eConfig {
    provider: String,
    model: String,
    scenario: ProviderE2eScenario,
    prompt: String,
    generate_audio: bool,
    out_dir: PathBuf,
    max_status_polls: usize,
    poll_interval: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderE2eScenario {
    TextToImage,
    TextToImageReplace,
    MultiImage,
    KreaTextToImage,
    RecraftTextToImage,
    ImageEdit,
    LocalImageEditReplace,
    LocalImageEditRetry,
    LocalImageEditCancellation,
    LocalImageEditFailure,
    LocalImageUpscale,
    LocalImageUpscaleReplace,
    LocalImageUpscaleRetry,
    LocalImageUpscaleCancellation,
    LocalImageUpscaleFailure,
    LocalVideoUpscale,
    LocalVideoUpscaleReplace,
    LocalVideoUpscaleRetry,
    LocalVideoUpscaleCancellation,
    LocalVideoUpscaleFailure,
    LocalVideoEditReplace,
    LocalVideoMotionControlReplace,
    LocalVideoEditRetry,
    LocalVideoEditCancellation,
    LocalVideoEditFailure,
    WanImageToVideo,
    WanReferenceToVideo,
    KlingImageToVideo,
    ReplicateFluxDev,
    ReplicateFlux11Pro,
    ReplicateFlux11ProUltra,
    ReplicateVideo,
    ReplicateVideoFast,
    TextToAudio,
    TextToMusic,
    TextToMusicInsert,
    VideoToVideo,
    VideoToMusic,
    LocalVideoToMusicInsert,
    VideoToSfx,
    LocalVideoToSfxInsert,
    XaiVideo,
    GoogleVideo,
    TextToVideoReplace,
    TextToVideoInsert,
    ReplicateLocalFileUpload,
}

impl ProviderE2eScenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text-to-image" => Ok(Self::TextToImage),
            "text-to-image-replace" => Ok(Self::TextToImageReplace),
            "multi-image" => Ok(Self::MultiImage),
            "krea-text-to-image" => Ok(Self::KreaTextToImage),
            "recraft-text-to-image" => Ok(Self::RecraftTextToImage),
            "image-edit" => Ok(Self::ImageEdit),
            "local-image-edit-replace" => Ok(Self::LocalImageEditReplace),
            "local-image-edit-retry" => Ok(Self::LocalImageEditRetry),
            "local-image-edit-cancellation" => Ok(Self::LocalImageEditCancellation),
            "local-image-edit-failure" => Ok(Self::LocalImageEditFailure),
            "local-image-upscale" => Ok(Self::LocalImageUpscale),
            "local-image-upscale-replace" => Ok(Self::LocalImageUpscaleReplace),
            "local-image-upscale-retry" => Ok(Self::LocalImageUpscaleRetry),
            "local-image-upscale-cancellation" => Ok(Self::LocalImageUpscaleCancellation),
            "local-image-upscale-failure" => Ok(Self::LocalImageUpscaleFailure),
            "local-video-upscale" => Ok(Self::LocalVideoUpscale),
            "local-video-upscale-replace" => Ok(Self::LocalVideoUpscaleReplace),
            "local-video-upscale-retry" => Ok(Self::LocalVideoUpscaleRetry),
            "local-video-upscale-cancellation" => Ok(Self::LocalVideoUpscaleCancellation),
            "local-video-upscale-failure" => Ok(Self::LocalVideoUpscaleFailure),
            "local-video-edit-replace" => Ok(Self::LocalVideoEditReplace),
            "local-video-motion-control-replace" => Ok(Self::LocalVideoMotionControlReplace),
            "local-video-edit-retry" => Ok(Self::LocalVideoEditRetry),
            "local-video-edit-cancellation" => Ok(Self::LocalVideoEditCancellation),
            "local-video-edit-failure" => Ok(Self::LocalVideoEditFailure),
            "wan-image-to-video" => Ok(Self::WanImageToVideo),
            "wan-reference-to-video" => Ok(Self::WanReferenceToVideo),
            "kling-image-to-video" => Ok(Self::KlingImageToVideo),
            "replicate-flux-dev" => Ok(Self::ReplicateFluxDev),
            "replicate-flux-1.1-pro" => Ok(Self::ReplicateFlux11Pro),
            "replicate-flux-1.1-pro-ultra" => Ok(Self::ReplicateFlux11ProUltra),
            "replicate-video" => Ok(Self::ReplicateVideo),
            "replicate-video-fast" => Ok(Self::ReplicateVideoFast),
            "text-to-audio" => Ok(Self::TextToAudio),
            "text-to-music" => Ok(Self::TextToMusic),
            "text-to-music-insert" => Ok(Self::TextToMusicInsert),
            "video-to-video" => Ok(Self::VideoToVideo),
            "video-to-music" => Ok(Self::VideoToMusic),
            "local-video-to-music-insert" => Ok(Self::LocalVideoToMusicInsert),
            "video-to-sfx" => Ok(Self::VideoToSfx),
            "local-video-to-sfx-insert" => Ok(Self::LocalVideoToSfxInsert),
            "xai-video" => Ok(Self::XaiVideo),
            "google-video" => Ok(Self::GoogleVideo),
            "text-to-video-replace" => Ok(Self::TextToVideoReplace),
            "text-to-video-insert" => Ok(Self::TextToVideoInsert),
            "replicate-local-file-upload" => Ok(Self::ReplicateLocalFileUpload),
            _ => Err(format!("unsupported provider E2E scenario: {value}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::TextToImage => "text-to-image",
            Self::TextToImageReplace => "text-to-image-replace",
            Self::MultiImage => "multi-image",
            Self::KreaTextToImage => "krea-text-to-image",
            Self::RecraftTextToImage => "recraft-text-to-image",
            Self::ImageEdit => "image-edit",
            Self::LocalImageEditReplace => "local-image-edit-replace",
            Self::LocalImageEditRetry => "local-image-edit-retry",
            Self::LocalImageEditCancellation => "local-image-edit-cancellation",
            Self::LocalImageEditFailure => "local-image-edit-failure",
            Self::LocalImageUpscale => "local-image-upscale",
            Self::LocalImageUpscaleReplace => "local-image-upscale-replace",
            Self::LocalImageUpscaleRetry => "local-image-upscale-retry",
            Self::LocalImageUpscaleCancellation => "local-image-upscale-cancellation",
            Self::LocalImageUpscaleFailure => "local-image-upscale-failure",
            Self::LocalVideoUpscale => "local-video-upscale",
            Self::LocalVideoUpscaleReplace => "local-video-upscale-replace",
            Self::LocalVideoUpscaleRetry => "local-video-upscale-retry",
            Self::LocalVideoUpscaleCancellation => "local-video-upscale-cancellation",
            Self::LocalVideoUpscaleFailure => "local-video-upscale-failure",
            Self::LocalVideoEditReplace => "local-video-edit-replace",
            Self::LocalVideoMotionControlReplace => "local-video-motion-control-replace",
            Self::LocalVideoEditRetry => "local-video-edit-retry",
            Self::LocalVideoEditCancellation => "local-video-edit-cancellation",
            Self::LocalVideoEditFailure => "local-video-edit-failure",
            Self::WanImageToVideo => "wan-image-to-video",
            Self::WanReferenceToVideo => "wan-reference-to-video",
            Self::KlingImageToVideo => "kling-image-to-video",
            Self::ReplicateFluxDev => "replicate-flux-dev",
            Self::ReplicateFlux11Pro => "replicate-flux-1.1-pro",
            Self::ReplicateFlux11ProUltra => "replicate-flux-1.1-pro-ultra",
            Self::ReplicateVideo => "replicate-video",
            Self::ReplicateVideoFast => "replicate-video-fast",
            Self::TextToAudio => "text-to-audio",
            Self::TextToMusic => "text-to-music",
            Self::TextToMusicInsert => "text-to-music-insert",
            Self::VideoToVideo => "video-to-video",
            Self::VideoToMusic => "video-to-music",
            Self::LocalVideoToMusicInsert => "local-video-to-music-insert",
            Self::VideoToSfx => "video-to-sfx",
            Self::LocalVideoToSfxInsert => "local-video-to-sfx-insert",
            Self::XaiVideo => "xai-video",
            Self::GoogleVideo => "google-video",
            Self::TextToVideoReplace => "text-to-video-replace",
            Self::TextToVideoInsert => "text-to-video-insert",
            Self::ReplicateLocalFileUpload => "replicate-local-file-upload",
        }
    }
}

struct TimelineAudioInsertConfig {
    source_item_id: &'static str,
    inserted_item_id: &'static str,
    placement_intent: &'static str,
    inserted_label: &'static str,
    timeline_start_seconds: f64,
    video_source_start_seconds: f64,
    video_source_end_seconds: f64,
}

fn timeline_audio_insert_config(
    scenario: ProviderE2eScenario,
) -> Option<TimelineAudioInsertConfig> {
    match scenario {
        ProviderE2eScenario::LocalVideoToMusicInsert => Some(TimelineAudioInsertConfig {
            source_item_id: "item-video-to-music-source",
            inserted_item_id: "item-video-to-music-inserted-audio",
            placement_intent: "insert-audio:item-video-to-music-source",
            inserted_label: "Provider E2E generated music",
            timeline_start_seconds: 12.0,
            video_source_start_seconds: 1.25,
            video_source_end_seconds: 5.75,
        }),
        ProviderE2eScenario::LocalVideoToSfxInsert => Some(TimelineAudioInsertConfig {
            source_item_id: "item-video-to-sfx-source",
            inserted_item_id: "item-video-to-sfx-inserted-audio",
            placement_intent: "insert-audio:item-video-to-sfx-source",
            inserted_label: "Provider E2E generated SFX",
            timeline_start_seconds: 8.0,
            video_source_start_seconds: 0.5,
            video_source_end_seconds: 2.0,
        }),
        _ => None,
    }
}

fn main() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    match parse_args(std::env::args().skip(1)).and_then(run_provider_e2e) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn parse_args<I>(args: I) -> Result<ProviderE2eConfig, String>
where
    I: IntoIterator<Item = String>,
{
    let mut provider = REPLICATE_PROVIDER.to_string();
    let mut model = "black-forest-labs/flux-schnell".to_string();
    let mut scenario = ProviderE2eScenario::TextToImage;
    let mut prompt = None;
    let mut generate_audio = true;
    let mut out_dir = PathBuf::from("output/provider-e2e/replicate");
    let mut max_status_polls = 60usize;
    let mut poll_interval = Duration::from_secs(5);
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--provider" => provider = require_value(&arg, args.next())?,
            "--model" => model = require_value(&arg, args.next())?,
            "--scenario" => {
                scenario = ProviderE2eScenario::parse(&require_value(&arg, args.next())?)?
            }
            "--prompt" => prompt = Some(require_value(&arg, args.next())?),
            "--generate-audio" => {
                generate_audio = parse_bool(&arg, &require_value(&arg, args.next())?)?
            }
            "--out-dir" => out_dir = PathBuf::from(require_value(&arg, args.next())?),
            "--max-status-polls" => {
                max_status_polls = parse_positive_usize(&arg, &require_value(&arg, args.next())?)?
            }
            "--poll-interval-ms" => {
                let millis = parse_positive_u64(&arg, &require_value(&arg, args.next())?)?;
                poll_interval = Duration::from_millis(millis);
            }
            "--help" | "-h" => {
                println!(
                    "Usage: video-creater-provider-e2e --provider replicate --model black-forest-labs/flux-schnell"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if provider != REPLICATE_PROVIDER
        && provider != FAL_PROVIDER
        && provider != OPENAI_PROVIDER
        && provider != XAI_PROVIDER
        && provider != GOOGLE_PROVIDER
        && provider != ELEVENLABS_PROVIDER
        && provider != MINIMAX_PROVIDER
    {
        return Err(format!("unsupported provider: {provider}"));
    }
    if provider == MINIMAX_PROVIDER
        && !matches!(
            scenario,
            ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
        )
    {
        return Err("minimax provider E2E supports text-to-music scenarios".to_string());
    }
    if provider == OPENAI_PROVIDER
        && !matches!(
            scenario,
            ProviderE2eScenario::TextToImage
                | ProviderE2eScenario::TextToImageReplace
                | ProviderE2eScenario::MultiImage
                | ProviderE2eScenario::ImageEdit
                | ProviderE2eScenario::LocalImageEditReplace
                | ProviderE2eScenario::TextToAudio
        )
    {
        return Err(
            "openai provider E2E supports text-to-image, multi-image, image-edit, and text-to-audio scenarios"
                .to_string(),
        );
    }
    if matches!(
        scenario,
        ProviderE2eScenario::TextToImage | ProviderE2eScenario::TextToImageReplace
    ) && provider == OPENAI_PROVIDER
        && model != OPENAI_GPT_IMAGE_2_MODEL_ID
    {
        return Err(format!(
            "openai text-to-image provider E2E requires model {OPENAI_GPT_IMAGE_2_MODEL_ID}"
        ));
    }
    if matches!(
        scenario,
        ProviderE2eScenario::TextToImage | ProviderE2eScenario::TextToImageReplace
    ) && provider == XAI_PROVIDER
        && model != XAI_GROK_IMAGE_QUALITY_MODEL_ID
    {
        return Err(format!(
            "xai text-to-image provider E2E requires model {XAI_GROK_IMAGE_QUALITY_MODEL_ID}"
        ));
    }
    if scenario == ProviderE2eScenario::KreaTextToImage {
        if provider != FAL_PROVIDER {
            return Err("krea-text-to-image provider E2E requires fal.ai".to_string());
        }
        if model != FAL_KREA_2_TURBO_MODEL_ID {
            return Err(format!(
                "krea-text-to-image provider E2E requires model {FAL_KREA_2_TURBO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::RecraftTextToImage {
        if provider != FAL_PROVIDER {
            return Err("recraft-text-to-image provider E2E requires fal.ai".to_string());
        }
        if model != FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID {
            return Err(format!(
                "recraft-text-to-image provider E2E requires model {FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::MultiImage {
        match provider.as_str() {
            FAL_PROVIDER => {
                if model != FAL_FLUX_SCHNELL_MODEL_ID {
                    return Err(format!(
                        "multi-image fal.ai provider E2E requires model {FAL_FLUX_SCHNELL_MODEL_ID}"
                    ));
                }
            }
            OPENAI_PROVIDER => {
                if model != OPENAI_GPT_IMAGE_2_MODEL_ID {
                    return Err(format!(
                        "multi-image OpenAI provider E2E requires model {OPENAI_GPT_IMAGE_2_MODEL_ID}"
                    ));
                }
            }
            REPLICATE_PROVIDER => {
                if model != REPLICATE_FLUX_SCHNELL_MODEL_ID {
                    return Err(format!(
                        "multi-image replicate provider E2E requires model {REPLICATE_FLUX_SCHNELL_MODEL_ID}"
                    ));
                }
            }
            _ => {
                return Err(
                    "multi-image provider E2E requires fal.ai, OpenAI, or replicate".to_string(),
                );
            }
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::ReplicateFluxDev
            | ProviderE2eScenario::ReplicateFlux11Pro
            | ProviderE2eScenario::ReplicateFlux11ProUltra
    ) {
        if provider != REPLICATE_PROVIDER {
            return Err("Replicate Flux provider E2E requires replicate".to_string());
        }
        let expected_model = match scenario {
            ProviderE2eScenario::ReplicateFluxDev => REPLICATE_FLUX_DEV_MODEL_ID,
            ProviderE2eScenario::ReplicateFlux11Pro => REPLICATE_FLUX_11_PRO_MODEL_ID,
            ProviderE2eScenario::ReplicateFlux11ProUltra => REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
            _ => unreachable!("checked Replicate Flux scenario"),
        };
        if model != expected_model {
            return Err(format!(
                "{} provider E2E requires model {expected_model}",
                scenario.as_str()
            ));
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::LocalImageUpscale
            | ProviderE2eScenario::LocalImageUpscaleReplace
            | ProviderE2eScenario::LocalImageUpscaleRetry
    ) {
        if provider != FAL_PROVIDER {
            return Err("local image upscale provider E2E requires fal.ai".to_string());
        }
        if model != FAL_AURA_SR_MODEL_ID {
            return Err(format!(
                "local image upscale provider E2E requires model {FAL_AURA_SR_MODEL_ID}"
            ));
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::LocalVideoUpscale
            | ProviderE2eScenario::LocalVideoUpscaleReplace
            | ProviderE2eScenario::LocalVideoUpscaleRetry
    ) {
        if provider != FAL_PROVIDER {
            return Err("local video upscale provider E2E requires fal.ai".to_string());
        }
        if model != FAL_VIDEO_UPSCALER_MODEL_ID {
            return Err(format!(
                "local video upscale provider E2E requires model {FAL_VIDEO_UPSCALER_MODEL_ID}"
            ));
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::ImageEdit
            | ProviderE2eScenario::LocalImageEditReplace
            | ProviderE2eScenario::LocalImageEditRetry
    ) {
        match provider.as_str() {
            FAL_PROVIDER => {
                if model != FAL_NANO_BANANA_PRO_EDIT_MODEL_ID {
                    return Err(format!(
                        "fal.ai image-edit provider E2E requires model {FAL_NANO_BANANA_PRO_EDIT_MODEL_ID}"
                    ));
                }
            }
            OPENAI_PROVIDER => {
                if model != OPENAI_GPT_IMAGE_EDIT_MODEL_ID {
                    return Err(format!(
                        "openai image-edit provider E2E requires model {OPENAI_GPT_IMAGE_EDIT_MODEL_ID}"
                    ));
                }
            }
            XAI_PROVIDER => {
                if model != XAI_GROK_IMAGE_QUALITY_MODEL_ID {
                    return Err(format!(
                        "xai image-edit provider E2E requires model {XAI_GROK_IMAGE_QUALITY_MODEL_ID}"
                    ));
                }
            }
            _ => {
                return Err("image-edit provider E2E requires fal.ai, openai, or xai".to_string());
            }
        }
    }
    if scenario == ProviderE2eScenario::WanImageToVideo {
        if provider != FAL_PROVIDER {
            return Err("wan-image-to-video provider E2E requires fal.ai".to_string());
        }
        if model != FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID {
            return Err(format!(
                "wan-image-to-video provider E2E requires model {FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::WanReferenceToVideo {
        if provider != FAL_PROVIDER {
            return Err("wan-reference-to-video provider E2E requires fal.ai".to_string());
        }
        if model != FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID {
            return Err(format!(
                "wan-reference-to-video provider E2E requires model {FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::KlingImageToVideo {
        if provider != FAL_PROVIDER {
            return Err("kling-image-to-video provider E2E requires fal.ai".to_string());
        }
        if model != FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID {
            return Err(format!(
                "kling-image-to-video provider E2E requires model {FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::LocalVideoMotionControlReplace {
        if provider != FAL_PROVIDER {
            return Err(
                "local-video-motion-control-replace provider E2E requires fal.ai".to_string(),
            );
        }
        if model != FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID {
            return Err(format!(
                "local-video-motion-control-replace provider E2E requires model {FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::ReplicateVideo {
        if provider != REPLICATE_PROVIDER {
            return Err("replicate-video provider E2E requires replicate".to_string());
        }
        if model != REPLICATE_SEEDANCE_20_MODEL_ID {
            return Err(format!(
                "replicate-video provider E2E requires model {REPLICATE_SEEDANCE_20_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::ReplicateVideoFast {
        if provider != REPLICATE_PROVIDER {
            return Err("replicate-video-fast provider E2E requires replicate".to_string());
        }
        if model != REPLICATE_SEEDANCE_20_FAST_MODEL_ID {
            return Err(format!(
                "replicate-video-fast provider E2E requires model {REPLICATE_SEEDANCE_20_FAST_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::TextToAudio {
        match provider.as_str() {
            FAL_PROVIDER => {
                if model != FAL_SEED_AUDIO_MODEL_ID {
                    return Err(format!(
                        "fal.ai text-to-audio provider E2E requires model {FAL_SEED_AUDIO_MODEL_ID}"
                    ));
                }
            }
            OPENAI_PROVIDER => {
                if model != OPENAI_GPT_4O_MINI_TTS_MODEL_ID {
                    return Err(format!(
                        "openai text-to-audio provider E2E requires model {OPENAI_GPT_4O_MINI_TTS_MODEL_ID}"
                    ));
                }
            }
            ELEVENLABS_PROVIDER => {
                if model != ELEVENLABS_TTS_V3_MODEL_ID {
                    return Err(format!(
                        "elevenlabs text-to-audio provider E2E requires model {ELEVENLABS_TTS_V3_MODEL_ID}"
                    ));
                }
            }
            GOOGLE_PROVIDER => {
                if model != GOOGLE_GEMINI_TTS_MODEL_ID {
                    return Err(format!(
                        "google text-to-audio provider E2E requires model {GOOGLE_GEMINI_TTS_MODEL_ID}"
                    ));
                }
            }
            _ => {
                return Err(
                    "text-to-audio provider E2E requires fal.ai, openai, elevenlabs, or google"
                        .to_string(),
                );
            }
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
    ) {
        match provider.as_str() {
            FAL_PROVIDER => {
                if model != FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID {
                    return Err(format!(
                        "fal.ai text-to-music provider E2E requires model {FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID}"
                    ));
                }
            }
            ELEVENLABS_PROVIDER => {
                if model != ELEVENLABS_MUSIC_MODEL_ID {
                    return Err(format!(
                        "elevenlabs text-to-music provider E2E requires model {ELEVENLABS_MUSIC_MODEL_ID}"
                    ));
                }
            }
            GOOGLE_PROVIDER => {
                if model != GOOGLE_LYRIA_3_PRO_MODEL_ID {
                    return Err(format!(
                        "google text-to-music provider E2E requires model {GOOGLE_LYRIA_3_PRO_MODEL_ID}"
                    ));
                }
            }
            MINIMAX_PROVIDER => {
                if model != MINIMAX_MUSIC_MODEL_ID {
                    return Err(format!(
                        "minimax text-to-music provider E2E requires model {MINIMAX_MUSIC_MODEL_ID}"
                    ));
                }
            }
            _ => {
                return Err(
                    "text-to-music provider E2E requires fal.ai, elevenlabs, google, or minimax"
                        .to_string(),
                );
            }
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::VideoToVideo | ProviderE2eScenario::LocalVideoEditRetry
    ) {
        if provider != FAL_PROVIDER {
            return Err("video-to-video provider E2E requires fal.ai".to_string());
        }
        if model != FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID {
            return Err(format!(
                "video-to-video provider E2E requires model {FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::LocalVideoEditReplace {
        match provider.as_str() {
            FAL_PROVIDER => {
                if model != FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID {
                    return Err(format!(
                        "local-video-edit-replace fal.ai provider E2E requires model {FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID}"
                    ));
                }
            }
            XAI_PROVIDER => {
                if model != XAI_GROK_VIDEO_MODEL_ID {
                    return Err(format!(
                        "local-video-edit-replace xai provider E2E requires model {XAI_GROK_VIDEO_MODEL_ID}"
                    ));
                }
            }
            _ => {
                return Err(
                    "local-video-edit-replace provider E2E requires fal.ai or xai".to_string(),
                );
            }
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::VideoToMusic | ProviderE2eScenario::LocalVideoToMusicInsert
    ) {
        if provider != FAL_PROVIDER {
            return Err("video-to-music provider E2E requires fal.ai".to_string());
        }
        if model != FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID {
            return Err(format!(
                "video-to-music provider E2E requires model {FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID}"
            ));
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::VideoToSfx | ProviderE2eScenario::LocalVideoToSfxInsert
    ) {
        if provider != FAL_PROVIDER {
            return Err("video-to-sfx provider E2E requires fal.ai".to_string());
        }
        if model != FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID {
            return Err(format!(
                "video-to-sfx provider E2E requires model {FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::XaiVideo {
        if provider != XAI_PROVIDER {
            return Err("xai-video provider E2E requires xai".to_string());
        }
        if model != XAI_GROK_VIDEO_MODEL_ID {
            return Err(format!(
                "xai-video provider E2E requires model {XAI_GROK_VIDEO_MODEL_ID}"
            ));
        }
    }
    if scenario == ProviderE2eScenario::GoogleVideo {
        if provider != GOOGLE_PROVIDER {
            return Err("google-video provider E2E requires google".to_string());
        }
        if model != GOOGLE_VEO_31_FAST_MODEL_ID {
            return Err(format!(
                "google-video provider E2E requires model {GOOGLE_VEO_31_FAST_MODEL_ID}"
            ));
        }
    }
    if matches!(
        scenario,
        ProviderE2eScenario::TextToVideoReplace | ProviderE2eScenario::TextToVideoInsert
    ) {
        let scenario_label = scenario.as_str();
        match provider.as_str() {
            FAL_PROVIDER => {
                if model != FAL_WAN_TEXT_TO_VIDEO_MODEL_ID {
                    return Err(format!(
                        "{scenario_label} fal.ai provider E2E requires model {FAL_WAN_TEXT_TO_VIDEO_MODEL_ID}"
                    ));
                }
            }
            GOOGLE_PROVIDER => {
                if model != GOOGLE_VEO_31_FAST_MODEL_ID {
                    return Err(format!(
                        "{scenario_label} google provider E2E requires model {GOOGLE_VEO_31_FAST_MODEL_ID}"
                    ));
                }
            }
            XAI_PROVIDER => {
                if model != XAI_GROK_VIDEO_MODEL_ID {
                    return Err(format!(
                        "{scenario_label} xai provider E2E requires model {XAI_GROK_VIDEO_MODEL_ID}"
                    ));
                }
            }
            REPLICATE_PROVIDER => {
                if model != REPLICATE_SEEDANCE_20_MODEL_ID {
                    return Err(format!(
                        "{scenario_label} replicate provider E2E requires model {REPLICATE_SEEDANCE_20_MODEL_ID}"
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "{scenario_label} provider E2E requires fal.ai, google, xai, or replicate"
                )
                .to_string());
            }
        }
    }
    if scenario == ProviderE2eScenario::ReplicateLocalFileUpload && provider != REPLICATE_PROVIDER {
        return Err("replicate-local-file-upload provider E2E requires replicate".to_string());
    }

    let prompt = prompt.unwrap_or_else(|| default_prompt_for_scenario(scenario));

    Ok(ProviderE2eConfig {
        provider,
        model,
        scenario,
        prompt,
        generate_audio,
        out_dir,
        max_status_polls,
        poll_interval,
    })
}

fn default_prompt_for_scenario(scenario: ProviderE2eScenario) -> String {
    if matches!(
        scenario,
        ProviderE2eScenario::VideoToMusic
            | ProviderE2eScenario::LocalVideoToMusicInsert
            | ProviderE2eScenario::VideoToSfx
            | ProviderE2eScenario::LocalVideoToSfxInsert
    ) {
        String::new()
    } else {
        "small product still on a clean studio background".to_string()
    }
}

fn require_value(flag: &str, value: Option<String>) -> Result<String, String> {
    let Some(value) = value else {
        return Err(format!("missing value for {flag}"));
    };
    if value.starts_with("--") || value.trim().is_empty() {
        return Err(format!("missing value for {flag}"));
    }
    Ok(value)
}

fn parse_positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{flag} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be a positive integer"));
    }
    Ok(parsed)
}

fn parse_positive_u64(flag: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{flag} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be a positive integer"));
    }
    Ok(parsed)
}

fn parse_bool(flag: &str, value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{flag} must be true or false")),
    }
}

fn run_provider_e2e(config: ProviderE2eConfig) -> Result<String, String> {
    let credential_env_var = match config.provider.as_str() {
        REPLICATE_PROVIDER => REPLICATE_API_TOKEN_ENV_VAR,
        OPENAI_PROVIDER => OPENAI_API_KEY_ENV_VAR,
        XAI_PROVIDER => XAI_API_KEY_ENV_VAR,
        GOOGLE_PROVIDER => GEMINI_API_KEY_ENV_VAR,
        ELEVENLABS_PROVIDER => ELEVENLABS_API_KEY_ENV_VAR,
        MINIMAX_PROVIDER => MINIMAX_API_KEY_ENV_VAR,
        _ => FAL_KEY_ENV_VAR,
    };
    let credential = std::env::var(credential_env_var)
        .map_err(|_| format!("{credential_env_var} is required"))?
        .trim()
        .to_string();
    if credential.is_empty() {
        return Err(format!("{credential_env_var} is required"));
    }

    std::fs::create_dir_all(&config.out_dir).map_err(|error| error.to_string())?;
    let project_dir = config.out_dir;
    let now = Utc::now().to_rfc3339();
    let project_id = "provider-e2e-project";
    let asset_id = "provider-e2e-generated";
    let source_media_id = "provider-e2e-source";
    let first_frame_media_id = "provider-e2e-first-frame";
    let last_frame_media_id = "provider-e2e-last-frame";
    let audio_media_id = "provider-e2e-audio";
    let source_video_media_id = "provider-e2e-video-source";
    let image_reference_media_id = "provider-e2e-image-ref";
    if config.scenario == ProviderE2eScenario::ReplicateLocalFileUpload {
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
        let source_path = media_dir.join("provider-e2e-source.png");
        write_provider_e2e_source_png(
            &source_path,
            PROVIDER_E2E_SOURCE_WIDTH,
            PROVIDER_E2E_SOURCE_HEIGHT,
            0,
        )?;
        let client = reqwest::blocking::Client::new();
        let api_base_url = std::env::var(VIDEO_CREATER_REPLICATE_API_BASE_URL_ENV_VAR)
            .unwrap_or_else(|_| REPLICATE_API_BASE_URL.to_string());
        let provider_file_url = upload_replicate_local_file_with_client(
            &client,
            &api_base_url,
            &source_path,
            &credential,
        )
        .map_err(|error| error.to_string())?;
        return Ok(json!({
            "ok": true,
            "scenario": config.scenario.as_str(),
            "provider": config.provider,
            "model": config.model,
            "projectDir": project_dir,
            "artifactPath": source_path,
            "outputCount": 0,
            "jobStatus": "Uploaded",
            "providerFileUrl": provider_file_url,
        })
        .to_string());
    }
    let mut project = VideoProject::new_empty(
        project_id.to_string(),
        "Provider E2E".to_string(),
        now.clone(),
    );
    if matches!(
        config.scenario,
        ProviderE2eScenario::LocalImageUpscale
            | ProviderE2eScenario::LocalImageUpscaleReplace
            | ProviderE2eScenario::LocalImageUpscaleRetry
    ) {
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
        let source_relative_path = "media/provider-e2e-source.png";
        write_provider_e2e_source_png(
            &project_dir.join(source_relative_path),
            PROVIDER_E2E_SOURCE_WIDTH,
            PROVIDER_E2E_SOURCE_HEIGHT,
            0,
        )?;
        project.media.push(MediaAsset {
            id: source_media_id.to_string(),
            name: Some("Provider E2E source image".to_string()),
            relative_path: source_relative_path.to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(PROVIDER_E2E_SOURCE_WIDTH),
            height: Some(PROVIDER_E2E_SOURCE_HEIGHT),
            fps: None,
            folder_id: None,
        });
        if config.scenario == ProviderE2eScenario::LocalImageUpscaleReplace {
            project.timeline.duration_seconds = 4.0;
            let link_group_id = "item-upscale-target-link";
            project.timeline.tracks[0].items.push(TimelineItem {
                id: "item-upscale-target".to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 0.0,
                duration_seconds: 4.0,
                source: TimelineSource::Media {
                    media_id: source_media_id.to_string(),
                },
                label: "Provider E2E source image".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(0.0)),
                    ("sourceOut".to_string(), json!(4.0)),
                    ("linkGroupId".to_string(), json!(link_group_id)),
                ]),
            });
            project.timeline.tracks[0].items.push(TimelineItem {
                id: "item-upscale-target-linked".to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 4.0,
                duration_seconds: 3.0,
                source: TimelineSource::Media {
                    media_id: source_media_id.to_string(),
                },
                label: "Provider E2E linked source image".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(1.0)),
                    ("sourceOut".to_string(), json!(4.0)),
                    ("linkGroupId".to_string(), json!(link_group_id)),
                ]),
            });
        }
    }
    if matches!(
        config.scenario,
        ProviderE2eScenario::ImageEdit
            | ProviderE2eScenario::LocalImageEditReplace
            | ProviderE2eScenario::LocalImageEditRetry
            | ProviderE2eScenario::WanReferenceToVideo
            | ProviderE2eScenario::LocalVideoMotionControlReplace
    ) {
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
        let reference_relative_path = "media/provider-e2e-image-ref.png";
        write_provider_e2e_source_png(&project_dir.join(reference_relative_path), 1024, 1024, 96)?;
        project.media.push(MediaAsset {
            id: image_reference_media_id.to_string(),
            name: Some("Provider E2E image edit reference".to_string()),
            relative_path: reference_relative_path.to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(1024),
            fps: None,
            folder_id: None,
        });
        if config.scenario == ProviderE2eScenario::LocalImageEditReplace {
            project.timeline.duration_seconds = 4.0;
            let link_group_id = "item-image-edit-target-link";
            project.timeline.tracks[0].items.push(TimelineItem {
                id: "item-image-edit-target".to_string(),
                kind: TimelineItemKind::ImageClip,
                start_seconds: 0.0,
                duration_seconds: 4.0,
                source: TimelineSource::Media {
                    media_id: image_reference_media_id.to_string(),
                },
                label: "Provider E2E source image".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(0.0)),
                    ("sourceOut".to_string(), json!(4.0)),
                    ("linkGroupId".to_string(), json!(link_group_id)),
                ]),
            });
            project.timeline.tracks[0].items.push(TimelineItem {
                id: "item-image-edit-target-linked".to_string(),
                kind: TimelineItemKind::ImageClip,
                start_seconds: 4.0,
                duration_seconds: 3.0,
                source: TimelineSource::Media {
                    media_id: image_reference_media_id.to_string(),
                },
                label: "Provider E2E linked source image".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(1.0)),
                    ("sourceOut".to_string(), json!(4.0)),
                    ("linkGroupId".to_string(), json!(link_group_id)),
                ]),
            });
        }
    }
    if config.scenario == ProviderE2eScenario::TextToImageReplace {
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
        let source_relative_path = "media/provider-e2e-text-image-source.png";
        write_provider_e2e_source_png(&project_dir.join(source_relative_path), 1024, 768, 32)?;
        project.media.push(MediaAsset {
            id: source_media_id.to_string(),
            name: Some("Provider E2E direct image replacement source".to_string()),
            relative_path: source_relative_path.to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1024),
            height: Some(768),
            fps: None,
            folder_id: None,
        });
        project.timeline.duration_seconds = 4.0;
        let link_group_id = "item-text-image-target-link";
        project.timeline.tracks[0].items.push(TimelineItem {
            id: "item-text-image-target".to_string(),
            kind: TimelineItemKind::ImageClip,
            start_seconds: 0.0,
            duration_seconds: 5.0,
            source: TimelineSource::Media {
                media_id: source_media_id.to_string(),
            },
            label: "Provider E2E direct image replacement source".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), json!(0.0)),
                ("sourceOut".to_string(), json!(4.0)),
                ("linkGroupId".to_string(), json!(link_group_id)),
            ]),
        });
        project.timeline.tracks[0].items.push(TimelineItem {
            id: "item-text-image-target-linked".to_string(),
            kind: TimelineItemKind::ImageClip,
            start_seconds: 4.0,
            duration_seconds: 3.0,
            source: TimelineSource::Media {
                media_id: source_media_id.to_string(),
            },
            label: "Provider E2E linked direct image source".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), json!(1.0)),
                ("sourceOut".to_string(), json!(4.0)),
                ("linkGroupId".to_string(), json!(link_group_id)),
            ]),
        });
    }
    if matches!(
        config.scenario,
        ProviderE2eScenario::WanImageToVideo
            | ProviderE2eScenario::WanReferenceToVideo
            | ProviderE2eScenario::KlingImageToVideo
            | ProviderE2eScenario::ReplicateVideo
            | ProviderE2eScenario::ReplicateVideoFast
    ) {
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
        let first_frame_relative_path = "media/provider-e2e-first-frame.png";
        let last_frame_relative_path = "media/provider-e2e-last-frame.png";
        let audio_relative_path = "media/provider-e2e-audio.wav";
        write_provider_e2e_source_png(&project_dir.join(first_frame_relative_path), 1280, 720, 0)?;
        write_provider_e2e_source_png(&project_dir.join(last_frame_relative_path), 1280, 720, 64)?;
        write_provider_e2e_silent_wav(&project_dir.join(audio_relative_path))?;
        project.media.push(MediaAsset {
            id: first_frame_media_id.to_string(),
            name: Some("Provider E2E first frame".to_string()),
            relative_path: first_frame_relative_path.to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: last_frame_media_id.to_string(),
            name: Some("Provider E2E last frame".to_string()),
            relative_path: last_frame_relative_path.to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1280),
            height: Some(720),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: audio_media_id.to_string(),
            name: Some("Provider E2E audio reference".to_string()),
            relative_path: audio_relative_path.to_string(),
            kind: MediaKind::Audio,
            duration_seconds: 4.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        });
    }
    if matches!(
        config.scenario,
        ProviderE2eScenario::VideoToVideo
            | ProviderE2eScenario::WanReferenceToVideo
            | ProviderE2eScenario::TextToVideoReplace
            | ProviderE2eScenario::TextToVideoInsert
            | ProviderE2eScenario::LocalVideoEditReplace
            | ProviderE2eScenario::LocalVideoMotionControlReplace
            | ProviderE2eScenario::LocalVideoEditRetry
            | ProviderE2eScenario::LocalVideoUpscaleReplace
            | ProviderE2eScenario::LocalVideoUpscale
            | ProviderE2eScenario::LocalVideoUpscaleRetry
            | ProviderE2eScenario::VideoToMusic
            | ProviderE2eScenario::LocalVideoToMusicInsert
            | ProviderE2eScenario::VideoToSfx
            | ProviderE2eScenario::LocalVideoToSfxInsert
    ) {
        let media_dir = project_dir.join("media");
        std::fs::create_dir_all(&media_dir).map_err(|error| error.to_string())?;
        let source_video_relative_path = "media/provider-e2e-source.mp4";
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/media/edison-speech-1920s-30s.mp4");
        std::fs::copy(&fixture_path, project_dir.join(source_video_relative_path))
            .map_err(|error| format!("copy provider E2E source video fixture: {error}"))?;
        project.media.push(MediaAsset {
            id: source_video_media_id.to_string(),
            name: Some("Provider E2E source video".to_string()),
            relative_path: source_video_relative_path.to_string(),
            kind: MediaKind::Video,
            duration_seconds: 10.0,
            width: Some(640),
            height: Some(360),
            fps: Some(24.0),
            folder_id: None,
        });
        if matches!(
            config.scenario,
            ProviderE2eScenario::TextToVideoReplace
                | ProviderE2eScenario::LocalVideoEditReplace
                | ProviderE2eScenario::LocalVideoMotionControlReplace
                | ProviderE2eScenario::LocalVideoUpscaleReplace
        ) {
            let (item_id, duration_seconds, source_in, source_out) =
                if config.scenario == ProviderE2eScenario::TextToVideoReplace {
                    ("item-text-video-target", 5.0, 0.0, 5.0)
                } else if config.scenario == ProviderE2eScenario::LocalVideoEditReplace {
                    ("item-video-edit-target", 3.0, 0.5, 3.5)
                } else if config.scenario == ProviderE2eScenario::LocalVideoMotionControlReplace {
                    ("item-video-motion-control-target", 3.0, 0.5, 3.5)
                } else {
                    ("item-video-upscale-target", 4.0, 0.0, 4.0)
                };
            let linked_item_id = format!("{item_id}-linked");
            let link_group_id = format!("{item_id}-link");
            let linked_duration_seconds = (duration_seconds - 1.0_f64).max(0.001);
            project.timeline.duration_seconds = duration_seconds;
            project.timeline.tracks[0].items.push(TimelineItem {
                id: item_id.to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 0.0,
                duration_seconds,
                source: TimelineSource::Media {
                    media_id: source_video_media_id.to_string(),
                },
                label: "Provider E2E source video".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(source_in)),
                    ("sourceOut".to_string(), json!(source_out)),
                    ("linkGroupId".to_string(), json!(link_group_id)),
                ]),
            });
            project.timeline.tracks[0].items.push(TimelineItem {
                id: linked_item_id,
                kind: TimelineItemKind::VideoClip,
                start_seconds: duration_seconds,
                duration_seconds: linked_duration_seconds,
                source: TimelineSource::Media {
                    media_id: source_video_media_id.to_string(),
                },
                label: "Provider E2E linked source video".to_string(),
                properties: BTreeMap::from([
                    ("sourceIn".to_string(), json!(source_in)),
                    (
                        "sourceOut".to_string(),
                        json!(source_in + linked_duration_seconds),
                    ),
                    ("linkGroupId".to_string(), json!(link_group_id)),
                ]),
            });
        }
        if let Some(insert_config) = timeline_audio_insert_config(config.scenario) {
            project.timeline.duration_seconds = 24.0;
            project.timeline.tracks[0].items.push(TimelineItem {
                id: insert_config.source_item_id.to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: insert_config.timeline_start_seconds,
                duration_seconds: insert_config.video_source_end_seconds
                    - insert_config.video_source_start_seconds,
                source: TimelineSource::Media {
                    media_id: source_video_media_id.to_string(),
                },
                label: "Provider E2E source video".to_string(),
                properties: BTreeMap::from([
                    (
                        "sourceIn".to_string(),
                        json!(insert_config.video_source_start_seconds),
                    ),
                    (
                        "sourceOut".to_string(),
                        json!(insert_config.video_source_end_seconds),
                    ),
                ]),
            });
            if project.timeline.tracks.len() > 1 {
                project.timeline.tracks[1].items.push(TimelineItem {
                    id: insert_config.inserted_item_id.to_string(),
                    kind: TimelineItemKind::AudioClip,
                    start_seconds: insert_config.timeline_start_seconds,
                    duration_seconds: 10.0,
                    source: TimelineSource::Media {
                        media_id: format!("{asset_id}-fal-output"),
                    },
                    label: insert_config.inserted_label.to_string(),
                    properties: BTreeMap::from([
                        ("sourceIn".to_string(), json!(0.0)),
                        ("sourceOut".to_string(), json!(10.0)),
                        (
                            "generatedOutputMediaId".to_string(),
                            json!(format!("{asset_id}-fal-output")),
                        ),
                        (
                            "sourceVideoMediaRef".to_string(),
                            json!(source_video_media_id),
                        ),
                    ]),
                });
            }
        }
    }
    let (media_ids, first_frame_media_id_value, last_frame_media_id_value, audio_media_refs) =
        match config.scenario {
            ProviderE2eScenario::ImageEdit
            | ProviderE2eScenario::LocalImageEditReplace
            | ProviderE2eScenario::LocalImageEditRetry => (
                vec![image_reference_media_id.to_string()],
                None,
                None,
                Vec::new(),
            ),
            ProviderE2eScenario::LocalImageUpscale
            | ProviderE2eScenario::LocalImageUpscaleReplace
            | ProviderE2eScenario::LocalImageUpscaleRetry => {
                (vec![source_media_id.to_string()], None, None, Vec::new())
            }
            ProviderE2eScenario::KlingImageToVideo => (
                vec![
                    first_frame_media_id.to_string(),
                    last_frame_media_id.to_string(),
                ],
                Some(first_frame_media_id.to_string()),
                Some(last_frame_media_id.to_string()),
                Vec::new(),
            ),
            ProviderE2eScenario::WanImageToVideo
            | ProviderE2eScenario::ReplicateVideo
            | ProviderE2eScenario::ReplicateVideoFast => (
                vec![
                    first_frame_media_id.to_string(),
                    last_frame_media_id.to_string(),
                    audio_media_id.to_string(),
                ],
                Some(first_frame_media_id.to_string()),
                Some(last_frame_media_id.to_string()),
                vec![audio_media_id.to_string()],
            ),
            ProviderE2eScenario::WanReferenceToVideo => (
                vec![
                    image_reference_media_id.to_string(),
                    source_video_media_id.to_string(),
                ],
                None,
                None,
                Vec::new(),
            ),
            ProviderE2eScenario::VideoToMusic
            | ProviderE2eScenario::LocalVideoToMusicInsert
            | ProviderE2eScenario::VideoToSfx
            | ProviderE2eScenario::LocalVideoToSfxInsert => (
                vec![source_video_media_id.to_string()],
                None,
                None,
                Vec::new(),
            ),
            ProviderE2eScenario::LocalVideoMotionControlReplace => (
                vec![
                    source_video_media_id.to_string(),
                    image_reference_media_id.to_string(),
                ],
                None,
                None,
                Vec::new(),
            ),
            ProviderE2eScenario::VideoToVideo
            | ProviderE2eScenario::LocalVideoUpscale
            | ProviderE2eScenario::LocalVideoUpscaleReplace
            | ProviderE2eScenario::LocalVideoUpscaleRetry
            | ProviderE2eScenario::LocalVideoEditReplace
            | ProviderE2eScenario::LocalVideoEditRetry => (
                vec![source_video_media_id.to_string()],
                None,
                None,
                Vec::new(),
            ),
            _ => (Vec::new(), None, None, Vec::new()),
        };
    let openai_image_generation = config.provider == OPENAI_PROVIDER
        && matches!(
            config.scenario,
            ProviderE2eScenario::TextToImage
                | ProviderE2eScenario::TextToImageReplace
                | ProviderE2eScenario::ImageEdit
                | ProviderE2eScenario::LocalImageEditReplace
                | ProviderE2eScenario::LocalImageEditRetry
        );
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: asset_id.to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some(match config.scenario {
            ProviderE2eScenario::WanImageToVideo
            | ProviderE2eScenario::WanReferenceToVideo
            | ProviderE2eScenario::KlingImageToVideo
            | ProviderE2eScenario::LocalVideoUpscale
            | ProviderE2eScenario::LocalVideoUpscaleReplace
            | ProviderE2eScenario::LocalVideoUpscaleRetry
            | ProviderE2eScenario::LocalVideoEditReplace
            | ProviderE2eScenario::LocalVideoMotionControlReplace
            | ProviderE2eScenario::LocalVideoEditRetry
            | ProviderE2eScenario::VideoToVideo
            | ProviderE2eScenario::ReplicateVideo
            | ProviderE2eScenario::ReplicateVideoFast
            | ProviderE2eScenario::XaiVideo
            | ProviderE2eScenario::GoogleVideo
            | ProviderE2eScenario::TextToVideoReplace
            | ProviderE2eScenario::TextToVideoInsert => "Provider E2E video".to_string(),
            ProviderE2eScenario::TextToAudio
            | ProviderE2eScenario::TextToMusic
            | ProviderE2eScenario::TextToMusicInsert
            | ProviderE2eScenario::VideoToMusic
            | ProviderE2eScenario::LocalVideoToMusicInsert
            | ProviderE2eScenario::VideoToSfx
            | ProviderE2eScenario::LocalVideoToSfxInsert => "Provider E2E audio".to_string(),
            _ => "Provider E2E still".to_string(),
        }),
        target_folder_id: None,
        placement_intent: if config.scenario == ProviderE2eScenario::LocalImageUpscaleReplace {
            Some("replace:item-upscale-target".to_string())
        } else if config.scenario == ProviderE2eScenario::TextToImageReplace {
            Some("replace:item-text-image-target".to_string())
        } else if config.scenario == ProviderE2eScenario::LocalImageEditReplace {
            Some("replace:item-image-edit-target".to_string())
        } else if config.scenario == ProviderE2eScenario::TextToVideoReplace {
            Some("replace:item-text-video-target".to_string())
        } else if config.scenario == ProviderE2eScenario::TextToVideoInsert {
            Some("insert-video:track-video".to_string())
        } else if config.scenario == ProviderE2eScenario::TextToMusicInsert {
            Some("insert-audio:track-audio".to_string())
        } else if config.scenario == ProviderE2eScenario::LocalVideoUpscaleReplace {
            Some("replace:item-video-upscale-target".to_string())
        } else if config.scenario == ProviderE2eScenario::LocalVideoEditReplace {
            Some("replace:item-video-edit-target".to_string())
        } else if config.scenario == ProviderE2eScenario::LocalVideoMotionControlReplace {
            Some("replace:item-video-motion-control-target".to_string())
        } else {
            timeline_audio_insert_config(config.scenario)
                .map(|insert_config| insert_config.placement_intent.to_string())
        },
        prompt: config.prompt.clone(),
        model: GenerationModel {
            provider: config.provider.clone(),
            id: config.model.clone(),
        },
        references: GeneratedAssetReferences {
            media_ids,
            source_video_media_ref: if matches!(
                config.scenario,
                ProviderE2eScenario::VideoToVideo
                    | ProviderE2eScenario::LocalVideoEditReplace
                    | ProviderE2eScenario::LocalVideoMotionControlReplace
                    | ProviderE2eScenario::LocalVideoEditRetry
                    | ProviderE2eScenario::LocalVideoUpscaleReplace
                    | ProviderE2eScenario::LocalVideoUpscale
                    | ProviderE2eScenario::LocalVideoUpscaleRetry
                    | ProviderE2eScenario::VideoToMusic
                    | ProviderE2eScenario::LocalVideoToMusicInsert
                    | ProviderE2eScenario::VideoToSfx
                    | ProviderE2eScenario::LocalVideoToSfxInsert
            ) {
                Some(source_video_media_id.to_string())
            } else {
                None
            },
            first_frame_media_id: first_frame_media_id_value,
            last_frame_media_id: last_frame_media_id_value,
            reference_audio_media_refs: audio_media_refs,
            reference_image_media_refs: if matches!(
                config.scenario,
                ProviderE2eScenario::ImageEdit
                    | ProviderE2eScenario::LocalImageEditReplace
                    | ProviderE2eScenario::LocalImageEditRetry
                    | ProviderE2eScenario::WanReferenceToVideo
                    | ProviderE2eScenario::LocalVideoMotionControlReplace
            ) {
                vec![image_reference_media_id.to_string()]
            } else {
                Vec::new()
            },
            reference_video_media_refs: if config.scenario
                == ProviderE2eScenario::WanReferenceToVideo
            {
                vec![source_video_media_id.to_string()]
            } else {
                Vec::new()
            },
            provider_input_urls: Vec::new(),
        },
        settings: GeneratedAssetSettings {
            width: if matches!(
                config.scenario,
                ProviderE2eScenario::TextToAudio
                    | ProviderE2eScenario::TextToMusic
                    | ProviderE2eScenario::TextToMusicInsert
                    | ProviderE2eScenario::VideoToMusic
                    | ProviderE2eScenario::LocalVideoToMusicInsert
                    | ProviderE2eScenario::VideoToSfx
                    | ProviderE2eScenario::LocalVideoToSfxInsert
            ) {
                None
            } else {
                Some(
                    if matches!(
                        config.scenario,
                        ProviderE2eScenario::WanImageToVideo
                            | ProviderE2eScenario::WanReferenceToVideo
                            | ProviderE2eScenario::KlingImageToVideo
                            | ProviderE2eScenario::LocalVideoUpscale
                            | ProviderE2eScenario::LocalVideoUpscaleReplace
                            | ProviderE2eScenario::LocalVideoUpscaleRetry
                            | ProviderE2eScenario::LocalVideoEditReplace
                            | ProviderE2eScenario::LocalVideoMotionControlReplace
                            | ProviderE2eScenario::LocalVideoEditRetry
                            | ProviderE2eScenario::VideoToVideo
                            | ProviderE2eScenario::ReplicateVideo
                            | ProviderE2eScenario::ReplicateVideoFast
                            | ProviderE2eScenario::XaiVideo
                            | ProviderE2eScenario::GoogleVideo
                            | ProviderE2eScenario::TextToVideoReplace
                            | ProviderE2eScenario::TextToVideoInsert
                    ) {
                        1280
                    } else {
                        1024
                    },
                )
            },
            height: if matches!(
                config.scenario,
                ProviderE2eScenario::TextToAudio
                    | ProviderE2eScenario::TextToMusic
                    | ProviderE2eScenario::TextToMusicInsert
                    | ProviderE2eScenario::VideoToMusic
                    | ProviderE2eScenario::LocalVideoToMusicInsert
                    | ProviderE2eScenario::VideoToSfx
                    | ProviderE2eScenario::LocalVideoToSfxInsert
            ) {
                None
            } else {
                Some(
                    if matches!(
                        config.scenario,
                        ProviderE2eScenario::WanImageToVideo
                            | ProviderE2eScenario::WanReferenceToVideo
                            | ProviderE2eScenario::KlingImageToVideo
                            | ProviderE2eScenario::LocalVideoUpscale
                            | ProviderE2eScenario::LocalVideoUpscaleReplace
                            | ProviderE2eScenario::LocalVideoUpscaleRetry
                            | ProviderE2eScenario::LocalVideoEditReplace
                            | ProviderE2eScenario::LocalVideoMotionControlReplace
                            | ProviderE2eScenario::LocalVideoEditRetry
                            | ProviderE2eScenario::VideoToVideo
                            | ProviderE2eScenario::ReplicateVideo
                            | ProviderE2eScenario::ReplicateVideoFast
                            | ProviderE2eScenario::XaiVideo
                            | ProviderE2eScenario::GoogleVideo
                            | ProviderE2eScenario::TextToVideoReplace
                            | ProviderE2eScenario::TextToVideoInsert
                    ) {
                        720
                    } else if openai_image_generation {
                        1024
                    } else {
                        768
                    },
                )
            },
            duration_seconds: if matches!(
                config.scenario,
                ProviderE2eScenario::WanImageToVideo
                    | ProviderE2eScenario::WanReferenceToVideo
                    | ProviderE2eScenario::KlingImageToVideo
                    | ProviderE2eScenario::LocalVideoUpscale
                    | ProviderE2eScenario::LocalVideoUpscaleReplace
                    | ProviderE2eScenario::LocalVideoUpscaleRetry
                    | ProviderE2eScenario::LocalVideoEditReplace
                    | ProviderE2eScenario::LocalVideoMotionControlReplace
                    | ProviderE2eScenario::LocalVideoEditRetry
                    | ProviderE2eScenario::VideoToVideo
                    | ProviderE2eScenario::ReplicateVideo
                    | ProviderE2eScenario::ReplicateVideoFast
                    | ProviderE2eScenario::XaiVideo
                    | ProviderE2eScenario::GoogleVideo
                    | ProviderE2eScenario::TextToVideoReplace
                    | ProviderE2eScenario::TextToVideoInsert
            ) {
                Some(
                    if matches!(
                        config.scenario,
                        ProviderE2eScenario::WanImageToVideo
                            | ProviderE2eScenario::KlingImageToVideo
                            | ProviderE2eScenario::TextToVideoReplace
                            | ProviderE2eScenario::TextToVideoInsert
                    ) {
                        5.0
                    } else if config.scenario == ProviderE2eScenario::GoogleVideo {
                        8.0
                    } else if matches!(
                        config.scenario,
                        ProviderE2eScenario::LocalVideoEditReplace
                            | ProviderE2eScenario::LocalVideoMotionControlReplace
                            | ProviderE2eScenario::LocalVideoEditRetry
                    ) {
                        3.0
                    } else {
                        4.0
                    },
                )
            } else if matches!(
                config.scenario,
                ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
            ) {
                Some(30.0)
            } else if config.scenario == ProviderE2eScenario::TextToAudio {
                Some(12.0)
            } else if matches!(
                config.scenario,
                ProviderE2eScenario::VideoToMusic
                    | ProviderE2eScenario::LocalVideoToMusicInsert
                    | ProviderE2eScenario::VideoToSfx
                    | ProviderE2eScenario::LocalVideoToSfxInsert
            ) {
                Some(10.0)
            } else {
                None
            },
            fps: if matches!(
                config.scenario,
                ProviderE2eScenario::WanImageToVideo
                    | ProviderE2eScenario::WanReferenceToVideo
                    | ProviderE2eScenario::KlingImageToVideo
                    | ProviderE2eScenario::LocalVideoUpscale
                    | ProviderE2eScenario::LocalVideoUpscaleReplace
                    | ProviderE2eScenario::LocalVideoUpscaleRetry
                    | ProviderE2eScenario::LocalVideoEditReplace
                    | ProviderE2eScenario::LocalVideoMotionControlReplace
                    | ProviderE2eScenario::LocalVideoEditRetry
                    | ProviderE2eScenario::VideoToVideo
                    | ProviderE2eScenario::ReplicateVideo
                    | ProviderE2eScenario::ReplicateVideoFast
                    | ProviderE2eScenario::XaiVideo
                    | ProviderE2eScenario::GoogleVideo
                    | ProviderE2eScenario::TextToVideoReplace
                    | ProviderE2eScenario::TextToVideoInsert
            ) {
                Some(24.0)
            } else {
                None
            },
            aspect_ratio: Some(
                if matches!(
                    config.scenario,
                    ProviderE2eScenario::ImageEdit
                        | ProviderE2eScenario::LocalImageEditReplace
                        | ProviderE2eScenario::LocalImageEditRetry
                ) || openai_image_generation
                {
                    "1:1".to_string()
                } else if matches!(
                    config.scenario,
                    ProviderE2eScenario::WanImageToVideo
                        | ProviderE2eScenario::WanReferenceToVideo
                        | ProviderE2eScenario::KlingImageToVideo
                        | ProviderE2eScenario::LocalVideoUpscale
                        | ProviderE2eScenario::LocalVideoUpscaleReplace
                        | ProviderE2eScenario::LocalVideoUpscaleRetry
                        | ProviderE2eScenario::LocalVideoEditReplace
                        | ProviderE2eScenario::LocalVideoMotionControlReplace
                        | ProviderE2eScenario::LocalVideoEditRetry
                        | ProviderE2eScenario::VideoToVideo
                        | ProviderE2eScenario::ReplicateVideo
                        | ProviderE2eScenario::ReplicateVideoFast
                        | ProviderE2eScenario::XaiVideo
                        | ProviderE2eScenario::GoogleVideo
                        | ProviderE2eScenario::TextToVideoReplace
                        | ProviderE2eScenario::TextToVideoInsert
                ) {
                    "16:9".to_string()
                } else {
                    "4:3".to_string()
                },
            ),
            resolution: if matches!(
                config.scenario,
                ProviderE2eScenario::WanImageToVideo
                    | ProviderE2eScenario::WanReferenceToVideo
                    | ProviderE2eScenario::KlingImageToVideo
                    | ProviderE2eScenario::LocalVideoUpscale
                    | ProviderE2eScenario::LocalVideoUpscaleReplace
                    | ProviderE2eScenario::LocalVideoUpscaleRetry
                    | ProviderE2eScenario::LocalVideoEditReplace
                    | ProviderE2eScenario::LocalVideoMotionControlReplace
                    | ProviderE2eScenario::LocalVideoEditRetry
                    | ProviderE2eScenario::VideoToVideo
                    | ProviderE2eScenario::ReplicateVideo
                    | ProviderE2eScenario::ReplicateVideoFast
                    | ProviderE2eScenario::XaiVideo
                    | ProviderE2eScenario::GoogleVideo
                    | ProviderE2eScenario::TextToVideoReplace
                    | ProviderE2eScenario::TextToVideoInsert
            ) {
                Some("720p".to_string())
            } else if openai_image_generation {
                Some("1024x1024".to_string())
            } else if matches!(
                config.scenario,
                ProviderE2eScenario::ImageEdit
                    | ProviderE2eScenario::LocalImageEditReplace
                    | ProviderE2eScenario::LocalImageEditRetry
            ) {
                Some("1K".to_string())
            } else {
                None
            },
            generate_audio: if matches!(
                config.scenario,
                ProviderE2eScenario::WanImageToVideo
                    | ProviderE2eScenario::WanReferenceToVideo
                    | ProviderE2eScenario::KlingImageToVideo
                    | ProviderE2eScenario::LocalVideoUpscale
                    | ProviderE2eScenario::LocalVideoUpscaleReplace
                    | ProviderE2eScenario::LocalVideoUpscaleRetry
                    | ProviderE2eScenario::LocalVideoEditReplace
                    | ProviderE2eScenario::LocalVideoMotionControlReplace
                    | ProviderE2eScenario::LocalVideoEditRetry
                    | ProviderE2eScenario::VideoToVideo
                    | ProviderE2eScenario::ReplicateVideo
                    | ProviderE2eScenario::ReplicateVideoFast
                    | ProviderE2eScenario::XaiVideo
                    | ProviderE2eScenario::GoogleVideo
                    | ProviderE2eScenario::TextToVideoReplace
                    | ProviderE2eScenario::TextToVideoInsert
            ) {
                Some(config.generate_audio)
            } else {
                None
            },
            category: if config.scenario == ProviderE2eScenario::TextToAudio {
                Some("tts".to_string())
            } else if matches!(
                config.scenario,
                ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
            ) {
                Some("music".to_string())
            } else {
                None
            },
            instrumental: if matches!(
                config.scenario,
                ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
            ) {
                Some(false)
            } else {
                None
            },
            voice: if config.provider == OPENAI_PROVIDER
                && config.scenario == ProviderE2eScenario::TextToAudio
            {
                Some("marin".to_string())
            } else if config.provider == ELEVENLABS_PROVIDER
                && config.scenario == ProviderE2eScenario::TextToAudio
            {
                Some(ELEVENLABS_DEFAULT_VOICE.to_string())
            } else if config.provider == GOOGLE_PROVIDER
                && config.scenario == ProviderE2eScenario::TextToAudio
            {
                Some(GOOGLE_GEMINI_TTS_DEFAULT_VOICE.to_string())
            } else {
                None
            },
            style_instructions: if config.provider == OPENAI_PROVIDER
                && config.scenario == ProviderE2eScenario::TextToAudio
            {
                Some("clear product narration".to_string())
            } else if config.provider == GOOGLE_PROVIDER
                && config.scenario == ProviderE2eScenario::TextToAudio
            {
                Some("firm product narration".to_string())
            } else if matches!(
                config.scenario,
                ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
            ) {
                Some("bright, commercial, loopable".to_string())
            } else {
                None
            },
            lyrics: if config.provider == ELEVENLABS_PROVIDER
                && matches!(
                    config.scenario,
                    ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
                ) {
                Some(ELEVENLABS_TEXT_TO_MUSIC_DEFAULT_LYRICS.to_string())
            } else if config.provider == GOOGLE_PROVIDER
                && matches!(
                    config.scenario,
                    ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
                )
            {
                Some(GOOGLE_LYRIA_TEXT_TO_MUSIC_DEFAULT_LYRICS.to_string())
            } else if config.provider == MINIMAX_PROVIDER
                && matches!(
                    config.scenario,
                    ProviderE2eScenario::TextToMusic | ProviderE2eScenario::TextToMusicInsert
                )
            {
                Some(MINIMAX_TEXT_TO_MUSIC_DEFAULT_LYRICS.to_string())
            } else {
                None
            },
            video_source_start_seconds: if config.scenario
                == ProviderE2eScenario::LocalVideoEditReplace
                || config.scenario == ProviderE2eScenario::LocalVideoMotionControlReplace
                || config.scenario == ProviderE2eScenario::LocalVideoEditRetry
            {
                Some(0.5)
            } else {
                timeline_audio_insert_config(config.scenario)
                    .map(|insert_config| insert_config.video_source_start_seconds)
            },
            video_source_end_seconds: if config.scenario
                == ProviderE2eScenario::LocalVideoEditReplace
                || config.scenario == ProviderE2eScenario::LocalVideoMotionControlReplace
                || config.scenario == ProviderE2eScenario::LocalVideoEditRetry
            {
                Some(3.5)
            } else {
                timeline_audio_insert_config(config.scenario)
                    .map(|insert_config| insert_config.video_source_end_seconds)
            },
            timeline_start_seconds: if let Some(insert_config) =
                timeline_audio_insert_config(config.scenario)
            {
                Some(insert_config.timeline_start_seconds)
            } else if config.scenario == ProviderE2eScenario::TextToVideoInsert {
                Some(6.0)
            } else {
                None
            },
            num_images: if config.scenario == ProviderE2eScenario::MultiImage {
                Some(2)
            } else {
                None
            },
            ..GeneratedAssetSettings::default()
        },
        outputs: Vec::new(),
        created_at: now.clone(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    });
    let start_request = temporal_generate_media_start_request(
        project_id,
        project_dir
            .to_str()
            .ok_or_else(|| "project dir is not utf8".to_string())?,
        asset_id,
        asset_id,
        false,
        Some(TemporalGenerateMediaBrief::default()),
    );
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        project_id,
        asset_id,
        JobStatus::Queued,
        &now,
    );
    job.start_request = Some(start_request.clone());
    project.jobs.push(job);
    save_split_project(&project_dir, &project).map_err(|error| error.to_string())?;
    let mut lifecycle_history = vec![json!({
        "status": "queued",
        "jobStatus": format!("{:?}", project.jobs.last().expect("queued job").status).to_ascii_lowercase(),
        "assetStatus": format!("{:?}", project.generated_assets.last().expect("queued asset").status).to_ascii_lowercase(),
        "observedAt": now,
        "source": "persisted-split-project"
    })];
    let running_at = Utc::now().to_rfc3339();
    project.jobs.last_mut().expect("queued job").status = JobStatus::Running;
    project.jobs.last_mut().expect("queued job").updated_at = running_at.clone();
    project
        .generated_assets
        .last_mut()
        .expect("queued asset")
        .status = GeneratedAssetStatus::Running;
    save_split_project(&project_dir, &project).map_err(|error| error.to_string())?;
    lifecycle_history.push(json!({
        "status": "running",
        "jobStatus": format!("{:?}", project.jobs.last().expect("running job").status).to_ascii_lowercase(),
        "assetStatus": format!("{:?}", project.generated_assets.last().expect("running asset").status).to_ascii_lowercase(),
        "observedAt": running_at,
        "source": "persisted-split-project"
    }));

    if matches!(
        config.scenario,
        ProviderE2eScenario::LocalImageUpscale
            | ProviderE2eScenario::LocalImageUpscaleReplace
            | ProviderE2eScenario::LocalImageUpscaleRetry
            | ProviderE2eScenario::WanImageToVideo
            | ProviderE2eScenario::WanReferenceToVideo
            | ProviderE2eScenario::KlingImageToVideo
            | ProviderE2eScenario::LocalVideoUpscale
            | ProviderE2eScenario::LocalVideoUpscaleReplace
            | ProviderE2eScenario::LocalVideoUpscaleRetry
            | ProviderE2eScenario::LocalVideoEditReplace
            | ProviderE2eScenario::LocalVideoMotionControlReplace
            | ProviderE2eScenario::LocalVideoEditRetry
            | ProviderE2eScenario::VideoToVideo
            | ProviderE2eScenario::ReplicateVideo
            | ProviderE2eScenario::ReplicateVideoFast
            | ProviderE2eScenario::ImageEdit
            | ProviderE2eScenario::LocalImageEditReplace
            | ProviderE2eScenario::LocalImageEditRetry
            | ProviderE2eScenario::VideoToMusic
            | ProviderE2eScenario::LocalVideoToMusicInsert
            | ProviderE2eScenario::VideoToSfx
            | ProviderE2eScenario::LocalVideoToSfxInsert
    ) {
        temporal_generate_media_prepare_provider_inputs_from_project_dir_with_runner(
            &start_request,
            &Utc::now().to_rfc3339(),
            &SystemProcessRunner,
        )
        .map_err(|error| error.to_string())?;
    }

    let submission = temporal_generate_media_provider_submission_from_project_dir(&start_request)
        .map_err(|error| error.to_string())?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| error.to_string())?;
    let updated_at = Utc::now().to_rfc3339();

    let (artifact_path, output_count, job_status, asset_status, terminal_at, provider_request) =
        match submission {
            TemporalGenerateMediaProviderSubmission::Replicate(submission) => {
                let result =
                temporal_generate_media_replicate_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                    ReplicateGenerationRunOptions {
                        max_status_polls: config.max_status_polls,
                        poll_interval: config.poll_interval,
                    },
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::Fal(submission) => {
                let result =
                temporal_generate_media_fal_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                    FalGenerationRunOptions {
                        max_status_polls: config.max_status_polls,
                        poll_interval: config.poll_interval,
                    },
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::OpenAi(submission) => {
                let result =
                temporal_generate_media_openai_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::ElevenLabs(submission) => {
                let result =
                temporal_generate_media_elevenlabs_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::Google(submission) => {
                let result =
                temporal_generate_media_google_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::GoogleGeminiTts(submission) => {
                let result =
                temporal_generate_media_google_gemini_tts_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::GoogleLyria(submission) => {
                let result =
                temporal_generate_media_google_lyria_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::Minimax(submission) => {
                let result =
                temporal_generate_media_minimax_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
            TemporalGenerateMediaProviderSubmission::XAi(submission) => {
                let result =
                temporal_generate_media_xai_run_and_attach_submission_from_project_dir_with_client(
                    &client,
                    &start_request,
                    &submission,
                    &credential,
                    &updated_at,
                    Some("provider-e2e-run"),
                )
                .map_err(|error| error.to_string())?;
                let asset = result
                    .write
                    .project
                    .generated_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| "missing generated asset after provider run".to_string())?;
                let job = result
                    .write
                    .project
                    .jobs
                    .iter()
                    .find(|job| job.id == asset_id)
                    .ok_or_else(|| "missing job after provider run".to_string())?;
                let provider_request = job
                    .provider_request
                    .clone()
                    .ok_or_else(|| "missing provider request after provider run".to_string())?;
                (
                    result.run.completion.output_path.display().to_string(),
                    asset.outputs.len(),
                    format!("{:?}", job.status),
                    format!("{:?}", asset.status),
                    job.updated_at.clone(),
                    provider_request,
                )
            }
        };
    lifecycle_history.push(json!({
        "status": job_status.to_ascii_lowercase(),
        "jobStatus": job_status.to_ascii_lowercase(),
        "assetStatus": asset_status.to_ascii_lowercase(),
        "observedAt": terminal_at,
        "source": "persisted-split-project"
    }));

    Ok(json!({
        "ok": true,
        "scenario": config.scenario.as_str(),
        "provider": config.provider,
        "model": config.model,
        "projectDir": project_dir,
        "artifactPath": artifact_path,
        "outputCount": output_count,
        "jobStatus": job_status,
        "lifecycleHistory": lifecycle_history,
        "providerRequest": provider_request,
    })
    .to_string())
}

fn write_provider_e2e_source_png(
    path: &Path,
    width: u32,
    height: u32,
    color_shift: u32,
) -> Result<(), String> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let checker = if ((x / 8) + (y / 8)) % 2 == 0 {
                36
            } else {
                196
            };
            pixels.extend_from_slice(&[
                checker,
                ((x * 4 + color_shift).min(255)) as u8,
                ((y * 4 + color_shift).min(255)) as u8,
                255,
            ]);
        }
    }
    let file = File::create(path).map_err(|error| error.to_string())?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())
}

fn write_provider_e2e_silent_wav(path: &Path) -> Result<(), String> {
    const SAMPLE_RATE: u32 = 16_000;
    const CHANNELS: u16 = 1;
    const BITS_PER_SAMPLE: u16 = 16;
    const DURATION_SECONDS: u32 = 5;
    let sample_count = SAMPLE_RATE * DURATION_SECONDS;
    let data_size = sample_count * CHANNELS as u32 * (BITS_PER_SAMPLE as u32 / 8);
    let byte_rate = SAMPLE_RATE * CHANNELS as u32 * (BITS_PER_SAMPLE as u32 / 8);
    let block_align = CHANNELS * (BITS_PER_SAMPLE / 8);
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&CHANNELS.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize(44 + data_size as usize, 0);
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use video_creater_lib::generation::openai::{
        OPENAI_GPT_4O_MINI_TTS_MODEL_ID, OPENAI_GPT_IMAGE_2_MODEL_ID,
        OPENAI_GPT_IMAGE_EDIT_MODEL_ID, OPENAI_PROVIDER,
    };

    #[test]
    fn source_png_writer_emits_decodable_non_placeholder_rgba() {
        let path = std::env::temp_dir().join(format!(
            "video-creater-provider-e2e-source-{}-{}.png",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        write_provider_e2e_source_png(&path, 16, 16, 24).expect("write source png");

        let decoder = png::Decoder::new(std::io::BufReader::new(
            File::open(&path).expect("open source png"),
        ));
        let mut reader = decoder.read_info().expect("read source png info");
        let mut buffer = vec![
            0;
            reader
                .output_buffer_size()
                .expect("source png output buffer size")
        ];
        let info = reader.next_frame(&mut buffer).expect("decode source png");
        let bytes = &buffer[..info.buffer_size()];

        assert_eq!(info.width, 16);
        assert_eq!(info.height, 16);
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        assert!(bytes.chunks_exact(4).any(|pixel| pixel[0] != bytes[0]));
        assert!(bytes.chunks_exact(4).any(|pixel| pixel[1] != bytes[1]));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn silent_wav_writer_matches_the_five_second_provider_reference_contract() {
        let path = std::env::temp_dir().join(format!(
            "video-creater-provider-e2e-audio-{}-{}.wav",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        write_provider_e2e_silent_wav(&path).expect("write source wav");
        let bytes = std::fs::read(&path).expect("read source wav");
        let _ = std::fs::remove_file(&path);

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        let sample_rate = u32::from_le_bytes(bytes[24..28].try_into().expect("sample rate"));
        let data_size = u32::from_le_bytes(bytes[40..44].try_into().expect("data size"));
        assert_eq!(sample_rate, 16_000);
        assert_eq!(data_size, 16_000 * 5 * 2);
        assert_eq!(bytes.len(), 44 + data_size as usize);
    }

    #[test]
    fn parse_args_accepts_openai_text_to_image_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            OPENAI_PROVIDER.to_string(),
            "--model".to_string(),
            OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-image".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/openai-live".to_string(),
        ])
        .expect("OpenAI text-to-image config");

        assert_eq!(config.provider, OPENAI_PROVIDER);
        assert_eq!(config.model, OPENAI_GPT_IMAGE_2_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToImage);
    }

    #[test]
    fn parse_args_accepts_replicate_flux_variant_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            REPLICATE_PROVIDER.to_string(),
            "--model".to_string(),
            REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "replicate-flux-1.1-pro-ultra".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/replicate-flux-1.1-pro-ultra-live".to_string(),
        ])
        .expect("Replicate Flux 1.1 Pro Ultra config");

        assert_eq!(config.provider, REPLICATE_PROVIDER);
        assert_eq!(config.model, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID);
        assert_eq!(
            config.scenario,
            ProviderE2eScenario::ReplicateFlux11ProUltra
        );
    }

    #[test]
    fn parse_args_rejects_replicate_flux_variant_with_wrong_model() {
        let error = parse_args([
            "--provider".to_string(),
            REPLICATE_PROVIDER.to_string(),
            "--model".to_string(),
            REPLICATE_FLUX_DEV_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "replicate-flux-1.1-pro".to_string(),
        ])
        .expect_err("Replicate Flux 1.1 Pro scenario should require Pro model");

        assert!(error.contains(REPLICATE_FLUX_11_PRO_MODEL_ID));
    }

    #[test]
    fn parse_args_accepts_multi_image_live_scenario() {
        for (provider, model, out_dir) in [
            (
                FAL_PROVIDER,
                FAL_FLUX_SCHNELL_MODEL_ID,
                "output/provider-e2e/fal.ai-multi-image-live",
            ),
            (
                OPENAI_PROVIDER,
                OPENAI_GPT_IMAGE_2_MODEL_ID,
                "output/provider-e2e/openai-multi-image-live",
            ),
        ] {
            let config = parse_args([
                "--provider".to_string(),
                provider.to_string(),
                "--model".to_string(),
                model.to_string(),
                "--scenario".to_string(),
                "multi-image".to_string(),
                "--out-dir".to_string(),
                out_dir.to_string(),
            ])
            .expect("multi-image config");

            assert_eq!(config.provider, provider);
            assert_eq!(config.model, model);
            assert_eq!(config.scenario, ProviderE2eScenario::MultiImage);
        }
    }

    #[test]
    fn parse_args_accepts_cross_provider_local_image_edit_replacement_live_scenario() {
        for (provider, model, out_dir) in [
            (
                OPENAI_PROVIDER,
                OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
                "output/provider-e2e/openai-image-edit-replace-live",
            ),
            (
                XAI_PROVIDER,
                XAI_GROK_IMAGE_QUALITY_MODEL_ID,
                "output/provider-e2e/xai-image-edit-replace-live",
            ),
        ] {
            let config = parse_args([
                "--provider".to_string(),
                provider.to_string(),
                "--model".to_string(),
                model.to_string(),
                "--scenario".to_string(),
                "local-image-edit-replace".to_string(),
                "--out-dir".to_string(),
                out_dir.to_string(),
            ])
            .expect("local image-edit replacement config");

            assert_eq!(config.provider, provider);
            assert_eq!(config.model, model);
            assert_eq!(config.scenario, ProviderE2eScenario::LocalImageEditReplace);
        }
    }

    #[test]
    fn parse_args_accepts_xai_source_video_edit_replacement_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            XAI_PROVIDER.to_string(),
            "--model".to_string(),
            XAI_GROK_VIDEO_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "local-video-edit-replace".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/xai-video-edit-replace-live".to_string(),
        ])
        .expect("xAI source-video edit replacement config");

        assert_eq!(config.provider, XAI_PROVIDER);
        assert_eq!(config.model, XAI_GROK_VIDEO_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::LocalVideoEditReplace);
    }

    #[test]
    fn parse_args_accepts_replicate_text_to_video_insert_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            REPLICATE_PROVIDER.to_string(),
            "--model".to_string(),
            REPLICATE_SEEDANCE_20_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-video-insert".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/replicate-text-video-insert-live".to_string(),
        ])
        .expect("Replicate text-to-video insert config");

        assert_eq!(config.provider, REPLICATE_PROVIDER);
        assert_eq!(config.model, REPLICATE_SEEDANCE_20_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToVideoInsert);
    }

    #[test]
    fn parse_args_accepts_video_generate_audio_override() {
        let config = parse_args([
            "--provider".to_string(),
            REPLICATE_PROVIDER.to_string(),
            "--model".to_string(),
            REPLICATE_SEEDANCE_20_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "replicate-video".to_string(),
            "--generate-audio".to_string(),
            "false".to_string(),
        ])
        .expect("Replicate video should accept generate-audio override");

        assert_eq!(config.scenario, ProviderE2eScenario::ReplicateVideo);
        assert!(!config.generate_audio);
    }

    #[test]
    fn parse_args_accepts_replicate_text_to_video_replace_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            REPLICATE_PROVIDER.to_string(),
            "--model".to_string(),
            REPLICATE_SEEDANCE_20_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-video-replace".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/replicate-text-video-replace-live".to_string(),
        ])
        .expect("Replicate text-to-video replacement config");

        assert_eq!(config.provider, REPLICATE_PROVIDER);
        assert_eq!(config.model, REPLICATE_SEEDANCE_20_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToVideoReplace);
    }

    #[test]
    fn parse_args_accepts_fal_text_to_video_placement_live_scenarios() {
        for (scenario, out_dir, expected_scenario) in [
            (
                "text-to-video-insert",
                "output/provider-e2e/fal.ai-text-video-insert-live",
                ProviderE2eScenario::TextToVideoInsert,
            ),
            (
                "text-to-video-replace",
                "output/provider-e2e/fal.ai-text-video-replace-live",
                ProviderE2eScenario::TextToVideoReplace,
            ),
        ] {
            let config = parse_args([
                "--provider".to_string(),
                FAL_PROVIDER.to_string(),
                "--model".to_string(),
                FAL_WAN_TEXT_TO_VIDEO_MODEL_ID.to_string(),
                "--scenario".to_string(),
                scenario.to_string(),
                "--out-dir".to_string(),
                out_dir.to_string(),
            ])
            .expect("fal text-to-video placement config");

            assert_eq!(config.provider, FAL_PROVIDER);
            assert_eq!(config.model, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID);
            assert_eq!(config.scenario, expected_scenario);
        }
    }

    #[test]
    fn parse_args_accepts_xai_text_to_video_insert_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            XAI_PROVIDER.to_string(),
            "--model".to_string(),
            XAI_GROK_VIDEO_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-video-insert".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/xai-text-video-insert-live".to_string(),
        ])
        .expect("xAI text-to-video insert config");

        assert_eq!(config.provider, XAI_PROVIDER);
        assert_eq!(config.model, XAI_GROK_VIDEO_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToVideoInsert);
    }

    #[test]
    fn parse_args_accepts_google_text_to_video_insert_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            GOOGLE_PROVIDER.to_string(),
            "--model".to_string(),
            GOOGLE_VEO_31_FAST_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-video-insert".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/google-text-video-insert-live".to_string(),
        ])
        .expect("Google text-to-video insert config");

        assert_eq!(config.provider, GOOGLE_PROVIDER);
        assert_eq!(config.model, GOOGLE_VEO_31_FAST_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToVideoInsert);
    }

    #[test]
    fn parse_args_accepts_google_text_to_video_replace_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            GOOGLE_PROVIDER.to_string(),
            "--model".to_string(),
            GOOGLE_VEO_31_FAST_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-video-replace".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/google-text-video-replace-live".to_string(),
        ])
        .expect("Google text-to-video replacement config");

        assert_eq!(config.provider, GOOGLE_PROVIDER);
        assert_eq!(config.model, GOOGLE_VEO_31_FAST_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToVideoReplace);
    }

    #[test]
    fn parse_args_rejects_xai_source_video_edit_replacement_with_wrong_model() {
        let error = parse_args([
            "--provider".to_string(),
            XAI_PROVIDER.to_string(),
            "--model".to_string(),
            XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "local-video-edit-replace".to_string(),
        ])
        .expect_err("xAI source-video edit replacement should require video model");

        assert!(error.contains(XAI_GROK_VIDEO_MODEL_ID));
    }

    #[test]
    fn parse_args_defaults_source_video_audio_scenarios_to_promptless() {
        for (scenario, model) in [
            ("video-to-music", FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID),
            (
                "local-video-to-music-insert",
                FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
            ),
            ("video-to-sfx", FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID),
            (
                "local-video-to-sfx-insert",
                FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
            ),
        ] {
            let config = parse_args([
                "--provider".to_string(),
                FAL_PROVIDER.to_string(),
                "--model".to_string(),
                model.to_string(),
                "--scenario".to_string(),
                scenario.to_string(),
            ])
            .expect("source-video audio scenario config");

            assert_eq!(config.prompt, "", "{scenario} should default to promptless");
        }
    }

    #[test]
    fn parse_args_preserves_explicit_source_video_audio_prompt() {
        let config = parse_args([
            "--provider".to_string(),
            FAL_PROVIDER.to_string(),
            "--model".to_string(),
            FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "local-video-to-sfx-insert".to_string(),
            "--prompt".to_string(),
            "sharp interface confirmation sound".to_string(),
        ])
        .expect("source-video audio scenario config with explicit prompt");

        assert_eq!(config.prompt, "sharp interface confirmation sound");
    }

    #[test]
    fn parse_args_accepts_wan_reference_to_video_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            FAL_PROVIDER.to_string(),
            "--model".to_string(),
            FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "wan-reference-to-video".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/fal.ai-wan-reference-to-video-live".to_string(),
        ])
        .expect("fal.ai WAN reference-to-video config");

        assert_eq!(config.provider, FAL_PROVIDER);
        assert_eq!(config.model, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::WanReferenceToVideo);
    }

    #[test]
    fn parse_args_accepts_kling_motion_control_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            FAL_PROVIDER.to_string(),
            "--model".to_string(),
            FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "local-video-motion-control-replace".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/fal.ai-video-motion-control-replace-live".to_string(),
        ])
        .expect("fal.ai Kling Motion Control config");

        assert_eq!(config.provider, FAL_PROVIDER);
        assert_eq!(config.model, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID);
        assert_eq!(
            config.scenario,
            ProviderE2eScenario::LocalVideoMotionControlReplace
        );
    }

    #[test]
    fn parse_args_rejects_wan_reference_to_video_with_image_to_video_model() {
        let error = parse_args([
            "--provider".to_string(),
            FAL_PROVIDER.to_string(),
            "--model".to_string(),
            FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "wan-reference-to-video".to_string(),
        ])
        .expect_err("WAN reference-to-video should require reference-to-video model");

        assert!(error.contains(FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID));
    }

    #[test]
    fn parse_args_rejects_multi_image_with_unsupported_provider() {
        let error = parse_args([
            "--provider".to_string(),
            XAI_PROVIDER.to_string(),
            "--model".to_string(),
            XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "multi-image".to_string(),
        ])
        .expect_err("multi-image should require fal.ai, OpenAI, or replicate");

        assert!(error.contains("multi-image provider E2E requires fal.ai, OpenAI, or replicate"));
    }

    #[test]
    fn parse_args_accepts_openai_image_edit_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            OPENAI_PROVIDER.to_string(),
            "--model".to_string(),
            OPENAI_GPT_IMAGE_EDIT_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "image-edit".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/openai-image-edit-live".to_string(),
        ])
        .expect("OpenAI image-edit config");

        assert_eq!(config.provider, OPENAI_PROVIDER);
        assert_eq!(config.model, OPENAI_GPT_IMAGE_EDIT_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::ImageEdit);
    }

    #[test]
    fn parse_args_rejects_openai_image_edit_with_text_to_image_model() {
        let error = parse_args([
            "--provider".to_string(),
            OPENAI_PROVIDER.to_string(),
            "--model".to_string(),
            OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "image-edit".to_string(),
        ])
        .expect_err("OpenAI image-edit should require edit model");

        assert!(error.contains(OPENAI_GPT_IMAGE_EDIT_MODEL_ID));
    }

    #[test]
    fn parse_args_accepts_openai_text_to_audio_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            OPENAI_PROVIDER.to_string(),
            "--model".to_string(),
            OPENAI_GPT_4O_MINI_TTS_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-audio".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/openai-audio-live".to_string(),
        ])
        .expect("OpenAI text-to-audio config");

        assert_eq!(config.provider, OPENAI_PROVIDER);
        assert_eq!(config.model, OPENAI_GPT_4O_MINI_TTS_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToAudio);
    }

    #[test]
    fn parse_args_rejects_openai_text_to_audio_with_image_model() {
        let error = parse_args([
            "--provider".to_string(),
            OPENAI_PROVIDER.to_string(),
            "--model".to_string(),
            OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-audio".to_string(),
        ])
        .expect_err("OpenAI text-to-audio should require speech model");

        assert!(error.contains(OPENAI_GPT_4O_MINI_TTS_MODEL_ID));
    }

    #[test]
    fn parse_args_accepts_elevenlabs_text_to_music_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            ELEVENLABS_PROVIDER.to_string(),
            "--model".to_string(),
            ELEVENLABS_MUSIC_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-music".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/elevenlabs-music-live".to_string(),
        ])
        .expect("ElevenLabs text-to-music config");

        assert_eq!(config.provider, ELEVENLABS_PROVIDER);
        assert_eq!(config.model, ELEVENLABS_MUSIC_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToMusic);
    }

    #[test]
    fn parse_args_accepts_fal_text_to_music_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            FAL_PROVIDER.to_string(),
            "--model".to_string(),
            FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-music".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/fal.ai-sonilo-music-live".to_string(),
        ])
        .expect("fal.ai text-to-music config");

        assert_eq!(config.provider, FAL_PROVIDER);
        assert_eq!(config.model, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToMusic);
    }

    #[test]
    fn parse_args_accepts_fal_text_to_music_insert_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            FAL_PROVIDER.to_string(),
            "--model".to_string(),
            FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-music-insert".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/fal.ai-sonilo-music-insert-live".to_string(),
        ])
        .expect("fal.ai text-to-music insert config");

        assert_eq!(config.provider, FAL_PROVIDER);
        assert_eq!(config.model, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToMusicInsert);
    }

    #[test]
    fn parse_args_accepts_google_text_to_music_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            GOOGLE_PROVIDER.to_string(),
            "--model".to_string(),
            GOOGLE_LYRIA_3_PRO_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-music".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/google-lyria-live".to_string(),
        ])
        .expect("Google text-to-music config");

        assert_eq!(config.provider, GOOGLE_PROVIDER);
        assert_eq!(config.model, GOOGLE_LYRIA_3_PRO_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToMusic);
    }

    #[test]
    fn parse_args_accepts_minimax_text_to_music_live_scenario() {
        let config = parse_args([
            "--provider".to_string(),
            MINIMAX_PROVIDER.to_string(),
            "--model".to_string(),
            MINIMAX_MUSIC_MODEL_ID.to_string(),
            "--scenario".to_string(),
            "text-to-music".to_string(),
            "--out-dir".to_string(),
            "output/provider-e2e/minimax-music-live".to_string(),
        ])
        .expect("MiniMax text-to-music config");

        assert_eq!(config.provider, MINIMAX_PROVIDER);
        assert_eq!(config.model, MINIMAX_MUSIC_MODEL_ID);
        assert_eq!(config.scenario, ProviderE2eScenario::TextToMusic);
    }
}
