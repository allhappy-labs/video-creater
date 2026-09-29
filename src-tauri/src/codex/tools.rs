use super::conversation::{undo_codex_conversation_edit, undo_codex_conversation_edit_with};
use super::proposal::{validate_codex_edit_proposal, CodexEditProposal};
use crate::audio_sync::{build_audio_sync_report, AudioSyncRequest};
use crate::edit::preset::EditJobRequest;
use crate::edit::render_plan::{
    generate_spoken_semantic_multi_source_edit_timeline, RenderQuality, RenderQualityProfile,
};
use crate::effects::{canonical_effect_rank, effect_catalog, effect_catalog_payload};
use crate::generation::catalog::resolve_generation_catalog;
use crate::generation::elevenlabs::{
    ELEVENLABS_DEFAULT_VOICE, ELEVENLABS_MUSIC_MODEL_ID, ELEVENLABS_PROVIDER,
    ELEVENLABS_TTS_V3_MODEL_ID,
};
use crate::generation::fal::{
    FAL_AURA_SR_MODEL_ID, FAL_FLUX_SCHNELL_MODEL_ID, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
    FAL_KREA_2_TURBO_MODEL_ID, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
    FAL_NANO_BANANA_PRO_EDIT_MODEL_ID, FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
    FAL_SEED_AUDIO_MODEL_ID, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
    FAL_VIDEO_UPSCALER_MODEL_ID, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
    FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
    FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
};
use crate::generation::google::{
    GOOGLE_GEMINI_TTS_DEFAULT_VOICE, GOOGLE_GEMINI_TTS_MODEL_ID, GOOGLE_GEMINI_TTS_VOICES,
    GOOGLE_LYRIA_3_PRO_MODEL_ID, GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID,
};
use crate::generation::minimax::{MINIMAX_MUSIC_MODEL_ID, MINIMAX_PROVIDER};
use crate::generation::openai::{
    OPENAI_GPT_4O_MINI_TTS_MODEL_ID, OPENAI_GPT_IMAGE_2_MODEL_ID, OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
    OPENAI_PROVIDER,
};
use crate::generation::replicate::{
    REPLICATE_FLUX_11_PRO_MODEL_ID, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
    REPLICATE_FLUX_DEV_MODEL_ID, REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER,
    REPLICATE_SEEDANCE_20_FAST_MODEL_ID, REPLICATE_SEEDANCE_20_MODEL_ID,
};
use crate::generation::xai::{
    XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
};
use crate::project::action::{
    apply_project_action, item_speed, ProjectAction, ProjectActionColorGrade, ProjectActionEffect,
    ProjectActionGeneratedAsset, ProjectActionGeneratedAssetReferences,
    ProjectActionGeneratedAssetSettings, ProjectActionGenerationModel,
    ProjectActionItemPropertiesUpdate, ProjectActionKeyframe, ProjectActionKeyframeProperty,
    ProjectActionMove, ProjectActionResize, ProjectActionRippleDeleteRange, ProjectActionSplit,
    ProjectActionTextOverlayUpdate, ProjectActionTrim, ProjectActionVisualTransform,
};
use crate::project::export_options::ExportRenderOptions;
use crate::project::export_profiles::{
    mp4_export_profile_availability_report, ExportProfile, ExportProfileAvailability,
};
use crate::project::import::{
    import_media_files, path_is_lottie_media, persist_imported_project, project_uses_split_layout,
};
use crate::project::matte::{create_matte_for_project, MatteError, MatteRequest};
use crate::project::model::{
    CaptionRenderMode, GeneratedAsset, GeneratedAssetStatus, JobProviderRequest, JobStatus,
    MediaAsset, MediaFolder, MediaKind, MediaSilenceRange, TimelineItem, TimelineItemKind,
    TimelineSource, TimelineTrack, TrackKind, Transcript, TranscriptWord, VideoProject,
};
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::nle_export::{
    export_project_timeline_to_nle_xml, NleXmlExportError, NleXmlFormat,
};
use crate::project::reverse::{is_reversed, is_reversible_clip, SourceWindow};
use crate::project::split::{
    apply_project_actions_to_split_project, legacy_agent_undo_refusal, load_split_project,
    record_agent_project_action_batch, ProjectAgentUndoOutcome,
};
use crate::project::storage;
use crate::render_pipeline::error::PipelineError;
use crate::render_pipeline::project_export::build_project_webm_render_plan;
use crate::search::semantic_visual::SemanticVisualHit;
use crate::search::{
    query_project_search, query_project_search_for_project_dir, ProjectSearchQuery, SearchScope,
};
use crate::settings::skills::{
    mandatory_skill_definition, mandatory_skill_prompt_bundle, sha256_checksum,
};
use crate::transcription::model::transcription_model_catalog;
use crate::workflows::{
    temporal_codex_edit_start_request,
    temporal_export_media_start_request_with_options_and_overwrite,
    temporal_export_nle_xml_start_request, temporal_export_nle_xml_start_request_with_overwrite,
    temporal_generate_media_start_request, temporal_job_summary,
    temporal_render_draft_start_request, temporal_workflow_start_request,
    TemporalGenerateMediaBrief, TemporalWorkflowKind,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

const MAX_TOOL_MEDIA: usize = 80;
const MAX_TOOL_FOLDERS: usize = 40;
const MAX_TOOL_TRACKS: usize = 12;
const MAX_TOOL_ITEMS_PER_TRACK: usize = 120;
const MAX_CAPTION_GROUP_ROWS: usize = 200;
const DEFAULT_TRANSCRIPT_WORD_LIMIT: usize = 200;
const MAX_TRANSCRIPT_WORD_LIMIT: usize = 500;
const IMPORT_BYTES_MAX_BASE64_LENGTH: usize = 15 * 1024 * 1024;
const IMPORT_URL_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const AUDIO_DENOISE_EFFECT_TYPE: &str = "audio.denoise";
const AUDIO_DENOISE_DEFAULT_AMOUNT: f64 = 0.6;
const PALMIER_BLEND_MODES: &[&str] = &[
    "normal",
    "darken",
    "multiply",
    "colorBurn",
    "lighten",
    "screen",
    "colorDodge",
    "overlay",
    "softLight",
    "hardLight",
    "difference",
    "exclusion",
    "hue",
    "saturation",
    "color",
    "luminosity",
];
const REMOVE_SILENCE_MIN_GAP_SECONDS: f64 = 0.75;
const REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS: f64 = 0.12;
const AUDIO_TTS_PROMPT_LABEL: &str = "Text to speak";
const AUDIO_MUSIC_PROMPT_LABEL: &str = "Describe the music style or mood";
const AUDIO_SFX_PROMPT_LABEL: &str = "Describe the sound";

mod audio_edits;
mod transitions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexLocalToolDescriptor {
    pub name: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexLocalToolCallResult {
    pub tool_name: String,
    pub mutates_project: bool,
    pub payload: Value,
}

#[derive(Debug, Error, PartialEq)]
pub enum CodexLocalToolError {
    #[error("unknown Codex local tool: {0}")]
    UnknownTool(String),
    #[error("invalid Codex local tool arguments: {0}")]
    InvalidArguments(String),
    #[error("project action validation failed: {0}")]
    ProjectActionValidation(String),
    #[error("Codex edit proposal validation failed: {0}")]
    ProposalValidation(String),
    #[error("project action application failed: {0}")]
    ProjectActionApplication(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectContextArgs {
    project_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerationDefaultsArgs {
    selected_media_id: Option<String>,
    prompt: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListModelsArgs {
    #[serde(default)]
    #[serde(rename = "type")]
    model_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValidateProjectActionsArgs {
    actions: Vec<ProjectAction>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AddClipsArgs {
    Palmier(PalmierAddClipsArgs),
    Direct(DirectAddClipsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum InsertClipsArgs {
    Palmier(PalmierInsertClipsArgs),
    Direct(DirectInsertClipsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectAddClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    target_track_id: String,
    clips: Vec<DirectMediaClipEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PalmierAddClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    entries: Vec<PalmierMediaClipEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PalmierInsertClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    track_index: usize,
    at_frame: u64,
    entries: Vec<PalmierInsertClipEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PalmierInsertClipEntry {
    #[serde(alias = "mediaRef")]
    media_id: String,
    duration_frames: Option<u64>,
    trim_start_frame: Option<u64>,
    trim_end_frame: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PalmierMediaClipEntry {
    #[serde(default)]
    id: Option<String>,
    #[serde(alias = "mediaRef")]
    media_id: String,
    track_index: Option<usize>,
    start_frame: u64,
    duration_frames: Option<u64>,
    trim_start_frame: Option<u64>,
    trim_end_frame: Option<u64>,
    label: Option<String>,
    opacity: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectInsertClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    target_track_id: String,
    insert_seconds: f64,
    clips: Vec<DirectMediaClipEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectMediaClipEntry {
    id: String,
    media_id: String,
    start_seconds: Option<f64>,
    duration_seconds: f64,
    source_in: Option<f64>,
    source_out: Option<f64>,
    label: Option<String>,
    opacity: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AddTextsArgs {
    Direct(DirectAddTextsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectAddTextsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    target_track_id: Option<String>,
    #[serde(default)]
    texts: Vec<DirectTextOverlayEntry>,
    #[serde(default)]
    entries: Vec<PalmierTextEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectTextOverlayEntry {
    id: String,
    text: String,
    start_seconds: f64,
    duration_seconds: f64,
    label: Option<String>,
    visual_treatment: String,
    motion: String,
    safe_zone: String,
    avoid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PalmierTextEntry {
    id: Option<String>,
    track_index: Option<usize>,
    start_frame: u64,
    duration_frames: u64,
    content: String,
    transform: Option<Value>,
    font_name: Option<String>,
    font_size: Option<f64>,
    color: Option<String>,
    alignment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AddCaptionsArgs {
    Direct(DirectAddCaptionsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectAddCaptionsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    target_track_id: Option<String>,
    #[serde(default)]
    captions: Vec<DirectCaptionEntry>,
    #[serde(default)]
    clip_ids: Vec<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    font_name: Option<String>,
    #[serde(default)]
    font_size: Option<f64>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    center_x: Option<f64>,
    #[serde(default)]
    center_y: Option<f64>,
    #[serde(default)]
    text_case: Option<String>,
    #[serde(default)]
    censor_profanity: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectCaptionEntry {
    id: String,
    text: String,
    start_seconds: f64,
    duration_seconds: f64,
    label: Option<String>,
    visual_treatment: Option<String>,
    motion: Option<String>,
    safe_zone: Option<String>,
    avoid: Option<String>,
}

#[derive(Debug)]
enum RemoveClipsArgs {
    Direct(DirectRemoveClipsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

impl<'de> Deserialize<'de> for RemoveClipsArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.get("actions").is_some() {
            serde_json::from_value(value)
                .map(Self::ProjectActions)
                .map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Self::Direct)
                .map_err(serde::de::Error::custom)
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectRemoveClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(alias = "clipIds")]
    item_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MoveClipsArgs {
    Direct(DirectMoveClipsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectMoveClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    moves: Vec<MoveClipEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MoveClipEntry {
    #[serde(alias = "clipId")]
    item_id: String,
    #[serde(default)]
    target_track_id: Option<String>,
    #[serde(default)]
    start_seconds: Option<f64>,
    #[serde(default)]
    to_track: Option<usize>,
    #[serde(default)]
    to_frame: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SplitClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    splits: Vec<SplitClipEntry>,
    #[serde(default)]
    track_index: Option<usize>,
    #[serde(default)]
    frames: Vec<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SplitClipEntry {
    #[serde(alias = "clipId")]
    item_id: String,
    #[serde(default)]
    split_seconds: Option<f64>,
    #[serde(default)]
    at_frame: Option<u64>,
    #[serde(default)]
    new_item_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SetClipPropertiesArgs {
    Direct(Box<DirectSetClipPropertiesArgs>),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateTextArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    clip_ids: Vec<String>,
    #[serde(default)]
    caption_group_id: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    transform: Option<ProjectActionVisualTransform>,
    #[serde(default, alias = "fontFamily")]
    font_name: Option<String>,
    #[serde(default)]
    font_size: Option<f64>,
    #[serde(default)]
    is_bold: Option<bool>,
    #[serde(default)]
    is_italic: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    alignment: Option<String>,
    #[serde(default)]
    border_color: Option<String>,
    #[serde(default)]
    background_color: Option<String>,
    #[serde(default)]
    animation: Option<String>,
    #[serde(default)]
    highlight_color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectSetClipPropertiesArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    updates: Vec<ClipPropertiesUpdate>,
    #[serde(default)]
    clip_ids: Vec<String>,
    #[serde(default)]
    duration_frames: Option<u64>,
    #[serde(default)]
    trim_start_frame: Option<u64>,
    #[serde(default)]
    trim_end_frame: Option<u64>,
    #[serde(default)]
    speed: Option<f64>,
    #[serde(default)]
    reverse: Option<bool>,
    #[serde(default)]
    volume: Option<f64>,
    #[serde(default)]
    fade_out_frames: Option<u64>,
    #[serde(default)]
    fade_out_seconds: Option<f64>,
    #[serde(default)]
    opacity: Option<f64>,
    #[serde(default)]
    transform: Option<ProjectActionVisualTransform>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    font_name: Option<String>,
    #[serde(default)]
    font_size: Option<f64>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    alignment: Option<String>,
    #[serde(default)]
    blend_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SetKeyframesArgs {
    Direct(DirectSetKeyframesArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectSetKeyframesArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(alias = "clipId")]
    item_id: String,
    property: String,
    keyframes: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClipPropertiesUpdate {
    #[serde(alias = "clipId")]
    item_id: String,
    start_seconds: Option<f64>,
    duration_seconds: Option<f64>,
    source_in: Option<f64>,
    source_out: Option<f64>,
    opacity: Option<f64>,
    transform: Option<ProjectActionVisualTransform>,
    text: Option<String>,
    volume_db: Option<f64>,
    fade_out_seconds: Option<f64>,
    font_name: Option<String>,
    font_size: Option<f64>,
    color: Option<String>,
    alignment: Option<String>,
    blend_mode: Option<String>,
    reverse: Option<bool>,
    #[serde(skip)]
    speed: Option<f64>,
    #[serde(skip)]
    timing_source: ClipPropertiesTimingSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ClipPropertiesTimingSource {
    #[default]
    Direct,
    PalmierFrameFields,
    PalmierSpeedOnly,
}

impl ClipPropertiesUpdate {
    fn has_text_style(&self) -> bool {
        self.font_name.is_some()
            || self.font_size.is_some()
            || self.color.is_some()
            || self.alignment.is_some()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RemoveWordsArgs {
    Direct(DirectRemoveWordsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApplyLayoutArgs {
    Direct(DirectApplyLayoutArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RemoveSilenceArgs {
    Direct(DirectRemoveSilenceArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RippleDeleteRangesArgs {
    Direct(DirectRippleDeleteRangesArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectRippleDeleteRangesArgs {
    #[serde(default)]
    project_dir: Option<String>,
    ranges: Vec<Value>,
    #[serde(default)]
    track_index: Option<usize>,
    #[serde(default, alias = "itemId")]
    clip_id: Option<String>,
    #[serde(default)]
    units: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectRemoveWordsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    media_id: Option<String>,
    #[serde(default)]
    word_indexes: Vec<usize>,
    #[serde(default)]
    words: Vec<Value>,
    #[serde(default)]
    cut_aggressiveness: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectApplyLayoutArgs {
    #[serde(default)]
    project_dir: Option<String>,
    layout: String,
    slots: Vec<ApplyLayoutSlotArg>,
    #[serde(default)]
    start_frame: Option<u64>,
    #[serde(default)]
    duration_frames: Option<u64>,
    #[serde(default)]
    fit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyLayoutSlotArg {
    slot: String,
    #[serde(default, alias = "mediaRef")]
    media_id: Option<String>,
    #[serde(default)]
    clip_ids: Option<Vec<String>>,
    #[serde(default)]
    anchor: Option<String>,
    #[serde(default)]
    anchor_x: Option<f64>,
    #[serde(default)]
    anchor_y: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectRemoveSilenceArgs {
    #[serde(default)]
    project_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GetTimelineArgs {
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_query_frame")]
    start_frame: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_query_frame")]
    end_frame: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectTimelineArgs {
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_query_frame")]
    start_frame: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_query_frame")]
    end_frame: Option<u64>,
    max_items: Option<usize>,
    max_frames: Option<usize>,
}

fn deserialize_optional_query_frame<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(query_frame_from_value))
}

fn query_frame_from_value(value: Value) -> Option<u64> {
    let frame = match value {
        Value::Number(number) => {
            if let Some(frame) = number.as_u64() {
                frame
            } else if let Some(frame) = number.as_i64() {
                u64::try_from(frame).ok()?
            } else {
                let frame = number.as_f64()?;
                if !frame.is_finite() || frame < 0.0 || frame > i64::MAX as f64 {
                    return None;
                }
                frame.trunc() as u64
            }
        }
        _ => return None,
    };

    (frame <= i64::MAX as u64).then_some(frame)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolProjectActionsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    actions: Vec<ProjectAction>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SyncAudioArgs {
    Direct(DirectSyncAudioArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectSyncAudioArgs {
    reference_clip_id: String,
    #[serde(default)]
    target_clip_id: Option<String>,
    #[serde(default)]
    target_clip_ids: Vec<String>,
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    search_window_seconds: Option<f64>,
    #[serde(default)]
    min_confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportMediaArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    source_paths: Vec<String>,
    #[serde(default)]
    source: Option<PalmierImportSource>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateMatteArgs {
    #[serde(default)]
    project_dir: Option<String>,
    hex: String,
    #[serde(default)]
    aspect_ratio: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PalmierImportSource {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    bytes: Option<String>,
    #[serde(default)]
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadSkillArgs {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RemoveTracksArgs {
    Direct(DirectRemoveTracksArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectRemoveTracksArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    track_indexes: Vec<usize>,
    #[serde(default)]
    track_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LinkClipsArgs {
    Direct(DirectLinkClipsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectLinkClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default, alias = "itemIds")]
    clip_ids: Vec<String>,
    #[serde(default)]
    link_group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UnlinkClipsArgs {
    Direct(DirectUnlinkClipsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectUnlinkClipsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default, alias = "itemIds")]
    clip_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenameMediaArgs {
    #[serde(default)]
    project_dir: Option<String>,
    media_ref: Option<String>,
    name: Option<String>,
    #[serde(default)]
    entries: Vec<RenameMediaEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenameMediaEntry {
    media_ref: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeleteMediaArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(alias = "mediaIds")]
    asset_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CreateFolderArgs {
    Direct(DirectCreateFolderArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectCreateFolderArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
    name: Option<String>,
    #[serde(default, alias = "parentFolderId")]
    parent_id: Option<String>,
    #[serde(default)]
    entries: Vec<CreateFolderEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateFolderEntry {
    #[serde(default)]
    folder_id: Option<String>,
    name: String,
    #[serde(default)]
    #[serde(alias = "parentFolderId")]
    parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MoveToFolderArgs {
    Direct(DirectMoveToFolderArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectMoveToFolderArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default, alias = "assetIds")]
    media_ids: Vec<String>,
    #[serde(default)]
    folder_id: Option<String>,
    #[serde(default)]
    entries: Vec<MoveToFolderEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MoveToFolderEntry {
    #[serde(alias = "mediaIds")]
    asset_ids: Vec<String>,
    #[serde(default)]
    folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RenameFolderArgs {
    Direct(DirectRenameFolderArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectRenameFolderArgs {
    #[serde(default)]
    project_dir: Option<String>,
    folder_id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    entries: Vec<RenameFolderEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenameFolderEntry {
    folder_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DeleteFolderArgs {
    Direct(DirectDeleteFolderArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectDeleteFolderArgs {
    #[serde(default)]
    project_dir: Option<String>,
    folder_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SetProjectSettingsArgs {
    Direct(DirectSetProjectSettingsArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DirectSetProjectSettingsArgs {
    #[serde(default)]
    project_dir: Option<String>,
    fps: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    aspect_ratio: Option<String>,
    quality: Option<String>,
    loudness_lufs: Option<f64>,
    captions: Option<CaptionRenderMode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyEffectArgs {
    #[serde(default)]
    project_dir: Option<String>,
    clip_ids: Vec<String>,
    #[serde(default)]
    effects: Vec<ApplyEffectEntry>,
    #[serde(default)]
    remove: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyEffectEntry {
    #[serde(rename = "type")]
    effect_type: String,
    #[serde(default)]
    params: serde_json::Map<String, Value>,
    enabled: Option<bool>,
}

#[derive(Debug, Clone)]
struct NormalizedEffectEdit {
    effect_type: String,
    params: BTreeMap<String, Value>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DenoiseAudioArgs {
    #[serde(default)]
    project_dir: Option<String>,
    clip_ids: Vec<String>,
    strength: Option<f64>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApplyColorArgs {
    Direct(DirectApplyColorArgs),
    ProjectActions(ToolProjectActionsArgs),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectApplyColorArgs {
    #[serde(default)]
    project_dir: Option<String>,
    clip_ids: Vec<String>,
    #[serde(default)]
    reset: bool,
    exposure: Option<f64>,
    contrast: Option<f64>,
    saturation: Option<f64>,
    temperature: Option<f64>,
    tint: Option<f64>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectColorArgs {
    project_dir: Option<String>,
    clip_id: Option<String>,
    #[serde(default, alias = "mediaRef")]
    media_id: Option<String>,
    at_frame: Option<u64>,
    reference: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendFeedbackArgs {
    category: String,
    summary: String,
    details: Option<String>,
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchMediaArgs {
    query: String,
    limit: Option<usize>,
    scope: Option<String>,
    #[serde(default, alias = "mediaRef")]
    media_id: Option<String>,
    project_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildMultiSourceEditArgs {
    request: EditJobRequest,
    #[serde(default)]
    semantic_hits: Vec<SemanticVisualHit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InspectMediaArgs {
    #[serde(default, alias = "mediaRef")]
    media_id: String,
    #[serde(default, alias = "clipId")]
    clip_id: Option<String>,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    start_frame: Option<u64>,
    end_frame: Option<u64>,
    #[serde(default)]
    overview: Option<bool>,
    word_timestamps: Option<bool>,
    max_frames: Option<usize>,
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateCodexEditProposalArgs {
    request: EditJobRequest,
    proposal: CodexEditProposal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildGenerateMediaStartRequestArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    asset_id: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default)]
    mock_mode: Option<bool>,
    brief: Option<TemporalGenerateMediaBrief>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "folderId")]
    target_folder_id: Option<String>,
    #[serde(default)]
    placement_intent: Option<String>,
    #[serde(default)]
    replacement_item_id: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    model: Option<Value>,
    #[serde(default)]
    references: Option<Value>,
    #[serde(default)]
    settings: Option<Value>,
    #[serde(default, alias = "mediaRef")]
    media_id: Option<String>,
    #[serde(default)]
    source_clip_id: Option<String>,
    #[serde(default, alias = "sourceVideoMediaRef", alias = "videoSourceMediaRef")]
    source_video_media_id: Option<String>,
    #[serde(default)]
    start_frame_media_ref: Option<String>,
    #[serde(default)]
    end_frame_media_ref: Option<String>,
    #[serde(default)]
    reference_media_refs: Vec<String>,
    #[serde(default)]
    reference_image_media_refs: Vec<String>,
    #[serde(default)]
    reference_video_media_refs: Vec<String>,
    #[serde(default)]
    reference_audio_media_refs: Vec<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    aspect_ratio: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    num_images: Option<u32>,
    #[serde(default)]
    generate_audio: Option<bool>,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    voice: Option<String>,
    #[serde(default)]
    lyrics: Option<String>,
    #[serde(default)]
    style_instructions: Option<String>,
    #[serde(default)]
    instrumental: Option<bool>,
    #[serde(default)]
    video_source_start_frame: Option<u64>,
    #[serde(default)]
    video_source_end_frame: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RerunGeneratedAssetArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    asset_id: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default)]
    mock_mode: Option<bool>,
    source_asset_id: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "folderId")]
    target_folder_id: Option<String>,
    #[serde(default)]
    placement_intent: Option<String>,
    #[serde(default)]
    replacement_item_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpscaleMediaArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    asset_id: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default)]
    mock_mode: Option<bool>,
    #[serde(alias = "mediaRef")]
    media_id: String,
    #[serde(default)]
    source_clip_id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider_input_url: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "folderId")]
    target_folder_id: Option<String>,
    #[serde(default)]
    placement_intent: Option<String>,
    #[serde(default)]
    replacement_item_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildCodexEditStartRequestArgs {
    project_root: String,
    project_dir: String,
    job_id: String,
    request: EditJobRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildExportMediaStartRequestArgs {
    project_dir: String,
    job_id: String,
    profile: ExportProfile,
    quality: RenderQuality,
    width: u32,
    height: u32,
    output_path: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ExportProjectProfile {
    DraftWebm,
    FinalWebm,
    Mp4H264,
    Mp4H265,
    ProResMov,
    PremiereXmeml,
    DavinciFcpxml,
    PalmierProject,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportProjectStartRequestArgs {
    #[serde(default)]
    project_dir: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
    #[serde(default)]
    profile: Option<ExportProjectProfile>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    overwrite: Option<bool>,
    #[serde(default)]
    output_path: Option<String>,
    #[serde(default)]
    timeline_id: Option<String>,
    #[serde(default)]
    fcpxml_target: Option<String>,
}

#[derive(Debug)]
struct ResolvedExportProjectStartRequestArgs {
    project_dir: String,
    job_id: String,
    profile: Option<ExportProjectProfile>,
    mode: Option<String>,
    codec: Option<String>,
    resolution: Option<String>,
    overwrite: Option<bool>,
    output_path: Option<String>,
    timeline_id: Option<String>,
    fcpxml_target: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildExportNleXmlStartRequestArgs {
    project_dir: String,
    job_id: String,
    format: NleXmlFormat,
    output_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranscriptionReadinessArgs {
    media_id: String,
    model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranscriptWordsArgs {
    #[serde(default, alias = "mediaRef")]
    media_id: Option<String>,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    start_frame: Option<u64>,
    end_frame: Option<u64>,
    #[serde(default)]
    clip_id: Option<String>,
    #[serde(default)]
    language: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GetTranscriptArgs {
    #[serde(default, alias = "mediaRef")]
    media_id: Option<String>,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    start_frame: Option<u64>,
    end_frame: Option<u64>,
    #[serde(default)]
    clip_id: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    word_timestamps: Option<bool>,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApplyProjectActionsArgs {
    project_dir: String,
    actions: Vec<ProjectAction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UndoAgentEditArgs {
    project_dir: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetActiveTimelineArgs {
    timeline_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateTimelineArgs {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DuplicateTimelineArgs {
    #[serde(default)]
    timeline_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenProjectArgs {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewProjectArgs {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum DirectGenerationTool {
    Video,
    Image,
    Audio,
}

pub fn list_codex_local_tools() -> Vec<CodexLocalToolDescriptor> {
    let mut tools = vec![
        tool_descriptor(
            "video_creater.project_context",
            "Project Context",
            "query",
            "Return project identity, counts, policy, and split-project file hints.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.timeline",
            "Timeline",
            "query",
            "Return bounded timeline tracks, items, ids, timing, sources, and properties.",
            get_timeline_schema(),
        ),
        tool_descriptor(
            "video_creater.get_timeline",
            "Get Timeline",
            "query",
            "Return bounded timeline tracks, items, ids, timing, sources, and properties.",
            get_timeline_schema(),
        ),
        tool_descriptor(
            "video_creater.inspect_timeline",
            "Inspect Timeline",
            "query",
            "Return bounded composited timeline context around a frame, including active clips, overlays, captions, source ranges, and known validation issues.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "startSeconds": { "type": "number", "minimum": 0.0 },
                    "endSeconds": { "type": "number", "minimum": 0.0 },
                    "startFrame": { "type": "integer", "minimum": 0 },
                    "endFrame": { "type": "integer", "minimum": 0 },
                    "maxItems": { "type": "integer", "minimum": 1, "maximum": 50 },
                    "maxFrames": { "type": "integer", "minimum": 1, "maximum": 12 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.inspect_media",
            "Inspect Media",
            "query",
            "Return bounded media metadata, timeline references, transcript availability, and Lottie timeline sampling metadata for a media asset.",
            inspect_media_schema(),
        ),
        tool_descriptor(
            "video_creater.media_library",
            "Media Library",
            "query",
            "Return bounded media folders and media assets.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.get_media",
            "Get Media",
            "query",
            "Return bounded media folders and media assets.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.list_folders",
            "List Folders",
            "query",
            "Return media-library folders with ids and parents.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.get_transcript",
            "Get Transcript",
            "transcription",
            "Return the edited timeline transcript in timeline order, or media-scoped transcript words when mediaId is provided.",
            get_transcript_schema(),
        ),
        tool_descriptor(
            "video_creater.list_models",
            "List Models",
            "query",
            "Return configured generation models and local transcription model catalog entries without provider secrets.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["video", "image", "audio", "upscale", "transcription"]
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.read_skill",
            "Read Skill",
            "query",
            "Load one project-local Video Creater skill body by id from the allowlisted skill store.",
            json!({
                "type": "object",
                "required": ["id"],
                "additionalProperties": false,
                "properties": {
                    "id": {
                        "type": "string",
                        "enum": [
                            "video-creater-graphics",
                            "video-creater-video-pipeline",
                            "video-creater-visuals"
                        ]
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.search_media",
            "Search Media",
            "query",
            "Search media metadata, generated prompts, transcript words, and timeline labels with bounded results.",
            json!({
                "type": "object",
                "required": ["query"],
                "additionalProperties": false,
                "properties": {
                    "query": { "type": "string", "minLength": 1 },
                    "scope": { "type": "string", "enum": ["visual", "spoken", "both", "metadata", "generated"] },
                    "mediaRef": { "type": "string", "minLength": 1 },
                    "mediaId": { "type": "string", "minLength": 1 },
                    "projectDir": { "type": "string", "minLength": 1 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.build_multi_source_edit",
            "Build Multi-source Edit",
            "query",
            "Build a deterministic edit proposal from transcript segments and local semantic visual hits while preserving source media IDs and selection reasons.",
            json!({
                "type": "object",
                "required": ["request"],
                "additionalProperties": false,
                "properties": {
                    "request": { "type": "object" },
                    "semanticHits": { "type": "array", "items": { "type": "object" } }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.list_effects",
            "List Effects",
            "query",
            "Return the Palmier-compatible editable effect catalog for agent-applied visual effects.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_descriptor(
            "video_creater.apply_effect",
            "Apply Effect",
            "mutation",
            "Apply or remove non-color editable effects on visual timeline clips.",
            json!({
                "type": "object",
                "required": ["clipIds"],
                "additionalProperties": false,
                "properties": {
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "effects": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["type"],
                            "additionalProperties": false,
                            "properties": {
                                "type": { "type": "string", "minLength": 1 },
                                "params": { "type": "object" },
                                "enabled": { "type": "boolean" }
                            }
                        }
                    },
                    "remove": {
                        "type": "array",
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.denoise_audio",
            "Denoise Audio",
            "mutation",
            "Enable or disable Palmier-style audio denoise on one or more audio timeline clips.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["clipIds"],
                "additionalProperties": false,
                "properties": {
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "strength": { "type": "number", "minimum": 0, "maximum": 100 },
                    "enabled": { "type": "boolean" }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.apply_color",
            "Apply Color",
            "mutation",
            "Apply or refine a stored editable color grade on visual timeline clips.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["clipIds"],
                "additionalProperties": false,
                "properties": {
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "reset": { "type": "boolean" },
                    "exposure": { "type": "number", "minimum": -3, "maximum": 3 },
                    "contrast": { "type": "number", "minimum": 0.5, "maximum": 1.5 },
                    "saturation": { "type": "number", "minimum": 0, "maximum": 2 },
                    "vibrance": { "type": "number", "minimum": -1, "maximum": 1 },
                    "temperature": { "type": "number", "minimum": 2000, "maximum": 11000 },
                    "tint": { "type": "number", "minimum": -100, "maximum": 100 },
                    "highlights": { "type": "number", "minimum": -1, "maximum": 1 },
                    "shadows": { "type": "number", "minimum": -1, "maximum": 1 },
                    "blacks": { "type": "number", "minimum": -1, "maximum": 1 },
                    "whites": { "type": "number", "minimum": -1, "maximum": 1 },
                    "shadowsHue": { "type": "number", "minimum": 0, "maximum": 360 },
                    "shadowsAmount": { "type": "number", "minimum": 0, "maximum": 1 },
                    "shadowsLum": { "type": "number", "minimum": -0.5, "maximum": 0.5 },
                    "midsHue": { "type": "number", "minimum": 0, "maximum": 360 },
                    "midsAmount": { "type": "number", "minimum": 0, "maximum": 1 },
                    "midsGamma": { "type": "number", "minimum": 0.5, "maximum": 2 },
                    "highsHue": { "type": "number", "minimum": 0, "maximum": 360 },
                    "highsAmount": { "type": "number", "minimum": 0, "maximum": 1 },
                    "highsGain": { "type": "number", "minimum": 0.5, "maximum": 1.5 },
                    "masterCurve": { "type": "array" },
                    "redCurve": { "type": "array" },
                    "greenCurve": { "type": "array" },
                    "blueCurve": { "type": "array" },
                    "hueCurves": { "type": "object" },
                    "lut": { "type": "object" }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.inspect_color",
            "Inspect Color",
            "query",
            "Inspect stored color grade and editable effects for a visual clip or raw media asset.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "clipId": { "type": "string", "minLength": 1 },
                    "projectDir": { "type": "string", "minLength": 1 },
                    "mediaRef": { "type": "string", "minLength": 1 },
                    "mediaId": { "type": "string", "minLength": 1 },
                    "atFrame": { "type": "integer", "minimum": 0 },
                    "reference": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.generation_defaults",
            "Generation Defaults",
            "generation",
            "Return prompt, references, and conservative generation settings.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "selectedMediaId": { "type": "string", "minLength": 1 },
                    "prompt": { "type": "string" }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.generated_assets",
            "Generated Assets",
            "query",
            "Return bounded generated asset provenance and outputs.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.workflow_jobs",
            "Workflow Jobs",
            "query",
            "Return bounded workflow job summaries.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.render_reports",
            "Render Reports",
            "query",
            "Return bounded render-review reports.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.export_artifacts",
            "Export Artifacts",
            "query",
            "Return bounded project export artifacts.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.export_profiles",
            "Export Profiles",
            "export",
            "Return export profile availability and policy status.",
            empty_object_schema(),
        ),
        tool_descriptor(
            "video_creater.transcription_readiness",
            "Transcription Readiness",
            "transcription",
            "Report whether a media item has transcript data ready for edit generation.",
            json!({
                "type": "object",
                "required": ["mediaId"],
                "additionalProperties": false,
                "properties": {
                    "mediaId": { "type": "string", "minLength": 1 },
                    "modelId": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.transcript_words",
            "Transcript Words",
            "transcription",
            "Return bounded transcript words with word indices and source-media timings.",
            transcript_words_schema(),
        ),
        tool_descriptor(
            "video_creater.build_generate_media_start_request",
            "Build Generate Media Start Request",
            "generation",
            "Build a replayable Temporal generate-media start request without starting it.",
            generate_media_schema(),
        ),
        tool_descriptor(
            "video_creater.generate_video",
            "Generate Video",
            "generation",
            "Build a validated generate-media workflow start request for video generation.",
            generate_visual_media_schema(),
        ),
        tool_descriptor(
            "video_creater.generate_image",
            "Generate Image",
            "generation",
            "Build a validated generate-media workflow start request for image generation.",
            generate_visual_media_schema(),
        ),
        tool_descriptor(
            "video_creater.generate_audio",
            "Generate Audio",
            "generation",
            "Build a validated generate-media workflow start request for audio generation.",
            generate_media_schema(),
        ),
        tool_descriptor(
            "video_creater.generate_music",
            "Generate Music",
            "generation",
            "Build a validated audio-generation workflow start request for timeline music.",
            generate_media_schema(),
        ),
        tool_descriptor(
            "video_creater.generate_sfx",
            "Generate SFX",
            "generation",
            "Build a validated audio-generation workflow start request for sound effects.",
            generate_media_schema(),
        ),
        tool_descriptor(
            "video_creater.rerun_generated_asset",
            "Rerun Generated Asset",
            "generation",
            "Queue a new generated-asset variation from an existing generated asset recipe, optionally tweaking the prompt or replacement target.",
            rerun_generated_asset_schema(),
        ),
        tool_descriptor(
            "video_creater.upscale_media",
            "Upscale Media",
            "generation",
            "Build a validated generate-media workflow start request for provider-backed media upscale jobs.",
            upscale_media_schema(),
        ),
        tool_descriptor(
            "video_creater.build_codex_edit_start_request",
            "Build Codex Edit Start Request",
            "generation",
            "Build a replayable Temporal Codex edit start request without starting it.",
            json!({
                "type": "object",
                "required": ["projectRoot", "projectDir", "jobId", "request"],
                "additionalProperties": false,
                "properties": {
                    "projectRoot": { "type": "string", "minLength": 1 },
                    "projectDir": { "type": "string", "minLength": 1 },
                    "jobId": { "type": "string", "minLength": 1 },
                    "request": { "type": "object" }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.build_export_media_start_request",
            "Build Export Media Start Request",
            "export",
            "Build a replayable Temporal export-media start request without starting it.",
            json!({
                "type": "object",
                "required": ["projectDir", "jobId", "profile", "quality", "width", "height", "outputPath"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "jobId": { "type": "string", "minLength": 1 },
                    "profile": { "type": "string", "enum": ["mp4H264", "mp4H265", "proResMov"] },
                    "quality": { "type": "string", "enum": ["draft", "final"] },
                    "width": { "type": "integer", "minimum": 2, "maximum": 16384, "multipleOf": 2 },
                    "height": { "type": "integer", "minimum": 2, "maximum": 16384, "multipleOf": 2 },
                    "outputPath": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.export_project",
            "Export Project",
            "export",
            "Build a validated WebM render, media export, or NLE XML workflow start request for the current project.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "jobId": { "type": "string", "minLength": 1 },
                    "profile": {
                        "type": "string",
                        "enum": [
                            "draftWebm",
                            "finalWebm",
                            "mp4H264",
                            "mp4H265",
                            "proResMov",
                            "premiereXmeml",
                            "davinciFcpxml",
                            "palmierProject"
                        ]
                    },
                    "mode": { "type": "string", "enum": ["video", "xml", "fcpxml", "webm", "draftWebm", "palmier", "project", "projectBundle"] },
                    "codec": { "type": "string", "enum": ["H.264", "H.265", "HEVC", "ProRes", "WebM"] },
                    "resolution": { "type": "string", "enum": ["720p", "1080p", "2K", "4K", "Match Timeline"] },
                    "overwrite": { "type": "boolean" },
                    "outputPath": { "type": "string", "minLength": 1 },
                    "timelineId": { "type": "string", "minLength": 1 },
                    "fcpxmlTarget": { "type": "string", "enum": ["resolve", "fcp"] }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.build_export_nle_xml_start_request",
            "Build Export NLE XML Start Request",
            "export",
            "Build a replayable Temporal NLE XML export start request without starting it.",
            json!({
                "type": "object",
                "required": ["projectDir", "jobId", "format", "outputPath"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "jobId": { "type": "string", "minLength": 1 },
                    "format": { "type": "string", "enum": ["premiereXmeml", "davinciFcpxml"] },
                    "outputPath": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.validate_project_actions",
            "Validate Project Actions",
            "edit_validation",
            "Validate projectActions against a cloned project without mutating canonical state.",
            json!({
                "type": "object",
                "required": ["actions"],
                "additionalProperties": false,
                "properties": {
                    "actions": { "type": "array", "items": { "type": "object" } }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.add_clips",
            "Add Clips",
            "mutation",
            "Validate and apply direct media-backed timeline clip additions.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["targetTrackId", "clips"] },
                    { "required": ["entries"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "targetTrackId": { "type": "string", "minLength": 1 },
                    "clips": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["id", "mediaId", "startSeconds", "durationSeconds"],
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string", "minLength": 1 },
                                "mediaId": { "type": "string", "minLength": 1 },
                                "startSeconds": { "type": "number", "minimum": 0.0 },
                                "durationSeconds": { "type": "number", "exclusiveMinimum": 0.0 },
                                "sourceIn": { "type": "number", "minimum": 0.0 },
                                "sourceOut": { "type": "number", "exclusiveMinimum": 0.0 },
                                "label": { "type": "string", "minLength": 1 },
                                "opacity": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                            }
                        }
                    },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["startFrame", "durationFrames"],
                            "anyOf": [
                                { "required": ["mediaId"] },
                                { "required": ["mediaRef"] }
                            ],
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string", "minLength": 1 },
                                "mediaId": { "type": "string", "minLength": 1 },
                                "mediaRef": { "type": "string", "minLength": 1 },
                                "trackIndex": { "type": "integer", "minimum": 0 },
                                "startFrame": { "type": "integer", "minimum": 0 },
                                "durationFrames": { "type": "integer", "minimum": 1 },
                                "trimStartFrame": { "type": "integer", "minimum": 0 },
                                "trimEndFrame": { "type": "integer", "minimum": 0 },
                                "label": { "type": "string", "minLength": 1 },
                                "opacity": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.insert_clips",
            "Insert Clips",
            "mutation",
            "Validate and apply direct ripple timeline media clip insertions.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["targetTrackId", "insertSeconds", "clips"] },
                    { "required": ["trackIndex", "atFrame", "entries"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "targetTrackId": { "type": "string", "minLength": 1 },
                    "insertSeconds": { "type": "number", "minimum": 0.0 },
                    "clips": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["id", "mediaId", "durationSeconds"],
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string", "minLength": 1 },
                                "mediaId": { "type": "string", "minLength": 1 },
                                "durationSeconds": { "type": "number", "exclusiveMinimum": 0.0 },
                                "sourceIn": { "type": "number", "minimum": 0.0 },
                                "sourceOut": { "type": "number", "exclusiveMinimum": 0.0 },
                                "label": { "type": "string", "minLength": 1 },
                                "opacity": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                            }
                        }
                    },
                    "trackIndex": { "type": "integer", "minimum": 0 },
                    "atFrame": { "type": "integer", "minimum": 0 },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "anyOf": [
                                { "required": ["mediaId"] },
                                { "required": ["mediaRef"] }
                            ],
                            "additionalProperties": false,
                            "properties": {
                                "mediaId": { "type": "string", "minLength": 1 },
                                "mediaRef": { "type": "string", "minLength": 1 },
                                "durationFrames": { "type": "integer", "minimum": 1 },
                                "trimStartFrame": { "type": "integer", "minimum": 0 },
                                "trimEndFrame": { "type": "integer", "minimum": 0 }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.remove_clips",
            "Remove Clips",
            "mutation",
            "Validate source-backed timeline removals from direct item ids.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["itemIds"] },
                    { "required": ["clipIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "itemIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.remove_tracks",
            "Remove Tracks",
            "mutation",
            "Remove timeline tracks by 0-based indexes from get_timeline, including their contained items.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["trackIndexes"] },
                    { "required": ["trackIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "trackIndexes": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "integer", "minimum": 0 }
                    },
                    "trackIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.move_clips",
            "Move Clips",
            "mutation",
            "Validate timeline moves from direct item ids, target tracks, and start seconds.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["moves"] },
                    { "required": ["projectDir", "actions"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "actions": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "object" }
                    },
                    "moves": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "anyOf": [
                                { "required": ["itemId"] },
                                { "required": ["clipId"] }
                            ],
                            "additionalProperties": false,
                            "properties": {
                                "itemId": { "type": "string", "minLength": 1 },
                                "clipId": { "type": "string", "minLength": 1 },
                                "targetTrackId": { "type": "string", "minLength": 1 },
                                "startSeconds": { "type": "number", "minimum": 0.0 },
                                "toTrack": { "type": "integer", "minimum": 0 },
                                "toFrame": { "type": "integer", "minimum": 0 }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.apply_layout",
            "Apply Layout",
            "mutation",
            "Arrange visual clips or media refs into Palmier-style split-screen, PIP, grid, sidebar, or three-up layouts.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["layout", "slots"],
                "additionalProperties": false,
                "properties": {
                    "layout": {
                        "type": "string",
                        "enum": [
                            "full",
                            "side_by_side",
                            "top_bottom",
                            "pip_bottom_right",
                            "pip_bottom_left",
                            "pip_top_right",
                            "pip_top_left",
                            "grid_2x2",
                            "main_sidebar",
                            "three_up"
                        ]
                    },
                    "fit": { "type": "string", "enum": ["fill", "fit"] },
                    "startFrame": { "type": "integer", "minimum": 0 },
                    "durationFrames": { "type": "integer", "minimum": 1 },
                    "slots": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["slot"],
                            "additionalProperties": false,
                            "properties": {
                                "slot": { "type": "string", "minLength": 1 },
                                "mediaRef": { "type": "string", "minLength": 1 },
                                "mediaId": { "type": "string", "minLength": 1 },
                                "clipIds": {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": { "type": "string", "minLength": 1 }
                                },
                                "anchor": {
                                    "type": "string",
                                    "enum": [
                                        "center",
                                        "top",
                                        "bottom",
                                        "left",
                                        "right",
                                        "top_left",
                                        "top_right",
                                        "bottom_left",
                                        "bottom_right"
                                    ]
                                },
                                "anchorX": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                "anchorY": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.link_clips",
            "Link Clips",
            "mutation",
            "Assign a shared linkGroupId to two or more timeline clips so future linked edits keep A/V partners together.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["clipIds"] },
                    { "required": ["itemIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "clipIds": {
                        "type": "array",
                        "minItems": 2,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "itemIds": {
                        "type": "array",
                        "minItems": 2,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "linkGroupId": { "type": "string", "minLength": 1 }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.unlink_clips",
            "Unlink Clips",
            "mutation",
            "Clear linkGroupId from every clip in the link groups touched by the supplied timeline clip ids.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["clipIds"] },
                    { "required": ["itemIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "itemIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.split_clips",
            "Split Clips",
            "mutation",
            "Validate timeline clip splits from direct item ids and split seconds.",
            json!({
                "type": "object",
                "anyOf": [
                    { "required": ["splits"] },
                    { "required": ["trackIndex", "frames"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "splits": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "anyOf": [
                                { "required": ["itemId"] },
                                { "required": ["clipId"] }
                            ],
                            "additionalProperties": false,
                            "properties": {
                                "itemId": { "type": "string", "minLength": 1 },
                                "clipId": { "type": "string", "minLength": 1 },
                                "splitSeconds": { "type": "number", "minimum": 0.0 },
                                "atFrame": { "type": "integer", "minimum": 0 },
                                "newItemId": { "type": "string", "minLength": 1 }
                            }
                        }
                    },
                    "clipId": { "type": "string", "minLength": 1 },
                    "trackIndex": { "type": "integer", "minimum": 0 },
                    "frames": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "integer", "minimum": 0 }
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.ripple_delete_ranges",
            "Ripple Delete Ranges",
            "mutation",
            "Validate and apply direct timeline time-range removals that close gaps on target tracks.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["ranges"],
                "additionalProperties": false,
                "properties": {
                    "trackIndex": { "type": "integer", "minimum": 0 },
                    "clipId": { "type": "string", "minLength": 1 },
                    "itemId": { "type": "string", "minLength": 1 },
                    "units": { "type": "string", "enum": ["frames", "seconds"] },
                    "ranges": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "anyOf": [
                                {
                                    "type": "object",
                                    "required": ["startSeconds", "endSeconds", "trackIds"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "startSeconds": { "type": "number", "minimum": 0.0 },
                                        "endSeconds": { "type": "number", "exclusiveMinimum": 0.0 },
                                        "trackIds": {
                                            "type": "array",
                                            "minItems": 1,
                                            "items": { "type": "string", "minLength": 1 }
                                        }
                                    }
                                },
                                {
                                    "type": "array",
                                    "minItems": 2,
                                    "maxItems": 2,
                                    "items": { "type": "number", "minimum": 0.0 }
                                }
                            ]
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.remove_words",
            "Remove Words",
            "mutation",
            "Map transcript word indexes or Palmier word ranges to source-backed ripple-delete ranges and validate the timeline edit.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["wordIndexes"] },
                    { "required": ["words"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "mediaId": { "type": "string", "minLength": 1 },
                    "wordIndexes": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "integer", "minimum": 0 }
                    },
                    "words": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "anyOf": [
                                { "type": "integer", "minimum": 0 },
                                {
                                    "type": "array",
                                    "minItems": 2,
                                    "maxItems": 2,
                                    "items": { "type": "integer", "minimum": 0 }
                                }
                            ]
                        }
                    },
                    "cutAggressiveness": { "type": "string", "minLength": 1 },
                    "language": { "type": "string", "minLength": 1 }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.remove_silence",
            "Remove Silence",
            "mutation",
            "Remove long speech-free timeline gaps derived from transcript timing, ripple-closing linked clips like Palmier remove_silence.",
            project_actions_tool_schema(json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            })),
        ),
        tool_descriptor(
            "video_creater.set_clip_properties",
            "Set Clip Properties",
            "mutation",
            "Validate and apply direct clip property updates for timing, source range, opacity, text, audio volume, fades, and reverse playback.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["updates"] },
                    { "required": ["clipIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "updates": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["itemId"],
                            "additionalProperties": false,
                            "properties": {
                                "itemId": { "type": "string", "minLength": 1 },
                                "startSeconds": { "type": "number", "minimum": 0.0 },
                                "durationSeconds": { "type": "number", "exclusiveMinimum": 0.0 },
                                "sourceIn": { "type": "number", "minimum": 0.0 },
                                "sourceOut": { "type": "number", "exclusiveMinimum": 0.0 },
                                "opacity": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                "transform": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "centerX": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                        "centerY": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                                        "width": { "type": "number", "exclusiveMinimum": 0.0, "maximum": 1.0 },
                                        "height": { "type": "number", "exclusiveMinimum": 0.0, "maximum": 1.0 },
                                        "flipHorizontal": { "type": "boolean" },
                                        "flipVertical": { "type": "boolean" }
                                    }
                                },
                                "text": { "type": "string", "minLength": 1 },
                                "fontName": { "type": "string", "minLength": 1 },
                                "fontSize": { "type": "number", "exclusiveMinimum": 0.0 },
                                "color": { "type": "string", "minLength": 1 },
                                "alignment": { "type": "string", "minLength": 1 },
                                "blendMode": {
                                    "type": "string",
                                    "enum": PALMIER_BLEND_MODES
                                },
                                "volumeDb": { "type": "number", "minimum": -60.0, "maximum": 24.0 },
                                "fadeOutSeconds": { "type": "number", "minimum": 0.0 },
                                "reverse": { "type": "boolean" }
                            }
                        }
                    },
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "durationFrames": { "type": "integer", "minimum": 1 },
                    "trimStartFrame": { "type": "integer", "minimum": 0 },
                    "trimEndFrame": { "type": "integer", "minimum": 0 },
                    "speed": {
                        "type": "number",
                        "minimum": CLIP_SPEED_RANGE.start(),
                        "maximum": CLIP_SPEED_RANGE.end()
                    },
                    "reverse": { "type": "boolean" },
                    "volume": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    "fadeOutFrames": { "type": "integer", "minimum": 0 },
                    "fadeOutSeconds": { "type": "number", "minimum": 0.0 },
                    "opacity": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    "transform": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "centerX": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                            "centerY": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                            "width": { "type": "number", "exclusiveMinimum": 0.0, "maximum": 1.0 },
                            "height": { "type": "number", "exclusiveMinimum": 0.0, "maximum": 1.0 },
                            "flipHorizontal": { "type": "boolean" },
                            "flipVertical": { "type": "boolean" }
                        }
                    },
                    "content": { "type": "string", "minLength": 1 },
                    "fontName": { "type": "string", "minLength": 1 },
                    "fontSize": { "type": "number", "exclusiveMinimum": 0.0 },
                    "color": { "type": "string", "minLength": 1 },
                    "alignment": { "type": "string", "enum": ["left", "center", "right"] },
                    "blendMode": {
                        "type": "string",
                        "enum": PALMIER_BLEND_MODES
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.set_keyframes",
            "Set Keyframes",
            "mutation",
            "Validate and apply opacity, transform, or audio volume keyframes to a timeline item.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["itemId", "property", "keyframes"] },
                    { "required": ["clipId", "property", "keyframes"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "itemId": { "type": "string", "minLength": 1 },
                    "clipId": { "type": "string", "minLength": 1 },
                    "property": {
                        "type": "string",
                        "enum": [
                            "opacity",
                            "volumeDb",
                            "positionX",
                            "positionY",
                            "scale",
                            "scaleX",
                            "scaleY",
                            "rotationDegrees",
                            "cropTop",
                            "cropRight",
                            "cropBottom",
                            "cropLeft",
                            "position",
                            "crop"
                        ]
                    },
                    "keyframes": {
                        "type": "array",
                        "items": {
                            "anyOf": [
                                {
                                    "type": "object",
                                    "required": ["atSeconds", "value"],
                                    "additionalProperties": false,
                                    "properties": {
                                        "atSeconds": { "type": "number", "minimum": 0.0 },
                                        "value": { "type": "number" },
                                        "easing": { "type": "string", "minLength": 1 }
                                    }
                                },
                                { "type": "array" }
                            ]
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.import_media",
            "Import Media",
            "mutation",
            "Import supported local media files or Palmier-style source payloads into the split project media folder.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "sourcePaths": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "source": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "url": {
                                "type": "string",
                                "description": "HTTPS URL to download and import. Loopback HTTP is accepted only for local tool tests."
                            },
                            "path": {
                                "type": "string",
                                "minLength": 1,
                                "description": "Absolute local file path, or a directory imported recursively with media folders mirroring its subdirectories."
                            },
                            "bytes": {
                                "type": "string",
                                "description": "Base64-encoded inline media data. Requires source.mimeType."
                            },
                            "mimeType": {
                                "type": "string",
                                "description": "Required with source.bytes. Accepted values include video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, audio/aiff, audio/flac, image/png, image/jpeg, image/tiff, and image/heic."
                            }
                        }
                    },
                    "name": {
                        "type": "string",
                        "description": "Display name used when importing inline bytes."
                    },
                    "folderId": {
                        "type": "string",
                        "description": "Existing media folder id assigned to every imported asset."
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.create_matte",
            "Create Matte",
            "mutation",
            "Create a local solid-color PNG matte in the project media library without calling provider APIs.",
            json!({
                "type": "object",
                "required": ["hex"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "hex": {
                        "type": "string",
                        "description": "Hex color, e.g. #000000 or #FFFFFF."
                    },
                    "aspectRatio": {
                        "type": "string",
                        "enum": ["Project", "16:9", "9:16", "1:1", "4:3", "9:14", "2.4:1"],
                        "description": "Defaults to Project. Other values use the project short edge, matching Palmier."
                    },
                    "name": { "type": "string" },
                    "folderId": {
                        "type": "string",
                        "description": "Existing media folder id assigned to the matte."
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.add_texts",
            "Add Texts",
            "mutation",
            "Validate and apply direct text overlay insertions with visual treatment metadata.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["targetTrackId", "texts"] },
                    { "required": ["entries"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "targetTrackId": { "type": "string", "minLength": 1 },
                    "texts": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": [
                                "id",
                                "text",
                                "startSeconds",
                                "durationSeconds",
                                "visualTreatment",
                                "motion",
                                "safeZone",
                                "avoid"
                            ],
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string", "minLength": 1 },
                                "text": { "type": "string", "minLength": 1 },
                                "startSeconds": { "type": "number", "minimum": 0.0 },
                                "durationSeconds": { "type": "number", "exclusiveMinimum": 0.0 },
                                "label": { "type": "string", "minLength": 1 },
                                "visualTreatment": { "type": "string", "minLength": 1 },
                                "motion": { "type": "string", "minLength": 1 },
                                "safeZone": { "type": "string", "minLength": 1 },
                                "avoid": { "type": "string", "minLength": 1 }
                            }
                        }
                    },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["startFrame", "durationFrames", "content"],
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string", "minLength": 1 },
                                "trackIndex": { "type": "integer", "minimum": 0 },
                                "startFrame": { "type": "integer", "minimum": 0 },
                                "durationFrames": { "type": "integer", "minimum": 1 },
                                "content": { "type": "string", "minLength": 1 },
                                "transform": { "type": "object" },
                                "fontName": { "type": "string", "minLength": 1 },
                                "fontSize": { "type": "number", "exclusiveMinimum": 0.0 },
                                "color": { "type": "string", "minLength": 1 },
                                "alignment": { "type": "string", "enum": ["left", "center", "right"] }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.update_text",
            "Update Text",
            "mutation",
            "Update Palmier-style text clips or a caption group with content, typography, color, animation metadata, or text-box transform.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "clipIds": {
                        "type": "array",
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "captionGroupId": { "type": "string", "minLength": 1 },
                    "content": { "type": "string" },
                    "transform": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "centerX": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                            "centerY": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                            "width": { "type": "number", "exclusiveMinimum": 0.0, "maximum": 1.0 },
                            "height": { "type": "number", "exclusiveMinimum": 0.0, "maximum": 1.0 },
                            "flipHorizontal": { "type": "boolean" },
                            "flipVertical": { "type": "boolean" }
                        }
                    },
                    "fontName": { "type": "string", "minLength": 1 },
                    "fontFamily": { "type": "string", "minLength": 1 },
                    "fontSize": { "type": "number", "exclusiveMinimum": 0.0 },
                    "isBold": { "type": "boolean" },
                    "isItalic": { "type": "boolean" },
                    "color": { "type": "string", "minLength": 1 },
                    "alignment": { "type": "string", "enum": ["left", "center", "right"] },
                    "borderColor": { "type": "string", "minLength": 1 },
                    "backgroundColor": { "type": "string", "minLength": 1 },
                    "animation": { "type": "string", "minLength": 1 },
                    "highlightColor": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.add_captions",
            "Add Captions",
            "mutation",
            "Validate and apply direct caption insertions with readable default visual metadata.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["targetTrackId", "captions"] },
                    { "required": ["clipIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "targetTrackId": { "type": "string", "minLength": 1 },
                    "captions": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["id", "text", "startSeconds", "durationSeconds"],
                            "additionalProperties": false,
                            "properties": {
                                "id": { "type": "string", "minLength": 1 },
                                "text": { "type": "string", "minLength": 1 },
                                "startSeconds": { "type": "number", "minimum": 0.0 },
                                "durationSeconds": { "type": "number", "exclusiveMinimum": 0.0 },
                                "label": { "type": "string", "minLength": 1 },
                                "visualTreatment": { "type": "string", "minLength": 1 },
                                "motion": { "type": "string", "minLength": 1 },
                                "safeZone": { "type": "string", "minLength": 1 },
                                "avoid": { "type": "string", "minLength": 1 }
                            }
                        }
                    },
                    "clipIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "language": { "type": "string", "minLength": 1 },
                    "fontName": { "type": "string", "minLength": 1 },
                    "fontSize": { "type": "number", "exclusiveMinimum": 0.0 },
                    "color": { "type": "string", "minLength": 1 },
                    "centerX": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    "centerY": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    "textCase": { "type": "string", "minLength": 1 },
                    "censorProfanity": { "type": "boolean" }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.rename_media",
            "Rename Media",
            "mutation",
            "Rename one media asset with mediaRef/name or multiple assets with entries.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "mediaRef": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["mediaRef", "name"],
                            "additionalProperties": false,
                            "properties": {
                                "mediaRef": { "type": "string", "minLength": 1 },
                                "name": { "type": "string", "minLength": 1 }
                            }
                        }
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.delete_media",
            "Delete Media",
            "mutation",
            "Delete media assets by id and remove timeline items that reference them.",
            json!({
                "type": "object",
                "anyOf": [
                    { "required": ["assetIds"] },
                    { "required": ["mediaIds"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "assetIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "mediaIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.create_folder",
            "Create Folder",
            "mutation",
            "Create one folder with name/parentFolderId or multiple folders with entries. folderId is optional and generated when omitted.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["name"] },
                    { "required": ["entries"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "folderId": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "parentId": { "type": "string", "minLength": 1 },
                    "parentFolderId": { "type": "string", "minLength": 1 },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["name"],
                            "additionalProperties": false,
                            "properties": {
                                "folderId": { "type": "string", "minLength": 1 },
                                "name": { "type": "string", "minLength": 1 },
                                "parentId": { "type": "string", "minLength": 1 },
                                "parentFolderId": { "type": "string", "minLength": 1 }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.move_to_folder",
            "Move To Folder",
            "mutation",
            "Move media assets to folders. Pass either assetIds/mediaIds plus folderId for one destination or entries for multiple destinations.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["assetIds"] },
                    { "required": ["mediaIds"] },
                    { "required": ["entries"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "assetIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "mediaIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "folderId": {
                        "anyOf": [
                            { "type": "string", "minLength": 1 },
                            { "type": "null" }
                        ]
                    },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "anyOf": [
                                { "required": ["assetIds"] },
                                { "required": ["mediaIds"] }
                            ],
                            "additionalProperties": false,
                            "properties": {
                                "assetIds": {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": { "type": "string", "minLength": 1 }
                                },
                                "mediaIds": {
                                    "type": "array",
                                    "minItems": 1,
                                    "items": { "type": "string", "minLength": 1 }
                                },
                                "folderId": {
                                    "anyOf": [
                                        { "type": "string", "minLength": 1 },
                                        { "type": "null" }
                                    ]
                                }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.rename_folder",
            "Rename Folder",
            "mutation",
            "Rename one folder with folderId/name or multiple folders with entries.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["folderId", "name"] },
                    { "required": ["entries"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "folderId": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "entries": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["folderId", "name"],
                            "additionalProperties": false,
                            "properties": {
                                "folderId": { "type": "string", "minLength": 1 },
                                "name": { "type": "string", "minLength": 1 }
                            }
                        }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.delete_folder",
            "Delete Folder",
            "mutation",
            "Validate and apply direct media-folder deletion.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["folderIds"],
                "additionalProperties": false,
                "properties": {
                    "folderIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.set_project_settings",
            "Set Project Settings",
            "mutation",
            "Update project render settings from partial fps, resolution, aspect, quality, loudness, or caption inputs.",
            project_actions_tool_schema(json!({
                "type": "object",
                "anyOf": [
                    { "required": ["fps"] },
                    { "required": ["width"] },
                    { "required": ["height"] },
                    { "required": ["aspectRatio"] },
                    { "required": ["quality"] },
                    { "required": ["loudnessLufs"] },
                    { "required": ["captions"] }
                ],
                "additionalProperties": false,
                "properties": {
                    "fps": { "type": "number", "exclusiveMinimum": 0 },
                    "width": { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 },
                    "aspectRatio": {
                        "type": "string",
                        "enum": ["16:9", "9:16", "1:1", "4:3", "2.4:1", "9:14"]
                    },
                    "quality": {
                        "type": "string",
                        "enum": ["720p", "1080p", "2K", "4K"]
                    },
                    "loudnessLufs": { "type": "number" },
                    "captions": { "type": "string", "enum": ["burn_in", "mux", "off"] }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.create_timeline",
            "Create Timeline",
            "edit_mutation",
            "Create a named alternate timeline through a validated canonical project action. The new timeline becomes active.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.set_active_timeline",
            "Set Active Timeline",
            "edit_mutation",
            "Switch the active timeline through a validated canonical project action.",
            json!({
                "type": "object",
                "required": ["timelineId"],
                "additionalProperties": false,
                "properties": {
                    "timelineId": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.duplicate_timeline",
            "Duplicate Timeline",
            "edit_mutation",
            "Duplicate the active timeline through a validated canonical project action. Switch first when duplicating another timeline.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "timelineId": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.sync_audio",
            "Sync Audio",
            "edit_mutation",
            "Align target timeline clips to a reference clip by waveform correlation and return validated moveItems projectActions.",
            project_actions_tool_schema(json!({
                "type": "object",
                "required": ["referenceClipId"],
                "additionalProperties": false,
                "properties": {
                    "referenceClipId": { "type": "string", "minLength": 1 },
                    "targetClipId": { "type": "string", "minLength": 1 },
                    "targetClipIds": {
                        "type": "array",
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "projectDir": {
                        "type": "string",
                        "description": "Optional split project folder used to decode media audio when clips do not already carry waveformPeaks."
                    },
                    "searchWindowSeconds": { "type": "number", "exclusiveMinimum": 0 },
                    "minConfidence": { "type": "number", "minimum": 0, "maximum": 1 }
                }
            })),
        ),
        tool_descriptor(
            "video_creater.send_feedback",
            "Send Feedback",
            "query",
            "Validate and prepare a sanitized agent feedback report for app-owned delivery.",
            json!({
                "type": "object",
                "required": ["category", "summary"],
                "additionalProperties": false,
                "properties": {
                    "category": {
                        "type": "string",
                        "enum": [
                            "missing_capability",
                            "wrong_result",
                            "confusing_ux",
                            "failure",
                            "suggestion"
                        ]
                    },
                    "summary": { "type": "string", "minLength": 1 },
                    "details": { "type": "string" },
                    "severity": {
                        "type": "string",
                        "enum": ["low", "medium", "high"]
                    }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.validate_codex_edit_proposal",
            "Validate Codex Edit Proposal",
            "edit_validation",
            "Validate a Codex edit proposal, including its EDL and projectActions.",
            json!({
                "type": "object",
                "required": ["request", "proposal"],
                "additionalProperties": false,
                "properties": {
                    "request": { "type": "object" },
                    "proposal": { "type": "object" }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.apply_project_actions",
            "Apply Project Actions",
            "edit_mutation",
            "Apply projectActions to a split project through Rust validation and writers.",
            json!({
                "type": "object",
                "required": ["projectDir", "actions"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 },
                    "actions": { "type": "array", "items": { "type": "object" } }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.undo_agent_edit",
            "Undo Agent Edit",
            "edit_mutation",
            "Undo only the latest agent-authored project action batch when the project still matches that batch.",
            json!({
                "type": "object",
                "required": ["projectDir"],
                "additionalProperties": false,
                "properties": {
                    "projectDir": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.get_projects",
            "Get Projects",
            "query",
            "Palmier-compatible project registry entry point. Local Codex tools operate on the already loaded project, so app-level project navigation is reported as unavailable here.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        tool_descriptor(
            "video_creater.open_project",
            "Open Project",
            "query",
            "Palmier-compatible project navigation entry point. Local Codex tools operate on the already loaded project, so opening projects is handled by the app shell.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "path": { "type": "string", "minLength": 1 }
                }
            }),
        ),
        tool_descriptor(
            "video_creater.new_project",
            "New Project",
            "query",
            "Palmier-compatible project navigation entry point. Local Codex tools operate on the already loaded project, so creating projects is handled by the app shell.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "name": { "type": "string", "minLength": 1 }
                }
            }),
        ),
    ];
    tools.extend(transitions::transition_tool_descriptors());
    tools.extend(audio_edits::audio_edit_tool_descriptors());
    add_palmier_alias_tool_descriptors(&mut tools);
    tools
}

pub fn call_codex_local_tool(
    project: &VideoProject,
    tool_name: &str,
    args: Value,
) -> Result<CodexLocalToolCallResult, CodexLocalToolError> {
    let canonical_tool_name = canonical_codex_tool_name(tool_name);
    let args = expand_codex_id_prefixes(project, args)?;
    let (payload, mutates_project) = match canonical_tool_name.as_str() {
        "video_creater.project_context" => {
            let args: ProjectContextArgs = decode_args(args)?;
            (project_context_payload(project, args.project_dir), false)
        }
        "video_creater.timeline" | "video_creater.get_timeline" => {
            let args: GetTimelineArgs = decode_args(args)?;
            (timeline_payload(project, &args)?, false)
        }
        "video_creater.inspect_timeline" => {
            let args: InspectTimelineArgs = decode_args(args)?;
            (inspect_timeline_payload(project, &args)?, false)
        }
        "video_creater.inspect_media" => {
            let args: InspectMediaArgs = decode_args(args)?;
            (inspect_media_payload(project, &args)?, false)
        }
        "video_creater.media_library" | "video_creater.get_media" => {
            (media_library_payload(project), false)
        }
        "video_creater.list_folders" => (folders_payload(project), false),
        "video_creater.list_models" => {
            let args: ListModelsArgs = decode_args(args)?;
            (list_models_payload(args.model_type.as_deref())?, false)
        }
        "video_creater.read_skill" => {
            let args: ReadSkillArgs = decode_args(args)?;
            (read_skill_payload(args)?, false)
        }
        "video_creater.search_media" => {
            let args: SearchMediaArgs = decode_args(args)?;
            (search_media_payload(project, &args)?, false)
        }
        "video_creater.build_multi_source_edit" => {
            let args: BuildMultiSourceEditArgs = decode_args(args)?;
            (build_multi_source_edit_payload(project, args)?, false)
        }
        "video_creater.list_effects" => (list_effects_payload(), false),
        "video_creater.apply_effect" => {
            let args: ApplyEffectArgs = decode_args(args)?;
            (apply_effect_payload(project, args)?, true)
        }
        "video_creater.denoise_audio" => {
            let args: DenoiseAudioArgs = decode_args(args)?;
            (denoise_audio_payload(project, args)?, true)
        }
        "video_creater.apply_color" => {
            let args: ApplyColorArgs = decode_args(args)?;
            (apply_color_payload(project, args)?, true)
        }
        "video_creater.inspect_color" => {
            let args: InspectColorArgs = decode_args(args)?;
            (inspect_color_payload(project, args)?, false)
        }
        "video_creater.generation_defaults" => {
            let args: GenerationDefaultsArgs = decode_args(args)?;
            (generation_defaults_payload(project, &args), false)
        }
        "video_creater.generated_assets" => (generated_assets_payload(project), false),
        "video_creater.workflow_jobs" => (workflow_jobs_payload(project), false),
        "video_creater.render_reports" => (render_reports_payload(project), false),
        "video_creater.export_artifacts" => (export_artifacts_payload(project), false),
        "video_creater.export_profiles" => (export_profiles_payload(), false),
        "video_creater.transcription_readiness" => {
            let args: TranscriptionReadinessArgs = decode_args(args)?;
            (transcription_readiness_payload(project, &args), false)
        }
        "video_creater.transcript_words" => {
            let args: TranscriptWordsArgs = decode_args(args)?;
            if args.media_id.is_none() {
                return Err(CodexLocalToolError::InvalidArguments(
                    "mediaId is required for transcript_words; use get_transcript for the edited timeline transcript".to_string(),
                ));
            }
            (transcript_words_payload(project, &args)?, false)
        }
        "video_creater.get_transcript" => {
            let args: GetTranscriptArgs = decode_args(args)?;
            (get_transcript_payload(project, &args)?, false)
        }
        "video_creater.build_generate_media_start_request" => {
            let args: BuildGenerateMediaStartRequestArgs = decode_args(args)?;
            (
                build_generate_media_start_request_payload(project, args, None)?,
                false,
            )
        }
        "video_creater.generate_video" => {
            let args: BuildGenerateMediaStartRequestArgs = decode_args(args)?;
            direct_generation_tool_payload(project, args, DirectGenerationTool::Video)?
        }
        "video_creater.generate_image" => {
            let args: BuildGenerateMediaStartRequestArgs = decode_args(args)?;
            direct_generation_tool_payload(project, args, DirectGenerationTool::Image)?
        }
        "video_creater.generate_audio" => {
            let args: BuildGenerateMediaStartRequestArgs = decode_args(args)?;
            direct_generation_tool_payload(project, args, DirectGenerationTool::Audio)?
        }
        "video_creater.upscale_media" => {
            let args: UpscaleMediaArgs = decode_args(args)?;
            generated_media_upscale_tool_payload(project, args)?
        }
        "video_creater.generate_music" => {
            let args: BuildGenerateMediaStartRequestArgs = decode_args(args)?;
            audio_category_generation_tool_payload(project, args, "music")?
        }
        "video_creater.generate_sfx" => {
            let args: BuildGenerateMediaStartRequestArgs = decode_args(args)?;
            audio_category_generation_tool_payload(project, args, "sfx")?
        }
        "video_creater.rerun_generated_asset" => {
            let args: RerunGeneratedAssetArgs = decode_args(args)?;
            generated_media_rerun_tool_payload(project, args)?
        }
        "video_creater.build_codex_edit_start_request" => {
            let args: BuildCodexEditStartRequestArgs = decode_args(args)?;
            (build_codex_edit_start_request_payload(project, args), false)
        }
        "video_creater.build_export_media_start_request" => {
            let args: BuildExportMediaStartRequestArgs = decode_args(args)?;
            (
                build_export_media_start_request_payload(project, args)?,
                false,
            )
        }
        "video_creater.export_project" => {
            let args: ExportProjectStartRequestArgs = decode_args(args)?;
            (export_project_start_request_payload(project, args)?, false)
        }
        "video_creater.build_export_nle_xml_start_request" => {
            let args: BuildExportNleXmlStartRequestArgs = decode_args(args)?;
            (
                build_export_nle_xml_start_request_payload(project, args)?,
                false,
            )
        }
        "video_creater.validate_project_actions" => {
            let args: ValidateProjectActionsArgs = decode_args(args)?;
            (
                validate_project_actions_payload(project, args.actions)?,
                false,
            )
        }
        "video_creater.remove_clips" => {
            let args: RemoveClipsArgs = decode_args(args)?;
            (remove_clips_payload(project, args)?, true)
        }
        "video_creater.move_clips" => {
            let args: MoveClipsArgs = decode_args(args)?;
            (move_clips_payload(project, args)?, true)
        }
        "video_creater.apply_layout" => {
            let args: ApplyLayoutArgs = decode_args(args)?;
            (apply_layout_payload(project, args)?, true)
        }
        "video_creater.link_clips" => {
            let args: LinkClipsArgs = decode_args(args)?;
            (link_clips_payload(project, args)?, true)
        }
        "video_creater.add_transition"
        | "video_creater.update_transition"
        | "video_creater.remove_transition" => (
            transitions::call_transition_tool(project, &canonical_tool_name, args)?,
            true,
        ),
        "video_creater.detach_audio" => (
            audio_edits::call_audio_edit_tool(project, &canonical_tool_name, args)?,
            true,
        ),
        "video_creater.unlink_clips" => {
            let args: UnlinkClipsArgs = decode_args(args)?;
            (unlink_clips_payload(project, args)?, true)
        }
        "video_creater.split_clips" => {
            let args: SplitClipsArgs = decode_args(args)?;
            (split_clips_payload(project, args)?, true)
        }
        "video_creater.set_clip_properties" => {
            let args: SetClipPropertiesArgs = decode_args(args)?;
            (set_clip_properties_payload(project, args)?, true)
        }
        "video_creater.add_clips" => {
            let args: AddClipsArgs = decode_args(args)?;
            (add_clips_payload(project, args)?, true)
        }
        "video_creater.insert_clips" => {
            let args: InsertClipsArgs = decode_args(args)?;
            (insert_clips_payload(project, args)?, true)
        }
        "video_creater.remove_words" => {
            let args: RemoveWordsArgs = decode_args(args)?;
            (remove_words_payload(project, args)?, true)
        }
        "video_creater.remove_silence" => {
            let args: RemoveSilenceArgs = decode_args(args)?;
            (remove_silence_payload(project, args)?, true)
        }
        "video_creater.ripple_delete_ranges" => {
            let args: RippleDeleteRangesArgs = decode_args(args)?;
            (ripple_delete_ranges_payload(project, args)?, true)
        }
        "video_creater.add_texts" => {
            let args: AddTextsArgs = decode_args(args)?;
            (add_texts_payload(project, args)?, true)
        }
        "video_creater.update_text" => {
            let args: UpdateTextArgs = decode_args(args)?;
            (update_text_payload(project, args)?, true)
        }
        "video_creater.add_captions" => {
            let args: AddCaptionsArgs = decode_args(args)?;
            (add_captions_payload(project, args)?, true)
        }
        "video_creater.set_keyframes" => {
            let args: SetKeyframesArgs = decode_args(args)?;
            (set_keyframes_payload(project, args)?, true)
        }
        "video_creater.create_folder" => {
            let args: CreateFolderArgs = decode_args(args)?;
            (create_folder_payload(project, args)?, true)
        }
        "video_creater.move_to_folder" => {
            let args: MoveToFolderArgs = decode_args(args)?;
            (move_to_folder_payload(project, args)?, true)
        }
        "video_creater.rename_folder" => {
            let args: RenameFolderArgs = decode_args(args)?;
            (rename_folder_payload(project, args)?, true)
        }
        "video_creater.delete_folder" => {
            let args: DeleteFolderArgs = decode_args(args)?;
            (delete_folder_payload(project, args)?, true)
        }
        "video_creater.remove_tracks" => {
            let args: RemoveTracksArgs = decode_args(args)?;
            (remove_tracks_payload(project, args)?, true)
        }
        "video_creater.import_media" => {
            let args: ImportMediaArgs = decode_args(args)?;
            (import_media_payload(project, args)?, true)
        }
        "video_creater.create_matte" => {
            let args: CreateMatteArgs = decode_args(args)?;
            (create_matte_payload(project, args)?, true)
        }
        "video_creater.rename_media" => {
            let args: RenameMediaArgs = decode_args(args)?;
            (rename_media_payload(project, args)?, true)
        }
        "video_creater.delete_media" => {
            let args: DeleteMediaArgs = decode_args(args)?;
            (delete_media_payload(project, args)?, true)
        }
        "video_creater.set_project_settings" => {
            let args: SetProjectSettingsArgs = decode_args(args)?;
            (set_project_settings_payload(project, args)?, true)
        }
        "video_creater.create_timeline" => {
            let args: CreateTimelineArgs = decode_args(args)?;
            (create_timeline_payload(project, args)?, true)
        }
        "video_creater.set_active_timeline" => {
            let args: SetActiveTimelineArgs = decode_args(args)?;
            (set_active_timeline_payload(project, args)?, true)
        }
        "video_creater.duplicate_timeline" => {
            let args: DuplicateTimelineArgs = decode_args(args)?;
            (duplicate_timeline_payload(project, args)?, true)
        }
        "video_creater.sync_audio" => {
            let args: SyncAudioArgs = decode_args(args)?;
            (sync_audio_payload(project, args)?, true)
        }
        "video_creater.send_feedback" => {
            let args: SendFeedbackArgs = decode_args(args)?;
            (send_feedback_payload(project, args)?, false)
        }
        "video_creater.validate_codex_edit_proposal" => {
            let args: ValidateCodexEditProposalArgs = decode_args(args)?;
            (
                validate_codex_edit_proposal_payload(project, args.request, args.proposal)?,
                false,
            )
        }
        "video_creater.apply_project_actions" => {
            let args: ApplyProjectActionsArgs = decode_args(args)?;
            (apply_project_actions_payload(args)?, true)
        }
        "video_creater.undo_agent_edit" => {
            let args: UndoAgentEditArgs = decode_args(args)?;
            (undo_agent_edit_payload(args)?, true)
        }
        "video_creater.get_projects" => (get_projects_payload()?, false),
        "video_creater.open_project" => {
            let args: OpenProjectArgs = decode_args(args)?;
            (open_project_payload(args)?, false)
        }
        "video_creater.new_project" => {
            let args: NewProjectArgs = decode_args(args)?;
            (new_project_payload(args)?, false)
        }
        _ => return Err(CodexLocalToolError::UnknownTool(tool_name.to_string())),
    };

    Ok(CodexLocalToolCallResult {
        tool_name: tool_name.to_string(),
        mutates_project,
        payload,
    })
}

fn canonical_codex_tool_name(tool_name: &str) -> String {
    if tool_name.starts_with("video_creater.") {
        return tool_name.to_string();
    }
    if tool_name == "undo" {
        return "video_creater.undo_agent_edit".to_string();
    }

    format!("video_creater.{tool_name}")
}

pub fn resolve_codex_tool_name(tool_name: &str) -> Option<String> {
    let canonical = canonical_codex_tool_name(tool_name);
    list_codex_local_tools()
        .iter()
        .any(|tool| tool.name == canonical)
        .then_some(canonical)
}

fn expand_codex_id_prefixes(
    project: &VideoProject,
    args: Value,
) -> Result<Value, CodexLocalToolError> {
    let ids = codex_project_id_index(project);
    expand_known_id_prefix_fields(args, &ids)
}

fn codex_project_id_index(project: &VideoProject) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();

    for media in &project.media {
        ids.insert(media.id.clone());
    }
    for folder in &project.media_folders {
        ids.insert(folder.id.clone());
    }
    for asset in &project.generated_assets {
        ids.insert(asset.id.clone());
        for output in &asset.outputs {
            ids.insert(output.media_id.clone());
        }
    }
    for transcript in &project.transcripts {
        ids.insert(transcript.id.clone());
    }
    for track in &project.timeline.tracks {
        ids.insert(track.id.clone());
        for item in &track.items {
            ids.insert(item.id.clone());
            match &item.source {
                TimelineSource::Media { media_id } => {
                    ids.insert(media_id.clone());
                }
                TimelineSource::Generated { artifact_id } => {
                    ids.insert(artifact_id.clone());
                }
                TimelineSource::Timeline { timeline_id } => {
                    ids.insert(timeline_id.clone());
                }
                TimelineSource::Text { .. } => {}
            }
        }
    }

    ids
}

fn expand_known_id_prefix_fields(
    value: Value,
    ids: &BTreeSet<String>,
) -> Result<Value, CodexLocalToolError> {
    match value {
        Value::Object(mut object) => {
            for (field, field_value) in object.iter_mut() {
                if is_codex_scalar_id_field(field) {
                    if let Some(id) = field_value.as_str() {
                        *field_value = Value::String(resolve_codex_id_prefix(id, ids)?);
                        continue;
                    }
                }
                if is_codex_array_id_field(field) {
                    if let Some(values) = field_value.as_array_mut() {
                        for value in values {
                            if let Some(id) = value.as_str() {
                                *value = Value::String(resolve_codex_id_prefix(id, ids)?);
                            }
                        }
                        continue;
                    }
                }

                let nested_value = std::mem::take(field_value);
                *field_value = expand_known_id_prefix_fields(nested_value, ids)?;
            }
            Ok(Value::Object(object))
        }
        Value::Array(values) => values
            .into_iter()
            .map(|value| expand_known_id_prefix_fields(value, ids))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        value => Ok(value),
    }
}

fn resolve_codex_id_prefix(
    candidate: &str,
    ids: &BTreeSet<String>,
) -> Result<String, CodexLocalToolError> {
    if ids.contains(candidate) || candidate.len() < 8 {
        return Ok(candidate.to_string());
    }

    let matches = ids
        .iter()
        .filter(|id| id.starts_with(candidate))
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [id] => Ok(id.clone()),
        [] => Ok(candidate.to_string()),
        _ => Err(CodexLocalToolError::InvalidArguments(format!(
            "ambiguous id prefix `{candidate}` matches multiple project ids; re-run video_creater.get_timeline or video_creater.get_media and use a longer prefix"
        ))),
    }
}

fn is_codex_scalar_id_field(field: &str) -> bool {
    matches!(
        field,
        "clipId"
            | "sourceClipId"
            | "referenceClipId"
            | "targetClipId"
            | "itemId"
            | "replacementItemId"
            | "trackId"
            | "targetTrackId"
            | "mediaRef"
            | "mediaId"
            | "sourceMediaId"
            | "sourceVideoMediaRef"
            | "videoSourceMediaRef"
            | "firstFrameMediaRef"
            | "lastFrameMediaRef"
            | "startFrameMediaRef"
            | "endFrameMediaRef"
            | "referenceMediaRef"
            | "referenceImageMediaRef"
            | "referenceVideoMediaRef"
            | "referenceAudioMediaRef"
            | "folderId"
            | "parentFolderId"
            | "parentId"
            | "targetFolderId"
            | "transcriptId"
    )
}

fn is_codex_array_id_field(field: &str) -> bool {
    matches!(
        field,
        "clipIds"
            | "itemIds"
            | "targetClipIds"
            | "targetItemIds"
            | "trackIds"
            | "mediaIds"
            | "assetIds"
            | "folderIds"
            | "referenceMediaRefs"
            | "referenceImageMediaRefs"
            | "referenceVideoMediaRefs"
            | "referenceAudioMediaRefs"
    )
}

fn add_palmier_alias_tool_descriptors(tools: &mut Vec<CodexLocalToolDescriptor>) {
    let aliases = tools
        .iter()
        .filter_map(|tool| {
            palmier_bare_tool_alias(&tool.name).map(|alias| {
                let mut alias_tool = tool.clone();
                alias_tool.name = alias.to_string();
                alias_tool
            })
        })
        .collect::<Vec<_>>();
    tools.extend(aliases);
}

fn palmier_bare_tool_alias(canonical_name: &str) -> Option<&'static str> {
    match canonical_name {
        "video_creater.get_timeline" => Some("get_timeline"),
        "video_creater.get_media" => Some("get_media"),
        "video_creater.add_clips" => Some("add_clips"),
        "video_creater.insert_clips" => Some("insert_clips"),
        "video_creater.remove_clips" => Some("remove_clips"),
        "video_creater.remove_tracks" => Some("remove_tracks"),
        "video_creater.move_clips" => Some("move_clips"),
        "video_creater.apply_layout" => Some("apply_layout"),
        "video_creater.link_clips" => Some("link_clips"),
        "video_creater.unlink_clips" => Some("unlink_clips"),
        "video_creater.set_clip_properties" => Some("set_clip_properties"),
        "video_creater.set_keyframes" => Some("set_keyframes"),
        "video_creater.split_clips" => Some("split_clips"),
        "video_creater.ripple_delete_ranges" => Some("ripple_delete_ranges"),
        "video_creater.remove_words" => Some("remove_words"),
        "video_creater.remove_silence" => Some("remove_silence"),
        "video_creater.sync_audio" => Some("sync_audio"),
        "video_creater.undo_agent_edit" => Some("undo"),
        "video_creater.add_texts" => Some("add_texts"),
        "video_creater.update_text" => Some("update_text"),
        "video_creater.add_captions" => Some("add_captions"),
        "video_creater.export_project" => Some("export_project"),
        "video_creater.generate_video" => Some("generate_video"),
        "video_creater.generate_image" => Some("generate_image"),
        "video_creater.generate_audio" => Some("generate_audio"),
        "video_creater.upscale_media" => Some("upscale_media"),
        "video_creater.rerun_generated_asset" => Some("rerun_generated_asset"),
        "video_creater.import_media" => Some("import_media"),
        "video_creater.create_matte" => Some("create_matte"),
        "video_creater.list_models" => Some("list_models"),
        "video_creater.inspect_media" => Some("inspect_media"),
        "video_creater.get_transcript" => Some("get_transcript"),
        "video_creater.inspect_timeline" => Some("inspect_timeline"),
        "video_creater.search_media" => Some("search_media"),
        "video_creater.build_multi_source_edit" => Some("build_multi_source_edit"),
        "video_creater.apply_color" => Some("apply_color"),
        "video_creater.apply_effect" => Some("apply_effect"),
        "video_creater.denoise_audio" => Some("denoise_audio"),
        "video_creater.inspect_color" => Some("inspect_color"),
        "video_creater.list_folders" => Some("list_folders"),
        "video_creater.create_folder" => Some("create_folder"),
        "video_creater.move_to_folder" => Some("move_to_folder"),
        "video_creater.rename_media" => Some("rename_media"),
        "video_creater.rename_folder" => Some("rename_folder"),
        "video_creater.delete_media" => Some("delete_media"),
        "video_creater.delete_folder" => Some("delete_folder"),
        "video_creater.send_feedback" => Some("send_feedback"),
        "video_creater.set_project_settings" => Some("set_project_settings"),
        "video_creater.create_timeline" => Some("create_timeline"),
        "video_creater.set_active_timeline" => Some("set_active_timeline"),
        "video_creater.duplicate_timeline" => Some("duplicate_timeline"),
        "video_creater.read_skill" => Some("read_skill"),
        "video_creater.get_projects" => Some("get_projects"),
        "video_creater.open_project" => Some("open_project"),
        "video_creater.new_project" => Some("new_project"),
        _ => None,
    }
}

fn tool_descriptor(
    name: &str,
    title: &str,
    category: &str,
    description: &str,
    input_schema: Value,
) -> CodexLocalToolDescriptor {
    CodexLocalToolDescriptor {
        name: name.to_string(),
        title: title.to_string(),
        category: category.to_string(),
        description: description.to_string(),
        input_schema,
    }
}

fn project_actions_tool_schema(mut schema: Value) -> Value {
    let Some(object) = schema.as_object_mut() else {
        return schema;
    };
    let properties = object.entry("properties").or_insert_with(|| json!({}));
    if let Some(properties) = properties.as_object_mut() {
        properties
            .entry("projectDir".to_string())
            .or_insert_with(|| json!({ "type": "string", "minLength": 1 }));
        properties.entry("actions".to_string()).or_insert_with(|| {
            json!({
                "type": "array",
                "minItems": 1,
                "items": { "type": "object" }
            })
        });
    }

    let project_actions_variant = json!({ "required": ["projectDir", "actions"] });
    if let Some(any_of) = object.get_mut("anyOf").and_then(Value::as_array_mut) {
        if !any_of
            .iter()
            .any(|candidate| candidate == &project_actions_variant)
        {
            any_of.push(project_actions_variant);
        }
        return schema;
    }

    let mut any_of = Vec::new();
    if let Some(required) = object.remove("required") {
        any_of.push(json!({ "required": required }));
    }
    any_of.push(project_actions_variant);
    object.insert("anyOf".to_string(), json!(any_of));
    schema
}

fn empty_object_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {}
    })
}

fn get_timeline_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "startSeconds": { "type": "number", "minimum": 0 },
            "endSeconds": { "type": "number", "minimum": 0 },
            "startFrame": { "type": "integer", "minimum": 0 },
            "endFrame": { "type": "integer", "minimum": 0 }
        }
    })
}

fn transcript_words_schema() -> Value {
    json!({
        "type": "object",
        "anyOf": [
            { "required": ["mediaId"] },
            { "required": ["mediaRef"] }
        ],
        "additionalProperties": false,
        "properties": {
            "mediaId": { "type": "string", "minLength": 1 },
            "mediaRef": { "type": "string", "minLength": 1 },
            "startSeconds": { "type": "number", "minimum": 0 },
            "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
            "startFrame": { "type": "integer", "minimum": 0 },
            "endFrame": { "type": "integer", "minimum": 0 },
            "clipId": { "type": "string", "minLength": 1 },
            "language": { "type": "string", "minLength": 1 },
            "wordTimestamps": { "type": "boolean" },
            "offset": { "type": "integer", "minimum": 0 },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_TRANSCRIPT_WORD_LIMIT }
        }
    })
}

fn get_transcript_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "mediaId": { "type": "string", "minLength": 1 },
            "mediaRef": { "type": "string", "minLength": 1 },
            "startSeconds": { "type": "number", "minimum": 0 },
            "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
            "startFrame": { "type": "integer", "minimum": 0 },
            "endFrame": { "type": "integer", "minimum": 0 },
            "clipId": { "type": "string", "minLength": 1 },
            "language": { "type": "string", "minLength": 1 },
            "wordTimestamps": { "type": "boolean" },
            "offset": { "type": "integer", "minimum": 0 },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_TRANSCRIPT_WORD_LIMIT }
        }
    })
}

fn inspect_media_schema() -> Value {
    json!({
        "type": "object",
        "anyOf": [
            { "required": ["mediaRef"] },
            { "required": ["mediaId"] }
        ],
        "additionalProperties": false,
        "properties": {
            "mediaRef": { "type": "string", "minLength": 1 },
            "mediaId": { "type": "string", "minLength": 1 },
            "clipId": { "type": "string", "minLength": 1 },
            "startSeconds": { "type": "number", "minimum": 0 },
            "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
            "startFrame": { "type": "integer", "minimum": 0 },
            "endFrame": { "type": "integer", "minimum": 0 },
            "overview": { "type": "boolean" },
            "wordTimestamps": { "type": "boolean" },
            "maxFrames": { "type": "integer", "minimum": 1, "maximum": 12 },
            "language": { "type": "string", "minLength": 1 }
        }
    })
}

fn generate_media_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "projectDir": { "type": "string", "minLength": 1 },
            "assetId": { "type": "string", "minLength": 1 },
            "jobId": { "type": "string", "minLength": 1 },
            "mockMode": { "type": "boolean" },
            "name": { "type": "string", "minLength": 1 },
            "targetFolderId": { "type": "string", "minLength": 1 },
            "folderId": { "type": "string", "minLength": 1 },
            "placementIntent": { "type": "string", "minLength": 1 },
            "replacementItemId": { "type": "string", "minLength": 1 },
            "prompt": { "type": "string" },
            "model": {
                "anyOf": [
                    { "type": "object" },
                    { "type": "string", "minLength": 1 }
                ]
            },
            "references": { "type": "object" },
            "settings": { "type": "object" },
            "mediaRef": { "type": "string", "minLength": 1 },
            "mediaId": { "type": "string", "minLength": 1 },
            "sourceClipId": { "type": "string", "minLength": 1 },
            "sourceVideoMediaRef": { "type": "string", "minLength": 1 },
            "videoSourceMediaRef": { "type": "string", "minLength": 1 },
            "startFrameMediaRef": { "type": "string", "minLength": 1 },
            "endFrameMediaRef": { "type": "string", "minLength": 1 },
            "referenceMediaRefs": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 }
            },
            "referenceImageMediaRefs": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 }
            },
            "referenceVideoMediaRefs": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 }
            },
            "referenceAudioMediaRefs": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 }
            },
            "duration": { "type": "number", "exclusiveMinimum": 0.0 },
            "aspectRatio": { "type": "string", "minLength": 1 },
            "resolution": { "type": "string", "minLength": 1 },
            "numImages": { "type": "integer", "minimum": 1, "maximum": 4 },
            "generateAudio": { "type": "boolean" },
            "quality": { "type": "string", "minLength": 1 },
            "voice": { "type": "string", "minLength": 1 },
            "lyrics": { "type": "string", "minLength": 1 },
            "styleInstructions": { "type": "string", "minLength": 1 },
            "instrumental": { "type": "boolean" },
            "videoSourceStartFrame": { "type": "integer", "minimum": 0 },
            "videoSourceEndFrame": { "type": "integer", "minimum": 0 },
            "brief": { "type": "object" }
        }
    })
}

fn generate_visual_media_schema() -> Value {
    let mut schema = generate_media_schema();
    let object = schema
        .as_object_mut()
        .expect("generate media schema should be object");
    object.insert("required".to_string(), json!(["prompt"]));
    if let Some(prompt) = object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("prompt"))
        .and_then(Value::as_object_mut)
    {
        prompt.insert("minLength".to_string(), json!(1));
    }
    schema
}

fn rerun_generated_asset_schema() -> Value {
    json!({
        "type": "object",
        "required": ["sourceAssetId"],
        "additionalProperties": false,
        "properties": {
            "projectDir": { "type": "string", "minLength": 1 },
            "assetId": { "type": "string", "minLength": 1 },
            "jobId": { "type": "string", "minLength": 1 },
            "mockMode": { "type": "boolean" },
            "sourceAssetId": { "type": "string", "minLength": 1 },
            "prompt": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 },
            "targetFolderId": { "type": "string", "minLength": 1 },
            "folderId": { "type": "string", "minLength": 1 },
            "placementIntent": { "type": "string", "minLength": 1 },
            "replacementItemId": { "type": "string", "minLength": 1 }
        }
    })
}

fn upscale_media_schema() -> Value {
    json!({
        "type": "object",
        "anyOf": [
            { "required": ["mediaRef"] },
            { "required": ["mediaId"] }
        ],
        "additionalProperties": false,
        "properties": {
            "projectDir": { "type": "string", "minLength": 1 },
            "assetId": { "type": "string", "minLength": 1 },
            "jobId": { "type": "string", "minLength": 1 },
            "mockMode": { "type": "boolean" },
            "mediaRef": { "type": "string", "minLength": 1 },
            "mediaId": { "type": "string", "minLength": 1 },
            "sourceClipId": { "type": "string", "minLength": 1 },
            "model": { "type": "string", "minLength": 1 },
            "providerInputUrl": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 },
            "targetFolderId": { "type": "string", "minLength": 1 },
            "folderId": { "type": "string", "minLength": 1 },
            "placementIntent": { "type": "string", "minLength": 1 },
            "replacementItemId": { "type": "string", "minLength": 1 }
        }
    })
}

fn get_transcript_payload(
    project: &VideoProject,
    args: &GetTranscriptArgs,
) -> Result<Value, CodexLocalToolError> {
    let _tolerate_inspect_media_word_timestamps = args.word_timestamps;
    transcript_words_payload(
        project,
        &TranscriptWordsArgs {
            media_id: args.media_id.clone(),
            start_seconds: args.start_seconds,
            end_seconds: args.end_seconds,
            start_frame: args.start_frame,
            end_frame: args.end_frame,
            clip_id: args.clip_id.clone(),
            language: args.language.clone(),
            offset: args.offset,
            limit: args.limit,
        },
    )
}

fn decode_args<T>(args: Value) -> Result<T, CodexLocalToolError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args)
        .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))
}

fn project_context_payload(project: &VideoProject, project_dir: Option<String>) -> Value {
    let split_project = project_dir
        .filter(|dir| !dir.trim().is_empty())
        .map(|dir| {
            let root = PathBuf::from(dir);
            json!({
                "root": root.display().to_string(),
                "manifest": root.join("video-creater.project.json").display().to_string(),
                "timeline": root.join("timeline.json").display().to_string(),
                "media": root.join("media/index.json").display().to_string(),
                "transcripts": root.join("transcripts").display().to_string(),
                "generated": root.join("generated").display().to_string(),
                "renders": root.join("renders").display().to_string(),
                "jobs": root.join("jobs").display().to_string(),
                "exports": root.join("exports").display().to_string(),
                "context": root.join("context/project.json").display().to_string(),
                "logs": root.join("logs").display().to_string()
            })
        })
        .unwrap_or_else(|| json!(null));

    json!({
        "project": {
            "id": project.id,
            "name": project.name,
            "schemaVersion": project.schema_version,
            "createdAt": project.created_at,
            "updatedAt": project.updated_at,
            "codexThreadId": project.codex_thread_id
        },
        "counts": {
            "media": project.media.len(),
            "mediaFolders": project.media_folders.len(),
            "timelineTracks": project.timeline.tracks.len(),
            "timelineItems": project.timeline.tracks.iter().map(|track| track.items.len()).sum::<usize>(),
            "transcripts": project.transcripts.len(),
            "generatedAssets": project.generated_assets.len(),
            "renderReports": project.render_reports.len(),
            "exportArtifacts": project.export_artifacts.len(),
            "jobs": project.jobs.len(),
            "templateOverrides": project.template_overrides.len()
        },
        "splitProject": split_project,
        "policy": {
            "canonicalMutation": "projectActionsValidatedByRust",
            "directFileMutation": false,
            "networkAccess": false,
            "providerCredentialsExposed": false
        }
    })
}

#[derive(Debug, Clone, Copy)]
struct TimelineWindow {
    start_seconds: f64,
    end_seconds: f64,
    start_frame: u64,
    end_frame: u64,
}

fn timeline_payload(
    project: &VideoProject,
    args: &GetTimelineArgs,
) -> Result<Value, CodexLocalToolError> {
    let fps = project_fps(project)?;
    let window = timeline_window(project, args, fps)?;
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media))
        .collect::<BTreeMap<_, _>>();
    let tracks = project
        .timeline
        .tracks
        .iter()
        .take(MAX_TOOL_TRACKS)
        .map(|track| {
            let visible_items = track
                .items
                .iter()
                .filter(|item| timeline_window_contains_item(window, item))
                .collect::<Vec<_>>();
            let items = visible_items
                .iter()
                .take(MAX_TOOL_ITEMS_PER_TRACK)
                .map(|item| {
                    let item = *item;
                    json!({
                        "id": item.id,
                        "kind": item.kind,
                        "startSeconds": item.start_seconds,
                        "durationSeconds": item.duration_seconds,
                        "source": item.source,
                        "label": item.label,
                        "properties": item.properties
                    })
                })
                .collect::<Vec<_>>();
            let clips = visible_items
                .iter()
                .take(MAX_TOOL_ITEMS_PER_TRACK)
                .map(|item| timeline_clip_payload(item, fps, &media_by_id))
                .collect::<Vec<_>>();
            let mut track_payload = json!({
                "id": track.id,
                "name": track.name,
                "kind": track.kind,
                "locked": track.locked,
                "enabled": track.enabled,
                "items": items,
                "clips": clips
            });
            if let Some(object) = track_payload.as_object_mut() {
                if visible_items.len() > MAX_TOOL_ITEMS_PER_TRACK {
                    object.insert("totalItems".to_string(), json!(visible_items.len()));
                    object.insert("totalClips".to_string(), json!(visible_items.len()));
                } else if window.is_some() && visible_items.len() < track.items.len() {
                    object.insert("totalItems".to_string(), json!(track.items.len()));
                    object.insert("totalClips".to_string(), json!(track.items.len()));
                }
                let caption_groups = caption_groups_payload(&visible_items, fps);
                if !caption_groups.is_empty() {
                    object.insert("captionGroups".to_string(), json!(caption_groups));
                }
                if !track.transitions.is_empty() {
                    object.insert("transitions".to_string(), json!(track.transitions));
                }
            }
            track_payload
        })
        .collect::<Vec<_>>();

    let active_timeline_id = project.active_timeline_id.as_deref().unwrap_or("main");
    let timelines = if project.timelines.is_empty() {
        vec![
            json!({ "timelineId": active_timeline_id, "id": active_timeline_id, "name": "Timeline 1", "active": true, "durationSeconds": project.timeline.duration_seconds, "totalFrames": seconds_to_frames(project.timeline.duration_seconds, fps), "fps": fps }),
        ]
    } else {
        project
            .timelines
            .iter()
            .map(|timeline| {
                let timeline_data = if timeline.id == active_timeline_id {
                    &project.timeline
                } else {
                    &timeline.timeline
                };
                json!({
                    "timelineId": timeline.id,
                    "id": timeline.id,
                    "name": timeline.name,
                    "active": timeline.id == active_timeline_id,
                    "durationSeconds": timeline_data.duration_seconds,
                    "totalFrames": seconds_to_frames(timeline_data.duration_seconds, fps),
                    "fps": fps
                })
            })
            .collect::<Vec<_>>()
    };
    let mut payload = json!({
        "timelineId": active_timeline_id,
        "activeTimelineId": active_timeline_id,
        "timelines": timelines,
        "durationSeconds": project.timeline.duration_seconds,
        "totalFrames": seconds_to_frames(project.timeline.duration_seconds, fps),
        "currentFrame": 0,
        "fps": fps,
        "resolution": {
            "width": project.render_settings.width,
            "height": project.render_settings.height
        },
        "canGenerate": true,
        "tracks": tracks,
        "truncated": project.timeline.tracks.len() > MAX_TOOL_TRACKS
    });
    if let Some(window) = window {
        if let Some(object) = payload.as_object_mut() {
            object.insert(
                "window".to_string(),
                json!({
                    "startFrame": window.start_frame,
                    "endFrame": window.end_frame,
                    "startSeconds": window.start_seconds,
                    "endSeconds": window.end_seconds
                }),
            );
        }
    }
    Ok(payload)
}

fn create_timeline_payload(
    project: &VideoProject,
    args: CreateTimelineArgs,
) -> Result<Value, CodexLocalToolError> {
    let requested_name = optional_trimmed(args.name).unwrap_or_else(|| "Timeline 2".to_string());
    let used_ids = project
        .timelines
        .iter()
        .map(|timeline| timeline.id.as_str())
        .collect::<BTreeSet<_>>();
    let base = requested_name
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    let base = match base.trim_matches('-') {
        "" => "untitled",
        value => value,
    };
    let mut timeline_id = format!("timeline-{base}");
    let mut suffix = 2;
    while used_ids.contains(timeline_id.as_str()) {
        timeline_id = format!("timeline-{base}-{suffix}");
        suffix += 1;
    }
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: None,
            actions: vec![ProjectAction::CreateTimeline {
                timeline_id: timeline_id.clone(),
                name: requested_name.clone(),
                duplicate_active: false,
                source_timeline_id: None,
            }],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("timelineId".to_string(), json!(timeline_id));
        object.insert("name".to_string(), json!(requested_name));
    }
    Ok(payload)
}

fn set_active_timeline_payload(
    project: &VideoProject,
    args: SetActiveTimelineArgs,
) -> Result<Value, CodexLocalToolError> {
    let timeline_id = trim_required(&args.timeline_id, "timelineId")?;
    let exists = project.timelines.is_empty() && timeline_id == "main"
        || project
            .timelines
            .iter()
            .any(|timeline| timeline.id == timeline_id);
    if !exists {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "timeline `{timeline_id}` was not found"
        )));
    }
    mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: None,
            actions: vec![ProjectAction::SetActiveTimeline { timeline_id }],
        },
    )
}

fn duplicate_timeline_payload(
    project: &VideoProject,
    args: DuplicateTimelineArgs,
) -> Result<Value, CodexLocalToolError> {
    let source_timeline_id = optional_trimmed(args.timeline_id).unwrap_or_else(|| {
        project
            .active_timeline_id
            .clone()
            .unwrap_or_else(|| "main".to_string())
    });
    let requested_name =
        optional_trimmed(args.name).unwrap_or_else(|| format!("{} copy", project.name));
    if source_timeline_id
        != project
            .active_timeline_id
            .clone()
            .unwrap_or_else(|| "main".to_string())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "duplicate_timeline currently duplicates the active timeline; switch first".to_string(),
        ));
    }
    let timeline_id = format!("timeline-copy-{}", project.timelines.len() + 1);
    mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: None,
            actions: vec![ProjectAction::CreateTimeline {
                timeline_id,
                name: requested_name,
                duplicate_active: true,
                source_timeline_id: Some(source_timeline_id),
            }],
        },
    )
}

fn get_projects_payload() -> Result<Value, CodexLocalToolError> {
    Err(CodexLocalToolError::InvalidArguments(
        "get_projects is Palmier app-level project navigation; Video Creater local tools operate on the already loaded project context".to_string(),
    ))
}

fn open_project_payload(args: OpenProjectArgs) -> Result<Value, CodexLocalToolError> {
    let target = optional_trimmed(args.id)
        .or_else(|| optional_trimmed(args.path))
        .unwrap_or_else(|| "<unspecified>".to_string());
    Err(CodexLocalToolError::InvalidArguments(format!(
        "open_project is Palmier app-level project navigation; open `{target}` from the Video Creater app shell before using local project tools"
    )))
}

fn new_project_payload(args: NewProjectArgs) -> Result<Value, CodexLocalToolError> {
    let requested_name =
        optional_trimmed(args.name).unwrap_or_else(|| "Untitled Project".to_string());
    Err(CodexLocalToolError::InvalidArguments(format!(
        "new_project is Palmier app-level project navigation; create `{requested_name}` from the Video Creater app shell before using local project tools"
    )))
}

fn timeline_window(
    project: &VideoProject,
    args: &GetTimelineArgs,
    fps: f64,
) -> Result<Option<TimelineWindow>, CodexLocalToolError> {
    if args.start_seconds.is_some() && args.start_frame.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "get_timeline accepts startSeconds or startFrame, not both".to_string(),
        ));
    }
    if args.end_seconds.is_some() && args.end_frame.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "get_timeline accepts endSeconds or endFrame, not both".to_string(),
        ));
    }
    if args.start_seconds.is_none()
        && args.end_seconds.is_none()
        && args.start_frame.is_none()
        && args.end_frame.is_none()
    {
        return Ok(None);
    }

    let start_seconds = args
        .start_seconds
        .or_else(|| args.start_frame.map(|frame| frames_to_seconds(frame, fps)))
        .unwrap_or(0.0);
    let end_seconds = args
        .end_seconds
        .or_else(|| args.end_frame.map(|frame| frames_to_seconds(frame, fps)))
        .unwrap_or(project.timeline.duration_seconds);
    if !start_seconds.is_finite() || !end_seconds.is_finite() || start_seconds < 0.0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "get_timeline window values must be finite and non-negative".to_string(),
        ));
    }
    if end_seconds <= start_seconds {
        return Err(CodexLocalToolError::InvalidArguments(
            "get_timeline end must be greater than start".to_string(),
        ));
    }

    Ok(Some(TimelineWindow {
        start_seconds,
        end_seconds,
        start_frame: seconds_to_frames(start_seconds, fps),
        end_frame: seconds_to_frames(end_seconds, fps),
    }))
}

fn timeline_window_contains_item(window: Option<TimelineWindow>, item: &TimelineItem) -> bool {
    let Some(window) = window else {
        return true;
    };
    let item_start = item.start_seconds;
    let item_end = item.start_seconds + item.duration_seconds;
    item_start < window.end_seconds && item_end > window.start_seconds
}

fn caption_groups_payload(items: &[&TimelineItem], fps: f64) -> Vec<Value> {
    let mut group_order = Vec::new();
    let mut grouped: BTreeMap<String, Vec<&TimelineItem>> = BTreeMap::new();
    for item in items {
        let Some(group_id) = caption_group_id(item) else {
            continue;
        };
        if !grouped.contains_key(group_id) {
            group_order.push(group_id.to_string());
        }
        grouped.entry(group_id.to_string()).or_default().push(*item);
    }

    group_order
        .into_iter()
        .filter_map(|group_id| {
            let members = grouped.remove(&group_id)?;
            Some(caption_group_payload(&group_id, &members, fps))
        })
        .collect()
}

fn caption_group_payload(group_id: &str, members: &[&TimelineItem], fps: f64) -> Value {
    let mut shared_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut shared_payloads: BTreeMap<String, Value> = BTreeMap::new();
    let mut modal_key = String::new();

    for item in members {
        let shared = caption_shared_payload(item);
        let key = serde_json::to_string(&shared).unwrap_or_default();
        let count = shared_counts.entry(key.clone()).or_insert(0);
        *count += 1;
        shared_payloads.entry(key.clone()).or_insert(shared);
        if *count > shared_counts.get(&modal_key).copied().unwrap_or(0) {
            modal_key = key;
        }
    }

    let mut clip_rows = Vec::new();
    let mut item_rows = Vec::new();
    let mut deviant_clip_ids = Vec::new();
    let mut frame_min = u64::MAX;
    let mut frame_max = 0;
    for item in members {
        let start_frame = seconds_to_frames(item.start_seconds, fps);
        let duration_frames = seconds_to_frames(item.duration_seconds, fps);
        let end_frame = start_frame.saturating_add(duration_frames);
        frame_min = frame_min.min(start_frame);
        frame_max = frame_max.max(end_frame);

        let shared = caption_shared_payload(item);
        let key = serde_json::to_string(&shared).unwrap_or_default();
        if key == modal_key {
            let text = caption_item_text(item);
            clip_rows.push(json!([item.id, start_frame, duration_frames, text]));
            item_rows.push(json!([
                item.id,
                round_tool_seconds(item.start_seconds),
                round_tool_seconds(item.duration_seconds),
                text
            ]));
        } else {
            deviant_clip_ids.push(item.id.clone());
        }
    }
    clip_rows.sort_by_key(|row| row[1].as_u64().unwrap_or(0));
    item_rows.sort_by(|left, right| {
        left[1]
            .as_f64()
            .unwrap_or(0.0)
            .partial_cmp(&right[1].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    deviant_clip_ids.sort();
    let total_clip_rows = clip_rows.len();
    let clips_truncated = total_clip_rows > MAX_CAPTION_GROUP_ROWS;
    if clips_truncated {
        clip_rows.truncate(MAX_CAPTION_GROUP_ROWS);
        item_rows.truncate(MAX_CAPTION_GROUP_ROWS);
    }

    let mut payload = json!({
        "captionGroupId": group_id,
        "clipCount": total_clip_rows,
        "itemCount": members.len(),
        "frameRange": [frame_min, frame_max],
        "clipFormat": ["clipId", "startFrame", "durationFrames", "text"],
        "clips": clip_rows,
        "itemFormat": ["itemId", "startSeconds", "durationSeconds", "text"],
        "items": item_rows
    });
    if let Some(object) = payload.as_object_mut() {
        if let Some(shared) = shared_payloads.remove(&modal_key) {
            object.insert("shared".to_string(), shared);
        }
        if !deviant_clip_ids.is_empty() {
            object.insert("deviantClipIds".to_string(), json!(deviant_clip_ids));
        }
        if clips_truncated {
            object.insert(
                "clipsNote".to_string(),
                json!(format!(
                    "Showing first {MAX_CAPTION_GROUP_ROWS} of {total_clip_rows} caption rows; page with startFrame/endFrame."
                )),
            );
        }
    }
    payload
}

fn caption_group_id(item: &TimelineItem) -> Option<&str> {
    if item.kind != TimelineItemKind::Caption {
        return None;
    }
    item.properties
        .get("captionGroupId")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn caption_shared_payload(item: &TimelineItem) -> Value {
    let properties = item
        .properties
        .iter()
        .filter(|(key, _)| key.as_str() != "captionGroupId")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    json!({
        "kind": item.kind,
        "properties": properties
    })
}

fn caption_item_text(item: &TimelineItem) -> &str {
    match &item.source {
        TimelineSource::Text { text } => text,
        _ => item.label.as_str(),
    }
}

fn timeline_clip_payload(
    item: &TimelineItem,
    fps: f64,
    media_by_id: &BTreeMap<&str, &MediaAsset>,
) -> Value {
    let start_frame = seconds_to_frames(item.start_seconds, fps);
    let duration_frames = seconds_to_frames(item.duration_seconds, fps);
    let mut payload = json!({
        "id": item.id,
        "clipId": item.id,
        "startFrame": start_frame,
        "durationFrames": duration_frames,
        "startSeconds": round_tool_seconds(item.start_seconds),
        "durationSeconds": round_tool_seconds(item.duration_seconds),
        "label": item.label,
        "properties": item.properties
    });

    let object = payload
        .as_object_mut()
        .expect("timeline clip payload should be object");
    match &item.source {
        TimelineSource::Media { media_id } => {
            object.insert("mediaRef".to_string(), json!(media_id));
            object.insert("mediaId".to_string(), json!(media_id));
            let media_type = media_by_id
                .get(media_id.as_str())
                .map(|media| json!(media.kind))
                .unwrap_or_else(|| json!("unknown"));
            object.insert("mediaType".to_string(), media_type);
            let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
            object.insert(
                "trimStartFrame".to_string(),
                json!(seconds_to_frames(source_in, fps)),
            );
            if let Some(media) = media_by_id.get(media_id.as_str()) {
                let source_out = timeline_item_number_property(item, "sourceOut")
                    .unwrap_or(
                        source_in + item.duration_seconds * timeline_item_playback_speed(item),
                    )
                    .min(media.duration_seconds)
                    .max(source_in);
                let trim_end_seconds = (media.duration_seconds - source_out).max(0.0);
                object.insert(
                    "trimEndFrame".to_string(),
                    json!(seconds_to_frames(trim_end_seconds, fps)),
                );
            }
        }
        TimelineSource::Generated { artifact_id } => {
            object.insert("mediaRef".to_string(), json!(artifact_id));
            object.insert("mediaId".to_string(), json!(artifact_id));
            object.insert("mediaType".to_string(), json!("generated"));
        }
        TimelineSource::Timeline { timeline_id } => {
            object.insert("timelineRef".to_string(), json!(timeline_id));
            object.insert("mediaType".to_string(), json!("timeline"));
        }
        TimelineSource::Text { text } => {
            object.insert("mediaType".to_string(), json!("text"));
            object.insert("textContent".to_string(), json!(text));
        }
    }
    if let Some(keyframes) = timeline_clip_keyframes_payload(item, fps) {
        object.insert("keyframes".to_string(), keyframes);
    }
    if let Some(speed) = timeline_item_number_property(item, "speed")
        .filter(|speed| speed.is_finite() && (speed - 1.0).abs() > f64::EPSILON)
    {
        object.insert("speed".to_string(), json!(speed));
    }
    if let Some(volume) = timeline_item_palmier_volume(item) {
        object.insert("volume".to_string(), json!(volume));
    }
    if let Some(opacity) = timeline_item_number_property(item, "opacity")
        .filter(|opacity| opacity.is_finite() && (*opacity - 1.0).abs() > f64::EPSILON)
    {
        object.insert("opacity".to_string(), json!(opacity));
    }
    if let Some(transform) = item.properties.get("transform").and_then(Value::as_object) {
        if !transform.is_empty() {
            object.insert("transform".to_string(), json!(transform));
        }
    }
    if let Some(link_group_id) = timeline_item_link_group_id(item) {
        object.insert("linkGroupId".to_string(), json!(link_group_id));
    }
    if let Some(blend_mode) = item.properties.get("blendMode").and_then(Value::as_str) {
        if PALMIER_BLEND_MODES.contains(&blend_mode) && blend_mode != "normal" {
            object.insert("blendMode".to_string(), json!(blend_mode));
        }
    }
    if let Some(audio_sync) = item.properties.get("audioSync").and_then(Value::as_object) {
        if !audio_sync.is_empty() {
            object.insert("audioSync".to_string(), json!(audio_sync));
        }
    }
    if let Some(effects) = item.properties.get("effects").and_then(Value::as_array) {
        if !effects.is_empty() {
            object.insert("effects".to_string(), json!(effects));
        }
    }
    if let Some(color_grade) = item.properties.get("colorGrade").and_then(Value::as_object) {
        if !color_grade.is_empty() {
            object.insert("colorGrade".to_string(), json!(color_grade));
        }
    }

    payload
}

fn timeline_item_palmier_volume(item: &TimelineItem) -> Option<f64> {
    let volume_db = timeline_item_number_property(item, "volumeDb")?;
    if !volume_db.is_finite() {
        return None;
    }
    if volume_db <= -60.0 {
        return Some(0.0);
    }
    Some(round_tool_seconds(10_f64.powf(volume_db / 20.0)))
}

#[derive(Debug, Clone)]
struct TimelineKeyframeRow {
    at_seconds: f64,
    value: f64,
    easing: Option<String>,
}

fn timeline_clip_keyframes_payload(item: &TimelineItem, fps: f64) -> Option<Value> {
    let keyframes = item
        .properties
        .get("keyframes")
        .and_then(Value::as_object)?;
    let mut payload = serde_json::Map::new();

    for property in ["opacity", "scale", "scaleX", "scaleY", "rotationDegrees"] {
        if let Some(rows) = timeline_scalar_keyframe_rows(keyframes.get(property), fps, Some) {
            payload.insert(property.to_string(), rows);
        }
    }
    if let Some(rows) = timeline_scalar_keyframe_rows(keyframes.get("volumeDb"), fps, |value| {
        Some(db_to_palmier_volume(value))
    }) {
        payload.insert("volume".to_string(), rows);
    }
    if let Some(rows) = timeline_scalar_keyframe_rows(keyframes.get("volumeDb"), fps, Some) {
        payload.insert("volumeDb".to_string(), rows);
    }
    if let Some(rows) =
        timeline_pair_keyframe_rows(keyframes.get("positionX"), keyframes.get("positionY"), fps)
    {
        payload.insert("position".to_string(), rows);
    }
    if !payload.contains_key("scale") {
        if let Some(rows) =
            timeline_pair_keyframe_rows(keyframes.get("scaleX"), keyframes.get("scaleY"), fps)
        {
            payload.insert("scale".to_string(), rows);
        }
    }
    if let Some(rows) = timeline_crop_keyframe_rows(
        keyframes.get("cropTop"),
        keyframes.get("cropRight"),
        keyframes.get("cropBottom"),
        keyframes.get("cropLeft"),
        fps,
    ) {
        payload.insert("crop".to_string(), rows);
    }

    (!payload.is_empty()).then_some(Value::Object(payload))
}

fn timeline_scalar_keyframe_rows(
    value: Option<&Value>,
    fps: f64,
    map_value: impl Fn(f64) -> Option<f64>,
) -> Option<Value> {
    let rows = parse_timeline_keyframe_rows(value)?
        .into_iter()
        .filter_map(|keyframe| {
            let value = map_value(keyframe.value)?;
            let mut row = vec![
                json!(seconds_to_frames(keyframe.at_seconds, fps)),
                json!(round_tool_seconds(value)),
            ];
            push_palmier_easing(&mut row, keyframe.easing.as_deref());
            Some(Value::Array(row))
        })
        .collect::<Vec<_>>();
    (!rows.is_empty()).then_some(Value::Array(rows))
}

fn timeline_pair_keyframe_rows(
    first: Option<&Value>,
    second: Option<&Value>,
    fps: f64,
) -> Option<Value> {
    let first = parse_timeline_keyframe_rows(first)?;
    let second = parse_timeline_keyframe_rows(second)?;
    let mut rows = Vec::new();
    for first_keyframe in first {
        let Some(second_keyframe) = second
            .iter()
            .find(|candidate| candidate.at_seconds == first_keyframe.at_seconds)
        else {
            continue;
        };
        let mut row = vec![
            json!(seconds_to_frames(first_keyframe.at_seconds, fps)),
            json!(round_tool_seconds(first_keyframe.value)),
            json!(round_tool_seconds(second_keyframe.value)),
        ];
        let easing = first_keyframe
            .easing
            .as_deref()
            .or(second_keyframe.easing.as_deref());
        push_palmier_easing(&mut row, easing);
        rows.push(Value::Array(row));
    }
    (!rows.is_empty()).then_some(Value::Array(rows))
}

fn timeline_crop_keyframe_rows(
    top: Option<&Value>,
    right: Option<&Value>,
    bottom: Option<&Value>,
    left: Option<&Value>,
    fps: f64,
) -> Option<Value> {
    let top = parse_timeline_keyframe_rows(top)?;
    let right = parse_timeline_keyframe_rows(right)?;
    let bottom = parse_timeline_keyframe_rows(bottom)?;
    let left = parse_timeline_keyframe_rows(left)?;
    let mut rows = Vec::new();
    for top_keyframe in top {
        let Some(right_keyframe) = right
            .iter()
            .find(|candidate| candidate.at_seconds == top_keyframe.at_seconds)
        else {
            continue;
        };
        let Some(bottom_keyframe) = bottom
            .iter()
            .find(|candidate| candidate.at_seconds == top_keyframe.at_seconds)
        else {
            continue;
        };
        let Some(left_keyframe) = left
            .iter()
            .find(|candidate| candidate.at_seconds == top_keyframe.at_seconds)
        else {
            continue;
        };
        let mut row = vec![
            json!(seconds_to_frames(top_keyframe.at_seconds, fps)),
            json!(round_tool_seconds(top_keyframe.value)),
            json!(round_tool_seconds(right_keyframe.value)),
            json!(round_tool_seconds(bottom_keyframe.value)),
            json!(round_tool_seconds(left_keyframe.value)),
        ];
        let easing = top_keyframe
            .easing
            .as_deref()
            .or(right_keyframe.easing.as_deref())
            .or(bottom_keyframe.easing.as_deref())
            .or(left_keyframe.easing.as_deref());
        push_palmier_easing(&mut row, easing);
        rows.push(Value::Array(row));
    }
    (!rows.is_empty()).then_some(Value::Array(rows))
}

fn parse_timeline_keyframe_rows(value: Option<&Value>) -> Option<Vec<TimelineKeyframeRow>> {
    let mut rows = value?
        .as_array()?
        .iter()
        .filter_map(|keyframe| {
            let at_seconds = keyframe.get("atSeconds").and_then(Value::as_f64)?;
            let value = keyframe.get("value").and_then(Value::as_f64)?;
            if !at_seconds.is_finite() || !value.is_finite() {
                return None;
            }
            Some(TimelineKeyframeRow {
                at_seconds,
                value,
                easing: keyframe
                    .get("easing")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.at_seconds.total_cmp(&right.at_seconds));
    (!rows.is_empty()).then_some(rows)
}

fn push_palmier_easing(row: &mut Vec<Value>, easing: Option<&str>) {
    if let Some(easing) = easing.map(str::trim).filter(|value| !value.is_empty()) {
        if easing != "smooth" {
            row.push(json!(easing));
        }
    }
}

fn db_to_palmier_volume(db: f64) -> f64 {
    if db <= -60.0 {
        return 0.0;
    }
    round_tool_seconds(10_f64.powf(db / 20.0).clamp(0.0, 1.0))
}

fn timeline_item_payload(item: &TimelineItem) -> Value {
    json!({
        "id": item.id,
        "kind": item.kind,
        "startSeconds": item.start_seconds,
        "durationSeconds": item.duration_seconds,
        "source": item.source,
        "label": item.label,
        "properties": item.properties
    })
}

fn inspect_timeline_payload(
    project: &VideoProject,
    args: &InspectTimelineArgs,
) -> Result<Value, CodexLocalToolError> {
    let fps = project_fps(project)?;
    let start_seconds = args
        .start_seconds
        .or_else(|| args.start_frame.map(|frame| frames_to_seconds(frame, fps)))
        .unwrap_or(0.0);
    let end_seconds = args
        .end_seconds
        .or_else(|| args.end_frame.map(|frame| frames_to_seconds(frame, fps)))
        .unwrap_or(project.timeline.duration_seconds.max(start_seconds));
    if start_seconds < 0.0 || end_seconds < start_seconds {
        return Err(CodexLocalToolError::InvalidArguments(
            "timeline inspection window must use non-negative increasing seconds".to_string(),
        ));
    }

    let max_items = args.max_items.unwrap_or(20).clamp(1, 50);
    let max_frames = args.max_frames.unwrap_or(1).clamp(1, 12);
    let mut issues = Vec::new();
    let mut active_items = Vec::new();
    let media_ids = project
        .media
        .iter()
        .map(|media| media.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    'tracks: for track in &project.timeline.tracks {
        if !track.enabled {
            continue;
        }

        for item in &track.items {
            if active_items.len() >= max_items {
                break 'tracks;
            }
            let item_end = item.start_seconds + item.duration_seconds;
            if item_end <= start_seconds || item.start_seconds >= end_seconds {
                continue;
            }

            if let TimelineSource::Media { media_id } = &item.source {
                if !media_ids.contains(media_id.as_str()) {
                    issues.push(format!(
                        "Timeline item {} references missing media {}.",
                        item.id, media_id
                    ));
                }
            }

            active_items.push(json!({
                "trackId": track.id,
                "itemId": item.id,
                "kind": item.kind,
                "label": item.label,
                "startSeconds": item.start_seconds,
                "durationSeconds": item.duration_seconds,
                "startFrame": seconds_to_frames(item.start_seconds, fps),
                "durationFrames": seconds_to_frames(item.duration_seconds, fps),
                "source": item.source,
                "sourceIn": item.properties.get("sourceIn").cloned().unwrap_or(Value::Null),
                "sourceOut": item.properties.get("sourceOut").cloned().unwrap_or(Value::Null)
            }));
        }
    }
    let preview_time_seconds = start_seconds;
    let preview_frame = seconds_to_frames(preview_time_seconds, fps);
    let frame_numbers = inspect_timeline_frame_numbers(start_seconds, end_seconds, fps, max_frames);
    let preview_layers =
        inspect_timeline_preview_layers(project, preview_time_seconds, max_items, &mut issues);

    Ok(json!({
        "timelineDurationSeconds": project.timeline.duration_seconds,
        "timelineDurationFrames": seconds_to_frames(project.timeline.duration_seconds, fps),
        "window": {
            "startSeconds": start_seconds,
            "endSeconds": end_seconds,
            "startFrame": seconds_to_frames(start_seconds, fps),
            "endFrame": seconds_to_frames(end_seconds, fps)
        },
        "previewTimeSeconds": preview_time_seconds,
        "previewFrame": preview_frame,
        "frameNumbers": frame_numbers,
        "previewLayers": preview_layers,
        "activeItems": active_items,
        "issues": issues,
        "truncated": active_items.len() >= max_items
    }))
}

fn inspect_timeline_frame_numbers(
    start_seconds: f64,
    end_seconds: f64,
    fps: f64,
    max_frames: usize,
) -> Vec<u64> {
    let start_frame = seconds_to_frames(start_seconds, fps);
    let mut end_frame = seconds_to_frames(end_seconds, fps);
    if end_frame <= start_frame {
        return vec![start_frame];
    }
    if max_frames <= 1 {
        return vec![start_frame];
    }
    end_frame = end_frame.saturating_sub(1);
    let span = end_frame.saturating_sub(start_frame);
    (0..max_frames)
        .map(|index| {
            start_frame + ((span as f64 * index as f64) / (max_frames - 1) as f64).round() as u64
        })
        .collect()
}

fn inspect_timeline_preview_layers(
    project: &VideoProject,
    preview_time_seconds: f64,
    max_items: usize,
    issues: &mut Vec<String>,
) -> Vec<Value> {
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media))
        .collect::<BTreeMap<_, _>>();
    let generated_assets_by_id = project
        .generated_assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<BTreeMap<_, _>>();
    let mut layers = Vec::new();

    'tracks: for track in &project.timeline.tracks {
        if !track.enabled {
            continue;
        }

        for item in &track.items {
            if layers.len() >= max_items {
                break 'tracks;
            }
            if !timeline_item_active_at(item, preview_time_seconds) {
                continue;
            }
            if item.kind == TimelineItemKind::AudioClip {
                continue;
            }
            if timeline_item_string_property(item, "templateId").is_some() {
                if let Some(layer) =
                    text_preview_layer(track.id.as_str(), item, "", preview_time_seconds)
                {
                    layers.push(layer);
                }
                continue;
            }

            match &item.source {
                TimelineSource::Media { media_id } => {
                    let Some(media) = media_by_id.get(media_id.as_str()) else {
                        issues.push(format!(
                            "Timeline item {} references missing media {}.",
                            item.id, media_id
                        ));
                        continue;
                    };
                    if matches!(media.kind, MediaKind::Audio) {
                        continue;
                    }
                    if media.relative_path.trim().is_empty() {
                        issues.push(format!(
                            "Timeline item {} media {} has no preview path.",
                            item.id, media.id
                        ));
                        continue;
                    }
                    layers.push(media_preview_layer(
                        track.id.as_str(),
                        item,
                        preview_time_seconds,
                        MediaPreviewSource {
                            media_id: media.id.as_str(),
                            media_kind: &media.kind,
                            relative_path: media.relative_path.as_str(),
                            duration_seconds: media.duration_seconds,
                            artifact_id: None,
                        },
                    ));
                }
                TimelineSource::Generated { artifact_id } => {
                    let Some(generated_asset) = generated_assets_by_id.get(artifact_id.as_str())
                    else {
                        issues.push(format!(
                            "Timeline item {} references missing generated asset {}.",
                            item.id, artifact_id
                        ));
                        continue;
                    };
                    if generated_asset.status != GeneratedAssetStatus::Completed {
                        issues.push(format!(
                            "Timeline item {} generated asset {} is not completed.",
                            item.id, artifact_id
                        ));
                        continue;
                    }
                    let Some(output) = generated_asset.outputs.first() else {
                        issues.push(format!(
                            "Timeline item {} generated asset {} has no preview output.",
                            item.id, artifact_id
                        ));
                        continue;
                    };
                    let linked_media = media_by_id.get(output.media_id.as_str());
                    let media_kind = linked_media
                        .map(|media| &media.kind)
                        .unwrap_or(&generated_asset.kind);
                    if matches!(media_kind, MediaKind::Audio) {
                        continue;
                    }
                    let relative_path = linked_media
                        .map(|media| media.relative_path.as_str())
                        .unwrap_or(output.relative_path.as_str());
                    if relative_path.trim().is_empty() {
                        issues.push(format!(
                            "Timeline item {} generated asset {} has no preview path.",
                            item.id, artifact_id
                        ));
                        continue;
                    }
                    let duration_seconds = linked_media
                        .map(|media| media.duration_seconds)
                        .unwrap_or(output.duration_seconds);
                    layers.push(media_preview_layer(
                        track.id.as_str(),
                        item,
                        preview_time_seconds,
                        MediaPreviewSource {
                            media_id: output.media_id.as_str(),
                            media_kind,
                            relative_path,
                            duration_seconds,
                            artifact_id: Some(artifact_id.as_str()),
                        },
                    ));
                }
                TimelineSource::Timeline { timeline_id } => {
                    issues.push(format!(
                        "Timeline item {} nests timeline {}; nested timeline preview is not implemented yet.",
                        item.id, timeline_id
                    ));
                }
                TimelineSource::Text { text } => {
                    if let Some(layer) = text_preview_layer(
                        track.id.as_str(),
                        item,
                        text.as_str(),
                        preview_time_seconds,
                    ) {
                        layers.push(layer);
                    }
                }
            }
        }
    }

    layers
}

struct MediaPreviewSource<'a> {
    media_id: &'a str,
    media_kind: &'a MediaKind,
    relative_path: &'a str,
    duration_seconds: f64,
    artifact_id: Option<&'a str>,
}

fn media_preview_layer(
    track_id: &str,
    item: &TimelineItem,
    preview_time_seconds: f64,
    source: MediaPreviewSource<'_>,
) -> Value {
    let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
    let source_out = timeline_item_number_property(item, "sourceOut");
    let max_source_time = source_out.unwrap_or({
        if source.duration_seconds > 0.0 {
            source.duration_seconds
        } else {
            f64::INFINITY
        }
    });
    let unclamped_source_time = source_in
        + (preview_time_seconds - item.start_seconds) * timeline_item_playback_speed(item);
    let source_time_seconds = unclamped_source_time.clamp(0.0, max_source_time);
    let motion = preview_motion_for_item(item, preview_time_seconds);
    let mut layer = json!({
        "layerKind": "media",
        "trackId": track_id,
        "itemId": item.id,
        "label": item.label,
        "mediaId": source.media_id,
        "mediaKind": source.media_kind,
        "relativePath": source.relative_path,
        "timelineStartSeconds": item.start_seconds,
        "timelineEndSeconds": item.start_seconds + item.duration_seconds,
        "sourceTimeSeconds": round_tool_seconds(source_time_seconds),
        "opacity": motion.opacity,
        "positionX": motion.position_x,
        "positionY": motion.position_y,
        "scale": motion.scale,
        "scaleX": motion.scale_x,
        "scaleY": motion.scale_y,
        "rotationDegrees": motion.rotation_degrees
    });
    if let Some(artifact_id) = source.artifact_id {
        if let Some(object) = layer.as_object_mut() {
            object.insert("sourceType".to_string(), json!("generated"));
            object.insert("artifactId".to_string(), json!(artifact_id));
        }
    }
    layer
}

fn text_preview_layer(
    track_id: &str,
    item: &TimelineItem,
    text: &str,
    preview_time_seconds: f64,
) -> Option<Value> {
    let template_id = timeline_item_string_property(item, "templateId");
    let motion = preview_motion_for_item(item, preview_time_seconds);
    if let Some(template_id) = template_id {
        return Some(json!({
            "layerKind": "overlay",
            "overlayKind": "template",
            "trackId": track_id,
            "itemId": item.id,
            "label": item.label,
            "templateId": template_id,
            "text": Value::Null,
            "timelineStartSeconds": item.start_seconds,
            "timelineEndSeconds": item.start_seconds + item.duration_seconds,
            "opacity": motion.opacity,
            "positionX": motion.position_x,
            "positionY": motion.position_y,
            "scale": motion.scale,
            "scaleX": motion.scale_x,
            "scaleY": motion.scale_y,
            "rotationDegrees": motion.rotation_degrees
        }));
    }

    let overlay_kind = match item.kind {
        TimelineItemKind::Caption => "caption",
        TimelineItemKind::Overlay => "text",
        _ => return None,
    };
    Some(json!({
        "layerKind": "overlay",
        "overlayKind": overlay_kind,
        "trackId": track_id,
        "itemId": item.id,
        "label": item.label,
        "templateId": Value::Null,
        "text": if text.trim().is_empty() { item.label.as_str() } else { text },
        "timelineStartSeconds": item.start_seconds,
        "timelineEndSeconds": item.start_seconds + item.duration_seconds,
        "opacity": motion.opacity,
        "positionX": motion.position_x,
        "positionY": motion.position_y,
        "scale": motion.scale,
        "scaleX": motion.scale_x,
        "scaleY": motion.scale_y,
        "rotationDegrees": motion.rotation_degrees
    }))
}

#[derive(Debug, Clone, Copy)]
struct PreviewMotion {
    opacity: f64,
    position_x: f64,
    position_y: f64,
    scale: f64,
    scale_x: f64,
    scale_y: f64,
    rotation_degrees: f64,
}

fn preview_motion_for_item(item: &TimelineItem, preview_time_seconds: f64) -> PreviewMotion {
    let local_seconds = preview_time_seconds - item.start_seconds;
    let scale = keyframed_number_property(item, "scale", local_seconds, 1.0);
    PreviewMotion {
        opacity: keyframed_number_property(
            item,
            "opacity",
            local_seconds,
            timeline_item_number_property(item, "opacity").unwrap_or(1.0),
        ),
        position_x: keyframed_number_property(item, "positionX", local_seconds, 0.0),
        position_y: keyframed_number_property(item, "positionY", local_seconds, 0.0),
        scale,
        scale_x: keyframed_number_property(item, "scaleX", local_seconds, scale),
        scale_y: keyframed_number_property(item, "scaleY", local_seconds, scale),
        rotation_degrees: keyframed_number_property(item, "rotationDegrees", local_seconds, 0.0),
    }
}

fn keyframed_number_property(
    item: &TimelineItem,
    property: &str,
    local_seconds: f64,
    fallback: f64,
) -> f64 {
    let Some(raw_keyframes) = item
        .properties
        .get("keyframes")
        .and_then(|keyframes| keyframes.get(property))
        .and_then(Value::as_array)
    else {
        return fallback;
    };
    let mut keyframes = raw_keyframes
        .iter()
        .filter_map(|keyframe| {
            let at_seconds = keyframe.get("atSeconds").and_then(Value::as_f64)?;
            let value = keyframe.get("value").and_then(Value::as_f64)?;
            if at_seconds.is_finite() && value.is_finite() {
                Some((at_seconds, value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    keyframes.sort_by(|left, right| left.0.total_cmp(&right.0));

    let Some(first) = keyframes.first().copied() else {
        return fallback;
    };
    if local_seconds <= first.0 {
        return round_tool_seconds(first.1);
    }
    let last = keyframes.last().copied().unwrap_or(first);
    if local_seconds >= last.0 {
        return round_tool_seconds(last.1);
    }
    for window in keyframes.windows(2) {
        let (current_at, current_value) = window[0];
        let (next_at, next_value) = window[1];
        if local_seconds >= current_at && local_seconds <= next_at {
            let span = next_at - current_at;
            if span <= 0.0 {
                return round_tool_seconds(current_value);
            }
            let progress = (local_seconds - current_at) / span;
            return round_tool_seconds(current_value + (next_value - current_value) * progress);
        }
    }

    fallback
}

fn media_library_payload(project: &VideoProject) -> Value {
    let folders = project
        .media_folders
        .iter()
        .take(MAX_TOOL_FOLDERS)
        .map(|folder| {
            json!({
                "id": folder.id,
                "name": folder.name,
                "parentId": folder.parent_id,
                "parentFolderId": folder.parent_id
            })
        })
        .collect::<Vec<_>>();
    let media = project
        .media
        .iter()
        .take(MAX_TOOL_MEDIA)
        .map(media_payload)
        .collect::<Vec<_>>();
    let entries = project
        .media
        .iter()
        .take(MAX_TOOL_MEDIA)
        .map(media_entry_payload)
        .collect::<Vec<_>>();

    json!({
        "folders": folders,
        "media": media,
        "entries": entries,
        "truncated": project.media_folders.len() > MAX_TOOL_FOLDERS || project.media.len() > MAX_TOOL_MEDIA
    })
}

fn folders_payload(project: &VideoProject) -> Value {
    json!({
        "folders": project
            .media_folders
            .iter()
            .take(MAX_TOOL_FOLDERS)
            .map(|folder| {
                json!({
                    "id": folder.id,
                    "name": folder.name,
                    "parentId": folder.parent_id,
                    "parentFolderId": folder.parent_id
                })
            })
            .collect::<Vec<_>>(),
        "truncated": project.media_folders.len() > MAX_TOOL_FOLDERS
    })
}

fn media_payload(media: &MediaAsset) -> Value {
    json!({
        "id": media.id,
        "name": media.name,
        "relativePath": media.relative_path,
        "kind": media.kind,
        "durationSeconds": media.duration_seconds,
        "width": media.width,
        "height": media.height,
        "fps": media.fps,
        "folderId": media.folder_id
    })
}

fn media_entry_payload(media: &MediaAsset) -> Value {
    json!({
        "id": media.id,
        "mediaRef": media.id,
        "name": media.name,
        "type": media.kind,
        "kind": media.kind,
        "duration": round_tool_seconds(media.duration_seconds),
        "durationSeconds": round_tool_seconds(media.duration_seconds),
        "relativePath": media.relative_path,
        "fileName": Path::new(&media.relative_path)
            .file_name()
            .and_then(|file_name| file_name.to_str()),
        "sourceWidth": media.width,
        "sourceHeight": media.height,
        "sourceFPS": media.fps.map(round_tool_seconds),
        "folderId": media.folder_id,
        "generationStatus": "none"
    })
}

fn apply_effect_payload(
    project: &VideoProject,
    args: ApplyEffectArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clipIds must not be empty".to_string(),
        ));
    }
    if args.effects.is_empty() && args.remove.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "provide effects to add/update or remove types to delete".to_string(),
        ));
    }

    let remove_types = args
        .remove
        .iter()
        .map(|effect_type| effect_type.trim().to_string())
        .filter(|effect_type| !effect_type.is_empty())
        .collect::<Vec<_>>();
    let edits = normalize_effect_edits(args.effects)?;

    let mut actions = Vec::with_capacity(args.clip_ids.len());
    let mut effects_by_item = serde_json::Map::new();
    for clip_id in &args.clip_ids {
        let stack = apply_effect_edits_to_clip_stack(project, clip_id, &remove_types, &edits)?;
        effects_by_item.insert(clip_id.clone(), json!(stack));
        actions.push(ProjectAction::UpdateItemEffects {
            item_ids: vec![clip_id.clone()],
            effects: stack,
        });
    }

    let first_stack = args
        .clip_ids
        .first()
        .and_then(|clip_id| effects_by_item.get(clip_id))
        .cloned()
        .unwrap_or_else(|| json!([]));

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("changedItemIds".to_string(), json!(args.clip_ids));
        object.insert("effects".to_string(), first_stack);
        object.insert("effectsByItem".to_string(), Value::Object(effects_by_item));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.inspect_timeline"),
        );
    }
    Ok(payload)
}

fn denoise_audio_payload(
    project: &VideoProject,
    args: DenoiseAudioArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clipIds is empty.".to_string(),
        ));
    }
    if let Some(strength) = args.strength {
        if !strength.is_finite() || !(0.0..=100.0).contains(&strength) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "strength must be 0-100 (got {})",
                format_tool_number(strength)
            )));
        }
    }

    let enabled = args.enabled.unwrap_or(true);
    let amount = args.strength.map(|strength| strength / 100.0);
    let mut actions = Vec::with_capacity(args.clip_ids.len());
    let mut effects_by_item = serde_json::Map::new();
    for clip_id in &args.clip_ids {
        let stack = denoise_audio_stack_for_clip(project, clip_id, enabled, amount)?;
        effects_by_item.insert(clip_id.clone(), json!(stack));
        actions.push(ProjectAction::UpdateItemEffects {
            item_ids: vec![clip_id.clone()],
            effects: stack,
        });
    }

    let first_stack = args
        .clip_ids
        .first()
        .and_then(|clip_id| effects_by_item.get(clip_id))
        .cloned()
        .unwrap_or_else(|| json!([]));

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("changedItemIds".to_string(), json!(args.clip_ids));
        object.insert("effects".to_string(), first_stack);
        object.insert("effectsByItem".to_string(), Value::Object(effects_by_item));
        object.insert(
            "message".to_string(),
            if enabled {
                let pct = ((amount.unwrap_or(AUDIO_DENOISE_DEFAULT_AMOUNT)) * 100.0).round();
                json!(format!(
                    "Denoise enabled at {}% on {} {}.",
                    format_tool_number(pct),
                    args.clip_ids.len(),
                    if args.clip_ids.len() == 1 {
                        "clip"
                    } else {
                        "clips"
                    }
                ))
            } else {
                json!(format!(
                    "Disabled denoise on {} {}.",
                    args.clip_ids.len(),
                    if args.clip_ids.len() == 1 {
                        "clip"
                    } else {
                        "clips"
                    }
                ))
            },
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.inspect_timeline"),
        );
    }
    Ok(payload)
}

fn normalize_effect_edits(
    entries: Vec<ApplyEffectEntry>,
) -> Result<Vec<NormalizedEffectEdit>, CodexLocalToolError> {
    let catalog = effect_catalog();
    entries
        .into_iter()
        .map(|entry| {
            let effect_type = entry.effect_type.trim().to_string();
            if effect_type.is_empty() {
                return Err(CodexLocalToolError::InvalidArguments(
                    "effect type must not be blank".to_string(),
                ));
            }
            if effect_type == AUDIO_DENOISE_EFFECT_TYPE {
                return Err(CodexLocalToolError::InvalidArguments(
                    "audio.denoise is an audio effect; use video_creater.denoise_audio".to_string(),
                ));
            }
            let descriptor = catalog
                .iter()
                .find(|descriptor| descriptor.id == effect_type.as_str())
                .ok_or_else(|| {
                    CodexLocalToolError::InvalidArguments(format!("unknown effect `{effect_type}`"))
                })?;
            if descriptor.color_effect {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "{effect_type} is a color effect; use video_creater.apply_color"
                )));
            }
            let mut params = BTreeMap::new();
            for (key, value) in entry.params {
                let spec = descriptor
                    .params
                    .iter()
                    .find(|param| param.key == key)
                    .ok_or_else(|| {
                        CodexLocalToolError::InvalidArguments(format!(
                            "{effect_type} unknown effect parameter `{key}`"
                        ))
                    })?;
                let number = value.as_f64().ok_or_else(|| {
                    CodexLocalToolError::InvalidArguments(format!(
                        "{effect_type} effect parameter `{key}` must be numeric"
                    ))
                })?;
                params.insert(key, json!(number.clamp(spec.min, spec.max)));
            }
            Ok(NormalizedEffectEdit {
                effect_type,
                params,
                enabled: entry.enabled,
            })
        })
        .collect()
}

fn apply_effect_edits_to_clip_stack(
    project: &VideoProject,
    clip_id: &str,
    remove_types: &[String],
    edits: &[NormalizedEffectEdit],
) -> Result<Vec<ProjectActionEffect>, CodexLocalToolError> {
    let mut stack = current_effect_stack(project, clip_id)?;
    stack.retain(|effect| {
        !remove_types
            .iter()
            .any(|remove| remove == &effect.effect_type)
    });

    for edit in edits {
        let existing_index = stack
            .iter()
            .position(|effect| effect.effect_type == edit.effect_type);
        let mut effect = existing_index
            .and_then(|index| stack.get(index).cloned())
            .unwrap_or(ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: edit.effect_type.clone(),
                enabled: true,
                params: BTreeMap::new(),
            });
        if let Some(enabled) = edit.enabled {
            effect.enabled = enabled;
        }
        effect.params.extend(edit.params.clone());

        if let Some(index) = existing_index {
            stack.remove(index);
        }
        insert_effect_in_canonical_order(&mut stack, effect);
    }

    Ok(stack)
}

fn apply_color_payload(
    project: &VideoProject,
    args: ApplyColorArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        ApplyColorArgs::Direct(args) => args,
        ApplyColorArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    if args.clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clipIds must not be empty".to_string(),
        ));
    }
    let has_grade_value = args.exposure.is_some()
        || args.contrast.is_some()
        || args.saturation.is_some()
        || args.temperature.is_some()
        || args.tint.is_some()
        || !args.extra.is_empty();
    if !has_grade_value && !args.reset {
        return Err(CodexLocalToolError::InvalidArguments(
            "provide at least one color control or reset=true".to_string(),
        ));
    }

    let grade = ProjectActionColorGrade {
        exposure: args.exposure,
        contrast: args.contrast,
        saturation: args.saturation,
        temperature: args.temperature,
        tint: args.tint,
        extra: args.extra,
    };
    let action = ProjectAction::UpdateItemColorGrade {
        item_ids: args.clip_ids.clone(),
        reset: args.reset,
        grade: grade.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("changedItemIds".to_string(), json!(args.clip_ids));
        object.insert("grade".to_string(), json!(grade));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.inspect_color"),
        );
    }
    Ok(payload)
}

fn inspect_color_payload(
    project: &VideoProject,
    args: InspectColorArgs,
) -> Result<Value, CodexLocalToolError> {
    let clip_id = args
        .clip_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let media_id = args
        .media_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    if clip_id.is_some() == media_id.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "provide exactly one of clipId or mediaRef".to_string(),
        ));
    }

    if let Some(clip_id) = clip_id {
        let (track, item) = timeline_item_with_track(project, clip_id)?;
        if !is_visual_timeline_item(item) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "clip is not visual: {clip_id}"
            )));
        }
        let media_id = match &item.source {
            TimelineSource::Media { media_id } => Some(media_id.clone()),
            TimelineSource::Generated { .. }
            | TimelineSource::Timeline { .. }
            | TimelineSource::Text { .. } => None,
        };
        if let Some(media_id) = media_id.as_deref() {
            if let Some(media) = project.media.iter().find(|media| media.id == media_id) {
                if let Some(scopes) = inspect_color_image_clip_scopes(
                    args.project_dir.as_deref(),
                    media,
                    item.properties.get("colorGrade"),
                )? {
                    let reference = inspect_color_reference_payload(
                        project,
                        args.project_dir.as_deref(),
                        args.reference.as_deref(),
                    )?;
                    let gap = if reference.get("meanRGB").is_some() {
                        color_scope_gap(&scopes, &reference)
                    } else {
                        Value::Null
                    };
                    return Ok(json!({
                        "target": {
                            "clipId": item.id,
                            "trackId": track.id,
                            "mediaId": media_id,
                            "atFrame": args.at_frame
                        },
                        "clip": scopes,
                        "grade": item.properties.get("colorGrade").cloned().unwrap_or_else(|| json!({})),
                        "effects": item.properties.get("effects").cloned().unwrap_or_else(|| json!([])),
                        "reference": reference,
                        "gap": gap,
                        "scopeStatus": "measured_image_clip_scopes"
                    }));
                }
            }
        }
        return Ok(json!({
            "target": {
                "clipId": item.id,
                "trackId": track.id,
                "mediaId": media_id,
                "atFrame": args.at_frame
            },
            "grade": item.properties.get("colorGrade").cloned().unwrap_or_else(|| json!({})),
            "effects": item.properties.get("effects").cloned().unwrap_or_else(|| json!([])),
            "reference": inspect_color_reference_payload(
                project,
                args.project_dir.as_deref(),
                args.reference.as_deref()
            )?,
            "scopeStatus": "stored_metadata_only"
        }));
    }

    let media_id = media_id.expect("media_id checked");
    let media = project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!("media was not found: {media_id}"))
        })?;
    if media.kind == crate::project::model::MediaKind::Audio {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "media is not visual: {media_id}"
        )));
    }
    if let Some(scopes) = inspect_color_image_scopes(args.project_dir.as_deref(), media)? {
        let reference = inspect_color_reference_payload(
            project,
            args.project_dir.as_deref(),
            args.reference.as_deref(),
        )?;
        let gap = if reference.get("meanRGB").is_some() {
            color_scope_gap(&scopes, &reference)
        } else {
            Value::Null
        };
        return Ok(json!({
            "target": {
                "mediaRef": media.id,
                "kind": media.kind,
                "width": media.width,
                "height": media.height
            },
            "media": scopes,
            "grade": {},
            "effects": [],
            "reference": reference,
            "gap": gap,
            "scopeStatus": "measured_image_scopes"
        }));
    }
    Ok(json!({
        "target": {
            "mediaRef": media.id,
            "kind": media.kind,
            "width": media.width,
            "height": media.height
        },
        "grade": {},
        "effects": [],
        "reference": inspect_color_reference_payload(
            project,
            args.project_dir.as_deref(),
            args.reference.as_deref()
        )?,
        "scopeStatus": "raw_media_metadata_only"
    }))
}

fn inspect_color_image_scopes(
    project_dir: Option<&str>,
    media: &MediaAsset,
) -> Result<Option<Value>, CodexLocalToolError> {
    inspect_color_image_clip_scopes(project_dir, media, None)
}

fn inspect_color_image_clip_scopes(
    project_dir: Option<&str>,
    media: &MediaAsset,
    grade: Option<&Value>,
) -> Result<Option<Value>, CodexLocalToolError> {
    if media.kind != MediaKind::Image {
        return Ok(None);
    }
    let media_path = Path::new(&media.relative_path);
    let absolute_path = if media_path.is_absolute() {
        media_path.to_path_buf()
    } else {
        let Some(project_dir) = project_dir
            .map(str::trim)
            .filter(|project_dir| !project_dir.is_empty())
        else {
            return Ok(None);
        };
        Path::new(project_dir).join(media_path)
    };
    let image = image::open(&absolute_path)
        .map_err(|error| {
            CodexLocalToolError::InvalidArguments(format!(
                "could not decode image for inspect_color: {} ({error})",
                absolute_path.display()
            ))
        })?
        .to_rgba8();
    let image = if let Some(grade) = grade {
        apply_image_color_grade(image, grade)
    } else {
        image
    };
    Ok(Some(color_scope_readout(&image)))
}

fn apply_image_color_grade(mut image: image::RgbaImage, grade: &Value) -> image::RgbaImage {
    let exposure = grade
        .get("exposure")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(-3.0, 3.0);
    let contrast = grade
        .get("contrast")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(1.0)
        .clamp(0.5, 1.5);
    let saturation = grade
        .get("saturation")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 2.0);
    let highlights = grade
        .get("highlights")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let shadows = grade
        .get("shadows")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let blacks = grade
        .get("blacks")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let whites = grade
        .get("whites")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let (wheel_lift, wheel_gain, wheel_inv_gamma) = color_wheel_coefficients(grade);
    let wheels_neutral = color_wheels_neutral(wheel_lift, wheel_gain, wheel_inv_gamma);
    let curves = color_grade_curves(grade);
    let curves_neutral = color_grade_curves_neutral(&curves);
    if exposure == 0.0
        && contrast == 1.0
        && saturation == 1.0
        && highlights == 0.0
        && shadows == 0.0
        && blacks == 0.0
        && whites == 0.0
        && wheels_neutral
        && curves_neutral
    {
        return image;
    }

    let exposure_factor = 2.0_f64.powf(exposure);
    let black_point = -blacks * 0.4;
    let white_point = 1.0 - whites * 0.4;
    let levels_width = (white_point - black_point).max(0.05);
    for pixel in image.pixels_mut() {
        let alpha = pixel[3];
        let mut red = pixel[0] as f64 / 255.0;
        let mut green = pixel[1] as f64 / 255.0;
        let mut blue = pixel[2] as f64 / 255.0;

        red = (red * exposure_factor).clamp(0.0, 1.0);
        green = (green * exposure_factor).clamp(0.0, 1.0);
        blue = (blue * exposure_factor).clamp(0.0, 1.0);

        red = ((red - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
        green = ((green - 0.5) * contrast + 0.5).clamp(0.0, 1.0);
        blue = ((blue - 0.5) * contrast + 0.5).clamp(0.0, 1.0);

        if highlights != 0.0 || shadows != 0.0 {
            let y = color_luma(
                red.clamp(0.0, 1.0),
                green.clamp(0.0, 1.0),
                blue.clamp(0.0, 1.0),
            );
            let hi = y * y * y;
            let lo = (1.0 - y) * (1.0 - y) * (1.0 - y);
            let delta_y = (highlights * hi + shadows * lo) * 0.5;
            red = (red + delta_y).clamp(0.0, 1.0);
            green = (green + delta_y).clamp(0.0, 1.0);
            blue = (blue + delta_y).clamp(0.0, 1.0);
        }

        if blacks != 0.0 || whites != 0.0 {
            red = ((red - black_point) / levels_width).clamp(0.0, 1.0);
            green = ((green - black_point) / levels_width).clamp(0.0, 1.0);
            blue = ((blue - black_point) / levels_width).clamp(0.0, 1.0);
        }

        if saturation != 1.0 {
            let luma = color_luma(red, green, blue);
            red = (luma + (red - luma) * saturation).clamp(0.0, 1.0);
            green = (luma + (green - luma) * saturation).clamp(0.0, 1.0);
            blue = (luma + (blue - luma) * saturation).clamp(0.0, 1.0);
        }

        if !wheels_neutral {
            red = color_wheel_channel(red, wheel_lift[0], wheel_gain[0], wheel_inv_gamma[0]);
            green = color_wheel_channel(green, wheel_lift[1], wheel_gain[1], wheel_inv_gamma[1]);
            blue = color_wheel_channel(blue, wheel_lift[2], wheel_gain[2], wheel_inv_gamma[2]);
        }

        if !curves_neutral {
            let y = color_luma(
                red.clamp(0.0, 1.0),
                green.clamp(0.0, 1.0),
                blue.clamp(0.0, 1.0),
            );
            let yp = color_curve_eval(&curves.master, y).clamp(0.0, 1.0);
            if y > 1e-4 {
                let gain = (yp / y).min(8.0);
                red = (red * gain).clamp(0.0, 1.0);
                green = (green * gain).clamp(0.0, 1.0);
                blue = (blue * gain).clamp(0.0, 1.0);
            } else {
                red = yp;
                green = yp;
                blue = yp;
            }
            red = color_curve_eval(&curves.red, red).clamp(0.0, 1.0);
            green = color_curve_eval(&curves.green, green).clamp(0.0, 1.0);
            blue = color_curve_eval(&curves.blue, blue).clamp(0.0, 1.0);
        }

        pixel[0] = color_byte(red);
        pixel[1] = color_byte(green);
        pixel[2] = color_byte(blue);
        pixel[3] = alpha;
    }

    image
}

fn color_byte(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn color_wheel_coefficients(grade: &Value) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let (lift_x, lift_y) = color_wheel_xy(
        color_grade_number(grade, "shadowsHue", 0.0, 0.0, 360.0),
        color_grade_number(grade, "shadowsAmount", 0.0, 0.0, 1.0),
    );
    let (gamma_x, gamma_y) = color_wheel_xy(
        color_grade_number(grade, "midsHue", 0.0, 0.0, 360.0),
        color_grade_number(grade, "midsAmount", 0.0, 0.0, 1.0),
    );
    let (gain_x, gain_y) = color_wheel_xy(
        color_grade_number(grade, "highsHue", 0.0, 0.0, 360.0),
        color_grade_number(grade, "highsAmount", 0.0, 0.0, 1.0),
    );

    let lift = color_wheel_chroma_offset(lift_x, lift_y);
    let gamma = color_wheel_chroma_offset(gamma_x, gamma_y);
    let gain = color_wheel_chroma_offset(gain_x, gain_y);
    let lift_m = color_grade_number(grade, "shadowsLum", 0.0, -0.5, 0.5);
    let gamma_m = color_grade_number(grade, "midsGamma", 1.0, 0.5, 2.0);
    let gain_m = color_grade_number(grade, "highsGain", 1.0, 0.5, 1.5);

    let wheel_lift = [
        lift_m + lift[0] * 0.2,
        lift_m + lift[1] * 0.2,
        lift_m + lift[2] * 0.2,
    ];
    let wheel_gain = [
        gain_m * (1.0 + gain[0] * 0.35),
        gain_m * (1.0 + gain[1] * 0.35),
        gain_m * (1.0 + gain[2] * 0.35),
    ];
    let wheel_inv_gamma = [
        1.0 / (gamma_m * (1.0 + gamma[0] * 0.35)).max(0.01),
        1.0 / (gamma_m * (1.0 + gamma[1] * 0.35)).max(0.01),
        1.0 / (gamma_m * (1.0 + gamma[2] * 0.35)).max(0.01),
    ];

    (wheel_lift, wheel_gain, wheel_inv_gamma)
}

fn color_wheels_neutral(lift: [f64; 3], gain: [f64; 3], inv_gamma: [f64; 3]) -> bool {
    lift.iter().all(|value| value.abs() <= f64::EPSILON)
        && gain.iter().all(|value| (value - 1.0).abs() <= f64::EPSILON)
        && inv_gamma
            .iter()
            .all(|value| (value - 1.0).abs() <= f64::EPSILON)
}

fn color_wheel_channel(value: f64, lift: f64, gain: f64, inv_gamma: f64) -> f64 {
    let lit = (value * (1.0 - lift) + lift).max(0.0) * gain;
    lit.powf(inv_gamma).clamp(0.0, 1.0)
}

fn color_wheel_xy(hue: f64, amount: f64) -> (f64, f64) {
    let radians = hue * std::f64::consts::PI / 180.0;
    (amount * radians.cos(), amount * radians.sin())
}

fn color_wheel_chroma_offset(x: f64, y: f64) -> [f64; 3] {
    let radius = (x * x + y * y).sqrt().min(1.0);
    if radius <= 1e-6 {
        return [0.0, 0.0, 0.0];
    }
    let hue = y.atan2(x) / (2.0 * std::f64::consts::PI);
    let hue_rgb = color_wheel_hue_rgb(hue);
    let mean = (hue_rgb[0] + hue_rgb[1] + hue_rgb[2]) / 3.0;
    [
        (hue_rgb[0] - mean) * radius,
        (hue_rgb[1] - mean) * radius,
        (hue_rgb[2] - mean) * radius,
    ]
}

fn color_wheel_hue_rgb(hue: f64) -> [f64; 3] {
    let x = (hue - hue.floor()) * 6.0;
    let f = x - x.floor();
    match (x as i32).rem_euclid(6) {
        0 => [1.0, f, 0.0],
        1 => [1.0 - f, 1.0, 0.0],
        2 => [0.0, 1.0, f],
        3 => [0.0, 1.0 - f, 1.0],
        4 => [f, 0.0, 1.0],
        _ => [1.0, 0.0, 1.0 - f],
    }
}

fn color_grade_number(grade: &Value, key: &str, default: f64, min: f64, max: f64) -> f64 {
    grade
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
        .clamp(min, max)
}

#[derive(Debug, Default)]
struct ColorGradeCurves {
    master: Vec<(f64, f64)>,
    red: Vec<(f64, f64)>,
    green: Vec<(f64, f64)>,
    blue: Vec<(f64, f64)>,
}

fn color_grade_curves(grade: &Value) -> ColorGradeCurves {
    ColorGradeCurves {
        master: color_curve_points(grade.get("masterCurve")),
        red: color_curve_points(grade.get("redCurve")),
        green: color_curve_points(grade.get("greenCurve")),
        blue: color_curve_points(grade.get("blueCurve")),
    }
}

fn color_grade_curves_neutral(curves: &ColorGradeCurves) -> bool {
    color_curve_neutral(&curves.master)
        && color_curve_neutral(&curves.red)
        && color_curve_neutral(&curves.green)
        && color_curve_neutral(&curves.blue)
}

fn color_curve_neutral(points: &[(f64, f64)]) -> bool {
    points.is_empty()
        || (points.len() == 2
            && (points[0].0 - 0.0).abs() <= f64::EPSILON
            && (points[0].1 - 0.0).abs() <= f64::EPSILON
            && (points[1].0 - 1.0).abs() <= f64::EPSILON
            && (points[1].1 - 1.0).abs() <= f64::EPSILON)
}

fn color_curve_points(value: Option<&Value>) -> Vec<(f64, f64)> {
    let Some(points) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut parsed = points
        .iter()
        .filter_map(|point| {
            let pair = point.as_array()?;
            let x = pair.first()?.as_f64()?;
            let y = pair.get(1)?.as_f64()?;
            if !x.is_finite() || !y.is_finite() {
                return None;
            }
            Some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
        })
        .collect::<Vec<_>>();
    parsed.sort_by(|left, right| left.0.total_cmp(&right.0));
    parsed
}

fn color_curve_eval(points: &[(f64, f64)], x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    if points.is_empty() {
        return x;
    }
    if x <= points[0].0 {
        return points[0].1;
    }
    if let Some(last) = points.last() {
        if x >= last.0 {
            return last.1;
        }
    }
    for window in points.windows(2) {
        let (ax, ay) = window[0];
        let (bx, by) = window[1];
        if x <= bx {
            let t = if (bx - ax).abs() <= f64::EPSILON {
                0.0
            } else {
                (x - ax) / (bx - ax)
            };
            return ay + (by - ay) * t;
        }
    }
    x
}

#[derive(Debug, Clone, Copy)]
struct ColorSample {
    red: f64,
    green: f64,
    blue: f64,
    luma: f64,
    saturation: f64,
    hue_degrees: Option<f64>,
}

fn color_scope_readout(image: &image::RgbaImage) -> Value {
    let mut samples = Vec::new();
    for pixel in image.pixels() {
        let alpha = pixel[3] as f64 / 255.0;
        if alpha <= 0.0 {
            continue;
        }
        let red = pixel[0] as f64 / 255.0;
        let green = pixel[1] as f64 / 255.0;
        let blue = pixel[2] as f64 / 255.0;
        let max_channel = red.max(green).max(blue);
        let min_channel = red.min(green).min(blue);
        let saturation = if max_channel > 0.0 {
            (max_channel - min_channel) / max_channel
        } else {
            0.0
        };
        samples.push(ColorSample {
            red,
            green,
            blue,
            luma: color_luma(red, green, blue),
            saturation,
            hue_degrees: hue_degrees(red, green, blue),
        });
    }

    if samples.is_empty() {
        return json!({
            "luma": {
                "black": 0.0,
                "white": 0.0,
                "mean": 0.0,
                "clipLowPct": 0.0,
                "clipHighPct": 0.0,
                "histogram16": vec![json!(0.0); 16]
            },
            "meanRGB": [0.0, 0.0, 0.0],
            "blackRGB": [0.0, 0.0, 0.0],
            "whiteRGB": [0.0, 0.0, 0.0],
            "zones": {
                "shadows": [0.0, 0.0, 0.0],
                "mids": [0.0, 0.0, 0.0],
                "highs": [0.0, 0.0, 0.0]
            },
            "saturation": 0.0,
            "balance": { "warmCool": 0.0, "greenMagenta": 0.0 },
            "hueHistogram12": vec![json!(0.0); 12],
            "colorfulPct": 0.0
        });
    }

    let sample_count = samples.len() as f64;
    let mut luma_histogram = vec![0.0; 16];
    let mut hue_histogram = vec![0.0; 12];
    let mut mean = [0.0; 3];
    let mut clip_low = 0.0;
    let mut clip_high = 0.0;
    let mut colorful = 0.0;
    let mut saturation_sum = 0.0;
    let mut black_sample = samples[0];
    let mut white_sample = samples[0];
    let mut shadow = ColorAccumulator::default();
    let mut mid = ColorAccumulator::default();
    let mut high = ColorAccumulator::default();

    for sample in &samples {
        mean[0] += sample.red;
        mean[1] += sample.green;
        mean[2] += sample.blue;
        saturation_sum += sample.saturation;
        if sample.saturation > 0.1 {
            colorful += 1.0;
        }
        if sample.luma <= 0.02 {
            clip_low += 1.0;
        }
        if sample.luma >= 0.98 {
            clip_high += 1.0;
        }
        if sample.luma < black_sample.luma {
            black_sample = *sample;
        }
        if sample.luma > white_sample.luma {
            white_sample = *sample;
        }
        let luma_bin = ((sample.luma.clamp(0.0, 1.0) * 15.0).floor() as usize).min(15);
        luma_histogram[luma_bin] += 1.0;
        if let Some(hue) = sample.hue_degrees {
            let hue_bin = ((hue.rem_euclid(360.0) / 30.0).floor() as usize).min(11);
            hue_histogram[hue_bin] += 1.0;
        }
        if sample.luma < 0.33 {
            shadow.add(*sample);
        } else if sample.luma < 0.66 {
            mid.add(*sample);
        } else {
            high.add(*sample);
        }
    }

    mean[0] /= sample_count;
    mean[1] /= sample_count;
    mean[2] /= sample_count;
    let luma_mean = samples.iter().map(|sample| sample.luma).sum::<f64>() / sample_count;
    for value in &mut luma_histogram {
        *value /= sample_count;
    }
    for value in &mut hue_histogram {
        *value /= sample_count;
    }

    json!({
        "luma": {
            "black": round_scope_number(black_sample.luma),
            "white": round_scope_number(white_sample.luma),
            "mean": round_scope_number(luma_mean),
            "clipLowPct": round_scope_number(clip_low * 100.0 / sample_count),
            "clipHighPct": round_scope_number(clip_high * 100.0 / sample_count),
            "histogram16": luma_histogram.into_iter().map(round_scope_number).collect::<Vec<_>>()
        },
        "meanRGB": rgb_scope_array(mean),
        "blackRGB": rgb_scope_array([black_sample.red, black_sample.green, black_sample.blue]),
        "whiteRGB": rgb_scope_array([white_sample.red, white_sample.green, white_sample.blue]),
        "zones": {
            "shadows": shadow.rgb(),
            "mids": mid.rgb(),
            "highs": high.rgb()
        },
        "saturation": round_scope_number(saturation_sum / sample_count),
        "balance": {
            "warmCool": round_scope_number(mean[0] - mean[2]),
            "greenMagenta": round_scope_number(mean[1] - ((mean[0] + mean[2]) / 2.0))
        },
        "hueHistogram12": hue_histogram.into_iter().map(round_scope_number).collect::<Vec<_>>(),
        "colorfulPct": round_scope_number(colorful * 100.0 / sample_count)
    })
}

#[derive(Debug, Default)]
struct ColorAccumulator {
    count: f64,
    red: f64,
    green: f64,
    blue: f64,
}

impl ColorAccumulator {
    fn add(&mut self, sample: ColorSample) {
        self.count += 1.0;
        self.red += sample.red;
        self.green += sample.green;
        self.blue += sample.blue;
    }

    fn rgb(&self) -> Vec<Value> {
        if self.count <= 0.0 {
            return rgb_scope_array([0.0, 0.0, 0.0]);
        }
        rgb_scope_array([
            self.red / self.count,
            self.green / self.count,
            self.blue / self.count,
        ])
    }
}

fn color_luma(red: f64, green: f64, blue: f64) -> f64 {
    red * 0.2126 + green * 0.7152 + blue * 0.0722
}

fn hue_degrees(red: f64, green: f64, blue: f64) -> Option<f64> {
    let max_channel = red.max(green).max(blue);
    let min_channel = red.min(green).min(blue);
    let chroma = max_channel - min_channel;
    if chroma <= f64::EPSILON {
        return None;
    }
    let hue = if (max_channel - red).abs() <= f64::EPSILON {
        60.0 * ((green - blue) / chroma).rem_euclid(6.0)
    } else if (max_channel - green).abs() <= f64::EPSILON {
        60.0 * (((blue - red) / chroma) + 2.0)
    } else {
        60.0 * (((red - green) / chroma) + 4.0)
    };
    Some(hue.rem_euclid(360.0))
}

fn rgb_scope_array(values: [f64; 3]) -> Vec<Value> {
    values.into_iter().map(round_scope_number).collect()
}

fn round_scope_number(value: f64) -> Value {
    json!((value * 1000.0).round() / 1000.0)
}

fn color_scope_gap(current: &Value, reference: &Value) -> Value {
    let luma_black =
        scope_number(current, &["luma", "black"]) - scope_number(reference, &["luma", "black"]);
    let luma_white =
        scope_number(current, &["luma", "white"]) - scope_number(reference, &["luma", "white"]);
    let luma_mean =
        scope_number(current, &["luma", "mean"]) - scope_number(reference, &["luma", "mean"]);
    let warm_cool = scope_number(current, &["balance", "warmCool"])
        - scope_number(reference, &["balance", "warmCool"]);
    let green_magenta = scope_number(current, &["balance", "greenMagenta"])
        - scope_number(reference, &["balance", "greenMagenta"]);
    let saturation =
        scope_number(current, &["saturation"]) - scope_number(reference, &["saturation"]);
    let mut hints = Vec::new();
    if luma_black.abs() > 0.03 {
        hints.push(if luma_black > 0.0 {
            "blacks higher than ref; lower blacks or deepen shadows"
        } else {
            "blacks lower than ref; raise blacks"
        });
    }
    if warm_cool.abs() > 0.03 {
        hints.push(if warm_cool > 0.0 {
            "warmer than ref; cool temperature"
        } else {
            "cooler than ref; warm temperature"
        });
    }
    if green_magenta.abs() > 0.02 {
        hints.push(if green_magenta > 0.0 {
            "greener than ref; tint toward magenta"
        } else {
            "more magenta than ref; tint toward green"
        });
    }
    if saturation.abs() > 0.03 {
        hints.push(if saturation > 0.0 {
            "more saturated than ref; lower saturation"
        } else {
            "less saturated than ref; raise saturation"
        });
    }

    json!({
        "lumaBlack": round_scope_number(luma_black),
        "lumaWhite": round_scope_number(luma_white),
        "lumaMean": round_scope_number(luma_mean),
        "warmCool": round_scope_number(warm_cool),
        "greenMagenta": round_scope_number(green_magenta),
        "saturation": round_scope_number(saturation),
        "hints": hints
    })
}

fn scope_number(value: &Value, path: &[&str]) -> f64 {
    let mut cursor = value;
    for segment in path {
        let Some(next) = cursor.get(*segment) else {
            return 0.0;
        };
        cursor = next;
    }
    cursor.as_f64().unwrap_or(0.0)
}

fn current_effect_stack(
    project: &VideoProject,
    clip_id: &str,
) -> Result<Vec<ProjectActionEffect>, CodexLocalToolError> {
    let (_, item) = timeline_item_with_track(project, clip_id)?;
    if !is_visual_timeline_item(item) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "clip is not visual: {clip_id}"
        )));
    }
    item.properties
        .get("effects")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))
        .map(|effects| effects.unwrap_or_default())
}

fn denoise_audio_stack_for_clip(
    project: &VideoProject,
    clip_id: &str,
    enabled: bool,
    amount: Option<f64>,
) -> Result<Vec<ProjectActionEffect>, CodexLocalToolError> {
    let mut stack = current_audio_effect_stack(project, clip_id)?;
    let existing_index = stack
        .iter()
        .position(|effect| effect.effect_type == AUDIO_DENOISE_EFFECT_TYPE);
    if let Some(index) = existing_index {
        if let Some(effect) = stack.get_mut(index) {
            effect.enabled = enabled;
            if let Some(amount) = amount {
                effect.params.insert("amount".to_string(), json!(amount));
            }
        }
        return Ok(stack);
    }

    if enabled {
        let mut params = BTreeMap::new();
        params.insert(
            "amount".to_string(),
            json!(amount.unwrap_or(AUDIO_DENOISE_DEFAULT_AMOUNT)),
        );
        insert_effect_in_canonical_order(
            &mut stack,
            ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: AUDIO_DENOISE_EFFECT_TYPE.to_string(),
                enabled: true,
                params,
            },
        );
    }

    Ok(stack)
}

fn current_audio_effect_stack(
    project: &VideoProject,
    clip_id: &str,
) -> Result<Vec<ProjectActionEffect>, CodexLocalToolError> {
    let (_, item) = timeline_item_with_track(project, clip_id)?;
    if item.kind != TimelineItemKind::AudioClip {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "Clip {clip_id} is a {} clip; denoise_audio needs an audio clip.",
            timeline_item_kind_label(&item.kind)
        )));
    }
    item.properties
        .get("effects")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))
        .map(|effects| effects.unwrap_or_default())
}

fn timeline_item_kind_label(kind: &TimelineItemKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn format_tool_number(value: f64) -> String {
    if value.is_finite() && value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn insert_effect_in_canonical_order(
    stack: &mut Vec<ProjectActionEffect>,
    effect: ProjectActionEffect,
) {
    let effect_rank = canonical_effect_rank(&effect.effect_type);
    let index = stack
        .iter()
        .position(|existing| canonical_effect_rank(&existing.effect_type) > effect_rank)
        .unwrap_or(stack.len());
    stack.insert(index, effect);
}

fn timeline_item_with_track<'a>(
    project: &'a VideoProject,
    item_id: &str,
) -> Result<(&'a crate::project::model::TimelineTrack, &'a TimelineItem), CodexLocalToolError> {
    project
        .timeline
        .tracks
        .iter()
        .find_map(|track| {
            track
                .items
                .iter()
                .find(|item| item.id == item_id)
                .map(|item| (track, item))
        })
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!("clip was not found: {item_id}"))
        })
}

fn is_visual_timeline_item(item: &TimelineItem) -> bool {
    matches!(
        item.kind,
        crate::project::model::TimelineItemKind::VideoClip
            | crate::project::model::TimelineItemKind::Overlay
            | crate::project::model::TimelineItemKind::HyperframeScene
    )
}

fn inspect_color_reference_payload(
    project: &VideoProject,
    project_dir: Option<&str>,
    reference: Option<&str>,
) -> Result<Value, CodexLocalToolError> {
    let Some(reference) = reference
        .map(str::trim)
        .filter(|reference| !reference.is_empty())
    else {
        return Ok(Value::Null);
    };
    let media = project
        .media
        .iter()
        .find(|media| media.id == reference)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "reference media was not found: {reference}"
            ))
        })?;
    if let Some(scopes) = inspect_color_image_scopes(project_dir, media)? {
        return Ok(scopes);
    }
    Ok(json!({
        "mediaRef": media.id,
        "kind": media.kind,
        "width": media.width,
        "height": media.height
    }))
}

fn add_clips_payload(
    project: &VideoProject,
    args: AddClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        AddClipsArgs::Palmier(args) => add_palmier_clips_payload(project, args),
        AddClipsArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        AddClipsArgs::Direct(args) => add_direct_clips_payload(project, args),
    }
}

fn add_palmier_clips_payload(
    project: &VideoProject,
    args: PalmierAddClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.entries.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries must not be empty".to_string(),
        ));
    }

    let fps = project_fps(project)?;
    let mut affected_item_ids = Vec::with_capacity(args.entries.len());
    let mut affected_items = Vec::with_capacity(args.entries.len());
    let mut items_by_track: BTreeMap<String, Vec<TimelineItem>> = BTreeMap::new();
    let mut auto_tracks_by_id: BTreeMap<String, TimelineTrack> = BTreeMap::new();
    let explicit_track_entries = args
        .entries
        .iter()
        .filter(|entry| entry.track_index.is_some())
        .count();
    if explicit_track_entries > 0 && explicit_track_entries != args.entries.len() {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries.trackIndex must be provided on every entry or omitted on every entry"
                .to_string(),
        ));
    }
    let auto_track_ids = if explicit_track_entries == 0 {
        Some(AutoPalmierTrackIds {
            video: next_palmier_track_id(project, "video"),
            audio: next_palmier_track_id(project, "audio"),
        })
    } else {
        None
    };

    for entry in args.entries {
        let uses_auto_track = entry.track_index.is_none();
        let creates_linked_audio = palmier_entry_media_kind(project, &entry)? == MediaKind::Video;
        let target_track_id = if let Some(track_index) = entry.track_index {
            palmier_entry_target_track_id(project, track_index)?
        } else {
            let auto_track_ids = auto_track_ids
                .as_ref()
                .expect("auto track ids should exist when trackIndex is omitted");
            palmier_entry_auto_track_id(project, &entry, auto_track_ids)?
        };
        let linked_audio_track_id = if creates_linked_audio {
            let track_id = auto_track_ids
                .as_ref()
                .map(|auto_track_ids| auto_track_ids.audio.clone())
                .or_else(|| existing_audio_track_id(project))
                .unwrap_or_else(|| next_palmier_track_id(project, "audio"));
            Some(track_id)
        } else {
            None
        };
        let clip = palmier_media_clip_entry_to_direct(project, entry, fps)?;
        let start_seconds = clip.start_seconds.ok_or_else(|| {
            CodexLocalToolError::InvalidArguments("entries.startFrame is required".to_string())
        })?;
        let (item_id, mut item) = direct_media_clip_item(project, clip, start_seconds)?;
        let linked_audio_item = if creates_linked_audio {
            let link_group_id = format!("link-{item_id}");
            item.properties
                .insert("linkGroupId".to_string(), json!(link_group_id));
            let mut audio_item = item.clone();
            audio_item.id = format!("{item_id}-audio");
            audio_item.kind = TimelineItemKind::AudioClip;
            audio_item.label = format!("{} audio", item.label);
            audio_item
                .properties
                .insert("sourceClipType".to_string(), json!("audio"));
            Some(audio_item)
        } else {
            None
        };
        if uses_auto_track {
            let track = auto_palmier_track_for_item(&target_track_id, &item)?;
            auto_tracks_by_id
                .entry(target_track_id.clone())
                .or_insert(track);
        }
        if let (Some(audio_track_id), Some(audio_item)) =
            (linked_audio_track_id.as_ref(), linked_audio_item.as_ref())
        {
            if !project
                .timeline
                .tracks
                .iter()
                .any(|track| track.id == *audio_track_id)
            {
                let track = auto_palmier_track_for_item(audio_track_id, audio_item)?;
                auto_tracks_by_id
                    .entry(audio_track_id.clone())
                    .or_insert(track);
            }
        }
        affected_item_ids.push(item_id);
        affected_items.push(timeline_item_payload(&item));
        items_by_track
            .entry(target_track_id)
            .or_default()
            .push(item);
        if let (Some(audio_track_id), Some(audio_item)) = (linked_audio_track_id, linked_audio_item)
        {
            affected_item_ids.push(audio_item.id.clone());
            affected_items.push(timeline_item_payload(&audio_item));
            items_by_track
                .entry(audio_track_id)
                .or_default()
                .push(audio_item);
        }
    }

    let mut affected_track_ids = Vec::new();
    let mut actions = Vec::new();
    for (track_id, track) in auto_tracks_by_id {
        affected_track_ids.push(track_id);
        actions.push(ProjectAction::CreateTrack {
            track,
            after_track_id: None,
        });
    }
    for target_track_id in items_by_track.keys() {
        if !affected_track_ids.contains(target_track_id) {
            affected_track_ids.push(target_track_id.clone());
        }
    }
    actions.extend(items_by_track.into_iter().map(|(target_track_id, items)| {
        ProjectAction::AddItems {
            target_track_id,
            items,
        }
    }));
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("affectedTrackIds".to_string(), json!(affected_track_ids));
        object.insert("affectedItems".to_string(), json!(affected_items));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("get_timeline"),
        );
    }
    Ok(payload)
}

fn add_direct_clips_payload(
    project: &VideoProject,
    args: DirectAddClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let target_track_id = args.target_track_id.trim();
    if target_track_id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "targetTrackId must not be blank".to_string(),
        ));
    }
    if args.clips.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clips must not be empty".to_string(),
        ));
    }

    let mut affected_item_ids = Vec::with_capacity(args.clips.len());
    let mut items = Vec::with_capacity(args.clips.len());
    for clip in args.clips {
        let start_seconds = clip.start_seconds.ok_or_else(|| {
            CodexLocalToolError::InvalidArguments("clips.startSeconds is required".to_string())
        })?;
        let (item_id, item) = direct_media_clip_item(project, clip, start_seconds)?;
        affected_item_ids.push(item_id);
        items.push(item);
    }

    let action = ProjectAction::AddItems {
        target_track_id: target_track_id.to_string(),
        items,
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn palmier_media_clip_entry_to_direct(
    project: &VideoProject,
    entry: PalmierMediaClipEntry,
    fps: f64,
) -> Result<DirectMediaClipEntry, CodexLocalToolError> {
    let duration_frames = palmier_add_duration_frames(project, &entry, fps)?;
    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    let media = project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })?;
    let item_id = entry
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("clip-{media_ref}-{}", entry.start_frame));
    let start_seconds = frames_to_seconds(entry.start_frame, fps);
    let duration_seconds = frames_to_seconds(duration_frames, fps);
    let source_in = entry
        .trim_start_frame
        .map(|trim_start_frame| frames_to_seconds(trim_start_frame, fps));
    let source_out = match (source_in, entry.trim_end_frame) {
        (_, Some(trim_end_frame)) => {
            let source_out = media.duration_seconds - frames_to_seconds(trim_end_frame, fps);
            if !source_out.is_finite() || source_out <= 0.0 {
                return Err(CodexLocalToolError::InvalidArguments(
                    "entries.trimEndFrame leaves no source media".to_string(),
                ));
            }
            Some(round_tool_seconds(source_out))
        }
        (Some(source_in), None) => Some(round_tool_seconds(source_in + duration_seconds)),
        (None, None) => None,
    };

    Ok(DirectMediaClipEntry {
        id: item_id,
        media_id: media_ref.to_string(),
        start_seconds: Some(start_seconds),
        duration_seconds,
        source_in,
        source_out,
        label: entry.label,
        opacity: entry.opacity,
    })
}

fn palmier_insert_clip_entry_to_direct(
    project: &VideoProject,
    entry: PalmierInsertClipEntry,
    fps: f64,
    start_frame: u64,
    duration_frames: u64,
) -> Result<DirectMediaClipEntry, CodexLocalToolError> {
    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    let media = project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })?;
    let duration_seconds = frames_to_seconds(duration_frames, fps);
    let source_in = entry
        .trim_start_frame
        .map(|trim_start_frame| frames_to_seconds(trim_start_frame, fps));
    let source_out = match (source_in, entry.trim_end_frame) {
        (_, Some(trim_end_frame)) => {
            let source_out = media.duration_seconds - frames_to_seconds(trim_end_frame, fps);
            if !source_out.is_finite() || source_out <= 0.0 {
                return Err(CodexLocalToolError::InvalidArguments(
                    "entries.trimEndFrame leaves no source media".to_string(),
                ));
            }
            if source_in.is_some_and(|source_in| source_out <= source_in) {
                return Err(CodexLocalToolError::InvalidArguments(
                    "entries trim range leaves no source media".to_string(),
                ));
            }
            Some(round_tool_seconds(source_out))
        }
        (Some(source_in), None) => Some(round_tool_seconds(source_in + duration_seconds)),
        (None, None) => None,
    };

    Ok(DirectMediaClipEntry {
        id: format!("clip-{media_ref}-{start_frame}"),
        media_id: media_ref.to_string(),
        start_seconds: None,
        duration_seconds,
        source_in,
        source_out,
        label: None,
        opacity: None,
    })
}

fn palmier_add_duration_frames(
    project: &VideoProject,
    entry: &PalmierMediaClipEntry,
    fps: f64,
) -> Result<u64, CodexLocalToolError> {
    if entry.duration_frames.is_some() && entry.trim_end_frame.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries: set durationFrames OR trimEndFrame, not both".to_string(),
        ));
    }
    if let Some(duration_frames) = entry.duration_frames {
        if duration_frames == 0 {
            return Err(CodexLocalToolError::InvalidArguments(
                "entries.durationFrames must be greater than zero".to_string(),
            ));
        }
        return Ok(duration_frames);
    }

    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    let media = project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })?;
    let source_frames = seconds_to_frames(media.duration_seconds, fps);
    if source_frames == 0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries.durationFrames is required for media with unknown source length".to_string(),
        ));
    }
    let trimmed_frames = entry.trim_start_frame.unwrap_or(0) + entry.trim_end_frame.unwrap_or(0);
    if trimmed_frames >= source_frames {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries trim range leaves no source media".to_string(),
        ));
    }
    Ok(source_frames - trimmed_frames)
}

fn palmier_insert_entry_media_kind(
    project: &VideoProject,
    entry: &PalmierInsertClipEntry,
) -> Result<MediaKind, CodexLocalToolError> {
    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .map(|asset| asset.kind.clone())
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })
}

fn palmier_insert_duration_frames(
    project: &VideoProject,
    entry: &PalmierInsertClipEntry,
    fps: f64,
) -> Result<u64, CodexLocalToolError> {
    if let Some(duration_frames) = entry.duration_frames {
        if duration_frames == 0 {
            return Err(CodexLocalToolError::InvalidArguments(
                "entries.durationFrames must be greater than zero".to_string(),
            ));
        }
        return Ok(duration_frames);
    }

    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    let media = project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })?;
    let source_frames = seconds_to_frames(media.duration_seconds, fps);
    let trimmed_frames = entry.trim_start_frame.unwrap_or(0) + entry.trim_end_frame.unwrap_or(0);
    let duration_frames = source_frames.saturating_sub(trimmed_frames);
    if duration_frames == 0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries duration resolves to zero frames".to_string(),
        ));
    }
    Ok(duration_frames)
}

fn palmier_entry_media_kind(
    project: &VideoProject,
    entry: &PalmierMediaClipEntry,
) -> Result<MediaKind, CodexLocalToolError> {
    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .map(|asset| asset.kind.clone())
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })
}

struct AutoPalmierTrackIds {
    video: String,
    audio: String,
}

fn palmier_entry_target_track_id(
    project: &VideoProject,
    track_index: usize,
) -> Result<String, CodexLocalToolError> {
    project
        .timeline
        .tracks
        .get(track_index)
        .map(|track| track.id.clone())
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.trackIndex is out of range: {track_index}"
            ))
        })
}

fn palmier_entry_auto_track_id(
    project: &VideoProject,
    entry: &PalmierMediaClipEntry,
    auto_track_ids: &AutoPalmierTrackIds,
) -> Result<String, CodexLocalToolError> {
    let media_ref = trim_required(&entry.media_id, "entries.mediaRef")?;
    let media = project
        .media
        .iter()
        .find(|asset| asset.id == media_ref)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef was not found: {media_ref}"
            ))
        })?;
    Ok(match media.kind {
        MediaKind::Audio => auto_track_ids.audio.clone(),
        MediaKind::Video | MediaKind::Image | MediaKind::Lottie | MediaKind::Generated => {
            auto_track_ids.video.clone()
        }
    })
}

fn existing_audio_track_id(project: &VideoProject) -> Option<String> {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .map(|track| track.id.clone())
}

fn auto_palmier_track_for_item(
    track_id: &str,
    item: &TimelineItem,
) -> Result<TimelineTrack, CodexLocalToolError> {
    let (name, kind) = match item.kind {
        TimelineItemKind::VideoClip
        | TimelineItemKind::ImageClip
        | TimelineItemKind::LottieClip
        | TimelineItemKind::GeneratedClip => ("Palmier Video", TrackKind::Video),
        TimelineItemKind::AudioClip => ("Palmier Audio", TrackKind::Audio),
        _ => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "entries.mediaRef resolved to unsupported item kind: {:?}",
                item.kind
            )))
        }
    };
    Ok(TimelineTrack::empty(track_id, name, kind))
}

fn next_palmier_track_id(project: &VideoProject, suffix: &str) -> String {
    let mut index = 1;
    loop {
        let candidate = format!("track-palmier-{suffix}-{index}");
        if !project
            .timeline
            .tracks
            .iter()
            .any(|track| track.id == candidate)
        {
            return candidate;
        }
        index += 1;
    }
}

fn insert_clips_payload(
    project: &VideoProject,
    args: InsertClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        InsertClipsArgs::Palmier(args) => insert_palmier_clips_payload(project, args),
        InsertClipsArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        InsertClipsArgs::Direct(args) => insert_direct_clips_payload(project, args),
    }
}

fn insert_palmier_clips_payload(
    project: &VideoProject,
    args: PalmierInsertClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.entries.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "entries must not be empty".to_string(),
        ));
    }

    let fps = project_fps(project)?;
    let target_track_id = palmier_entry_target_track_id(project, args.track_index)?;
    let insert_seconds = frames_to_seconds(args.at_frame, fps);
    let mut affected_item_ids = Vec::with_capacity(args.entries.len());
    let mut affected_items = Vec::with_capacity(args.entries.len());
    let mut target_items = Vec::with_capacity(args.entries.len());
    let mut linked_audio_items = Vec::new();
    let mut linked_audio_track_id = None;
    let mut next_start_frame = args.at_frame;

    for entry in args.entries {
        let media_kind = palmier_insert_entry_media_kind(project, &entry)?;
        let duration_frames = palmier_insert_duration_frames(project, &entry, fps)?;
        let clip = palmier_insert_clip_entry_to_direct(
            project,
            entry,
            fps,
            next_start_frame,
            duration_frames,
        )?;
        next_start_frame += duration_frames;
        let (item_id, mut item) = direct_media_clip_item(project, clip, 0.0)?;

        let linked_audio_item = if media_kind == MediaKind::Video {
            let link_group_id = format!("link-{item_id}");
            item.properties
                .insert("linkGroupId".to_string(), json!(link_group_id));
            let mut audio_item = item.clone();
            audio_item.id = format!("{item_id}-audio");
            audio_item.kind = TimelineItemKind::AudioClip;
            audio_item.label = format!("{} audio", item.label);
            audio_item
                .properties
                .insert("sourceClipType".to_string(), json!("audio"));
            if linked_audio_track_id.is_none() {
                linked_audio_track_id = Some(
                    existing_audio_track_id(project)
                        .unwrap_or_else(|| next_palmier_track_id(project, "audio")),
                );
            }
            Some(audio_item)
        } else {
            None
        };

        affected_item_ids.push(item_id);
        affected_items.push(timeline_item_payload(&item));
        target_items.push(item);
        if let Some(audio_item) = linked_audio_item {
            affected_item_ids.push(audio_item.id.clone());
            affected_items.push(timeline_item_payload(&audio_item));
            linked_audio_items.push(audio_item);
        }
    }

    let mut affected_track_ids = BTreeSet::from([target_track_id.clone()]);
    let mut actions = Vec::new();
    if let Some(audio_track_id) = linked_audio_track_id.as_ref() {
        affected_track_ids.insert(audio_track_id.clone());
        if !project
            .timeline
            .tracks
            .iter()
            .any(|track| track.id == *audio_track_id)
        {
            let first_audio_item = linked_audio_items.first().ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(
                    "internal: linked audio track requested without audio items".to_string(),
                )
            })?;
            actions.push(ProjectAction::CreateTrack {
                track: auto_palmier_track_for_item(audio_track_id, first_audio_item)?,
                after_track_id: None,
            });
        }
    }
    actions.push(ProjectAction::InsertItems {
        target_track_id,
        insert_seconds,
        items: target_items,
    });
    if let Some(audio_track_id) = linked_audio_track_id {
        if !linked_audio_items.is_empty() {
            actions.push(ProjectAction::InsertItems {
                target_track_id: audio_track_id,
                insert_seconds,
                items: linked_audio_items,
            });
        }
    }

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert(
            "affectedTrackIds".to_string(),
            json!(affected_track_ids.into_iter().collect::<Vec<_>>()),
        );
        object.insert("affectedItems".to_string(), json!(affected_items));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("get_timeline"),
        );
    }
    Ok(payload)
}

fn insert_direct_clips_payload(
    project: &VideoProject,
    args: DirectInsertClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let target_track_id = args.target_track_id.trim();
    if target_track_id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "targetTrackId must not be blank".to_string(),
        ));
    }
    if args.clips.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clips must not be empty".to_string(),
        ));
    }

    let mut affected_item_ids = Vec::with_capacity(args.clips.len());
    let mut items = Vec::with_capacity(args.clips.len());
    for clip in args.clips {
        let (item_id, item) = direct_media_clip_item(project, clip, 0.0)?;
        affected_item_ids.push(item_id);
        items.push(item);
    }

    let action = ProjectAction::InsertItems {
        target_track_id: target_track_id.to_string(),
        insert_seconds: args.insert_seconds,
        items,
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn direct_media_clip_item(
    project: &VideoProject,
    clip: DirectMediaClipEntry,
    start_seconds: f64,
) -> Result<(String, TimelineItem), CodexLocalToolError> {
    let item_id = clip.id.trim();
    if item_id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clips.id must not be blank".to_string(),
        ));
    }
    let media_id = clip.media_id.trim();
    if media_id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clips.mediaId must not be blank".to_string(),
        ));
    }
    let media = project
        .media
        .iter()
        .find(|asset| asset.id == media_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "clips.mediaId was not found: {media_id}"
            ))
        })?;
    let label = clip
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| media.name.clone())
        .unwrap_or_else(|| item_id.to_string());
    let mut properties = BTreeMap::new();
    if let Some(source_in) = clip.source_in {
        properties.insert("sourceIn".to_string(), json!(source_in));
    }
    if let Some(source_out) = clip.source_out {
        properties.insert("sourceOut".to_string(), json!(source_out));
    } else if let Some(source_in) = clip.source_in {
        properties.insert(
            "sourceOut".to_string(),
            json!(source_in + clip.duration_seconds),
        );
    }
    if let Some(opacity) = clip.opacity {
        properties.insert("opacity".to_string(), json!(opacity));
    }

    Ok((
        item_id.to_string(),
        TimelineItem {
            id: item_id.to_string(),
            kind: timeline_item_kind_for_media(media),
            start_seconds,
            duration_seconds: clip.duration_seconds,
            source: TimelineSource::Media {
                media_id: media_id.to_string(),
            },
            label,
            properties,
        },
    ))
}

fn timeline_item_kind_for_media(media: &MediaAsset) -> TimelineItemKind {
    match media.kind {
        MediaKind::Audio => TimelineItemKind::AudioClip,
        MediaKind::Video => TimelineItemKind::VideoClip,
        MediaKind::Image => TimelineItemKind::ImageClip,
        MediaKind::Lottie => TimelineItemKind::LottieClip,
        MediaKind::Generated => TimelineItemKind::GeneratedClip,
    }
}

fn add_texts_payload(
    project: &VideoProject,
    args: AddTextsArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        AddTextsArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        AddTextsArgs::Direct(args) => add_direct_texts_payload(project, args),
    }
}

fn add_direct_texts_payload(
    project: &VideoProject,
    args: DirectAddTextsArgs,
) -> Result<Value, CodexLocalToolError> {
    if !args.texts.is_empty() && !args.entries.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either texts or entries, not both".to_string(),
        ));
    }
    if args.texts.is_empty() && args.entries.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "texts or entries must not be empty".to_string(),
        ));
    }
    if !args.entries.is_empty() {
        return add_palmier_text_entries_payload(project, args);
    }

    let target_track_id = trim_required(
        args.target_track_id.as_deref().unwrap_or(""),
        "targetTrackId",
    )?;

    let mut affected_item_ids = Vec::with_capacity(args.texts.len());
    let mut items = Vec::with_capacity(args.texts.len());
    for entry in args.texts {
        let item_id = trim_required(&entry.id, "texts.id")?;
        let text = trim_required(&entry.text, "texts.text")?;
        let visual_treatment = trim_required(&entry.visual_treatment, "texts.visualTreatment")?;
        let motion = trim_required(&entry.motion, "texts.motion")?;
        let safe_zone = trim_required(&entry.safe_zone, "texts.safeZone")?;
        let avoid = trim_required(&entry.avoid, "texts.avoid")?;
        let label = entry
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| text.clone());

        affected_item_ids.push(item_id.clone());
        items.push(text_timeline_item(TextTimelineItemSpec {
            id: item_id,
            kind: TimelineItemKind::Overlay,
            start_seconds: entry.start_seconds,
            duration_seconds: entry.duration_seconds,
            label,
            text,
            visual_treatment,
            motion,
            safe_zone,
            avoid,
        }));
    }

    add_text_items_payload(
        project,
        args.project_dir,
        target_track_id,
        items,
        affected_item_ids,
    )
}

fn add_palmier_text_entries_payload(
    project: &VideoProject,
    args: DirectAddTextsArgs,
) -> Result<Value, CodexLocalToolError> {
    let target_track_id = palmier_text_target_track_id(project, &args)?;
    let fps = project_fps(project)?;
    let mut affected_item_ids = Vec::with_capacity(args.entries.len());
    let mut items = Vec::with_capacity(args.entries.len());

    for (index, entry) in args.entries.into_iter().enumerate() {
        let text = trim_required(&entry.content, "entries.content")?;
        let start_seconds = frames_to_seconds(entry.start_frame, fps);
        let duration_seconds = frames_to_seconds(entry.duration_frames, fps);
        if duration_seconds <= 0.0 {
            return Err(CodexLocalToolError::InvalidArguments(
                "entries.durationFrames must be greater than zero".to_string(),
            ));
        }
        let item_id = entry
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| generated_text_item_id(start_seconds, index));
        let mut item = text_timeline_item(TextTimelineItemSpec {
            id: item_id.clone(),
            kind: TimelineItemKind::Overlay,
            start_seconds,
            duration_seconds,
            label: text.clone(),
            text,
            visual_treatment:
                "caption-safe overlay text with transparent backing, shadow, and high contrast"
                    .to_string(),
            motion: "static hold with subtle fade handles".to_string(),
            safe_zone: "inside title-safe bounds and clear of important faces or UI".to_string(),
            avoid:
                "opaque full-width slabs, default-font template look, and long unmoving title cards"
                    .to_string(),
        });
        if let Some(transform) = entry.transform {
            item.properties.insert("transform".to_string(), transform);
        }
        insert_optional_text_style(&mut item, "fontName", entry.font_name);
        if let Some(font_size) = entry.font_size {
            item.properties
                .insert("fontSize".to_string(), json!(font_size));
        }
        insert_optional_text_style(&mut item, "color", entry.color);
        insert_optional_text_style(&mut item, "alignment", entry.alignment);
        affected_item_ids.push(item_id);
        items.push(item);
    }

    add_text_items_payload(
        project,
        args.project_dir,
        target_track_id,
        items,
        affected_item_ids,
    )
}

fn palmier_text_target_track_id(
    project: &VideoProject,
    args: &DirectAddTextsArgs,
) -> Result<String, CodexLocalToolError> {
    if let Some(target_track_id) = args.target_track_id.as_deref() {
        return trim_required(target_track_id, "targetTrackId");
    }
    let track_indexes = args
        .entries
        .iter()
        .filter_map(|entry| entry.track_index)
        .collect::<BTreeSet<_>>();
    let explicit_track_index_count = args
        .entries
        .iter()
        .filter(|entry| entry.track_index.is_some())
        .count();
    if explicit_track_index_count > 0 && explicit_track_index_count < args.entries.len() {
        return Err(CodexLocalToolError::InvalidArguments(
            "Mixed trackIndex usage is not supported in one add_texts call".to_string(),
        ));
    }
    match track_indexes.len() {
        0 => Ok("track-overlays".to_string()),
        1 => {
            let track_index = track_indexes.into_iter().next().expect("one track index");
            project
                .timeline
                .tracks
                .get(track_index)
                .map(|track| track.id.clone())
                .ok_or_else(|| {
                    CodexLocalToolError::InvalidArguments(format!(
                        "entries.trackIndex is out of range: {track_index}"
                    ))
                })
        }
        _ => Err(CodexLocalToolError::InvalidArguments(
            "entries must target one trackIndex per call in this compatibility layer".to_string(),
        )),
    }
}

fn insert_optional_text_style(item: &mut TimelineItem, key: &str, value: Option<String>) {
    if let Some(value) = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        item.properties.insert(key.to_string(), json!(value));
    }
}

fn generated_text_item_id(start_seconds: f64, index: usize) -> String {
    format!(
        "text-{}-{}",
        (start_seconds.max(0.0) * 1000.0).round() as u64,
        index
    )
}

fn add_captions_payload(
    project: &VideoProject,
    args: AddCaptionsArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        AddCaptionsArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        AddCaptionsArgs::Direct(args) => add_direct_captions_payload(project, args),
    }
}

fn add_direct_captions_payload(
    project: &VideoProject,
    args: DirectAddCaptionsArgs,
) -> Result<Value, CodexLocalToolError> {
    if !args.captions.is_empty() && !args.clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either captions or clipIds, not both".to_string(),
        ));
    }
    if args.captions.is_empty() && !args.clip_ids.is_empty() {
        return add_palmier_captions_payload(project, args);
    }

    let target_track_id = trim_required(
        args.target_track_id.as_deref().unwrap_or(""),
        "targetTrackId",
    )?;
    if args.captions.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "captions or clipIds must not be empty".to_string(),
        ));
    }

    let mut affected_item_ids = Vec::with_capacity(args.captions.len());
    let mut items = Vec::with_capacity(args.captions.len());
    for entry in args.captions {
        let item_id = trim_required(&entry.id, "captions.id")?;
        let text = trim_required(&entry.text, "captions.text")?;
        let label = entry
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| text.clone());
        let visual_treatment = optional_trimmed(entry.visual_treatment)
            .unwrap_or_else(|| "bold readable caption with transparent shaped backing".to_string());
        let motion = optional_trimmed(entry.motion)
            .unwrap_or_else(|| "word-group pop-in with subtle fade-out".to_string());
        let safe_zone = optional_trimmed(entry.safe_zone)
            .unwrap_or_else(|| "keep inside lower-third safe margins".to_string());
        let avoid = optional_trimmed(entry.avoid).unwrap_or_else(|| {
            "full-width opaque black slabs or centered static title-card layout".to_string()
        });

        affected_item_ids.push(item_id.clone());
        items.push(text_timeline_item(TextTimelineItemSpec {
            id: item_id,
            kind: TimelineItemKind::Caption,
            start_seconds: entry.start_seconds,
            duration_seconds: entry.duration_seconds,
            label,
            text,
            visual_treatment,
            motion,
            safe_zone,
            avoid,
        }));
    }

    add_text_items_payload(
        project,
        args.project_dir,
        target_track_id,
        items,
        affected_item_ids,
    )
}

fn add_palmier_captions_payload(
    project: &VideoProject,
    args: DirectAddCaptionsArgs,
) -> Result<Value, CodexLocalToolError> {
    validate_palmier_caption_options(&args)?;
    let target_track_id = args
        .target_track_id
        .as_deref()
        .map(|track_id| trim_required(track_id, "targetTrackId"))
        .transpose()?
        .unwrap_or_else(|| "track-captions".to_string());

    let mut affected_item_ids = Vec::new();
    let mut items = Vec::new();
    for clip_id in &args.clip_ids {
        let item_id = resolve_timeline_item_id(project, clip_id, "clipIds")?;
        let (_, item) = timeline_item_with_track(project, &item_id)?;
        let TimelineSource::Media { media_id } = &item.source else {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "clip is not media-backed: {item_id}"
            )));
        };
        let transcript = project
            .transcripts
            .iter()
            .find(|transcript| transcript.media_id == *media_id)
            .ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(format!(
                    "transcript was not found for clip: {item_id}"
                ))
            })?;
        if is_reversed(item) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "clip plays reversed, so its transcript words would run backwards: {item_id}"
            )));
        }
        let speed = timeline_item_playback_speed(item);
        let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
        let source_out = timeline_item_number_property(item, "sourceOut")
            .unwrap_or(source_in + item.duration_seconds * speed);

        for (word_index, word) in transcript.words.iter().enumerate() {
            if word.start_seconds < source_in || word.end_seconds > source_out {
                continue;
            }
            if !word.start_seconds.is_finite()
                || !word.end_seconds.is_finite()
                || word.end_seconds <= word.start_seconds
            {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "wordIndex {word_index} has invalid timing"
                )));
            }
            let caption_text = palmier_caption_text(&word.text, args.text_case.as_deref())?;
            let (start_seconds, end_seconds) = timeline_range_for_source_seconds(
                item,
                (source_in, source_out, speed),
                (word.start_seconds, word.end_seconds),
            );
            let (start_seconds, end_seconds) = (
                round_tool_seconds(start_seconds),
                round_tool_seconds(end_seconds),
            );
            let caption_id = format!("caption-{item_id}-{word_index}");
            let mut caption = text_timeline_item(TextTimelineItemSpec {
                id: caption_id.clone(),
                kind: TimelineItemKind::Caption,
                start_seconds,
                duration_seconds: round_tool_seconds(end_seconds - start_seconds),
                label: caption_text.clone(),
                text: caption_text,
                visual_treatment:
                    "bold phone-readable caption with translucent shaped backing and text shadow"
                        .to_string(),
                motion: "word-level pop-in with short fade and no long static hold".to_string(),
                safe_zone:
                    "keep inside lower-third safe margins and away from faces or product details"
                        .to_string(),
                avoid: "full-width opaque black slabs, tiny text, and default-font template look"
                    .to_string(),
            });
            caption
                .properties
                .insert("transcriptId".to_string(), json!(transcript.id));
            caption
                .properties
                .insert("wordIndex".to_string(), json!(word_index));
            caption.properties.insert(
                "sourceIn".to_string(),
                json!(round_tool_seconds(word.start_seconds)),
            );
            caption.properties.insert(
                "sourceOut".to_string(),
                json!(round_tool_seconds(word.end_seconds)),
            );
            insert_optional_text_style(&mut caption, "language", args.language.clone());
            insert_optional_text_style(&mut caption, "fontName", args.font_name.clone());
            if let Some(font_size) = args.font_size {
                caption
                    .properties
                    .insert("fontSize".to_string(), json!(font_size));
            }
            insert_optional_text_style(&mut caption, "color", args.color.clone());
            if args.center_x.is_some() || args.center_y.is_some() {
                caption.properties.insert(
                    "transform".to_string(),
                    json!({
                        "centerX": args.center_x.unwrap_or(0.5),
                        "centerY": args.center_y.unwrap_or(0.9)
                    }),
                );
            }
            if let Some(censor_profanity) = args.censor_profanity {
                caption
                    .properties
                    .insert("censorProfanity".to_string(), json!(censor_profanity));
            }
            affected_item_ids.push(caption_id);
            items.push(caption);
        }
    }

    if items.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "no transcript words overlap the requested clipIds".to_string(),
        ));
    }

    add_text_items_payload(
        project,
        args.project_dir,
        target_track_id,
        items,
        affected_item_ids,
    )
}

fn validate_palmier_caption_options(
    args: &DirectAddCaptionsArgs,
) -> Result<(), CodexLocalToolError> {
    if let Some(language) = args.language.as_deref() {
        trim_required(language, "language")?;
    }
    if let Some(text_case) = args.text_case.as_deref() {
        match text_case.trim() {
            "auto" | "upper" | "lower" => {}
            other => {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "unsupported textCase: {other}"
                )));
            }
        }
    }
    Ok(())
}

fn palmier_caption_text(
    text: &str,
    text_case: Option<&str>,
) -> Result<String, CodexLocalToolError> {
    let text = trim_required(text, "caption word text")?;
    match text_case.unwrap_or("auto").trim() {
        "auto" => Ok(text),
        "upper" => Ok(text.to_uppercase()),
        "lower" => Ok(text.to_lowercase()),
        other => Err(CodexLocalToolError::InvalidArguments(format!(
            "unsupported textCase: {other}"
        ))),
    }
}

fn add_text_items_payload(
    project: &VideoProject,
    project_dir: Option<String>,
    target_track_id: String,
    items: Vec<TimelineItem>,
    affected_item_ids: Vec<String>,
) -> Result<Value, CodexLocalToolError> {
    let item_payloads = items.iter().map(text_item_payload).collect::<Vec<_>>();
    let action = ProjectAction::AddItems {
        target_track_id,
        items,
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("items".to_string(), json!(item_payloads));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn text_item_payload(item: &TimelineItem) -> Value {
    json!({
        "itemId": item.id,
        "kind": match item.kind {
            TimelineItemKind::Caption => "caption",
            _ => "overlay",
        },
        "startSeconds": item.start_seconds,
        "durationSeconds": item.duration_seconds,
        "text": match &item.source {
            TimelineSource::Text { text } => text.as_str(),
            _ => ""
        }
    })
}

struct TextTimelineItemSpec {
    id: String,
    kind: TimelineItemKind,
    start_seconds: f64,
    duration_seconds: f64,
    label: String,
    text: String,
    visual_treatment: String,
    motion: String,
    safe_zone: String,
    avoid: String,
}

fn text_timeline_item(spec: TextTimelineItemSpec) -> TimelineItem {
    let TextTimelineItemSpec {
        id,
        kind,
        start_seconds,
        duration_seconds,
        label,
        text,
        visual_treatment,
        motion,
        safe_zone,
        avoid,
    } = spec;
    TimelineItem {
        id,
        kind,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Text { text: text.clone() },
        label,
        properties: BTreeMap::from([
            ("text".to_string(), json!(text)),
            ("visualTreatment".to_string(), json!(visual_treatment)),
            ("motion".to_string(), json!(motion)),
            ("safeZone".to_string(), json!(safe_zone)),
            ("avoid".to_string(), json!(avoid)),
        ]),
    }
}

fn trim_required(value: &str, field: &str) -> Result<String, CodexLocalToolError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{field} must not be blank"
        )));
    }
    Ok(trimmed.to_string())
}

fn optional_trimmed(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn remove_clips_payload(
    project: &VideoProject,
    args: RemoveClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        RemoveClipsArgs::Direct(args) => args,
        RemoveClipsArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    if args.item_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "itemIds must not be empty".to_string(),
        ));
    }
    if args
        .item_ids
        .iter()
        .any(|item_id| item_id.trim().is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "itemIds must not contain blank ids".to_string(),
        ));
    }

    let resolved_item_ids = args
        .item_ids
        .iter()
        .map(|item_id| resolve_timeline_item_id(project, item_id, "itemIds"))
        .collect::<Result<Vec<_>, _>>()?;
    let expanded_item_ids = expand_linked_timeline_item_ids(project, &resolved_item_ids);

    let action = ProjectAction::RemoveItems {
        item_ids: expanded_item_ids.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(expanded_item_ids));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn expand_linked_timeline_item_ids(project: &VideoProject, item_ids: &[String]) -> Vec<String> {
    let requested = item_ids.iter().cloned().collect::<BTreeSet<_>>();
    let linked_groups = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .filter(|item| requested.contains(&item.id))
        .filter_map(timeline_item_link_group_id)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();

    let mut expanded = Vec::new();
    let mut seen = BTreeSet::new();
    for item in project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
    {
        let linked = timeline_item_link_group_id(item)
            .map(|group_id| linked_groups.contains(group_id))
            .unwrap_or(false);
        if (requested.contains(&item.id) || linked) && seen.insert(item.id.clone()) {
            expanded.push(item.id.clone());
        }
    }
    expanded
}

fn timeline_item_link_group_id(item: &TimelineItem) -> Option<&str> {
    item.properties
        .get("linkGroupId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn resolve_timeline_item_id(
    project: &VideoProject,
    raw_id: &str,
    field: &str,
) -> Result<String, CodexLocalToolError> {
    let id = raw_id.trim();
    if id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{field} must not contain blank ids"
        )));
    }

    let mut prefix_matches = Vec::new();
    for track in &project.timeline.tracks {
        for item in &track.items {
            if item.id == id {
                return Ok(item.id.clone());
            }
            if item.id.starts_with(id) {
                prefix_matches.push(item.id.clone());
            }
        }
    }

    match prefix_matches.len() {
        1 => Ok(prefix_matches.remove(0)),
        0 => Err(CodexLocalToolError::InvalidArguments(format!(
            "{field} did not match a timeline item: {id}"
        ))),
        _ => Err(CodexLocalToolError::InvalidArguments(format!(
            "{field} is ambiguous: {id} matches {} timeline items",
            prefix_matches.len()
        ))),
    }
}

fn move_clips_payload(
    project: &VideoProject,
    args: MoveClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        MoveClipsArgs::Direct(args) => args,
        MoveClipsArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };

    if args.moves.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "moves must not be empty".to_string(),
        ));
    }

    let mut affected_item_ids = Vec::with_capacity(args.moves.len());
    let mut move_payloads = Vec::with_capacity(args.moves.len());
    let mut moves = Vec::with_capacity(args.moves.len());
    let mut seen_moves = BTreeSet::new();
    for entry in args.moves {
        let item_id = resolve_timeline_item_id(project, &entry.item_id, "moves.itemId")?;
        let (current_track, current_item) = timeline_item_with_track(project, &item_id)?;
        let target_track_id =
            resolve_move_target_track_id(project, current_track.id.as_str(), &entry)?;
        let start_seconds =
            resolve_move_start_seconds(project, current_item.start_seconds, &entry)?;
        push_move_payload(
            &mut affected_item_ids,
            &mut move_payloads,
            &mut moves,
            &mut seen_moves,
            item_id.clone(),
            target_track_id,
            start_seconds,
        );
        if entry.to_frame.is_some() || entry.start_seconds.is_some() {
            let delta_seconds = start_seconds - current_item.start_seconds;
            for partner in linked_move_partners(project, &item_id, delta_seconds)? {
                push_move_payload(
                    &mut affected_item_ids,
                    &mut move_payloads,
                    &mut moves,
                    &mut seen_moves,
                    partner.item_id,
                    partner.target_track_id,
                    partner.start_seconds,
                );
            }
        }
    }

    let action = ProjectAction::MoveItems { moves };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("moves".to_string(), json!(move_payloads));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

#[derive(Debug, Clone)]
struct LayoutSlotSpec {
    id: &'static str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    z: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyLayoutMode {
    MediaRefs,
    ClipIds,
}

#[derive(Debug, Clone, Copy)]
struct LayoutAnchor {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone)]
struct LayoutPlacement {
    transform: ProjectActionVisualTransform,
    crop_top: f64,
    crop_right: f64,
    crop_bottom: f64,
    crop_left: f64,
}

fn apply_layout_payload(
    project: &VideoProject,
    args: ApplyLayoutArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        ApplyLayoutArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        ApplyLayoutArgs::Direct(args) => apply_layout_direct_payload(project, args),
    }
}

fn apply_layout_direct_payload(
    project: &VideoProject,
    args: DirectApplyLayoutArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.slots.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "apply_layout needs a non-empty slots array".to_string(),
        ));
    }
    let layout = args.layout.trim();
    let layout_slots = layout_slot_specs(layout)?;
    let fit = match args.fit.as_deref().unwrap_or("fill").trim() {
        "fill" => "fill",
        "fit" => "fit",
        other => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "invalid fit '{other}'. Valid: fill, fit"
            )))
        }
    };
    let fps = project_fps(project)?;
    let canvas_aspect =
        f64::from(project.render_settings.width) / f64::from(project.render_settings.height.max(1));
    let slot_by_id = layout_slots
        .iter()
        .map(|slot| (slot.id, slot))
        .collect::<BTreeMap<_, _>>();
    let mut seen_slots = BTreeSet::new();
    let mut seen_clip_ids = BTreeSet::new();
    let mut uses_media_refs = false;
    let mut uses_clip_ids = false;
    let mut entries = Vec::with_capacity(args.slots.len());
    for (index, entry) in args.slots.iter().enumerate() {
        let slot_name = trim_required(&entry.slot, "slots.slot")?;
        let slot = slot_by_id.get(slot_name.as_str()).ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "slots[{index}]: '{slot_name}' is not a slot of layout '{layout}'"
            ))
        })?;
        if !seen_slots.insert(slot_name.to_string()) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "slots[{index}]: duplicate slot '{slot_name}'"
            )));
        }
        let has_media_ref = entry
            .media_id
            .as_deref()
            .is_some_and(|media_id| !media_id.trim().is_empty());
        let has_clip_ids = entry
            .clip_ids
            .as_ref()
            .is_some_and(|clip_ids| clip_ids.iter().any(|clip_id| !clip_id.trim().is_empty()));
        if has_media_ref == has_clip_ids {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "slots[{index}]: provide exactly one of mediaRef or clipIds"
            )));
        }
        if let Some(clip_ids) = entry.clip_ids.as_ref() {
            if clip_ids.is_empty() {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "slots[{index}]: clipIds must not be empty"
                )));
            }
            for clip_id in clip_ids {
                let item_id = resolve_timeline_item_id(project, clip_id, "slots.clipIds")?;
                if !seen_clip_ids.insert(item_id.clone()) {
                    return Err(CodexLocalToolError::InvalidArguments(format!(
                        "slots[{index}]: clip '{item_id}' is assigned to more than one slot"
                    )));
                }
            }
        }
        uses_media_refs |= has_media_ref;
        uses_clip_ids |= has_clip_ids;
        entries.push(((*slot).clone(), entry, resolve_layout_anchor(entry, index)?));
    }
    let missing_slots = layout_slots
        .iter()
        .filter(|slot| !seen_slots.contains(slot.id))
        .map(|slot| slot.id)
        .collect::<Vec<_>>();
    if !missing_slots.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "layout '{layout}' needs every slot filled. Missing: {}",
            missing_slots.join(", ")
        )));
    }
    if uses_media_refs && uses_clip_ids {
        return Err(CodexLocalToolError::InvalidArguments(
            "apply_layout: don't mix mediaRef and clipIds".to_string(),
        ));
    }
    let mode = if uses_media_refs {
        ApplyLayoutMode::MediaRefs
    } else {
        ApplyLayoutMode::ClipIds
    };

    let mut actions = Vec::new();
    let mut affected_item_ids = Vec::new();
    let mut created_track_ids = Vec::new();
    let mut created_visual_item_ids = Vec::new();
    let mut created_audio_item_ids = Vec::new();
    let mut slot_updates = Vec::new();

    match mode {
        ApplyLayoutMode::ClipIds => {
            validate_layout_clip_slot_overlap(project, &entries)?;
            for (slot, entry, anchor) in &entries {
                for clip_id in entry.clip_ids.as_ref().expect("clipIds mode") {
                    let item_id = resolve_timeline_item_id(project, clip_id, "slots.clipIds")?;
                    let (_, item) = timeline_item_with_track(project, &item_id)?;
                    ensure_layout_item_is_visual(item, slot.id)?;
                    let media = layout_item_media(project, item)?;
                    let placement =
                        layout_placement_for_media(media, slot, fit, *anchor, canvas_aspect);
                    push_layout_actions(&mut actions, &item_id, &placement);
                    affected_item_ids.push(item_id.clone());
                    slot_updates.push(layout_slot_update_payload(slot.id, &item_id, &placement));
                }
            }
        }
        ApplyLayoutMode::MediaRefs => {
            let start_frame = args.start_frame.unwrap_or(0);
            let duration_frames = args.duration_frames.ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(
                    "apply_layout placing new clips requires durationFrames >= 1".to_string(),
                )
            })?;
            if duration_frames == 0 {
                return Err(CodexLocalToolError::InvalidArguments(
                    "apply_layout placing new clips requires durationFrames >= 1".to_string(),
                ));
            }
            let start_seconds = frames_to_seconds(start_frame, fps);
            let duration_seconds = frames_to_seconds(duration_frames, fps);
            let mut last_created_track_id: Option<String> = None;
            let mut layout_audio_track_id = existing_audio_track_id(project);
            let mut ordered_layout_slots = layout_slots.clone();
            ordered_layout_slots.sort_by_key(|slot| slot.z);
            for slot in ordered_layout_slots {
                let (_, entry, anchor) = entries
                    .iter()
                    .find(|(entry_slot, _, _)| entry_slot.id == slot.id)
                    .expect("slot was validated");
                let media_id =
                    trim_required(entry.media_id.as_deref().unwrap_or(""), "slots.mediaRef")?;
                let media = project
                    .media
                    .iter()
                    .find(|media| media.id == media_id)
                    .ok_or_else(|| {
                        CodexLocalToolError::InvalidArguments(format!(
                            "slot '{}': mediaRef was not found: {media_id}",
                            slot.id
                        ))
                    })?;
                if !matches!(media.kind, MediaKind::Video | MediaKind::Image) {
                    return Err(CodexLocalToolError::InvalidArguments(format!(
                        "slot '{}': asset {media_id} is {:?}; layout slots take video or image",
                        slot.id, media.kind
                    )));
                }
                let track_id = next_layout_track_id(project, slot.id);
                actions.push(ProjectAction::CreateTrack {
                    track: TimelineTrack::empty(&track_id, "Palmier Layout", TrackKind::Video),
                    after_track_id: last_created_track_id.clone(),
                });
                created_track_ids.push(track_id.clone());
                last_created_track_id = Some(track_id.clone());
                let item_id = layout_item_id(slot.id, start_frame);
                let placement =
                    layout_placement_for_media(media, &slot, fit, *anchor, canvas_aspect);
                let (_, mut item) = direct_media_clip_item(
                    project,
                    DirectMediaClipEntry {
                        id: item_id.clone(),
                        media_id: media_id.to_string(),
                        start_seconds: None,
                        duration_seconds,
                        source_in: None,
                        source_out: None,
                        label: Some(
                            media
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("{} layout", slot.id)),
                        ),
                        opacity: None,
                    },
                    start_seconds,
                )?;
                if media.kind == MediaKind::Video {
                    let link_group_id = format!("layout-link-{}-{start_frame}", slot.id);
                    item.properties
                        .insert("linkGroupId".to_string(), json!(link_group_id.clone()));
                    let audio_item_id = format!("{item_id}-audio");
                    let (_, mut audio_item) = direct_media_clip_item(
                        project,
                        DirectMediaClipEntry {
                            id: audio_item_id.clone(),
                            media_id: media_id.to_string(),
                            start_seconds: None,
                            duration_seconds,
                            source_in: None,
                            source_out: None,
                            label: Some(format!("{} audio", item.label)),
                            opacity: None,
                        },
                        start_seconds,
                    )?;
                    audio_item.kind = TimelineItemKind::AudioClip;
                    audio_item
                        .properties
                        .insert("linkGroupId".to_string(), json!(link_group_id));
                    audio_item
                        .properties
                        .insert("sourceClipType".to_string(), json!("audio"));
                    let audio_track_id = if let Some(audio_track_id) = layout_audio_track_id.clone()
                    {
                        audio_track_id
                    } else {
                        let audio_track_id = next_palmier_track_id(project, "layout-audio");
                        actions.push(ProjectAction::CreateTrack {
                            track: TimelineTrack::empty(
                                &audio_track_id,
                                "Palmier Layout Audio",
                                TrackKind::Audio,
                            ),
                            after_track_id: None,
                        });
                        created_track_ids.push(audio_track_id.clone());
                        layout_audio_track_id = Some(audio_track_id.clone());
                        audio_track_id
                    };
                    actions.push(ProjectAction::AddItems {
                        target_track_id: track_id.clone(),
                        items: vec![item],
                    });
                    push_layout_actions(&mut actions, &item_id, &placement);
                    affected_item_ids.push(item_id.clone());
                    created_visual_item_ids.push(item_id.clone());
                    slot_updates.push(layout_slot_update_payload(slot.id, &item_id, &placement));
                    actions.push(ProjectAction::AddItems {
                        target_track_id: audio_track_id,
                        items: vec![audio_item],
                    });
                    affected_item_ids.push(audio_item_id.clone());
                    created_audio_item_ids.push(audio_item_id);
                    continue;
                }
                actions.push(ProjectAction::AddItems {
                    target_track_id: track_id,
                    items: vec![item],
                });
                push_layout_actions(&mut actions, &item_id, &placement);
                affected_item_ids.push(item_id.clone());
                created_visual_item_ids.push(item_id.clone());
                slot_updates.push(layout_slot_update_payload(slot.id, &item_id, &placement));
            }
        }
    }

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("layout".to_string(), json!(layout));
        object.insert("fit".to_string(), json!(fit));
        object.insert(
            "mode".to_string(),
            json!(match mode {
                ApplyLayoutMode::MediaRefs => "mediaRef",
                ApplyLayoutMode::ClipIds => "clipIds",
            }),
        );
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("slotUpdates".to_string(), json!(slot_updates));
        if !created_track_ids.is_empty() {
            object.insert("createdTrackIds".to_string(), json!(created_track_ids));
        }
        if !created_visual_item_ids.is_empty() {
            object.insert(
                "createdVisualItemIds".to_string(),
                json!(created_visual_item_ids),
            );
        }
        if !created_audio_item_ids.is_empty() {
            object.insert(
                "createdAudioItemIds".to_string(),
                json!(created_audio_item_ids),
            );
        }
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.inspect_timeline"),
        );
    }
    Ok(payload)
}

fn layout_slot_specs(layout: &str) -> Result<Vec<LayoutSlotSpec>, CodexLocalToolError> {
    const PIP_INSET: f64 = 0.28;
    const PIP_MARGIN: f64 = 0.035;
    let slots = match layout {
        "full" => vec![LayoutSlotSpec {
            id: "main",
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            z: 0,
        }],
        "side_by_side" => vec![
            LayoutSlotSpec {
                id: "left",
                x: 0.0,
                y: 0.0,
                width: 0.5,
                height: 1.0,
                z: 0,
            },
            LayoutSlotSpec {
                id: "right",
                x: 0.5,
                y: 0.0,
                width: 0.5,
                height: 1.0,
                z: 1,
            },
        ],
        "top_bottom" => vec![
            LayoutSlotSpec {
                id: "top",
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 0.5,
                z: 0,
            },
            LayoutSlotSpec {
                id: "bottom",
                x: 0.0,
                y: 0.5,
                width: 1.0,
                height: 0.5,
                z: 1,
            },
        ],
        "pip_bottom_right" => {
            pip_layout_slots(1.0 - PIP_MARGIN - PIP_INSET, 1.0 - PIP_MARGIN - PIP_INSET)
        }
        "pip_bottom_left" => pip_layout_slots(PIP_MARGIN, 1.0 - PIP_MARGIN - PIP_INSET),
        "pip_top_right" => pip_layout_slots(1.0 - PIP_MARGIN - PIP_INSET, PIP_MARGIN),
        "pip_top_left" => pip_layout_slots(PIP_MARGIN, PIP_MARGIN),
        "grid_2x2" => vec![
            LayoutSlotSpec {
                id: "top_left",
                x: 0.0,
                y: 0.0,
                width: 0.5,
                height: 0.5,
                z: 0,
            },
            LayoutSlotSpec {
                id: "top_right",
                x: 0.5,
                y: 0.0,
                width: 0.5,
                height: 0.5,
                z: 1,
            },
            LayoutSlotSpec {
                id: "bottom_left",
                x: 0.0,
                y: 0.5,
                width: 0.5,
                height: 0.5,
                z: 2,
            },
            LayoutSlotSpec {
                id: "bottom_right",
                x: 0.5,
                y: 0.5,
                width: 0.5,
                height: 0.5,
                z: 3,
            },
        ],
        "main_sidebar" => vec![
            LayoutSlotSpec {
                id: "main",
                x: 0.0,
                y: 0.0,
                width: 0.7,
                height: 1.0,
                z: 0,
            },
            LayoutSlotSpec {
                id: "sidebar",
                x: 0.7,
                y: 0.0,
                width: 0.3,
                height: 1.0,
                z: 1,
            },
        ],
        "three_up" => {
            let third = 1.0 / 3.0;
            vec![
                LayoutSlotSpec {
                    id: "left",
                    x: 0.0,
                    y: 0.0,
                    width: third,
                    height: 1.0,
                    z: 0,
                },
                LayoutSlotSpec {
                    id: "center",
                    x: third,
                    y: 0.0,
                    width: third,
                    height: 1.0,
                    z: 1,
                },
                LayoutSlotSpec {
                    id: "right",
                    x: third * 2.0,
                    y: 0.0,
                    width: third,
                    height: 1.0,
                    z: 2,
                },
            ]
        }
        other => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "unknown layout '{other}'"
            )))
        }
    };
    Ok(slots)
}

fn pip_layout_slots(inset_x: f64, inset_y: f64) -> Vec<LayoutSlotSpec> {
    vec![
        LayoutSlotSpec {
            id: "main",
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            z: 0,
        },
        LayoutSlotSpec {
            id: "inset",
            x: inset_x,
            y: inset_y,
            width: 0.28,
            height: 0.28,
            z: 1,
        },
    ]
}

fn resolve_layout_anchor(
    entry: &ApplyLayoutSlotArg,
    index: usize,
) -> Result<LayoutAnchor, CodexLocalToolError> {
    let mut anchor = match entry.anchor.as_deref().unwrap_or("center").trim() {
        "center" => LayoutAnchor { x: 0.5, y: 0.5 },
        "top" => LayoutAnchor { x: 0.5, y: 0.0 },
        "bottom" => LayoutAnchor { x: 0.5, y: 1.0 },
        "left" => LayoutAnchor { x: 0.0, y: 0.5 },
        "right" => LayoutAnchor { x: 1.0, y: 0.5 },
        "top_left" => LayoutAnchor { x: 0.0, y: 0.0 },
        "top_right" => LayoutAnchor { x: 1.0, y: 0.0 },
        "bottom_left" => LayoutAnchor { x: 0.0, y: 1.0 },
        "bottom_right" => LayoutAnchor { x: 1.0, y: 1.0 },
        other => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "slots[{index}]: invalid anchor '{other}'"
            )))
        }
    };
    for (field, value) in [("anchorX", entry.anchor_x), ("anchorY", entry.anchor_y)] {
        if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "slots[{index}]: {field} must be between 0 and 1"
            )));
        }
    }
    if let Some(anchor_x) = entry.anchor_x {
        anchor.x = anchor_x;
    }
    if let Some(anchor_y) = entry.anchor_y {
        anchor.y = anchor_y;
    }
    Ok(anchor)
}

fn validate_layout_clip_slot_overlap(
    project: &VideoProject,
    entries: &[(LayoutSlotSpec, &ApplyLayoutSlotArg, LayoutAnchor)],
) -> Result<(), CodexLocalToolError> {
    let mut ranges_by_track: BTreeMap<String, Vec<(&str, f64, f64)>> = BTreeMap::new();
    let mut intervals_by_slot: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
    for (slot, entry, _) in entries {
        for clip_id in entry.clip_ids.as_ref().expect("clipIds mode") {
            let item_id = resolve_timeline_item_id(project, clip_id, "slots.clipIds")?;
            let (track, item) = timeline_item_with_track(project, &item_id)?;
            ensure_layout_item_is_visual(item, slot.id)?;
            let start = item.start_seconds;
            let end = item.start_seconds + item.duration_seconds;
            for (other_slot, other_start, other_end) in
                ranges_by_track.get(&track.id).into_iter().flatten()
            {
                if *other_slot != slot.id && start < *other_end && *other_start < end {
                    return Err(CodexLocalToolError::InvalidArguments(format!(
                        "clips in slots '{other_slot}' and '{}' are on the same track and their times overlap",
                        slot.id
                    )));
                }
            }
            ranges_by_track
                .entry(track.id.clone())
                .or_default()
                .push((slot.id, start, end));
            intervals_by_slot
                .entry(slot.id)
                .or_default()
                .push((start, end));
        }
    }
    if entries.len() > 1 {
        let candidates = intervals_by_slot
            .values()
            .flat_map(|intervals| intervals.iter().map(|(start, _)| *start))
            .collect::<Vec<_>>();
        let coincides = candidates.into_iter().any(|candidate| {
            intervals_by_slot.values().all(|intervals| {
                intervals
                    .iter()
                    .any(|(start, end)| *start <= candidate && candidate < *end)
            })
        });
        if !coincides {
            return Err(CodexLocalToolError::InvalidArguments(
                "the selected clips never play at the same time".to_string(),
            ));
        }
    }
    Ok(())
}

fn ensure_layout_item_is_visual(
    item: &TimelineItem,
    slot_id: &str,
) -> Result<(), CodexLocalToolError> {
    if !matches!(
        item.kind,
        TimelineItemKind::VideoClip
            | TimelineItemKind::ImageClip
            | TimelineItemKind::LottieClip
            | TimelineItemKind::GeneratedClip
    ) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "slot '{slot_id}': clip {} is not a visual clip",
            item.id
        )));
    }
    Ok(())
}

fn layout_item_media<'a>(
    project: &'a VideoProject,
    item: &TimelineItem,
) -> Result<&'a MediaAsset, CodexLocalToolError> {
    let TimelineSource::Media { media_id } = &item.source else {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "clip {} is not backed by media",
            item.id
        )));
    };
    project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!("media was not found: {media_id}"))
        })
}

fn layout_placement_for_media(
    media: &MediaAsset,
    slot: &LayoutSlotSpec,
    fit: &str,
    anchor: LayoutAnchor,
    canvas_aspect: f64,
) -> LayoutPlacement {
    let source_aspect = media
        .width
        .zip(media.height)
        .filter(|(_, height)| *height > 0)
        .map(|(width, height)| f64::from(width) / f64::from(height));
    let mut x = slot.x;
    let mut y = slot.y;
    let mut width = slot.width;
    let mut height = slot.height;
    let mut crop_top: f64 = 0.0;
    let mut crop_right: f64 = 0.0;
    let mut crop_bottom: f64 = 0.0;
    let mut crop_left: f64 = 0.0;

    if let Some(source_aspect) = source_aspect.filter(|aspect| *aspect > 0.0) {
        let slot_pixel_aspect = if slot.height > 0.0 {
            (slot.width / slot.height) * canvas_aspect
        } else {
            canvas_aspect
        };
        if fit == "fill" {
            if source_aspect > slot_pixel_aspect {
                let visible_width = (slot_pixel_aspect / source_aspect).clamp(0.0, 1.0);
                let crop = 1.0 - visible_width;
                crop_left = crop * anchor.x;
                crop_right = crop * (1.0 - anchor.x);
            } else if source_aspect < slot_pixel_aspect {
                let visible_height = (source_aspect / slot_pixel_aspect).clamp(0.0, 1.0);
                let crop = 1.0 - visible_height;
                crop_top = crop * anchor.y;
                crop_bottom = crop * (1.0 - anchor.y);
            }
        } else {
            let relative_aspect = source_aspect / canvas_aspect;
            if relative_aspect * slot.height <= slot.width {
                height = slot.height;
                width = relative_aspect * slot.height;
            } else {
                width = slot.width;
                height = slot.width / relative_aspect;
            }
            x = slot.x + (slot.width - width) * anchor.x;
            y = slot.y + (slot.height - height) * anchor.y;
        }
    }

    LayoutPlacement {
        transform: ProjectActionVisualTransform {
            center_x: Some(round_tool_seconds(x + width / 2.0)),
            center_y: Some(round_tool_seconds(y + height / 2.0)),
            width: Some(round_tool_seconds(width)),
            height: Some(round_tool_seconds(height)),
            flip_horizontal: None,
            flip_vertical: None,
        },
        crop_top: round_tool_seconds(crop_top),
        crop_right: round_tool_seconds(crop_right),
        crop_bottom: round_tool_seconds(crop_bottom),
        crop_left: round_tool_seconds(crop_left),
    }
}

fn push_layout_actions(
    actions: &mut Vec<ProjectAction>,
    item_id: &str,
    placement: &LayoutPlacement,
) {
    actions.push(ProjectAction::UpdateVisualClipTransform {
        item_id: item_id.to_string(),
        transform: placement.transform.clone(),
    });
    for property in [
        ProjectActionKeyframeProperty::PositionX,
        ProjectActionKeyframeProperty::PositionY,
        ProjectActionKeyframeProperty::Scale,
        ProjectActionKeyframeProperty::ScaleX,
        ProjectActionKeyframeProperty::ScaleY,
        ProjectActionKeyframeProperty::RotationDegrees,
    ] {
        actions.push(ProjectAction::SetItemKeyframes {
            item_id: item_id.to_string(),
            property,
            keyframes: Vec::new(),
        });
    }
    push_layout_crop_actions(actions, item_id, placement);
}

fn push_layout_crop_actions(
    actions: &mut Vec<ProjectAction>,
    item_id: &str,
    placement: &LayoutPlacement,
) {
    for (property, value) in [
        (ProjectActionKeyframeProperty::CropTop, placement.crop_top),
        (
            ProjectActionKeyframeProperty::CropRight,
            placement.crop_right,
        ),
        (
            ProjectActionKeyframeProperty::CropBottom,
            placement.crop_bottom,
        ),
        (ProjectActionKeyframeProperty::CropLeft, placement.crop_left),
    ] {
        actions.push(ProjectAction::SetItemKeyframes {
            item_id: item_id.to_string(),
            property,
            keyframes: vec![ProjectActionKeyframe {
                at_seconds: 0.0,
                value,
                easing: Some("hold".to_string()),
            }],
        });
    }
}

fn layout_slot_update_payload(slot: &str, item_id: &str, placement: &LayoutPlacement) -> Value {
    json!({
        "slot": slot,
        "itemId": item_id,
        "transform": placement.transform,
        "crop": {
            "top": placement.crop_top,
            "right": placement.crop_right,
            "bottom": placement.crop_bottom,
            "left": placement.crop_left
        }
    })
}

fn next_layout_track_id(project: &VideoProject, slot: &str) -> String {
    let mut index = 1;
    loop {
        let candidate = format!("track-palmier-layout-{slot}-{index}");
        if !project
            .timeline
            .tracks
            .iter()
            .any(|track| track.id == candidate)
        {
            return candidate;
        }
        index += 1;
    }
}

fn layout_item_id(slot: &str, start_frame: u64) -> String {
    format!("layout-{slot}-{start_frame}")
}

fn link_clips_payload(
    project: &VideoProject,
    args: LinkClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        LinkClipsArgs::Direct(args) => args,
        LinkClipsArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    if args.clip_ids.len() < 2 {
        return Err(CodexLocalToolError::InvalidArguments(
            "clipIds must include at least two ids".to_string(),
        ));
    }

    let affected_item_ids = resolve_unique_timeline_item_ids(project, &args.clip_ids, "clipIds")?;
    if affected_item_ids.len() < 2 {
        return Err(CodexLocalToolError::InvalidArguments(
            "link_clips requires at least two distinct timeline items".to_string(),
        ));
    }
    let link_group_id = args
        .link_group_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("link-{}", affected_item_ids.join("-")));
    let updates = affected_item_ids
        .iter()
        .map(|item_id| ProjectActionItemPropertiesUpdate {
            item_id: item_id.clone(),
            set: BTreeMap::from([("linkGroupId".to_string(), json!(link_group_id))]),
            remove: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![ProjectAction::UpdateItemProperties { updates }],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("linkGroupId".to_string(), json!(link_group_id));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn unlink_clips_payload(
    project: &VideoProject,
    args: UnlinkClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        UnlinkClipsArgs::Direct(args) => args,
        UnlinkClipsArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    if args.clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "clipIds must not be empty".to_string(),
        ));
    }

    let resolved_item_ids = resolve_unique_timeline_item_ids(project, &args.clip_ids, "clipIds")?;
    let affected_item_ids = expand_linked_timeline_item_ids(project, &resolved_item_ids)
        .into_iter()
        .filter(|item_id| {
            timeline_item_by_id(project, item_id)
                .and_then(timeline_item_link_group_id)
                .is_some()
        })
        .collect::<Vec<_>>();
    if affected_item_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "unlink_clips did not find linked timeline items".to_string(),
        ));
    }
    let updates = affected_item_ids
        .iter()
        .map(|item_id| ProjectActionItemPropertiesUpdate {
            item_id: item_id.clone(),
            set: BTreeMap::new(),
            remove: vec!["linkGroupId".to_string()],
        })
        .collect::<Vec<_>>();
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![ProjectAction::UpdateItemProperties { updates }],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn resolve_unique_timeline_item_ids(
    project: &VideoProject,
    item_ids: &[String],
    field: &str,
) -> Result<Vec<String>, CodexLocalToolError> {
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::new();
    for item_id in item_ids {
        let item_id = resolve_timeline_item_id(project, item_id, field)?;
        if seen.insert(item_id.clone()) {
            resolved.push(item_id);
        }
    }
    Ok(resolved)
}

fn timeline_item_by_id<'a>(project: &'a VideoProject, item_id: &str) -> Option<&'a TimelineItem> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == item_id)
}

struct LinkedMovePartner {
    item_id: String,
    target_track_id: String,
    start_seconds: f64,
}

fn push_move_payload(
    affected_item_ids: &mut Vec<String>,
    move_payloads: &mut Vec<Value>,
    moves: &mut Vec<ProjectActionMove>,
    seen_moves: &mut BTreeSet<String>,
    item_id: String,
    target_track_id: String,
    start_seconds: f64,
) {
    if !seen_moves.insert(item_id.clone()) {
        return;
    }
    affected_item_ids.push(item_id.clone());
    move_payloads.push(json!({
        "itemId": item_id.clone(),
        "targetTrackId": target_track_id.clone(),
        "startSeconds": start_seconds
    }));
    moves.push(ProjectActionMove {
        item_id,
        target_track_id,
        start_seconds,
    });
}

fn linked_move_partners(
    project: &VideoProject,
    item_id: &str,
    delta_seconds: f64,
) -> Result<Vec<LinkedMovePartner>, CodexLocalToolError> {
    if delta_seconds.abs() <= f64::EPSILON {
        return Ok(Vec::new());
    }
    let (_, lead_item) = timeline_item_with_track(project, item_id)?;
    let Some(group_id) = timeline_item_link_group_id(lead_item) else {
        return Ok(Vec::new());
    };
    let mut partners = Vec::new();
    for track in &project.timeline.tracks {
        for item in &track.items {
            if item.id == item_id || timeline_item_link_group_id(item) != Some(group_id) {
                continue;
            }
            partners.push(LinkedMovePartner {
                item_id: item.id.clone(),
                target_track_id: track.id.clone(),
                start_seconds: (item.start_seconds + delta_seconds).max(0.0),
            });
        }
    }
    Ok(partners)
}

fn resolve_move_target_track_id(
    project: &VideoProject,
    current_track_id: &str,
    entry: &MoveClipEntry,
) -> Result<String, CodexLocalToolError> {
    if let Some(target_track_id) = entry.target_track_id.as_deref() {
        return trim_required(target_track_id, "moves.targetTrackId");
    }
    if let Some(to_track) = entry.to_track {
        return project
            .timeline
            .tracks
            .get(to_track)
            .map(|track| track.id.clone())
            .ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(format!(
                    "moves.toTrack is out of range: {to_track}"
                ))
            });
    }
    Ok(current_track_id.to_string())
}

fn resolve_move_start_seconds(
    project: &VideoProject,
    current_start_seconds: f64,
    entry: &MoveClipEntry,
) -> Result<f64, CodexLocalToolError> {
    if let Some(start_seconds) = entry.start_seconds {
        if !start_seconds.is_finite() {
            return Err(CodexLocalToolError::InvalidArguments(
                "moves.startSeconds must be finite".to_string(),
            ));
        }
        return Ok(start_seconds);
    }
    if let Some(to_frame) = entry.to_frame {
        return Ok(frames_to_seconds(to_frame, project_fps(project)?));
    }
    Ok(current_start_seconds)
}

fn split_clips_payload(
    project: &VideoProject,
    args: SplitClipsArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = args.project_dir.clone();
    if args.splits.is_empty() && args.frames.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "splits or trackIndex/frames must not be empty".to_string(),
        ));
    }
    if !args.splits.is_empty() && (args.track_index.is_some() || !args.frames.is_empty()) {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either splits or trackIndex/frames, not both".to_string(),
        ));
    }
    if args.splits.is_empty() && args.track_index.is_none() {
        return Err(CodexLocalToolError::InvalidArguments(
            "trackIndex is required when frames are provided".to_string(),
        ));
    }

    let mut affected_item_ids = Vec::new();
    let mut split_payloads = Vec::new();
    let mut actions = Vec::new();
    let mut simulated_project = project.clone();
    let mut requests = expand_linked_split_requests(project, split_clip_requests(project, args)?);

    requests.sort_by(|left, right| {
        left.split_seconds
            .partial_cmp(&right.split_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.track_index.cmp(&right.track_index))
    });
    requests.dedup_by(|left, right| {
        left.track_index == right.track_index && left.split_seconds == right.split_seconds
    });

    for request in requests {
        let item_id = split_target_item_id(
            &simulated_project,
            request.track_index,
            request.split_seconds,
        )?;
        let new_item_id = request
            .new_item_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| generated_split_item_id(&request.id_seed, request.split_seconds));

        let split = ProjectActionSplit {
            item_id: item_id.clone(),
            new_item_id: new_item_id.clone(),
            split_seconds: request.split_seconds,
        };
        let action = ProjectAction::SplitItems {
            splits: vec![split],
        };
        apply_project_action(&mut simulated_project, action.clone())
            .map_err(|error| CodexLocalToolError::ProjectActionValidation(error.to_string()))?;

        affected_item_ids.push(item_id.clone());
        affected_item_ids.push(new_item_id.clone());
        split_payloads.push(json!({
            "itemId": item_id,
            "newItemId": new_item_id,
            "splitSeconds": request.split_seconds
        }));
        actions.push(action);
    }

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("splits".to_string(), json!(split_payloads));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

#[derive(Debug)]
struct SplitClipRequest {
    track_index: usize,
    split_seconds: f64,
    id_seed: String,
    new_item_id: Option<String>,
}

fn expand_linked_split_requests(
    project: &VideoProject,
    requests: Vec<SplitClipRequest>,
) -> Vec<SplitClipRequest> {
    let mut expanded = Vec::new();
    let mut seen = BTreeSet::new();
    for request in requests {
        let key = (
            request.id_seed.clone(),
            seconds_to_millis(request.split_seconds),
        );
        if seen.insert(key) {
            expanded.push(SplitClipRequest {
                track_index: request.track_index,
                split_seconds: request.split_seconds,
                id_seed: request.id_seed.clone(),
                new_item_id: request.new_item_id.clone(),
            });
        }

        let Some((_, item)) = timeline_item_with_track(project, &request.id_seed).ok() else {
            continue;
        };
        let Some(group_id) = timeline_item_link_group_id(item) else {
            continue;
        };
        for (track_index, track) in project.timeline.tracks.iter().enumerate() {
            for partner in &track.items {
                if partner.id == request.id_seed
                    || timeline_item_link_group_id(partner) != Some(group_id)
                    || !timeline_item_strictly_contains(partner, request.split_seconds)
                {
                    continue;
                }
                let key = (partner.id.clone(), seconds_to_millis(request.split_seconds));
                if seen.insert(key) {
                    expanded.push(SplitClipRequest {
                        track_index,
                        split_seconds: request.split_seconds,
                        id_seed: partner.id.clone(),
                        new_item_id: None,
                    });
                }
            }
        }
    }
    expanded
}

fn split_clip_requests(
    project: &VideoProject,
    args: SplitClipsArgs,
) -> Result<Vec<SplitClipRequest>, CodexLocalToolError> {
    if let Some(track_index) = args.track_index {
        let track = project.timeline.tracks.get(track_index).ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "trackIndex is out of range: {track_index}"
            ))
        })?;
        let mut frames = args.frames;
        frames.sort_unstable();
        frames.dedup();
        return frames
            .into_iter()
            .map(|frame| {
                let split_seconds = frames_to_seconds(frame, project_fps(project)?);
                let id_seed = track
                    .items
                    .iter()
                    .find(|item| timeline_item_strictly_contains(item, split_seconds))
                    .map(|item| item.id.clone())
                    .ok_or_else(|| {
                        CodexLocalToolError::InvalidArguments(format!(
                            "frames contains a split outside any clip on trackIndex {track_index}: {frame}"
                        ))
                    })?;
                Ok(SplitClipRequest {
                    track_index,
                    split_seconds,
                    id_seed,
                    new_item_id: None,
                })
            })
            .collect();
    }

    args.splits
        .into_iter()
        .map(|entry| {
            let item_id = resolve_timeline_item_id(project, &entry.item_id, "splits.itemId")?;
            let track_index = timeline_item_track_index(project, &item_id)?;
            let (_, item) = timeline_item_with_track(project, &item_id)?;
            let split_seconds = resolve_split_seconds(project, &entry, item.start_seconds)?;
            if !timeline_item_strictly_contains(item, split_seconds) {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "split point is outside clip: {item_id}"
                )));
            }
            Ok(SplitClipRequest {
                track_index,
                split_seconds,
                id_seed: item_id,
                new_item_id: entry.new_item_id,
            })
        })
        .collect()
}

fn resolve_split_seconds(
    project: &VideoProject,
    entry: &SplitClipEntry,
    item_start_seconds: f64,
) -> Result<f64, CodexLocalToolError> {
    if let Some(split_seconds) = entry.split_seconds {
        if !split_seconds.is_finite() {
            return Err(CodexLocalToolError::InvalidArguments(
                "splits.splitSeconds must be finite".to_string(),
            ));
        }
        return Ok(split_seconds);
    }
    if let Some(at_frame) = entry.at_frame {
        return Ok(round_tool_seconds(
            item_start_seconds + frames_to_seconds(at_frame, project_fps(project)?),
        ));
    }
    Err(CodexLocalToolError::InvalidArguments(
        "splits.splitSeconds or splits.atFrame is required".to_string(),
    ))
}

fn timeline_item_track_index(
    project: &VideoProject,
    item_id: &str,
) -> Result<usize, CodexLocalToolError> {
    project
        .timeline
        .tracks
        .iter()
        .position(|track| track.items.iter().any(|item| item.id == item_id))
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!("clip was not found: {item_id}"))
        })
}

fn split_target_item_id(
    project: &VideoProject,
    track_index: usize,
    split_seconds: f64,
) -> Result<String, CodexLocalToolError> {
    project
        .timeline
        .tracks
        .get(track_index)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "trackIndex is out of range: {track_index}"
            ))
        })?
        .items
        .iter()
        .find(|item| timeline_item_strictly_contains(item, split_seconds))
        .map(|item| item.id.clone())
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "split point is outside any clip on trackIndex {track_index}"
            ))
        })
}

fn timeline_item_strictly_contains(item: &TimelineItem, seconds: f64) -> bool {
    seconds > item.start_seconds && seconds < item.start_seconds + item.duration_seconds
}

fn generated_split_item_id(item_id: &str, split_seconds: f64) -> String {
    format!("{}-split-{}", item_id, seconds_to_millis(split_seconds))
}

fn seconds_to_millis(seconds: f64) -> u64 {
    (seconds.max(0.0) * 1000.0).round() as u64
}

fn set_clip_properties_payload(
    project: &VideoProject,
    args: SetClipPropertiesArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        SetClipPropertiesArgs::Direct(args) => *args,
        SetClipPropertiesArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    let project_dir = args.project_dir.clone();
    let updates = expand_linked_clip_property_updates(
        project,
        clip_property_updates_from_args(project, args)?,
    )?;

    let mut affected_item_ids = Vec::with_capacity(updates.len());
    let mut update_payloads = Vec::with_capacity(updates.len());
    let mut cleared_keyframes = Vec::new();
    let mut actions = Vec::new();
    for update in updates {
        let item_id = resolve_timeline_item_id(project, &update.item_id, "updates.itemId")?;
        let (_, item) = timeline_item_with_track(project, &item_id)?;
        let blend_mode = update
            .blend_mode
            .as_deref()
            .map(|mode| normalize_palmier_blend_mode(mode, item))
            .transpose()?;
        affected_item_ids.push(item_id.clone());
        update_payloads.push(clip_property_update_payload(&update, &item_id));

        let speed_only_retime = update.timing_source
            == ClipPropertiesTimingSource::PalmierSpeedOnly
            && update.duration_seconds.is_some()
            && update.start_seconds.is_none()
            && update.source_in.is_none()
            && update.source_out.is_none();
        let explicit_trim = update.start_seconds.is_some()
            || update.duration_seconds.is_some()
            || update.source_in.is_some()
            || update.source_out.is_some();

        // Apply speed before timing so trimItems validates the source span
        // against the playback speed the clip ends up with.
        if let Some(speed) = update.speed {
            if item.kind == TimelineItemKind::AudioClip {
                actions.push(ProjectAction::UpdateAudioClipSpeed {
                    item_id: item_id.clone(),
                    speed,
                });
            } else {
                actions.push(ProjectAction::UpdateItemProperties {
                    updates: vec![ProjectActionItemPropertiesUpdate {
                        item_id: item_id.clone(),
                        set: BTreeMap::from([("speed".to_string(), json!(speed))]),
                        remove: Vec::new(),
                    }],
                });
            }
        }

        if let Some(reverse) = update.reverse {
            actions.push(ProjectAction::UpdateClipReverse {
                item_id: item_id.clone(),
                reverse,
            });
        }

        if explicit_trim && !speed_only_retime {
            let speed = match update.speed {
                Some(speed) => validated_clip_speed(speed)?,
                None => item_speed(item).map_err(|error| {
                    CodexLocalToolError::ProjectActionValidation(error.to_string())
                })?,
            };
            let source_in = update
                .source_in
                .or_else(|| timeline_item_number_property(item, "sourceIn"))
                .unwrap_or(0.0);
            let mut duration_seconds = update.duration_seconds.unwrap_or(item.duration_seconds);
            if update.duration_seconds.is_none() {
                if let Some(source_out) = update.source_out {
                    duration_seconds = (source_out - source_in) / speed;
                }
            }
            let source_out = if update.source_in.is_some() || update.source_out.is_some() {
                Some(
                    update
                        .source_out
                        .unwrap_or(source_in + duration_seconds * speed),
                )
            } else {
                None
            };
            actions.push(ProjectAction::TrimItems {
                trims: vec![ProjectActionTrim {
                    item_id: item_id.clone(),
                    start_seconds: update.start_seconds.unwrap_or(item.start_seconds),
                    duration_seconds,
                    source_in: source_out.map(|_| source_in),
                    source_out,
                }],
            });
        }
        if speed_only_retime {
            actions.push(ProjectAction::ResizeItems {
                resizes: vec![ProjectActionResize {
                    item_id: item_id.clone(),
                    duration_seconds: update.duration_seconds.expect("retime duration checked"),
                }],
            });
        }

        if let Some(opacity) = update.opacity {
            actions.push(ProjectAction::UpdateVisualClipOpacity {
                item_id: item_id.clone(),
                opacity,
            });
            if timeline_item_has_keyframe_track(item, "opacity") {
                actions.push(ProjectAction::SetItemKeyframes {
                    item_id: item_id.clone(),
                    property: ProjectActionKeyframeProperty::Opacity,
                    keyframes: Vec::new(),
                });
                cleared_keyframes.push(json!({
                    "itemId": item_id.clone(),
                    "property": "opacity"
                }));
            }
        }

        if let Some(transform) = update.transform.clone() {
            actions.push(ProjectAction::UpdateVisualClipTransform {
                item_id: item_id.clone(),
                transform,
            });
            for (property_name, property) in transform_keyframe_tracks_to_clear(item, &update) {
                actions.push(ProjectAction::SetItemKeyframes {
                    item_id: item_id.clone(),
                    property,
                    keyframes: Vec::new(),
                });
                cleared_keyframes.push(json!({
                    "itemId": item_id.clone(),
                    "property": property_name
                }));
            }
        }

        if update.has_text_style() {
            actions.push(ProjectAction::UpdateTextOverlayItems {
                updates: vec![ProjectActionTextOverlayUpdate {
                    item_id: item_id.clone(),
                    start_seconds: update.start_seconds.unwrap_or(item.start_seconds),
                    duration_seconds: update.duration_seconds.unwrap_or(item.duration_seconds),
                    text: update
                        .text
                        .unwrap_or_else(|| timeline_item_text(item).unwrap_or("").to_string()),
                    visual_treatment: timeline_item_string_property(item, "visualTreatment")
                        .unwrap_or("compact text overlay with transparent backing and clear hierarchy")
                        .to_string(),
                    motion: timeline_item_string_property(item, "motion")
                        .unwrap_or("quick slide-in, short hold, soft fade-out")
                        .to_string(),
                    safe_zone: timeline_item_string_property(item, "safeZone")
                        .unwrap_or("keep essential text inside 10% margins")
                        .to_string(),
                    avoid: timeline_item_string_property(item, "avoid")
                        .unwrap_or("full-width opaque black slabs, centered static boxes, and default-font template looks")
                        .to_string(),
                    font_name: update.font_name,
                    font_size: update.font_size,
                    color: update.color,
                    alignment: update.alignment,
                }],
            });
        } else if let Some(text) = update.text {
            if item.kind == crate::project::model::TimelineItemKind::Caption {
                actions.push(ProjectAction::EditCaptionText {
                    item_id: item_id.clone(),
                    text,
                });
            } else {
                actions.push(ProjectAction::EditTextItem {
                    item_id: item_id.clone(),
                    text,
                });
            }
        }

        if let Some(volume_db) = update.volume_db {
            actions.push(ProjectAction::UpdateAudioVolume {
                item_id: item_id.clone(),
                volume_db: Some(volume_db),
            });
            if timeline_item_has_keyframe_track(item, "volumeDb") {
                actions.push(ProjectAction::SetItemKeyframes {
                    item_id: item_id.clone(),
                    property: ProjectActionKeyframeProperty::VolumeDb,
                    keyframes: Vec::new(),
                });
                cleared_keyframes.push(json!({
                    "itemId": item_id.clone(),
                    "property": "volumeDb"
                }));
            }
        }

        if let Some(fade_out_seconds) = update.fade_out_seconds {
            actions.push(ProjectAction::UpdateAudioFadeOut {
                item_id: item_id.clone(),
                fade_out_seconds,
            });
        }

        if let Some(blend_mode) = blend_mode {
            let (set, remove) = match blend_mode.as_deref() {
                Some(mode) => (
                    BTreeMap::from([("blendMode".to_string(), json!(mode))]),
                    Vec::new(),
                ),
                None => (BTreeMap::new(), vec!["blendMode".to_string()]),
            };
            actions.push(ProjectAction::UpdateItemProperties {
                updates: vec![ProjectActionItemPropertiesUpdate {
                    item_id: item_id.clone(),
                    set,
                    remove,
                }],
            });
        }
    }

    if actions.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "updates must include at least one editable property".to_string(),
        ));
    }

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("updates".to_string(), json!(update_payloads));
        if !cleared_keyframes.is_empty() {
            object.insert("clearedKeyframes".to_string(), json!(cleared_keyframes));
        }
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn update_text_payload(
    project: &VideoProject,
    args: UpdateTextArgs,
) -> Result<Value, CodexLocalToolError> {
    if let Some(transform) = args.transform.as_ref() {
        validate_visual_transform_args(transform)?;
    }
    let item_ids = update_text_target_item_ids(project, &args)?;
    let has_content = args.content.is_some();
    let has_core_style = update_text_has_core_style(&args);
    let property_set = update_text_property_set(&args, false)?;
    if !has_content && !has_core_style && args.transform.is_none() && property_set.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "update_text needs at least one text property to apply".to_string(),
        ));
    }

    let mut affected_item_ids = Vec::with_capacity(item_ids.len());
    let mut update_payloads = Vec::with_capacity(item_ids.len());
    let mut actions = Vec::new();
    for item_id in item_ids {
        let (_, item) = timeline_item_with_track(project, &item_id)?;
        if !is_text_timeline_item(item) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "update_text only applies to text clips: {item_id} is {}",
                timeline_item_kind_label(&item.kind)
            )));
        }

        affected_item_ids.push(item_id.clone());
        update_payloads.push(update_text_update_payload(&item_id, &args));

        if let Some(transform) = args.transform.clone() {
            actions.push(ProjectAction::UpdateVisualClipTransform {
                item_id: item_id.clone(),
                transform,
            });
        }

        let is_overlay_text = item.kind == TimelineItemKind::Overlay
            && matches!(item.source, TimelineSource::Text { .. });
        if is_overlay_text && has_core_style {
            actions.push(ProjectAction::UpdateTextOverlayItems {
                updates: vec![ProjectActionTextOverlayUpdate {
                    item_id: item_id.clone(),
                    start_seconds: item.start_seconds,
                    duration_seconds: item.duration_seconds,
                    text: args
                        .content
                        .clone()
                        .unwrap_or_else(|| timeline_item_text(item).unwrap_or("").to_string()),
                    visual_treatment: timeline_item_string_property(item, "visualTreatment")
                        .unwrap_or("compact text overlay with transparent backing and clear hierarchy")
                        .to_string(),
                    motion: timeline_item_string_property(item, "motion")
                        .unwrap_or("quick slide-in, short hold, soft fade-out")
                        .to_string(),
                    safe_zone: timeline_item_string_property(item, "safeZone")
                        .unwrap_or("keep essential text inside 10% margins")
                        .to_string(),
                    avoid: timeline_item_string_property(item, "avoid")
                        .unwrap_or("full-width opaque black slabs, centered static boxes, and default-font template looks")
                        .to_string(),
                    font_name: args.font_name.clone(),
                    font_size: args.font_size,
                    color: args.color.clone(),
                    alignment: args.alignment.clone(),
                }],
            });
        } else if let Some(content) = args.content.clone() {
            if item.kind == TimelineItemKind::Caption {
                actions.push(ProjectAction::EditCaptionText {
                    item_id: item_id.clone(),
                    text: content,
                });
            } else {
                actions.push(ProjectAction::EditTextItem {
                    item_id: item_id.clone(),
                    text: content,
                });
            }
        }

        let property_set = update_text_property_set(&args, !is_overlay_text && has_core_style)?;
        if !property_set.is_empty() {
            actions.push(ProjectAction::UpdateItemProperties {
                updates: vec![ProjectActionItemPropertiesUpdate {
                    item_id,
                    set: property_set,
                    remove: Vec::new(),
                }],
            });
        }
    }

    let project_dir = args.project_dir.clone();
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("valid".to_string(), json!(true));
        object.insert("affectedItemIds".to_string(), json!(affected_item_ids));
        object.insert("updates".to_string(), json!(update_payloads));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn update_text_target_item_ids(
    project: &VideoProject,
    args: &UpdateTextArgs,
) -> Result<Vec<String>, CodexLocalToolError> {
    let mut item_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for clip_id in &args.clip_ids {
        let item_id = resolve_timeline_item_id(project, clip_id, "clipIds")?;
        if seen.insert(item_id.clone()) {
            item_ids.push(item_id);
        }
    }
    if let Some(group_id) = args.caption_group_id.as_deref() {
        let group_id = trim_required(group_id, "captionGroupId")?;
        let mut matched = false;
        for track in &project.timeline.tracks {
            for item in &track.items {
                if caption_group_id(item) == Some(group_id.as_str()) {
                    matched = true;
                    if seen.insert(item.id.clone()) {
                        item_ids.push(item.id.clone());
                    }
                }
            }
        }
        if !matched {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "No caption clips found for captionGroupId: {group_id}"
            )));
        }
    }
    if item_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "Provide a non-empty clipIds array or a captionGroupId".to_string(),
        ));
    }
    Ok(item_ids)
}

fn update_text_has_core_style(args: &UpdateTextArgs) -> bool {
    args.font_name.is_some()
        || args.font_size.is_some()
        || args.color.is_some()
        || args.alignment.is_some()
}

fn update_text_property_set(
    args: &UpdateTextArgs,
    include_core_style: bool,
) -> Result<BTreeMap<String, Value>, CodexLocalToolError> {
    let mut set = BTreeMap::new();
    if include_core_style {
        insert_optional_text_property(&mut set, "fontName", args.font_name.as_deref())?;
        if let Some(font_size) = args.font_size {
            if !font_size.is_finite() || font_size <= 0.0 {
                return Err(CodexLocalToolError::InvalidArguments(
                    "fontSize must be finite and greater than zero".to_string(),
                ));
            }
            set.insert("fontSize".to_string(), json!(font_size));
        }
        insert_optional_text_property(&mut set, "color", args.color.as_deref())?;
        insert_optional_text_property(&mut set, "alignment", args.alignment.as_deref())?;
    }
    if let Some(is_bold) = args.is_bold {
        set.insert("isBold".to_string(), json!(is_bold));
    }
    if let Some(is_italic) = args.is_italic {
        set.insert("isItalic".to_string(), json!(is_italic));
    }
    insert_optional_text_property(&mut set, "borderColor", args.border_color.as_deref())?;
    insert_optional_text_property(
        &mut set,
        "backgroundColor",
        args.background_color.as_deref(),
    )?;
    insert_optional_text_property(&mut set, "animation", args.animation.as_deref())?;
    insert_optional_text_property(&mut set, "highlightColor", args.highlight_color.as_deref())?;
    Ok(set)
}

fn insert_optional_text_property(
    set: &mut BTreeMap<String, Value>,
    key: &str,
    value: Option<&str>,
) -> Result<(), CodexLocalToolError> {
    if let Some(value) = value {
        set.insert(key.to_string(), json!(trim_required(value, key)?));
    }
    Ok(())
}

fn update_text_update_payload(item_id: &str, args: &UpdateTextArgs) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert("itemId".to_string(), json!(item_id));
    if let Some(content) = args.content.as_deref() {
        payload.insert("text".to_string(), json!(content));
    }
    if let Some(transform) = args.transform.as_ref() {
        payload.insert("transform".to_string(), json!(transform));
    }
    if let Some(font_name) = args.font_name.as_deref() {
        payload.insert("fontName".to_string(), json!(font_name));
    }
    if let Some(font_size) = args.font_size {
        payload.insert("fontSize".to_string(), json!(font_size));
    }
    if let Some(is_bold) = args.is_bold {
        payload.insert("isBold".to_string(), json!(is_bold));
    }
    if let Some(is_italic) = args.is_italic {
        payload.insert("isItalic".to_string(), json!(is_italic));
    }
    if let Some(color) = args.color.as_deref() {
        payload.insert("color".to_string(), json!(color));
    }
    if let Some(alignment) = args.alignment.as_deref() {
        payload.insert("alignment".to_string(), json!(alignment));
    }
    if let Some(border_color) = args.border_color.as_deref() {
        payload.insert("borderColor".to_string(), json!(border_color));
    }
    if let Some(background_color) = args.background_color.as_deref() {
        payload.insert("backgroundColor".to_string(), json!(background_color));
    }
    if let Some(animation) = args.animation.as_deref() {
        payload.insert("animation".to_string(), json!(animation));
    }
    if let Some(highlight_color) = args.highlight_color.as_deref() {
        payload.insert("highlightColor".to_string(), json!(highlight_color));
    }
    Value::Object(payload)
}

fn expand_linked_clip_property_updates(
    project: &VideoProject,
    updates: Vec<ClipPropertiesUpdate>,
) -> Result<Vec<ClipPropertiesUpdate>, CodexLocalToolError> {
    let mut requested_item_ids = BTreeSet::new();
    for update in &updates {
        requested_item_ids.insert(resolve_timeline_item_id(
            project,
            &update.item_id,
            "updates.itemId",
        )?);
    }

    let mut expanded = Vec::new();
    let mut seen = BTreeSet::new();
    for update in updates {
        let resolved_item_id =
            resolve_timeline_item_id(project, &update.item_id, "updates.itemId")?;
        if seen.insert(resolved_item_id.clone()) {
            expanded.push(update.clone());
        }

        if !clip_property_update_has_linked_timing(&update) && update.reverse.is_none() {
            continue;
        }
        let (_, item) = timeline_item_with_track(project, &resolved_item_id)?;
        let Some(group_id) = timeline_item_link_group_id(item) else {
            continue;
        };
        for track in &project.timeline.tracks {
            for partner in &track.items {
                if partner.id == resolved_item_id
                    || requested_item_ids.contains(&partner.id)
                    || timeline_item_link_group_id(partner) != Some(group_id)
                    || !seen.insert(partner.id.clone())
                {
                    continue;
                }
                let partner_update = ClipPropertiesUpdate {
                    item_id: partner.id.clone(),
                    start_seconds: None,
                    duration_seconds: linked_partner_duration_seconds(&update, partner),
                    source_in: linked_partner_source_in(&update, partner),
                    source_out: linked_partner_source_out(&update, partner),
                    opacity: None,
                    transform: None,
                    text: None,
                    volume_db: None,
                    fade_out_seconds: None,
                    font_name: None,
                    font_size: None,
                    color: None,
                    alignment: None,
                    blend_mode: None,
                    reverse: linked_partner_reverse(project, &update, partner),
                    speed: linked_partner_speed(&update, partner),
                    timing_source: update.timing_source,
                };
                if clip_property_update_has_editable_property(&partner_update) {
                    expanded.push(partner_update);
                }
            }
        }
    }
    Ok(expanded)
}

fn clip_property_update_has_linked_timing(update: &ClipPropertiesUpdate) -> bool {
    update.duration_seconds.is_some() || update.source_in.is_some() || update.source_out.is_some()
}

fn clip_property_update_has_editable_property(update: &ClipPropertiesUpdate) -> bool {
    update.start_seconds.is_some()
        || update.duration_seconds.is_some()
        || update.source_in.is_some()
        || update.source_out.is_some()
        || update.opacity.is_some()
        || update.transform.is_some()
        || update.text.is_some()
        || update.volume_db.is_some()
        || update.fade_out_seconds.is_some()
        || update.speed.is_some()
        || update.reverse.is_some()
        || update.blend_mode.is_some()
        || update.has_text_style()
}

fn linked_partner_duration_seconds(
    update: &ClipPropertiesUpdate,
    partner: &TimelineItem,
) -> Option<f64> {
    if is_text_timeline_item(partner)
        && update.timing_source == ClipPropertiesTimingSource::PalmierSpeedOnly
    {
        None
    } else {
        update.duration_seconds
    }
}

fn linked_partner_source_in(update: &ClipPropertiesUpdate, partner: &TimelineItem) -> Option<f64> {
    if is_text_timeline_item(partner) {
        None
    } else {
        update.source_in
    }
}

fn linked_partner_source_out(update: &ClipPropertiesUpdate, partner: &TimelineItem) -> Option<f64> {
    if is_text_timeline_item(partner) {
        None
    } else {
        update.source_out
    }
}

fn linked_partner_speed(update: &ClipPropertiesUpdate, partner: &TimelineItem) -> Option<f64> {
    if is_text_timeline_item(partner) {
        None
    } else {
        update.speed
    }
}

/// Reverse follows links to the partner clips that can play reversed.
fn linked_partner_reverse(
    project: &VideoProject,
    update: &ClipPropertiesUpdate,
    partner: &TimelineItem,
) -> Option<bool> {
    is_reversible_clip(project, partner)
        .then_some(update.reverse)
        .flatten()
}

fn is_text_timeline_item(item: &TimelineItem) -> bool {
    matches!(
        item.kind,
        TimelineItemKind::Overlay | TimelineItemKind::Caption
    ) || matches!(item.source, TimelineSource::Text { .. })
}

fn transform_keyframe_tracks_to_clear(
    item: &TimelineItem,
    update: &ClipPropertiesUpdate,
) -> Vec<(&'static str, ProjectActionKeyframeProperty)> {
    let Some(transform) = update.transform.as_ref() else {
        return Vec::new();
    };

    [
        (
            transform.center_x,
            "positionX",
            ProjectActionKeyframeProperty::PositionX,
        ),
        (
            transform.center_y,
            "positionY",
            ProjectActionKeyframeProperty::PositionY,
        ),
        (
            transform.width,
            "scaleX",
            ProjectActionKeyframeProperty::ScaleX,
        ),
        (
            transform.height,
            "scaleY",
            ProjectActionKeyframeProperty::ScaleY,
        ),
    ]
    .into_iter()
    .filter_map(|(value, property_name, property)| {
        if value.is_some() && timeline_item_has_keyframe_track(item, property_name) {
            Some((property_name, property))
        } else {
            None
        }
    })
    .collect()
}

fn clip_property_updates_from_args(
    project: &VideoProject,
    args: DirectSetClipPropertiesArgs,
) -> Result<Vec<ClipPropertiesUpdate>, CodexLocalToolError> {
    if !args.updates.is_empty() {
        if !args.clip_ids.is_empty() {
            return Err(CodexLocalToolError::InvalidArguments(
                "pass either updates or clipIds, not both".to_string(),
            ));
        }
        return Ok(args.updates);
    }

    if args.clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "updates or clipIds must not be empty".to_string(),
        ));
    }
    if let Some(transform) = args.transform.as_ref() {
        validate_visual_transform_args(transform)?;
    }

    let fps = project_fps(project)?;
    let duration_seconds = args
        .duration_frames
        .map(|frames| frames_to_seconds(frames, fps));
    let source_in = args
        .trim_start_frame
        .map(|frames| frames_to_seconds(frames, fps));
    let source_out = args
        .trim_end_frame
        .map(|frames| frames_to_seconds(frames, fps));
    let volume_db = args.volume.map(palmier_volume_to_db).transpose()?;
    let fade_out_seconds = match (args.fade_out_seconds, args.fade_out_frames) {
        (Some(_), Some(_)) => {
            return Err(CodexLocalToolError::InvalidArguments(
                "pass either fadeOutSeconds or fadeOutFrames, not both".to_string(),
            ));
        }
        (Some(seconds), None) => Some(validate_fade_out_seconds(seconds)?),
        (None, Some(frames)) => Some(frames_to_seconds(frames, fps)),
        (None, None) => None,
    };

    let opacity = args.opacity;
    let transform = args.transform;
    let content = args.content;
    let speed = args.speed.map(validated_clip_speed).transpose()?;
    let reverse = args.reverse;
    let font_name = args.font_name;
    let font_size = args.font_size;
    let color = args.color;
    let alignment = args.alignment;
    let blend_mode = args.blend_mode;

    args.clip_ids
        .into_iter()
        .map(|clip_id| {
            let item_id = resolve_timeline_item_id(project, &clip_id, "clipIds")?;
            let (_, item) = timeline_item_with_track(project, &item_id)?;
            let duration_seconds = match (duration_seconds, speed, source_out) {
                (Some(duration_seconds), _, _) => Some(duration_seconds),
                // An explicit source out point fixes the consumed source span,
                // so the new speed determines the timeline duration.
                (None, Some(speed), Some(source_out)) => {
                    let source_in = source_in
                        .or_else(|| timeline_item_number_property(item, "sourceIn"))
                        .unwrap_or(0.0);
                    Some((source_out - source_in) / validated_clip_speed(speed)?)
                }
                (None, Some(speed), None) => Some(speed_adjusted_duration_seconds(item, speed)?),
                (None, None, _) => None,
            };
            let timing_source = if args.duration_frames.is_some()
                || args.trim_start_frame.is_some()
                || args.trim_end_frame.is_some()
            {
                ClipPropertiesTimingSource::PalmierFrameFields
            } else if speed.is_some() {
                ClipPropertiesTimingSource::PalmierSpeedOnly
            } else {
                ClipPropertiesTimingSource::Direct
            };
            Ok(ClipPropertiesUpdate {
                item_id,
                start_seconds: None,
                duration_seconds,
                source_in,
                source_out,
                opacity,
                transform: transform.clone(),
                text: content.clone(),
                volume_db,
                fade_out_seconds,
                font_name: font_name.clone(),
                font_size,
                color: color.clone(),
                alignment: alignment.clone(),
                blend_mode: blend_mode.clone(),
                reverse,
                speed,
                timing_source,
            })
        })
        .collect()
}

fn normalize_palmier_blend_mode(
    mode: &str,
    item: &TimelineItem,
) -> Result<Option<String>, CodexLocalToolError> {
    if !is_blend_mode_timeline_item(item) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "blendMode only applies to video or image clips: {} is {}",
            item.id,
            timeline_item_kind_label(&item.kind)
        )));
    }
    if !PALMIER_BLEND_MODES.contains(&mode) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "invalid blendMode `{mode}`. Valid: {}",
            PALMIER_BLEND_MODES.join(", ")
        )));
    }
    if mode == "normal" {
        Ok(None)
    } else {
        Ok(Some(mode.to_string()))
    }
}

fn is_blend_mode_timeline_item(item: &TimelineItem) -> bool {
    matches!(
        item.kind,
        TimelineItemKind::VideoClip | TimelineItemKind::ImageClip | TimelineItemKind::GeneratedClip
    )
}

/// Clip speeds `updateAudioClipSpeed` and `updateVisualClipSpeed` accept. A speed
/// outside it on a video clip would also be copied to its linked audio and fail there.
const CLIP_SPEED_RANGE: std::ops::RangeInclusive<f64> = crate::project::audio_edits::SPEED_RANGE;

fn validated_clip_speed(speed: f64) -> Result<f64, CodexLocalToolError> {
    if speed.is_finite() && CLIP_SPEED_RANGE.contains(&speed) {
        Ok(speed)
    } else {
        Err(CodexLocalToolError::InvalidArguments(format!(
            "speed must be between {} and {}",
            CLIP_SPEED_RANGE.start(),
            CLIP_SPEED_RANGE.end()
        )))
    }
}

fn speed_adjusted_duration_seconds(
    item: &TimelineItem,
    speed: f64,
) -> Result<f64, CodexLocalToolError> {
    let speed = validated_clip_speed(speed)?;
    let current_speed = timeline_item_number_property(item, "speed")
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);
    Ok(round_tool_seconds(
        item.duration_seconds * current_speed / speed,
    ))
}

fn palmier_volume_to_db(volume: f64) -> Result<f64, CodexLocalToolError> {
    if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
        return Err(CodexLocalToolError::InvalidArguments(
            "volume must be finite and between 0.0 and 1.0".to_string(),
        ));
    }
    if volume <= 0.0 {
        return Ok(-60.0);
    }
    Ok(round_tool_seconds((20.0 * volume.log10()).max(-60.0)))
}

fn validate_fade_out_seconds(fade_out_seconds: f64) -> Result<f64, CodexLocalToolError> {
    if !fade_out_seconds.is_finite() || fade_out_seconds < 0.0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "fadeOutSeconds must be finite and greater than or equal to zero".to_string(),
        ));
    }
    Ok(round_tool_seconds(fade_out_seconds))
}

fn validate_visual_transform_args(
    transform: &ProjectActionVisualTransform,
) -> Result<(), CodexLocalToolError> {
    if transform.center_x.is_none()
        && transform.center_y.is_none()
        && transform.width.is_none()
        && transform.height.is_none()
        && transform.flip_horizontal.is_none()
        && transform.flip_vertical.is_none()
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "transform must include at least one field".to_string(),
        ));
    }
    for (field, value) in [
        ("transform.centerX", transform.center_x),
        ("transform.centerY", transform.center_y),
    ] {
        if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{field} must be finite and between 0.0 and 1.0"
            )));
        }
    }
    for (field, value) in [
        ("transform.width", transform.width),
        ("transform.height", transform.height),
    ] {
        if value.is_some_and(|value| !value.is_finite() || value <= 0.0 || value > 1.0) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{field} must be finite and greater than 0.0 up to 1.0"
            )));
        }
    }
    Ok(())
}

fn clip_property_update_payload(update: &ClipPropertiesUpdate, item_id: &str) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert("itemId".to_string(), json!(item_id));
    if let Some(start_seconds) = update.start_seconds {
        payload.insert("startSeconds".to_string(), json!(start_seconds));
    }
    if let Some(duration_seconds) = update.duration_seconds {
        payload.insert("durationSeconds".to_string(), json!(duration_seconds));
    }
    if let Some(source_in) = update.source_in {
        payload.insert("sourceIn".to_string(), json!(source_in));
    }
    if let Some(source_out) = update.source_out {
        payload.insert("sourceOut".to_string(), json!(source_out));
    }
    if let Some(opacity) = update.opacity {
        payload.insert("opacity".to_string(), json!(opacity));
    }
    if let Some(transform) = update.transform.as_ref() {
        payload.insert("transform".to_string(), json!(transform));
    }
    if let Some(text) = update.text.as_deref() {
        payload.insert("text".to_string(), json!(text));
    }
    if let Some(font_name) = update.font_name.as_deref() {
        payload.insert("fontName".to_string(), json!(font_name));
    }
    if let Some(font_size) = update.font_size {
        payload.insert("fontSize".to_string(), json!(font_size));
    }
    if let Some(color) = update.color.as_deref() {
        payload.insert("color".to_string(), json!(color));
    }
    if let Some(alignment) = update.alignment.as_deref() {
        payload.insert("alignment".to_string(), json!(alignment));
    }
    if let Some(blend_mode) = update.blend_mode.as_deref() {
        payload.insert("blendMode".to_string(), json!(blend_mode));
    }
    if let Some(reverse) = update.reverse {
        payload.insert("reverse".to_string(), json!(reverse));
    }
    if let Some(speed) = update.speed {
        payload.insert("speed".to_string(), json!(speed));
    }
    if let Some(volume_db) = update.volume_db {
        payload.insert("volumeDb".to_string(), json!(volume_db));
    }
    if let Some(fade_out_seconds) = update.fade_out_seconds {
        payload.insert("fadeOutSeconds".to_string(), json!(fade_out_seconds));
    }
    Value::Object(payload)
}

fn set_keyframes_payload(
    project: &VideoProject,
    args: SetKeyframesArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        SetKeyframesArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        SetKeyframesArgs::Direct(args) => set_keyframes_direct_payload(project, args),
    }
}

fn set_keyframes_direct_payload(
    project: &VideoProject,
    args: DirectSetKeyframesArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = args.project_dir.clone();
    let item_id = resolve_timeline_item_id(project, &args.item_id, "itemId")?;
    if args.property.trim() == "position" {
        return set_pair_keyframes_direct_payload(
            project,
            project_dir,
            item_id,
            args.keyframes,
            PairKeyframeProperties {
                source: "position",
                first_payload_name: "positionX",
                second_payload_name: "positionY",
                first: ProjectActionKeyframeProperty::PositionX,
                second: ProjectActionKeyframeProperty::PositionY,
            },
        );
    }
    if args.property.trim() == "scale" && palmier_rows_are_pair_keyframes(&args.keyframes) {
        return set_pair_keyframes_direct_payload(
            project,
            project_dir,
            item_id,
            args.keyframes,
            PairKeyframeProperties {
                source: "scale",
                first_payload_name: "scaleX",
                second_payload_name: "scaleY",
                first: ProjectActionKeyframeProperty::ScaleX,
                second: ProjectActionKeyframeProperty::ScaleY,
            },
        );
    }
    if args.property.trim() == "crop" {
        return set_crop_keyframes_direct_payload(project, project_dir, item_id, args.keyframes);
    }
    let property = parse_keyframe_property(&args.property)?;
    let keyframes = normalize_keyframes(project, &args.property, &property, args.keyframes)?;

    let action = ProjectAction::SetItemKeyframes {
        item_id: item_id.clone(),
        property,
        keyframes: keyframes.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!([item_id]));
        object.insert("keyframes".to_string(), json!(keyframes));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

struct PairKeyframeProperties<'a> {
    source: &'a str,
    first_payload_name: &'a str,
    second_payload_name: &'a str,
    first: ProjectActionKeyframeProperty,
    second: ProjectActionKeyframeProperty,
}

fn set_pair_keyframes_direct_payload(
    project: &VideoProject,
    project_dir: Option<String>,
    item_id: String,
    keyframes: Vec<Value>,
    properties: PairKeyframeProperties<'_>,
) -> Result<Value, CodexLocalToolError> {
    let (first_keyframes, second_keyframes) =
        normalize_palmier_pair_keyframes(project, properties.source, keyframes)?;
    let actions = vec![
        ProjectAction::SetItemKeyframes {
            item_id: item_id.clone(),
            property: properties.first,
            keyframes: first_keyframes.clone(),
        },
        ProjectAction::SetItemKeyframes {
            item_id: item_id.clone(),
            property: properties.second,
            keyframes: second_keyframes.clone(),
        },
    ];
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        let first_payload_name = properties.first_payload_name;
        let second_payload_name = properties.second_payload_name;
        object.insert("affectedItemIds".to_string(), json!([item_id]));
        object.insert(
            "keyframes".to_string(),
            json!({
                first_payload_name: first_keyframes,
                second_payload_name: second_keyframes
            }),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn set_crop_keyframes_direct_payload(
    project: &VideoProject,
    project_dir: Option<String>,
    item_id: String,
    keyframes: Vec<Value>,
) -> Result<Value, CodexLocalToolError> {
    let (crop_top, crop_right, crop_bottom, crop_left) =
        normalize_palmier_crop_keyframes(project, keyframes)?;
    let actions = vec![
        ProjectAction::SetItemKeyframes {
            item_id: item_id.clone(),
            property: ProjectActionKeyframeProperty::CropTop,
            keyframes: crop_top.clone(),
        },
        ProjectAction::SetItemKeyframes {
            item_id: item_id.clone(),
            property: ProjectActionKeyframeProperty::CropRight,
            keyframes: crop_right.clone(),
        },
        ProjectAction::SetItemKeyframes {
            item_id: item_id.clone(),
            property: ProjectActionKeyframeProperty::CropBottom,
            keyframes: crop_bottom.clone(),
        },
        ProjectAction::SetItemKeyframes {
            item_id: item_id.clone(),
            property: ProjectActionKeyframeProperty::CropLeft,
            keyframes: crop_left.clone(),
        },
    ];
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedItemIds".to_string(), json!([item_id]));
        object.insert(
            "keyframes".to_string(),
            json!({
                "cropTop": crop_top,
                "cropRight": crop_right,
                "cropBottom": crop_bottom,
                "cropLeft": crop_left
            }),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn parse_keyframe_property(
    property: &str,
) -> Result<ProjectActionKeyframeProperty, CodexLocalToolError> {
    match property.trim() {
        "opacity" => Ok(ProjectActionKeyframeProperty::Opacity),
        "volume" | "volumeDb" => Ok(ProjectActionKeyframeProperty::VolumeDb),
        "positionX" => Ok(ProjectActionKeyframeProperty::PositionX),
        "positionY" => Ok(ProjectActionKeyframeProperty::PositionY),
        "scale" => Ok(ProjectActionKeyframeProperty::Scale),
        "scaleX" => Ok(ProjectActionKeyframeProperty::ScaleX),
        "scaleY" => Ok(ProjectActionKeyframeProperty::ScaleY),
        "rotation" | "rotationDegrees" => Ok(ProjectActionKeyframeProperty::RotationDegrees),
        "cropTop" => Ok(ProjectActionKeyframeProperty::CropTop),
        "cropRight" => Ok(ProjectActionKeyframeProperty::CropRight),
        "cropBottom" => Ok(ProjectActionKeyframeProperty::CropBottom),
        "cropLeft" => Ok(ProjectActionKeyframeProperty::CropLeft),
        "position" => Err(CodexLocalToolError::InvalidArguments(
            "set_keyframes.position must use Palmier rows [frame, x, y, easing?]".to_string(),
        )),
        "crop" => Err(CodexLocalToolError::InvalidArguments(
            "set_keyframes.crop must use Palmier rows [frame, top, right, bottom, left, easing?]"
                .to_string(),
        )),
        other => Err(CodexLocalToolError::InvalidArguments(format!(
            "unsupported keyframe property: {other}"
        ))),
    }
}

fn normalize_keyframes(
    project: &VideoProject,
    source_property: &str,
    property: &ProjectActionKeyframeProperty,
    keyframes: Vec<Value>,
) -> Result<Vec<ProjectActionKeyframe>, CodexLocalToolError> {
    let keyframes = keyframes
        .into_iter()
        .map(|value| normalize_keyframe(project, source_property, property, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sort_dedupe_keyframes(keyframes))
}

fn palmier_rows_are_pair_keyframes(keyframes: &[Value]) -> bool {
    if keyframes.is_empty() {
        return true;
    }
    keyframes.iter().any(|value| {
        value
            .as_array()
            .is_some_and(|row| row.len() == 3 || row.len() == 4)
    })
}

fn normalize_palmier_pair_keyframes(
    project: &VideoProject,
    source_property: &str,
    keyframes: Vec<Value>,
) -> Result<(Vec<ProjectActionKeyframe>, Vec<ProjectActionKeyframe>), CodexLocalToolError> {
    let mut first_keyframes = Vec::new();
    let mut second_keyframes = Vec::new();
    for value in keyframes {
        let (first, second) = match value {
            Value::Array(row) => normalize_palmier_pair_keyframe_row(project, source_property, row),
            _ => Err(CodexLocalToolError::InvalidArguments(
                format!(
                    "{source_property} keyframes must be Palmier rows [frame, valueA, valueB] or [frame, valueA, valueB, easing]"
                ),
            )),
        }?;
        first_keyframes.push(first);
        second_keyframes.push(second);
    }
    Ok((
        sort_dedupe_keyframes(first_keyframes),
        sort_dedupe_keyframes(second_keyframes),
    ))
}

fn normalize_palmier_pair_keyframe_row(
    project: &VideoProject,
    source_property: &str,
    row: Vec<Value>,
) -> Result<(ProjectActionKeyframe, ProjectActionKeyframe), CodexLocalToolError> {
    if row.len() < 3 || row.len() > 4 {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{source_property} keyframe rows must be [frame, valueA, valueB] or [frame, valueA, valueB, easing]"
        )));
    }
    let frame = row[0].as_u64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!(
            "{source_property} keyframe row frame must be an integer"
        ))
    })?;
    let first = row[1].as_f64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!(
            "{source_property} keyframe row first value must be numeric"
        ))
    })?;
    let second = row[2].as_f64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!(
            "{source_property} keyframe row second value must be numeric"
        ))
    })?;
    let easing = row
        .get(3)
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(format!(
                    "{source_property} keyframe row easing must be a string"
                ))
            })
        })
        .transpose()?;
    let at_seconds = frames_to_seconds(frame, project_fps(project)?);

    Ok((
        ProjectActionKeyframe {
            at_seconds,
            value: first,
            easing: easing.clone(),
        },
        ProjectActionKeyframe {
            at_seconds,
            value: second,
            easing,
        },
    ))
}

type CropKeyframes = (
    Vec<ProjectActionKeyframe>,
    Vec<ProjectActionKeyframe>,
    Vec<ProjectActionKeyframe>,
    Vec<ProjectActionKeyframe>,
);

fn normalize_palmier_crop_keyframes(
    project: &VideoProject,
    keyframes: Vec<Value>,
) -> Result<CropKeyframes, CodexLocalToolError> {
    let mut top_keyframes = Vec::new();
    let mut right_keyframes = Vec::new();
    let mut bottom_keyframes = Vec::new();
    let mut left_keyframes = Vec::new();
    for value in keyframes {
        let Value::Array(row) = value else {
            return Err(CodexLocalToolError::InvalidArguments(
                "crop keyframes must be Palmier rows [frame, top, right, bottom, left] or [frame, top, right, bottom, left, easing]".to_string(),
            ));
        };
        let (top, right, bottom, left) = normalize_palmier_crop_keyframe_row(project, row)?;
        top_keyframes.push(top);
        right_keyframes.push(right);
        bottom_keyframes.push(bottom);
        left_keyframes.push(left);
    }
    Ok((
        sort_dedupe_keyframes(top_keyframes),
        sort_dedupe_keyframes(right_keyframes),
        sort_dedupe_keyframes(bottom_keyframes),
        sort_dedupe_keyframes(left_keyframes),
    ))
}

fn sort_dedupe_keyframes(keyframes: Vec<ProjectActionKeyframe>) -> Vec<ProjectActionKeyframe> {
    let mut indexed = keyframes.into_iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by(|left, right| {
        left.1
            .at_seconds
            .total_cmp(&right.1.at_seconds)
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut deduped: Vec<(usize, ProjectActionKeyframe)> = Vec::new();
    for (index, keyframe) in indexed {
        if deduped
            .last()
            .is_some_and(|(_, previous)| previous.at_seconds == keyframe.at_seconds)
        {
            let last = deduped.last_mut().expect("last checked");
            *last = (index, keyframe);
        } else {
            deduped.push((index, keyframe));
        }
    }

    deduped.into_iter().map(|(_, keyframe)| keyframe).collect()
}

fn normalize_palmier_crop_keyframe_row(
    project: &VideoProject,
    row: Vec<Value>,
) -> Result<
    (
        ProjectActionKeyframe,
        ProjectActionKeyframe,
        ProjectActionKeyframe,
        ProjectActionKeyframe,
    ),
    CodexLocalToolError,
> {
    if row.len() < 5 || row.len() > 6 {
        return Err(CodexLocalToolError::InvalidArguments(
            "crop keyframe rows must be [frame, top, right, bottom, left] or [frame, top, right, bottom, left, easing]".to_string(),
        ));
    }
    let frame = row[0].as_u64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(
            "crop keyframe row frame must be an integer".to_string(),
        )
    })?;
    let values = [
        row[1].as_f64().ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "crop keyframe row top value must be numeric".to_string(),
            )
        })?,
        row[2].as_f64().ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "crop keyframe row right value must be numeric".to_string(),
            )
        })?,
        row[3].as_f64().ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "crop keyframe row bottom value must be numeric".to_string(),
            )
        })?,
        row[4].as_f64().ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "crop keyframe row left value must be numeric".to_string(),
            )
        })?,
    ];
    let easing = row
        .get(5)
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(
                    "crop keyframe row easing must be a string".to_string(),
                )
            })
        })
        .transpose()?;
    let at_seconds = frames_to_seconds(frame, project_fps(project)?);
    Ok((
        ProjectActionKeyframe {
            at_seconds,
            value: values[0],
            easing: easing.clone(),
        },
        ProjectActionKeyframe {
            at_seconds,
            value: values[1],
            easing: easing.clone(),
        },
        ProjectActionKeyframe {
            at_seconds,
            value: values[2],
            easing: easing.clone(),
        },
        ProjectActionKeyframe {
            at_seconds,
            value: values[3],
            easing,
        },
    ))
}

fn normalize_keyframe(
    project: &VideoProject,
    source_property: &str,
    property: &ProjectActionKeyframeProperty,
    value: Value,
) -> Result<ProjectActionKeyframe, CodexLocalToolError> {
    match value {
        Value::Array(row) => {
            normalize_palmier_keyframe_row(project, source_property, property, row)
        }
        value => {
            let mut keyframe: ProjectActionKeyframe = serde_json::from_value(value)
                .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
            keyframe.value = normalize_keyframe_value(source_property, property, keyframe.value)?;
            Ok(keyframe)
        }
    }
}

fn normalize_palmier_keyframe_row(
    project: &VideoProject,
    source_property: &str,
    property: &ProjectActionKeyframeProperty,
    row: Vec<Value>,
) -> Result<ProjectActionKeyframe, CodexLocalToolError> {
    if row.len() < 2 || row.len() > 3 {
        return Err(CodexLocalToolError::InvalidArguments(
            "keyframe rows must be [frame, value] or [frame, value, easing]".to_string(),
        ));
    }
    let frame = row[0].as_u64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments("keyframe row frame must be an integer".to_string())
    })?;
    let value = row[1].as_f64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments("keyframe row value must be numeric".to_string())
    })?;
    let easing = row
        .get(2)
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(
                    "keyframe row easing must be a string".to_string(),
                )
            })
        })
        .transpose()?;

    Ok(ProjectActionKeyframe {
        at_seconds: frames_to_seconds(frame, project_fps(project)?),
        value: normalize_keyframe_value(source_property, property, value)?,
        easing,
    })
}

fn normalize_keyframe_value(
    source_property: &str,
    property: &ProjectActionKeyframeProperty,
    value: f64,
) -> Result<f64, CodexLocalToolError> {
    if source_property.trim() == "volume"
        && matches!(property, ProjectActionKeyframeProperty::VolumeDb)
    {
        return palmier_volume_to_db(value);
    }
    Ok(value)
}

fn timeline_item_number_property(item: &TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(Value::as_f64)
}

fn timeline_item_playback_speed(item: &TimelineItem) -> f64 {
    timeline_item_number_property(item, "speed")
        .filter(|speed| speed.is_finite() && *speed > 0.0)
        .unwrap_or(1.0)
}

fn timeline_item_source_window(item: &TimelineItem) -> (f64, f64, f64) {
    let speed = timeline_item_playback_speed(item);
    let source_in = timeline_item_number_property(item, "sourceIn")
        .filter(|source_in| source_in.is_finite() && *source_in >= 0.0)
        .unwrap_or(0.0);
    let default_source_out = source_in + item.duration_seconds.max(0.0) * speed;
    let source_out = timeline_item_number_property(item, "sourceOut")
        .filter(|source_out| source_out.is_finite() && *source_out > source_in)
        .unwrap_or(default_source_out);

    (source_in, source_out, speed)
}

/// The ordered timeline range over which source `[start, end]` plays in `item`,
/// mirrored for reversed clips.
fn timeline_range_for_source_seconds(
    item: &TimelineItem,
    (source_in, source_out, speed): (f64, f64, f64),
    (source_start, source_end): (f64, f64),
) -> (f64, f64) {
    SourceWindow {
        source_in,
        source_out,
        speed: speed.max(0.0001),
        reverse: is_reversed(item),
    }
    .timeline_range_for_source(item.start_seconds, source_start, source_end)
}

fn timeline_item_has_keyframe_track(item: &TimelineItem, key: &str) -> bool {
    item.properties
        .get("keyframes")
        .and_then(Value::as_object)
        .is_some_and(|keyframes| keyframes.contains_key(key))
}

fn timeline_item_string_property<'a>(item: &'a TimelineItem, key: &str) -> Option<&'a str> {
    item.properties.get(key).and_then(Value::as_str)
}

fn timeline_item_text(item: &TimelineItem) -> Option<&str> {
    match &item.source {
        TimelineSource::Text { text } => Some(text),
        _ => None,
    }
}

fn timeline_item_active_at(item: &TimelineItem, seconds: f64) -> bool {
    seconds >= item.start_seconds && seconds < item.start_seconds + item.duration_seconds
}

fn ripple_delete_ranges_payload(
    project: &VideoProject,
    args: RippleDeleteRangesArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        RippleDeleteRangesArgs::ProjectActions(args) => {
            mutating_project_actions_payload(project, args)
        }
        RippleDeleteRangesArgs::Direct(args) => ripple_delete_ranges_direct_payload(project, args),
    }
}

fn ripple_delete_ranges_direct_payload(
    project: &VideoProject,
    args: DirectRippleDeleteRangesArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = args.project_dir.clone();
    if args.ranges.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "ranges must not be empty".to_string(),
        ));
    }
    let ranges = ripple_delete_ranges_from_args(project, args)?
        .into_iter()
        .map(|mut range| {
            range.track_ids = expand_linked_ripple_track_ids(project, &range);
            range
        })
        .collect::<Vec<_>>();
    let ranges = normalize_ripple_delete_ranges(ranges);

    let affected_track_ids = ranges
        .iter()
        .flat_map(|range| range.track_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let fps = project_fps(project)?;
    let removed_frames = ranges
        .iter()
        .map(|range| seconds_to_frames(range.end_seconds - range.start_seconds, fps))
        .sum::<u64>();
    let action = ProjectAction::RippleDeleteRanges {
        ranges: ranges.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: vec![action],
        },
    )?;
    let mut projected = project.clone();
    apply_project_action(
        &mut projected,
        ProjectAction::RippleDeleteRanges {
            ranges: ranges.clone(),
        },
    )
    .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
    let resulting_clips = ripple_resulting_clips_payload(&projected, &affected_track_ids, fps);
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedTrackIds".to_string(), json!(affected_track_ids));
        object.insert("ranges".to_string(), json!(ranges));
        object.insert("removedFrames".to_string(), json!(removed_frames));
        object.insert("resultingClips".to_string(), json!(resulting_clips));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn ripple_resulting_clips_payload(
    project: &VideoProject,
    affected_track_ids: &[String],
    fps: f64,
) -> Vec<Value> {
    let affected_track_ids = affected_track_ids.iter().collect::<BTreeSet<_>>();
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media))
        .collect::<BTreeMap<_, _>>();
    project
        .timeline
        .tracks
        .iter()
        .filter(|track| affected_track_ids.contains(&track.id))
        .flat_map(|track| {
            track
                .items
                .iter()
                .take(MAX_TOOL_ITEMS_PER_TRACK)
                .map(|item| timeline_clip_payload(item, fps, &media_by_id))
        })
        .collect()
}

fn expand_linked_ripple_track_ids(
    project: &VideoProject,
    range: &ProjectActionRippleDeleteRange,
) -> Vec<String> {
    let anchor_track_ids = range.track_ids.iter().cloned().collect::<BTreeSet<_>>();
    let linked_groups = project
        .timeline
        .tracks
        .iter()
        .filter(|track| anchor_track_ids.contains(&track.id))
        .flat_map(|track| track.items.iter())
        .filter(|item| timeline_item_intersects_range(item, range.start_seconds, range.end_seconds))
        .filter_map(timeline_item_link_group_id)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if linked_groups.is_empty() {
        return range.track_ids.clone();
    }

    let mut track_ids = anchor_track_ids;
    for track in &project.timeline.tracks {
        if track.items.iter().any(|item| {
            timeline_item_intersects_range(item, range.start_seconds, range.end_seconds)
                && timeline_item_link_group_id(item)
                    .map(|group_id| linked_groups.contains(group_id))
                    .unwrap_or(false)
        }) {
            track_ids.insert(track.id.clone());
        }
    }
    track_ids.into_iter().collect()
}

fn normalize_ripple_delete_ranges(
    ranges: Vec<ProjectActionRippleDeleteRange>,
) -> Vec<ProjectActionRippleDeleteRange> {
    let mut normalized = ranges
        .into_iter()
        .map(|mut range| {
            range.track_ids = range
                .track_ids
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            range
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| {
        left.track_ids
            .cmp(&right.track_ids)
            .then_with(|| {
                left.start_seconds
                    .partial_cmp(&right.start_seconds)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                left.end_seconds
                    .partial_cmp(&right.end_seconds)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut merged: Vec<ProjectActionRippleDeleteRange> = Vec::new();
    for range in normalized {
        if let Some(previous) = merged.last_mut() {
            if previous.track_ids == range.track_ids && range.start_seconds <= previous.end_seconds
            {
                previous.end_seconds = previous.end_seconds.max(range.end_seconds);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn timeline_item_intersects_range(
    item: &TimelineItem,
    start_seconds: f64,
    end_seconds: f64,
) -> bool {
    let item_start = item.start_seconds;
    let item_end = item.start_seconds + item.duration_seconds;
    item_start < end_seconds && item_end > start_seconds
}

fn ripple_delete_ranges_from_args(
    project: &VideoProject,
    args: DirectRippleDeleteRangesArgs,
) -> Result<Vec<ProjectActionRippleDeleteRange>, CodexLocalToolError> {
    if args.track_index.is_some() && args.clip_id.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either trackIndex or clipId, not both".to_string(),
        ));
    }

    match (args.track_index, args.clip_id) {
        (Some(track_index), None) => {
            let units = ripple_delete_units(args.units.as_deref())?;
            if units != RippleDeleteUnits::Frames {
                return Err(CodexLocalToolError::InvalidArguments(
                    "trackIndex ripple_delete_ranges requires units='frames'".to_string(),
                ));
            }
            let track = project.timeline.tracks.get(track_index).ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(format!(
                    "trackIndex is out of range: {track_index}"
                ))
            })?;
            args.ranges
                .into_iter()
                .map(|range| {
                    let (start, end) = parse_ripple_range_pair(range)?;
                    Ok(ProjectActionRippleDeleteRange {
                        start_seconds: frame_number_to_seconds(start, project)?,
                        end_seconds: frame_number_to_seconds(end, project)?,
                        track_ids: vec![track.id.clone()],
                    })
                })
                .collect()
        }
        (None, Some(clip_id)) => {
            let item_id = resolve_timeline_item_id(project, &clip_id, "clipId")?;
            let (track, item) = timeline_item_with_track(project, &item_id)?;
            let units = ripple_delete_units(args.units.as_deref())?;
            let speed = timeline_item_playback_speed(item);
            let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
            let source_out = timeline_item_number_property(item, "sourceOut")
                .unwrap_or(source_in + item.duration_seconds * speed);
            args.ranges
                .into_iter()
                .map(|range| {
                    let (start, end) = parse_ripple_range_pair(range)?;
                    let (start_seconds, end_seconds) = match units {
                        RippleDeleteUnits::Frames => {
                            let start_offset = frame_number_to_seconds(start, project)?;
                            let end_offset = frame_number_to_seconds(end, project)?;
                            (
                                round_tool_seconds(item.start_seconds + start_offset),
                                round_tool_seconds(item.start_seconds + end_offset),
                            )
                        }
                        RippleDeleteUnits::Seconds => {
                            if start < source_in || end > source_out {
                                return Err(CodexLocalToolError::InvalidArguments(format!(
                                    "clipId range is outside visible source span for {item_id}"
                                )));
                            }
                            let (start_seconds, end_seconds) = timeline_range_for_source_seconds(
                                item,
                                (source_in, source_out, speed),
                                (start, end),
                            );
                            (
                                round_tool_seconds(start_seconds),
                                round_tool_seconds(end_seconds),
                            )
                        }
                    };
                    if start_seconds < item.start_seconds
                        || end_seconds > item.start_seconds + item.duration_seconds
                    {
                        return Err(CodexLocalToolError::InvalidArguments(format!(
                            "clipId range is outside timeline span for {item_id}"
                        )));
                    }
                    Ok(ProjectActionRippleDeleteRange {
                        start_seconds,
                        end_seconds,
                        track_ids: vec![track.id.clone()],
                    })
                })
                .collect()
        }
        (None, None) => args
            .ranges
            .into_iter()
            .map(|range| {
                serde_json::from_value(range)
                    .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))
            })
            .collect(),
        (Some(_), Some(_)) => Err(CodexLocalToolError::InvalidArguments(
            "pass either trackIndex or clipId, not both".to_string(),
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RippleDeleteUnits {
    Frames,
    Seconds,
}

fn ripple_delete_units(units: Option<&str>) -> Result<RippleDeleteUnits, CodexLocalToolError> {
    match units.unwrap_or("frames").trim() {
        "frames" => Ok(RippleDeleteUnits::Frames),
        "seconds" => Ok(RippleDeleteUnits::Seconds),
        other => Err(CodexLocalToolError::InvalidArguments(format!(
            "unsupported ripple_delete_ranges units: {other}"
        ))),
    }
}

fn parse_ripple_range_pair(range: Value) -> Result<(f64, f64), CodexLocalToolError> {
    let Value::Array(values) = range else {
        return Err(CodexLocalToolError::InvalidArguments(
            "Palmier ripple ranges must be [start, end] pairs".to_string(),
        ));
    };
    if values.len() != 2 {
        return Err(CodexLocalToolError::InvalidArguments(
            "Palmier ripple ranges must contain exactly two values".to_string(),
        ));
    }
    let start = values[0].as_f64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments("range start must be numeric".to_string())
    })?;
    let end = values[1].as_f64().ok_or_else(|| {
        CodexLocalToolError::InvalidArguments("range end must be numeric".to_string())
    })?;
    if !start.is_finite() || !end.is_finite() || end <= start {
        return Err(CodexLocalToolError::InvalidArguments(
            "range end must be greater than range start".to_string(),
        ));
    }
    Ok((start, end))
}

fn frame_number_to_seconds(frame: f64, project: &VideoProject) -> Result<f64, CodexLocalToolError> {
    if !frame.is_finite() || frame < 0.0 || frame.fract() != 0.0 || frame > i64::MAX as f64 {
        return Err(CodexLocalToolError::InvalidArguments(
            "frame values must be non-negative integers".to_string(),
        ));
    }
    Ok(frames_to_seconds(frame as u64, project_fps(project)?))
}

fn remove_words_payload(
    project: &VideoProject,
    args: RemoveWordsArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        RemoveWordsArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        RemoveWordsArgs::Direct(args) => remove_words_direct_payload(project, args),
    }
}

fn remove_words_direct_payload(
    project: &VideoProject,
    args: DirectRemoveWordsArgs,
) -> Result<Value, CodexLocalToolError> {
    validate_remove_words_options(&args)?;
    let project_dir = args.project_dir.clone();
    let word_indexes = remove_word_indexes_from_args(&args)?;
    if word_indexes.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "wordIndexes must not be empty".to_string(),
        ));
    }
    if args.media_id.is_none() {
        return remove_timeline_words_direct_payload(project, project_dir, &word_indexes);
    }
    let media_id = remove_words_media_id(project, args.media_id.as_deref(), &word_indexes)?;
    if !project.media.iter().any(|media| media.id == media_id) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "media was not found: {media_id}"
        )));
    }
    let transcript = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == media_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "transcript was not found for media: {media_id}"
            ))
        })?;

    let mut seen_word_indexes = BTreeSet::new();
    let mut ranges = Vec::new();
    let mut removed_word_ranges = Vec::new();
    for word_index in word_indexes {
        if !seen_word_indexes.insert(word_index) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "duplicate word index: {word_index}"
            )));
        }
        let word = transcript.words.get(word_index).ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "wordIndex {word_index} is outside transcript {}",
                transcript.id
            ))
        })?;
        if !word.start_seconds.is_finite()
            || !word.end_seconds.is_finite()
            || word.end_seconds <= word.start_seconds
        {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "wordIndex {word_index} has invalid timing"
            )));
        }

        let mut matched = false;
        for track in &project.timeline.tracks {
            for item in &track.items {
                let TimelineSource::Media {
                    media_id: item_media_id,
                } = &item.source
                else {
                    continue;
                };
                if item_media_id != &media_id {
                    continue;
                }
                let speed = timeline_item_playback_speed(item);
                let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
                let source_out = timeline_item_number_property(item, "sourceOut")
                    .unwrap_or(source_in + item.duration_seconds * speed);
                if word.start_seconds < source_in || word.end_seconds > source_out {
                    continue;
                }

                let (timeline_start_seconds, timeline_end_seconds) =
                    timeline_range_for_source_seconds(
                        item,
                        (source_in, source_out, speed),
                        (word.start_seconds, word.end_seconds),
                    );
                ranges.push(ProjectActionRippleDeleteRange {
                    start_seconds: timeline_start_seconds,
                    end_seconds: timeline_end_seconds,
                    track_ids: vec![track.id.clone()],
                });
                removed_word_ranges.push(json!({
                    "mediaId": media_id,
                    "transcriptId": transcript.id,
                    "wordIndex": word_index,
                    "text": word.text,
                    "trackId": track.id,
                    "itemId": item.id,
                    "timelineStartSeconds": round_tool_seconds(timeline_start_seconds),
                    "timelineEndSeconds": round_tool_seconds(timeline_end_seconds),
                    "sourceStartSeconds": word.start_seconds,
                    "sourceEndSeconds": word.end_seconds
                }));
                matched = true;
            }
        }

        if !matched {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "wordIndex {word_index} is not present in any source-backed timeline clip for media {media_id}"
            )));
        }
    }

    let ranges = ranges
        .into_iter()
        .map(|range| {
            let track_ids = expand_linked_ripple_track_ids(project, &range);
            ProjectActionRippleDeleteRange { track_ids, ..range }
        })
        .collect::<Vec<_>>();
    let ranges = normalize_ripple_delete_ranges(ranges);
    let affected_track_ids = ranges
        .iter()
        .flat_map(|range| range.track_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let action = ProjectAction::RippleDeleteRanges {
        ranges: ranges.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedTrackIds".to_string(), json!(affected_track_ids));
        object.insert("ranges".to_string(), json!(ranges));
        object.insert("removedWordRanges".to_string(), json!(removed_word_ranges));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn remove_timeline_words_direct_payload(
    project: &VideoProject,
    project_dir: Option<String>,
    word_indexes: &[usize],
) -> Result<Value, CodexLocalToolError> {
    let timeline_words = collect_timeline_transcript_word_matches(project);
    if timeline_words.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "No transcribable speech on the timeline".to_string(),
        ));
    }

    let mut seen_word_indexes = BTreeSet::new();
    let mut ranges = Vec::new();
    let mut removed_word_ranges = Vec::new();
    for word_index in word_indexes {
        if !seen_word_indexes.insert(*word_index) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "duplicate word index: {word_index}"
            )));
        }
        let word = timeline_words
            .iter()
            .find(|word| word.word_index == *word_index)
            .ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(format!(
                    "wordIndex {word_index} is outside the timeline transcript"
                ))
            })?;
        ranges.push(ProjectActionRippleDeleteRange {
            start_seconds: word.timeline_start_seconds,
            end_seconds: word.timeline_end_seconds,
            track_ids: vec![word.track_id.clone()],
        });
        removed_word_ranges.push(json!({
            "mediaId": word.media_id,
            "transcriptId": word.transcript_id,
            "wordIndex": word.word_index,
            "sourceWordIndex": word.source_word_index,
            "text": word.text,
            "trackId": word.track_id,
            "itemId": word.item_id,
            "timelineStartSeconds": round_tool_seconds(word.timeline_start_seconds),
            "timelineEndSeconds": round_tool_seconds(word.timeline_end_seconds),
            "sourceStartSeconds": word.source_start_seconds,
            "sourceEndSeconds": word.source_end_seconds
        }));
    }

    let ranges = ranges
        .into_iter()
        .map(|range| {
            let track_ids = expand_linked_ripple_track_ids(project, &range);
            ProjectActionRippleDeleteRange { track_ids, ..range }
        })
        .collect::<Vec<_>>();
    let ranges = normalize_ripple_delete_ranges(ranges);
    let affected_track_ids = ranges
        .iter()
        .flat_map(|range| range.track_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let action = ProjectAction::RippleDeleteRanges {
        ranges: ranges.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: vec![action],
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedTrackIds".to_string(), json!(affected_track_ids));
        object.insert("ranges".to_string(), json!(ranges));
        object.insert("removedWordRanges".to_string(), json!(removed_word_ranges));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn remove_silence_payload(
    project: &VideoProject,
    args: RemoveSilenceArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        RemoveSilenceArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        RemoveSilenceArgs::Direct(args) => remove_silence_direct_payload(project, args),
    }
}

fn remove_silence_direct_payload(
    project: &VideoProject,
    args: DirectRemoveSilenceArgs,
) -> Result<Value, CodexLocalToolError> {
    let timeline_words = collect_timeline_transcript_word_matches(project);
    if timeline_words.is_empty() && project.media_silence_ranges.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "No dead air on the timeline. Speech analysis may still be running, or the timeline has no transcribable speech.".to_string(),
        ));
    }

    let mut words_by_item = BTreeMap::<String, Vec<&TimelineTranscriptWordMatch>>::new();
    for word in &timeline_words {
        words_by_item
            .entry(word.item_id.clone())
            .or_default()
            .push(word);
    }

    let mut ranges = Vec::new();
    let mut removed_silence_ranges = Vec::new();
    for words in words_by_item.values_mut() {
        words.sort_by(|left, right| {
            left.timeline_start_seconds
                .partial_cmp(&right.timeline_start_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.source_word_index.cmp(&right.source_word_index))
        });
        for pair in words.windows(2) {
            let previous = pair[0];
            let next = pair[1];
            let gap_seconds = next.timeline_start_seconds - previous.timeline_end_seconds;
            if gap_seconds < REMOVE_SILENCE_MIN_GAP_SECONDS {
                continue;
            }
            let timeline_start_seconds =
                previous.timeline_end_seconds + REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS;
            let timeline_end_seconds =
                next.timeline_start_seconds - REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS;
            if timeline_end_seconds <= timeline_start_seconds {
                continue;
            }
            // A reversed clip plays the later source word first.
            let source_gap = (
                previous.source_end_seconds.min(next.source_end_seconds),
                previous.source_start_seconds.max(next.source_start_seconds),
            );
            ranges.push(ProjectActionRippleDeleteRange {
                start_seconds: round_tool_seconds(timeline_start_seconds),
                end_seconds: round_tool_seconds(timeline_end_seconds),
                track_ids: vec![previous.track_id.clone()],
            });
            removed_silence_ranges.push(json!({
                "mediaId": previous.media_id,
                "transcriptId": previous.transcript_id,
                "trackId": previous.track_id,
                "itemId": previous.item_id,
                "previousWordIndex": previous.word_index,
                "nextWordIndex": next.word_index,
                "timelineStartSeconds": round_tool_seconds(timeline_start_seconds),
                "timelineEndSeconds": round_tool_seconds(timeline_end_seconds),
                "sourceStartSeconds": round_tool_seconds(source_gap.0 + REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS),
                "sourceEndSeconds": round_tool_seconds(source_gap.1 - REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS),
                "gapSeconds": round_tool_seconds(gap_seconds)
            }));
        }
    }
    let analysis_ranges = media_silence_ripple_ranges(project);
    for (range, payload) in analysis_ranges {
        ranges.push(range);
        removed_silence_ranges.push(payload);
    }

    if ranges.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "No dead air on the timeline. Speech analysis may still be running, or the audio has no long quiet non-speech sections.".to_string(),
        ));
    }

    let ranges = ranges
        .into_iter()
        .map(|range| {
            let track_ids = expand_linked_ripple_track_ids(project, &range);
            ProjectActionRippleDeleteRange { track_ids, ..range }
        })
        .collect::<Vec<_>>();
    let ranges = normalize_ripple_delete_ranges(ranges);
    let affected_track_ids = ranges
        .iter()
        .flat_map(|range| range.track_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let fps = project_fps(project)?;
    let removed_frames = ranges
        .iter()
        .map(|range| seconds_to_frames(range.end_seconds - range.start_seconds, fps))
        .sum::<u64>();
    let action = ProjectAction::RippleDeleteRanges {
        ranges: ranges.clone(),
    };
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions: vec![action],
        },
    )?;
    let mut projected = project.clone();
    apply_project_action(
        &mut projected,
        ProjectAction::RippleDeleteRanges {
            ranges: ranges.clone(),
        },
    )
    .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
    let resulting_clips = ripple_resulting_clips_payload(&projected, &affected_track_ids, fps);
    if let Some(object) = payload.as_object_mut() {
        object.insert("affectedTrackIds".to_string(), json!(affected_track_ids));
        object.insert("ranges".to_string(), json!(ranges));
        object.insert(
            "removedSilenceRanges".to_string(),
            json!(removed_silence_ranges),
        );
        object.insert(
            "sectionsRemoved".to_string(),
            json!(removed_silence_ranges.len()),
        );
        object.insert("removedFrames".to_string(), json!(removed_frames));
        object.insert("resultingClips".to_string(), json!(resulting_clips));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn media_silence_ripple_ranges(
    project: &VideoProject,
) -> Vec<(ProjectActionRippleDeleteRange, Value)> {
    let mut ranges = Vec::new();
    let mut seen_timeline_ranges = BTreeSet::new();
    for silence in &project.media_silence_ranges {
        if !valid_media_silence_range(silence) {
            continue;
        }
        let source_start_seconds = silence.source_in + REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS;
        let source_end_seconds = silence.source_out - REMOVE_SILENCE_BOUNDARY_PADDING_SECONDS;
        if source_end_seconds <= source_start_seconds
            || source_end_seconds - source_start_seconds < REMOVE_SILENCE_MIN_GAP_SECONDS
        {
            continue;
        }
        for track in &project.timeline.tracks {
            for item in &track.items {
                let TimelineSource::Media { media_id } = &item.source else {
                    continue;
                };
                if media_id != &silence.media_id {
                    continue;
                }
                let speed = timeline_item_playback_speed(item);
                let item_source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
                let item_source_out = timeline_item_number_property(item, "sourceOut")
                    .unwrap_or(item_source_in + item.duration_seconds * speed);
                if source_start_seconds < item_source_in || source_end_seconds > item_source_out {
                    continue;
                }
                let (timeline_start_seconds, timeline_end_seconds) =
                    timeline_range_for_source_seconds(
                        item,
                        (item_source_in, item_source_out, speed),
                        (source_start_seconds, source_end_seconds),
                    );
                if timeline_end_seconds <= timeline_start_seconds {
                    continue;
                }
                let rounded_start = round_tool_seconds(timeline_start_seconds);
                let rounded_end = round_tool_seconds(timeline_end_seconds);
                if !seen_timeline_ranges.insert((rounded_start.to_bits(), rounded_end.to_bits())) {
                    continue;
                }
                ranges.push((
                    ProjectActionRippleDeleteRange {
                        start_seconds: rounded_start,
                        end_seconds: rounded_end,
                        track_ids: vec![track.id.clone()],
                    },
                    json!({
                        "mediaId": silence.media_id,
                        "trackId": track.id,
                        "itemId": item.id,
                        "timelineStartSeconds": rounded_start,
                        "timelineEndSeconds": rounded_end,
                        "sourceStartSeconds": round_tool_seconds(source_start_seconds),
                        "sourceEndSeconds": round_tool_seconds(source_end_seconds),
                        "confidence": round_tool_seconds(silence.confidence),
                        "label": silence.label
                    }),
                ));
            }
        }
    }
    ranges
}

fn valid_media_silence_range(silence: &MediaSilenceRange) -> bool {
    !silence.media_id.trim().is_empty()
        && silence.source_in.is_finite()
        && silence.source_out.is_finite()
        && silence.source_out > silence.source_in
}

fn validate_remove_words_options(args: &DirectRemoveWordsArgs) -> Result<(), CodexLocalToolError> {
    if let Some(cut_aggressiveness) = args.cut_aggressiveness.as_deref() {
        match cut_aggressiveness.trim() {
            "tight" | "balanced" | "loose" => {}
            other => {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "unsupported cutAggressiveness: {other}"
                )));
            }
        }
    }
    if let Some(language) = args.language.as_deref() {
        trim_required(language, "language")?;
    }
    Ok(())
}

fn remove_word_indexes_from_args(
    args: &DirectRemoveWordsArgs,
) -> Result<Vec<usize>, CodexLocalToolError> {
    if !args.word_indexes.is_empty() && !args.words.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either wordIndexes or words, not both".to_string(),
        ));
    }
    if !args.word_indexes.is_empty() {
        return Ok(args.word_indexes.clone());
    }

    let mut indexes = Vec::new();
    for word in &args.words {
        match word {
            Value::Number(number) => {
                let index = number.as_u64().ok_or_else(|| {
                    CodexLocalToolError::InvalidArguments(
                        "words entries must be non-negative integer indexes".to_string(),
                    )
                })?;
                indexes.push(index as usize);
            }
            Value::Array(values) => {
                if values.len() != 2 {
                    return Err(CodexLocalToolError::InvalidArguments(
                        "word ranges must be [startIndex, endIndex]".to_string(),
                    ));
                }
                let start = values[0].as_u64().ok_or_else(|| {
                    CodexLocalToolError::InvalidArguments(
                        "word range start must be a non-negative integer".to_string(),
                    )
                })? as usize;
                let end = values[1].as_u64().ok_or_else(|| {
                    CodexLocalToolError::InvalidArguments(
                        "word range end must be a non-negative integer".to_string(),
                    )
                })? as usize;
                if end < start {
                    return Err(CodexLocalToolError::InvalidArguments(
                        "word range end must be greater than or equal to start".to_string(),
                    ));
                }
                indexes.extend(start..=end);
            }
            _ => {
                return Err(CodexLocalToolError::InvalidArguments(
                    "words entries must be indexes or [startIndex, endIndex] ranges".to_string(),
                ));
            }
        }
    }
    Ok(indexes)
}

fn remove_words_media_id(
    project: &VideoProject,
    media_id: Option<&str>,
    word_indexes: &[usize],
) -> Result<String, CodexLocalToolError> {
    if let Some(media_id) = media_id {
        return trim_required(media_id, "mediaId");
    }

    let matching_media_ids = project
        .transcripts
        .iter()
        .filter(|transcript| {
            word_indexes
                .iter()
                .all(|word_index| *word_index < transcript.words.len())
                && media_is_visible_on_timeline(project, &transcript.media_id)
        })
        .map(|transcript| transcript.media_id.clone())
        .collect::<BTreeSet<_>>();

    match matching_media_ids.len() {
        1 => Ok(matching_media_ids.into_iter().next().expect("one media id")),
        0 => Err(CodexLocalToolError::InvalidArguments(
            "mediaId is required because no visible transcript contains every requested word index"
                .to_string(),
        )),
        _ => Err(CodexLocalToolError::InvalidArguments(
            "mediaId is required because requested word indexes are ambiguous".to_string(),
        )),
    }
}

fn media_is_visible_on_timeline(project: &VideoProject, media_id: &str) -> bool {
    project.timeline.tracks.iter().any(|track| {
        track.items.iter().any(|item| {
            matches!(
                &item.source,
                TimelineSource::Media {
                    media_id: item_media_id
                } if item_media_id == media_id
            )
        })
    })
}

fn round_tool_seconds(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn project_fps(project: &VideoProject) -> Result<f64, CodexLocalToolError> {
    let fps = project.render_settings.fps;
    if !fps.is_finite() || fps <= 0.0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "project fps must be finite and greater than zero".to_string(),
        ));
    }
    Ok(fps)
}

fn frames_to_seconds(frames: u64, fps: f64) -> f64 {
    round_tool_seconds(frames as f64 / fps)
}

fn seconds_to_frames(seconds: f64, fps: f64) -> u64 {
    (seconds.max(0.0) * fps).round() as u64
}

fn remove_tracks_payload(
    project: &VideoProject,
    args: RemoveTracksArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        RemoveTracksArgs::Direct(args) => args,
        RemoveTracksArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    let has_track_indexes = !args.track_indexes.is_empty();
    let has_track_ids = !args.track_ids.is_empty();
    if has_track_indexes == has_track_ids {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either trackIndexes or trackIds".to_string(),
        ));
    }

    let mut track_ids = Vec::new();
    let mut removed_tracks = Vec::new();
    let mut push_track =
        |track_index: usize, track: &TimelineTrack| -> Result<(), CodexLocalToolError> {
            if track_ids.iter().any(|id| id == &track.id) {
                return Ok(());
            }
            track_ids.push(track.id.clone());
            removed_tracks.push(json!({
                "trackIndex": track_index,
                "trackId": track.id,
                "label": track.name,
                "itemCount": track.items.len()
            }));
            Ok(())
        };

    if has_track_indexes {
        for track_index in args.track_indexes {
            let Some(track) = project.timeline.tracks.get(track_index) else {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "track index {track_index} out of range"
                )));
            };
            push_track(track_index, track)?;
        }
    } else {
        for track_id in args.track_ids {
            let track_id = track_id.trim();
            if track_id.is_empty() {
                return Err(CodexLocalToolError::InvalidArguments(
                    "trackIds must not contain empty ids".to_string(),
                ));
            }
            let Some((track_index, track)) = project
                .timeline
                .tracks
                .iter()
                .enumerate()
                .find(|(_, track)| track.id == track_id)
            else {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "track id {track_id} not found"
                )));
            };
            push_track(track_index, track)?;
        }
    }

    let actions = vec![ProjectAction::RemoveTracks {
        track_ids: track_ids.clone(),
    }];
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("removedTracks".to_string(), json!(removed_tracks));
        object.insert("changedTrackIds".to_string(), json!(track_ids));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn rename_media_payload(
    project: &VideoProject,
    args: RenameMediaArgs,
) -> Result<Value, CodexLocalToolError> {
    let has_single = args.media_ref.is_some() || args.name.is_some();
    let has_entries = !args.entries.is_empty();
    if has_single == has_entries {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either mediaRef/name or entries, not both".to_string(),
        ));
    }
    let entries = if has_entries {
        args.entries
    } else {
        vec![RenameMediaEntry {
            media_ref: args.media_ref.unwrap_or_default(),
            name: args.name.unwrap_or_default(),
        }]
    };

    let actions = entries
        .iter()
        .map(|entry| ProjectAction::RenameMedia {
            media_id: entry.media_ref.clone(),
            name: entry.name.clone(),
        })
        .collect::<Vec<_>>();
    let renamed = entries
        .iter()
        .map(|entry| {
            json!({
                "mediaRef": entry.media_ref,
                "name": entry.name
            })
        })
        .collect::<Vec<_>>();
    let changed_media_ids = entries
        .iter()
        .map(|entry| entry.media_ref.clone())
        .collect::<Vec<_>>();

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("renamedMedia".to_string(), json!(renamed));
        object.insert("changedMediaIds".to_string(), json!(changed_media_ids));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_media"),
        );
    }
    Ok(payload)
}

fn delete_media_payload(
    project: &VideoProject,
    args: DeleteMediaArgs,
) -> Result<Value, CodexLocalToolError> {
    if args.asset_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "assetIds must not be empty".to_string(),
        ));
    }

    let removed_timeline_items = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| {
            args.asset_ids.iter().flat_map(move |asset_id| {
                track
                    .items
                    .iter()
                    .filter_map(move |item| match &item.source {
                        TimelineSource::Media { media_id } if media_id == asset_id => Some(json!({
                            "trackId": track.id,
                            "itemId": item.id,
                            "mediaId": media_id
                        })),
                        _ => None,
                    })
            })
        })
        .collect::<Vec<_>>();

    let actions = vec![ProjectAction::DeleteMedia {
        media_ids: args.asset_ids.clone(),
    }];
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("deletedMediaIds".to_string(), json!(args.asset_ids));
        object.insert(
            "removedTimelineItems".to_string(),
            json!(removed_timeline_items),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_media"),
        );
    }
    Ok(payload)
}

fn create_folder_payload(
    project: &VideoProject,
    args: CreateFolderArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        CreateFolderArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        CreateFolderArgs::Direct(args) => {
            let has_single =
                args.folder_id.is_some() || args.name.is_some() || args.parent_id.is_some();
            let has_entries = !args.entries.is_empty();
            if has_single == has_entries {
                return Err(CodexLocalToolError::InvalidArguments(
                    "pass either name/parentFolderId or entries, not both".to_string(),
                ));
            }
            let mut used_folder_ids = project
                .media_folders
                .iter()
                .map(|folder| folder.id.clone())
                .collect::<BTreeSet<_>>();
            let entries = if has_entries {
                args.entries
            } else {
                vec![CreateFolderEntry {
                    folder_id: args.folder_id,
                    name: args.name.unwrap_or_default(),
                    parent_id: args.parent_id,
                }]
            };

            let mut folders = Vec::with_capacity(entries.len());
            for entry in entries {
                let name = trim_required(&entry.name, "name")?;
                let folder_id = entry
                    .folder_id
                    .unwrap_or_else(|| generate_media_folder_id(&name, &mut used_folder_ids));
                used_folder_ids.insert(folder_id.clone());
                folders.push(MediaFolder {
                    id: folder_id,
                    name,
                    parent_id: entry.parent_id,
                });
            }

            let actions = folders
                .iter()
                .cloned()
                .map(|folder| ProjectAction::CreateMediaFolder { folder })
                .collect::<Vec<_>>();
            let affected_folder_ids = folders
                .iter()
                .map(|folder| folder.id.clone())
                .collect::<Vec<_>>();
            let direct_folder = if !has_entries && folders.len() == 1 {
                folders.first().cloned()
            } else {
                None
            };
            let created_folders = folders
                .iter()
                .map(|folder| {
                    json!({
                        "id": folder.id,
                        "folderId": folder.id,
                        "name": folder.name,
                        "parentFolderId": folder.parent_id
                    })
                })
                .collect::<Vec<_>>();
            let mut payload = mutating_project_actions_payload(
                project,
                ToolProjectActionsArgs {
                    project_dir: args.project_dir,
                    actions,
                },
            )?;
            if let Some(object) = payload.as_object_mut() {
                if let Some(folder) = direct_folder {
                    object.insert("id".to_string(), json!(folder.id));
                    object.insert("name".to_string(), json!(folder.name));
                    object.insert("parentFolderId".to_string(), json!(folder.parent_id));
                }
                object.insert("affectedFolderIds".to_string(), json!(affected_folder_ids));
                object.insert("folders".to_string(), json!(created_folders));
                object.insert(
                    "nextRecommendedInspection".to_string(),
                    json!("video_creater.get_media"),
                );
            }
            Ok(payload)
        }
    }
}

fn move_to_folder_payload(
    project: &VideoProject,
    args: MoveToFolderArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        MoveToFolderArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        MoveToFolderArgs::Direct(args) => {
            let has_single = !args.media_ids.is_empty() || args.folder_id.is_some();
            let has_entries = !args.entries.is_empty();
            if has_single == has_entries {
                return Err(CodexLocalToolError::InvalidArguments(
                    "pass either assetIds/folderId or entries, not both".to_string(),
                ));
            }
            let entries = if has_entries {
                args.entries
            } else {
                vec![MoveToFolderEntry {
                    asset_ids: args.media_ids,
                    folder_id: args.folder_id,
                }]
            };

            let mut actions = Vec::new();
            let mut changed_media_ids = Vec::new();
            let mut moves = Vec::new();
            for entry in entries {
                if entry.asset_ids.is_empty() {
                    return Err(CodexLocalToolError::InvalidArguments(
                        "assetIds must not be empty".to_string(),
                    ));
                }
                for media_id in &entry.asset_ids {
                    actions.push(ProjectAction::AssignMediaFolder {
                        media_id: media_id.clone(),
                        folder_id: entry.folder_id.clone(),
                    });
                    changed_media_ids.push(media_id.clone());
                }
                moves.push(json!({
                    "assetIds": entry.asset_ids,
                    "folderId": entry.folder_id
                }));
            }
            let mut payload = mutating_project_actions_payload(
                project,
                ToolProjectActionsArgs {
                    project_dir: args.project_dir,
                    actions,
                },
            )?;
            if let Some(object) = payload.as_object_mut() {
                if moves.len() == 1 {
                    object.insert("targetFolderId".to_string(), moves[0]["folderId"].clone());
                }
                object.insert("changedMediaIds".to_string(), json!(changed_media_ids));
                object.insert("moves".to_string(), json!(moves));
                object.insert(
                    "nextRecommendedInspection".to_string(),
                    json!("video_creater.get_media"),
                );
            }
            Ok(payload)
        }
    }
}

fn generate_media_folder_id(name: &str, used_folder_ids: &mut BTreeSet<String>) -> String {
    let slug = media_folder_id_slug(name);
    let base = if slug.is_empty() {
        "folder".to_string()
    } else {
        format!("folder-{slug}")
    };
    if !used_folder_ids.contains(&base) {
        return base;
    }

    let mut index = 2;
    loop {
        let candidate = format!("{base}-{index}");
        if !used_folder_ids.contains(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn media_folder_id_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn rename_folder_payload(
    project: &VideoProject,
    args: RenameFolderArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        RenameFolderArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        RenameFolderArgs::Direct(args) => {
            let has_single = args.folder_id.is_some() || args.name.is_some();
            let has_entries = !args.entries.is_empty();
            if has_single == has_entries {
                return Err(CodexLocalToolError::InvalidArguments(
                    "pass either folderId/name or entries, not both".to_string(),
                ));
            }
            let entries = if has_entries {
                args.entries
            } else {
                vec![RenameFolderEntry {
                    folder_id: args.folder_id.unwrap_or_default(),
                    name: args.name.unwrap_or_default(),
                }]
            };

            let actions = entries
                .iter()
                .map(|entry| ProjectAction::RenameMediaFolder {
                    folder_id: entry.folder_id.clone(),
                    name: entry.name.clone(),
                })
                .collect::<Vec<_>>();
            let renamed = entries
                .iter()
                .map(|entry| {
                    json!({
                        "folderId": entry.folder_id,
                        "name": entry.name
                    })
                })
                .collect::<Vec<_>>();
            let affected_folder_ids = entries
                .iter()
                .map(|entry| entry.folder_id.clone())
                .collect::<Vec<_>>();

            let mut payload = mutating_project_actions_payload(
                project,
                ToolProjectActionsArgs {
                    project_dir: args.project_dir,
                    actions,
                },
            )?;
            if let Some(object) = payload.as_object_mut() {
                object.insert("renamedFolders".to_string(), json!(renamed));
                object.insert("affectedFolderIds".to_string(), json!(affected_folder_ids));
                object.insert(
                    "nextRecommendedInspection".to_string(),
                    json!("video_creater.get_media"),
                );
            }
            Ok(payload)
        }
    }
}

fn delete_folder_payload(
    project: &VideoProject,
    args: DeleteFolderArgs,
) -> Result<Value, CodexLocalToolError> {
    match args {
        DeleteFolderArgs::ProjectActions(args) => mutating_project_actions_payload(project, args),
        DeleteFolderArgs::Direct(args) => {
            if args.folder_ids.is_empty() {
                return Err(CodexLocalToolError::InvalidArguments(
                    "folderIds must not be empty".to_string(),
                ));
            }
            let actions = args
                .folder_ids
                .iter()
                .map(|folder_id| ProjectAction::DeleteMediaFolder {
                    folder_id: folder_id.clone(),
                })
                .collect::<Vec<_>>();
            let mut payload = mutating_project_actions_payload(
                project,
                ToolProjectActionsArgs {
                    project_dir: args.project_dir,
                    actions,
                },
            )?;
            if let Some(object) = payload.as_object_mut() {
                object.insert("affectedFolderIds".to_string(), json!(args.folder_ids));
                object.insert(
                    "nextRecommendedInspection".to_string(),
                    json!("video_creater.get_media"),
                );
            }
            Ok(payload)
        }
    }
}

fn set_project_settings_payload(
    project: &VideoProject,
    args: SetProjectSettingsArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        SetProjectSettingsArgs::Direct(args) => args,
        SetProjectSettingsArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    let mut settings = project.render_settings.clone();
    if args.fps.is_none()
        && args.width.is_none()
        && args.height.is_none()
        && args.aspect_ratio.is_none()
        && args.quality.is_none()
        && args.loudness_lufs.is_none()
        && args.captions.is_none()
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "provide at least one project setting".to_string(),
        ));
    }
    if args.aspect_ratio.is_some() && (args.width.is_some() || args.height.is_some()) {
        return Err(CodexLocalToolError::InvalidArguments(
            "aspectRatio and explicit width/height are mutually exclusive".to_string(),
        ));
    }

    if let Some(fps) = args.fps {
        if !fps.is_finite() || !(1.0..=120.0).contains(&fps) {
            return Err(CodexLocalToolError::InvalidArguments(
                "fps must be between 1 and 120".to_string(),
            ));
        }
        settings.fps = fps;
    }
    if let Some(loudness_lufs) = args.loudness_lufs {
        if !loudness_lufs.is_finite() {
            return Err(CodexLocalToolError::InvalidArguments(
                "loudnessLufs must be finite".to_string(),
            ));
        }
        settings.loudness_lufs = loudness_lufs;
    }
    if let Some(captions) = args.captions {
        settings.captions = captions;
    }

    if let Some(aspect_ratio) = args.aspect_ratio {
        let (width, height) = aspect_ratio_dimensions(&aspect_ratio)?;
        settings.width = width;
        settings.height = height;
    } else {
        if let Some(width) = args.width {
            settings.width = width;
        }
        if let Some(height) = args.height {
            settings.height = height;
        }
    }
    if let Some(quality) = args.quality {
        let (width, height) =
            scale_resolution_to_quality(settings.width, settings.height, &quality)?;
        settings.width = width;
        settings.height = height;
    }

    let actions = vec![ProjectAction::UpdateRenderSettings {
        settings: settings.clone(),
    }];
    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir: args.project_dir,
            actions,
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "settings".to_string(),
            json!({
                "width": settings.width,
                "height": settings.height,
                "fps": settings.fps,
                "loudnessLufs": settings.loudness_lufs,
                "captions": settings.captions
            }),
        );
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.project_context"),
        );
    }
    Ok(payload)
}

fn sync_audio_payload(
    project: &VideoProject,
    args: SyncAudioArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = match args {
        SyncAudioArgs::Direct(args) => args,
        SyncAudioArgs::ProjectActions(args) => {
            return mutating_project_actions_payload(project, args);
        }
    };
    let project_dir = args.project_dir.clone();
    let mut target_clip_ids = args.target_clip_ids;
    if let Some(target_clip_id) = args.target_clip_id {
        target_clip_ids.push(target_clip_id);
    }
    if target_clip_ids.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "sync_audio: provide targetClipId or targetClipIds".to_string(),
        ));
    }

    let report = build_audio_sync_report(
        project,
        AudioSyncRequest {
            reference_clip_id: args.reference_clip_id,
            target_clip_ids,
            project_dir: args.project_dir.map(PathBuf::from),
            search_window_seconds: args.search_window_seconds.unwrap_or(30.0),
            min_confidence: args.min_confidence.unwrap_or(0.5),
        },
    )
    .map_err(|error| CodexLocalToolError::InvalidArguments(format!("sync_audio: {error}")))?;

    if report.synced.is_empty() {
        let reason = report
            .failed
            .first()
            .map(|failure| failure.reason.as_str())
            .unwrap_or("no clips aligned");
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "sync_audio: {reason}"
        )));
    }

    let mut payload = mutating_project_actions_payload(
        project,
        ToolProjectActionsArgs {
            project_dir,
            actions: report.actions.clone(),
        },
    )?;
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "referenceClipId".to_string(),
            json!(report.reference_clip_id),
        );
        object.insert("synced".to_string(), json!(report.synced));
        object.insert("failed".to_string(), json!(report.failed));
        object.insert("actions".to_string(), json!(report.actions));
        object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.get_timeline"),
        );
    }
    Ok(payload)
}

fn send_feedback_payload(
    project: &VideoProject,
    args: SendFeedbackArgs,
) -> Result<Value, CodexLocalToolError> {
    let category = args.category.trim();
    if !matches!(
        category,
        "missing_capability" | "wrong_result" | "confusing_ux" | "failure" | "suggestion"
    ) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "invalid feedback category: {category}"
        )));
    }

    let summary = trim_limited(&args.summary, "summary", 160)?;
    if summary.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "summary must not be blank".to_string(),
        ));
    }
    let details = args
        .details
        .as_deref()
        .map(|details| trim_limited(details, "details", 1_500))
        .transpose()?
        .filter(|details| !details.is_empty());
    let severity = args
        .severity
        .as_deref()
        .map(str::trim)
        .filter(|severity| !severity.is_empty())
        .unwrap_or("low");
    if !matches!(severity, "low" | "medium" | "high") {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "invalid feedback severity: {severity}"
        )));
    }

    Ok(json!({
        "accepted": true,
        "submitted": false,
        "delivery": "local_handoff",
        "report": {
            "category": category,
            "summary": summary,
            "details": details,
            "severity": severity,
            "projectIdPrefix": project.id.chars().take(8).collect::<String>()
        },
        "policy": {
            "projectContentIncluded": false,
            "providerCredentialsExposed": false,
            "verbatimUserContentAllowed": false
        },
        "nextStep": "Host app can submit this sanitized report through its own feedback transport."
    }))
}

fn trim_limited(value: &str, label: &str, max_chars: usize) -> Result<String, CodexLocalToolError> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max_chars {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{label} must be at most {max_chars} characters"
        )));
    }

    Ok(trimmed.to_string())
}

fn aspect_ratio_dimensions(aspect_ratio: &str) -> Result<(u32, u32), CodexLocalToolError> {
    match aspect_ratio {
        "16:9" => Ok((1920, 1080)),
        "9:16" => Ok((1080, 1920)),
        "1:1" => Ok((1080, 1080)),
        "4:3" => Ok((1440, 1080)),
        "2.4:1" => Ok((2400, 1000)),
        "9:14" => Ok((1080, 1680)),
        _ => Err(CodexLocalToolError::InvalidArguments(format!(
            "unknown aspectRatio: {aspect_ratio}"
        ))),
    }
}

fn scale_resolution_to_quality(
    width: u32,
    height: u32,
    quality: &str,
) -> Result<(u32, u32), CodexLocalToolError> {
    let target_short_edge = match quality {
        "720p" => 720.0,
        "1080p" => 1080.0,
        "2K" => 1440.0,
        "4K" => 2160.0,
        _ => {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "unknown quality: {quality}"
            )))
        }
    };
    if width == 0 || height == 0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "resolution must have positive width and height".to_string(),
        ));
    }

    let width = width as f64;
    let height = height as f64;
    if width >= height {
        Ok((
            (target_short_edge * (width / height)).round().max(1.0) as u32,
            target_short_edge as u32,
        ))
    } else {
        Ok((
            target_short_edge as u32,
            (target_short_edge * (height / width)).round().max(1.0) as u32,
        ))
    }
}

fn import_media_payload(
    project: &VideoProject,
    args: ImportMediaArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = PathBuf::from(import_media_project_dir(project, &args)?);
    let _project_lease = acquire_split_project_mutation_lease(&project_dir)
        .map_err(CodexLocalToolError::ProjectActionApplication)?;
    let canonical_project;
    let project = if project_uses_split_layout(&project_dir)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?
    {
        canonical_project = load_split_project(&project_dir)
            .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
        &canonical_project
    } else {
        project
    };
    let import_folder_id = resolve_import_folder_id(project, args.folder_id.as_deref())?;
    let import_sources = import_media_sources(&args, &project_dir)?;
    if import_sources.paths.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "sourcePaths must not be empty".to_string(),
        ));
    }

    let mut result = import_media_files(&project_dir, project.clone(), &import_sources.paths)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    if let Some(folder_id) = import_folder_id {
        apply_import_folder_id(
            &project_dir,
            &mut result.project,
            &mut result.imported,
            folder_id,
        )?;
    } else if !import_sources.mirrored_folders.is_empty() {
        apply_import_mirrored_folders(
            &project_dir,
            &mut result.project,
            &mut result.imported,
            &import_sources,
        )?;
    }

    Ok(json!({
        "projectId": result.project.id,
        "projectDir": project_dir.display().to_string(),
        "imported": result.imported.iter().map(media_payload).collect::<Vec<_>>(),
        "skipped": result.skipped,
        "importedCount": result.imported.len(),
        "skippedCount": result.skipped.len()
    }))
}

fn create_matte_payload(
    project: &VideoProject,
    args: CreateMatteArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = PathBuf::from(create_matte_project_dir(
        project,
        args.project_dir.as_deref(),
    )?);
    let result = create_matte_for_project(
        &project_dir,
        project,
        MatteRequest {
            hex: args.hex,
            aspect_ratio: args.aspect_ratio.unwrap_or_else(|| "Project".to_string()),
            name: args.name,
            folder_id: args.folder_id,
        },
    )
    .map_err(|error| match error {
        MatteError::InvalidArgument(message) => CodexLocalToolError::InvalidArguments(message),
        MatteError::Persistence(message) => CodexLocalToolError::ProjectActionApplication(message),
    })?;
    let asset = result.media;
    let name = asset.name.clone().unwrap_or_else(|| "Matte".to_string());
    let width = asset.width.unwrap_or_default();
    let height = asset.height.unwrap_or_default();
    let folder_id = asset.folder_id.clone();
    let relative_path = asset.relative_path.clone();

    Ok(json!({
        "valid": true,
        "projectId": result.project.id,
        "projectDir": project_dir.display().to_string(),
        "mediaRef": asset.id,
        "id": asset.id,
        "name": name,
        "kind": "image",
        "relativePath": relative_path,
        "width": width,
        "height": height,
        "folderId": folder_id,
        "media": media_payload(&asset),
        "nextRecommendedInspection": "video_creater.get_media"
    }))
}

const DEFAULT_AGENT_WORKSPACE_ROOT: &str = "/tmp/video-creater-agent";

/// Fallback package location for agent tools called without `projectDir`.
///
/// Canonical project writers lock a sibling file in the package's parent directory, so the shared
/// agent workspace root must exist before a first-run import, matte, or render can take the lease.
/// Creation is best effort: if it fails, the lease acquisition reports the concrete error.
fn default_agent_project_dir(project: &VideoProject) -> String {
    let _ = fs::create_dir_all(DEFAULT_AGENT_WORKSPACE_ROOT);
    format!(
        "{DEFAULT_AGENT_WORKSPACE_ROOT}/{}",
        export_project_slug(project)
    )
}

fn create_matte_project_dir(
    project: &VideoProject,
    project_dir: Option<&str>,
) -> Result<String, CodexLocalToolError> {
    if let Some(project_dir) = project_dir {
        return trim_required(project_dir, "projectDir");
    }
    Ok(default_agent_project_dir(project))
}

fn import_media_project_dir(
    project: &VideoProject,
    args: &ImportMediaArgs,
) -> Result<String, CodexLocalToolError> {
    if let Some(project_dir) = args.project_dir.as_deref() {
        let project_dir = project_dir.trim();
        if project_dir.is_empty() {
            return Err(CodexLocalToolError::InvalidArguments(
                "projectDir must not be blank".to_string(),
            ));
        }
        return Ok(project_dir.to_string());
    }
    Ok(default_agent_project_dir(project))
}

fn resolve_import_folder_id(
    project: &VideoProject,
    folder_id: Option<&str>,
) -> Result<Option<String>, CodexLocalToolError> {
    let Some(folder_id) = folder_id else {
        return Ok(None);
    };
    let folder_id = trim_required(folder_id, "folderId")?;
    if !project
        .media_folders
        .iter()
        .any(|folder| folder.id == folder_id)
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "folderId references missing media folder: {folder_id}"
        )));
    }
    Ok(Some(folder_id))
}

fn apply_import_folder_id(
    project_dir: &Path,
    project: &mut VideoProject,
    imported: &mut [MediaAsset],
    folder_id: String,
) -> Result<(), CodexLocalToolError> {
    let imported_ids = imported
        .iter()
        .map(|media| media.id.as_str())
        .collect::<BTreeSet<_>>();
    for media in &mut project.media {
        if imported_ids.contains(media.id.as_str()) {
            media.folder_id = Some(folder_id.clone());
        }
    }
    for media in imported {
        media.folder_id = Some(folder_id.clone());
    }
    persist_imported_project(project_dir, project)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    Ok(())
}

fn apply_import_mirrored_folders(
    project_dir: &Path,
    project: &mut VideoProject,
    imported: &mut [MediaAsset],
    sources: &ImportMediaSources,
) -> Result<(), CodexLocalToolError> {
    let mut existing_folder_ids = project
        .media_folders
        .iter()
        .map(|folder| folder.id.clone())
        .collect::<BTreeSet<_>>();
    for folder in &sources.mirrored_folders {
        if existing_folder_ids.insert(folder.id.clone()) {
            project.media_folders.push(folder.clone());
        }
    }

    let mut imported_index = 0;
    for source_path in &sources.paths {
        if !source_path.is_file() || importable_media_kind_for_codex(source_path).is_none() {
            continue;
        }
        let Some(imported_media) = imported.get_mut(imported_index) else {
            break;
        };
        imported_index += 1;
        let Some(folder_id) = sources.folder_by_source_path.get(source_path).cloned() else {
            continue;
        };
        imported_media.folder_id = Some(folder_id.clone());
        if let Some(project_media) = project
            .media
            .iter_mut()
            .find(|media| media.id == imported_media.id)
        {
            project_media.folder_id = Some(folder_id);
        }
    }

    persist_imported_project(project_dir, project)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    Ok(())
}

#[derive(Debug, Default)]
struct ImportMediaSources {
    paths: Vec<PathBuf>,
    mirrored_folders: Vec<MediaFolder>,
    folder_by_source_path: BTreeMap<PathBuf, String>,
}

fn import_media_sources(
    args: &ImportMediaArgs,
    project_dir: &Path,
) -> Result<ImportMediaSources, CodexLocalToolError> {
    if !args.source_paths.is_empty() && args.source.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "pass either sourcePaths or source, not both".to_string(),
        ));
    }
    if !args.source_paths.is_empty() {
        return import_media_source_path_list(&args.source_paths);
    }
    if args
        .name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "name must not be blank".to_string(),
        ));
    }
    let Some(source) = args.source.as_ref() else {
        return Ok(ImportMediaSources::default());
    };
    let selected_source_count = [
        source.url.as_ref(),
        source.path.as_ref(),
        source.bytes.as_ref(),
    ]
    .iter()
    .filter(|value| value.is_some())
    .count();
    if selected_source_count != 1 {
        return Err(CodexLocalToolError::InvalidArguments(
            "source must set exactly one of url, path, or bytes".to_string(),
        ));
    }
    if let Some(url) = source.url.as_deref() {
        return Ok(ImportMediaSources {
            paths: vec![PathBuf::from(download_imported_source_url(
                project_dir,
                source,
                url,
                args.name.as_deref(),
            )?)],
            ..ImportMediaSources::default()
        });
    }
    if let Some(bytes) = source.bytes.as_deref() {
        return Ok(ImportMediaSources {
            paths: vec![PathBuf::from(write_imported_source_bytes(
                project_dir,
                source,
                bytes,
                args.name.as_deref(),
            )?)],
            ..ImportMediaSources::default()
        });
    }
    if let Some(mime_type) = source.mime_type.as_deref() {
        trim_required(mime_type, "source.mimeType")?;
    }
    let path = trim_required(source.path.as_deref().unwrap_or(""), "source.path")?;
    let path = PathBuf::from(path);
    if path.is_dir() {
        collect_directory_import_sources(&path)
    } else {
        Ok(ImportMediaSources {
            paths: vec![path],
            ..ImportMediaSources::default()
        })
    }
}

fn import_media_source_path_list(
    source_paths: &[String],
) -> Result<ImportMediaSources, CodexLocalToolError> {
    let mut paths = Vec::new();
    for (index, source_path) in source_paths.iter().enumerate() {
        let path = trim_required(source_path, &format!("sourcePaths[{index}]"))?;
        paths.push(PathBuf::from(path));
    }
    Ok(ImportMediaSources {
        paths,
        ..ImportMediaSources::default()
    })
}

fn collect_directory_import_sources(
    root: &Path,
) -> Result<ImportMediaSources, CodexLocalToolError> {
    let root_name = directory_display_name(root);
    let root_id = import_folder_id_for_path(root, None);
    let root_folder = MediaFolder {
        id: root_id.clone(),
        name: root_name,
        parent_id: None,
    };
    let mut sources = ImportMediaSources {
        mirrored_folders: vec![root_folder],
        ..ImportMediaSources::default()
    };
    collect_directory_import_sources_inner(root, root, &root_id, &mut sources)?;
    Ok(sources)
}

fn collect_directory_import_sources_inner(
    root: &Path,
    directory: &Path,
    folder_id: &str,
    sources: &mut ImportMediaSources,
) -> Result<(), CodexLocalToolError> {
    for entry_path in read_sorted_directory_entries(directory)? {
        if entry_path.is_dir() {
            let child_folder_id = import_folder_id_for_path(&entry_path, Some(root));
            sources.mirrored_folders.push(MediaFolder {
                id: child_folder_id.clone(),
                name: directory_display_name(&entry_path),
                parent_id: Some(folder_id.to_string()),
            });
            collect_directory_import_sources_inner(root, &entry_path, &child_folder_id, sources)?;
        } else {
            sources
                .folder_by_source_path
                .insert(entry_path.clone(), folder_id.to_string());
            sources.paths.push(entry_path);
        }
    }
    Ok(())
}

fn read_sorted_directory_entries(directory: &Path) -> Result<Vec<PathBuf>, CodexLocalToolError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            CodexLocalToolError::ProjectActionApplication(format!(
                "failed to read import directory {}: {error}",
                directory.display()
            ))
        })?
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                CodexLocalToolError::ProjectActionApplication(format!(
                    "failed to read import directory entry in {}: {error}",
                    directory.display()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    Ok(entries)
}

fn directory_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Imported media")
        .to_string()
}

fn import_folder_id_for_path(path: &Path, root: Option<&Path>) -> String {
    let seed = root
        .and_then(|root| path.strip_prefix(root).ok())
        .filter(|relative| relative.components().next().is_some())
        .unwrap_or(path);
    let stem = sanitize_import_file_stem(&seed.display().to_string())
        .unwrap_or_else(|| "folder".to_string());
    format!("import-{}-{stem}", uuid::Uuid::new_v4().simple())
}

fn importable_media_kind_for_codex(path: &Path) -> Option<MediaKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "mov" | "m4v" | "webm" => Some(MediaKind::Video),
        "wav" | "mp3" | "m4a" | "aac" | "aiff" | "aifc" | "flac" => Some(MediaKind::Audio),
        "png" | "jpg" | "jpeg" | "webp" | "tiff" | "tif" | "heic" | "heif" => {
            Some(MediaKind::Image)
        }
        "lottie" | "json" if path_is_lottie_media(path) => Some(MediaKind::Lottie),
        _ => None,
    }
}

fn download_imported_source_url(
    project_dir: &Path,
    source: &PalmierImportSource,
    url: &str,
    name: Option<&str>,
) -> Result<String, CodexLocalToolError> {
    let url = trim_required(url, "source.url")?;
    let parsed = reqwest::Url::parse(&url).map_err(|_| {
        CodexLocalToolError::InvalidArguments("source.url is not a valid URL".to_string())
    })?;
    validate_import_url(&parsed)?;
    let extension = import_url_extension(source, &parsed)?;
    let stem = name
        .and_then(|name| sanitize_import_file_stem(name).filter(|stem| !stem.is_empty()))
        .or_else(|| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .and_then(|segment| Path::new(segment).file_stem())
                .and_then(|stem| stem.to_str())
                .and_then(sanitize_import_file_stem)
        })
        .unwrap_or_else(|| "imported-asset".to_string());

    let import_dir = project_dir
        .join(".agent-imports")
        .join(uuid::Uuid::new_v4().simple().to_string());
    fs::create_dir_all(&import_dir).map_err(|error| {
        CodexLocalToolError::ProjectActionApplication(format!(
            "failed to create import staging directory: {error}"
        ))
    })?;
    let staging_path = import_dir.join(format!("{stem}.{extension}"));

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|error| {
            CodexLocalToolError::ProjectActionApplication(format!(
                "failed to create URL import client: {error}"
            ))
        })?;
    let mut response = client.get(parsed).send().map_err(|error| {
        CodexLocalToolError::ProjectActionApplication(format!(
            "failed to download source.url: {error}"
        ))
    })?;
    if !response.status().is_success() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "source.url returned HTTP {}",
            response.status()
        )));
    }
    if response
        .content_length()
        .is_some_and(|length| length > IMPORT_URL_MAX_BYTES)
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "source.url is too large; max {IMPORT_URL_MAX_BYTES} bytes"
        )));
    }

    let mut limited = response.by_ref().take(IMPORT_URL_MAX_BYTES + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes).map_err(|error| {
        CodexLocalToolError::ProjectActionApplication(format!(
            "failed to read source.url response: {error}"
        ))
    })?;
    if bytes.len() as u64 > IMPORT_URL_MAX_BYTES {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "source.url is too large; max {IMPORT_URL_MAX_BYTES} bytes"
        )));
    }
    if bytes.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.url response was empty".to_string(),
        ));
    }
    fs::write(&staging_path, bytes).map_err(|error| {
        CodexLocalToolError::ProjectActionApplication(format!(
            "failed to write source.url import: {error}"
        ))
    })?;
    Ok(staging_path.display().to_string())
}

fn validate_import_url(url: &reqwest::Url) -> Result<(), CodexLocalToolError> {
    let scheme = url.scheme();
    if scheme != "https" && !(scheme == "http" && import_url_is_loopback(url)) {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.url must use https".to_string(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.url must not embed credentials".to_string(),
        ));
    }
    if url.host_str().is_none_or(|host| host.trim().is_empty()) {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.url has no host".to_string(),
        ));
    }
    Ok(())
}

fn import_url_is_loopback(url: &reqwest::Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

fn import_url_extension(
    source: &PalmierImportSource,
    url: &reqwest::Url,
) -> Result<&'static str, CodexLocalToolError> {
    if let Some(mime_type) = source.mime_type.as_deref() {
        let mime_type = trim_required(mime_type, "source.mimeType")?;
        return import_extension_for_mime_type(&mime_type).ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "Unsupported source.mimeType '{mime_type}'. Accepted: video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, audio/aiff, audio/flac, image/png, image/jpeg, image/tiff, image/heic, application/json, application/vnd.lottie+json."
            ))
        });
    }
    let extension = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|segment| Path::new(segment).extension())
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|extension| !extension.is_empty())
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "Cannot infer media type from source.url extension; set source.mimeType"
                    .to_string(),
            )
        })?;
    match extension.as_str() {
        "mp4" => Ok("mp4"),
        "mov" => Ok("mov"),
        "m4v" => Ok("m4v"),
        "webm" => Ok("webm"),
        "wav" => Ok("wav"),
        "mp3" => Ok("mp3"),
        "m4a" => Ok("m4a"),
        "aac" => Ok("aac"),
        "flac" => Ok("flac"),
        "png" => Ok("png"),
        "jpg" => Ok("jpg"),
        "jpeg" => Ok("jpeg"),
        "webp" => Ok("webp"),
        "lottie" => Ok("lottie"),
        "json" => Ok("json"),
        _ => Err(CodexLocalToolError::InvalidArguments(format!(
            "Unsupported source.url extension '.{extension}'"
        ))),
    }
}

fn write_imported_source_bytes(
    project_dir: &Path,
    source: &PalmierImportSource,
    bytes: &str,
    name: Option<&str>,
) -> Result<String, CodexLocalToolError> {
    if bytes.len() > IMPORT_BYTES_MAX_BASE64_LENGTH {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "source.bytes is too large ({} chars; max {IMPORT_BYTES_MAX_BASE64_LENGTH})",
            bytes.len()
        )));
    }

    let mime_type = trim_required(
        source.mime_type.as_deref().unwrap_or(""),
        "source.mimeType is required when source.bytes is set",
    )
    .map_err(|_| {
        CodexLocalToolError::InvalidArguments(
            "source.mimeType is required when source.bytes is set".to_string(),
        )
    })?;
    let extension = import_extension_for_mime_type(&mime_type).ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!(
            "Unsupported source.mimeType '{mime_type}'. Accepted: video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, audio/aiff, audio/flac, image/png, image/jpeg, image/tiff, image/heic, application/json, application/vnd.lottie+json."
        ))
    })?;
    let decoded = decode_base64_import_bytes(bytes)?;
    let import_dir = project_dir
        .join(".agent-imports")
        .join(uuid::Uuid::new_v4().simple().to_string());
    fs::create_dir_all(&import_dir).map_err(|error| {
        CodexLocalToolError::ProjectActionApplication(format!(
            "failed to create import staging directory: {error}"
        ))
    })?;
    let stem = name
        .and_then(|name| sanitize_import_file_stem(name).filter(|stem| !stem.is_empty()))
        .unwrap_or_else(|| "imported-asset".to_string());
    let staging_path = import_dir.join(format!("{stem}.{extension}"));
    fs::write(&staging_path, decoded).map_err(|error| {
        CodexLocalToolError::ProjectActionApplication(format!(
            "failed to write source.bytes import: {error}"
        ))
    })?;
    Ok(staging_path.display().to_string())
}

fn import_extension_for_mime_type(mime_type: &str) -> Option<&'static str> {
    match mime_type.to_ascii_lowercase().as_str() {
        "video/mp4" | "video/mpeg4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/aac" => Some("aac"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "audio/aiff" | "audio/x-aiff" => Some("aiff"),
        "audio/aifc" | "audio/x-aifc" => Some("aifc"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/tiff" => Some("tiff"),
        "image/heic" | "image/heif" => Some("heic"),
        "application/json" | "application/vnd.lottie+json" => Some("json"),
        "application/vnd.lottie" | "application/x-lottie" => Some("lottie"),
        _ => None,
    }
}

fn sanitize_import_file_stem(stem: &str) -> Option<String> {
    let sanitized = stem
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    (!sanitized.is_empty()).then_some(sanitized)
}

fn decode_base64_import_bytes(input: &str) -> Result<Vec<u8>, CodexLocalToolError> {
    let mut cleaned = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if cleaned.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.bytes is not valid non-empty base64".to_string(),
        ));
    }
    if cleaned.len() % 4 != 0 {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.bytes is not valid non-empty base64".to_string(),
        ));
    }

    let mut output = Vec::with_capacity(cleaned.len() / 4 * 3);
    let chunk_count = cleaned.len() / 4;
    for (chunk_index, chunk) in cleaned.chunks_mut(4).enumerate() {
        let values = [
            decode_base64_value(chunk[0]),
            decode_base64_value(chunk[1]),
            decode_base64_value(chunk[2]),
            decode_base64_value(chunk[3]),
        ];
        if values[0].is_none()
            || values[1].is_none()
            || (chunk[2] != b'=' && values[2].is_none())
            || (chunk[3] != b'=' && values[3].is_none())
        {
            return Err(CodexLocalToolError::InvalidArguments(
                "source.bytes is not valid non-empty base64".to_string(),
            ));
        }
        if chunk_index + 1 != chunk_count && (chunk[2] == b'=' || chunk[3] == b'=') {
            return Err(CodexLocalToolError::InvalidArguments(
                "source.bytes is not valid non-empty base64".to_string(),
            ));
        }
        if chunk[2] == b'=' && chunk[3] != b'=' {
            return Err(CodexLocalToolError::InvalidArguments(
                "source.bytes is not valid non-empty base64".to_string(),
            ));
        }
        if chunk[0] == b'=' || chunk[1] == b'=' {
            return Err(CodexLocalToolError::InvalidArguments(
                "source.bytes is not valid non-empty base64".to_string(),
            ));
        }

        let a = values[0].unwrap();
        let b = values[1].unwrap();
        let c = values[2].unwrap_or(0);
        let d = values[3].unwrap_or(0);

        output.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            output.push(((b & 0x0f) << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            output.push(((c & 0x03) << 6) | d);
        }
    }

    if output.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "source.bytes is not valid non-empty base64".to_string(),
        ));
    }
    Ok(output)
}

fn decode_base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        b'=' => None,
        _ => None,
    }
}

fn inspect_media_payload(
    project: &VideoProject,
    args: &InspectMediaArgs,
) -> Result<Value, CodexLocalToolError> {
    let media_id = args.media_id.trim();
    if media_id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "mediaRef must not be blank".to_string(),
        ));
    }
    if let (Some(start), Some(end)) = (args.start_seconds, args.end_seconds) {
        if end <= start {
            return Err(CodexLocalToolError::InvalidArguments(
                "transcript end must be greater than start".to_string(),
            ));
        }
    }
    if let (Some(start), Some(end)) = (args.start_frame, args.end_frame) {
        if end <= start {
            return Err(CodexLocalToolError::InvalidArguments(
                "endFrame must be greater than startFrame".to_string(),
            ));
        }
    }
    if matches!(args.start_seconds, Some(value) if value < 0.0)
        || matches!(args.end_seconds, Some(value) if value <= 0.0)
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "inspect_media range must use non-negative seconds".to_string(),
        ));
    }
    if matches!(args.max_frames, Some(0)) {
        return Err(CodexLocalToolError::InvalidArguments(
            "maxFrames must be between 1 and 12".to_string(),
        ));
    }
    if args
        .language
        .as_deref()
        .is_some_and(|language| language.trim().is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "language must not be blank".to_string(),
        ));
    }

    let fps = project_fps(project)?;
    let media = project
        .media
        .iter()
        .find(|media| media.id == media_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!("media was not found: {media_id}"))
        })?;
    let clip_context = args
        .clip_id
        .as_deref()
        .map(|clip_id| {
            let item_id = resolve_timeline_item_id(project, clip_id, "clipId")?;
            let (track, item) = timeline_item_with_track(project, &item_id)?;
            match &item.source {
                TimelineSource::Media {
                    media_id: item_media_id,
                } if item_media_id == &media.id => Ok((track, item)),
                TimelineSource::Media { .. } => Err(CodexLocalToolError::InvalidArguments(
                    format!("clipId {item_id} does not reference mediaRef {}", media.id),
                )),
                _ => Err(CodexLocalToolError::InvalidArguments(format!(
                    "clipId {item_id} is not a media clip"
                ))),
            }
        })
        .transpose()?;
    let transcript = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == media.id);
    let timeline_items = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| {
            track.items.iter().filter_map(|item| match &item.source {
                TimelineSource::Media { media_id } if media_id == &media.id => Some(json!({
                    "trackId": track.id,
                    "itemId": item.id,
                    "kind": item.kind,
                    "label": item.label,
                    "startSeconds": item.start_seconds,
                    "durationSeconds": item.duration_seconds,
                    "properties": item.properties
                })),
                _ => None,
            })
        })
        .take(25)
        .collect::<Vec<_>>();
    let word_limit = if args.word_timestamps.unwrap_or(false) {
        MAX_TRANSCRIPT_WORD_LIMIT.min(100)
    } else {
        0
    };
    let words = match (transcript, clip_context.as_ref()) {
        (Some(transcript), Some((track, item))) => inspect_clip_transcript_words(
            transcript,
            track.id.as_str(),
            item,
            args,
            fps,
            word_limit,
        ),
        (Some(transcript), None) => bounded_transcript_words(
            transcript,
            args.start_seconds
                .or_else(|| args.start_frame.map(|frame| frames_to_seconds(frame, fps))),
            args.end_seconds
                .or_else(|| args.end_frame.map(|frame| frames_to_seconds(frame, fps))),
            0,
            word_limit,
        ),
        (None, _) => Vec::new(),
    };
    let clip_payload = clip_context.as_ref().map(|(track, item)| {
        let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
        let source_out = timeline_item_number_property(item, "sourceOut")
            .unwrap_or(source_in + item.duration_seconds * timeline_item_playback_speed(item));
        json!({
            "clipId": item.id,
            "itemId": item.id,
            "trackId": track.id,
            "mediaRef": media.id,
            "startSeconds": item.start_seconds,
            "durationSeconds": item.duration_seconds,
            "startFrame": seconds_to_frames(item.start_seconds, fps),
            "endFrame": seconds_to_frames(item.start_seconds + item.duration_seconds, fps),
            "sourceIn": source_in,
            "sourceOut": source_out
        })
    });

    let visual_payload = inspect_media_visual_payload(media, args);

    let mut payload = json!({
        "media": media_payload(media),
        "timelineItems": timeline_items,
        "transcript": {
            "status": if transcript.is_some() { "ready" } else { "missing" },
            "wordCount": transcript.map(|transcript| transcript.words.len()).unwrap_or(0),
            "segmentCount": transcript.map(|transcript| transcript.segments.len()).unwrap_or(0),
            "timebase": if clip_context.is_some() { "projectFrames" } else { "sourceSeconds" },
            "language": args.language.as_deref().map(str::trim),
            "words": words
        },
        "visual": visual_payload
    });
    if let Some(clip_payload) = clip_payload {
        if let Some(object) = payload.as_object_mut() {
            object.insert("clip".to_string(), clip_payload);
        }
    }
    if args.overview.unwrap_or(false) {
        if let Some(object) = payload.as_object_mut() {
            object.insert("overview".to_string(), json!({"status": "notInstalled"}));
        }
    }
    Ok(payload)
}

fn inspect_media_visual_payload(media: &MediaAsset, args: &InspectMediaArgs) -> Value {
    if media.kind != MediaKind::Lottie {
        return json!({
            "status": "notInstalled",
            "frames": [],
            "message": "Visual frame extraction and embedding search are not installed in this build."
        });
    }

    let requested_count = args.max_frames.unwrap_or(6).clamp(1, 12);
    let duration_seconds = if media.duration_seconds.is_finite() && media.duration_seconds > 0.0 {
        media.duration_seconds
    } else {
        0.0
    };
    let framerate = media
        .fps
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(0.0);
    let frame_count = if duration_seconds > 0.0 && framerate > 0.0 {
        (duration_seconds * framerate).round().max(1.0) as u64
    } else {
        0
    };
    let sampled_indices = sampled_lottie_frame_indices(frame_count, requested_count);
    let frames = sampled_indices
        .iter()
        .map(|frame_index| {
            let timestamp_seconds = if framerate > 0.0 {
                (*frame_index as f64 / framerate).min(duration_seconds)
            } else {
                0.0
            };
            json!({
                "frameIndex": frame_index,
                "timestampSeconds": timestamp_seconds,
                "status": "metadataOnly"
            })
        })
        .collect::<Vec<_>>();

    json!({
        "status": "metadataOnly",
        "kind": "lottie",
        "frames": frames,
        "framerate": if framerate > 0.0 { Some(framerate) } else { None },
        "frameCount": if frame_count > 0 { Some(frame_count) } else { None },
        "durationSeconds": duration_seconds,
        "sampledFrameIndices": sampled_indices,
        "message": "Native Lottie frame rasterization is not installed in this build; sampled frames describe the animation timeline from imported metadata."
    })
}

fn sampled_lottie_frame_indices(frame_count: u64, requested_count: usize) -> Vec<u64> {
    if frame_count == 0 || requested_count == 0 {
        return Vec::new();
    }
    if frame_count == 1 || requested_count == 1 {
        return vec![0];
    }

    let count = requested_count.min(frame_count as usize);
    let max_index = frame_count - 1;
    (0..count)
        .map(|index| {
            ((index as f64 * max_index as f64) / (count.saturating_sub(1) as f64)).round() as u64
        })
        .collect()
}

fn search_media_payload(
    project: &VideoProject,
    args: &SearchMediaArgs,
) -> Result<Value, CodexLocalToolError> {
    let scope = SearchScope::parse(args.scope.as_deref().unwrap_or("both"))
        .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
    if let Some(media_id) = args.media_id.as_deref().map(str::trim) {
        if media_id.is_empty() {
            return Err(CodexLocalToolError::InvalidArguments(
                "mediaRef must not be blank".to_string(),
            ));
        }
        if !project.media.iter().any(|media| media.id == media_id) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "mediaRef was not found: {media_id}"
            )));
        }
    }
    let query = ProjectSearchQuery {
        query: args.query.clone(),
        limit: args.limit.unwrap_or(20),
        scope,
        media_id: args.media_id.as_deref().map(str::trim).map(str::to_string),
    };
    if let Some(project_dir) = args
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
    {
        return query_project_search_for_project_dir(Path::new(project_dir), project, query)
            .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()));
    }
    query_project_search(project, query)
        .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))
}

fn build_multi_source_edit_payload(
    project: &VideoProject,
    args: BuildMultiSourceEditArgs,
) -> Result<Value, CodexLocalToolError> {
    let mut proposed_project = project.clone();
    let draft = generate_spoken_semantic_multi_source_edit_timeline(
        &mut proposed_project,
        args.request,
        &args.semantic_hits,
    )
    .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
    Ok(json!({
        "status": "proposed",
        "workflow": "deterministic-spoken-semantic-multi-source-edl",
        "draft": draft.clone(),
        "project": proposed_project,
        "sourceClips": draft.edl.clips.iter().map(|clip| json!({
            "mediaId": clip.media_id,
            "sourceIn": clip.source_in,
            "sourceOut": clip.source_out,
            "reason": clip.reason,
            "selectionReasons": clip.selection_reasons
        })).collect::<Vec<_>>()
    }))
}

fn generated_assets_payload(project: &VideoProject) -> Value {
    json!({
        "assets": project.generated_assets,
        "truncated": false
    })
}

fn workflow_jobs_payload(project: &VideoProject) -> Value {
    json!({
        "jobs": project.jobs,
        "truncated": false
    })
}

fn render_reports_payload(project: &VideoProject) -> Value {
    json!({
        "renderReports": project.render_reports,
        "truncated": false
    })
}

fn export_artifacts_payload(project: &VideoProject) -> Value {
    json!({
        "exportArtifacts": project.export_artifacts,
        "truncated": false
    })
}

fn export_profiles_payload() -> Value {
    json!({
        "profiles": mp4_export_profile_availability_report()
    })
}

fn list_effects_payload() -> Value {
    effect_catalog_payload()
}

pub fn list_models_payload(model_type: Option<&str>) -> Result<Value, CodexLocalToolError> {
    let normalized_filter = model_type.map(normalized_export_option);
    let generation_models = attach_generation_pricing(attach_generation_display_names(vec![
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
            "kind": "video",
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["480p", "720p", "1080p"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "maxReferenceAudios": 1,
            "referenceTagNoun": "audio"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["480p", "720p", "1080p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 2,
            "maxReferenceVideos": 1,
            "maxReferenceAudios": 1,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [2, 3, 4, 5, 6, 7, 8, 9, 10],
            "aspectRatios": ["16:9", "9:16", "1:1", "4:3", "3:4"],
            "resolutions": ["720p", "1080p"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresReferenceImage": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["auto", "16:9", "9:16", "1:1"],
            "resolutions": ["480p", "580p", "720p"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresSourceVideo": true,
            "maxSourceVideoSeconds": 10.0,
            "maxReferenceImages": 1,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
            "kind": "video",
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": false
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 2,
            "referenceTagNoun": "frame"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresSourceVideo": true,
            "requiresReferenceImage": true,
            "maxSourceVideoSeconds": 10.0,
            "maxReferenceImages": 1,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_FLUX_SCHNELL_MODEL_ID,
            "kind": "image",
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "maxImages": 4
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KREA_2_TURBO_MODEL_ID,
            "kind": "image",
            "supportsImageReference": false,
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "resolutions": ["1024x1024", "1024x576", "576x1024", "1024x768", "768x1024"],
            "maxImages": 1
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
            "kind": "image",
            "supportsImageReference": false,
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "resolutions": ["1024x1024", "1280x720", "720x1280", "1280x960", "960x1280"],
            "qualities": ["realistic_image", "digital_illustration", "vector_illustration"],
            "maxImages": 1
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
            "aliases": ["nano-banana-pro"],
            "kind": "image",
            "requiresProviderInputUrl": true,
            "aspectRatios": ["auto", "21:9", "16:9", "3:2", "4:3", "5:4", "1:1", "4:5", "3:4", "2:3", "9:16"],
            "resolutions": ["1K", "2K", "4K"],
            "supportsImageReference": true,
            "maxImages": 1
        }),
        json!({
            "provider": OPENAI_PROVIDER,
            "id": OPENAI_GPT_IMAGE_2_MODEL_ID,
            "kind": "image",
            "supportsImageReference": false,
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["1024x1024", "1536x1024", "1024x1536"],
            "supportsFlexibleResolution": true,
            "resolutionFormat": "WIDTHxHEIGHT",
            "maxResolution": "3840x2160",
            "maxImages": 4
        }),
        json!({
            "provider": OPENAI_PROVIDER,
            "id": OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
            "kind": "image",
            "supportsImageReference": true,
            "requiresProviderInputUrl": true,
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["auto", "1024x1024", "1536x1024", "1024x1536"],
            "maxReferenceImages": 16,
            "maxImages": 1
        }),
        json!({
            "provider": XAI_PROVIDER,
            "id": XAI_GROK_IMAGE_QUALITY_MODEL_ID,
            "kind": "image",
            "supportsImageReference": true,
            "requiresProviderInputUrl": true,
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "maxImages": 1
        }),
        json!({
            "provider": XAI_PROVIDER,
            "id": XAI_GROK_VIDEO_MODEL_ID,
            "kind": "video",
            "durations": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            "aspectRatios": ["16:9", "9:16", "1:1", "4:3", "3:4", "3:2", "2:3"],
            "resolutions": ["480p", "720p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresSourceVideo": false,
            "supportsSourceVideo": true,
            "maxSourceVideoSeconds": 8.7,
            "maxReferenceImages": 7,
            "maxTotalReferences": 7,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_VEO_31_FAST_MODEL_ID,
            "kind": "video",
            "durations": [8],
            "aspectRatios": ["16:9", "9:16"],
            "resolutions": ["720p", "1080p", "4k"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "requiresSourceVideo": false,
            "maxReferenceImages": 3,
            "maxTotalReferences": 3,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SEED_AUDIO_MODEL_ID,
            "kind": "audio",
            "category": "tts",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": false
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
            "aliases": ["sonilo-v1.1-video-to-music"],
            "kind": "audio",
            "requiresProviderInputUrl": true,
            "category": "music",
            "inputs": ["video"],
            "minPromptLength": 0,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "minSeconds": 1,
            "maxSeconds": 900
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID,
            "aliases": ["sonilo-v1.1-text-to-music"],
            "kind": "audio",
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true,
            "minSeconds": 1,
            "maxSeconds": 600
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
            "aliases": ["mirelo-sfx-v1.5-video-to-audio"],
            "kind": "audio",
            "requiresProviderInputUrl": true,
            "category": "sfx",
            "inputs": ["video"],
            "minPromptLength": 0,
            "promptLabel": AUDIO_SFX_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "minSeconds": 1,
            "maxSeconds": 900
        }),
        json!({
            "provider": OPENAI_PROVIDER,
            "id": OPENAI_GPT_4O_MINI_TTS_MODEL_ID,
            "kind": "audio",
            "category": "tts",
            "inputs": ["text"],
            "defaultVoice": "alloy",
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "voicesSample": ["alloy", "ash", "ballad"],
            "voiceCount": 13
        }),
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_GEMINI_TTS_MODEL_ID,
            "aliases": ["gemini-3.1-flash-tts"],
            "kind": "audio",
            "category": "tts",
            "inputs": ["text"],
            "defaultVoice": GOOGLE_GEMINI_TTS_DEFAULT_VOICE,
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "voicesSample": ["Kore", "Puck", "Charon"],
            "voiceCount": GOOGLE_GEMINI_TTS_VOICES.len()
        }),
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_LYRIA_3_PRO_MODEL_ID,
            "kind": "audio",
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": true,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true
        }),
        json!({
            "provider": MINIMAX_PROVIDER,
            "id": MINIMAX_MUSIC_MODEL_ID,
            "kind": "audio",
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 10,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": true,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true
        }),
        json!({
            "provider": ELEVENLABS_PROVIDER,
            "id": ELEVENLABS_TTS_V3_MODEL_ID,
            "kind": "audio",
            "category": "tts",
            "inputs": ["text"],
            "defaultVoice": ELEVENLABS_DEFAULT_VOICE,
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "voicesSample": [ELEVENLABS_DEFAULT_VOICE],
            "voiceCount": 1
        }),
        json!({
            "provider": ELEVENLABS_PROVIDER,
            "id": ELEVENLABS_MUSIC_MODEL_ID,
            "kind": "audio",
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": true,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true,
            "minSeconds": 3,
            "maxSeconds": 600
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_FLUX_SCHNELL_MODEL_ID,
            "kind": "image",
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "supportsImageReference": false,
            "maxImages": 4
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_FLUX_DEV_MODEL_ID,
            "kind": "image",
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "supportsImageReference": false,
            "maxImages": 4
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_FLUX_11_PRO_MODEL_ID,
            "kind": "image",
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "supportsImageReference": false,
            "maxImages": 4
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
            "kind": "image",
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "supportsImageReference": false,
            "maxImages": 4
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_SEEDANCE_20_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["720p", "1080p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "maxReferenceAudios": 3,
            "maxTotalReferences": 6,
            "maxCombinedVideoRefSeconds": 15,
            "maxCombinedAudioRefSeconds": 15,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_SEEDANCE_20_FAST_MODEL_ID,
            "kind": "video",
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["720p", "1080p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "maxReferenceAudios": 3,
            "maxTotalReferences": 6,
            "maxCombinedVideoRefSeconds": 15,
            "maxCombinedAudioRefSeconds": 15,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": "mock",
            "id": "mock-upscale-v1",
            "kind": "upscale",
            "supportedTypes": ["image", "video"],
            "speed": "Fast",
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_AURA_SR_MODEL_ID,
            "aliases": ["seedvr-image-upscaler"],
            "kind": "upscale",
            "supportedTypes": ["image"],
            "speed": "Medium",
            "requiresProviderInputUrl": true
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_VIDEO_UPSCALER_MODEL_ID,
            "aliases": ["bytedance-upscaler"],
            "kind": "upscale",
            "supportedTypes": ["video"],
            "speed": "Slow",
            "requiresProviderInputUrl": true
        }),
    ]));
    let generation_models = attach_generation_endpoint_metadata(generation_models);
    let generation_models = attach_generation_ui_capabilities(generation_models);
    let resolved_generation_catalog = resolve_generation_catalog(generation_models);
    let generation_catalog_provenance = resolved_generation_catalog.provenance;
    let generation_models = resolved_generation_catalog.models;
    let flat_generation_models = attach_generation_pricing(attach_generation_display_names(vec![
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video"],
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["480p", "720p", "1080p"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "maxReferenceAudios": 1,
            "referenceTagNoun": "audio"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image", "audio"],
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["480p", "720p", "1080p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 2,
            "maxReferenceVideos": 1,
            "maxReferenceAudios": 1,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image"],
            "requiresProviderInputUrl": true,
            "durations": [2, 3, 4, 5, 6, 7, 8, 9, 10],
            "aspectRatios": ["16:9", "9:16", "1:1", "4:3", "3:4"],
            "resolutions": ["720p", "1080p"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresReferenceImage": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video"],
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["auto", "16:9", "9:16", "1:1"],
            "resolutions": ["480p", "580p", "720p"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresSourceVideo": true,
            "maxSourceVideoSeconds": 10.0,
            "maxReferenceImages": 1,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video"],
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": false
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image"],
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 2,
            "referenceTagNoun": "frame"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image"],
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "supportsFirstFrame": false,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresSourceVideo": true,
            "requiresReferenceImage": true,
            "maxSourceVideoSeconds": 10.0,
            "maxReferenceImages": 1,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_FLUX_SCHNELL_MODEL_ID,
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "maxImages": 4
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_KREA_2_TURBO_MODEL_ID,
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "supportsImageReference": false,
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "resolutions": ["1024x1024", "1024x576", "576x1024", "1024x768", "768x1024"],
            "maxImages": 1
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "supportsImageReference": false,
            "aspectRatios": ["1:1", "16:9", "9:16", "4:3", "3:4"],
            "resolutions": ["1024x1024", "1280x720", "720x1280", "1280x960", "960x1280"],
            "qualities": ["realistic_image", "digital_illustration", "vector_illustration"],
            "maxImages": 1
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
            "aliases": ["nano-banana-pro"],
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "requiresProviderInputUrl": true,
            "aspectRatios": ["auto", "21:9", "16:9", "3:2", "4:3", "5:4", "1:1", "4:5", "3:4", "2:3", "9:16"],
            "resolutions": ["1K", "2K", "4K"],
            "supportsImageReference": true,
            "maxImages": 1
        }),
        json!({
            "provider": OPENAI_PROVIDER,
            "id": OPENAI_GPT_IMAGE_2_MODEL_ID,
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "supportsImageReference": false,
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["1024x1024", "1536x1024", "1024x1536"],
            "supportsFlexibleResolution": true,
            "resolutionFormat": "WIDTHxHEIGHT",
            "maxResolution": "3840x2160",
            "maxImages": 4
        }),
        json!({
            "provider": OPENAI_PROVIDER,
            "id": OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "supportsImageReference": true,
            "requiresProviderInputUrl": true,
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["auto", "1024x1024", "1536x1024", "1024x1536"],
            "maxReferenceImages": 16,
            "maxImages": 1
        }),
        json!({
            "provider": XAI_PROVIDER,
            "id": XAI_GROK_IMAGE_QUALITY_MODEL_ID,
            "type": "image",
            "kind": "image",
            "supports": ["image"],
            "supportsImageReference": true,
            "requiresProviderInputUrl": true,
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "maxImages": 1
        }),
        json!({
            "provider": XAI_PROVIDER,
            "id": XAI_GROK_VIDEO_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image"],
            "durations": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            "aspectRatios": ["16:9", "9:16", "1:1", "4:3", "3:4", "3:2", "2:3"],
            "resolutions": ["480p", "720p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": false,
            "supportsReferences": true,
            "requiresSourceVideo": false,
            "supportsSourceVideo": true,
            "maxSourceVideoSeconds": 8.7,
            "maxReferenceImages": 7,
            "maxTotalReferences": 7,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_VEO_31_FAST_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image"],
            "durations": [8],
            "aspectRatios": ["16:9", "9:16"],
            "resolutions": ["720p", "1080p", "4k"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "requiresSourceVideo": false,
            "maxReferenceImages": 3,
            "maxTotalReferences": 3,
            "referenceTagNoun": "image"
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SEED_AUDIO_MODEL_ID,
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "tts",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": false
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
            "aliases": ["sonilo-v1.1-video-to-music"],
            "type": "audio",
            "kind": "audio",
            "supports": ["audio", "video"],
            "requiresProviderInputUrl": true,
            "category": "music",
            "inputs": ["video"],
            "minPromptLength": 0,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "minSeconds": 1,
            "maxSeconds": 900
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID,
            "aliases": ["sonilo-v1.1-text-to-music"],
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true,
            "minSeconds": 1,
            "maxSeconds": 600
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
            "aliases": ["mirelo-sfx-v1.5-video-to-audio"],
            "type": "audio",
            "kind": "audio",
            "supports": ["audio", "video"],
            "requiresProviderInputUrl": true,
            "category": "sfx",
            "inputs": ["video"],
            "minPromptLength": 0,
            "promptLabel": AUDIO_SFX_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "minSeconds": 1,
            "maxSeconds": 900
        }),
        json!({
            "provider": OPENAI_PROVIDER,
            "id": OPENAI_GPT_4O_MINI_TTS_MODEL_ID,
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "tts",
            "inputs": ["text"],
            "defaultVoice": "alloy",
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "voicesSample": ["alloy", "ash", "ballad"],
            "voiceCount": 13
        }),
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_GEMINI_TTS_MODEL_ID,
            "aliases": ["gemini-3.1-flash-tts"],
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "tts",
            "inputs": ["text"],
            "defaultVoice": GOOGLE_GEMINI_TTS_DEFAULT_VOICE,
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "voicesSample": ["Kore", "Puck", "Charon"],
            "voiceCount": GOOGLE_GEMINI_TTS_VOICES.len()
        }),
        json!({
            "provider": GOOGLE_PROVIDER,
            "id": GOOGLE_LYRIA_3_PRO_MODEL_ID,
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": true,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true
        }),
        json!({
            "provider": MINIMAX_PROVIDER,
            "id": MINIMAX_MUSIC_MODEL_ID,
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 10,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": true,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true
        }),
        json!({
            "provider": ELEVENLABS_PROVIDER,
            "id": ELEVENLABS_TTS_V3_MODEL_ID,
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "tts",
            "inputs": ["text"],
            "defaultVoice": ELEVENLABS_DEFAULT_VOICE,
            "minPromptLength": 1,
            "promptLabel": AUDIO_TTS_PROMPT_LABEL,
            "supportsLyrics": false,
            "supportsInstrumental": false,
            "supportsStyleInstructions": true,
            "voicesSample": [ELEVENLABS_DEFAULT_VOICE],
            "voiceCount": 1
        }),
        json!({
            "provider": ELEVENLABS_PROVIDER,
            "id": ELEVENLABS_MUSIC_MODEL_ID,
            "type": "audio",
            "kind": "audio",
            "supports": ["audio"],
            "category": "music",
            "inputs": ["text"],
            "minPromptLength": 1,
            "promptLabel": AUDIO_MUSIC_PROMPT_LABEL,
            "supportsLyrics": true,
            "supportsInstrumental": true,
            "supportsStyleInstructions": true,
            "minSeconds": 3,
            "maxSeconds": 600
        }),
        json!({
            "provider": "mock",
            "id": "mock-upscale-v1",
            "type": "upscale",
            "kind": "upscale",
            "supports": ["video", "image"],
            "supportedTypes": ["image", "video"],
            "speed": "Fast",
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_AURA_SR_MODEL_ID,
            "aliases": ["seedvr-image-upscaler"],
            "type": "upscale",
            "kind": "upscale",
            "supports": ["image"],
            "supportedTypes": ["image"],
            "speed": "Medium",
            "requiresProviderInputUrl": true
        }),
        json!({
            "provider": FAL_PROVIDER,
            "id": FAL_VIDEO_UPSCALER_MODEL_ID,
            "aliases": ["bytedance-upscaler"],
            "type": "upscale",
            "kind": "upscale",
            "supports": ["video"],
            "supportedTypes": ["video"],
            "speed": "Slow",
            "requiresProviderInputUrl": true
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_SEEDANCE_20_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image", "audio"],
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["720p", "1080p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "maxReferenceAudios": 3,
            "maxTotalReferences": 6,
            "maxCombinedVideoRefSeconds": 15,
            "maxCombinedAudioRefSeconds": 15,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "reference"
        }),
        json!({
            "provider": REPLICATE_PROVIDER,
            "id": REPLICATE_SEEDANCE_20_FAST_MODEL_ID,
            "type": "video",
            "kind": "video",
            "supports": ["video", "image", "audio"],
            "requiresProviderInputUrl": true,
            "durations": [5, 10],
            "aspectRatios": ["16:9", "9:16", "1:1"],
            "resolutions": ["720p", "1080p"],
            "supportsFirstFrame": true,
            "supportsLastFrame": true,
            "supportsReferences": true,
            "maxReferenceImages": 4,
            "maxReferenceVideos": 3,
            "maxReferenceAudios": 3,
            "maxTotalReferences": 6,
            "maxCombinedVideoRefSeconds": 15,
            "maxCombinedAudioRefSeconds": 15,
            "framesAndReferencesExclusive": true,
            "referenceTagNoun": "reference"
        }),
    ]));
    let flat_generation_models = attach_generation_endpoint_metadata(flat_generation_models);
    let flat_generation_models = attach_generation_ui_capabilities(flat_generation_models);
    let flat_generation_models =
        flat_generation_models_from_resolved_catalog(&generation_models, flat_generation_models);
    let transcription_models = transcription_model_catalog()
        .into_iter()
        .map(|entry| {
            json!({
                "id": entry.id,
                "label": entry.display_name,
                "supportedRuntimes": entry.supported_runtimes,
                "modality": entry.modality,
                "artifactFormat": entry.artifact_format,
                "revision": entry.revision,
                "requiredFiles": entry.required_files.len()
            })
        })
        .collect::<Vec<_>>();
    let flat_transcription_models = transcription_models
        .iter()
        .map(|model| {
            json!({
                "provider": "local",
                "id": model["id"],
                "label": model["label"],
                "displayName": model["label"],
                "type": "transcription",
                "kind": "transcription",
                "supports": ["audio", "video"],
            })
        })
        .collect::<Vec<_>>();
    let mut models = Vec::new();
    models.extend(flat_generation_models.iter().cloned());
    models.extend(flat_transcription_models);
    if let Some(filter) = normalized_filter.as_deref() {
        if !matches!(
            filter,
            "video" | "image" | "audio" | "upscale" | "transcription"
        ) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "list_models type must be video, image, audio, upscale, or transcription: {filter}"
            )));
        }
        models.retain(|model| model["type"].as_str() == Some(filter));
    }
    let filtered_generation_models = if normalized_filter.as_deref() == Some("transcription") {
        Vec::new()
    } else if let Some(filter) = normalized_filter.as_deref() {
        generation_models
            .iter()
            .filter(|model| model["kind"].as_str() == Some(filter))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        generation_models
    };

    Ok(json!({
        "loaded": true,
        "models": models,
        "generationModels": filtered_generation_models,
        "generationCatalog": generation_catalog_provenance,
        "transcriptionModels": transcription_models,
        "providerCredentialsExposed": false
    }))
}

fn flat_generation_models_from_resolved_catalog(
    generation_models: &[Value],
    legacy_flat_models: Vec<Value>,
) -> Vec<Value> {
    let legacy_by_key = legacy_flat_models
        .into_iter()
        .filter_map(|model| {
            let provider = model.get("provider")?.as_str()?;
            let id = model.get("id")?.as_str()?;
            Some((format!("{provider}:{id}"), model))
        })
        .collect::<BTreeMap<_, _>>();

    generation_models
        .iter()
        .cloned()
        .map(|mut model| {
            let provider = model
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let id = model
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let kind = model
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let legacy = legacy_by_key.get(&format!("{provider}:{id}"));
            let supports = legacy
                .and_then(|entry| entry.get("supports"))
                .cloned()
                .unwrap_or_else(|| match kind.as_str() {
                    "upscale" => json!(["image", "video"]),
                    "audio" => json!(["audio"]),
                    "image" => json!(["image"]),
                    _ => json!(["video"]),
                });
            if let Some(object) = model.as_object_mut() {
                object.insert("type".to_string(), Value::String(kind));
                object.insert("supports".to_string(), supports);
            }
            model
        })
        .collect()
}

pub fn resolved_generation_model_payload(
    provider: &str,
    model_id: &str,
) -> Result<Option<Value>, CodexLocalToolError> {
    let payload = list_models_payload(None)?;
    let models = payload
        .get("generationModels")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "resolved generation catalog did not contain generationModels".to_string(),
            )
        })?;
    Ok(models
        .iter()
        .find(|model| {
            model.get("provider").and_then(Value::as_str) == Some(provider)
                && model.get("id").and_then(Value::as_str) == Some(model_id)
        })
        .cloned())
}

fn attach_generation_display_names(models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .map(|mut model| {
            let provider = model
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let id = model
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if let Some(object) = model.as_object_mut() {
                object.entry("displayName").or_insert_with(|| {
                    Value::String(generation_model_display_name(&provider, &id))
                });
            }
            model
        })
        .collect()
}

fn attach_generation_pricing(models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .map(|mut model| {
            let provider = model
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let id = model
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if let Some(object) = model.as_object_mut() {
                match (provider.as_str(), id.as_str()) {
                    (FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID)
                    | (FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"480p": 0.7, "720p": 1.0, "1080p": 1.0}));
                        object
                            .entry("audioDiscountRate")
                            .or_insert_with(|| json!({"": 0.8}));
                    }
                    (FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"720p": 1.0, "1080p": 1.0}));
                        object
                            .entry("audioDiscountRate")
                            .or_insert_with(|| json!({"": 0.8}));
                    }
                    (FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"480p": 0.7, "580p": 1.0, "720p": 1.0}));
                    }
                    (FAL_PROVIDER, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID)
                    | (FAL_PROVIDER, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID)
                    | (FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"": 1.0}));
                        object
                            .entry("audioDiscountRate")
                            .or_insert_with(|| json!({"": 0.8}));
                    }
                    (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID)
                    | (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"720p": 1.0, "1080p": 1.0}));
                        object
                            .entry("audioDiscountRate")
                            .or_insert_with(|| json!({"": 0.8}));
                    }
                    (XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"480p": 1.0, "720p": 1.0}));
                    }
                    (GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID) => {
                        object
                            .entry("creditsPerSecond")
                            .or_insert_with(|| json!({"720p": 1.0, "1080p": 1.0, "4k": 2.0}));
                        object
                            .entry("audioDiscountRate")
                            .or_insert_with(|| json!({"": 0.8}));
                    }
                    (FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID)
                    | (FAL_PROVIDER, FAL_KREA_2_TURBO_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"": 1.0}));
                    }
                    (FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID) => {
                        object.entry("creditsPerImage").or_insert_with(|| {
                            json!({
                                "realistic_image": 2.0,
                                "digital_illustration": 2.0,
                                "vector_illustration": 2.0
                            })
                        });
                    }
                    (FAL_PROVIDER, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"1K": 1.0, "2K": 2.0, "4K": 4.0}));
                    }
                    (REPLICATE_PROVIDER, REPLICATE_FLUX_SCHNELL_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"": 1.0}));
                    }
                    (REPLICATE_PROVIDER, REPLICATE_FLUX_DEV_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"": 2.0}));
                    }
                    (REPLICATE_PROVIDER, REPLICATE_FLUX_11_PRO_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"": 3.0}));
                    }
                    (REPLICATE_PROVIDER, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"": 4.0}));
                    }
                    (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID)
                    | (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID)
                    | (XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID) => {
                        object
                            .entry("creditsPerImage")
                            .or_insert_with(|| json!({"": 2.0}));
                    }
                    (OPENAI_PROVIDER, OPENAI_GPT_4O_MINI_TTS_MODEL_ID)
                    | (ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID)
                    | (GOOGLE_PROVIDER, GOOGLE_GEMINI_TTS_MODEL_ID) => {
                        object
                            .entry("audioPricing")
                            .or_insert_with(|| json!({"mode": "perThousandChars", "rate": 15.0}));
                    }
                    (ELEVENLABS_PROVIDER, ELEVENLABS_MUSIC_MODEL_ID)
                    | (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID)
                    | (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID) => {
                        object
                            .entry("audioPricing")
                            .or_insert_with(|| json!({"mode": "flat", "price": 10.0}));
                    }
                    _ => {}
                }
            }
            model
        })
        .collect()
}

fn attach_generation_endpoint_metadata(models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .map(|mut model| {
            let provider = model
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let kind = model
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let category = model
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if let Some(object) = model.as_object_mut() {
                object
                    .entry("allowedEndpoints")
                    .or_insert_with(|| generation_allowed_endpoints(&kind, &category));
                object.entry("paidOnly").or_insert_with(|| json!(false));
                object.entry("cancellationCapability").or_insert_with(|| {
                    json!(
                        if matches!(provider.as_str(), FAL_PROVIDER | REPLICATE_PROVIDER) {
                            "provider"
                        } else {
                            "local"
                        }
                    )
                });
                if let Some(response_shape) = generation_response_shape(&kind) {
                    object
                        .entry("responseShape")
                        .or_insert_with(|| Value::String(response_shape.to_string()));
                }
            }
            model
        })
        .collect()
}

fn attach_generation_ui_capabilities(models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .map(|mut model| {
            let capabilities = generation_ui_capabilities(&model);
            if let (Some(object), Some(capabilities)) = (model.as_object_mut(), capabilities) {
                object
                    .entry("uiCapabilities")
                    .or_insert_with(|| capabilities);
            }
            model
        })
        .collect()
}

fn generation_ui_capabilities(model: &Value) -> Option<Value> {
    match model.get("kind").and_then(Value::as_str)? {
        "video" => Some(json!({
            "durations": generation_catalog_array(model, "durations"),
            "resolutions": generation_catalog_field(model, "resolutions"),
            "aspectRatios": generation_catalog_array(model, "aspectRatios"),
            "supportsFirstFrame": generation_catalog_bool(model, "supportsFirstFrame"),
            "supportsLastFrame": generation_catalog_bool(model, "supportsLastFrame"),
            "maxReferenceImages": generation_catalog_u64(model, "maxReferenceImages"),
            "maxReferenceVideos": generation_catalog_u64(model, "maxReferenceVideos"),
            "maxReferenceAudios": generation_catalog_u64(model, "maxReferenceAudios"),
            "maxTotalReferences": generation_catalog_field(model, "maxTotalReferences"),
            "maxCombinedVideoRefSeconds": generation_catalog_field(model, "maxCombinedVideoRefSeconds"),
            "maxCombinedAudioRefSeconds": generation_catalog_field(model, "maxCombinedAudioRefSeconds"),
            "framesAndReferencesExclusive": generation_catalog_bool(model, "framesAndReferencesExclusive"),
            "referenceTagNoun": model
                .get("referenceTagNoun")
                .cloned()
                .unwrap_or_else(|| json!("reference")),
            "requiresSourceVideo": generation_catalog_bool(model, "requiresSourceVideo"),
            "requiresReferenceImage": generation_catalog_bool(model, "requiresReferenceImage")
        })),
        "image" => Some(json!({
            "resolutions": generation_catalog_field(model, "resolutions"),
            "aspectRatios": generation_catalog_array_or(model, "aspectRatios", json!(["16:9", "9:16", "1:1"])),
            "qualities": generation_catalog_field(model, "qualities"),
            "supportsImageReference": generation_catalog_bool(model, "supportsImageReference"),
            "maxImages": generation_catalog_u64_or(model, "maxImages", 1)
        })),
        "audio" => Some(json!({
            "category": model.get("category").cloned().unwrap_or_else(|| json!("tts")),
            "voices": generation_audio_catalog_voices(model),
            "defaultVoice": generation_catalog_field(model, "defaultVoice"),
            "supportsLyrics": generation_catalog_bool(model, "supportsLyrics"),
            "supportsInstrumental": generation_catalog_bool(model, "supportsInstrumental"),
            "supportsStyleInstructions": generation_catalog_bool(model, "supportsStyleInstructions"),
            "durations": generation_catalog_field(model, "durations"),
            "minPromptLength": generation_catalog_u64_or(model, "minPromptLength", 1),
            "inputs": generation_catalog_array_or(model, "inputs", json!(["text"])),
            "promptLabel": model
                .get("promptLabel")
                .cloned()
                .unwrap_or_else(|| json!(AUDIO_SFX_PROMPT_LABEL)),
            "minSeconds": generation_catalog_u64_or(model, "minSeconds", 1),
            "maxSeconds": generation_catalog_u64_or(model, "maxSeconds", 900)
        })),
        "upscale" => Some(json!({
            "speed": model.get("speed").cloned().unwrap_or_else(|| json!("Medium")),
            "p75DurationSeconds": generation_upscale_p75_duration_seconds(model),
            "supportedTypes": generation_catalog_array(model, "supportedTypes")
        })),
        _ => None,
    }
}

fn generation_catalog_field(model: &Value, field: &str) -> Value {
    model.get(field).cloned().unwrap_or(Value::Null)
}

fn generation_catalog_array(model: &Value, field: &str) -> Value {
    generation_catalog_array_or(model, field, json!([]))
}

fn generation_catalog_array_or(model: &Value, field: &str, default: Value) -> Value {
    model
        .get(field)
        .filter(|value| value.is_array())
        .cloned()
        .unwrap_or(default)
}

fn generation_catalog_bool(model: &Value, field: &str) -> bool {
    model.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn generation_catalog_u64(model: &Value, field: &str) -> u64 {
    generation_catalog_u64_or(model, field, 0)
}

fn generation_catalog_u64_or(model: &Value, field: &str, default: u64) -> u64 {
    model.get(field).and_then(Value::as_u64).unwrap_or(default)
}

fn generation_audio_catalog_voices(model: &Value) -> Value {
    let provider = model
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = model.get("id").and_then(Value::as_str).unwrap_or_default();
    audio_generation_capabilities(provider, id)
        .filter(|capabilities| !capabilities.voices.is_empty())
        .map(|capabilities| json!(capabilities.voices))
        .unwrap_or(Value::Null)
}

fn generation_upscale_p75_duration_seconds(model: &Value) -> u64 {
    if let Some(duration) = model.get("p75DurationSeconds").and_then(Value::as_u64) {
        return duration;
    }

    let provider = model
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = model.get("id").and_then(Value::as_str).unwrap_or_default();
    match (provider, id) {
        ("mock", "mock-upscale-v1") => 2,
        (FAL_PROVIDER, FAL_AURA_SR_MODEL_ID) => 10,
        (FAL_PROVIDER, FAL_VIDEO_UPSCALER_MODEL_ID) => 120,
        _ => 0,
    }
}

fn generation_allowed_endpoints(kind: &str, category: &str) -> Value {
    match kind {
        "video" => json!(["generate_video"]),
        "image" => json!(["generate_image"]),
        "audio" => match category {
            "music" => json!(["generate_audio", "generate_music"]),
            "sfx" => json!(["generate_audio", "generate_sfx"]),
            _ => json!(["generate_audio"]),
        },
        "upscale" => json!(["upscale_media"]),
        _ => json!([]),
    }
}

fn generation_response_shape(kind: &str) -> Option<&'static str> {
    match kind {
        "video" => Some("video"),
        "image" => Some("images"),
        "audio" => Some("audio"),
        "upscale" => Some("upscaledImage"),
        _ => None,
    }
}

fn generation_model_display_name(provider: &str, model_id: &str) -> String {
    match (provider, model_id) {
        (FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID) => "fal.ai WAN Text to Video",
        (FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID) => "fal.ai WAN Image to Video",
        (FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID) => "fal.ai WAN Reference to Video",
        (FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID) => "fal.ai WAN Video to Video",
        (FAL_PROVIDER, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID) => {
            "fal.ai Kling V3 Standard Text to Video"
        }
        (FAL_PROVIDER, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID) => {
            "fal.ai Kling V3 Pro Image to Video"
        }
        (FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID) => {
            "fal.ai Kling V3 Pro Motion Control"
        }
        (FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID) => "fal.ai Flux Schnell",
        (FAL_PROVIDER, FAL_KREA_2_TURBO_MODEL_ID) => "fal.ai Krea 2 Turbo",
        (FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID) => "fal.ai Recraft V3",
        (FAL_PROVIDER, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID) => "fal.ai Nano Banana Pro Edit",
        (FAL_PROVIDER, FAL_SEED_AUDIO_MODEL_ID) => "fal.ai Seed Audio",
        (FAL_PROVIDER, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID) => "fal.ai Sonilo Text to Music",
        (FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID) => "fal.ai Sonilo Video to Music",
        (FAL_PROVIDER, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID) => "fal.ai Mirelo Video to Audio",
        (FAL_PROVIDER, FAL_AURA_SR_MODEL_ID) => "fal.ai AuraSR",
        (FAL_PROVIDER, FAL_VIDEO_UPSCALER_MODEL_ID) => "fal.ai Video Upscaler",
        (REPLICATE_PROVIDER, REPLICATE_FLUX_SCHNELL_MODEL_ID) => "Replicate Flux Schnell",
        (REPLICATE_PROVIDER, REPLICATE_FLUX_DEV_MODEL_ID) => "Replicate Flux Dev",
        (REPLICATE_PROVIDER, REPLICATE_FLUX_11_PRO_MODEL_ID) => "Replicate Flux 1.1 Pro",
        (REPLICATE_PROVIDER, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID) => {
            "Replicate Flux 1.1 Pro Ultra"
        }
        (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID) => "Replicate Seedance 2.0",
        (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID) => "Replicate Seedance 2.0 Fast",
        (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID) => "OpenAI GPT-image-2",
        (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID) => "OpenAI GPT-image-1.5",
        (OPENAI_PROVIDER, OPENAI_GPT_4O_MINI_TTS_MODEL_ID) => "OpenAI GPT-4o Mini TTS",
        (XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID) => "xAI Grok Image",
        (XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID) => "xAI Grok Video",
        (GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID) => "Google Veo 3.1 Fast",
        (GOOGLE_PROVIDER, GOOGLE_GEMINI_TTS_MODEL_ID) => "Google Gemini 3.1 Flash TTS",
        (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID) => "Google Lyria 3 Pro",
        (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID) => "MiniMax Music 2.6",
        (ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID) => "ElevenLabs TTS v3",
        (ELEVENLABS_PROVIDER, ELEVENLABS_MUSIC_MODEL_ID) => "ElevenLabs Music",
        ("mock", "mock-upscale-v1") => "Mock Upscaler",
        _ => model_id,
    }
    .to_string()
}

fn read_skill_payload(args: ReadSkillArgs) -> Result<Value, CodexLocalToolError> {
    let id = args.id.trim();
    if id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "id must not be blank".to_string(),
        ));
    }

    let definition = mandatory_skill_definition(id).ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!("unknown project skill: {id}"))
    })?;
    let body = definition.bundled_content;
    let prompt = mandatory_skill_prompt_bundle();

    Ok(json!({
        "id": id,
        "path": definition.relative_path,
        "body": body,
        "bodyBytes": body.len(),
        "checksum": sha256_checksum(body),
        "bundledChecksum": sha256_checksum(body),
        "loadState": "compiled",
        "promptIncluded": prompt.contains(&format!("### {id}")) && prompt.contains(body)
    }))
}

fn transcription_readiness_payload(
    project: &VideoProject,
    args: &TranscriptionReadinessArgs,
) -> Value {
    let Some(media) = project.media.iter().find(|media| media.id == args.media_id) else {
        return json!({
            "ready": false,
            "readyToTranscribe": false,
            "mediaId": args.media_id,
            "modelId": args.model_id,
            "mediaKind": Value::Null,
            "transcriptStatus": "missingMedia",
            "hasTranscript": false,
            "wordCount": 0,
            "segmentCount": 0,
            "supportedMediaKinds": ["audio", "video", "generated"],
            "nextAction": "Choose an existing audio, video, or generated media asset.",
            "error": format!("media was not found: {}", args.media_id)
        });
    };

    let transcript = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == args.media_id);
    let transcribable = matches!(
        media.kind,
        MediaKind::Audio | MediaKind::Video | MediaKind::Generated
    );

    if !transcribable {
        return json!({
            "ready": false,
            "readyToTranscribe": false,
            "mediaId": args.media_id,
            "modelId": args.model_id,
            "mediaKind": media.kind,
            "transcriptStatus": "unsupportedMedia",
            "hasTranscript": transcript.is_some(),
            "wordCount": transcript.map(|transcript| transcript.words.len()).unwrap_or(0),
            "segmentCount": transcript.map(|transcript| transcript.segments.len()).unwrap_or(0),
            "supportedMediaKinds": ["audio", "video", "generated"],
            "nextAction": "Select an audio, video, or generated media asset before requesting transcription.",
            "error": format!(
                "transcription supports audio, video, or generated media; {} is {:?}",
                args.media_id,
                media.kind
            )
        });
    }

    let Some(transcript) = transcript else {
        return json!({
            "ready": false,
            "readyToTranscribe": true,
            "mediaId": args.media_id,
            "modelId": args.model_id,
            "mediaKind": media.kind,
            "transcriptStatus": "missing",
            "hasTranscript": false,
            "wordCount": 0,
            "segmentCount": 0,
            "supportedMediaKinds": ["audio", "video", "generated"],
            "nextAction": "Start transcription for this media, then call video_creater.get_transcript.",
            "error": format!("transcript was not found for media: {}", args.media_id)
        });
    };

    json!({
        "ready": true,
        "readyToTranscribe": true,
        "mediaId": args.media_id,
        "modelId": args.model_id,
        "mediaKind": media.kind,
        "transcriptStatus": "ready",
        "hasTranscript": true,
        "transcriptId": transcript.id,
        "engine": transcript.engine,
        "rawArtifactPath": transcript.raw_artifact_path,
        "wordCount": transcript.words.len(),
        "segmentCount": transcript.segments.len(),
        "supportedMediaKinds": ["audio", "video", "generated"],
        "nextAction": "Call video_creater.get_transcript to inspect words or video_creater.remove_words to edit.",
        "error": Value::Null
    })
}

fn transcript_words_payload(
    project: &VideoProject,
    args: &TranscriptWordsArgs,
) -> Result<Value, CodexLocalToolError> {
    if args
        .media_id
        .as_deref()
        .is_some_and(|media_id| media_id.trim().is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "mediaId must not be blank".to_string(),
        ));
    }
    if matches!(args.limit, Some(0)) {
        return Err(CodexLocalToolError::InvalidArguments(
            "limit must be between 1 and 500".to_string(),
        ));
    }
    if args
        .language
        .as_deref()
        .is_some_and(|language| language.trim().is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "language must not be blank".to_string(),
        ));
    }
    if args
        .clip_id
        .as_deref()
        .is_some_and(|clip_id| clip_id.trim().is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "clipId must not be blank".to_string(),
        ));
    }
    let start_seconds = transcript_arg_start_seconds(project, args)?;
    let end_seconds = transcript_arg_end_seconds(project, args)?;
    if let (Some(start), Some(end)) = (start_seconds, end_seconds) {
        if end <= start {
            return Err(CodexLocalToolError::InvalidArguments(
                "endSeconds must be greater than startSeconds".to_string(),
            ));
        }
    }
    if matches!(start_seconds, Some(value) if value < 0.0)
        || matches!(end_seconds, Some(value) if value <= 0.0)
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "transcript word range must use non-negative seconds".to_string(),
        ));
    }

    let offset = args.offset.unwrap_or(0);
    let limit = args
        .limit
        .unwrap_or(DEFAULT_TRANSCRIPT_WORD_LIMIT)
        .min(MAX_TRANSCRIPT_WORD_LIMIT);
    let Some(media_id) = args.media_id.as_deref() else {
        return timeline_transcript_words_payload(project, args, offset, limit);
    };

    let media_exists = project.media.iter().any(|media| media.id == media_id);
    if !media_exists {
        return Ok(json!({
            "mediaId": media_id,
            "transcriptId": Value::Null,
            "engine": Value::Null,
            "rawArtifactPath": Value::Null,
            "range": {
                "startSeconds": args.start_seconds,
                "endSeconds": args.end_seconds
            },
            "offset": offset,
            "limit": limit,
            "error": format!("media was not found: {media_id}"),
            "words": [],
            "totalWords": 0,
            "matchedWords": 0,
            "returnedWords": 0,
            "nextOffset": Value::Null,
            "truncated": false
        }));
    }

    let Some(transcript) = project
        .transcripts
        .iter()
        .find(|transcript| transcript.media_id == media_id)
    else {
        return Ok(json!({
            "mediaId": media_id,
            "transcriptId": Value::Null,
            "engine": Value::Null,
            "rawArtifactPath": Value::Null,
            "range": {
                "startSeconds": args.start_seconds,
                "endSeconds": args.end_seconds
            },
            "offset": offset,
            "limit": limit,
            "error": format!("transcript was not found for media: {media_id}"),
            "words": [],
            "totalWords": 0,
            "matchedWords": 0,
            "returnedWords": 0,
            "nextOffset": Value::Null,
            "truncated": false
        }));
    };

    let start = start_seconds.unwrap_or(0.0);
    let end = end_seconds.unwrap_or(f64::INFINITY);
    let matched = transcript
        .words
        .iter()
        .enumerate()
        .filter(|(_, word)| word.end_seconds >= start && word.start_seconds <= end)
        .collect::<Vec<_>>();
    let words = matched
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(word_index, word)| {
            let timeline_ranges = transcript_word_timeline_ranges(project, media_id, word);
            json!({
                "wordIndex": word_index,
                "text": word.text,
                "startSeconds": word.start_seconds,
                "endSeconds": word.end_seconds,
                "sourceStartSeconds": word.start_seconds,
                "sourceEndSeconds": word.end_seconds,
                "timelineStartSeconds": timeline_ranges
                    .first()
                    .map(|range| range["timelineStartSeconds"].clone())
                    .unwrap_or(Value::Null),
                "timelineEndSeconds": timeline_ranges
                    .first()
                    .map(|range| range["timelineEndSeconds"].clone())
                    .unwrap_or(Value::Null),
                "timelineRanges": timeline_ranges,
                "confidence": word.confidence,
                "speaker": word.speaker
            })
        })
        .collect::<Vec<_>>();
    let next_offset = offset + words.len();
    let truncated = next_offset < matched.len();

    Ok(json!({
        "mediaId": media_id,
        "transcriptId": transcript.id,
        "engine": transcript.engine,
        "rawArtifactPath": transcript.raw_artifact_path,
        "range": {
            "startSeconds": args.start_seconds,
            "endSeconds": args.end_seconds
        },
        "offset": offset,
        "limit": limit,
        "totalWords": transcript.words.len(),
        "matchedWords": matched.len(),
        "returnedWords": words.len(),
        "nextOffset": if truncated { json!(next_offset) } else { Value::Null },
        "truncated": truncated,
        "words": words
    }))
}

fn transcript_arg_start_seconds(
    project: &VideoProject,
    args: &TranscriptWordsArgs,
) -> Result<Option<f64>, CodexLocalToolError> {
    match (args.start_seconds, args.start_frame) {
        (Some(seconds), _) => Ok(Some(seconds)),
        (None, Some(frame)) => Ok(Some(frames_to_seconds(frame, project_fps(project)?))),
        (None, None) => Ok(None),
    }
}

fn transcript_arg_end_seconds(
    project: &VideoProject,
    args: &TranscriptWordsArgs,
) -> Result<Option<f64>, CodexLocalToolError> {
    match (args.end_seconds, args.end_frame) {
        (Some(seconds), _) => Ok(Some(seconds)),
        (None, Some(frame)) => Ok(Some(frames_to_seconds(frame, project_fps(project)?))),
        (None, None) => Ok(None),
    }
}

#[derive(Debug, Clone)]
struct TimelineTranscriptWordMatch {
    word_index: usize,
    source_word_index: usize,
    text: String,
    timeline_start_seconds: f64,
    timeline_end_seconds: f64,
    source_start_seconds: f64,
    source_end_seconds: f64,
    media_id: String,
    transcript_id: String,
    track_id: String,
    track_index: usize,
    item_id: String,
    source_in: f64,
    source_out: f64,
    confidence: Option<f64>,
    speaker: Option<String>,
}

#[derive(Debug)]
struct TimelineTranscriptClipGroup {
    track_id: String,
    track_index: usize,
    clip_id: String,
    media_id: String,
    source_in: f64,
    source_out: f64,
    words: Vec<Value>,
    frame_words: Vec<Value>,
}

fn collect_timeline_transcript_word_matches(
    project: &VideoProject,
) -> Vec<TimelineTranscriptWordMatch> {
    let mut matches = Vec::new();
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        if !track.enabled {
            continue;
        }
        for item in &track.items {
            let TimelineSource::Media { media_id } = &item.source else {
                continue;
            };
            let Some(transcript) = project
                .transcripts
                .iter()
                .find(|transcript| transcript.media_id == *media_id)
            else {
                continue;
            };
            let (source_in, source_out, speed) = timeline_item_source_window(item);
            for (source_word_index, word) in transcript.words.iter().enumerate() {
                if word.start_seconds < source_in || word.end_seconds > source_out {
                    continue;
                }
                let (timeline_start_seconds, timeline_end_seconds) =
                    timeline_range_for_source_seconds(
                        item,
                        (source_in, source_out, speed),
                        (word.start_seconds, word.end_seconds),
                    );
                matches.push(TimelineTranscriptWordMatch {
                    word_index: 0,
                    source_word_index,
                    text: word.text.clone(),
                    timeline_start_seconds,
                    timeline_end_seconds,
                    source_start_seconds: word.start_seconds,
                    source_end_seconds: word.end_seconds,
                    media_id: media_id.clone(),
                    transcript_id: transcript.id.clone(),
                    track_id: track.id.clone(),
                    track_index,
                    item_id: item.id.clone(),
                    source_in,
                    source_out,
                    confidence: word.confidence,
                    speaker: word.speaker.clone(),
                });
            }
        }
    }
    matches.sort_by(|left, right| {
        left.timeline_start_seconds
            .partial_cmp(&right.timeline_start_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.track_index.cmp(&right.track_index))
            .then_with(|| left.item_id.cmp(&right.item_id))
            .then_with(|| left.source_word_index.cmp(&right.source_word_index))
    });
    for (word_index, timeline_word) in matches.iter_mut().enumerate() {
        timeline_word.word_index = word_index;
    }
    matches
}

fn timeline_word_payload(word: &TimelineTranscriptWordMatch, fps: f64) -> Value {
    let start_frame = seconds_to_frames(word.timeline_start_seconds, fps);
    let end_frame = seconds_to_frames(word.timeline_end_seconds, fps);
    json!({
        "wordIndex": word.word_index,
        "sourceWordIndex": word.source_word_index,
        "text": word.text,
        "startSeconds": round_tool_seconds(word.timeline_start_seconds),
        "endSeconds": round_tool_seconds(word.timeline_end_seconds),
        "timelineStartSeconds": round_tool_seconds(word.timeline_start_seconds),
        "timelineEndSeconds": round_tool_seconds(word.timeline_end_seconds),
        "startFrame": start_frame,
        "endFrame": end_frame,
        "timelineStartFrame": start_frame,
        "timelineEndFrame": end_frame,
        "sourceStartSeconds": word.source_start_seconds,
        "sourceEndSeconds": word.source_end_seconds,
        "mediaId": word.media_id,
        "transcriptId": word.transcript_id,
        "trackId": word.track_id,
        "itemId": word.item_id,
        "confidence": word.confidence,
        "speaker": word.speaker
    })
}

fn timeline_transcript_words_payload(
    project: &VideoProject,
    args: &TranscriptWordsArgs,
    offset: usize,
    limit: usize,
) -> Result<Value, CodexLocalToolError> {
    let fps = project_fps(project)?;
    let start = transcript_arg_start_seconds(project, args)?.unwrap_or(0.0);
    let end = transcript_arg_end_seconds(project, args)?.unwrap_or(f64::INFINITY);
    let clip_id_filter = args
        .clip_id
        .as_deref()
        .map(|clip_id| resolve_timeline_item_id(project, clip_id, "clipId"))
        .transpose()?;

    let filtered_words = collect_timeline_transcript_word_matches(project)
        .into_iter()
        .filter(|word| {
            clip_id_filter
                .as_deref()
                .is_none_or(|clip_id| clip_id == word.item_id)
                && word.timeline_end_seconds >= start
                && word.timeline_start_seconds <= end
        })
        .collect::<Vec<_>>();
    let includes_speakers = filtered_words.iter().any(|word| word.speaker.is_some());
    let word_format = if includes_speakers {
        json!(["wordIndex", "text", "startSeconds", "endSeconds", "speaker"])
    } else {
        json!(["wordIndex", "text", "startSeconds", "endSeconds"])
    };
    let frame_word_format = if includes_speakers {
        json!(["wordIndex", "text", "startFrame", "endFrame", "speaker"])
    } else {
        json!(["wordIndex", "text", "startFrame", "endFrame"])
    };
    let mut clips = Vec::<TimelineTranscriptClipGroup>::new();
    for word in &filtered_words {
        let start_frame = seconds_to_frames(word.timeline_start_seconds, fps);
        let end_frame = seconds_to_frames(word.timeline_end_seconds, fps);
        let clip_index = clips
            .iter()
            .position(|clip| clip.clip_id == word.item_id)
            .unwrap_or_else(|| {
                clips.push(TimelineTranscriptClipGroup {
                    track_id: word.track_id.clone(),
                    track_index: word.track_index,
                    clip_id: word.item_id.clone(),
                    media_id: word.media_id.clone(),
                    source_in: word.source_in,
                    source_out: word.source_out,
                    words: Vec::new(),
                    frame_words: Vec::new(),
                });
                clips.len() - 1
            });
        let mut word_row = vec![
            json!(word.word_index),
            json!(word.text),
            json!(round_tool_seconds(word.timeline_start_seconds)),
            json!(round_tool_seconds(word.timeline_end_seconds)),
        ];
        let mut frame_word_row = vec![
            json!(word.word_index),
            json!(word.text),
            json!(start_frame),
            json!(end_frame),
        ];
        if includes_speakers {
            word_row.push(
                word.speaker
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            frame_word_row.push(
                word.speaker
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
        }
        clips[clip_index].words.push(Value::Array(word_row));
        clips[clip_index]
            .frame_words
            .push(Value::Array(frame_word_row));
    }

    let total_words = filtered_words.len();
    let words = filtered_words
        .iter()
        .skip(offset)
        .take(limit)
        .map(|word| timeline_word_payload(word, fps))
        .collect::<Vec<_>>();
    let next_offset = offset + words.len();
    let truncated = next_offset < total_words;
    let clips = clips
        .into_iter()
        .map(|clip| {
            json!({
                "trackId": clip.track_id,
                "trackIndex": clip.track_index,
                "clipId": clip.clip_id,
                "mediaId": clip.media_id,
                "sourceIn": clip.source_in,
                "sourceOut": clip.source_out,
                "wordFormat": word_format,
                "frameWordFormat": frame_word_format,
                "frameWords": clip.frame_words,
                "words": clip.words
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "mode": "timeline",
        "mediaId": Value::Null,
        "range": {
            "startSeconds": start,
            "endSeconds": if end.is_finite() { json!(end) } else { Value::Null },
            "startFrame": args.start_frame,
            "endFrame": args.end_frame,
            "clipId": clip_id_filter
        },
        "offset": offset,
        "limit": limit,
        "wordFormat": word_format,
        "frameWordFormat": frame_word_format,
        "totalWords": total_words,
        "matchedWords": total_words,
        "returnedWords": words.len(),
        "nextOffset": if truncated { json!(next_offset) } else { Value::Null },
        "truncated": truncated,
        "clips": clips,
        "words": words
    }))
}

fn bounded_transcript_words(
    transcript: &Transcript,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    offset: usize,
    limit: usize,
) -> Vec<Value> {
    if limit == 0 {
        return Vec::new();
    }

    let start = start_seconds.unwrap_or(0.0);
    let end = end_seconds.unwrap_or(f64::INFINITY);
    transcript
        .words
        .iter()
        .enumerate()
        .filter(|(_, word)| word.end_seconds >= start && word.start_seconds <= end)
        .skip(offset)
        .take(limit)
        .map(|(word_index, word)| {
            json!({
                "wordIndex": word_index,
                "text": word.text,
                "startSeconds": word.start_seconds,
                "endSeconds": word.end_seconds,
                "confidence": word.confidence,
                "speaker": word.speaker
            })
        })
        .collect()
}

fn inspect_clip_transcript_words(
    transcript: &Transcript,
    track_id: &str,
    item: &TimelineItem,
    args: &InspectMediaArgs,
    fps: f64,
    limit: usize,
) -> Vec<Value> {
    if limit == 0 {
        return Vec::new();
    }

    let (source_in, source_out, speed) = timeline_item_source_window(item);
    let window_start_seconds = args
        .start_seconds
        .or_else(|| args.start_frame.map(|frame| frames_to_seconds(frame, fps)))
        .unwrap_or(0.0);
    let window_end_seconds = args
        .end_seconds
        .or_else(|| args.end_frame.map(|frame| frames_to_seconds(frame, fps)))
        .unwrap_or(f64::INFINITY);

    transcript
        .words
        .iter()
        .enumerate()
        .filter_map(|(word_index, word)| {
            if word.start_seconds < source_in || word.end_seconds > source_out {
                return None;
            }
            let (timeline_start_seconds, timeline_end_seconds) = timeline_range_for_source_seconds(
                item,
                (source_in, source_out, speed),
                (word.start_seconds, word.end_seconds),
            );
            if timeline_end_seconds < window_start_seconds
                || timeline_start_seconds > window_end_seconds
            {
                return None;
            }
            let start_frame = seconds_to_frames(timeline_start_seconds, fps);
            let end_frame = seconds_to_frames(timeline_end_seconds, fps);
            Some(json!({
                "wordIndex": word_index,
                "text": word.text,
                "startSeconds": round_tool_seconds(timeline_start_seconds),
                "endSeconds": round_tool_seconds(timeline_end_seconds),
                "timelineStartSeconds": round_tool_seconds(timeline_start_seconds),
                "timelineEndSeconds": round_tool_seconds(timeline_end_seconds),
                "sourceStartSeconds": word.start_seconds,
                "sourceEndSeconds": word.end_seconds,
                "startFrame": start_frame,
                "endFrame": end_frame,
                "timelineStartFrame": start_frame,
                "timelineEndFrame": end_frame,
                "clipId": item.id,
                "itemId": item.id,
                "trackId": track_id,
                "transcriptId": transcript.id,
                "confidence": word.confidence,
                "speaker": word.speaker
            }))
        })
        .take(limit)
        .collect()
}

fn transcript_word_timeline_ranges(
    project: &VideoProject,
    media_id: &str,
    word: &TranscriptWord,
) -> Vec<Value> {
    if !word.start_seconds.is_finite()
        || !word.end_seconds.is_finite()
        || word.end_seconds <= word.start_seconds
    {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    for track in &project.timeline.tracks {
        for item in &track.items {
            let TimelineSource::Media {
                media_id: item_media_id,
            } = &item.source
            else {
                continue;
            };
            if item_media_id != media_id {
                continue;
            }

            let (source_in, source_out, speed) = timeline_item_source_window(item);
            if word.start_seconds < source_in || word.end_seconds > source_out {
                continue;
            }

            let (timeline_start_seconds, timeline_end_seconds) = timeline_range_for_source_seconds(
                item,
                (source_in, source_out, speed),
                (word.start_seconds, word.end_seconds),
            );
            ranges.push(json!({
                "trackId": track.id,
                "itemId": item.id,
                "timelineStartSeconds": round_tool_seconds(timeline_start_seconds),
                "timelineEndSeconds": round_tool_seconds(timeline_end_seconds),
                "sourceStartSeconds": word.start_seconds,
                "sourceEndSeconds": word.end_seconds,
                "sourceIn": source_in,
                "sourceOut": source_out
            }));
        }
    }

    ranges
}

fn generation_defaults_payload(project: &VideoProject, args: &GenerationDefaultsArgs) -> Value {
    let selected_media = args
        .selected_media_id
        .as_ref()
        .and_then(|media_id| project.media.iter().find(|media| media.id == *media_id));
    let width = selected_media.and_then(|media| media.width).unwrap_or(1080);
    let height = selected_media
        .and_then(|media| media.height)
        .unwrap_or(1920);
    let fps = selected_media.and_then(|media| media.fps).unwrap_or(24.0);
    let duration_seconds = selected_media
        .map(|media| media.duration_seconds.clamp(1.0, 8.0))
        .unwrap_or(4.0);
    let references = selected_media
        .map(|media| vec![media.id.clone()])
        .unwrap_or_default();

    json!({
        "kind": "generated",
        "prompt": args.prompt.as_deref().unwrap_or(""),
        "model": {
            "provider": "mock",
            "id": "mock-video-v1"
        },
        "references": {
            "mediaIds": references,
            "firstFrameMediaId": args.selected_media_id,
            "lastFrameMediaId": Value::Null
        },
        "settings": {
            "width": width,
            "height": height,
            "durationSeconds": duration_seconds,
            "fps": fps,
            "aspectRatio": aspect_ratio_label(width, height)
        },
        "placementIntent": "library"
    })
}

fn build_generate_media_start_request_payload(
    project: &VideoProject,
    args: BuildGenerateMediaStartRequestArgs,
    generation_tool: Option<DirectGenerationTool>,
) -> Result<Value, CodexLocalToolError> {
    let brief = generation_brief_for_tool(project, &args, generation_tool)?;
    let project_dir = generation_project_dir(project, &args);
    let asset_id = generation_asset_id(project, &args, generation_tool, brief.as_ref());
    let job_id = generation_job_id(&args, &asset_id);
    let start_request = temporal_generate_media_start_request(
        &project.id,
        &project_dir,
        &asset_id,
        &job_id,
        args.mock_mode.unwrap_or(false),
        brief.clone(),
    );
    let project_actions = if let Some(generation_tool) = generation_tool {
        let brief = brief.unwrap_or_else(empty_generate_media_brief);
        generation_record_actions(
            project,
            &asset_id,
            &job_id,
            &start_request,
            generation_tool,
            &brief,
        )?
    } else {
        Vec::new()
    };

    let mut payload = json!({
        "startRequest": start_request,
    });
    if !project_actions.is_empty() {
        let payload_object = payload
            .as_object_mut()
            .expect("generate media payload should be object");
        payload_object.insert(
            "projectActionCount".to_string(),
            json!(project_actions.len()),
        );
        payload_object.insert("projectActions".to_string(), json!(project_actions));
        payload_object.insert(
            "nextRecommendedInspection".to_string(),
            json!("video_creater.generated_assets"),
        );
    }
    if generation_tool.is_some() {
        attach_generation_started_summary(&mut payload);
    }

    Ok(payload)
}

fn attach_generation_started_summary(payload: &mut Value) {
    let Some(input) = payload
        .get("startRequest")
        .and_then(|start_request| start_request.get("input"))
    else {
        return;
    };
    let asset_id = input.get("assetId").cloned();
    let job_id = input.get("jobId").cloned();
    let model = input.get("model").cloned();
    let references = input.get("references");
    let source_media_refs = references
        .and_then(|references| references.get("mediaIds"))
        .filter(|media_ids| media_ids.is_array())
        .cloned();
    let source_media_ref = references
        .and_then(|references| references.get("sourceVideoMediaRef"))
        .filter(|media_ref| !media_ref.is_null())
        .cloned()
        .or_else(|| {
            source_media_refs
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|media_ids| media_ids.first())
                .cloned()
        });

    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object
        .entry("status".to_string())
        .or_insert_with(|| json!("started"));
    if let Some(asset_id) = asset_id {
        object
            .entry("placeholderAssetId".to_string())
            .or_insert_with(|| asset_id.clone());
        object.entry("assetId".to_string()).or_insert(asset_id);
    }
    if let Some(job_id) = job_id {
        object.entry("jobId".to_string()).or_insert(job_id);
    }
    if let Some(model) = model {
        object.entry("model".to_string()).or_insert(model);
    }
    if let Some(source_media_ref) = source_media_ref {
        object
            .entry("sourceMediaRef".to_string())
            .or_insert(source_media_ref);
    }
    if let Some(source_media_refs) = source_media_refs {
        object
            .entry("sourceMediaRefs".to_string())
            .or_insert(source_media_refs);
    }
}

fn direct_generation_tool_payload(
    project: &VideoProject,
    args: BuildGenerateMediaStartRequestArgs,
    generation_tool: DirectGenerationTool,
) -> Result<(Value, bool), CodexLocalToolError> {
    let explicit_project_dir = args
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
        .map(str::to_string);
    let mut payload =
        build_generate_media_start_request_payload(project, args, Some(generation_tool))?;
    let mutates_project =
        apply_generated_media_project_actions_if_split_project(&mut payload, explicit_project_dir)?;
    Ok((payload, mutates_project))
}

fn audio_category_generation_tool_payload(
    project: &VideoProject,
    args: BuildGenerateMediaStartRequestArgs,
    category: &str,
) -> Result<(Value, bool), CodexLocalToolError> {
    let explicit_project_dir = args
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
        .map(str::to_string);
    let mut payload = build_audio_category_start_request_payload(project, args, category)?;
    let mutates_project =
        apply_generated_media_project_actions_if_split_project(&mut payload, explicit_project_dir)?;
    Ok((payload, mutates_project))
}

fn generated_media_rerun_tool_payload(
    project: &VideoProject,
    args: RerunGeneratedAssetArgs,
) -> Result<(Value, bool), CodexLocalToolError> {
    let explicit_project_dir = args
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
        .map(str::to_string);
    let mut payload = rerun_generated_asset_payload(project, args)?;
    let mutates_project =
        apply_generated_media_project_actions_if_split_project(&mut payload, explicit_project_dir)?;
    Ok((payload, mutates_project))
}

fn generated_media_upscale_tool_payload(
    project: &VideoProject,
    args: UpscaleMediaArgs,
) -> Result<(Value, bool), CodexLocalToolError> {
    let explicit_project_dir = args
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
        .map(str::to_string);
    let mut payload = upscale_media_payload(project, args)?;
    let mutates_project =
        apply_generated_media_project_actions_if_split_project(&mut payload, explicit_project_dir)?;
    Ok((payload, mutates_project))
}

fn apply_generated_media_project_actions_if_split_project(
    payload: &mut Value,
    project_dir: Option<String>,
) -> Result<bool, CodexLocalToolError> {
    let Some(project_dir) = project_dir else {
        return Ok(false);
    };
    let project_dir_path = Path::new(&project_dir);
    if !project_dir_path.join(storage::PROJECT_FILE_NAME).is_file() {
        return Ok(false);
    }
    let Some(project_actions) = payload
        .get("projectActions")
        .and_then(Value::as_array)
        .filter(|actions| !actions.is_empty())
    else {
        return Ok(false);
    };
    let actions =
        serde_json::from_value::<Vec<ProjectAction>>(Value::Array(project_actions.clone()))
            .map_err(|error| CodexLocalToolError::ProjectActionValidation(error.to_string()))?;
    let mut application = apply_project_actions_payload(ApplyProjectActionsArgs {
        project_dir,
        actions,
    })?;
    if let Some(object) = application.as_object_mut() {
        object.insert("source".to_string(), json!("generatedMediaPlaceholder"));
    }
    let persisted_asset_id = payload
        .get("startRequest")
        .and_then(|start_request| start_request.get("input"))
        .and_then(|input| input.get("assetId"))
        .cloned();
    if let Some(object) = payload.as_object_mut() {
        object.insert("projectActionApplication".to_string(), application);
        if let Some(asset_id) = persisted_asset_id {
            object.insert("persistedPlaceholderAssetId".to_string(), asset_id);
        }
    }
    Ok(true)
}

fn generation_project_dir(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
) -> String {
    args.project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_agent_project_dir(project))
}

fn generation_asset_id(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
    generation_tool: Option<DirectGenerationTool>,
    brief: Option<&TemporalGenerateMediaBrief>,
) -> String {
    if let Some(asset_id) = args
        .asset_id
        .as_deref()
        .map(str::trim)
        .filter(|asset_id| !asset_id.is_empty())
    {
        return asset_id.to_string();
    }

    let prefix = match generation_tool {
        Some(DirectGenerationTool::Video) => "agent-video",
        Some(DirectGenerationTool::Image) => "agent-image",
        Some(DirectGenerationTool::Audio) => "agent-audio",
        None => "agent-media",
    };
    let seed = brief
        .and_then(|brief| brief.name.as_deref().or(brief.prompt.as_deref()))
        .unwrap_or(prefix);
    let slug = media_folder_id_slug(seed);
    let base = if slug.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}-{slug}")
    };
    unique_generation_id(project, &base)
}

fn generation_job_id(args: &BuildGenerateMediaStartRequestArgs, asset_id: &str) -> String {
    args.job_id
        .as_deref()
        .map(str::trim)
        .filter(|job_id| !job_id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("job-{asset_id}"))
}

fn unique_generation_id(project: &VideoProject, base: &str) -> String {
    let mut used_ids: BTreeSet<String> = project
        .media
        .iter()
        .map(|media| media.id.clone())
        .chain(
            project
                .generated_assets
                .iter()
                .map(|asset| asset.id.clone()),
        )
        .chain(project.jobs.iter().map(|job| job.id.clone()))
        .collect();
    if used_ids.insert(base.to_string()) {
        return base.to_string();
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if used_ids.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search should return a unique generation id")
}

fn generation_brief_for_tool(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
    generation_tool: Option<DirectGenerationTool>,
) -> Result<Option<TemporalGenerateMediaBrief>, CodexLocalToolError> {
    let Some(generation_tool) = generation_tool else {
        return Ok(generate_media_brief_from_args(args));
    };

    let mut brief = generate_media_brief_from_args(args).unwrap_or_else(empty_generate_media_brief);
    normalize_generation_model_arg(&mut brief, generation_tool)?;
    apply_direct_generation_replacement(project, args, &mut brief)?;
    apply_direct_generation_media_refs(project, args, &mut brief, generation_tool)?;
    apply_generation_video_source_span(project, args, &mut brief, generation_tool)?;
    apply_palmier_reference_media_refs(project, &mut brief, generation_tool)?;
    if matches!(generation_tool, DirectGenerationTool::Audio) {
        normalize_audio_source_video_reference(&mut brief);
        default_audio_source_video_duration(project, &mut brief)?;
    }
    default_generation_model_arg(&mut brief, generation_tool);
    apply_direct_generation_folder_fallback(project, &mut brief, generation_tool)?;
    validate_direct_generation_capabilities(project, &brief, generation_tool)?;
    normalize_direct_generation_settings(&mut brief, generation_tool)?;

    Ok(Some(brief))
}

fn apply_direct_generation_replacement(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
    brief: &mut TemporalGenerateMediaBrief,
) -> Result<(), CodexLocalToolError> {
    let Some(replacement_item_id) = trimmed_opt(args.replacement_item_id.as_deref()) else {
        return Ok(());
    };
    timeline_item_with_track(project, replacement_item_id)?;
    brief.placement_intent = Some(generated_replacement_placement_intent(replacement_item_id));
    Ok(())
}

fn apply_direct_generation_folder_fallback(
    project: &VideoProject,
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    if brief
        .target_folder_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|folder_id| !folder_id.is_empty())
    {
        return Ok(());
    }
    if !matches!(
        generation_tool,
        DirectGenerationTool::Image | DirectGenerationTool::Video
    ) {
        return Ok(());
    }

    let references = generation_references_from_brief(brief)?;
    let media_folder = |media_id: &str| -> Option<String> {
        project
            .media
            .iter()
            .find(|media| media.id == media_id)
            .and_then(|media| media.folder_id.clone())
    };

    if matches!(generation_tool, DirectGenerationTool::Video) {
        if let Some(folder_id) = references
            .source_video_media_ref
            .as_deref()
            .and_then(&media_folder)
        {
            brief.target_folder_id = Some(folder_id);
            return Ok(());
        }
    }

    let reference_ids = references
        .first_frame_media_id
        .iter()
        .chain(references.last_frame_media_id.iter())
        .chain(references.reference_image_media_refs.iter())
        .chain(references.reference_video_media_refs.iter())
        .chain(references.reference_audio_media_refs.iter());
    if let Some(folder_id) = reference_ids
        .filter_map(|media_id| media_folder(media_id))
        .next_back()
    {
        brief.target_folder_id = Some(folder_id);
    }

    Ok(())
}

fn normalize_direct_generation_settings(
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    match generation_tool {
        DirectGenerationTool::Image => {
            let model = generation_model_from_brief(brief, generation_tool)?;
            let Some(capabilities) = image_generation_capabilities(&model.provider, &model.id)
            else {
                return Ok(());
            };
            let mut settings = generation_settings_from_brief(brief)?;
            let mut changed = false;

            let aspect_ratio_missing = settings
                .aspect_ratio
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty);
            if aspect_ratio_missing {
                if let Some(default_aspect_ratio) = capabilities.aspect_ratios.first() {
                    settings.aspect_ratio = Some((*default_aspect_ratio).to_string());
                    changed = true;
                }
            }

            let resolution_missing = settings
                .resolution
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty);
            if resolution_missing {
                if let Some(default_resolution) = capabilities.resolutions.first() {
                    settings.resolution = Some((*default_resolution).to_string());
                    changed = true;
                }
            }

            let quality_missing = settings
                .quality
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty);
            if quality_missing {
                if let Some(default_quality) = capabilities.qualities.last() {
                    settings.quality = Some((*default_quality).to_string());
                    changed = true;
                }
            }

            if let Some(resolution) = settings
                .resolution
                .as_deref()
                .map(str::trim)
                .filter(|resolution| !resolution.is_empty())
            {
                if let Some((width, height)) = parse_image_resolution(resolution) {
                    if capabilities.supports_flexible_resolution
                        || capabilities.resolutions.is_empty()
                        || capabilities.resolutions.contains(&resolution)
                    {
                        settings.width = Some(width);
                        settings.height = Some(height);
                        settings
                            .aspect_ratio
                            .get_or_insert_with(|| aspect_ratio_label(width, height));
                        changed = true;
                    }
                }
            }

            if changed {
                brief.settings = Some(json!(settings));
            }
        }
        DirectGenerationTool::Audio => {
            let model = generation_model_from_brief(brief, generation_tool)?;
            let Some(capabilities) = audio_generation_capabilities(&model.provider, &model.id)
            else {
                return Ok(());
            };
            let mut settings = generation_settings_from_brief(brief)?;
            let mut changed = false;

            if capabilities.default_voice.is_none()
                && capabilities.voices.is_empty()
                && settings.voice.take().is_some()
            {
                changed = true;
            }
            if !audio_model_supports_lyrics(&model.provider, &model.id)
                && settings.lyrics.take().is_some()
            {
                changed = true;
            }
            if !audio_model_supports_style_instructions(&model.provider, &model.id)
                && settings.style_instructions.take().is_some()
            {
                changed = true;
            }
            if !audio_model_supports_instrumental(&model.provider, &model.id)
                && settings.instrumental.take().is_some()
            {
                changed = true;
            }
            if audio_model_ignores_duration(&model.provider, &model.id)
                && settings.duration_seconds.take().is_some()
            {
                changed = true;
            }
            if audio_model_supports_instrumental(&model.provider, &model.id)
                && settings.instrumental.is_none()
            {
                settings.instrumental = Some(false);
                changed = true;
            }

            if let Some(default_voice) = capabilities.default_voice {
                let voice_missing = settings
                    .voice
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(str::is_empty);
                if voice_missing {
                    settings.voice = Some(default_voice.to_string());
                    changed = true;
                }
            }

            if !capabilities.durations.is_empty() {
                let duration_missing = settings
                    .duration_seconds
                    .filter(|duration| duration.is_finite() && *duration > 0.0)
                    .is_none();
                if duration_missing {
                    if let Some(default_duration) = capabilities.durations.first() {
                        settings.duration_seconds = Some(*default_duration as f64);
                        changed = true;
                    }
                }
            }

            if changed {
                brief.settings = Some(json!(settings));
            }
        }
        DirectGenerationTool::Video => {
            let model = generation_model_from_brief(brief, generation_tool)?;
            let references = generation_references_from_brief(brief)?;
            let settings = brief.settings.get_or_insert_with(|| json!({}));
            let object = settings.as_object_mut().ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(
                    "invalid generation settings: expected object".to_string(),
                )
            })?;
            if let Some(capabilities) = video_generation_capabilities(&model.provider, &model.id) {
                let is_source_video_edit = references.source_video_media_ref.is_some()
                    && video_capabilities_accept_source_video(&capabilities);
                if is_source_video_edit {
                    if object.get("sourceClipId").is_none() {
                        object.remove("durationSeconds");
                    }
                    object.remove("aspectRatio");
                    object.remove("resolution");
                } else {
                    let duration_missing = object
                        .get("durationSeconds")
                        .and_then(Value::as_f64)
                        .filter(|duration| duration.is_finite() && *duration > 0.0)
                        .is_none();
                    if duration_missing {
                        if let Some(default_duration) = capabilities.durations.first() {
                            object.insert("durationSeconds".to_string(), json!(default_duration));
                        }
                    }

                    let aspect_ratio_missing = object
                        .get("aspectRatio")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .is_none_or(str::is_empty);
                    if aspect_ratio_missing {
                        if let Some(default_aspect_ratio) = capabilities.aspect_ratios.first() {
                            object.insert("aspectRatio".to_string(), json!(default_aspect_ratio));
                        }
                    }

                    let resolution_missing = object
                        .get("resolution")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .is_none_or(str::is_empty);
                    if resolution_missing {
                        if let Some(default_resolution) = capabilities.resolutions.first() {
                            object.insert("resolution".to_string(), json!(default_resolution));
                        }
                    }
                }
                if !video_model_supports_audio_toggle(&model.provider, &model.id)
                    && object
                        .get("generateAudio")
                        .and_then(Value::as_bool)
                        .is_some_and(|generate_audio| !generate_audio)
                {
                    object.insert("generateAudio".to_string(), json!(true));
                }
            }
            if object
                .get("generateAudio")
                .and_then(Value::as_bool)
                .is_none()
            {
                object.insert("generateAudio".to_string(), json!(true));
            }
        }
    }
    Ok(())
}

fn validate_direct_generation_capabilities(
    project: &VideoProject,
    brief: &TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    let model = generation_model_from_brief(brief, generation_tool)?;
    let references = generation_references_from_brief(brief)?;
    let settings = generation_settings_from_brief(brief)?;

    match generation_tool {
        DirectGenerationTool::Video => {
            if let Some(capabilities) = video_generation_capabilities(&model.provider, &model.id) {
                validate_video_generation_capabilities(
                    project,
                    &capabilities,
                    &settings,
                    &references,
                )?;
            }
        }
        DirectGenerationTool::Image => {
            if let Some(capabilities) = image_generation_capabilities(&model.provider, &model.id) {
                validate_image_generation_capabilities(&capabilities, &settings, &references)?;
            }
        }
        DirectGenerationTool::Audio => {
            if let Some(capabilities) = audio_generation_capabilities(&model.provider, &model.id) {
                validate_audio_generation_capabilities(
                    &capabilities,
                    project,
                    brief.prompt.as_deref(),
                    &settings,
                    &references,
                )?;
            }
        }
    }

    validate_direct_generation_reference_media_kinds(project, &references)?;

    Ok(())
}

fn validate_direct_generation_reference_media_kinds(
    project: &VideoProject,
    references: &ProjectActionGeneratedAssetReferences,
) -> Result<(), CodexLocalToolError> {
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media.kind.clone()))
        .collect::<BTreeMap<_, _>>();

    validate_direct_generation_reference_kind(
        &media_by_id,
        references.source_video_media_ref.iter(),
        "sourceVideoMediaRef",
        |kind| matches!(kind, MediaKind::Video),
        "a video asset",
    )?;
    validate_direct_generation_reference_kind(
        &media_by_id,
        references.reference_video_media_refs.iter(),
        "referenceVideoMediaRefs",
        |kind| matches!(kind, MediaKind::Video),
        "a video asset",
    )?;
    validate_direct_generation_reference_kind(
        &media_by_id,
        references.reference_audio_media_refs.iter(),
        "referenceAudioMediaRefs",
        |kind| matches!(kind, MediaKind::Audio),
        "an audio asset",
    )?;
    validate_direct_generation_reference_kind(
        &media_by_id,
        references.first_frame_media_id.iter(),
        "firstFrameMediaId",
        |kind| matches!(kind, MediaKind::Image),
        "an image asset",
    )?;
    validate_direct_generation_reference_kind(
        &media_by_id,
        references.last_frame_media_id.iter(),
        "lastFrameMediaId",
        |kind| matches!(kind, MediaKind::Image),
        "an image asset",
    )?;
    validate_direct_generation_reference_kind(
        &media_by_id,
        references.reference_image_media_refs.iter(),
        "referenceImageMediaRefs",
        |kind| matches!(kind, MediaKind::Image),
        "an image asset",
    )?;
    validate_direct_generation_reference_exists(
        &media_by_id,
        references.media_ids.iter(),
        "mediaIds",
    )?;

    Ok(())
}

fn validate_direct_generation_reference_exists<'a, I>(
    media_by_id: &BTreeMap<&str, MediaKind>,
    media_ids: I,
    field_name: &str,
) -> Result<(), CodexLocalToolError>
where
    I: IntoIterator<Item = &'a String>,
{
    for media_id in media_ids {
        if !media_by_id.contains_key(media_id.as_str()) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{field_name} entry '{media_id}' references missing media"
            )));
        }
    }
    Ok(())
}

fn validate_direct_generation_reference_kind<'a, I, F>(
    media_by_id: &BTreeMap<&str, MediaKind>,
    media_ids: I,
    field_name: &str,
    accepts_kind: F,
    expected_label: &str,
) -> Result<(), CodexLocalToolError>
where
    I: IntoIterator<Item = &'a String>,
    F: Fn(&MediaKind) -> bool,
{
    for media_id in media_ids {
        let Some(kind) = media_by_id.get(media_id.as_str()) else {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{field_name} entry '{media_id}' references missing media"
            )));
        };
        if !accepts_kind(kind) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{field_name} entry '{media_id}' must be {expected_label}"
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct VideoGenerationCapabilities {
    display_name: &'static str,
    durations: &'static [u64],
    aspect_ratios: &'static [&'static str],
    resolutions: &'static [&'static str],
    supports_first_frame: bool,
    supports_last_frame: bool,
    requires_source_video: bool,
    requires_reference_image: bool,
    max_source_video_seconds: Option<f64>,
    max_reference_images: usize,
    max_reference_videos: usize,
    max_reference_audios: usize,
    max_total_references: Option<usize>,
    max_combined_video_ref_seconds: Option<f64>,
    max_combined_audio_ref_seconds: Option<f64>,
    frames_and_references_exclusive: bool,
}

#[derive(Debug, Clone, Copy)]
struct ImageGenerationCapabilities {
    display_name: &'static str,
    aspect_ratios: &'static [&'static str],
    resolutions: &'static [&'static str],
    qualities: &'static [&'static str],
    supports_flexible_resolution: bool,
    supports_image_reference: bool,
    max_reference_images: usize,
    max_images: u32,
}

#[derive(Debug, Clone, Copy)]
struct AudioGenerationCapabilities {
    display_name: &'static str,
    accepts_text: bool,
    accepts_video: bool,
    min_prompt_length: usize,
    default_voice: Option<&'static str>,
    voices: &'static [&'static str],
    durations: &'static [u64],
    min_seconds: Option<f64>,
    max_seconds: Option<f64>,
}

fn video_generation_capabilities(
    provider: &str,
    model_id: &str,
) -> Option<VideoGenerationCapabilities> {
    match (provider, model_id) {
        (FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID) => Some(VideoGenerationCapabilities {
            display_name: FAL_WAN_TEXT_TO_VIDEO_MODEL_ID,
            durations: &[5, 10],
            aspect_ratios: &["16:9", "9:16", "1:1"],
            resolutions: &["480p", "720p", "1080p"],
            supports_first_frame: false,
            supports_last_frame: false,
            requires_source_video: false,
            requires_reference_image: false,
            max_source_video_seconds: None,
            max_reference_images: 0,
            max_reference_videos: 0,
            max_reference_audios: 1,
            max_total_references: Some(1),
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: false,
        }),
        (FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID) => Some(VideoGenerationCapabilities {
            display_name: FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID,
            durations: &[5, 10],
            aspect_ratios: &["16:9", "9:16", "1:1"],
            resolutions: &["480p", "720p", "1080p"],
            supports_first_frame: true,
            supports_last_frame: true,
            requires_source_video: false,
            requires_reference_image: false,
            max_source_video_seconds: None,
            max_reference_images: 2,
            max_reference_videos: 1,
            max_reference_audios: 1,
            max_total_references: None,
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: false,
        }),
        (FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID) => Some(VideoGenerationCapabilities {
            display_name: FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID,
            durations: &[2, 3, 4, 5, 6, 7, 8, 9, 10],
            aspect_ratios: &["16:9", "9:16", "1:1", "4:3", "3:4"],
            resolutions: &["720p", "1080p"],
            supports_first_frame: false,
            supports_last_frame: false,
            requires_source_video: false,
            requires_reference_image: true,
            max_source_video_seconds: None,
            max_reference_images: 4,
            max_reference_videos: 3,
            max_reference_audios: 0,
            max_total_references: None,
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: false,
        }),
        (FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID) => Some(VideoGenerationCapabilities {
            display_name: FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID,
            durations: &[5, 10],
            aspect_ratios: &["auto", "16:9", "9:16", "1:1"],
            resolutions: &["480p", "580p", "720p"],
            supports_first_frame: false,
            supports_last_frame: false,
            requires_source_video: true,
            requires_reference_image: false,
            max_source_video_seconds: Some(10.0),
            max_reference_images: 1,
            max_reference_videos: 0,
            max_reference_audios: 0,
            max_total_references: None,
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: false,
        }),
        (FAL_PROVIDER, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID) => {
            Some(VideoGenerationCapabilities {
                display_name: FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID,
                durations: &[5, 10],
                aspect_ratios: &["16:9", "9:16", "1:1"],
                resolutions: &[],
                supports_first_frame: false,
                supports_last_frame: false,
                requires_source_video: false,
                requires_reference_image: false,
                max_source_video_seconds: None,
                max_reference_images: 0,
                max_reference_videos: 0,
                max_reference_audios: 0,
                max_total_references: Some(0),
                max_combined_video_ref_seconds: None,
                max_combined_audio_ref_seconds: None,
                frames_and_references_exclusive: false,
            })
        }
        (FAL_PROVIDER, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID) => {
            Some(VideoGenerationCapabilities {
                display_name: FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID,
                durations: &[5, 10],
                aspect_ratios: &["16:9", "9:16", "1:1"],
                resolutions: &[],
                supports_first_frame: true,
                supports_last_frame: true,
                requires_source_video: false,
                requires_reference_image: false,
                max_source_video_seconds: None,
                max_reference_images: 2,
                max_reference_videos: 0,
                max_reference_audios: 0,
                max_total_references: None,
                max_combined_video_ref_seconds: None,
                max_combined_audio_ref_seconds: None,
                frames_and_references_exclusive: false,
            })
        }
        (FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID) => {
            Some(VideoGenerationCapabilities {
                display_name: FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID,
                durations: &[5, 10],
                aspect_ratios: &[],
                resolutions: &[],
                supports_first_frame: false,
                supports_last_frame: false,
                requires_source_video: true,
                requires_reference_image: true,
                max_source_video_seconds: Some(10.0),
                max_reference_images: 1,
                max_reference_videos: 0,
                max_reference_audios: 0,
                max_total_references: None,
                max_combined_video_ref_seconds: None,
                max_combined_audio_ref_seconds: None,
                frames_and_references_exclusive: false,
            })
        }
        (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID) => {
            Some(VideoGenerationCapabilities {
                display_name: if model_id == REPLICATE_SEEDANCE_20_FAST_MODEL_ID {
                    REPLICATE_SEEDANCE_20_FAST_MODEL_ID
                } else {
                    REPLICATE_SEEDANCE_20_MODEL_ID
                },
                durations: &[5, 10],
                aspect_ratios: &["16:9", "9:16", "1:1"],
                resolutions: &["720p", "1080p"],
                supports_first_frame: true,
                supports_last_frame: true,
                requires_source_video: false,
                requires_reference_image: false,
                max_source_video_seconds: None,
                max_reference_images: 4,
                max_reference_videos: 3,
                max_reference_audios: 3,
                max_total_references: Some(6),
                max_combined_video_ref_seconds: Some(15.0),
                max_combined_audio_ref_seconds: Some(15.0),
                frames_and_references_exclusive: true,
            })
        }
        (GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID) => Some(VideoGenerationCapabilities {
            display_name: GOOGLE_VEO_31_FAST_MODEL_ID,
            durations: &[8],
            aspect_ratios: &["16:9", "9:16"],
            resolutions: &["720p", "1080p", "4k"],
            supports_first_frame: true,
            supports_last_frame: true,
            requires_source_video: false,
            requires_reference_image: false,
            max_source_video_seconds: None,
            max_reference_images: 3,
            max_reference_videos: 0,
            max_reference_audios: 0,
            max_total_references: Some(3),
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: false,
        }),
        (XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID) => Some(VideoGenerationCapabilities {
            display_name: XAI_GROK_VIDEO_MODEL_ID,
            durations: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            aspect_ratios: &["16:9", "9:16", "1:1", "4:3", "3:4", "3:2", "2:3"],
            resolutions: &["480p", "720p"],
            supports_first_frame: true,
            supports_last_frame: false,
            requires_source_video: false,
            requires_reference_image: false,
            max_source_video_seconds: Some(8.7),
            max_reference_images: 7,
            max_reference_videos: 0,
            max_reference_audios: 0,
            max_total_references: Some(7),
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: true,
        }),
        _ => None,
    }
}

fn video_capabilities_accept_source_video(capabilities: &VideoGenerationCapabilities) -> bool {
    capabilities.requires_source_video || capabilities.max_source_video_seconds.is_some()
}

fn video_model_supports_audio_toggle(provider: &str, model_id: &str) -> bool {
    matches!(
        (provider, model_id),
        (FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID)
            | (FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID)
            | (FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID)
            | (FAL_PROVIDER, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID)
            | (FAL_PROVIDER, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID)
            | (FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID)
            | (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID)
            | (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID)
            | (GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID)
    )
}

fn audio_generation_capabilities(
    provider: &str,
    model_id: &str,
) -> Option<AudioGenerationCapabilities> {
    match (provider, model_id) {
        (FAL_PROVIDER, FAL_SEED_AUDIO_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: FAL_SEED_AUDIO_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: None,
            max_seconds: None,
        }),
        (FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID,
            accepts_text: false,
            accepts_video: true,
            min_prompt_length: 0,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: Some(1.0),
            max_seconds: Some(900.0),
        }),
        (FAL_PROVIDER, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: Some(1.0),
            max_seconds: Some(600.0),
        }),
        (FAL_PROVIDER, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID,
            accepts_text: false,
            accepts_video: true,
            min_prompt_length: 0,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: Some(1.0),
            max_seconds: Some(900.0),
        }),
        (OPENAI_PROVIDER, OPENAI_GPT_4O_MINI_TTS_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: OPENAI_GPT_4O_MINI_TTS_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: Some("alloy"),
            voices: &[
                "alloy", "ash", "ballad", "cedar", "coral", "echo", "fable", "marin", "nova",
                "onyx", "sage", "shimmer", "verse",
            ],
            durations: &[],
            min_seconds: None,
            max_seconds: None,
        }),
        (GOOGLE_PROVIDER, GOOGLE_GEMINI_TTS_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: GOOGLE_GEMINI_TTS_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: Some(GOOGLE_GEMINI_TTS_DEFAULT_VOICE),
            voices: GOOGLE_GEMINI_TTS_VOICES,
            durations: &[],
            min_seconds: None,
            max_seconds: None,
        }),
        (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: GOOGLE_LYRIA_3_PRO_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: None,
            max_seconds: None,
        }),
        (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: MINIMAX_MUSIC_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 10,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: None,
            max_seconds: None,
        }),
        (ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: ELEVENLABS_TTS_V3_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: Some(ELEVENLABS_DEFAULT_VOICE),
            voices: &[ELEVENLABS_DEFAULT_VOICE],
            durations: &[],
            min_seconds: None,
            max_seconds: None,
        }),
        (ELEVENLABS_PROVIDER, ELEVENLABS_MUSIC_MODEL_ID) => Some(AudioGenerationCapabilities {
            display_name: ELEVENLABS_MUSIC_MODEL_ID,
            accepts_text: true,
            accepts_video: false,
            min_prompt_length: 1,
            default_voice: None,
            voices: &[],
            durations: &[],
            min_seconds: Some(3.0),
            max_seconds: Some(600.0),
        }),
        _ => None,
    }
}

fn audio_model_supports_lyrics(provider: &str, model_id: &str) -> bool {
    matches!(
        (provider, model_id),
        (ELEVENLABS_PROVIDER, ELEVENLABS_MUSIC_MODEL_ID)
            | (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID)
            | (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID)
    )
}

fn audio_model_supports_instrumental(provider: &str, model_id: &str) -> bool {
    matches!(
        (provider, model_id),
        (FAL_PROVIDER, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID)
            | (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID)
            | (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID)
            | (ELEVENLABS_PROVIDER, ELEVENLABS_MUSIC_MODEL_ID)
    )
}

fn audio_model_supports_style_instructions(provider: &str, model_id: &str) -> bool {
    !matches!(
        (provider, model_id),
        (FAL_PROVIDER, FAL_SEED_AUDIO_MODEL_ID)
    )
}

fn audio_model_ignores_duration(provider: &str, model_id: &str) -> bool {
    matches!(
        (provider, model_id),
        (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID) | (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID)
    )
}

fn image_generation_capabilities(
    provider: &str,
    model_id: &str,
) -> Option<ImageGenerationCapabilities> {
    match (provider, model_id) {
        (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID) => Some(ImageGenerationCapabilities {
            display_name: OPENAI_GPT_IMAGE_2_MODEL_ID,
            aspect_ratios: &["16:9", "9:16", "1:1"],
            resolutions: &["1024x1024", "1536x1024", "1024x1536"],
            qualities: &[],
            supports_flexible_resolution: true,
            supports_image_reference: false,
            max_reference_images: 0,
            max_images: 4,
        }),
        (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID) => Some(ImageGenerationCapabilities {
            display_name: OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
            aspect_ratios: &["16:9", "9:16", "1:1"],
            resolutions: &["auto", "1024x1024", "1536x1024", "1024x1536"],
            qualities: &[],
            supports_flexible_resolution: false,
            supports_image_reference: true,
            max_reference_images: 16,
            max_images: 1,
        }),
        (XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID) => Some(ImageGenerationCapabilities {
            display_name: XAI_GROK_IMAGE_QUALITY_MODEL_ID,
            aspect_ratios: &["16:9", "9:16", "1:1"],
            resolutions: &[],
            qualities: &[],
            supports_flexible_resolution: false,
            supports_image_reference: true,
            max_reference_images: 1,
            max_images: 1,
        }),
        (FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID) => Some(ImageGenerationCapabilities {
            display_name: FAL_FLUX_SCHNELL_MODEL_ID,
            aspect_ratios: &["1:1", "16:9", "9:16", "4:3", "3:4"],
            resolutions: &[],
            qualities: &[],
            supports_flexible_resolution: false,
            supports_image_reference: false,
            max_reference_images: 0,
            max_images: 4,
        }),
        (REPLICATE_PROVIDER, REPLICATE_FLUX_SCHNELL_MODEL_ID) => {
            Some(ImageGenerationCapabilities {
                display_name: REPLICATE_FLUX_SCHNELL_MODEL_ID,
                aspect_ratios: &["1:1", "16:9", "9:16", "4:3", "3:4"],
                resolutions: &[],
                qualities: &[],
                supports_flexible_resolution: false,
                supports_image_reference: false,
                max_reference_images: 0,
                max_images: 4,
            })
        }
        (FAL_PROVIDER, FAL_KREA_2_TURBO_MODEL_ID) => Some(ImageGenerationCapabilities {
            display_name: FAL_KREA_2_TURBO_MODEL_ID,
            aspect_ratios: &["1:1", "16:9", "9:16", "4:3", "3:4"],
            resolutions: &["1024x1024", "1024x576", "576x1024", "1024x768", "768x1024"],
            qualities: &[],
            supports_flexible_resolution: false,
            supports_image_reference: false,
            max_reference_images: 0,
            max_images: 1,
        }),
        (FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID) => {
            Some(ImageGenerationCapabilities {
                display_name: FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID,
                aspect_ratios: &["1:1", "16:9", "9:16", "4:3", "3:4"],
                resolutions: &["1024x1024", "1280x720", "720x1280", "1280x960", "960x1280"],
                qualities: &[
                    "realistic_image",
                    "digital_illustration",
                    "vector_illustration",
                ],
                supports_flexible_resolution: false,
                supports_image_reference: false,
                max_reference_images: 0,
                max_images: 1,
            })
        }
        (FAL_PROVIDER, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID) => Some(ImageGenerationCapabilities {
            display_name: FAL_NANO_BANANA_PRO_EDIT_MODEL_ID,
            aspect_ratios: &[
                "auto", "21:9", "16:9", "3:2", "4:3", "5:4", "1:1", "4:5", "3:4", "2:3", "9:16",
            ],
            resolutions: &["1K", "2K", "4K"],
            qualities: &[],
            supports_flexible_resolution: false,
            supports_image_reference: true,
            max_reference_images: 1,
            max_images: 1,
        }),
        _ => None,
    }
}

fn validate_video_generation_capabilities(
    project: &VideoProject,
    capabilities: &VideoGenerationCapabilities,
    settings: &ProjectActionGeneratedAssetSettings,
    references: &ProjectActionGeneratedAssetReferences,
) -> Result<(), CodexLocalToolError> {
    let has_source_video = references.source_video_media_ref.is_some();
    let is_source_video_edit =
        has_source_video && video_capabilities_accept_source_video(capabilities);

    if !is_source_video_edit {
        if let Some(duration) = settings.duration_seconds {
            let rounded_duration = duration.round() as u64;
            if !capabilities.durations.is_empty()
                && !capabilities.durations.contains(&rounded_duration)
            {
                return Err(CodexLocalToolError::InvalidArguments(
                    unsupported_generation_value(
                        capabilities.display_name,
                        "duration",
                        &format!("{rounded_duration}s"),
                        &capabilities
                            .durations
                            .iter()
                            .map(|duration| format!("{duration}s"))
                            .collect::<Vec<_>>(),
                    ),
                ));
            }
        }

        if let Some(aspect_ratio) = settings.aspect_ratio.as_deref().map(str::trim) {
            if !aspect_ratio.is_empty()
                && !capabilities.aspect_ratios.is_empty()
                && !capabilities.aspect_ratios.contains(&aspect_ratio)
            {
                return Err(CodexLocalToolError::InvalidArguments(
                    unsupported_generation_value(
                        capabilities.display_name,
                        "aspect ratio",
                        aspect_ratio,
                        &capabilities
                            .aspect_ratios
                            .iter()
                            .map(|value| value.to_string())
                            .collect::<Vec<_>>(),
                    ),
                ));
            }
        }
        if let Some(resolution) = settings.resolution.as_deref().map(str::trim) {
            if !resolution.is_empty()
                && !capabilities.resolutions.is_empty()
                && !capabilities.resolutions.contains(&resolution)
            {
                return Err(CodexLocalToolError::InvalidArguments(
                    unsupported_generation_value(
                        capabilities.display_name,
                        "resolution",
                        resolution,
                        &capabilities
                            .resolutions
                            .iter()
                            .map(|value| value.to_string())
                            .collect::<Vec<_>>(),
                    ),
                ));
            }
        }
    }

    if capabilities.requires_source_video && references.source_video_media_ref.is_none() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} requires a source video.",
            capabilities.display_name
        )));
    }
    if capabilities.requires_reference_image && references.reference_image_media_refs.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} requires an image reference",
            capabilities.display_name
        )));
    }
    if is_source_video_edit
        && (references.first_frame_media_id.is_some()
            || references.last_frame_media_id.is_some()
            || !references.reference_video_media_refs.is_empty()
            || !references.reference_audio_media_refs.is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} only accepts a source video and image references",
            capabilities.display_name
        )));
    }
    if has_source_video && !video_capabilities_accept_source_video(capabilities) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} does not accept a source video",
            capabilities.display_name
        )));
    }
    if is_source_video_edit {
        if let Some(max_source_video_seconds) = capabilities.max_source_video_seconds {
            if let Some(source_video_seconds) =
                source_video_input_duration_seconds(project, settings, references)
            {
                if source_video_seconds > max_source_video_seconds {
                    return Err(CodexLocalToolError::InvalidArguments(format!(
                        "{} accepts at most {:.1}s of source video",
                        capabilities.display_name, max_source_video_seconds
                    )));
                }
            }
        }
    }
    if references.first_frame_media_id.is_some() && !capabilities.supports_first_frame {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} does not accept frame references",
            capabilities.display_name
        )));
    }
    if references.last_frame_media_id.is_some() && !capabilities.supports_last_frame {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} does not accept a last frame",
            capabilities.display_name
        )));
    }
    if capabilities.frames_and_references_exclusive
        && (references.first_frame_media_id.is_some() || references.last_frame_media_id.is_some())
        && (!references.reference_image_media_refs.is_empty()
            || !references.reference_video_media_refs.is_empty()
            || !references.reference_audio_media_refs.is_empty())
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} uses frames OR references, not both. Clear one side.",
            capabilities.display_name
        )));
    }

    validate_reference_count(
        capabilities.display_name,
        "image",
        references.reference_image_media_refs.len(),
        capabilities.max_reference_images,
    )?;
    validate_reference_count(
        capabilities.display_name,
        "video",
        references.reference_video_media_refs.len(),
        capabilities.max_reference_videos,
    )?;
    validate_reference_count(
        capabilities.display_name,
        "audio",
        references.reference_audio_media_refs.len(),
        capabilities.max_reference_audios,
    )?;
    if let Some(max_total_references) = capabilities.max_total_references {
        let total_references = references.reference_image_media_refs.len()
            + references.reference_video_media_refs.len()
            + references.reference_audio_media_refs.len();
        if total_references > max_total_references {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{} accepts at most {} references total",
                capabilities.display_name, max_total_references
            )));
        }
    }
    if let Some(max_video_seconds) = capabilities.max_combined_video_ref_seconds {
        let total_video_seconds =
            combined_reference_duration_seconds(project, &references.reference_video_media_refs);
        if total_video_seconds > max_video_seconds {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "Combined video reference duration exceeds {:.0}s",
                max_video_seconds
            )));
        }
    }
    if let Some(max_audio_seconds) = capabilities.max_combined_audio_ref_seconds {
        let total_audio_seconds =
            combined_reference_duration_seconds(project, &references.reference_audio_media_refs);
        if total_audio_seconds > max_audio_seconds {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "Combined audio reference duration exceeds {:.0}s",
                max_audio_seconds
            )));
        }
    }

    Ok(())
}

fn combined_reference_duration_seconds(project: &VideoProject, media_refs: &[String]) -> f64 {
    media_refs
        .iter()
        .filter_map(|media_ref| project.media.iter().find(|media| media.id == *media_ref))
        .map(|media| media.duration_seconds.max(0.0))
        .sum()
}

fn source_video_input_duration_seconds(
    project: &VideoProject,
    settings: &ProjectActionGeneratedAssetSettings,
    references: &ProjectActionGeneratedAssetReferences,
) -> Option<f64> {
    let source_media_id = references.source_video_media_ref.as_deref()?;
    let media_duration = project
        .media
        .iter()
        .find(|media| media.id == source_media_id)
        .map(|media| media.duration_seconds)
        .filter(|duration| duration.is_finite() && *duration > 0.0)?;
    let start = settings
        .video_source_start_seconds
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(0.0)
        .min(media_duration);
    let end = settings
        .video_source_end_seconds
        .filter(|value| value.is_finite() && *value > start)
        .unwrap_or(media_duration)
        .min(media_duration);
    Some((end - start).max(0.0))
}

fn validate_image_generation_capabilities(
    capabilities: &ImageGenerationCapabilities,
    settings: &ProjectActionGeneratedAssetSettings,
    references: &ProjectActionGeneratedAssetReferences,
) -> Result<(), CodexLocalToolError> {
    if references.source_video_media_ref.is_some()
        || references.first_frame_media_id.is_some()
        || references.last_frame_media_id.is_some()
        || !references.reference_video_media_refs.is_empty()
        || !references.reference_audio_media_refs.is_empty()
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} only accepts image references",
            capabilities.display_name
        )));
    }

    if let Some(aspect_ratio) = settings.aspect_ratio.as_deref().map(str::trim) {
        if !aspect_ratio.is_empty()
            && !capabilities.aspect_ratios.is_empty()
            && !capabilities.aspect_ratios.contains(&aspect_ratio)
        {
            return Err(CodexLocalToolError::InvalidArguments(
                unsupported_generation_value(
                    capabilities.display_name,
                    "aspect ratio",
                    aspect_ratio,
                    &capabilities
                        .aspect_ratios
                        .iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>(),
                ),
            ));
        }
    }
    if let Some(resolution) = settings.resolution.as_deref().map(str::trim) {
        if !resolution.is_empty() {
            if capabilities.supports_flexible_resolution {
                validate_flexible_image_resolution(capabilities.display_name, resolution)?;
            } else if !capabilities.resolutions.is_empty()
                && !capabilities.resolutions.contains(&resolution)
            {
                return Err(CodexLocalToolError::InvalidArguments(
                    unsupported_generation_value(
                        capabilities.display_name,
                        "resolution",
                        resolution,
                        &capabilities
                            .resolutions
                            .iter()
                            .map(|value| value.to_string())
                            .collect::<Vec<_>>(),
                    ),
                ));
            }
        }
    }
    if let Some(quality) = settings.quality.as_deref().map(str::trim) {
        if !quality.is_empty()
            && !capabilities.qualities.is_empty()
            && !capabilities.qualities.contains(&quality)
        {
            return Err(CodexLocalToolError::InvalidArguments(
                unsupported_generation_value(
                    capabilities.display_name,
                    "quality",
                    quality,
                    &capabilities
                        .qualities
                        .iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>(),
                ),
            ));
        }
    }
    if let Some(num_images) = settings.num_images {
        if num_images < 1 || num_images > capabilities.max_images {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{} supports 1..{} images per request (got {})",
                capabilities.display_name, capabilities.max_images, num_images
            )));
        }
    }

    let image_ref_count = references.reference_image_media_refs.len();
    if image_ref_count > 0 && !capabilities.supports_image_reference {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} does not accept reference images",
            capabilities.display_name
        )));
    }
    validate_reference_count(
        capabilities.display_name,
        "image",
        image_ref_count,
        capabilities.max_reference_images,
    )?;
    Ok(())
}

fn validate_flexible_image_resolution(
    display_name: &str,
    resolution: &str,
) -> Result<(), CodexLocalToolError> {
    let Some((width, height)) = parse_image_resolution(resolution) else {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{display_name} resolution must use WIDTHxHEIGHT with positive integers"
        )));
    };
    if width % 16 != 0 || height % 16 != 0 {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{display_name} resolution must use WIDTHxHEIGHT with width and height divisible by 16"
        )));
    }
    let aspect_ratio = width as f64 / height as f64;
    if !(1.0 / 3.0..=3.0).contains(&aspect_ratio) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{display_name} resolution aspect ratio must be between 1:3 and 3:1"
        )));
    }
    if width > 3840 || height > 2160 {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{display_name} resolution must be at most 3840x2160"
        )));
    }
    Ok(())
}

fn parse_image_resolution(resolution: &str) -> Option<(u32, u32)> {
    let (width, height) = resolution.split_once('x')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

fn validate_audio_generation_capabilities(
    capabilities: &AudioGenerationCapabilities,
    project: &VideoProject,
    prompt: Option<&str>,
    settings: &ProjectActionGeneratedAssetSettings,
    references: &ProjectActionGeneratedAssetReferences,
) -> Result<(), CodexLocalToolError> {
    let has_video_input = references.source_video_media_ref.is_some()
        || settings.video_source_start_frame.is_some()
        || settings.video_source_end_frame.is_some();

    if !capabilities.accepts_video && has_video_input {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} does not accept a video input",
            capabilities.display_name
        )));
    }
    if capabilities.accepts_video && !has_video_input {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{} requires a video input",
            capabilities.display_name
        )));
    }
    if capabilities.accepts_text && capabilities.min_prompt_length > 0 {
        let prompt_len = prompt.map(str::trim).unwrap_or_default().chars().count();
        if prompt_len < capabilities.min_prompt_length {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{} requires a prompt with at least {} character",
                capabilities.display_name, capabilities.min_prompt_length
            )));
        }
    }
    if let Some(voice) = settings.voice.as_deref().map(str::trim) {
        if !voice.is_empty()
            && !capabilities.voices.is_empty()
            && !capabilities.voices.contains(&voice)
        {
            return Err(CodexLocalToolError::InvalidArguments(
                unsupported_generation_value(
                    capabilities.display_name,
                    "voice",
                    voice,
                    &capabilities
                        .voices
                        .iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>(),
                ),
            ));
        }
    }
    if let Some(duration_seconds) = settings.duration_seconds {
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "{} requires a positive duration",
                capabilities.display_name
            )));
        }
        let rounded_duration = duration_seconds.round() as u64;
        if !capabilities.durations.is_empty() && !capabilities.durations.contains(&rounded_duration)
        {
            return Err(CodexLocalToolError::InvalidArguments(
                unsupported_generation_value(
                    capabilities.display_name,
                    "duration",
                    &format!("{rounded_duration}s"),
                    &capabilities
                        .durations
                        .iter()
                        .map(|duration| format!("{duration}s"))
                        .collect::<Vec<_>>(),
                ),
            ));
        }
    }
    let constrained_duration_seconds = if capabilities.accepts_video {
        source_video_input_duration_seconds(project, settings, references)
            .or(settings.duration_seconds)
    } else {
        settings.duration_seconds
    };
    if let Some(duration_seconds) = constrained_duration_seconds {
        let duration_subject =
            if capabilities.accepts_video && references.source_video_media_ref.is_some() {
                "source video"
            } else {
                "duration"
            };
        if let Some(min_seconds) = capabilities.min_seconds {
            if duration_seconds < min_seconds {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "{} requires at least {:.0}s of {}",
                    capabilities.display_name, min_seconds, duration_subject
                )));
            }
        }
        if let Some(max_seconds) = capabilities.max_seconds {
            if duration_seconds > max_seconds {
                return Err(CodexLocalToolError::InvalidArguments(format!(
                    "{} accepts at most {:.0}s of {}",
                    capabilities.display_name, max_seconds, duration_subject
                )));
            }
        }
    }
    Ok(())
}

fn validate_reference_count(
    display_name: &str,
    noun: &str,
    actual: usize,
    max: usize,
) -> Result<(), CodexLocalToolError> {
    if actual > max {
        let suffix = if max == 1 { "" } else { "s" };
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{display_name} accepts at most {max} {noun} reference{suffix}"
        )));
    }
    Ok(())
}

fn unsupported_generation_value(
    display_name: &str,
    field: &str,
    value: &str,
    allowed: &[String],
) -> String {
    format!(
        "{display_name} does not support {field} '{value}'. Valid: {}.",
        allowed.join(", ")
    )
}

fn empty_generate_media_brief() -> TemporalGenerateMediaBrief {
    TemporalGenerateMediaBrief {
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: None,
        model: None,
        references: None,
        settings: None,
    }
}

fn normalize_generation_model_arg(
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    let Some(model) = &brief.model else {
        return Ok(());
    };
    if model.is_object() {
        return Ok(());
    }
    let Some(id) = model.as_str().map(str::trim).filter(|id| !id.is_empty()) else {
        return Err(CodexLocalToolError::InvalidArguments(
            "generation model must be an object or non-empty string".to_string(),
        ));
    };
    let (provider, id) = generation_provider_and_model_from_string(generation_tool, id);
    brief.model = Some(json!({
        "provider": provider,
        "id": id
    }));
    Ok(())
}

fn generation_provider_and_model_from_string(
    generation_tool: DirectGenerationTool,
    model: &str,
) -> (&'static str, String) {
    if let Some((provider, id)) = provider_qualified_model_string(model) {
        if let Some((alias_provider, alias_id)) =
            palmier_generation_model_alias(generation_tool, &id)
        {
            if provider == alias_provider {
                return (alias_provider, alias_id.to_string());
            }
        }
        return (provider, id);
    }

    if let Some((provider, id)) = palmier_generation_model_alias(generation_tool, model) {
        return (provider, id.to_string());
    }

    let provider = match generation_tool {
        DirectGenerationTool::Image
            if matches!(
                model,
                OPENAI_GPT_IMAGE_2_MODEL_ID | OPENAI_GPT_IMAGE_EDIT_MODEL_ID
            ) =>
        {
            OPENAI_PROVIDER
        }
        DirectGenerationTool::Image if model == XAI_GROK_IMAGE_QUALITY_MODEL_ID => XAI_PROVIDER,
        DirectGenerationTool::Video if model == XAI_GROK_VIDEO_MODEL_ID => XAI_PROVIDER,
        DirectGenerationTool::Video if model == GOOGLE_VEO_31_FAST_MODEL_ID => GOOGLE_PROVIDER,
        DirectGenerationTool::Audio
            if matches!(
                model,
                GOOGLE_GEMINI_TTS_MODEL_ID | GOOGLE_LYRIA_3_PRO_MODEL_ID
            ) =>
        {
            GOOGLE_PROVIDER
        }
        DirectGenerationTool::Audio if model == MINIMAX_MUSIC_MODEL_ID => MINIMAX_PROVIDER,
        DirectGenerationTool::Audio
            if matches!(
                model,
                ELEVENLABS_TTS_V3_MODEL_ID | ELEVENLABS_MUSIC_MODEL_ID
            ) =>
        {
            ELEVENLABS_PROVIDER
        }
        DirectGenerationTool::Image
            if matches!(
                model,
                REPLICATE_FLUX_SCHNELL_MODEL_ID
                    | REPLICATE_FLUX_DEV_MODEL_ID
                    | REPLICATE_FLUX_11_PRO_MODEL_ID
                    | REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID
            ) =>
        {
            REPLICATE_PROVIDER
        }
        DirectGenerationTool::Video | DirectGenerationTool::Image => FAL_PROVIDER,
        DirectGenerationTool::Audio if model == "mock-audio-v1" => "mock",
        DirectGenerationTool::Audio => FAL_PROVIDER,
    };
    (provider, model.to_string())
}

fn palmier_generation_model_alias(
    generation_tool: DirectGenerationTool,
    model: &str,
) -> Option<(&'static str, &'static str)> {
    match (generation_tool, model) {
        (DirectGenerationTool::Image, "nano-banana-pro") => {
            Some((FAL_PROVIDER, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID))
        }
        (DirectGenerationTool::Audio, "sonilo-v1.1-video-to-music") => {
            Some((FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID))
        }
        (DirectGenerationTool::Audio, "sonilo-v1.1-text-to-music") => {
            Some((FAL_PROVIDER, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID))
        }
        (DirectGenerationTool::Audio, "mirelo-sfx-v1.5-video-to-audio") => {
            Some((FAL_PROVIDER, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID))
        }
        (DirectGenerationTool::Audio, "gemini-3.1-flash-tts") => {
            Some((GOOGLE_PROVIDER, GOOGLE_GEMINI_TTS_MODEL_ID))
        }
        _ => None,
    }
}

fn provider_qualified_model_string(model: &str) -> Option<(&'static str, String)> {
    for separator in [':', '/'] {
        if let Some((provider, id)) = model.split_once(separator) {
            let provider = provider.trim();
            let id = id.trim();
            if provider == REPLICATE_PROVIDER && !id.is_empty() {
                return Some((REPLICATE_PROVIDER, id.to_string()));
            }
            if provider == FAL_PROVIDER && !id.is_empty() {
                return Some((FAL_PROVIDER, id.to_string()));
            }
            if provider == OPENAI_PROVIDER && !id.is_empty() {
                return Some((OPENAI_PROVIDER, id.to_string()));
            }
            if provider == XAI_PROVIDER && !id.is_empty() {
                return Some((XAI_PROVIDER, id.to_string()));
            }
            if provider == ELEVENLABS_PROVIDER && !id.is_empty() {
                return Some((ELEVENLABS_PROVIDER, id.to_string()));
            }
            if provider == GOOGLE_PROVIDER && !id.is_empty() {
                return Some((GOOGLE_PROVIDER, id.to_string()));
            }
            if provider == "mock" && !id.is_empty() {
                return Some(("mock", id.to_string()));
            }
        }
    }
    None
}

fn default_generation_model_arg(
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) {
    if brief.model.is_some() {
        return;
    }
    let (provider, id) = default_generation_model_for_brief(brief, generation_tool);
    brief.model = Some(json!({
        "provider": provider,
        "id": id
    }));
}

fn default_generation_model_for_brief(
    brief: &TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> (&'static str, &'static str) {
    if matches!(generation_tool, DirectGenerationTool::Video)
        && generation_brief_has_source_video(brief)
    {
        return (FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID);
    }
    if matches!(generation_tool, DirectGenerationTool::Video)
        && generation_brief_has_first_frame_input(brief)
    {
        return (FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID);
    }
    if matches!(generation_tool, DirectGenerationTool::Audio)
        && generation_brief_has_video_source_input(brief)
    {
        return (FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID);
    }

    default_generation_model(generation_tool)
}

fn apply_direct_generation_media_refs(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    let media_id = direct_generation_media_id(project, args)?;
    if let Some(media_id) = &media_id {
        let references = brief
            .references
            .take()
            .filter(|value| value.is_object())
            .unwrap_or_else(|| json!({}));
        let mut references = references;
        let object = references
            .as_object_mut()
            .expect("references should be object");
        let mut media_ids = object
            .get("mediaIds")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !media_ids
            .iter()
            .any(|value| value.as_str() == Some(media_id.as_str()))
        {
            media_ids.push(json!(media_id));
        }
        object.insert("mediaIds".to_string(), json!(media_ids));
        let media_is_video = project
            .media
            .iter()
            .any(|media| media.id == *media_id && media.kind == MediaKind::Video);
        if media_is_video
            && (args.source_clip_id.is_some()
                || (matches!(generation_tool, DirectGenerationTool::Video)
                    && args.source_video_media_id.is_none()))
        {
            object.insert("sourceVideoMediaRef".to_string(), json!(media_id));
        }
        if matches!(generation_tool, DirectGenerationTool::Video)
            && args.source_video_media_id.is_none()
            && args.start_frame_media_ref.is_none()
            && project
                .media
                .iter()
                .any(|media| media.id == *media_id && media.kind == MediaKind::Image)
        {
            object.insert("firstFrameMediaId".to_string(), json!(media_id));
            object.insert("startFrameMediaRef".to_string(), json!(media_id));
        }
        brief.references = Some(references);
    }

    if let Some(source_clip_id) = args.source_clip_id.as_deref() {
        let (_track, item) = timeline_item_with_track(project, source_clip_id)?;
        let TimelineSource::Media {
            media_id: item_media_id,
        } = &item.source
        else {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "sourceClipId {source_clip_id} does not reference media"
            )));
        };
        if media_id
            .as_deref()
            .is_some_and(|media_id| media_id != item_media_id)
        {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "sourceClipId {source_clip_id} does not reference mediaRef {}",
                media_id.as_deref().unwrap_or_default()
            )));
        }
        let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
        let source_out = timeline_item_number_property(item, "sourceOut")
            .unwrap_or(source_in + item.duration_seconds * timeline_item_playback_speed(item));
        let settings = brief
            .settings
            .take()
            .filter(|value| value.is_object())
            .unwrap_or_else(|| json!({}));
        let mut settings = settings;
        let object = settings.as_object_mut().expect("settings should be object");
        object.insert("sourceClipId".to_string(), json!(source_clip_id));
        object.insert("sourceIn".to_string(), json!(source_in));
        object.insert("sourceOut".to_string(), json!(source_out));
        object.insert("videoSourceStartSeconds".to_string(), json!(source_in));
        object.insert("videoSourceEndSeconds".to_string(), json!(source_out));
        object.insert("durationSeconds".to_string(), json!(item.duration_seconds));
        brief.settings = Some(settings);
    }

    Ok(())
}

fn direct_generation_media_id(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
) -> Result<Option<String>, CodexLocalToolError> {
    let top_level_media_id = args
        .media_id
        .as_deref()
        .or(args.source_video_media_id.as_deref());

    if let Some(media_id) = top_level_media_id
        .map(str::trim)
        .filter(|media_id| !media_id.is_empty())
    {
        if !project.media.iter().any(|media| media.id == media_id) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "mediaRef was not found: {media_id}"
            )));
        }
        return Ok(Some(media_id.to_string()));
    }

    let Some(source_clip_id) = args.source_clip_id.as_deref() else {
        return Ok(None);
    };
    let (_track, item) = timeline_item_with_track(project, source_clip_id)?;
    let TimelineSource::Media { media_id } = &item.source else {
        return Ok(None);
    };
    Ok(Some(media_id.clone()))
}

fn generation_record_actions(
    project: &VideoProject,
    asset_id: &str,
    job_id: &str,
    start_request: &crate::project::model::TemporalWorkflowStartRequest,
    generation_tool: DirectGenerationTool,
    brief: &TemporalGenerateMediaBrief,
) -> Result<Vec<ProjectAction>, CodexLocalToolError> {
    let prompt = brief
        .prompt
        .clone()
        .unwrap_or_else(|| "Agent requested generated media.".to_string());
    let model = generation_model_from_brief(brief, generation_tool)?;
    let references = generation_references_from_brief(brief)?;
    let settings = generation_settings_from_brief(brief)?;
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        job_id,
        JobStatus::Queued,
        &project.updated_at,
    );
    job.start_request = Some(start_request.clone());

    let mut actions = vec![
        ProjectAction::RecordGeneratedAsset {
            asset: Box::new(ProjectActionGeneratedAsset {
                id: asset_id.to_string(),
                kind: MediaKind::Generated,
                status: GeneratedAssetStatus::Queued,
                name: brief.name.clone(),
                target_folder_id: brief.target_folder_id.clone(),
                placement_intent: brief.placement_intent.clone(),
                prompt,
                model,
                references,
                settings: settings.clone(),
                outputs: Vec::new(),
                created_at: project.updated_at.clone(),
                parent_asset_id: None,
                retry_of_asset_id: None,
            }),
        },
        ProjectAction::RecordJob { job: Box::new(job) },
    ];
    actions.extend(generated_audio_timeline_placeholder_actions(
        project,
        asset_id,
        generation_tool,
        brief,
        &settings,
    ));
    validate_project_actions_payload(project, actions.clone())?;
    Ok(actions)
}

fn generated_audio_timeline_placeholder_actions(
    project: &VideoProject,
    asset_id: &str,
    generation_tool: DirectGenerationTool,
    brief: &TemporalGenerateMediaBrief,
    settings: &ProjectActionGeneratedAssetSettings,
) -> Vec<ProjectAction> {
    if !matches!(generation_tool, DirectGenerationTool::Audio) {
        return Vec::new();
    }
    if brief.placement_intent.as_deref() != Some("timeline") {
        return Vec::new();
    }
    let Some(start_seconds) = settings
        .timeline_start_seconds
        .filter(|start_seconds| start_seconds.is_finite() && *start_seconds >= 0.0)
    else {
        return Vec::new();
    };
    let Some(duration_seconds) = settings
        .duration_seconds
        .filter(|duration_seconds| duration_seconds.is_finite() && *duration_seconds > 0.0)
    else {
        return Vec::new();
    };

    let mut actions = Vec::new();
    let target_track_id = existing_audio_track_id(project).unwrap_or_else(|| {
        let track_id = next_palmier_track_id(project, "audio");
        actions.push(ProjectAction::CreateTrack {
            track: TimelineTrack::empty(&track_id, "Palmier Audio", TrackKind::Audio),
            after_track_id: None,
        });
        track_id
    });

    let item_id = format!("{asset_id}-timeline-audio");
    actions.push(ProjectAction::AddItems {
        target_track_id,
        items: vec![TimelineItem {
            id: item_id,
            kind: TimelineItemKind::AudioClip,
            start_seconds,
            duration_seconds,
            source: TimelineSource::Generated {
                artifact_id: asset_id.to_string(),
            },
            label: brief
                .name
                .clone()
                .unwrap_or_else(|| "Generated audio".to_string()),
            properties: BTreeMap::from([
                ("pendingGeneratedAssetId".to_string(), json!(asset_id)),
                ("generatedPlaceholder".to_string(), json!(true)),
            ]),
        }],
    });
    actions
}

fn rerun_generated_asset_payload(
    project: &VideoProject,
    args: RerunGeneratedAssetArgs,
) -> Result<Value, CodexLocalToolError> {
    let source_asset_id = args.source_asset_id.trim();
    if source_asset_id.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "sourceAssetId must not be blank".to_string(),
        ));
    }
    let source_asset = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == source_asset_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "generated asset was not found: {source_asset_id}"
            ))
        })?;
    let prompt = args
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
        .unwrap_or_else(|| source_asset.prompt.trim());
    if prompt.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "rerun prompt must not be blank".to_string(),
        ));
    }

    let generation_tool =
        direct_generation_tool_for_model(&source_asset.model).ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "generated asset model cannot be rerun directly: {}/{}",
                source_asset.model.provider, source_asset.model.id
            ))
        })?;
    if let Some(replacement_item_id) = trimmed_opt(args.replacement_item_id.as_deref()) {
        timeline_item_with_track(project, replacement_item_id)?;
    }

    let project_dir = rerun_generated_project_dir(project, &args)?;
    let asset_id = rerun_generated_asset_id(project, source_asset, &args);
    let job_id = rerun_generated_job_id(&args, &asset_id);
    let placement_intent = trimmed_opt(args.replacement_item_id.as_deref())
        .map(generated_replacement_placement_intent)
        .or_else(|| trimmed_opt(args.placement_intent.as_deref()).map(str::to_string))
        .or_else(|| source_asset.placement_intent.clone())
        .unwrap_or_else(|| "library".to_string());
    let name = trimmed_opt(args.name.as_deref())
        .map(str::to_string)
        .or_else(|| source_asset.name.clone());
    let target_folder_id = trimmed_opt(args.target_folder_id.as_deref())
        .map(str::to_string)
        .or_else(|| source_asset.target_folder_id.clone());
    let mut references = generated_asset_action_references(source_asset);
    normalize_rerun_generation_references(
        project,
        &source_asset.model,
        generation_tool,
        &mut references,
    );
    let settings = generated_asset_action_settings(source_asset);
    let mut brief = TemporalGenerateMediaBrief {
        name: name.clone(),
        target_folder_id: target_folder_id.clone(),
        placement_intent: Some(placement_intent.clone()),
        prompt: Some(prompt.to_string()),
        model: Some(json!({
            "provider": source_asset.model.provider.clone(),
            "id": source_asset.model.id.clone()
        })),
        references: Some(json!(references)),
        settings: Some(json!(settings)),
    };
    validate_direct_generation_capabilities(project, &brief, generation_tool)?;
    normalize_direct_generation_settings(&mut brief, generation_tool)?;
    let start_request = temporal_generate_media_start_request(
        &project.id,
        &project_dir,
        &asset_id,
        &job_id,
        args.mock_mode.unwrap_or(false),
        Some(brief.clone()),
    );
    let model = generation_model_from_brief(&brief, generation_tool)?;
    let references = generation_references_from_brief(&brief)?;
    let settings = generation_settings_from_brief(&brief)?;
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        &job_id,
        JobStatus::Queued,
        &project.updated_at,
    );
    job.start_request = Some(start_request.clone());
    let parent_asset_id = source_asset
        .parent_asset_id
        .clone()
        .unwrap_or_else(|| source_asset.id.clone());

    let project_actions = vec![
        ProjectAction::RecordGeneratedAsset {
            asset: Box::new(ProjectActionGeneratedAsset {
                id: asset_id,
                kind: MediaKind::Generated,
                status: GeneratedAssetStatus::Queued,
                name,
                target_folder_id,
                placement_intent: Some(placement_intent),
                prompt: prompt.to_string(),
                model,
                references,
                settings,
                outputs: Vec::new(),
                created_at: project.updated_at.clone(),
                parent_asset_id: Some(parent_asset_id),
                retry_of_asset_id: Some(source_asset.id.clone()),
            }),
        },
        ProjectAction::RecordJob { job: Box::new(job) },
    ];
    validate_project_actions_payload(project, project_actions.clone())?;

    Ok(json!({
        "startRequest": start_request,
        "projectActionCount": project_actions.len(),
        "projectActions": project_actions,
        "nextRecommendedInspection": "video_creater.generated_assets"
    }))
}

fn rerun_generated_project_dir(
    project: &VideoProject,
    args: &RerunGeneratedAssetArgs,
) -> Result<String, CodexLocalToolError> {
    if let Some(project_dir) = args.project_dir.as_deref() {
        let project_dir = project_dir.trim();
        if project_dir.is_empty() {
            return Err(CodexLocalToolError::InvalidArguments(
                "projectDir must not be blank".to_string(),
            ));
        }
        return Ok(project_dir.to_string());
    }
    Ok(default_agent_project_dir(project))
}

fn rerun_generated_asset_id(
    project: &VideoProject,
    source_asset: &GeneratedAsset,
    args: &RerunGeneratedAssetArgs,
) -> String {
    if let Some(asset_id) = trimmed_opt(args.asset_id.as_deref()) {
        return asset_id.to_string();
    }

    unique_generation_id(project, &format!("{}-rerun", source_asset.id))
}

fn rerun_generated_job_id(args: &RerunGeneratedAssetArgs, asset_id: &str) -> String {
    trimmed_opt(args.job_id.as_deref())
        .map(str::to_string)
        .unwrap_or_else(|| format!("job-{asset_id}"))
}

fn generated_replacement_placement_intent(item_id: &str) -> String {
    format!("replace:{item_id}")
}

fn direct_generation_tool_for_model(
    model: &crate::project::model::GenerationModel,
) -> Option<DirectGenerationTool> {
    match (model.provider.as_str(), model.id.as_str()) {
        (FAL_PROVIDER, FAL_WAN_TEXT_TO_VIDEO_MODEL_ID)
        | (FAL_PROVIDER, FAL_WAN_IMAGE_TO_VIDEO_MODEL_ID)
        | (FAL_PROVIDER, FAL_WAN_REFERENCE_TO_VIDEO_MODEL_ID)
        | (FAL_PROVIDER, FAL_WAN_VIDEO_TO_VIDEO_MODEL_ID)
        | (FAL_PROVIDER, FAL_KLING_V3_STANDARD_TEXT_TO_VIDEO_MODEL_ID)
        | (FAL_PROVIDER, FAL_KLING_V3_PRO_IMAGE_TO_VIDEO_MODEL_ID)
        | (FAL_PROVIDER, FAL_KLING_V3_PRO_MOTION_CONTROL_MODEL_ID)
        | (FAL_PROVIDER, FAL_VIDEO_UPSCALER_MODEL_ID)
        | (GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_MODEL_ID)
        | (XAI_PROVIDER, XAI_GROK_VIDEO_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID) => {
            Some(DirectGenerationTool::Video)
        }
        (FAL_PROVIDER, FAL_FLUX_SCHNELL_MODEL_ID)
        | (FAL_PROVIDER, FAL_KREA_2_TURBO_MODEL_ID)
        | (FAL_PROVIDER, FAL_RECRAFT_V3_TEXT_TO_IMAGE_MODEL_ID)
        | (FAL_PROVIDER, FAL_AURA_SR_MODEL_ID)
        | (FAL_PROVIDER, FAL_NANO_BANANA_PRO_EDIT_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_FLUX_SCHNELL_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_FLUX_DEV_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_FLUX_11_PRO_MODEL_ID)
        | (REPLICATE_PROVIDER, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID)
        | (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID)
        | (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_EDIT_MODEL_ID)
        | (XAI_PROVIDER, XAI_GROK_IMAGE_QUALITY_MODEL_ID) => Some(DirectGenerationTool::Image),
        (FAL_PROVIDER, FAL_SEED_AUDIO_MODEL_ID)
        | (FAL_PROVIDER, FAL_SONILO_TEXT_TO_MUSIC_MODEL_ID)
        | (FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID)
        | (FAL_PROVIDER, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID)
        | (OPENAI_PROVIDER, OPENAI_GPT_4O_MINI_TTS_MODEL_ID)
        | (GOOGLE_PROVIDER, GOOGLE_GEMINI_TTS_MODEL_ID)
        | (GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID)
        | (MINIMAX_PROVIDER, MINIMAX_MUSIC_MODEL_ID)
        | (ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID)
        | (ELEVENLABS_PROVIDER, ELEVENLABS_MUSIC_MODEL_ID) => Some(DirectGenerationTool::Audio),
        ("mock", _) => Some(DirectGenerationTool::Video),
        _ => None,
    }
}

fn generated_asset_action_references(
    asset: &GeneratedAsset,
) -> ProjectActionGeneratedAssetReferences {
    ProjectActionGeneratedAssetReferences {
        media_ids: asset.references.media_ids.clone(),
        source_video_media_ref: asset.references.source_video_media_ref.clone(),
        first_frame_media_id: asset.references.first_frame_media_id.clone(),
        last_frame_media_id: asset.references.last_frame_media_id.clone(),
        reference_image_media_refs: asset.references.reference_image_media_refs.clone(),
        reference_video_media_refs: asset.references.reference_video_media_refs.clone(),
        reference_audio_media_refs: asset.references.reference_audio_media_refs.clone(),
        provider_input_urls: asset.references.provider_input_urls.clone(),
    }
}

fn normalize_rerun_generation_references(
    project: &VideoProject,
    model: &crate::project::model::GenerationModel,
    generation_tool: DirectGenerationTool,
    references: &mut ProjectActionGeneratedAssetReferences,
) {
    if !matches!(generation_tool, DirectGenerationTool::Video)
        || references.source_video_media_ref.is_some()
    {
        return;
    }
    let Some(capabilities) = video_generation_capabilities(&model.provider, &model.id) else {
        return;
    };
    if !capabilities.requires_source_video {
        return;
    }
    if let Some(media_id) = references.media_ids.iter().find(|media_id| {
        project
            .media
            .iter()
            .any(|media| media.id == **media_id && matches!(media.kind, MediaKind::Video))
    }) {
        references.source_video_media_ref = Some(media_id.clone());
    }
}

fn generated_asset_action_settings(asset: &GeneratedAsset) -> ProjectActionGeneratedAssetSettings {
    ProjectActionGeneratedAssetSettings {
        width: asset.settings.width,
        height: asset.settings.height,
        duration_seconds: asset.settings.duration_seconds,
        fps: asset.settings.fps,
        aspect_ratio: asset.settings.aspect_ratio.clone(),
        resolution: asset.settings.resolution.clone(),
        num_images: asset.settings.num_images,
        quality: asset.settings.quality.clone(),
        generate_audio: asset.settings.generate_audio,
        category: asset.settings.category.clone(),
        voice: asset.settings.voice.clone(),
        lyrics: asset.settings.lyrics.clone(),
        style_instructions: asset.settings.style_instructions.clone(),
        instrumental: asset.settings.instrumental,
        video_source_start_frame: asset.settings.video_source_start_frame,
        video_source_end_frame: asset.settings.video_source_end_frame,
        video_source_start_seconds: asset.settings.video_source_start_seconds,
        video_source_end_seconds: asset.settings.video_source_end_seconds,
        timeline_start_seconds: asset.settings.timeline_start_seconds,
    }
}

fn trimmed_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn upscale_media_payload(
    project: &VideoProject,
    args: UpscaleMediaArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = upscale_project_dir(project, &args)?;
    let media = project
        .media
        .iter()
        .find(|media| media.id == args.media_id)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!("media was not found: {}", args.media_id))
        })?;
    if generated_output_is_upscale_result(project, &media.id) {
        return Err(CodexLocalToolError::InvalidArguments(
            "Already upscaled".to_string(),
        ));
    }
    let effective_media_kind = upscale_effective_media_kind(media).ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!(
            "upscale_media supports video and image assets only: {} is {:?}",
            media.id, media.kind
        ))
    })?;
    if !matches!(effective_media_kind, MediaKind::Video | MediaKind::Image) {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "upscale_media supports video and image assets only: {} is {:?}",
            media.id, media.kind
        )));
    }
    if matches!(effective_media_kind, MediaKind::Video) && media.height.unwrap_or(0) >= 2160 {
        return Err(CodexLocalToolError::InvalidArguments(
            "Already 4K or higher".to_string(),
        ));
    }

    let (source_clip_id, source_in, source_out) = upscale_source_range(
        project,
        media,
        &effective_media_kind,
        args.source_clip_id.as_deref(),
    )?;
    let duration_seconds = if matches!(effective_media_kind, MediaKind::Video) {
        Some((source_out - source_in).max(0.0))
    } else {
        None
    };
    let display_name = media_display_name(media);
    let asset_name = args
        .name
        .clone()
        .unwrap_or_else(|| format!("Upscaled {display_name}"));
    if let Some(replacement_item_id) = trimmed_opt(args.replacement_item_id.as_deref()) {
        timeline_item_with_track(project, replacement_item_id)?;
    }
    let placement_intent = trimmed_opt(args.replacement_item_id.as_deref())
        .map(generated_replacement_placement_intent)
        .or_else(|| trimmed_opt(args.placement_intent.as_deref()).map(str::to_string))
        .unwrap_or_else(|| "library".to_string());
    let model_arg = args
        .model
        .clone()
        .unwrap_or_else(|| "mock-upscale-v1".to_string());
    let (model_provider, model_id) = upscale_provider_and_model_from_string(&model_arg);
    validate_upscale_model_media_kind(model_provider, &model_id, &effective_media_kind)?;
    let provider_input_urls = upscale_provider_input_urls(args.provider_input_url.as_deref());
    let asset_id = upscale_asset_id(project, &args, &display_name);
    let job_id = upscale_job_id(&args, &asset_id);
    let target_folder_id = trimmed_opt(args.target_folder_id.as_deref())
        .map(str::to_string)
        .or_else(|| media.folder_id.clone());
    let source_video_media_ref = if matches!(effective_media_kind, MediaKind::Video) {
        Some(media.id.clone())
    } else {
        None
    };
    let reference_image_media_refs = if matches!(effective_media_kind, MediaKind::Image) {
        vec![media.id.clone()]
    } else {
        Vec::new()
    };
    let reference_video_media_refs = if matches!(effective_media_kind, MediaKind::Video) {
        vec![media.id.clone()]
    } else {
        Vec::new()
    };
    let references = json!({
        "mediaIds": [media.id],
        "sourceVideoMediaRef": source_video_media_ref,
        "firstFrameMediaId": Value::Null,
        "lastFrameMediaId": Value::Null,
        "referenceImageMediaRefs": reference_image_media_refs,
        "referenceVideoMediaRefs": reference_video_media_refs,
        "providerInputUrls": provider_input_urls
    });
    let mut workflow_settings = json!({
        "operation": "upscale",
        "mediaId": media.id,
        "sourceIn": source_in,
        "sourceOut": source_out,
        "durationSeconds": duration_seconds,
        "width": media.width,
        "height": media.height,
        "fps": media.fps,
        "aspectRatio": media
            .width
            .zip(media.height)
            .map(|(width, height)| aspect_ratio_label(width, height))
    });
    if let Some(source_clip_id) = &source_clip_id {
        workflow_settings
            .as_object_mut()
            .expect("upscale settings should be object")
            .insert("sourceClipId".to_string(), json!(source_clip_id));
    }

    let brief = TemporalGenerateMediaBrief {
        name: Some(asset_name.clone()),
        target_folder_id: target_folder_id.clone(),
        placement_intent: Some(placement_intent.clone()),
        prompt: Some(format!("Upscale {display_name}")),
        model: Some(json!({
            "provider": model_provider,
            "id": model_id
        })),
        references: Some(references.clone()),
        settings: Some(workflow_settings),
    };
    let start_request = temporal_generate_media_start_request(
        &project.id,
        &project_dir,
        &asset_id,
        &job_id,
        args.mock_mode.unwrap_or(false),
        Some(brief),
    );
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        &project.id,
        &job_id,
        JobStatus::Queued,
        &project.updated_at,
    );
    job.start_request = Some(start_request.clone());

    let action_settings = ProjectActionGeneratedAssetSettings {
        width: media.width.map(|width| width.saturating_mul(2)),
        height: media.height.map(|height| height.saturating_mul(2)),
        duration_seconds,
        fps: media.fps,
        aspect_ratio: media
            .width
            .zip(media.height)
            .map(|(width, height)| aspect_ratio_label(width, height)),
        resolution: None,
        num_images: None,
        quality: None,
        generate_audio: None,
        category: None,
        voice: None,
        lyrics: None,
        style_instructions: None,
        instrumental: None,
        video_source_start_frame: None,
        video_source_end_frame: None,
        video_source_start_seconds: if matches!(effective_media_kind, MediaKind::Video) {
            Some(source_in)
        } else {
            None
        },
        video_source_end_seconds: if matches!(effective_media_kind, MediaKind::Video) {
            Some(source_out)
        } else {
            None
        },
        timeline_start_seconds: None,
    };
    let project_actions = vec![
        ProjectAction::RecordGeneratedAsset {
            asset: Box::new(ProjectActionGeneratedAsset {
                id: asset_id,
                kind: MediaKind::Generated,
                status: GeneratedAssetStatus::Queued,
                name: Some(asset_name),
                target_folder_id,
                placement_intent: Some(placement_intent),
                prompt: format!("Upscale {display_name}"),
                model: ProjectActionGenerationModel {
                    provider: model_provider.to_string(),
                    id: model_id,
                },
                references: ProjectActionGeneratedAssetReferences {
                    media_ids: vec![media.id.clone()],
                    source_video_media_ref,
                    first_frame_media_id: None,
                    last_frame_media_id: None,
                    reference_image_media_refs,
                    reference_video_media_refs,
                    reference_audio_media_refs: Vec::new(),
                    provider_input_urls,
                },
                settings: action_settings,
                outputs: Vec::new(),
                created_at: project.updated_at.clone(),
                parent_asset_id: None,
                retry_of_asset_id: None,
            }),
        },
        ProjectAction::RecordJob { job: Box::new(job) },
    ];
    validate_project_actions_payload(project, project_actions.clone())?;

    let mut payload = json!({
        "startRequest": start_request,
        "projectActionCount": project_actions.len(),
        "projectActions": project_actions,
        "nextRecommendedInspection": "video_creater.generated_assets"
    });
    attach_generation_started_summary(&mut payload);
    Ok(payload)
}

fn upscale_project_dir(
    project: &VideoProject,
    args: &UpscaleMediaArgs,
) -> Result<String, CodexLocalToolError> {
    if let Some(project_dir) = args.project_dir.as_deref() {
        let project_dir = project_dir.trim();
        if project_dir.is_empty() {
            return Err(CodexLocalToolError::InvalidArguments(
                "projectDir must not be blank".to_string(),
            ));
        }
        return Ok(project_dir.to_string());
    }
    Ok(default_agent_project_dir(project))
}

fn upscale_asset_id(project: &VideoProject, args: &UpscaleMediaArgs, display_name: &str) -> String {
    if let Some(asset_id) = args
        .asset_id
        .as_deref()
        .map(str::trim)
        .filter(|asset_id| !asset_id.is_empty())
    {
        return asset_id.to_string();
    }

    let slug = media_folder_id_slug(display_name);
    let base = if slug.is_empty() {
        "agent-upscale".to_string()
    } else {
        format!("agent-upscale-{slug}")
    };
    unique_generation_id(project, &base)
}

fn upscale_job_id(args: &UpscaleMediaArgs, asset_id: &str) -> String {
    args.job_id
        .as_deref()
        .map(str::trim)
        .filter(|job_id| !job_id.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("job-{asset_id}"))
}

fn upscale_provider_and_model_from_string(model: &str) -> (&'static str, String) {
    if let Some((provider, id)) = provider_qualified_model_string(model) {
        if provider == FAL_PROVIDER {
            if let Some(id) = palmier_upscale_model_alias(&id) {
                return (FAL_PROVIDER, id.to_string());
            }
        }
        if provider == FAL_PROVIDER && is_fal_upscale_model(&id) {
            return (FAL_PROVIDER, id);
        }
        if provider == "mock" {
            return ("mock", id);
        }
    }

    if let Some(id) = palmier_upscale_model_alias(model) {
        return (FAL_PROVIDER, id.to_string());
    }

    (upscale_model_provider(model), model.to_string())
}

fn palmier_upscale_model_alias(model: &str) -> Option<&'static str> {
    match model {
        "bytedance-upscaler" => Some(FAL_VIDEO_UPSCALER_MODEL_ID),
        "seedvr-image-upscaler" => Some(FAL_AURA_SR_MODEL_ID),
        _ => None,
    }
}

fn upscale_model_provider(model_id: &str) -> &'static str {
    if is_fal_upscale_model(model_id) {
        FAL_PROVIDER
    } else {
        "mock"
    }
}

fn is_fal_upscale_model(model_id: &str) -> bool {
    matches!(model_id, FAL_AURA_SR_MODEL_ID | FAL_VIDEO_UPSCALER_MODEL_ID)
}

fn generated_output_is_upscale_result(project: &VideoProject, media_id: &str) -> bool {
    project.generated_assets.iter().any(|asset| {
        asset
            .outputs
            .iter()
            .any(|output| output.media_id == media_id)
            && is_upscale_model(&asset.model.provider, &asset.model.id)
    })
}

fn is_upscale_model(provider: &str, model_id: &str) -> bool {
    (provider == FAL_PROVIDER && is_fal_upscale_model(model_id)) || model_id == "mock-upscale-v1"
}

fn upscale_effective_media_kind(media: &MediaAsset) -> Option<MediaKind> {
    match media.kind {
        MediaKind::Video | MediaKind::Image => Some(media.kind.clone()),
        MediaKind::Generated => {
            let has_visual_dimensions =
                media.width.unwrap_or(0) > 0 && media.height.unwrap_or(0) > 0;
            if has_visual_dimensions
                && (media.duration_seconds > 0.0 || media.fps.unwrap_or(0.0) > 0.0)
            {
                Some(MediaKind::Video)
            } else if has_visual_dimensions {
                Some(MediaKind::Image)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn validate_upscale_model_media_kind(
    provider: &str,
    model_id: &str,
    media_kind: &MediaKind,
) -> Result<(), CodexLocalToolError> {
    if provider == FAL_PROVIDER
        && model_id == FAL_AURA_SR_MODEL_ID
        && !matches!(media_kind, MediaKind::Image)
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{model_id} supports image assets only"
        )));
    }
    if provider == FAL_PROVIDER
        && model_id == FAL_VIDEO_UPSCALER_MODEL_ID
        && !matches!(media_kind, MediaKind::Video)
    {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "{model_id} supports video assets only"
        )));
    }
    Ok(())
}

fn upscale_provider_input_urls(provider_input_url: Option<&str>) -> Vec<String> {
    provider_input_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(|url| vec![url.to_string()])
        .unwrap_or_default()
}

fn upscale_source_range(
    project: &VideoProject,
    media: &MediaAsset,
    effective_media_kind: &MediaKind,
    source_clip_id: Option<&str>,
) -> Result<(Option<String>, f64, f64), CodexLocalToolError> {
    let Some(source_clip_id) = source_clip_id else {
        return Ok((None, 0.0, media.duration_seconds));
    };
    if !matches!(effective_media_kind, MediaKind::Video) {
        return Err(CodexLocalToolError::InvalidArguments(
            "sourceClipId only applies to video sources".to_string(),
        ));
    }
    let Some(item) = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == source_clip_id)
    else {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "sourceClipId was not found: {source_clip_id}"
        )));
    };
    let TimelineSource::Media { media_id } = &item.source else {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "sourceClipId {source_clip_id} does not reference media"
        )));
    };
    if media_id != &media.id {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "sourceClipId {source_clip_id} references {media_id}, not {}",
            media.id
        )));
    }
    let source_in = timeline_item_number_property(item, "sourceIn").unwrap_or(0.0);
    let source_out = timeline_item_number_property(item, "sourceOut")
        .unwrap_or(source_in + item.duration_seconds * timeline_item_playback_speed(item))
        .min(media.duration_seconds)
        .max(source_in);
    Ok((Some(source_clip_id.to_string()), source_in, source_out))
}

fn media_display_name(media: &MediaAsset) -> String {
    media.name.clone().unwrap_or_else(|| {
        Path::new(&media.relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&media.id)
            .to_string()
    })
}

fn generation_model_from_brief(
    brief: &TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<ProjectActionGenerationModel, CodexLocalToolError> {
    if let Some(model) = &brief.model {
        let provider = model
            .get("provider")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CodexLocalToolError::InvalidArguments(
                    "generation model.provider must be a string".to_string(),
                )
            })?;
        let id = model.get("id").and_then(Value::as_str).ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(
                "generation model.id must be a string".to_string(),
            )
        })?;
        return Ok(ProjectActionGenerationModel {
            provider: provider.to_string(),
            id: id.to_string(),
        });
    }

    let (provider, id) = default_generation_model(generation_tool);

    Ok(ProjectActionGenerationModel {
        provider: provider.to_string(),
        id: id.to_string(),
    })
}

fn default_generation_model(generation_tool: DirectGenerationTool) -> (&'static str, &'static str) {
    match generation_tool {
        DirectGenerationTool::Video => (REPLICATE_PROVIDER, REPLICATE_SEEDANCE_20_FAST_MODEL_ID),
        DirectGenerationTool::Image => (OPENAI_PROVIDER, OPENAI_GPT_IMAGE_2_MODEL_ID),
        DirectGenerationTool::Audio => (FAL_PROVIDER, FAL_SEED_AUDIO_MODEL_ID),
    }
}

fn generation_references_from_brief(
    brief: &TemporalGenerateMediaBrief,
) -> Result<ProjectActionGeneratedAssetReferences, CodexLocalToolError> {
    let Some(references) = &brief.references else {
        return Ok(ProjectActionGeneratedAssetReferences {
            media_ids: Vec::new(),
            source_video_media_ref: None,
            first_frame_media_id: None,
            last_frame_media_id: None,
            reference_image_media_refs: Vec::new(),
            reference_video_media_refs: Vec::new(),
            reference_audio_media_refs: Vec::new(),
            provider_input_urls: Vec::new(),
        });
    };
    serde_json::from_value(references.clone()).map_err(|error| {
        CodexLocalToolError::InvalidArguments(format!("invalid generation references: {error}"))
    })
}

fn generation_settings_from_brief(
    brief: &TemporalGenerateMediaBrief,
) -> Result<ProjectActionGeneratedAssetSettings, CodexLocalToolError> {
    let Some(settings) = &brief.settings else {
        return Ok(ProjectActionGeneratedAssetSettings::default());
    };
    serde_json::from_value(settings.clone()).map_err(|error| {
        CodexLocalToolError::InvalidArguments(format!("invalid generation settings: {error}"))
    })
}

fn build_audio_category_start_request_payload(
    project: &VideoProject,
    args: BuildGenerateMediaStartRequestArgs,
    category: &str,
) -> Result<Value, CodexLocalToolError> {
    let mut brief = generate_media_brief_from_args(&args).unwrap_or(TemporalGenerateMediaBrief {
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: None,
        model: None,
        references: None,
        settings: None,
    });
    if brief.model.is_none() {
        let has_source_video = generation_brief_has_source_video(&brief);
        let default_model = match (category, has_source_video) {
            ("music", true) => Some((FAL_PROVIDER, FAL_SONILO_VIDEO_TO_MUSIC_MODEL_ID)),
            ("music", false) => Some((GOOGLE_PROVIDER, GOOGLE_LYRIA_3_PRO_MODEL_ID)),
            ("sfx", _) => Some((FAL_PROVIDER, FAL_MIRELO_VIDEO_TO_AUDIO_MODEL_ID)),
            _ => None,
        };
        if let Some((provider, model_id)) = default_model {
            brief.model = Some(json!({
                "provider": provider,
                "id": model_id
            }));
        }
    }
    let mut settings = brief
        .settings
        .take()
        .filter(|settings| settings.is_object())
        .unwrap_or_else(|| json!({}));
    if let Some(object) = settings.as_object_mut() {
        object
            .entry("category".to_string())
            .or_insert_with(|| json!(category));
    }
    brief.settings = Some(settings);
    normalize_audio_source_video_reference(&mut brief);
    default_audio_source_video_duration(project, &mut brief)?;
    default_generation_model_arg(&mut brief, DirectGenerationTool::Audio);
    validate_direct_generation_capabilities(project, &brief, DirectGenerationTool::Audio)?;
    normalize_direct_generation_settings(&mut brief, DirectGenerationTool::Audio)?;

    let project_dir = generation_project_dir(project, &args);
    let asset_id = generation_asset_id(
        project,
        &args,
        Some(DirectGenerationTool::Audio),
        Some(&brief),
    );
    let job_id = generation_job_id(&args, &asset_id);
    let start_request = temporal_generate_media_start_request(
        &project.id,
        &project_dir,
        &asset_id,
        &job_id,
        args.mock_mode.unwrap_or(false),
        Some(brief.clone()),
    );
    let project_actions = generation_record_actions(
        project,
        &asset_id,
        &job_id,
        &start_request,
        DirectGenerationTool::Audio,
        &brief,
    )?;

    let mut payload = json!({
        "startRequest": start_request,
        "projectActionCount": project_actions.len(),
        "projectActions": project_actions,
        "nextRecommendedInspection": "video_creater.generated_assets"
    });
    attach_generation_started_summary(&mut payload);
    Ok(payload)
}

fn normalize_audio_source_video_reference(brief: &mut TemporalGenerateMediaBrief) {
    let Some(references) = brief.references.as_mut().and_then(Value::as_object_mut) else {
        return;
    };
    let Some(source_video_media_ref) = references
        .get("sourceVideoMediaRef")
        .and_then(Value::as_str)
        .and_then(|value| trimmed_optional(Some(value)))
    else {
        return;
    };
    append_unique_reference_strings(
        references,
        "referenceVideoMediaRefs",
        std::iter::once(source_video_media_ref.as_str()),
    );
}

fn generation_brief_has_source_video(brief: &TemporalGenerateMediaBrief) -> bool {
    brief
        .references
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|references| references.get("sourceVideoMediaRef"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn default_audio_source_video_duration(
    project: &VideoProject,
    brief: &mut TemporalGenerateMediaBrief,
) -> Result<(), CodexLocalToolError> {
    let references = generation_references_from_brief(brief)?;
    if references.source_video_media_ref.is_none() {
        return Ok(());
    }
    let mut settings = generation_settings_from_brief(brief)?;
    if settings
        .duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .is_some()
    {
        return Ok(());
    }
    let Some(duration_seconds) =
        source_video_input_duration_seconds(project, &settings, &references)
    else {
        return Ok(());
    };
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Ok(());
    }
    settings.duration_seconds = Some(round_tool_seconds(duration_seconds));
    brief.settings = Some(json!(settings));
    Ok(())
}

fn generation_brief_has_first_frame_input(brief: &TemporalGenerateMediaBrief) -> bool {
    let Some(references) = brief.references.as_ref().and_then(Value::as_object) else {
        return false;
    };
    references
        .get("firstFrameMediaId")
        .or_else(|| references.get("startFrameMediaRef"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn generation_brief_has_video_source_input(brief: &TemporalGenerateMediaBrief) -> bool {
    if generation_brief_has_source_video(brief) {
        return true;
    }

    let Some(settings) = brief.settings.as_ref().and_then(Value::as_object) else {
        return false;
    };
    settings.contains_key("videoSourceStartFrame") || settings.contains_key("videoSourceEndFrame")
}

fn generate_media_brief_from_args(
    args: &BuildGenerateMediaStartRequestArgs,
) -> Option<TemporalGenerateMediaBrief> {
    let mut brief = args.brief.clone().unwrap_or(TemporalGenerateMediaBrief {
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: None,
        model: None,
        references: None,
        settings: None,
    });

    if args.name.is_some() {
        brief.name = args.name.clone();
    }
    if args.target_folder_id.is_some() {
        brief.target_folder_id = args.target_folder_id.clone();
    }
    if args.placement_intent.is_some() {
        brief.placement_intent = args.placement_intent.clone();
    }
    if args.prompt.is_some() {
        brief.prompt = args.prompt.clone();
    }
    if args.model.is_some() {
        brief.model = args.model.clone();
    }
    if args.references.is_some() {
        brief.references = args.references.clone();
    }
    if args.settings.is_some() {
        brief.settings = args.settings.clone();
    }
    apply_generation_reference_args(args, &mut brief);
    apply_generation_settings_args(args, &mut brief);

    if brief.name.is_none()
        && brief.target_folder_id.is_none()
        && brief.placement_intent.is_none()
        && brief.prompt.is_none()
        && brief.model.is_none()
        && brief.references.is_none()
        && brief.settings.is_none()
    {
        None
    } else {
        Some(brief)
    }
}

fn apply_generation_reference_args(
    args: &BuildGenerateMediaStartRequestArgs,
    brief: &mut TemporalGenerateMediaBrief,
) {
    let mut references = brief
        .references
        .take()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| json!({}));
    let object = references
        .as_object_mut()
        .expect("generation references should be an object");
    let mut media_ids = object
        .get("mediaIds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut touched = false;

    if let Some(media_id) = trimmed_optional(args.source_video_media_id.as_deref()) {
        push_unique_json_string(&mut media_ids, &media_id);
        object.insert("sourceVideoMediaRef".to_string(), json!(media_id));
        touched = true;
    }
    if let Some(media_id) = trimmed_optional(args.start_frame_media_ref.as_deref()) {
        push_unique_json_string(&mut media_ids, &media_id);
        object.insert("firstFrameMediaId".to_string(), json!(media_id));
        object.insert("startFrameMediaRef".to_string(), json!(media_id));
        touched = true;
    }
    if let Some(media_id) = trimmed_optional(args.end_frame_media_ref.as_deref()) {
        push_unique_json_string(&mut media_ids, &media_id);
        object.insert("lastFrameMediaId".to_string(), json!(media_id));
        object.insert("endFrameMediaRef".to_string(), json!(media_id));
        touched = true;
    }

    touched |= append_reference_array(
        object,
        &mut media_ids,
        "referenceMediaRefs",
        &args.reference_media_refs,
    );
    touched |= append_reference_array(
        object,
        &mut media_ids,
        "referenceImageMediaRefs",
        &args.reference_image_media_refs,
    );
    touched |= append_reference_array(
        object,
        &mut media_ids,
        "referenceVideoMediaRefs",
        &args.reference_video_media_refs,
    );
    touched |= append_reference_array(
        object,
        &mut media_ids,
        "referenceAudioMediaRefs",
        &args.reference_audio_media_refs,
    );

    if touched {
        object.insert("mediaIds".to_string(), json!(media_ids));
        brief.references = Some(references);
    } else if !object.is_empty() {
        brief.references = Some(references);
    }
}

fn append_reference_array(
    object: &mut serde_json::Map<String, Value>,
    media_ids: &mut Vec<Value>,
    key: &str,
    values: &[String],
) -> bool {
    let values = values
        .iter()
        .filter_map(|value| trimmed_optional(Some(value.as_str())))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return false;
    }
    for value in &values {
        push_unique_json_string(media_ids, value);
    }
    object.insert(key.to_string(), json!(values));
    true
}

fn apply_palmier_reference_media_refs(
    project: &VideoProject,
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    if !matches!(
        generation_tool,
        DirectGenerationTool::Audio | DirectGenerationTool::Image | DirectGenerationTool::Video
    ) {
        return Ok(());
    }
    let Some(references) = brief.references.as_mut().and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let reference_media_refs = references
        .get("referenceMediaRefs")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| trimmed_optional(Some(value)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if reference_media_refs.is_empty() {
        return Ok(());
    }

    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media.kind.clone()))
        .collect::<BTreeMap<_, _>>();
    for media_id in &reference_media_refs {
        if !media_by_id.contains_key(media_id.as_str()) {
            return Err(CodexLocalToolError::InvalidArguments(format!(
                "referenceMediaRefs entry '{media_id}' references missing media"
            )));
        }
    }

    if matches!(generation_tool, DirectGenerationTool::Image) {
        for media_id in &reference_media_refs {
            if let Some(kind) = media_by_id.get(media_id.as_str()) {
                if !matches!(kind, MediaKind::Image) {
                    return Err(CodexLocalToolError::InvalidArguments(format!(
                        "referenceMediaRefs entry '{media_id}' must be an image asset"
                    )));
                }
            }
        }

        append_unique_reference_strings(
            references,
            "referenceImageMediaRefs",
            reference_media_refs.iter().map(String::as_str),
        );
        return Ok(());
    }

    let mut image_refs = Vec::new();
    let mut video_refs = Vec::new();
    let mut audio_refs = Vec::new();
    for media_id in &reference_media_refs {
        match media_by_id.get(media_id.as_str()) {
            Some(MediaKind::Audio) => audio_refs.push(media_id.as_str()),
            Some(MediaKind::Video) => video_refs.push(media_id.as_str()),
            _ => image_refs.push(media_id.as_str()),
        }
    }
    if matches!(generation_tool, DirectGenerationTool::Audio) {
        if !image_refs.is_empty() || !audio_refs.is_empty() {
            return Err(CodexLocalToolError::InvalidArguments(
                "referenceMediaRefs entries for audio generation must reference video assets"
                    .to_string(),
            ));
        }
        if let Some(source_video_media_ref) = video_refs.first().copied() {
            references
                .entry("sourceVideoMediaRef".to_string())
                .or_insert_with(|| json!(source_video_media_ref));
        }
        append_unique_reference_strings(references, "referenceVideoMediaRefs", video_refs);
        return Ok(());
    }
    append_unique_reference_strings(references, "referenceImageMediaRefs", image_refs);
    append_unique_reference_strings(references, "referenceVideoMediaRefs", video_refs);
    append_unique_reference_strings(references, "referenceAudioMediaRefs", audio_refs);
    Ok(())
}

fn append_unique_reference_strings<'a>(
    references: &mut serde_json::Map<String, Value>,
    key: &str,
    values: impl IntoIterator<Item = &'a str>,
) {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return;
    }
    let mut existing_values = references
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for value in values {
        push_unique_json_string(&mut existing_values, value);
    }
    references.insert(key.to_string(), json!(existing_values));
}

fn apply_generation_settings_args(
    args: &BuildGenerateMediaStartRequestArgs,
    brief: &mut TemporalGenerateMediaBrief,
) {
    let mut settings = brief
        .settings
        .take()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| json!({}));
    let object = settings
        .as_object_mut()
        .expect("generation settings should be an object");
    let mut touched = false;

    if let Some(duration) = args
        .duration
        .filter(|duration| duration.is_finite() && *duration > 0.0)
    {
        object.insert("durationSeconds".to_string(), json!(duration));
        touched = true;
    }
    touched |= insert_trimmed_setting(object, "aspectRatio", args.aspect_ratio.as_deref());
    touched |= insert_trimmed_setting(object, "resolution", args.resolution.as_deref());
    if let Some(num_images) = args.num_images {
        object.insert("numImages".to_string(), json!(num_images));
        touched = true;
    }
    if let Some(generate_audio) = args.generate_audio {
        object.insert("generateAudio".to_string(), json!(generate_audio));
        touched = true;
    }
    touched |= insert_trimmed_setting(object, "quality", args.quality.as_deref());
    touched |= insert_trimmed_setting(object, "voice", args.voice.as_deref());
    touched |= insert_trimmed_setting(object, "lyrics", args.lyrics.as_deref());
    touched |= insert_trimmed_setting(
        object,
        "styleInstructions",
        args.style_instructions.as_deref(),
    );
    if let Some(instrumental) = args.instrumental {
        object.insert("instrumental".to_string(), json!(instrumental));
        touched = true;
    }
    if let Some(start_frame) = args.video_source_start_frame {
        object.insert("videoSourceStartFrame".to_string(), json!(start_frame));
        touched = true;
    }
    if let Some(end_frame) = args.video_source_end_frame {
        object.insert("videoSourceEndFrame".to_string(), json!(end_frame));
        touched = true;
    }

    if touched || !object.is_empty() {
        brief.settings = Some(settings);
    }
}

fn apply_generation_video_source_span(
    project: &VideoProject,
    args: &BuildGenerateMediaStartRequestArgs,
    brief: &mut TemporalGenerateMediaBrief,
    generation_tool: DirectGenerationTool,
) -> Result<(), CodexLocalToolError> {
    let has_span = args.video_source_start_frame.is_some() || args.video_source_end_frame.is_some();
    if !has_span {
        return Ok(());
    }
    let (Some(start_frame), Some(end_frame)) =
        (args.video_source_start_frame, args.video_source_end_frame)
    else {
        return Err(CodexLocalToolError::InvalidArguments(
            "videoSourceStartFrame and videoSourceEndFrame must be passed together".to_string(),
        ));
    };
    if end_frame <= start_frame {
        return Err(CodexLocalToolError::InvalidArguments(
            "videoSourceEndFrame must be greater than videoSourceStartFrame".to_string(),
        ));
    }
    if matches!(generation_tool, DirectGenerationTool::Audio)
        && trimmed_optional(args.source_video_media_id.as_deref()).is_some()
    {
        return Err(CodexLocalToolError::InvalidArguments(
            "videoSourceMediaRef is mutually exclusive with videoSourceStartFrame and videoSourceEndFrame"
                .to_string(),
        ));
    }

    let fps = project_fps(project)?;
    let start_seconds = frames_to_seconds(start_frame, fps);
    let end_seconds = frames_to_seconds(end_frame, fps);
    let duration_seconds = round_tool_seconds(end_seconds - start_seconds);
    let mut settings = brief
        .settings
        .take()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| json!({}));
    let object = settings
        .as_object_mut()
        .expect("generation settings should be an object");
    object.insert("videoSourceStartFrame".to_string(), json!(start_frame));
    object.insert("videoSourceEndFrame".to_string(), json!(end_frame));
    object.insert(
        "videoSourceStartSeconds".to_string(),
        json!(round_tool_seconds(start_seconds)),
    );
    object.insert(
        "videoSourceEndSeconds".to_string(),
        json!(round_tool_seconds(end_seconds)),
    );
    if matches!(generation_tool, DirectGenerationTool::Audio) {
        object.insert(
            "timelineStartSeconds".to_string(),
            json!(round_tool_seconds(start_seconds)),
        );
    }
    object
        .entry("durationSeconds".to_string())
        .or_insert_with(|| json!(duration_seconds));
    brief.settings = Some(settings);

    if matches!(generation_tool, DirectGenerationTool::Audio) && brief.placement_intent.is_none() {
        brief.placement_intent = Some("timeline".to_string());
    }

    Ok(())
}

fn insert_trimmed_setting(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<&str>,
) -> bool {
    let Some(value) = trimmed_optional(value) else {
        return false;
    };
    object.insert(key.to_string(), json!(value));
    true
}

fn trimmed_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn push_unique_json_string(values: &mut Vec<Value>, value: &str) {
    if !values
        .iter()
        .any(|candidate| candidate.as_str() == Some(value))
    {
        values.push(json!(value));
    }
}

fn build_codex_edit_start_request_payload(
    project: &VideoProject,
    args: BuildCodexEditStartRequestArgs,
) -> Value {
    let start_request = temporal_codex_edit_start_request(
        &project.id,
        &args.project_root,
        &args.project_dir,
        &args.job_id,
        args.request,
    );

    json!({
        "startRequest": start_request
    })
}

fn build_export_media_start_request_payload(
    project: &VideoProject,
    args: BuildExportMediaStartRequestArgs,
) -> Result<Value, CodexLocalToolError> {
    let options = ExportRenderOptions::new(args.profile, args.quality, args.width, args.height)
        .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
    ensure_export_profile_available(args.profile, args.quality)?;
    let start_request = temporal_export_media_start_request_with_options_and_overwrite(
        &project.id,
        &args.project_dir,
        &args.job_id,
        options,
        &args.output_path,
        true,
    );

    Ok(json!({
        "startRequest": start_request
    }))
}

fn export_project_start_request_payload(
    project: &VideoProject,
    args: ExportProjectStartRequestArgs,
) -> Result<Value, CodexLocalToolError> {
    let args = resolve_export_project_args(project, args);
    let profile = resolve_export_project_profile(&args)?;
    let export_project = project_for_export_timeline(project, &args, profile)?;
    validate_export_project_resolution(&args, profile)?;
    let fcpxml_target = export_project_fcpxml_target(&args, profile)?;
    let mut start_request = match profile {
        ExportProjectProfile::DraftWebm => {
            webm_render_start_request(&export_project, &args, RenderQualityProfile::DraftWebm)?
        }
        ExportProjectProfile::FinalWebm => {
            webm_render_start_request(&export_project, &args, RenderQualityProfile::FinalWebm)?
        }
        ExportProjectProfile::Mp4H264 => {
            media_export_start_request(&export_project, &args, ExportProfile::Mp4H264, "mp4")?
        }
        ExportProjectProfile::Mp4H265 => {
            media_export_start_request(&export_project, &args, ExportProfile::Mp4H265, "mp4")?
        }
        ExportProjectProfile::ProResMov => {
            media_export_start_request(&export_project, &args, ExportProfile::ProResMov, "mov")?
        }
        ExportProjectProfile::PremiereXmeml => nle_xml_export_start_request(
            &export_project,
            &args,
            NleXmlFormat::PremiereXmeml,
            "xml",
            None,
        )?,
        ExportProjectProfile::DavinciFcpxml => nle_xml_export_start_request(
            &export_project,
            &args,
            NleXmlFormat::DavinciFcpxml,
            "fcpxml",
            fcpxml_target.as_deref(),
        )?,
        ExportProjectProfile::PalmierProject => {
            palmier_project_export_start_request(&export_project, &args)?
        }
    };
    if let Some(timeline_id) = args
        .timeline_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        start_request.input["timelineId"] = json!(timeline_id);
    }

    let mut payload = json!({
        "startRequest": start_request,
        "outputPolicy": {
            "overwrite": args.overwrite.unwrap_or(true)
        }
    });
    attach_export_project_summary(&mut payload, profile);
    Ok(payload)
}

fn attach_export_project_summary(payload: &mut Value, profile: ExportProjectProfile) {
    let Some(input) = payload
        .get("startRequest")
        .and_then(|start_request| start_request.get("input"))
    else {
        return;
    };
    let output_path = input.get("outputPath").cloned();
    let job_id = input.get("jobId").cloned();
    let profile_value = input.get("profile").cloned();
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object
        .entry("status".to_string())
        .or_insert_with(|| json!(export_project_summary_status(profile)));
    object
        .entry("mode".to_string())
        .or_insert_with(|| json!(export_project_summary_mode(profile)));
    if let Some(output_path) = output_path {
        object.entry("path".to_string()).or_insert(output_path);
    }
    if let Some(job_id) = job_id {
        object.entry("jobId".to_string()).or_insert(job_id);
    }
    if let Some(profile_value) = profile_value {
        object.entry("profile".to_string()).or_insert(profile_value);
    }
}

fn export_project_summary_status(profile: ExportProjectProfile) -> &'static str {
    match profile {
        ExportProjectProfile::PremiereXmeml
        | ExportProjectProfile::DavinciFcpxml
        | ExportProjectProfile::PalmierProject => "exported",
        ExportProjectProfile::DraftWebm
        | ExportProjectProfile::FinalWebm
        | ExportProjectProfile::Mp4H264
        | ExportProjectProfile::Mp4H265
        | ExportProjectProfile::ProResMov => "started",
    }
}

fn export_project_summary_mode(profile: ExportProjectProfile) -> &'static str {
    match profile {
        ExportProjectProfile::PremiereXmeml => "xml",
        ExportProjectProfile::DavinciFcpxml => "fcpxml",
        ExportProjectProfile::PalmierProject => "palmier",
        ExportProjectProfile::DraftWebm
        | ExportProjectProfile::FinalWebm
        | ExportProjectProfile::Mp4H264
        | ExportProjectProfile::Mp4H265
        | ExportProjectProfile::ProResMov => "video",
    }
}

fn resolve_export_project_args(
    project: &VideoProject,
    args: ExportProjectStartRequestArgs,
) -> ResolvedExportProjectStartRequestArgs {
    let project_slug = export_project_slug(project);
    ResolvedExportProjectStartRequestArgs {
        project_dir: args
            .project_dir
            .as_deref()
            .map(str::trim)
            .filter(|project_dir| !project_dir.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| default_agent_project_dir(project)),
        job_id: args
            .job_id
            .as_deref()
            .map(str::trim)
            .filter(|job_id| !job_id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| export_project_job_id(&project_slug, &args)),
        profile: args.profile,
        mode: args.mode,
        codec: args.codec,
        resolution: args.resolution,
        overwrite: args.overwrite,
        output_path: args.output_path,
        timeline_id: args.timeline_id,
        fcpxml_target: args.fcpxml_target,
    }
}

fn export_project_job_id(project_slug: &str, args: &ExportProjectStartRequestArgs) -> String {
    if let Some(profile) = args.profile {
        return format!(
            "export-{project_slug}-{}",
            export_project_profile_job_segment(profile)
        );
    }
    let mode = args
        .mode
        .as_deref()
        .map(normalized_export_option)
        .filter(|mode| !mode.is_empty())
        .unwrap_or_else(|| "video".to_string());
    format!("export-{project_slug}-{mode}")
}

fn export_project_profile_job_segment(profile: ExportProjectProfile) -> &'static str {
    match profile {
        ExportProjectProfile::DraftWebm => "draft-webm",
        ExportProjectProfile::FinalWebm => "final-webm",
        ExportProjectProfile::Mp4H264 => "mp4-h264",
        ExportProjectProfile::Mp4H265 => "mp4-h265",
        ExportProjectProfile::ProResMov => "prores-mov",
        ExportProjectProfile::PremiereXmeml => "premiere-xml",
        ExportProjectProfile::DavinciFcpxml => "davinci-fcpxml",
        ExportProjectProfile::PalmierProject => "palmier",
    }
}

fn webm_render_start_request(
    project: &VideoProject,
    args: &ResolvedExportProjectStartRequestArgs,
    profile: RenderQualityProfile,
) -> Result<crate::project::model::TemporalWorkflowStartRequest, CodexLocalToolError> {
    build_project_webm_render_plan(
        Path::new(&args.project_dir),
        project,
        &args.job_id,
        profile.clone(),
    )
    .map_err(export_project_render_plan_error)?;

    Ok(temporal_render_draft_start_request(
        &project.id,
        &args.project_dir,
        &args.job_id,
        profile,
    ))
}

fn export_project_render_plan_error(errors: Vec<PipelineError>) -> CodexLocalToolError {
    let Some(error) = errors.first() else {
        return CodexLocalToolError::InvalidArguments(
            "export_project render preflight failed without details".to_string(),
        );
    };

    CodexLocalToolError::InvalidArguments(format!(
        "export_project render preflight failed at {}: {} {}",
        error.path, error.message, error.fix
    ))
}

fn nle_xml_export_start_request(
    project: &VideoProject,
    args: &ResolvedExportProjectStartRequestArgs,
    format: NleXmlFormat,
    extension: &str,
    fcpxml_target: Option<&str>,
) -> Result<crate::project::model::TemporalWorkflowStartRequest, CodexLocalToolError> {
    preflight_nle_xml_export(project, format)?;
    let output_path = export_output_path(project, args, extension)?;
    let mut start_request = temporal_export_nle_xml_start_request_with_overwrite(
        &project.id,
        &args.project_dir,
        &args.job_id,
        format,
        &output_path,
        args.overwrite.unwrap_or(true),
    );
    if let Some(timeline_id) = args.timeline_id.as_deref() {
        start_request.input["timelineId"] = json!(timeline_id);
    }
    if let Some(target) = fcpxml_target {
        start_request.input["fcpxmlTarget"] = json!(target);
    }
    Ok(start_request)
}

fn preflight_nle_xml_export(
    project: &VideoProject,
    format: NleXmlFormat,
) -> Result<(), CodexLocalToolError> {
    export_project_timeline_to_nle_xml(project, format)
        .map(|_| ())
        .map_err(nle_xml_export_preflight_error)
}

fn nle_xml_export_preflight_error(error: NleXmlExportError) -> CodexLocalToolError {
    CodexLocalToolError::InvalidArguments(format!("NLE XML export preflight failed: {error}"))
}

fn resolve_export_project_profile(
    args: &ResolvedExportProjectStartRequestArgs,
) -> Result<ExportProjectProfile, CodexLocalToolError> {
    if let Some(profile) = args.profile {
        if args.mode.is_some() || args.codec.is_some() || args.resolution.is_some() {
            return Err(CodexLocalToolError::InvalidArguments(
                "export_project profile cannot be combined with mode, codec, or resolution"
                    .to_string(),
            ));
        }
        return Ok(profile);
    }

    let mode = args.mode.as_deref().unwrap_or("video");
    let mode = normalized_export_option(mode);
    match mode.as_str() {
        "video" => export_video_profile_from_codec(args.codec.as_deref()),
        "xml" | "xmeml" | "premierexmeml" => {
            ensure_no_video_export_options(args, "xml")?;
            Ok(ExportProjectProfile::PremiereXmeml)
        }
        "fcpxml" | "davincifcpxml" | "davinci" => {
            ensure_no_video_export_options(args, "fcpxml")?;
            Ok(ExportProjectProfile::DavinciFcpxml)
        }
        "webm" | "finalwebm" => {
            ensure_no_video_export_options(args, "webm")?;
            Ok(ExportProjectProfile::FinalWebm)
        }
        "draftwebm" => {
            ensure_no_video_export_options(args, "draftWebm")?;
            Ok(ExportProjectProfile::DraftWebm)
        }
        "palmier" | "project" | "projectbundle" => {
            ensure_no_video_export_options(args, "palmier")?;
            Ok(ExportProjectProfile::PalmierProject)
        }
        other => Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project mode must be video, xml, fcpxml, webm, draftWebm, or palmier: {other}"
        ))),
    }
}

fn project_for_export_timeline(
    project: &VideoProject,
    args: &ResolvedExportProjectStartRequestArgs,
    profile: ExportProjectProfile,
) -> Result<VideoProject, CodexLocalToolError> {
    let Some(timeline_id) = args
        .timeline_id
        .as_deref()
        .map(str::trim)
        .filter(|timeline_id| !timeline_id.is_empty())
    else {
        return Ok(project.clone());
    };

    if matches!(profile, ExportProjectProfile::PalmierProject) {
        return Err(CodexLocalToolError::InvalidArguments(
            "export_project timelineId is not valid for palmier mode because project packages include every timeline".to_string(),
        ));
    }

    if timeline_id == project.id {
        return Ok(project.clone());
    }

    project.projected_for_timeline(timeline_id).ok_or_else(|| {
        CodexLocalToolError::InvalidArguments(format!(
            "export_project timelineId `{timeline_id}` was not found"
        ))
    })
}

fn export_project_fcpxml_target(
    args: &ResolvedExportProjectStartRequestArgs,
    profile: ExportProjectProfile,
) -> Result<Option<String>, CodexLocalToolError> {
    let Some(target) = args
        .fcpxml_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
    else {
        return Ok(None);
    };

    if !matches!(profile, ExportProjectProfile::DavinciFcpxml) {
        return Err(CodexLocalToolError::InvalidArguments(
            "export_project fcpxmlTarget only applies to fcpxml mode".to_string(),
        ));
    }

    match normalized_export_option(target).as_str() {
        "resolve" | "davinci" | "davinciresolve" => Ok(Some("resolve".to_string())),
        "fcp" | "finalcut" | "finalcutpro" => Ok(Some("fcp".to_string())),
        other => Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project fcpxmlTarget must be resolve or fcp: {other}"
        ))),
    }
}

fn validate_export_project_resolution(
    args: &ResolvedExportProjectStartRequestArgs,
    profile: ExportProjectProfile,
) -> Result<(), CodexLocalToolError> {
    let Some(resolution) = args
        .resolution
        .as_deref()
        .map(str::trim)
        .filter(|resolution| !resolution.is_empty())
    else {
        return Ok(());
    };

    if !matches!(
        profile,
        ExportProjectProfile::DraftWebm
            | ExportProjectProfile::FinalWebm
            | ExportProjectProfile::Mp4H264
            | ExportProjectProfile::Mp4H265
            | ExportProjectProfile::ProResMov
    ) {
        return Ok(());
    }

    match normalized_export_option(resolution).as_str() {
        "matchtimeline" | "match" | "timeline" => Ok(()),
        "720p" | "1080p" | "2k" | "4k" => Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project resolution '{resolution}' is not supported yet by Video Creater exports; use Match Timeline until scaled video export is implemented."
        ))),
        _ => Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project resolution must be 720p, 1080p, 2K, 4K, or Match Timeline: {resolution}"
        ))),
    }
}

fn export_video_profile_from_codec(
    codec: Option<&str>,
) -> Result<ExportProjectProfile, CodexLocalToolError> {
    match codec.map(normalized_export_option).as_deref() {
        None | Some("h264") => Ok(ExportProjectProfile::Mp4H264),
        Some("h265") | Some("hevc") => Ok(ExportProjectProfile::Mp4H265),
        Some("prores") => Ok(ExportProjectProfile::ProResMov),
        Some("webm") => Ok(ExportProjectProfile::FinalWebm),
        Some(other) => Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project codec must be H.264, H.265, HEVC, ProRes, or WebM: {other}"
        ))),
    }
}

fn ensure_no_video_export_options(
    args: &ResolvedExportProjectStartRequestArgs,
    mode: &str,
) -> Result<(), CodexLocalToolError> {
    if args.codec.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project codec only applies to video mode, not {mode}"
        )));
    }
    if args.resolution.is_some() {
        return Err(CodexLocalToolError::InvalidArguments(format!(
            "export_project resolution only applies to video mode, not {mode}"
        )));
    }
    Ok(())
}

fn normalized_export_option(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn media_export_start_request(
    project: &VideoProject,
    args: &ResolvedExportProjectStartRequestArgs,
    profile: ExportProfile,
    extension: &str,
) -> Result<crate::project::model::TemporalWorkflowStartRequest, CodexLocalToolError> {
    let output_path = export_output_path(project, args, extension)?;
    let options = ExportRenderOptions::new(
        profile,
        RenderQuality::Final,
        project.render_settings.width,
        project.render_settings.height,
    )
    .map_err(|error| CodexLocalToolError::InvalidArguments(error.to_string()))?;
    ensure_export_profile_available(profile, options.quality)?;
    Ok(
        temporal_export_media_start_request_with_options_and_overwrite(
            &project.id,
            &args.project_dir,
            &args.job_id,
            options,
            &output_path,
            args.overwrite.unwrap_or(true),
        ),
    )
}

fn palmier_project_export_start_request(
    project: &VideoProject,
    args: &ResolvedExportProjectStartRequestArgs,
) -> Result<crate::project::model::TemporalWorkflowStartRequest, CodexLocalToolError> {
    let output_path = export_output_path(project, args, "palmier")?;
    let mut input = json!({
        "projectId": project.id,
        "projectDir": args.project_dir,
        "jobId": args.job_id,
        "profile": "palmierProject",
        "outputPath": output_path,
        "validation": {
            "container": "palmier",
            "extension": "palmier",
            "mimeType": "application/vnd.video-creater.project",
            "videoCodec": null,
            "audioCodec": null,
            "requireVideoStream": false,
            "requireAudioStreamWhenTimelineHasAudio": false,
            "packageKind": "splitProjectBundle"
        }
    });
    if args.overwrite == Some(false) {
        input["overwrite"] = json!(false);
    }

    Ok(temporal_workflow_start_request(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        &args.job_id,
        input,
    ))
}

fn ensure_export_profile_available(
    profile: ExportProfile,
    quality: RenderQuality,
) -> Result<(), CodexLocalToolError> {
    let availability = mp4_export_profile_availability_report()
        .into_iter()
        .find(|entry| entry.profile == profile)
        .ok_or_else(|| {
            CodexLocalToolError::InvalidArguments(format!(
                "{} export profile is not recognized",
                export_profile_name(profile)
            ))
        })?;

    ensure_export_profile_quality_available(&availability, profile, quality)
}

fn ensure_export_profile_quality_available(
    availability: &ExportProfileAvailability,
    profile: ExportProfile,
    quality: RenderQuality,
) -> Result<(), CodexLocalToolError> {
    if availability.quality_available(quality) {
        return Ok(());
    }

    let quality_name = match quality {
        RenderQuality::Draft => "Draft",
        RenderQuality::Final => "Final",
    };

    Err(CodexLocalToolError::InvalidArguments(format!(
        "{quality_name} {} export is unavailable: {}",
        export_profile_name(profile),
        availability
            .quality_unavailable_reason(quality)
            .or(availability.unavailable_reason.as_deref())
            .unwrap_or("approved local encoder runtime is unavailable")
    )))
}

fn export_profile_name(profile: ExportProfile) -> &'static str {
    match profile {
        ExportProfile::Webm => "webm",
        ExportProfile::Mp4H264 => "mp4H264",
        ExportProfile::Mp4H265 => "mp4H265",
        ExportProfile::ProResMov => "proResMov",
        ExportProfile::PalmierProject => "palmierProject",
    }
}

fn export_output_path(
    project: &VideoProject,
    args: &ResolvedExportProjectStartRequestArgs,
    extension: &str,
) -> Result<String, CodexLocalToolError> {
    if let Some(output_path) = args
        .output_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return with_default_extension(output_path, extension);
    }

    Ok(format!(
        "exports/{}.{}",
        export_project_slug(project),
        extension
    ))
}

fn with_default_extension(
    output_path: &str,
    extension: &str,
) -> Result<String, CodexLocalToolError> {
    if let Some(existing_extension) = Path::new(output_path)
        .extension()
        .and_then(|value| value.to_str())
    {
        if existing_extension.eq_ignore_ascii_case(extension) {
            Ok(output_path.to_string())
        } else {
            Err(CodexLocalToolError::InvalidArguments(format!(
                "{extension} exports must use .{extension}"
            )))
        }
    } else {
        Ok(format!("{output_path}.{extension}"))
    }
}

fn export_project_slug(project: &VideoProject) -> String {
    let base = project.name.trim();
    let base = if base.is_empty() {
        project.id.trim()
    } else {
        base
    };
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash && !slug.is_empty() {
            slug.push('-');
            previous_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "video-creater-project".to_string()
    } else {
        slug
    }
}

fn build_export_nle_xml_start_request_payload(
    project: &VideoProject,
    args: BuildExportNleXmlStartRequestArgs,
) -> Result<Value, CodexLocalToolError> {
    preflight_nle_xml_export(project, args.format)?;
    let start_request = temporal_export_nle_xml_start_request(
        &project.id,
        &args.project_dir,
        &args.job_id,
        args.format,
        &args.output_path,
    );

    Ok(json!({
        "startRequest": start_request
    }))
}

fn validate_project_actions_payload(
    project: &VideoProject,
    actions: Vec<ProjectAction>,
) -> Result<Value, CodexLocalToolError> {
    let mut next_project = project.clone();
    for action in &actions {
        apply_project_action(&mut next_project, action.clone())
            .map_err(|error| CodexLocalToolError::ProjectActionValidation(error.to_string()))?;
    }

    Ok(json!({
        "valid": true,
        "actionCount": actions.len(),
        "projectAfter": {
            "timelineTracks": next_project.timeline.tracks.len(),
            "timelineItems": next_project.timeline.tracks.iter().map(|track| track.items.len()).sum::<usize>(),
            "lockedTracks": next_project.timeline.tracks.iter().filter(|track| track.locked).count(),
            "media": next_project.media.len(),
            "generatedAssets": next_project.generated_assets.len(),
            "renderReports": next_project.render_reports.len(),
            "exportArtifacts": next_project.export_artifacts.len(),
            "jobs": next_project.jobs.len()
        }
    }))
}

fn validate_mutating_project_actions_payload(
    project: &VideoProject,
    actions: Vec<ProjectAction>,
) -> Result<Value, CodexLocalToolError> {
    if actions.is_empty() {
        return Err(CodexLocalToolError::InvalidArguments(
            "actions must contain at least one project action".to_string(),
        ));
    }

    let mut payload = validate_project_actions_payload(project, actions.clone())?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("mutatesProject".to_string(), json!(true));
        object.insert("projectActions".to_string(), json!(actions));
    }
    Ok(payload)
}

fn mutating_project_actions_payload(
    project: &VideoProject,
    args: ToolProjectActionsArgs,
) -> Result<Value, CodexLocalToolError> {
    if let Some(project_dir) = args
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|project_dir| !project_dir.is_empty())
    {
        let mut payload = apply_project_actions_payload(ApplyProjectActionsArgs {
            project_dir: project_dir.to_string(),
            actions: args.actions,
        })?;
        if let Some(object) = payload.as_object_mut() {
            object.insert("mutatesProject".to_string(), json!(true));
        }
        return Ok(payload);
    }

    validate_mutating_project_actions_payload(project, args.actions)
}

fn apply_project_actions_payload(
    args: ApplyProjectActionsArgs,
) -> Result<Value, CodexLocalToolError> {
    let project_dir = Path::new(&args.project_dir);
    let _project_lease = acquire_split_project_mutation_lease(project_dir)
        .map_err(CodexLocalToolError::ProjectActionApplication)?;
    let before_project = load_split_project(project_dir)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    let action_count = args.actions.len();
    let write_report = apply_project_actions_to_split_project(project_dir, args.actions.clone())
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    let agent_history = record_agent_project_action_batch(
        project_dir,
        before_project,
        write_report.project.clone(),
        action_count,
    )
    .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;

    Ok(json!({
        "applied": true,
        "actionCount": action_count,
        "writeReport": write_report,
        "agentHistory": agent_history
    }))
}

/// `video_creater.undo_agent_edit`: the editor's agent Undo, which also cancels the removed
/// generations' same-process runs and fal.ai/Replicate requests, with the legacy refusal errors.
fn undo_agent_edit_payload(args: UndoAgentEditArgs) -> Result<Value, CodexLocalToolError> {
    let outcome = undo_codex_conversation_edit(Path::new(&args.project_dir), None)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    undo_outcome_payload(outcome)
}

/// Test seam for `video_creater.undo_agent_edit` with the provider-side cancel injected, like
/// `undo_codex_conversation_edit_with`.
pub fn undo_agent_edit_payload_with(
    project_dir: &Path,
    cancel_provider: &mut dyn FnMut(&JobProviderRequest) -> Result<(), String>,
) -> Result<Value, CodexLocalToolError> {
    let outcome = undo_codex_conversation_edit_with(project_dir, None, cancel_provider)
        .map_err(|error| CodexLocalToolError::ProjectActionApplication(error.to_string()))?;
    undo_outcome_payload(outcome)
}

fn undo_outcome_payload(outcome: ProjectAgentUndoOutcome) -> Result<Value, CodexLocalToolError> {
    if let Some(refusal) = legacy_agent_undo_refusal(&outcome) {
        return Err(CodexLocalToolError::ProjectActionApplication(
            refusal.to_string(),
        ));
    }
    let ProjectAgentUndoOutcome::Undone {
        report,
        entry_id,
        action_count,
        remaining_agent_history,
        warnings,
        removed_generated_asset_ids,
        ..
    } = outcome
    else {
        unreachable!("refusals returned above");
    };
    Ok(json!({
        "undone": true,
        "entryId": entry_id,
        "actionCount": action_count,
        "remainingAgentHistory": remaining_agent_history,
        "writeReport": report,
        "warnings": warnings,
        "removedGeneratedAssetIds": removed_generated_asset_ids
    }))
}

fn validate_codex_edit_proposal_payload(
    project: &VideoProject,
    request: EditJobRequest,
    proposal: CodexEditProposal,
) -> Result<Value, CodexLocalToolError> {
    let edl = validate_codex_edit_proposal(project, &request, &proposal)
        .map_err(|error| CodexLocalToolError::ProposalValidation(error.to_string()))?;

    Ok(json!({
        "valid": true,
        "mediaId": proposal.media_id,
        "clipCount": proposal.clips.len(),
        "durationSeconds": edl.duration_seconds(),
        "projectActionCount": proposal.project_actions.len()
    }))
}

fn aspect_ratio_label(width: u32, height: u32) -> String {
    let gcd = greatest_common_divisor(width, height).max(1);
    format!("{}:{}", width / gcd, height / gcd)
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::split::load_split_project;

    #[test]
    fn create_matte_payload_preserves_codex_contract_through_shared_storage() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut project = sample_project();
        project.render_settings.width = 736;
        project.render_settings.height = 400;
        let args: CreateMatteArgs = serde_json::from_value(json!({
            "projectDir": temp_dir.path().display().to_string(),
            "hex": "#112233",
            "aspectRatio": "16:9",
            "name": "Slate"
        }))
        .expect("deserialize matte args");

        let payload = create_matte_payload(&project, args).expect("create matte payload");

        assert_eq!(payload["valid"], json!(true));
        assert_eq!(payload["projectId"], json!(project.id));
        assert_eq!(payload["name"], json!("Slate"));
        assert_eq!(payload["kind"], json!("image"));
        assert_eq!(payload["width"], json!(710));
        assert_eq!(payload["height"], json!(400));
        assert_eq!(
            payload["nextRecommendedInspection"],
            json!("video_creater.get_media")
        );
        assert!(temp_dir
            .path()
            .join(payload["relativePath"].as_str().unwrap())
            .is_file());
        assert!(load_split_project(temp_dir.path())
            .unwrap()
            .media
            .iter()
            .any(|media| media.id == payload["mediaRef"]));
    }

    #[test]
    fn foldered_import_preserves_split_manifest_and_folder_assignment() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("project");
        let source = temp_dir.path().join("source.mp4");
        fs::write(&source, b"test media").unwrap();
        let mut project = sample_project();
        project.media_folders.push(MediaFolder {
            id: "folder-1".to_string(),
            name: "Imported".to_string(),
            parent_id: None,
        });
        crate::project::split::save_split_project(&project_dir, &project).unwrap();
        let args: ImportMediaArgs = serde_json::from_value(json!({
            "projectDir": project_dir.display().to_string(),
            "sourcePaths": [source.display().to_string()],
            "folderId": "folder-1"
        }))
        .unwrap();

        let payload = import_media_payload(&project, args).expect("foldered import");

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(crate::project::storage::project_file_path(&project_dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["layout"], json!("split"));
        let persisted = load_split_project(&project_dir).unwrap();
        let imported_id = payload["imported"][0]["id"].as_str().unwrap();
        assert_eq!(
            persisted
                .media
                .iter()
                .find(|media| media.id == imported_id)
                .and_then(|media| media.folder_id.as_deref()),
            Some("folder-1")
        );
    }

    #[test]
    fn export_project_start_request_payload_accepts_webm_render_profile() {
        let mut project = sample_project();
        project.timeline.tracks[0].items[0]
            .properties
            .insert("sourceIn".to_string(), json!(0.0));
        project.timeline.tracks[0].items[0]
            .properties
            .insert("sourceOut".to_string(), json!(4.0));
        let args: ExportProjectStartRequestArgs = serde_json::from_value(json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "render-final-1",
            "profile": "finalWebm"
        }))
        .expect("deserialize export project args");

        let payload =
            export_project_start_request_payload(&project, args).expect("export project payload");

        assert_eq!(
            payload["startRequest"]["workflowType"],
            json!("VideoCreaterRenderDraftWorkflow")
        );
        assert_eq!(
            payload["startRequest"]["input"]["profile"],
            json!("finalWebm")
        );
    }

    #[test]
    fn export_project_start_request_payload_accepts_nle_xml_profile() {
        let project = sample_project();
        let args: ExportProjectStartRequestArgs = serde_json::from_value(json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-davinci-1",
            "profile": "davinciFcpxml",
            "outputPath": "exports/project.fcpxml"
        }))
        .expect("deserialize export project args");

        let payload =
            export_project_start_request_payload(&project, args).expect("export project payload");

        assert_eq!(
            payload["startRequest"]["workflowType"],
            json!("VideoCreaterExportNleXmlWorkflow")
        );
        assert_eq!(
            payload["startRequest"]["input"]["format"],
            json!("davinciFcpxml")
        );
    }

    #[test]
    fn export_project_start_request_payload_defaults_output_path_for_nle_export() {
        let project = sample_project();
        let args: ExportProjectStartRequestArgs = serde_json::from_value(json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-davinci-1",
            "profile": "davinciFcpxml"
        }))
        .expect("deserialize export project args");

        let payload =
            export_project_start_request_payload(&project, args).expect("export project payload");

        assert_eq!(
            payload["startRequest"]["input"]["outputPath"],
            json!("exports/test-project.fcpxml")
        );
    }

    #[test]
    fn export_project_start_request_payload_accepts_palmier_project_bundle() {
        let project = sample_project();
        let args: ExportProjectStartRequestArgs = serde_json::from_value(json!({
            "projectDir": "/tmp/video-creater-project",
            "jobId": "export-palmier-1",
            "mode": "palmier"
        }))
        .expect("deserialize export project args");

        let payload =
            export_project_start_request_payload(&project, args).expect("export project payload");

        assert_eq!(
            payload["startRequest"]["workflowType"],
            json!("VideoCreaterExportMediaWorkflow")
        );
        assert_eq!(
            payload["startRequest"]["input"]["profile"],
            json!("palmierProject")
        );
        assert_eq!(
            payload["startRequest"]["input"]["outputPath"],
            json!("exports/test-project.palmier")
        );
        assert_eq!(
            payload["startRequest"]["input"]["validation"]["packageKind"],
            json!("splitProjectBundle")
        );
        assert_eq!(
            payload["startRequest"]["input"]["validation"]["mimeType"],
            json!("application/vnd.video-creater.project")
        );
    }

    #[test]
    fn inspect_color_payload_measures_image_reference_gap() {
        let project_dir = tempfile::tempdir().expect("project dir");
        let media_dir = project_dir.path().join("media");
        fs::create_dir_all(&media_dir).expect("media dir");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
            .save(media_dir.join("subject.png"))
            .expect("write subject image");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 255]))
            .save(media_dir.join("reference.png"))
            .expect("write reference image");

        let mut project = sample_project();
        project.media.push(MediaAsset {
            id: "subject-image".to_string(),
            name: Some("Subject image".to_string()),
            relative_path: "media/subject.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1),
            height: Some(1),
            fps: None,
            folder_id: None,
        });
        project.media.push(MediaAsset {
            id: "reference-image".to_string(),
            name: Some("Reference image".to_string()),
            relative_path: "media/reference.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1),
            height: Some(1),
            fps: None,
            folder_id: None,
        });

        let payload = inspect_color_payload(
            &project,
            InspectColorArgs {
                project_dir: Some(project_dir.path().display().to_string()),
                clip_id: None,
                media_id: Some("subject-image".to_string()),
                at_frame: None,
                reference: Some("reference-image".to_string()),
            },
        )
        .expect("inspect image color scopes");

        assert_eq!(payload["scopeStatus"], json!("measured_image_scopes"));
        assert_eq!(payload["reference"]["meanRGB"], json!([0.0, 0.0, 1.0]));
        assert_eq!(payload["gap"]["warmCool"], json!(2.0));
    }

    #[test]
    fn inspect_color_payload_measures_image_clip_with_grade() {
        let project_dir = tempfile::tempdir().expect("project dir");
        let media_dir = project_dir.path().join("media");
        fs::create_dir_all(&media_dir).expect("media dir");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
            .save(media_dir.join("saturated-source.png"))
            .expect("write saturated source image");

        let mut project = sample_project();
        project.media.push(MediaAsset {
            id: "saturated-image".to_string(),
            name: Some("Saturated image".to_string()),
            relative_path: "media/saturated-source.png".to_string(),
            kind: MediaKind::Image,
            duration_seconds: 0.0,
            width: Some(1),
            height: Some(1),
            fps: None,
            folder_id: None,
        });
        project.timeline.tracks[0].items[0] = TimelineItem {
            id: "saturated-image-clip".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 1.0,
            source: TimelineSource::Media {
                media_id: "saturated-image".to_string(),
            },
            label: "Saturated image clip".to_string(),
            properties: BTreeMap::from([(
                "colorGrade".to_string(),
                json!({
                    "saturation": 0.0
                }),
            )]),
        };

        let payload = inspect_color_payload(
            &project,
            InspectColorArgs {
                project_dir: Some(project_dir.path().display().to_string()),
                clip_id: Some("saturated-image-clip".to_string()),
                media_id: None,
                at_frame: None,
                reference: None,
            },
        )
        .expect("inspect image-backed clip scopes");

        assert_eq!(payload["scopeStatus"], json!("measured_image_clip_scopes"));
        assert_eq!(payload["clip"]["meanRGB"], json!([0.212, 0.212, 0.212]));
        assert_eq!(payload["grade"]["saturation"], json!(0.0));
    }

    #[test]
    fn get_transcript_compact_rows_include_speakers_when_available() {
        let mut project = sample_project();
        project.timeline.tracks[0].items[0].start_seconds = 2.0;
        project.timeline.tracks[0].items[0].duration_seconds = 3.0;
        project.timeline.tracks[0].items[0]
            .properties
            .insert("sourceIn".to_string(), json!(1.0));
        project.timeline.tracks[0].items[0]
            .properties
            .insert("sourceOut".to_string(), json!(4.0));
        project.transcripts.push(Transcript {
            id: "transcript-1".to_string(),
            media_id: "media-1".to_string(),
            engine: Some("fixture".to_string()),
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: vec![
                TranscriptWord {
                    text: "founder".to_string(),
                    start_seconds: 1.5,
                    end_seconds: 1.8,
                    confidence: Some(0.9),
                    speaker: Some("Speaker 1".to_string()),
                },
                TranscriptWord {
                    text: "launch".to_string(),
                    start_seconds: 2.2,
                    end_seconds: 2.5,
                    confidence: Some(0.88),
                    speaker: None,
                },
            ],
        });
        let args: TranscriptWordsArgs = serde_json::from_value(json!({
            "clipId": "item-1",
            "limit": 10
        }))
        .expect("deserialize transcript args");

        let payload = transcript_words_payload(&project, &args).expect("transcript payload");

        assert_eq!(
            payload["wordFormat"],
            json!(["wordIndex", "text", "startSeconds", "endSeconds", "speaker"])
        );
        assert_eq!(
            payload["frameWordFormat"],
            json!(["wordIndex", "text", "startFrame", "endFrame", "speaker"])
        );
        assert_eq!(
            payload["clips"][0]["wordFormat"],
            json!(["wordIndex", "text", "startSeconds", "endSeconds", "speaker"])
        );
        assert_eq!(
            payload["clips"][0]["frameWordFormat"],
            json!(["wordIndex", "text", "startFrame", "endFrame", "speaker"])
        );
        assert_eq!(
            payload["clips"][0]["words"][0],
            json!([0, "founder", 2.5, 2.8, "Speaker 1"])
        );
        assert_eq!(
            payload["clips"][0]["frameWords"][0],
            json!([0, "founder", 60, 67, "Speaker 1"])
        );
        assert_eq!(
            payload["clips"][0]["words"][1],
            json!([1, "launch", 3.2, 3.5, null])
        );
    }

    #[test]
    fn export_guard_uses_the_requested_quality_reason() {
        let availability = crate::project::export_profiles::ExportProfileAvailability {
            profile: ExportProfile::ProResMov,
            label: "ProRes MOV".to_string(),
            available: true,
            container: "mov".to_string(),
            extension: "mov".to_string(),
            mime_type: "video/quicktime".to_string(),
            video_codec: "prores".to_string(),
            audio_codec: Some("pcm".to_string()),
            required_runtime: vec!["system:avfoundation".to_string()],
            policy_status: crate::project::export_profiles::ExportPolicyStatus::Approved,
            unavailable_reason: None,
            quality_availability: crate::project::export_profiles::ExportQualityAvailability {
                draft: false,
                final_quality: true,
            },
            quality_unavailable_reasons:
                crate::project::export_profiles::ExportQualityUnavailableReasons {
                    draft: Some(
                        "Draft ProRes requires native ProRes Proxy capability.".to_string(),
                    ),
                    final_quality: None,
                },
        };

        let error = ensure_export_profile_quality_available(
            &availability,
            ExportProfile::ProResMov,
            RenderQuality::Draft,
        )
        .expect_err("Draft guard must reject unavailable ProRes Proxy");
        assert!(error
            .to_string()
            .contains("Draft ProRes requires native ProRes Proxy"));
        ensure_export_profile_quality_available(
            &availability,
            ExportProfile::ProResMov,
            RenderQuality::Final,
        )
        .expect("Final guard should accept available ProRes 422");
    }

    #[test]
    fn read_skill_uses_the_fixed_catalog_with_checksum_provenance() {
        let definition =
            crate::settings::skills::mandatory_skill_definition("video-creater-video-pipeline")
                .expect("pipeline definition");

        let payload = read_skill_payload(ReadSkillArgs {
            id: definition.id.to_string(),
        })
        .expect("read mandatory skill");

        assert_eq!(payload["id"], definition.id);
        assert_eq!(payload["path"], definition.relative_path);
        assert_eq!(payload["body"], definition.bundled_content);
        assert_eq!(
            payload["bundledChecksum"],
            crate::settings::skills::sha256_checksum(definition.bundled_content)
        );
        assert_eq!(payload["loadState"], "compiled");
        assert_eq!(payload["promptIncluded"], true);
    }
}
