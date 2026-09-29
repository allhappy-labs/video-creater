use super::audio_edits;
use super::model::*;
use super::reverse::{self, SourceWindow};
use super::transitions;
use crate::effects::effect_descriptor;
use crate::frame_compositor::{KeyframeEasing, NumericCurve, NumericKeyframe};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectAction {
    AddItems {
        target_track_id: String,
        items: Vec<TimelineItem>,
    },
    InsertItems {
        target_track_id: String,
        insert_seconds: f64,
        items: Vec<TimelineItem>,
    },
    RemoveItems {
        item_ids: Vec<String>,
    },
    MoveItems {
        moves: Vec<ProjectActionMove>,
    },
    ReorderItems {
        reorder: ProjectActionReorder,
    },
    ResizeItems {
        resizes: Vec<ProjectActionResize>,
    },
    TrimItems {
        trims: Vec<ProjectActionTrim>,
    },
    RippleTrimItem {
        item_id: String,
        edge: ProjectActionTrimEdge,
        delta_seconds: f64,
        propagate_linked: bool,
        #[serde(default)]
        sync_locked_track_ids: Vec<String>,
    },
    RippleDeleteRanges {
        ranges: Vec<ProjectActionRippleDeleteRange>,
    },
    SplitItems {
        splits: Vec<ProjectActionSplit>,
    },
    CreateTrack {
        track: TimelineTrack,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after_track_id: Option<String>,
    },
    CreateTimeline {
        timeline_id: String,
        name: String,
        duplicate_active: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_timeline_id: Option<String>,
    },
    SetActiveTimeline {
        timeline_id: String,
    },
    RenameTimeline {
        timeline_id: String,
        name: String,
    },
    DeleteTimeline {
        timeline_id: String,
    },
    DecomposeTimelineItem {
        item_id: String,
    },
    SetTrackLocked {
        track_id: String,
        locked: bool,
    },
    ReorderTrack {
        track_id: String,
        target_track_id: String,
        placement: ProjectActionTrackPlacement,
    },
    SetTrackSyncLocked {
        track_id: String,
        sync_locked: bool,
    },
    SetTrackEnabled {
        track_id: String,
        enabled: bool,
    },
    EditCaptionText {
        item_id: String,
        text: String,
    },
    EditTextItem {
        item_id: String,
        text: String,
    },
    UpdateAudioFadeOut {
        item_id: String,
        fade_out_seconds: f64,
    },
    UpdateAudioFades {
        item_id: String,
        fade_in_seconds: f64,
        fade_out_seconds: f64,
    },
    UpdateAudioVolume {
        item_id: String,
        volume_db: Option<f64>,
    },
    UpdateAudioSync {
        item_id: String,
        sync: ProjectActionAudioSync,
    },
    UpdateVisualClipOpacity {
        item_id: String,
        opacity: f64,
    },
    UpdateVisualClipTransform {
        item_id: String,
        transform: ProjectActionVisualTransform,
    },
    UpdateVisualClipCrop {
        item_id: String,
        crop: ProjectActionVisualCrop,
    },
    UpdateVisualClipFades {
        item_id: String,
        fade_in_seconds: f64,
        fade_out_seconds: f64,
    },
    UpdateVisualClipSpeed {
        item_id: String,
        speed: f64,
    },
    UpdateAudioClipSpeed {
        item_id: String,
        speed: f64,
    },
    DetachAudio {
        item_id: String,
        audio_item_id: String,
        target_track_id: String,
        link_group_id: String,
    },
    UpdateClipReverse {
        item_id: String,
        reverse: bool,
    },
    SetItemKeyframes {
        item_id: String,
        property: ProjectActionKeyframeProperty,
        keyframes: Vec<ProjectActionKeyframe>,
    },
    UpsertItemKeyframe {
        item_id: String,
        property: ProjectActionKeyframeProperty,
        keyframe: ProjectActionKeyframe,
    },
    MoveItemKeyframe {
        item_id: String,
        property: ProjectActionKeyframeProperty,
        from_seconds: f64,
        to_seconds: f64,
    },
    DeleteItemKeyframe {
        item_id: String,
        property: ProjectActionKeyframeProperty,
        at_seconds: f64,
    },
    UpsertEffectParameterKeyframe {
        item_id: String,
        effect_instance_id: String,
        parameter_key: String,
        keyframe: ProjectActionKeyframe,
    },
    MoveEffectParameterKeyframe {
        item_id: String,
        effect_instance_id: String,
        parameter_key: String,
        from_seconds: f64,
        to_seconds: f64,
    },
    DeleteEffectParameterKeyframe {
        item_id: String,
        effect_instance_id: String,
        parameter_key: String,
        at_seconds: f64,
    },
    UpdateItemProperties {
        updates: Vec<ProjectActionItemPropertiesUpdate>,
    },
    LinkItems {
        item_ids: Vec<String>,
        link_group_id: String,
    },
    UnlinkItems {
        item_ids: Vec<String>,
    },
    UpdateItemEffects {
        item_ids: Vec<String>,
        effects: Vec<ProjectActionEffect>,
    },
    UpdateItemColorGrade {
        item_ids: Vec<String>,
        reset: bool,
        grade: ProjectActionColorGrade,
    },
    ApplyCaptionRepair {
        repair: ProjectActionCaptionRepair,
    },
    EditTranscriptWords {
        edits: Vec<ProjectActionTranscriptWordEdit>,
    },
    RecordGeneratedAsset {
        asset: Box<ProjectActionGeneratedAsset>,
    },
    UpdateGeneratedAssetStatus {
        asset_id: String,
        status: GeneratedAssetStatus,
    },
    UpdateGeneratedAssetReferences {
        asset_id: String,
        references: ProjectActionGeneratedAssetReferences,
    },
    CompleteGeneratedAsset {
        asset_id: String,
        outputs: Vec<ProjectActionGeneratedAssetOutput>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        completion: Option<ProjectActionGeneratedAssetCompletion>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replacement: Option<ProjectActionReplaceGeneratedOutput>,
    },
    ReplaceTimelineItemWithGeneratedOutput {
        replacement: ProjectActionReplaceGeneratedOutput,
    },
    AssignMediaFolder {
        media_id: String,
        folder_id: Option<String>,
    },
    CreateMediaFolder {
        folder: MediaFolder,
    },
    RenameMediaFolder {
        folder_id: String,
        name: String,
    },
    DeleteMediaFolder {
        folder_id: String,
    },
    RenameMedia {
        media_id: String,
        name: String,
    },
    DeleteMedia {
        media_ids: Vec<String>,
    },
    RemoveTracks {
        track_ids: Vec<String>,
    },
    UpdateProjectSettings {
        name: String,
        render_settings: RenderSettings,
    },
    UpdateRenderSettings {
        settings: RenderSettings,
    },
    AttachRenderReport {
        report: ProjectRenderReport,
    },
    RecordExportArtifact {
        artifact: ProjectExportArtifact,
    },
    RecordJob {
        job: Box<JobSummary>,
    },
    UpdateJobStatus {
        job_id: String,
        status: JobStatus,
        updated_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
    },
    UpdateJobProviderRequest {
        job_id: String,
        provider_request: JobProviderRequest,
    },
    /// Bookkeeping: fail a still-unfinished job with a plain reason. Finished
    /// jobs are left untouched. Not offered to Codex or MCP.
    RecordJobFailure {
        job_id: String,
        reason: String,
        updated_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
    },
    UpdateTemplateItems {
        updates: Vec<ProjectActionTemplateUpdate>,
    },
    UpdateTextOverlayItems {
        updates: Vec<ProjectActionTextOverlayUpdate>,
    },
    UpdateTemplateOverride {
        #[serde(rename = "override")]
        override_: ProjectActionTemplateOverrideUpdate,
    },
    AddTransition {
        track_id: String,
        transition: TimelineTransition,
    },
    UpdateTransition {
        track_id: String,
        transition_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<TransitionKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_seconds: Option<f64>,
    },
    RemoveTransition {
        track_id: String,
        transition_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionMove {
    pub item_id: String,
    pub target_track_id: String,
    pub start_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionItemPropertiesUpdate {
    pub item_id: String,
    #[serde(default)]
    pub set: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionAudioSync {
    pub reference_clip_id: String,
    pub offset_seconds: f64,
    pub confidence: f64,
    pub synced_at_start_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionReorder {
    pub target_track_id: String,
    pub item_ids: Vec<String>,
    pub start_seconds: f64,
    pub gap_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionResize {
    pub item_id: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionTrim {
    pub item_id: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_in: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_out: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectActionTrimEdge {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectActionTrackPlacement {
    Before,
    After,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionRippleDeleteRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
    #[serde(default)]
    pub track_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProjectActionKeyframeProperty {
    Opacity,
    VolumeDb,
    PositionX,
    PositionY,
    Scale,
    ScaleX,
    ScaleY,
    RotationDegrees,
    CropTop,
    CropRight,
    CropBottom,
    CropLeft,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionKeyframe {
    pub at_seconds: f64,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionVisualTransform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_horizontal: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_vertical: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionVisualCrop {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_top: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_right: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_bottom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_left: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionSplit {
    pub item_id: String,
    pub new_item_id: String,
    pub split_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionTemplateUpdate {
    pub item_id: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub template_fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionTextOverlayUpdate {
    pub item_id: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub text: String,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionEffect {
    #[serde(default)]
    pub effect_instance_id: String,
    #[serde(rename = "effectType")]
    pub effect_type: String,
    pub enabled: bool,
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionColorGrade {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<f64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionTemplateOverrideUpdate {
    pub template_id: String,
    pub name: String,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    #[serde(default)]
    pub style: BTreeMap<String, serde_json::Value>,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionCaptionRepair {
    pub caption_item_id: String,
    pub transcript_id: String,
    pub word_index: usize,
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub repair_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionTranscriptWordEdit {
    pub transcript_id: String,
    pub word_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_seconds: Option<f64>,
    pub repair_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionGeneratedAsset {
    pub id: String,
    pub kind: MediaKind,
    pub status: GeneratedAssetStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_intent: Option<String>,
    pub prompt: String,
    pub model: ProjectActionGenerationModel,
    pub references: ProjectActionGeneratedAssetReferences,
    #[serde(default)]
    pub settings: ProjectActionGeneratedAssetSettings,
    pub outputs: Vec<ProjectActionGeneratedAssetOutput>,
    pub created_at: String,
    pub parent_asset_id: Option<String>,
    pub retry_of_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionGenerationModel {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionGeneratedAssetReferences {
    #[serde(default)]
    pub media_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_video_media_ref: Option<String>,
    pub first_frame_media_id: Option<String>,
    pub last_frame_media_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_image_media_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_video_media_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_audio_media_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_input_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionGeneratedAssetSettings {
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub duration_seconds: Option<f64>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default)]
    pub aspect_ratio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_images: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyrics: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrumental: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_source_start_frame: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_source_end_frame: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_source_start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_source_end_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_start_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionGeneratedAssetOutput {
    pub media_id: String,
    pub relative_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub width: u32,
    pub height: u32,
    pub duration_seconds: f64,
    pub fps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionGeneratedAssetCompletion {
    pub generated_asset_id: String,
    pub generated_output_media_id: String,
    pub placement_intent: String,
    pub references: ProjectActionGeneratedAssetReferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionReplaceGeneratedOutput {
    pub item_id: String,
    pub media_id: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum ProjectActionError {
    #[error("action must include at least one item")]
    EmptyItems,
    #[error("time values cannot be negative or non-finite")]
    NegativeTime,
    #[error("duration must be finite and greater than zero")]
    NonPositiveDuration,
    #[error("timeline track was not found: {0}")]
    TrackNotFound(String),
    #[error("timeline tracks cannot be reordered across visual and audio zones")]
    InvalidTrackZone,
    #[error("timeline item was not found: {0}")]
    ItemNotFound(String),
    #[error("track is locked: {0}")]
    TrackLocked(String),
    #[error("timeline item id already exists: {0}")]
    DuplicateItemId(String),
    #[error("timeline track id already exists: {0}")]
    DuplicateTrackId(String),
    #[error("timeline track id cannot be empty")]
    EmptyTrackId,
    #[error("timeline track name cannot be empty")]
    EmptyTrackName,
    #[error("media id already exists: {0}")]
    DuplicateMediaId(String),
    #[error("generated asset id already exists: {0}")]
    DuplicateGeneratedAssetId(String),
    #[error("generated asset id must be a safe path segment: {0}")]
    InvalidGeneratedAssetId(String),
    #[error("split point must be inside timeline item: {0}")]
    SplitPointOutsideItem(String),
    #[error("source range duration does not match timeline duration: {0}")]
    SourceRangeDurationMismatch(String),
    #[error("source range is outside media duration: {0}")]
    SourceRangeOutsideMedia(String),
    #[error("generated edit video clip requires sourceIn and sourceOut: {0}")]
    MissingGeneratedEditSourceRange(String),
    #[error("generated edit video clip cannot pass through the full source: {0}")]
    GeneratedEditFullSourcePassThrough(String),
    #[error("media was not found: {0}")]
    MissingMedia(String),
    #[error("media folder was not found: {0}")]
    MissingMediaFolder(String),
    #[error("media folder id cannot be empty")]
    EmptyMediaFolderId,
    #[error("media folder id already exists: {0}")]
    DuplicateMediaFolderId(String),
    #[error("media folder id must be a safe path segment: {0}")]
    InvalidMediaFolderId(String),
    #[error("media folder name cannot be empty")]
    EmptyMediaFolderName,
    #[error("media folder parent was not found: {0}")]
    MissingMediaFolderParent(String),
    #[error("media name cannot be empty")]
    EmptyMediaName,
    #[error("media is still used by generated asset provenance: {0}")]
    MediaInUse(String),
    #[error("timeline track is not empty: {0}")]
    TrackNotEmpty(String),
    #[error("render settings are invalid: {0}")]
    InvalidRenderSettings(String),
    #[error("caption text cannot be empty")]
    EmptyCaptionText,
    #[error("timeline item is not a caption: {0}")]
    NotCaptionItem(String),
    #[error("text item content cannot be empty")]
    EmptyTextItemText,
    #[error("timeline item is not text-backed: {0}")]
    NotTextItem(String),
    #[error("timeline item is not an audio clip: {0}")]
    NotAudioClip(String),
    #[error("audio clip volume is outside the supported range: {0}")]
    InvalidAudioVolume(String),
    #[error("audio clip fades are invalid: {0}")]
    InvalidAudioFades(String),
    #[error("audio clip speed is invalid: {0}")]
    InvalidAudioClipSpeed(String),
    #[error("timeline item has no detachable audio: {0}")]
    NoDetachableAudio(String),
    #[error("timeline item audio is already on a linked audio clip: {0}")]
    AudioAlreadyDetached(String),
    #[error("timeline item cannot be reversed: {0}")]
    NotReversibleClip(String),
    #[error("audio sync metadata is invalid: {0}")]
    InvalidAudioSync(String),
    #[error("timeline item is not a visual clip: {0}")]
    NotVisualClip(String),
    #[error("visual clip opacity is outside the supported range: {0}")]
    InvalidVisualClipOpacity(String),
    #[error("visual clip transform is invalid: {0}")]
    InvalidVisualClipTransform(String),
    #[error("visual clip crop is invalid: {0}")]
    InvalidVisualClipCrop(String),
    #[error("visual clip fades are invalid: {0}")]
    InvalidVisualClipFades(String),
    #[error("visual clip speed is invalid: {0}")]
    InvalidVisualClipSpeed(String),
    #[error("effect stack is invalid: {0}")]
    InvalidEffectStack(String),
    #[error("effect parameter is invalid: {0}")]
    InvalidEffectParam(String),
    #[error("keyframe was not found at {at_seconds} seconds for {item_id}.{property}")]
    KeyframeNotFound {
        item_id: String,
        property: String,
        at_seconds: f64,
    },
    #[error("effect instance was not found on timeline item {item_id}: {effect_instance_id}")]
    EffectInstanceNotFound {
        item_id: String,
        effect_instance_id: String,
    },
    #[error("ripple trim would collide on track {track_id}: {left_item_id} with {right_item_id}")]
    RippleTrimCollision {
        track_id: String,
        left_item_id: String,
        right_item_id: String,
    },
    #[error("timeline items overlap on track {track_id}: {item_id} overlaps {blocking_item_id}")]
    TrackItemOverlap {
        track_id: String,
        item_id: String,
        blocking_item_id: String,
    },
    #[error("color grade is invalid: {0}")]
    InvalidColorGrade(String),
    #[error("timeline item is not a text overlay: {0}")]
    NotTextOverlayItem(String),
    #[error("text overlay visual metadata is missing: {0}")]
    MissingTextOverlayVisualMetadata(String),
    #[error("timeline item is not a template: {0}")]
    NotTemplateItem(String),
    #[error("template id cannot be empty")]
    EmptyTemplateId,
    #[error("template name cannot be empty")]
    EmptyTemplateName,
    #[error("template visual metadata is missing: {0}")]
    MissingTemplateVisualMetadata(String),
    #[error("transcript was not found: {0}")]
    TranscriptNotFound(String),
    #[error("transcript word was not found: {transcript_id}[{word_index}]")]
    TranscriptWordNotFound {
        transcript_id: String,
        word_index: usize,
    },
    #[error("transcript word text cannot be empty")]
    EmptyTranscriptText,
    #[error("transcript word timestamps must remain monotonic: {0}")]
    NonMonotonicTranscriptTiming(String),
    #[error("generated asset prompt cannot be empty")]
    EmptyGeneratedPrompt,
    #[error("generated asset model cannot be empty")]
    EmptyGeneratedModel,
    #[error("generated asset settings are invalid: {0}")]
    InvalidGeneratedAssetSettings(String),
    #[error("generated asset kind must be generated: {0:?}")]
    InvalidGeneratedAssetKind(MediaKind),
    #[error("generated asset media reference cannot be empty")]
    EmptyGeneratedMediaReference,
    #[error("generated frame reference must point to visual media: {0}")]
    InvalidGeneratedFrameReference(String),
    #[error("generated asset output media id cannot be empty")]
    EmptyGeneratedOutputMediaId,
    #[error("generated asset output path cannot be empty")]
    EmptyGeneratedOutputPath,
    #[error("generated output path cannot use a reserved project sidecar: {0}")]
    GeneratedOutputReservedPath(String),
    #[error("generated output path must stay under its asset directory: {0}")]
    GeneratedOutputOutsideAssetDirectory(String),
    #[error("generated asset lineage references missing asset: {0}")]
    MissingGeneratedAssetReference(String),
    #[error("generated asset is already completed: {0}")]
    GeneratedAssetAlreadyCompleted(String),
    #[error("completion replacement media must be one of the completed outputs: {0}")]
    CompletionReplacementOutputMismatch(String),
    #[error("generated output media was not found: {0}")]
    MissingGeneratedOutput(String),
    #[error("media is not a completed generated output: {0}")]
    NotGeneratedOutput(String),
    #[error("render report id already exists: {0}")]
    DuplicateRenderReportId(String),
    #[error("render report output path cannot be empty")]
    EmptyRenderOutputPath,
    #[error("render report must record at least one output stream")]
    EmptyRenderStreams,
    #[error("render report artifact list cannot be empty")]
    EmptyRenderArtifacts,
    #[error("render report log path cannot be empty")]
    EmptyRenderLogPath,
    #[error("render report is missing required check: {0}")]
    MissingRenderCheck(String),
    #[error("export artifact id cannot be empty")]
    EmptyExportArtifactId,
    #[error("export artifact id must be a safe path segment: {0}")]
    InvalidExportArtifactId(String),
    #[error("export artifact id already exists: {0}")]
    DuplicateExportArtifactId(String),
    #[error("export artifact format cannot be empty")]
    EmptyExportArtifactFormat,
    #[error("export artifact path cannot be empty")]
    EmptyExportArtifactPath,
    #[error("export artifact path must be project-relative: {0}")]
    UnsafeExportArtifactPath(String),
    #[error("export artifact contract is invalid: {0}")]
    InvalidExportArtifactContract(String),
    #[error("export artifact mimeType cannot be empty")]
    EmptyExportArtifactMimeType,
    #[error("export artifact createdAt cannot be empty")]
    EmptyExportArtifactCreatedAt,
    #[error("job id cannot be empty")]
    EmptyJobId,
    #[error("job id must be a safe path segment: {0}")]
    InvalidJobId(String),
    #[error("job id already exists: {0}")]
    DuplicateJobId(String),
    #[error("job was not found: {0}")]
    JobNotFound(String),
    #[error("job status is terminal and cannot be overwritten: {0}")]
    TerminalJobStatus(String),
    #[error("render attempt does not match the active canonical job attempt: {0}")]
    StaleRenderAttempt(String),
    #[error("generated asset status is terminal and cannot be overwritten: {0}")]
    TerminalGeneratedAssetStatus(String),
    #[error("job kind cannot be empty")]
    EmptyJobKind,
    #[error("job updatedAt cannot be empty")]
    EmptyJobUpdatedAt,
    #[error("job failure reason cannot be empty")]
    EmptyJobFailureReason,
    #[error("job export settings are invalid: {0}")]
    InvalidJobExportSettings(String),
    #[error("job provider request metadata is missing: {0}")]
    MissingJobProviderRequestMetadata(String),
    #[error("job workflow metadata is missing: {0}")]
    MissingJobWorkflowMetadata(String),
    #[error("job start request does not match workflow metadata: {0}")]
    MismatchedJobStartRequest(String),
    /// A user-facing transition validation message, e.g. "Not enough unused
    /// media after Opening shot for a 1.0s transition. Maximum is 0.4s."
    #[error("{0}")]
    InvalidTransition(String),
    #[error("transition was not found: {0}")]
    TransitionNotFound(String),
    #[error("item kind {item_kind:?} is incompatible with track kind {track_kind:?}")]
    TrackTypeMismatch {
        item_kind: TimelineItemKind,
        track_kind: TrackKind,
    },
}

pub fn apply_project_action(
    project: &mut VideoProject,
    action: ProjectAction,
) -> Result<(), ProjectActionError> {
    let mut next_project = project.clone();
    timeline_library(&mut next_project);
    let overlap_timeline_id = next_project.active_timeline_id.clone();
    let overlap_item_lineage = match &action {
        ProjectAction::SplitItems { splits } => splits
            .iter()
            .map(|split| (split.new_item_id.clone(), split.item_id.clone()))
            .collect(),
        _ => BTreeMap::new(),
    };

    match action {
        ProjectAction::AddItems {
            target_track_id,
            items,
        } => add_items(&mut next_project, &target_track_id, items),
        ProjectAction::InsertItems {
            target_track_id,
            insert_seconds,
            items,
        } => insert_items(&mut next_project, &target_track_id, insert_seconds, items),
        ProjectAction::RemoveItems { item_ids } => remove_items(&mut next_project, &item_ids),
        ProjectAction::MoveItems { moves } => move_items(&mut next_project, &moves),
        ProjectAction::ReorderItems { reorder } => reorder_items(&mut next_project, &reorder),
        ProjectAction::ResizeItems { resizes } => resize_items(&mut next_project, &resizes),
        ProjectAction::TrimItems { trims } => trim_items(&mut next_project, &trims),
        ProjectAction::RippleTrimItem {
            item_id,
            edge,
            delta_seconds,
            propagate_linked,
            sync_locked_track_ids,
        } => ripple_trim_item(
            &mut next_project,
            &item_id,
            edge,
            delta_seconds,
            propagate_linked,
            &sync_locked_track_ids,
        ),
        ProjectAction::RippleDeleteRanges { ranges } => {
            ripple_delete_ranges(&mut next_project, &ranges)
        }
        ProjectAction::SplitItems { splits } => split_items(&mut next_project, &splits),
        ProjectAction::CreateTrack {
            track,
            after_track_id,
        } => create_track(&mut next_project, track, after_track_id.as_deref()),
        ProjectAction::CreateTimeline {
            timeline_id,
            name,
            duplicate_active,
            source_timeline_id,
        } => create_timeline(
            &mut next_project,
            &timeline_id,
            &name,
            duplicate_active,
            source_timeline_id.as_deref(),
        ),
        ProjectAction::SetActiveTimeline { timeline_id } => {
            set_active_timeline(&mut next_project, &timeline_id)
        }
        ProjectAction::RenameTimeline { timeline_id, name } => {
            rename_timeline(&mut next_project, &timeline_id, &name)
        }
        ProjectAction::DeleteTimeline { timeline_id } => {
            delete_timeline(&mut next_project, &timeline_id)
        }
        ProjectAction::DecomposeTimelineItem { item_id } => {
            decompose_timeline_item(&mut next_project, &item_id)
        }
        ProjectAction::SetTrackLocked { track_id, locked } => {
            set_track_locked(&mut next_project, &track_id, locked)
        }
        ProjectAction::ReorderTrack {
            track_id,
            target_track_id,
            placement,
        } => reorder_track(&mut next_project, &track_id, &target_track_id, placement),
        ProjectAction::SetTrackSyncLocked {
            track_id,
            sync_locked,
        } => set_track_sync_locked(&mut next_project, &track_id, sync_locked),
        ProjectAction::SetTrackEnabled { track_id, enabled } => {
            set_track_enabled(&mut next_project, &track_id, enabled)
        }
        ProjectAction::EditCaptionText { item_id, text } => {
            edit_caption_text(&mut next_project, &item_id, &text)
        }
        ProjectAction::EditTextItem { item_id, text } => {
            edit_text_item(&mut next_project, &item_id, &text)
        }
        ProjectAction::UpdateAudioFadeOut {
            item_id,
            fade_out_seconds,
        } => update_audio_fade_out(&mut next_project, &item_id, fade_out_seconds),
        ProjectAction::UpdateAudioFades {
            item_id,
            fade_in_seconds,
            fade_out_seconds,
        } => update_audio_fades(
            &mut next_project,
            &item_id,
            fade_in_seconds,
            fade_out_seconds,
        ),
        ProjectAction::UpdateAudioVolume { item_id, volume_db } => {
            update_audio_volume(&mut next_project, &item_id, volume_db)
        }
        ProjectAction::UpdateAudioSync { item_id, sync } => {
            update_audio_sync(&mut next_project, &item_id, sync)
        }
        ProjectAction::UpdateVisualClipOpacity { item_id, opacity } => {
            update_visual_clip_opacity(&mut next_project, &item_id, opacity)
        }
        ProjectAction::UpdateVisualClipTransform { item_id, transform } => {
            update_visual_clip_transform(&mut next_project, &item_id, transform)
        }
        ProjectAction::UpdateVisualClipCrop { item_id, crop } => {
            update_visual_clip_crop(&mut next_project, &item_id, crop)
        }
        ProjectAction::UpdateVisualClipFades {
            item_id,
            fade_in_seconds,
            fade_out_seconds,
        } => update_visual_clip_fades(
            &mut next_project,
            &item_id,
            fade_in_seconds,
            fade_out_seconds,
        ),
        ProjectAction::UpdateVisualClipSpeed { item_id, speed } => {
            update_visual_clip_speed(&mut next_project, &item_id, speed)
        }
        ProjectAction::UpdateAudioClipSpeed { item_id, speed } => {
            audio_edits::update_audio_clip_speed(&mut next_project, &item_id, speed)
        }
        ProjectAction::DetachAudio {
            item_id,
            audio_item_id,
            target_track_id,
            link_group_id,
        } => audio_edits::detach_audio(
            &mut next_project,
            &item_id,
            &audio_item_id,
            &target_track_id,
            &link_group_id,
        ),
        ProjectAction::UpdateClipReverse { item_id, reverse } => {
            reverse::update_clip_reverse(&mut next_project, &item_id, reverse)
        }
        ProjectAction::SetItemKeyframes {
            item_id,
            property,
            keyframes,
        } => set_item_keyframes(&mut next_project, &item_id, property, &keyframes),
        ProjectAction::UpsertItemKeyframe {
            item_id,
            property,
            keyframe,
        } => upsert_item_keyframe(&mut next_project, &item_id, property, keyframe),
        ProjectAction::MoveItemKeyframe {
            item_id,
            property,
            from_seconds,
            to_seconds,
        } => move_item_keyframe(
            &mut next_project,
            &item_id,
            property,
            from_seconds,
            to_seconds,
        ),
        ProjectAction::DeleteItemKeyframe {
            item_id,
            property,
            at_seconds,
        } => delete_item_keyframe(&mut next_project, &item_id, property, at_seconds),
        ProjectAction::UpsertEffectParameterKeyframe {
            item_id,
            effect_instance_id,
            parameter_key,
            keyframe,
        } => upsert_effect_parameter_keyframe(
            &mut next_project,
            &item_id,
            &effect_instance_id,
            &parameter_key,
            keyframe,
        ),
        ProjectAction::MoveEffectParameterKeyframe {
            item_id,
            effect_instance_id,
            parameter_key,
            from_seconds,
            to_seconds,
        } => move_effect_parameter_keyframe(
            &mut next_project,
            &item_id,
            &effect_instance_id,
            &parameter_key,
            from_seconds,
            to_seconds,
        ),
        ProjectAction::DeleteEffectParameterKeyframe {
            item_id,
            effect_instance_id,
            parameter_key,
            at_seconds,
        } => delete_effect_parameter_keyframe(
            &mut next_project,
            &item_id,
            &effect_instance_id,
            &parameter_key,
            at_seconds,
        ),
        ProjectAction::UpdateItemProperties { updates } => {
            update_item_properties(&mut next_project, &updates)
        }
        ProjectAction::LinkItems {
            item_ids,
            link_group_id,
        } => link_items(&mut next_project, &item_ids, &link_group_id),
        ProjectAction::UnlinkItems { item_ids } => unlink_items(&mut next_project, &item_ids),
        ProjectAction::UpdateItemEffects { item_ids, effects } => {
            update_item_effects(&mut next_project, &item_ids, effects)
        }
        ProjectAction::UpdateItemColorGrade {
            item_ids,
            reset,
            grade,
        } => update_item_color_grade(&mut next_project, &item_ids, reset, grade),
        ProjectAction::ApplyCaptionRepair { repair } => {
            apply_caption_repair(&mut next_project, &repair)
        }
        ProjectAction::EditTranscriptWords { edits } => {
            edit_transcript_words(&mut next_project, &edits)
        }
        ProjectAction::RecordGeneratedAsset { asset } => {
            record_generated_asset(&mut next_project, *asset)
        }
        ProjectAction::UpdateGeneratedAssetStatus { asset_id, status } => {
            update_generated_asset_status(&mut next_project, &asset_id, status)
        }
        ProjectAction::UpdateGeneratedAssetReferences {
            asset_id,
            references,
        } => update_generated_asset_references(&mut next_project, &asset_id, references),
        ProjectAction::CompleteGeneratedAsset {
            asset_id,
            outputs,
            completion: _,
            replacement,
        } => complete_generated_asset(&mut next_project, &asset_id, outputs, replacement),
        ProjectAction::ReplaceTimelineItemWithGeneratedOutput { replacement } => {
            replace_timeline_item_with_generated_output(&mut next_project, &replacement)
        }
        ProjectAction::AssignMediaFolder {
            media_id,
            folder_id,
        } => assign_media_folder(&mut next_project, &media_id, folder_id),
        ProjectAction::CreateMediaFolder { folder } => {
            create_media_folder(&mut next_project, folder)
        }
        ProjectAction::RenameMediaFolder { folder_id, name } => {
            rename_media_folder(&mut next_project, &folder_id, &name)
        }
        ProjectAction::DeleteMediaFolder { folder_id } => {
            delete_media_folder(&mut next_project, &folder_id)
        }
        ProjectAction::RenameMedia { media_id, name } => {
            rename_media(&mut next_project, &media_id, &name)
        }
        ProjectAction::DeleteMedia { media_ids } => delete_media(&mut next_project, &media_ids),
        ProjectAction::RemoveTracks { track_ids } => remove_tracks(&mut next_project, &track_ids),
        ProjectAction::UpdateProjectSettings {
            name,
            render_settings,
        } => update_project_settings(&mut next_project, name, render_settings),
        ProjectAction::UpdateRenderSettings { settings } => {
            update_render_settings(&mut next_project, settings)
        }
        ProjectAction::AttachRenderReport { report } => {
            attach_render_report(&mut next_project, report)
        }
        ProjectAction::RecordExportArtifact { artifact } => {
            record_export_artifact(&mut next_project, artifact)
        }
        ProjectAction::RecordJob { job } => record_job(&mut next_project, *job),
        ProjectAction::UpdateJobStatus {
            job_id,
            status,
            updated_at,
            run_id,
        } => update_job_status(&mut next_project, &job_id, status, &updated_at, run_id),
        ProjectAction::UpdateJobProviderRequest {
            job_id,
            provider_request,
        } => update_job_provider_request(&mut next_project, &job_id, provider_request),
        ProjectAction::RecordJobFailure {
            job_id,
            reason,
            updated_at,
            run_id,
        } => record_job_failure(&mut next_project, &job_id, &reason, &updated_at, run_id),
        ProjectAction::UpdateTemplateItems { updates } => {
            update_template_items(&mut next_project, &updates)
        }
        ProjectAction::UpdateTextOverlayItems { updates } => {
            update_text_overlay_items(&mut next_project, &updates)
        }
        ProjectAction::UpdateTemplateOverride { override_ } => {
            update_template_override(&mut next_project, override_)
        }
        ProjectAction::AddTransition {
            track_id,
            transition,
        } => transitions::add_transition(&mut next_project, &track_id, transition),
        ProjectAction::UpdateTransition {
            track_id,
            transition_id,
            kind,
            duration_seconds,
        } => transitions::update_transition(
            &mut next_project,
            &track_id,
            &transition_id,
            kind,
            duration_seconds,
        ),
        ProjectAction::RemoveTransition {
            track_id,
            transition_id,
        } => transitions::remove_transition(&mut next_project, &track_id, &transition_id),
    }?;

    // Keep transitions valid in the same application: re-target transitions
    // whose left clip was cut in two, then drop or clamp invalid ones.
    if next_project.active_timeline_id == overlap_timeline_id {
        transitions::retarget_transitions_to_split_tails(&project.timeline, &mut next_project);
    }
    transitions::maintain_transitions(&mut next_project);
    recalculate_duration(&mut next_project.timeline);
    timeline_library(&mut next_project);
    validate_timeline_dependencies(&next_project)?;
    if next_project.active_timeline_id == overlap_timeline_id
        && timeline_geometry_changed(&project.timeline, &next_project.timeline)
    {
        validate_no_increased_timeline_overlap(
            &project.timeline,
            &next_project.timeline,
            &overlap_item_lineage,
        )?;
    }
    *project = next_project;
    Ok(())
}

fn timeline_item_geometry(timeline: &Timeline) -> BTreeMap<String, (String, f64, f64)> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| {
            track.items.iter().map(|item| {
                (
                    item.id.clone(),
                    (track.id.clone(), item.start_seconds, item.duration_seconds),
                )
            })
        })
        .collect()
}

fn timeline_geometry_changed(before: &Timeline, after: &Timeline) -> bool {
    timeline_item_geometry(before) != timeline_item_geometry(after)
}

fn canonical_overlap_item_id<'a>(
    item_id: &'a str,
    item_lineage: &'a BTreeMap<String, String>,
) -> &'a str {
    item_lineage
        .get(item_id)
        .map(String::as_str)
        .unwrap_or(item_id)
}

fn timeline_overlap_amounts(
    timeline: &Timeline,
    item_lineage: &BTreeMap<String, String>,
) -> BTreeMap<(String, String), f64> {
    let mut overlaps = BTreeMap::new();
    for track in &timeline.tracks {
        for (left_index, left) in track.items.iter().enumerate() {
            for right in track.items.iter().skip(left_index + 1) {
                let overlap = (left.start_seconds + left.duration_seconds)
                    .min(right.start_seconds + right.duration_seconds)
                    - left.start_seconds.max(right.start_seconds);
                if overlap <= 0.001 {
                    continue;
                }

                let left_id = canonical_overlap_item_id(&left.id, item_lineage);
                let right_id = canonical_overlap_item_id(&right.id, item_lineage);
                let (item_id, blocking_item_id) = if left_id <= right_id {
                    (left_id.to_string(), right_id.to_string())
                } else {
                    (right_id.to_string(), left_id.to_string())
                };
                overlaps
                    .entry((item_id, blocking_item_id))
                    .and_modify(|amount: &mut f64| *amount = amount.max(overlap))
                    .or_insert(overlap);
            }
        }
    }
    overlaps
}

fn validate_no_increased_timeline_overlap(
    before: &Timeline,
    after: &Timeline,
    item_lineage: &BTreeMap<String, String>,
) -> Result<(), ProjectActionError> {
    let before_overlaps = timeline_overlap_amounts(before, &BTreeMap::new());
    for track in &after.tracks {
        for (left_index, left) in track.items.iter().enumerate() {
            for right in track.items.iter().skip(left_index + 1) {
                let after_overlap = (left.start_seconds + left.duration_seconds)
                    .min(right.start_seconds + right.duration_seconds)
                    - left.start_seconds.max(right.start_seconds);
                if after_overlap <= 0.001 {
                    continue;
                }
                let left_canonical_id = canonical_overlap_item_id(&left.id, item_lineage);
                let right_canonical_id = canonical_overlap_item_id(&right.id, item_lineage);
                let key = if left_canonical_id <= right_canonical_id {
                    (
                        left_canonical_id.to_string(),
                        right_canonical_id.to_string(),
                    )
                } else {
                    (
                        right_canonical_id.to_string(),
                        left_canonical_id.to_string(),
                    )
                };
                let before_overlap = before_overlaps.get(&key).copied().unwrap_or(0.0);
                if after_overlap > before_overlap + 0.001 {
                    let (item_id, blocking_item_id) = if left.id <= right.id {
                        (left.id.clone(), right.id.clone())
                    } else {
                        (right.id.clone(), left.id.clone())
                    };
                    return Err(ProjectActionError::TrackItemOverlap {
                        track_id: track.id.clone(),
                        item_id,
                        blocking_item_id,
                    });
                }
            }
        }
    }
    Ok(())
}

fn create_track(
    project: &mut VideoProject,
    mut track: TimelineTrack,
    after_track_id: Option<&str>,
) -> Result<(), ProjectActionError> {
    if track.id.trim().is_empty() {
        return Err(ProjectActionError::EmptyTrackId);
    }
    if track.name.trim().is_empty() {
        return Err(ProjectActionError::EmptyTrackName);
    }
    if find_track(&project.timeline, &track.id).is_some() {
        return Err(ProjectActionError::DuplicateTrackId(track.id));
    }

    track.items.clear();
    let insert_index = match after_track_id {
        Some(track_id) => {
            find_track(&project.timeline, track_id)
                .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?
                + 1
        }
        None => project.timeline.tracks.len(),
    };
    project.timeline.tracks.insert(insert_index, track);
    Ok(())
}

fn timeline_library(project: &mut VideoProject) {
    let active_id = project
        .active_timeline_id
        .clone()
        .unwrap_or_else(|| "main".to_string());
    if project.timelines.is_empty() {
        project.timelines.push(ProjectTimeline {
            id: active_id.clone(),
            name: "Timeline 1".to_string(),
            timeline: project.timeline.clone(),
        });
    }
    if let Some(active) = project
        .timelines
        .iter_mut()
        .find(|entry| entry.id == active_id)
    {
        active.timeline = project.timeline.clone();
    }
    project.active_timeline_id = Some(active_id);
}

fn validate_timeline_dependencies(project: &VideoProject) -> Result<(), ProjectActionError> {
    let timeline_ids = project
        .timelines
        .iter()
        .map(|timeline| timeline.id.as_str())
        .collect::<BTreeSet<_>>();
    let edges = project
        .timelines
        .iter()
        .map(|timeline| {
            let references = timeline
                .timeline
                .tracks
                .iter()
                .flat_map(|track| track.items.iter())
                .filter_map(|item| match &item.source {
                    TimelineSource::Timeline { timeline_id } => Some(timeline_id.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (timeline.id.as_str(), references)
        })
        .collect::<BTreeMap<_, _>>();

    for (source_id, references) in &edges {
        for target_id in references {
            if target_id.trim().is_empty() || !timeline_ids.contains(target_id) {
                return Err(ProjectActionError::InvalidEffectParam(format!(
                    "timeline `{source_id}` references missing nested timeline `{target_id}`"
                )));
            }
        }
    }

    fn visit(
        timeline_id: &str,
        edges: &BTreeMap<&str, Vec<&str>>,
        visited: &mut BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
    ) -> Result<(), ProjectActionError> {
        if visited.contains(timeline_id) {
            return Ok(());
        }
        if !visiting.insert(timeline_id.to_string()) {
            return Err(ProjectActionError::InvalidEffectParam(format!(
                "nested timeline cycle includes `{timeline_id}`"
            )));
        }
        for target_id in edges.get(timeline_id).into_iter().flatten() {
            visit(target_id, edges, visited, visiting)?;
        }
        visiting.remove(timeline_id);
        visited.insert(timeline_id.to_string());
        Ok(())
    }

    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for timeline_id in &timeline_ids {
        visit(timeline_id, &edges, &mut visited, &mut visiting)?;
    }
    Ok(())
}

fn validate_timeline_identity(timeline_id: &str, name: &str) -> Result<(), ProjectActionError> {
    if timeline_id.trim().is_empty() || timeline_id.chars().count() > 128 {
        return Err(ProjectActionError::InvalidEffectParam(
            "timeline id must be non-empty and at most 128 characters".to_string(),
        ));
    }
    if name.trim().is_empty() || name.chars().count() > 256 {
        return Err(ProjectActionError::InvalidEffectParam(
            "timeline name must be non-empty and at most 256 characters".to_string(),
        ));
    }
    Ok(())
}

fn create_timeline(
    project: &mut VideoProject,
    timeline_id: &str,
    name: &str,
    duplicate_active: bool,
    source_timeline_id: Option<&str>,
) -> Result<(), ProjectActionError> {
    validate_timeline_identity(timeline_id, name)?;
    timeline_library(project);
    if project
        .timelines
        .iter()
        .any(|entry| entry.id == timeline_id)
    {
        return Err(ProjectActionError::DuplicateTrackId(
            timeline_id.to_string(),
        ));
    }
    let timeline = if duplicate_active {
        match source_timeline_id {
            Some(source_timeline_id) => project
                .timelines
                .iter()
                .find(|entry| entry.id == source_timeline_id)
                .map(|entry| entry.timeline.clone())
                .ok_or_else(|| ProjectActionError::TrackNotFound(source_timeline_id.to_string()))?,
            None => project.timeline.clone(),
        }
    } else {
        Timeline::default_editor_timeline()
    };
    project.timelines.push(ProjectTimeline {
        id: timeline_id.to_string(),
        name: name.trim().to_string(),
        timeline: timeline.clone(),
    });
    project.timeline = timeline;
    project.active_timeline_id = Some(timeline_id.to_string());
    Ok(())
}

fn set_active_timeline(
    project: &mut VideoProject,
    timeline_id: &str,
) -> Result<(), ProjectActionError> {
    timeline_library(project);
    let timeline = project
        .timelines
        .iter()
        .find(|entry| entry.id == timeline_id)
        .map(|entry| entry.timeline.clone())
        .ok_or_else(|| ProjectActionError::TrackNotFound(timeline_id.to_string()))?;
    project.timeline = timeline;
    project.active_timeline_id = Some(timeline_id.to_string());
    Ok(())
}

fn rename_timeline(
    project: &mut VideoProject,
    timeline_id: &str,
    name: &str,
) -> Result<(), ProjectActionError> {
    validate_timeline_identity(timeline_id, name)?;
    timeline_library(project);
    let timeline = project
        .timelines
        .iter_mut()
        .find(|entry| entry.id == timeline_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(timeline_id.to_string()))?;
    timeline.name = name.trim().to_string();
    Ok(())
}

fn delete_timeline(
    project: &mut VideoProject,
    timeline_id: &str,
) -> Result<(), ProjectActionError> {
    timeline_library(project);
    if project.timelines.len() <= 1 {
        return Err(ProjectActionError::InvalidEffectParam(
            "a project must keep at least one timeline".to_string(),
        ));
    }
    if !project
        .timelines
        .iter()
        .any(|entry| entry.id == timeline_id)
    {
        return Err(ProjectActionError::TrackNotFound(timeline_id.to_string()));
    }
    if let Some(owner) = project.timelines.iter().find(|entry| {
        entry.id != timeline_id
            && entry
                .timeline
                .tracks
                .iter()
                .flat_map(|track| &track.items)
                .any(|item| {
                    matches!(
                        &item.source,
                        TimelineSource::Timeline { timeline_id: nested_id } if nested_id == timeline_id
                    )
                })
    }) {
        return Err(ProjectActionError::InvalidEffectParam(format!(
            "timeline `{timeline_id}` is nested by timeline `{}` and cannot be deleted",
            owner.id
        )));
    }

    let active_timeline_id = project
        .active_timeline_id
        .clone()
        .unwrap_or_else(|| "main".to_string());
    project.timelines.retain(|entry| entry.id != timeline_id);
    if timeline_id == active_timeline_id {
        let next_active = project.timelines.first().expect("timeline remains");
        project.active_timeline_id = Some(next_active.id.clone());
        project.timeline = next_active.timeline.clone();
    }
    Ok(())
}

/// Replace a nested-sequence wrapper with independent tracks copied from its
/// referenced timeline. Only unstyled wrappers can be decomposed: transform,
/// effect, speed, fade, or other wrapper properties would otherwise be lost.
fn decompose_timeline_item(
    project: &mut VideoProject,
    item_id: &str,
) -> Result<(), ProjectActionError> {
    let (wrapper_track_index, wrapper_item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let wrapper_track = &project.timeline.tracks[wrapper_track_index];
    if wrapper_track.locked {
        return Err(ProjectActionError::TrackLocked(wrapper_track.id.clone()));
    }
    let wrapper = wrapper_track.items[wrapper_item_index].clone();
    let TimelineSource::Timeline { timeline_id } = &wrapper.source else {
        return Err(ProjectActionError::InvalidEffectParam(format!(
            "timeline item `{item_id}` is not a nested timeline source"
        )));
    };
    if !wrapper.properties.is_empty() {
        return Err(ProjectActionError::InvalidEffectParam(format!(
            "timeline item `{item_id}` has wrapper properties that cannot be preserved by decompose"
        )));
    }
    let nested_timeline = project
        .timelines
        .iter()
        .find(|entry| entry.id == *timeline_id)
        .map(|entry| entry.timeline.clone())
        .ok_or_else(|| {
            ProjectActionError::InvalidEffectParam(format!(
                "timeline item `{item_id}` references missing nested timeline `{timeline_id}`"
            ))
        })?;

    let mut existing_item_ids = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter().map(|item| item.id.clone()))
        .collect::<BTreeSet<_>>();
    existing_item_ids.remove(item_id);
    let mut existing_track_ids = project
        .timeline
        .tracks
        .iter()
        .map(|track| track.id.clone())
        .collect::<BTreeSet<_>>();
    let mut existing_transition_ids = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| {
            track
                .transitions
                .iter()
                .map(|transition| transition.id.clone())
        })
        .collect::<BTreeSet<_>>();
    let wrapper_end_seconds = wrapper.start_seconds + wrapper.duration_seconds;
    let mut decomposed_tracks = Vec::new();

    for nested_track in nested_timeline.tracks {
        let mut items = Vec::new();
        let mut decomposed_item_ids = BTreeMap::new();
        for child in nested_track.items {
            let start_seconds = wrapper.start_seconds + child.start_seconds;
            let end_seconds = (start_seconds + child.duration_seconds).min(wrapper_end_seconds);
            if end_seconds <= start_seconds {
                continue;
            }
            let child_duration_seconds = child.duration_seconds;
            let mut item = child;
            let child_id = std::mem::take(&mut item.id);
            item.id =
                unique_decomposed_id(&format!("{}-{}", item_id, child_id), &mut existing_item_ids);
            decomposed_item_ids.insert(child_id, item.id.clone());
            item.start_seconds = start_seconds;
            item.duration_seconds = end_seconds - start_seconds;
            if end_seconds < start_seconds + child_duration_seconds {
                let speed = timeline_item_number_property(&item, "speed").unwrap_or(1.0);
                if let Some((key, seconds)) = reverse::truncated_tail_source_property(
                    &item,
                    speed,
                    child_duration_seconds,
                    item.duration_seconds,
                ) {
                    item.properties
                        .insert(key.to_string(), serde_json::json!(seconds));
                }
            }
            items.push(item);
        }
        if items.is_empty() {
            continue;
        }
        let track_id = unique_decomposed_id(
            &format!("decomposed-{}-{}", item_id, nested_track.id),
            &mut existing_track_ids,
        );
        // Carry transitions between surviving children; maintenance then drops
        // or clamps any the wrapper's clipping made invalid.
        let mut transitions = Vec::new();
        for transition in nested_track.transitions {
            let (Some(left_item_id), Some(right_item_id)) = (
                decomposed_item_ids.get(&transition.left_item_id),
                decomposed_item_ids.get(&transition.right_item_id),
            ) else {
                continue;
            };
            transitions.push(TimelineTransition {
                id: unique_decomposed_id(
                    &format!("{}-{}", item_id, transition.id),
                    &mut existing_transition_ids,
                ),
                left_item_id: left_item_id.clone(),
                right_item_id: right_item_id.clone(),
                ..transition
            });
        }
        decomposed_tracks.push(TimelineTrack {
            transitions,
            id: track_id,
            name: format!("{}: {}", wrapper.label, nested_track.name),
            kind: nested_track.kind,
            locked: false,
            sync_locked: nested_track.sync_locked,
            enabled: wrapper_track.enabled && nested_track.enabled,
            items,
        });
    }

    project.timeline.tracks[wrapper_track_index]
        .items
        .remove(wrapper_item_index);
    project.timeline.tracks.extend(decomposed_tracks);
    Ok(())
}

fn timeline_item_number_property(item: &TimelineItem, key: &str) -> Option<f64> {
    item.properties.get(key).and_then(serde_json::Value::as_f64)
}

fn unique_decomposed_id(base: &str, existing_ids: &mut BTreeSet<String>) -> String {
    let base = base.trim();
    let mut candidate = base.to_string();
    let mut suffix = 2_u32;
    while existing_ids.contains(&candidate) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    existing_ids.insert(candidate.clone());
    candidate
}

fn set_track_locked(
    project: &mut VideoProject,
    track_id: &str,
    locked: bool,
) -> Result<(), ProjectActionError> {
    let track_index = find_track(&project.timeline, track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?;
    project.timeline.tracks[track_index].locked = locked;
    Ok(())
}

fn reorder_track(
    project: &mut VideoProject,
    track_id: &str,
    target_track_id: &str,
    placement: ProjectActionTrackPlacement,
) -> Result<(), ProjectActionError> {
    let source_index = find_track(&project.timeline, track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?;
    let target_index = find_track(&project.timeline, target_track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(target_track_id.to_string()))?;
    if source_index == target_index {
        return Ok(());
    }
    let source_is_audio = project.timeline.tracks[source_index].kind == TrackKind::Audio;
    let target_is_audio = project.timeline.tracks[target_index].kind == TrackKind::Audio;
    if source_is_audio != target_is_audio {
        return Err(ProjectActionError::InvalidTrackZone);
    }
    let track = project.timeline.tracks.remove(source_index);
    let remaining_target_index = find_track(&project.timeline, target_track_id)
        .expect("reorder target remains after removing a different track");
    let insertion_index = match placement {
        ProjectActionTrackPlacement::Before => remaining_target_index,
        ProjectActionTrackPlacement::After => remaining_target_index + 1,
    };
    project.timeline.tracks.insert(insertion_index, track);
    Ok(())
}

fn set_track_sync_locked(
    project: &mut VideoProject,
    track_id: &str,
    sync_locked: bool,
) -> Result<(), ProjectActionError> {
    let track_index = find_track(&project.timeline, track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?;
    project.timeline.tracks[track_index].sync_locked = sync_locked;
    Ok(())
}

fn set_track_enabled(
    project: &mut VideoProject,
    track_id: &str,
    enabled: bool,
) -> Result<(), ProjectActionError> {
    let track_index = find_track(&project.timeline, track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?;
    project.timeline.tracks[track_index].enabled = enabled;
    Ok(())
}

fn insert_items(
    project: &mut VideoProject,
    target_track_id: &str,
    insert_seconds: f64,
    items: Vec<TimelineItem>,
) -> Result<(), ProjectActionError> {
    if !insert_seconds.is_finite() || insert_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if items.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let target_track_index = find_track(&project.timeline, target_track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(target_track_id.to_string()))?;
    let target_track = &project.timeline.tracks[target_track_index];
    if target_track.locked {
        return Err(ProjectActionError::TrackLocked(target_track.id.clone()));
    }
    for item in &items {
        validate_item_for_add(project, target_track, item)?;
    }

    let inserted_duration = items
        .iter()
        .map(|item| item.duration_seconds)
        .fold(0.0, |total, duration| total + duration);
    let mut next_start_seconds = insert_seconds;
    let mut inserted_items = Vec::with_capacity(items.len());
    for mut item in items {
        item.start_seconds = next_start_seconds;
        next_start_seconds += item.duration_seconds;
        inserted_items.push(item);
    }

    let target_track = &mut project.timeline.tracks[target_track_index];
    split_and_shift_for_insert(target_track, insert_seconds, inserted_duration)?;
    target_track.items.extend(inserted_items);
    sort_track_items(target_track);

    Ok(())
}

fn split_and_shift_for_insert(
    track: &mut TimelineTrack,
    insert_seconds: f64,
    inserted_duration: f64,
) -> Result<(), ProjectActionError> {
    let mut existing_ids = track
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<BTreeSet<_>>();
    let mut appended_ids = BTreeSet::new();
    let mut next_items = Vec::new();
    for mut item in track.items.drain(..) {
        let item_end = item.start_seconds + item.duration_seconds;
        if item.start_seconds < insert_seconds && insert_seconds < item_end {
            let original = item.clone();
            let left_duration = insert_seconds - original.start_seconds;
            let right_duration = item_end - insert_seconds;
            let source_window = reverse::media_source_window(&original, item_speed(&original)?);

            item.duration_seconds = left_duration;
            item.properties = item_properties_for_subrange(
                &original.properties,
                0.0,
                left_duration,
                original.duration_seconds,
            )?;
            let mut right = original.clone();
            right.id = generated_overwrite_item_id(
                &original.id,
                insert_seconds,
                &existing_ids,
                &appended_ids,
            );
            appended_ids.insert(right.id.clone());
            right.start_seconds = insert_seconds + inserted_duration;
            right.duration_seconds = right_duration;
            right.properties = item_properties_for_subrange(
                &original.properties,
                left_duration,
                original.duration_seconds,
                original.duration_seconds,
            )?;
            if let Some(window) = source_window {
                let duration = original.duration_seconds;
                let (left_in, left_out) = window.subrange(0.0, left_duration, duration);
                let (right_in, right_out) = window.subrange(left_duration, duration, duration);
                set_item_source_range(&mut item, left_in, left_out);
                set_item_source_range(&mut right, right_in, right_out);
            }
            next_items.push(item);
            next_items.push(right);
            existing_ids.insert(original.id);
        } else {
            if item.start_seconds >= insert_seconds {
                item.start_seconds += inserted_duration;
            }
            next_items.push(item);
        }
    }
    track.items = next_items;
    Ok(())
}

fn add_items(
    project: &mut VideoProject,
    target_track_id: &str,
    items: Vec<TimelineItem>,
) -> Result<(), ProjectActionError> {
    if items.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }
    let target_track_index = find_track(&project.timeline, target_track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(target_track_id.to_string()))?;
    let target_track = &project.timeline.tracks[target_track_index];

    if target_track.locked {
        return Err(ProjectActionError::TrackLocked(target_track.id.clone()));
    }
    for item in &items {
        validate_item_for_add(project, target_track, item)?;
    }

    let target_track = &mut project.timeline.tracks[target_track_index];
    clear_add_item_landing_regions(target_track, &items)?;
    target_track.items.extend(items);
    sort_track_items(target_track);
    Ok(())
}

fn clear_add_item_landing_regions(
    target_track: &mut TimelineTrack,
    items: &[TimelineItem],
) -> Result<(), ProjectActionError> {
    let mut ordered_ranges = items
        .iter()
        .map(|item| {
            (
                item.start_seconds,
                item.start_seconds + item.duration_seconds,
            )
        })
        .collect::<Vec<_>>();
    ordered_ranges.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut existing_ids = target_track
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<BTreeSet<_>>();
    let mut appended_ids = BTreeSet::new();
    for (start_seconds, end_seconds) in ordered_ranges {
        clear_add_item_landing_region(
            target_track,
            start_seconds,
            end_seconds,
            &mut existing_ids,
            &mut appended_ids,
        )?;
    }
    Ok(())
}

fn clear_add_item_landing_region(
    target_track: &mut TimelineTrack,
    start_seconds: f64,
    end_seconds: f64,
    existing_ids: &mut BTreeSet<String>,
    appended_ids: &mut BTreeSet<String>,
) -> Result<(), ProjectActionError> {
    let mut next_items = Vec::new();
    for item in target_track.items.drain(..) {
        let item_start = item.start_seconds;
        let item_end = item.start_seconds + item.duration_seconds;

        if item_end <= start_seconds || item_start >= end_seconds {
            next_items.push(item);
            continue;
        }
        if item_start >= start_seconds && item_end <= end_seconds {
            existing_ids.insert(item.id);
            continue;
        }

        let left_duration = (start_seconds - item_start).max(0.0);
        let right_duration = (item_end - end_seconds).max(0.0);
        let source_window = reverse::media_source_window(&item, item_speed(&item)?);

        if left_duration > 0.0 {
            let mut left_item = item.clone();
            left_item.duration_seconds = left_duration;
            left_item.properties = item_properties_for_subrange(
                &item.properties,
                0.0,
                left_duration,
                item.duration_seconds,
            )?;
            if let Some(window) = source_window {
                let (source_in, source_out) =
                    window.subrange(0.0, left_duration, item.duration_seconds);
                set_item_source_range(&mut left_item, source_in, source_out);
            }
            next_items.push(left_item);
        }

        if right_duration > 0.0 {
            let mut right_item = item.clone();
            right_item.start_seconds = end_seconds;
            right_item.duration_seconds = right_duration;
            let skipped_timeline_seconds = end_seconds - item_start;
            right_item.properties = item_properties_for_subrange(
                &item.properties,
                skipped_timeline_seconds,
                item.duration_seconds,
                item.duration_seconds,
            )?;
            if left_duration > 0.0 {
                right_item.id =
                    generated_overwrite_item_id(&item.id, end_seconds, existing_ids, appended_ids);
                appended_ids.insert(right_item.id.clone());
            }
            if let Some(window) = source_window {
                let (source_in, source_out) = window.subrange(
                    skipped_timeline_seconds,
                    item.duration_seconds,
                    item.duration_seconds,
                );
                set_item_source_range(&mut right_item, source_in, source_out);
            }
            next_items.push(right_item);
        }
        existing_ids.insert(item.id);
    }
    target_track.items = next_items;
    sort_track_items(target_track);
    Ok(())
}

fn remove_items(project: &mut VideoProject, item_ids: &[String]) -> Result<(), ProjectActionError> {
    if item_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }
    for item_id in item_ids {
        let (track_index, _) = find_item(&project.timeline, item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.clone()))?;
        if project.timeline.tracks[track_index].locked {
            return Err(ProjectActionError::TrackLocked(
                project.timeline.tracks[track_index].id.clone(),
            ));
        }
    }

    let item_id_set = item_ids.iter().cloned().collect::<BTreeSet<_>>();
    for track in &mut project.timeline.tracks {
        track.items.retain(|item| !item_id_set.contains(&item.id));
    }

    Ok(())
}

fn move_items(
    project: &mut VideoProject,
    moves: &[ProjectActionMove],
) -> Result<(), ProjectActionError> {
    if moves.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for item_move in moves {
        if !item_move.start_seconds.is_finite() || item_move.start_seconds < 0.0 {
            return Err(ProjectActionError::NegativeTime);
        }
        let (source_track_index, item_index) = find_item(&project.timeline, &item_move.item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_move.item_id.clone()))?;
        let target_track_index = find_track(&project.timeline, &item_move.target_track_id)
            .ok_or_else(|| ProjectActionError::TrackNotFound(item_move.target_track_id.clone()))?;

        let source_track = &project.timeline.tracks[source_track_index];
        if source_track.locked {
            return Err(ProjectActionError::TrackLocked(source_track.id.clone()));
        }
        let target_track = &project.timeline.tracks[target_track_index];
        if target_track.locked {
            return Err(ProjectActionError::TrackLocked(target_track.id.clone()));
        }

        let item_kind = project.timeline.tracks[source_track_index].items[item_index]
            .kind
            .clone();
        let target_kind = target_track.kind.clone();
        if !item_allowed_on_track(&item_kind, &target_kind) {
            return Err(ProjectActionError::TrackTypeMismatch {
                item_kind,
                track_kind: target_kind,
            });
        }
    }

    for item_move in moves {
        let (source_track_index, item_index) =
            find_item(&project.timeline, &item_move.item_id).expect("move prevalidated item");
        let target_track_index =
            find_track(&project.timeline, &item_move.target_track_id).expect("prevalidated track");
        let mut item = project.timeline.tracks[source_track_index]
            .items
            .remove(item_index);
        item.start_seconds = item_move.start_seconds;
        project.timeline.tracks[target_track_index].items.push(item);
        sort_track_items(&mut project.timeline.tracks[target_track_index]);
    }

    Ok(())
}

fn reorder_items(
    project: &mut VideoProject,
    reorder: &ProjectActionReorder,
) -> Result<(), ProjectActionError> {
    if reorder.item_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }
    if !reorder.start_seconds.is_finite()
        || reorder.start_seconds < 0.0
        || !reorder.gap_seconds.is_finite()
        || reorder.gap_seconds < 0.0
    {
        return Err(ProjectActionError::NegativeTime);
    }

    let target_track_index = find_track(&project.timeline, &reorder.target_track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(reorder.target_track_id.clone()))?;
    let target_track = &project.timeline.tracks[target_track_index];
    if target_track.locked {
        return Err(ProjectActionError::TrackLocked(target_track.id.clone()));
    }

    let mut seen_item_ids = BTreeSet::new();
    for item_id in &reorder.item_ids {
        if !seen_item_ids.insert(item_id.clone()) {
            return Err(ProjectActionError::DuplicateItemId(item_id.clone()));
        }
        let (source_track_index, item_index) = find_item(&project.timeline, item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.clone()))?;
        let source_track = &project.timeline.tracks[source_track_index];
        if source_track.locked {
            return Err(ProjectActionError::TrackLocked(source_track.id.clone()));
        }

        let item_kind = source_track.items[item_index].kind.clone();
        let target_kind = target_track.kind.clone();
        if !item_allowed_on_track(&item_kind, &target_kind) {
            return Err(ProjectActionError::TrackTypeMismatch {
                item_kind,
                track_kind: target_kind,
            });
        }
    }

    let mut ordered_items = Vec::with_capacity(reorder.item_ids.len());
    for item_id in &reorder.item_ids {
        let (source_track_index, item_index) =
            find_item(&project.timeline, item_id).expect("reorder prevalidated item");
        ordered_items.push(
            project.timeline.tracks[source_track_index]
                .items
                .remove(item_index),
        );
    }

    let mut next_start_seconds = reorder.start_seconds;
    for mut item in ordered_items {
        item.start_seconds = next_start_seconds;
        next_start_seconds += item.duration_seconds + reorder.gap_seconds;
        project.timeline.tracks[target_track_index].items.push(item);
    }
    sort_track_items(&mut project.timeline.tracks[target_track_index]);

    Ok(())
}

fn resize_items(
    project: &mut VideoProject,
    resizes: &[ProjectActionResize],
) -> Result<(), ProjectActionError> {
    if resizes.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for item_resize in resizes {
        if !item_resize.duration_seconds.is_finite() || item_resize.duration_seconds <= 0.0 {
            return Err(ProjectActionError::NonPositiveDuration);
        }
        let (track_index, _) = find_item(&project.timeline, &item_resize.item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_resize.item_id.clone()))?;
        if project.timeline.tracks[track_index].locked {
            return Err(ProjectActionError::TrackLocked(
                project.timeline.tracks[track_index].id.clone(),
            ));
        }
    }

    for item_resize in resizes {
        let (track_index, item_index) =
            find_item(&project.timeline, &item_resize.item_id).expect("resize prevalidated item");
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        let previous_duration_seconds = item.duration_seconds;
        item.duration_seconds = item_resize.duration_seconds;
        rescale_item_keyframes(
            &mut item.properties,
            previous_duration_seconds,
            item_resize.duration_seconds,
        );
        sort_track_items(&mut project.timeline.tracks[track_index]);
    }

    Ok(())
}

fn rescale_item_keyframes(
    properties: &mut BTreeMap<String, serde_json::Value>,
    previous_duration_seconds: f64,
    next_duration_seconds: f64,
) {
    if !previous_duration_seconds.is_finite()
        || previous_duration_seconds <= 0.0
        || !next_duration_seconds.is_finite()
        || next_duration_seconds <= 0.0
    {
        return;
    }

    let Some(keyframes) = properties
        .get_mut("keyframes")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let scale = next_duration_seconds / previous_duration_seconds;
    for property_keyframes in keyframes.values_mut() {
        let Some(property_keyframes) = property_keyframes.as_array_mut() else {
            continue;
        };
        for keyframe in property_keyframes {
            let Some(keyframe) = keyframe.as_object_mut() else {
                continue;
            };
            let Some(at_seconds) = keyframe
                .get("atSeconds")
                .and_then(serde_json::Value::as_f64)
                .filter(|at_seconds| at_seconds.is_finite())
            else {
                continue;
            };
            keyframe.insert(
                "atSeconds".to_string(),
                serde_json::json!((at_seconds * scale).clamp(0.0, next_duration_seconds)),
            );
        }
    }
}

fn trim_items(
    project: &mut VideoProject,
    trims: &[ProjectActionTrim],
) -> Result<(), ProjectActionError> {
    if trims.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for trim in trims {
        validate_trim(project, trim)?;
    }

    for trim in trims {
        let (track_index, item_index) =
            find_item(&project.timeline, &trim.item_id).expect("trim prevalidated item");
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        item.start_seconds = trim.start_seconds;
        item.duration_seconds = trim.duration_seconds;
        if let (Some(source_in), Some(source_out)) = (trim.source_in, trim.source_out) {
            item.properties
                .insert("sourceIn".to_string(), serde_json::json!(source_in));
            item.properties
                .insert("sourceOut".to_string(), serde_json::json!(source_out));
        }
        sort_track_items(&mut project.timeline.tracks[track_index]);
    }

    Ok(())
}

fn ripple_trim_item(
    project: &mut VideoProject,
    item_id: &str,
    edge: ProjectActionTrimEdge,
    delta_seconds: f64,
    propagate_linked: bool,
    sync_locked_track_ids: &[String],
) -> Result<(), ProjectActionError> {
    if !delta_seconds.is_finite() {
        return Err(ProjectActionError::NegativeTime);
    }
    if delta_seconds == 0.0 {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    let (lead_track_index, lead_item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let lead = &project.timeline.tracks[lead_track_index].items[lead_item_index];
    let lead_end = lead.start_seconds + lead.duration_seconds;
    let link_group_id = lead
        .properties
        .get("linkGroupId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let duration_delta = match edge {
        ProjectActionTrimEdge::Left => -delta_seconds,
        ProjectActionTrimEdge::Right => delta_seconds,
    };

    let mut target_ids = BTreeSet::from([item_id.to_string()]);
    if propagate_linked {
        if let Some(link_group_id) = link_group_id.as_deref() {
            for track in &project.timeline.tracks {
                for item in &track.items {
                    if item
                        .properties
                        .get("linkGroupId")
                        .and_then(serde_json::Value::as_str)
                        == Some(link_group_id)
                    {
                        target_ids.insert(item.id.clone());
                    }
                }
            }
        }
    }

    let mut target_locations = Vec::new();
    let mut affected_boundaries = BTreeMap::<usize, f64>::new();
    for target_id in &target_ids {
        let (track_index, item_index) = find_item(&project.timeline, target_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(target_id.clone()))?;
        let track = &project.timeline.tracks[track_index];
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
        let item = &track.items[item_index];
        let item_end = item.start_seconds + item.duration_seconds;
        affected_boundaries
            .entry(track_index)
            .and_modify(|boundary| *boundary = boundary.min(item_end))
            .or_insert(item_end);
        target_locations.push((track_index, item_index));
    }

    let mut seen_sync_tracks = BTreeSet::new();
    for track_id in sync_locked_track_ids {
        if track_id.trim().is_empty() || !seen_sync_tracks.insert(track_id.clone()) {
            return Err(ProjectActionError::DuplicateTrackId(track_id.clone()));
        }
        let track_index = find_track(&project.timeline, track_id)
            .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.clone()))?;
        let track = &project.timeline.tracks[track_index];
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
        affected_boundaries.entry(track_index).or_insert(lead_end);
    }

    let media_durations = project
        .media
        .iter()
        .map(|media| (media.id.clone(), media.duration_seconds))
        .collect::<BTreeMap<_, _>>();
    for (track_index, item_index) in target_locations {
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        let original_duration = item.duration_seconds;
        let next_duration = item.duration_seconds + duration_delta;
        if !next_duration.is_finite() || next_duration <= 0.0 {
            return Err(ProjectActionError::NonPositiveDuration);
        }
        let speed = item
            .properties
            .get("speed")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1.0);
        if !speed.is_finite() || speed <= 0.0 {
            return Err(ProjectActionError::InvalidVisualClipSpeed(item.id.clone()));
        }
        let (subrange_start, subrange_end) = match edge {
            ProjectActionTrimEdge::Left => (original_duration - next_duration, original_duration),
            ProjectActionTrimEdge::Right => (0.0, next_duration),
        };
        item.properties = item_properties_for_subrange(
            &item.properties,
            subrange_start,
            subrange_end,
            original_duration,
        )?;
        let configured_source_in = item
            .properties
            .get("sourceIn")
            .and_then(serde_json::Value::as_f64);
        let configured_source_out = item
            .properties
            .get("sourceOut")
            .and_then(serde_json::Value::as_f64);
        let source_range = match (configured_source_in, configured_source_out) {
            (Some(source_in), Some(source_out)) => Some((source_in, source_out)),
            (None, None) => match &item.source {
                TimelineSource::Media { .. } => Some((0.0, item.duration_seconds * speed)),
                _ => None,
            },
            _ => {
                return Err(ProjectActionError::SourceRangeDurationMismatch(
                    item.id.clone(),
                ))
            }
        };
        if let Some((source_in, source_out)) = source_range {
            let (source_in, source_out) = SourceWindow {
                source_in,
                source_out,
                speed,
                reverse: reverse::is_reversed(item),
            }
            .trimmed(edge, duration_delta);
            if !source_in.is_finite() || source_in < 0.0 {
                return Err(ProjectActionError::NegativeTime);
            }
            if !source_out.is_finite() || source_out <= source_in {
                return Err(ProjectActionError::NonPositiveDuration);
            }
            if !nearly_equal(source_out - source_in, next_duration * speed) {
                return Err(ProjectActionError::SourceRangeDurationMismatch(
                    item.id.clone(),
                ));
            }
            if let TimelineSource::Media { media_id } = &item.source {
                let media_duration = media_durations
                    .get(media_id)
                    .ok_or_else(|| ProjectActionError::MissingMedia(media_id.clone()))?;
                if source_out > *media_duration {
                    return Err(ProjectActionError::SourceRangeOutsideMedia(
                        media_id.clone(),
                    ));
                }
            }
            set_item_source_range(item, source_in, source_out);
        }
        item.duration_seconds = next_duration;
    }

    for (track_index, boundary) in &affected_boundaries {
        for item in &mut project.timeline.tracks[*track_index].items {
            if !target_ids.contains(&item.id) && item.start_seconds >= *boundary {
                item.start_seconds += duration_delta;
                if item.start_seconds < 0.0 {
                    return Err(ProjectActionError::NegativeTime);
                }
            }
        }
        sort_track_items(&mut project.timeline.tracks[*track_index]);
        validate_ripple_trim_track_collisions(&project.timeline.tracks[*track_index])?;
    }
    project.timeline.duration_seconds = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.start_seconds + item.duration_seconds)
        .fold(0.0_f64, f64::max);
    Ok(())
}

fn validate_ripple_trim_track_collisions(track: &TimelineTrack) -> Result<(), ProjectActionError> {
    for items in track.items.windows(2) {
        let left = &items[0];
        let right = &items[1];
        if left.start_seconds + left.duration_seconds > right.start_seconds + 0.000_001 {
            return Err(ProjectActionError::RippleTrimCollision {
                track_id: track.id.clone(),
                left_item_id: left.id.clone(),
                right_item_id: right.id.clone(),
            });
        }
    }
    Ok(())
}

fn split_items(
    project: &mut VideoProject,
    splits: &[ProjectActionSplit],
) -> Result<(), ProjectActionError> {
    if splits.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let mut new_item_ids = BTreeSet::new();
    for split in splits {
        if !split.split_seconds.is_finite() || split.split_seconds < 0.0 {
            return Err(ProjectActionError::NegativeTime);
        }
        if find_item(&project.timeline, &split.new_item_id).is_some()
            || !new_item_ids.insert(split.new_item_id.clone())
        {
            return Err(ProjectActionError::DuplicateItemId(
                split.new_item_id.clone(),
            ));
        }

        let (track_index, item_index) = find_item(&project.timeline, &split.item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(split.item_id.clone()))?;
        let track = &project.timeline.tracks[track_index];
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }

        let item = &track.items[item_index];
        let item_end_seconds = item.start_seconds + item.duration_seconds;
        if split.split_seconds <= item.start_seconds || split.split_seconds >= item_end_seconds {
            return Err(ProjectActionError::SplitPointOutsideItem(
                split.item_id.clone(),
            ));
        }
    }

    for split in splits {
        let (track_index, item_index) =
            find_item(&project.timeline, &split.item_id).expect("split prevalidated item");
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        let original_start_seconds = item.start_seconds;
        let original_duration_seconds = item.duration_seconds;
        let left_duration_seconds = split.split_seconds - original_start_seconds;
        let right_duration_seconds = original_duration_seconds - left_duration_seconds;
        let original_properties = item.properties.clone();
        let source_window = reverse::media_source_window(item, item_speed(item)?);

        let mut right_item = item.clone();
        right_item.id = split.new_item_id.clone();
        right_item.start_seconds = split.split_seconds;
        right_item.duration_seconds = right_duration_seconds;
        item.duration_seconds = left_duration_seconds;
        item.properties = item_properties_for_subrange(
            &original_properties,
            0.0,
            left_duration_seconds,
            original_duration_seconds,
        )?;
        right_item.properties = item_properties_for_subrange(
            &original_properties,
            left_duration_seconds,
            original_duration_seconds,
            original_duration_seconds,
        )?;
        if let Some(window) = source_window {
            let duration = original_duration_seconds;
            let (left_in, left_out) = window.subrange(0.0, left_duration_seconds, duration);
            let (right_in, right_out) = window.subrange(left_duration_seconds, duration, duration);
            set_item_source_range(item, left_in, left_out);
            set_item_source_range(&mut right_item, right_in, right_out);
        }

        project.timeline.tracks[track_index].items.push(right_item);
        sort_track_items(&mut project.timeline.tracks[track_index]);
    }

    Ok(())
}

fn ripple_delete_ranges(
    project: &mut VideoProject,
    ranges: &[ProjectActionRippleDeleteRange],
) -> Result<(), ProjectActionError> {
    if ranges.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for range in ranges {
        validate_ripple_delete_range(project, range)?;
    }

    let mut ordered_ranges = ranges.to_vec();
    ordered_ranges.sort_by(|left, right| {
        right
            .start_seconds
            .partial_cmp(&left.start_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for range in ordered_ranges {
        let track_ids = range.track_ids.iter().cloned().collect::<BTreeSet<_>>();
        let range_duration = range.end_seconds - range.start_seconds;
        for track in &mut project.timeline.tracks {
            if !track_ids.contains(&track.id) {
                continue;
            }

            let mut next_items = Vec::new();
            let mut appended_ids = BTreeSet::new();
            let mut existing_ids = track
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<BTreeSet<_>>();
            for item in track.items.drain(..) {
                let item_start = item.start_seconds;
                let item_end = item.start_seconds + item.duration_seconds;

                if item_end <= range.start_seconds {
                    next_items.push(item);
                    continue;
                }
                if item_start >= range.end_seconds {
                    let mut shifted_item = item;
                    shifted_item.start_seconds -= range_duration;
                    next_items.push(shifted_item);
                    continue;
                }
                if item_start >= range.start_seconds && item_end <= range.end_seconds {
                    continue;
                }

                let left_duration = (range.start_seconds - item_start).max(0.0);
                let right_duration = (item_end - range.end_seconds).max(0.0);
                let source_window = reverse::media_source_window(&item, item_speed(&item)?);

                if left_duration > 0.0 {
                    let mut left_item = item.clone();
                    left_item.duration_seconds = left_duration;
                    left_item.properties = item_properties_for_subrange(
                        &item.properties,
                        0.0,
                        left_duration,
                        item.duration_seconds,
                    )?;
                    if let Some(window) = source_window {
                        let (source_in, source_out) =
                            window.subrange(0.0, left_duration, item.duration_seconds);
                        set_item_source_range(&mut left_item, source_in, source_out);
                    }
                    next_items.push(left_item);
                }

                if right_duration > 0.0 {
                    let mut right_item = item.clone();
                    right_item.start_seconds = range.start_seconds;
                    right_item.duration_seconds = right_duration;
                    let skipped_timeline_seconds = range.end_seconds - item_start;
                    right_item.properties = item_properties_for_subrange(
                        &item.properties,
                        skipped_timeline_seconds,
                        item.duration_seconds,
                        item.duration_seconds,
                    )?;
                    if left_duration > 0.0 {
                        right_item.id = generated_ripple_item_id(
                            &item.id,
                            range.end_seconds,
                            &existing_ids,
                            &appended_ids,
                        );
                        appended_ids.insert(right_item.id.clone());
                    }
                    if let Some(window) = source_window {
                        let (source_in, source_out) = window.subrange(
                            skipped_timeline_seconds,
                            item.duration_seconds,
                            item.duration_seconds,
                        );
                        set_item_source_range(&mut right_item, source_in, source_out);
                    }
                    next_items.push(right_item);
                }
                existing_ids.insert(item.id);
            }
            track.items = next_items;
            sort_track_items(track);
        }
    }

    Ok(())
}

fn validate_ripple_delete_range(
    project: &VideoProject,
    range: &ProjectActionRippleDeleteRange,
) -> Result<(), ProjectActionError> {
    if !range.start_seconds.is_finite()
        || !range.end_seconds.is_finite()
        || range.start_seconds < 0.0
        || range.end_seconds <= range.start_seconds
    {
        return Err(ProjectActionError::NegativeTime);
    }
    if range.track_ids.is_empty()
        || range
            .track_ids
            .iter()
            .any(|track_id| track_id.trim().is_empty())
    {
        return Err(ProjectActionError::EmptyItems);
    }

    let mut seen = BTreeSet::new();
    for track_id in &range.track_ids {
        if !seen.insert(track_id.clone()) {
            return Err(ProjectActionError::DuplicateTrackId(track_id.clone()));
        }
        let track_index = find_track(&project.timeline, track_id)
            .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.clone()))?;
        if project.timeline.tracks[track_index].locked {
            return Err(ProjectActionError::TrackLocked(track_id.clone()));
        }
    }

    Ok(())
}

fn validate_trim(
    project: &VideoProject,
    trim: &ProjectActionTrim,
) -> Result<(), ProjectActionError> {
    if !trim.start_seconds.is_finite() || trim.start_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !trim.duration_seconds.is_finite() || trim.duration_seconds <= 0.0 {
        return Err(ProjectActionError::NonPositiveDuration);
    }

    let (track_index, item_index) = find_item(&project.timeline, &trim.item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(trim.item_id.clone()))?;
    let track = &project.timeline.tracks[track_index];
    if track.locked {
        return Err(ProjectActionError::TrackLocked(track.id.clone()));
    }

    let item = &track.items[item_index];
    match (trim.source_in, trim.source_out) {
        (None, None) => Ok(()),
        (Some(source_in), Some(source_out)) => {
            validate_trim_source_range(project, item, trim, source_in, source_out)
        }
        _ => Err(ProjectActionError::SourceRangeDurationMismatch(
            trim.item_id.clone(),
        )),
    }
}

fn validate_trim_source_range(
    project: &VideoProject,
    item: &TimelineItem,
    trim: &ProjectActionTrim,
    source_in: f64,
    source_out: f64,
) -> Result<(), ProjectActionError> {
    if !source_in.is_finite() || source_in < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !source_out.is_finite() || source_out <= source_in {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    let speed = item_speed(item)?;
    if !nearly_equal(source_out - source_in, trim.duration_seconds * speed) {
        return Err(ProjectActionError::SourceRangeDurationMismatch(
            trim.item_id.clone(),
        ));
    }
    if let TimelineSource::Media { media_id } = &item.source {
        let media = project
            .media
            .iter()
            .find(|media| media.id == *media_id)
            .ok_or_else(|| ProjectActionError::MissingMedia(media_id.clone()))?;
        if source_out > media.duration_seconds {
            return Err(ProjectActionError::SourceRangeOutsideMedia(
                media_id.clone(),
            ));
        }
    }

    Ok(())
}

fn edit_caption_text(
    project: &mut VideoProject,
    item_id: &str,
    text: &str,
) -> Result<(), ProjectActionError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ProjectActionError::EmptyCaptionText);
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if item.kind != TimelineItemKind::Caption {
        return Err(ProjectActionError::NotCaptionItem(item_id.to_string()));
    }
    let TimelineSource::Text { text: caption_text } = &mut item.source else {
        return Err(ProjectActionError::NotCaptionItem(item_id.to_string()));
    };

    *caption_text = text.to_string();
    item.properties
        .insert("textEdited".to_string(), serde_json::json!(true));
    Ok(())
}

fn edit_text_item(
    project: &mut VideoProject,
    item_id: &str,
    text: &str,
) -> Result<(), ProjectActionError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ProjectActionError::EmptyTextItemText);
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    let TimelineSource::Text { text: item_text } = &mut item.source else {
        return Err(ProjectActionError::NotTextItem(item_id.to_string()));
    };

    *item_text = text.to_string();
    item.label = text.to_string();
    if item.properties.contains_key("text") {
        item.properties
            .insert("text".to_string(), serde_json::json!(text));
    }
    item.properties
        .insert("textEdited".to_string(), serde_json::json!(true));
    Ok(())
}

fn update_audio_fade_out(
    project: &mut VideoProject,
    item_id: &str,
    fade_out_seconds: f64,
) -> Result<(), ProjectActionError> {
    if !fade_out_seconds.is_finite() || fade_out_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if item.kind != TimelineItemKind::AudioClip {
        return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
    }

    if fade_out_seconds > 0.0 {
        item.properties.insert(
            "fadeOutSeconds".to_string(),
            serde_json::json!(fade_out_seconds),
        );
    } else {
        item.properties.remove("fadeOutSeconds");
    }

    Ok(())
}

fn update_audio_fades(
    project: &mut VideoProject,
    item_id: &str,
    fade_in_seconds: f64,
    fade_out_seconds: f64,
) -> Result<(), ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if item.kind != TimelineItemKind::AudioClip {
        return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
    }
    if !fade_in_seconds.is_finite()
        || !fade_out_seconds.is_finite()
        || fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > item.duration_seconds
    {
        return Err(ProjectActionError::InvalidAudioFades(item_id.to_string()));
    }

    for (key, value) in [
        ("fadeInSeconds", fade_in_seconds),
        ("fadeOutSeconds", fade_out_seconds),
    ] {
        if value > 0.0 {
            item.properties
                .insert(key.to_string(), serde_json::json!(value));
        } else {
            item.properties.remove(key);
        }
    }
    Ok(())
}

fn update_audio_volume(
    project: &mut VideoProject,
    item_id: &str,
    volume_db: Option<f64>,
) -> Result<(), ProjectActionError> {
    if let Some(volume_db) = volume_db {
        if !volume_db.is_finite() || !(-60.0..=24.0).contains(&volume_db) {
            return Err(ProjectActionError::InvalidAudioVolume(item_id.to_string()));
        }
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if item.kind != TimelineItemKind::AudioClip {
        return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
    }

    if let Some(volume_db) = volume_db {
        item.properties
            .insert("volumeDb".to_string(), serde_json::json!(volume_db));
    } else {
        item.properties.remove("volumeDb");
    }

    Ok(())
}

fn update_audio_sync(
    project: &mut VideoProject,
    item_id: &str,
    sync: ProjectActionAudioSync,
) -> Result<(), ProjectActionError> {
    if sync.reference_clip_id.trim().is_empty()
        || !sync.offset_seconds.is_finite()
        || !sync.confidence.is_finite()
        || !(0.0..=1.0).contains(&sync.confidence)
        || !sync.synced_at_start_seconds.is_finite()
        || sync.synced_at_start_seconds < 0.0
    {
        return Err(ProjectActionError::InvalidAudioSync(item_id.to_string()));
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if item.kind != TimelineItemKind::AudioClip {
        return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
    }

    item.properties.insert(
        "audioSync".to_string(),
        serde_json::to_value(sync).expect("audio sync serialize"),
    );

    Ok(())
}

fn update_visual_clip_opacity(
    project: &mut VideoProject,
    item_id: &str,
    opacity: f64,
) -> Result<(), ProjectActionError> {
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return Err(ProjectActionError::InvalidVisualClipOpacity(
            item_id.to_string(),
        ));
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if !is_visual_clip_kind(&item.kind) {
        return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
    }

    if opacity < 1.0 {
        item.properties
            .insert("opacity".to_string(), serde_json::json!(opacity));
    } else {
        item.properties.remove("opacity");
    }

    Ok(())
}

fn update_visual_clip_transform(
    project: &mut VideoProject,
    item_id: &str,
    transform: ProjectActionVisualTransform,
) -> Result<(), ProjectActionError> {
    validate_visual_transform(&transform)?;

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if !is_visual_clip_kind(&item.kind) {
        return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
    }

    let mut transform_object = item
        .properties
        .get("transform")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    insert_optional_transform_number(&mut transform_object, "centerX", transform.center_x);
    insert_optional_transform_number(&mut transform_object, "centerY", transform.center_y);
    insert_optional_transform_number(&mut transform_object, "width", transform.width);
    insert_optional_transform_number(&mut transform_object, "height", transform.height);
    if let Some(flip_horizontal) = transform.flip_horizontal {
        transform_object.insert(
            "flipHorizontal".to_string(),
            serde_json::json!(flip_horizontal),
        );
    }
    if let Some(flip_vertical) = transform.flip_vertical {
        transform_object.insert("flipVertical".to_string(), serde_json::json!(flip_vertical));
    }
    item.properties.insert(
        "transform".to_string(),
        serde_json::Value::Object(transform_object),
    );

    Ok(())
}

fn validate_visual_transform(
    transform: &ProjectActionVisualTransform,
) -> Result<(), ProjectActionError> {
    if transform.center_x.is_none()
        && transform.center_y.is_none()
        && transform.width.is_none()
        && transform.height.is_none()
        && transform.flip_horizontal.is_none()
        && transform.flip_vertical.is_none()
    {
        return Err(ProjectActionError::InvalidVisualClipTransform(
            "transform must include at least one field".to_string(),
        ));
    }
    for (field, value) in [
        ("centerX", transform.center_x),
        ("centerY", transform.center_y),
    ] {
        if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
            return Err(ProjectActionError::InvalidVisualClipTransform(format!(
                "{field} must be finite and between 0.0 and 1.0"
            )));
        }
    }
    for (field, value) in [("width", transform.width), ("height", transform.height)] {
        if value.is_some_and(|value| !value.is_finite() || value <= 0.0 || value > 1.0) {
            return Err(ProjectActionError::InvalidVisualClipTransform(format!(
                "{field} must be finite and greater than 0.0 up to 1.0"
            )));
        }
    }
    Ok(())
}

fn insert_optional_transform_number(
    transform_object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<f64>,
) {
    if let Some(value) = value {
        transform_object.insert(key.to_string(), serde_json::json!(value));
    }
}

fn update_visual_clip_crop(
    project: &mut VideoProject,
    item_id: &str,
    crop: ProjectActionVisualCrop,
) -> Result<(), ProjectActionError> {
    if crop.crop_top.is_none()
        && crop.crop_right.is_none()
        && crop.crop_bottom.is_none()
        && crop.crop_left.is_none()
    {
        return Err(ProjectActionError::InvalidVisualClipCrop(
            "crop must include at least one edge".to_string(),
        ));
    }

    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if !is_visual_clip_kind(&item.kind) {
        return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
    }

    let existing_crop = |key: &str| {
        item.properties
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
    };
    let crop_top = crop.crop_top.unwrap_or_else(|| existing_crop("cropTop"));
    let crop_right = crop
        .crop_right
        .unwrap_or_else(|| existing_crop("cropRight"));
    let crop_bottom = crop
        .crop_bottom
        .unwrap_or_else(|| existing_crop("cropBottom"));
    let crop_left = crop.crop_left.unwrap_or_else(|| existing_crop("cropLeft"));
    validate_visual_crop(crop_top, crop_right, crop_bottom, crop_left)?;

    for (key, value) in [
        ("cropTop", crop_top),
        ("cropRight", crop_right),
        ("cropBottom", crop_bottom),
        ("cropLeft", crop_left),
    ] {
        if value > 0.0 {
            item.properties
                .insert(key.to_string(), serde_json::json!(value));
        } else {
            item.properties.remove(key);
        }
    }

    Ok(())
}

fn validate_visual_crop(
    crop_top: f64,
    crop_right: f64,
    crop_bottom: f64,
    crop_left: f64,
) -> Result<(), ProjectActionError> {
    for (edge, value) in [
        ("cropTop", crop_top),
        ("cropRight", crop_right),
        ("cropBottom", crop_bottom),
        ("cropLeft", crop_left),
    ] {
        if !value.is_finite() || !(0.0..1.0).contains(&value) {
            return Err(ProjectActionError::InvalidVisualClipCrop(format!(
                "{edge} must be finite and between 0.0 inclusive and 1.0 exclusive"
            )));
        }
    }
    if crop_left + crop_right >= 1.0 {
        return Err(ProjectActionError::InvalidVisualClipCrop(
            "cropLeft and cropRight must leave visible content".to_string(),
        ));
    }
    if crop_top + crop_bottom >= 1.0 {
        return Err(ProjectActionError::InvalidVisualClipCrop(
            "cropTop and cropBottom must leave visible content".to_string(),
        ));
    }
    Ok(())
}

fn update_visual_clip_fades(
    project: &mut VideoProject,
    item_id: &str,
    fade_in_seconds: f64,
    fade_out_seconds: f64,
) -> Result<(), ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }
    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if !is_visual_clip_kind(&item.kind) {
        return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
    }
    if !fade_in_seconds.is_finite()
        || !fade_out_seconds.is_finite()
        || fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > item.duration_seconds
    {
        return Err(ProjectActionError::InvalidVisualClipFades(
            item_id.to_string(),
        ));
    }
    for (key, value) in [
        ("fadeInSeconds", fade_in_seconds),
        ("fadeOutSeconds", fade_out_seconds),
    ] {
        if value > 0.0 {
            item.properties
                .insert(key.to_string(), serde_json::json!(value));
        } else {
            item.properties.remove(key);
        }
    }
    Ok(())
}

fn update_visual_clip_speed(
    project: &mut VideoProject,
    item_id: &str,
    speed: f64,
) -> Result<(), ProjectActionError> {
    if !speed.is_finite() || !(0.1..=8.0).contains(&speed) {
        return Err(ProjectActionError::InvalidVisualClipSpeed(
            item_id.to_string(),
        ));
    }
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }
    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if !is_visual_clip_kind(&item.kind) {
        return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
    }
    if (speed - 1.0).abs() < f64::EPSILON {
        item.properties.remove("speed");
    } else {
        item.properties
            .insert("speed".to_string(), serde_json::json!(speed));
    }
    Ok(())
}

fn set_item_keyframes(
    project: &mut VideoProject,
    item_id: &str,
    property: ProjectActionKeyframeProperty,
    keyframes: &[ProjectActionKeyframe],
) -> Result<(), ProjectActionError> {
    let mut keyframes = keyframes.to_vec();
    canonicalize_keyframes(&mut keyframes)?;
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &project.timeline.tracks[track_index].items[item_index];
    match property {
        ProjectActionKeyframeProperty::Opacity => {
            if !is_visual_clip_kind(&item.kind) {
                return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
            }
            validate_keyframes(&keyframes, item.duration_seconds, 0.0, 1.0)?;
        }
        ProjectActionKeyframeProperty::PositionX | ProjectActionKeyframeProperty::PositionY => {
            if !is_visual_clip_kind(&item.kind) {
                return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
            }
            validate_keyframes(&keyframes, item.duration_seconds, -10000.0, 10000.0)?;
        }
        ProjectActionKeyframeProperty::Scale
        | ProjectActionKeyframeProperty::ScaleX
        | ProjectActionKeyframeProperty::ScaleY => {
            if !is_visual_clip_kind(&item.kind) {
                return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
            }
            validate_keyframes(&keyframes, item.duration_seconds, 0.01, 100.0)?;
        }
        ProjectActionKeyframeProperty::CropTop
        | ProjectActionKeyframeProperty::CropRight
        | ProjectActionKeyframeProperty::CropBottom
        | ProjectActionKeyframeProperty::CropLeft => {
            if !is_visual_clip_kind(&item.kind) {
                return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
            }
            validate_keyframes(&keyframes, item.duration_seconds, 0.0, 1.0)?;
        }
        ProjectActionKeyframeProperty::RotationDegrees => {
            if !is_visual_clip_kind(&item.kind) {
                return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
            }
            validate_keyframes(&keyframes, item.duration_seconds, -360.0, 360.0)?;
        }
        ProjectActionKeyframeProperty::VolumeDb => {
            if item.kind != TimelineItemKind::AudioClip {
                return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
            }
            validate_keyframes(&keyframes, item.duration_seconds, -60.0, 24.0)?;
        }
    }

    let item = &mut project.timeline.tracks[track_index].items[item_index];
    if keyframes.is_empty() {
        if let Some(mut keyframes_object) = item
            .properties
            .get("keyframes")
            .and_then(serde_json::Value::as_object)
            .cloned()
        {
            keyframes_object.remove(keyframe_property_name(&property));
            if keyframes_object.is_empty() {
                item.properties.remove("keyframes");
            } else {
                item.properties.insert(
                    "keyframes".to_string(),
                    serde_json::Value::Object(keyframes_object),
                );
            }
        }
        return Ok(());
    }
    let mut keyframes_object = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    keyframes_object.insert(
        keyframe_property_name(&property).to_string(),
        serde_json::to_value(&keyframes).expect("keyframes serialize"),
    );
    item.properties.insert(
        "keyframes".to_string(),
        serde_json::Value::Object(keyframes_object),
    );

    Ok(())
}

fn upsert_item_keyframe(
    project: &mut VideoProject,
    item_id: &str,
    property: ProjectActionKeyframeProperty,
    keyframe: ProjectActionKeyframe,
) -> Result<(), ProjectActionError> {
    let mut keyframes = stored_item_keyframes(project, item_id, &property)?;
    keyframes.retain(|candidate| candidate.at_seconds != keyframe.at_seconds);
    keyframes.push(keyframe);
    keyframes.sort_by(|left, right| left.at_seconds.total_cmp(&right.at_seconds));
    set_item_keyframes(project, item_id, property, &keyframes)
}

fn move_item_keyframe(
    project: &mut VideoProject,
    item_id: &str,
    property: ProjectActionKeyframeProperty,
    from_seconds: f64,
    to_seconds: f64,
) -> Result<(), ProjectActionError> {
    if !from_seconds.is_finite() || !to_seconds.is_finite() {
        return Err(ProjectActionError::NegativeTime);
    }
    let mut keyframes = stored_item_keyframes(project, item_id, &property)?;
    let source_index = keyframes
        .iter()
        .position(|candidate| candidate.at_seconds == from_seconds)
        .ok_or_else(|| ProjectActionError::KeyframeNotFound {
            item_id: item_id.to_string(),
            property: keyframe_property_name(&property).to_string(),
            at_seconds: from_seconds,
        })?;
    let mut moved = keyframes.remove(source_index);
    moved.at_seconds = to_seconds;
    // Moving onto an occupied timestamp deterministically replaces that point.
    keyframes.retain(|candidate| candidate.at_seconds != to_seconds);
    keyframes.push(moved);
    keyframes.sort_by(|left, right| left.at_seconds.total_cmp(&right.at_seconds));
    set_item_keyframes(project, item_id, property, &keyframes)
}

fn delete_item_keyframe(
    project: &mut VideoProject,
    item_id: &str,
    property: ProjectActionKeyframeProperty,
    at_seconds: f64,
) -> Result<(), ProjectActionError> {
    if !at_seconds.is_finite() {
        return Err(ProjectActionError::NegativeTime);
    }
    let mut keyframes = stored_item_keyframes(project, item_id, &property)?;
    let original_len = keyframes.len();
    keyframes.retain(|candidate| candidate.at_seconds != at_seconds);
    if keyframes.len() == original_len {
        return Err(ProjectActionError::KeyframeNotFound {
            item_id: item_id.to_string(),
            property: keyframe_property_name(&property).to_string(),
            at_seconds,
        });
    }
    set_item_keyframes(project, item_id, property, &keyframes)
}

fn stored_item_keyframes(
    project: &VideoProject,
    item_id: &str,
    property: &ProjectActionKeyframeProperty,
) -> Result<Vec<ProjectActionKeyframe>, ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let item = &project.timeline.tracks[track_index].items[item_index];
    let Some(value) = item
        .properties
        .get("keyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|keyframes| keyframes.get(keyframe_property_name(property)))
    else {
        return Ok(Vec::new());
    };
    serde_json::from_value(value.clone()).map_err(|_| {
        ProjectActionError::InvalidEffectParam(format!(
            "stored keyframes for {} are invalid",
            keyframe_property_name(property)
        ))
    })
}

pub fn legacy_effect_instance_id(effect_type: &str, occurrence: usize) -> String {
    let slug = effect_type
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("legacy:{slug}:{}", occurrence + 1)
}

pub fn canonicalize_project_action_effects(
    effects: &mut [ProjectActionEffect],
) -> Result<(), ProjectActionError> {
    let mut occurrences = BTreeMap::<String, usize>::new();
    let mut instance_ids = BTreeSet::new();
    for effect in effects {
        let occurrence = occurrences.entry(effect.effect_type.clone()).or_default();
        let instance_id = if effect.effect_instance_id.trim().is_empty() {
            legacy_effect_instance_id(&effect.effect_type, *occurrence)
        } else {
            effect.effect_instance_id.trim().to_string()
        };
        *occurrence += 1;
        if instance_id.len() > 160
            || !instance_id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | ':')
            })
        {
            return Err(ProjectActionError::InvalidEffectStack(format!(
                "effect instance id `{instance_id}` is invalid"
            )));
        }
        if !instance_ids.insert(instance_id.clone()) {
            return Err(ProjectActionError::InvalidEffectStack(format!(
                "duplicate effect instance id `{instance_id}`"
            )));
        }
        effect.effect_instance_id = instance_id;
    }
    Ok(())
}

fn stored_item_effects(
    project: &VideoProject,
    item_id: &str,
) -> Result<Vec<ProjectActionEffect>, ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let item = &project.timeline.tracks[track_index].items[item_index];
    let mut effects = match item.properties.get("effects") {
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| {
            ProjectActionError::InvalidEffectStack(format!(
                "stored effects for timeline item {item_id} are invalid"
            ))
        })?,
        None => Vec::new(),
    };
    canonicalize_project_action_effects(&mut effects)?;
    Ok(effects)
}

fn effect_parameter_lane(
    project: &VideoProject,
    item_id: &str,
    effect_instance_id: &str,
    parameter_key: &str,
) -> Result<
    (
        Vec<ProjectActionEffect>,
        Vec<ProjectActionKeyframe>,
        f64,
        f64,
    ),
    ProjectActionError,
> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let item = &project.timeline.tracks[track_index].items[item_index];
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }
    if !is_visual_clip_kind(&item.kind) {
        return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
    }
    let effects = stored_item_effects(project, item_id)?;
    let effect = effects
        .iter()
        .find(|effect| effect.effect_instance_id == effect_instance_id)
        .ok_or_else(|| ProjectActionError::EffectInstanceNotFound {
            item_id: item_id.to_string(),
            effect_instance_id: effect_instance_id.to_string(),
        })?;
    let descriptor = effect_descriptor(&effect.effect_type).ok_or_else(|| {
        ProjectActionError::InvalidEffectParam(format!(
            "effect `{}` does not advertise numeric parameters",
            effect.effect_type
        ))
    })?;
    let parameter = descriptor
        .params
        .iter()
        .find(|parameter| parameter.key == parameter_key)
        .ok_or_else(|| {
            ProjectActionError::InvalidEffectParam(format!(
                "effect `{}` has no numeric parameter `{parameter_key}`",
                effect.effect_type
            ))
        })?;
    let lane = item
        .properties
        .get("effectParameterKeyframes")
        .and_then(serde_json::Value::as_object)
        .and_then(|lanes| lanes.get(effect_instance_id))
        .and_then(serde_json::Value::as_object)
        .and_then(|parameters| parameters.get(parameter_key))
        .map(|value| {
            serde_json::from_value(value.clone()).map_err(|_| {
                ProjectActionError::InvalidEffectParam(format!(
                    "stored keyframes for {effect_instance_id}.{parameter_key} are invalid"
                ))
            })
        })
        .transpose()?
        .unwrap_or_default();
    Ok((effects, lane, parameter.min, parameter.max))
}

fn set_effect_parameter_lane(
    project: &mut VideoProject,
    item_id: &str,
    effect_instance_id: &str,
    parameter_key: &str,
    mut keyframes: Vec<ProjectActionKeyframe>,
) -> Result<(), ProjectActionError> {
    let (effects, _, minimum, maximum) =
        effect_parameter_lane(project, item_id, effect_instance_id, parameter_key)?;
    canonicalize_keyframes(&mut keyframes)?;
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let duration_seconds = project.timeline.tracks[track_index].items[item_index].duration_seconds;
    validate_keyframes(&keyframes, duration_seconds, minimum, maximum)?;
    let item = &mut project.timeline.tracks[track_index].items[item_index];

    // Persist deterministic ids when a legacy stack is first keyframed.
    item.properties.insert(
        "effects".to_string(),
        serde_json::to_value(effects).expect("effects serialize"),
    );
    let mut all_lanes = item
        .properties
        .get("effectParameterKeyframes")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut instance_lanes = all_lanes
        .get(effect_instance_id)
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    if keyframes.is_empty() {
        instance_lanes.remove(parameter_key);
    } else {
        instance_lanes.insert(
            parameter_key.to_string(),
            serde_json::to_value(keyframes).expect("keyframes serialize"),
        );
    }
    if instance_lanes.is_empty() {
        all_lanes.remove(effect_instance_id);
    } else {
        all_lanes.insert(
            effect_instance_id.to_string(),
            serde_json::Value::Object(instance_lanes),
        );
    }
    if all_lanes.is_empty() {
        item.properties.remove("effectParameterKeyframes");
    } else {
        item.properties.insert(
            "effectParameterKeyframes".to_string(),
            serde_json::Value::Object(all_lanes),
        );
    }
    Ok(())
}

fn upsert_effect_parameter_keyframe(
    project: &mut VideoProject,
    item_id: &str,
    effect_instance_id: &str,
    parameter_key: &str,
    keyframe: ProjectActionKeyframe,
) -> Result<(), ProjectActionError> {
    let (_, mut keyframes, _, _) =
        effect_parameter_lane(project, item_id, effect_instance_id, parameter_key)?;
    keyframes.retain(|candidate| candidate.at_seconds != keyframe.at_seconds);
    keyframes.push(keyframe);
    set_effect_parameter_lane(
        project,
        item_id,
        effect_instance_id,
        parameter_key,
        keyframes,
    )
}

fn move_effect_parameter_keyframe(
    project: &mut VideoProject,
    item_id: &str,
    effect_instance_id: &str,
    parameter_key: &str,
    from_seconds: f64,
    to_seconds: f64,
) -> Result<(), ProjectActionError> {
    if !from_seconds.is_finite() || !to_seconds.is_finite() {
        return Err(ProjectActionError::NegativeTime);
    }
    let (_, mut keyframes, _, _) =
        effect_parameter_lane(project, item_id, effect_instance_id, parameter_key)?;
    let index = keyframes
        .iter()
        .position(|candidate| candidate.at_seconds == from_seconds)
        .ok_or_else(|| ProjectActionError::KeyframeNotFound {
            item_id: item_id.to_string(),
            property: format!("{effect_instance_id}.{parameter_key}"),
            at_seconds: from_seconds,
        })?;
    let mut moved = keyframes.remove(index);
    moved.at_seconds = to_seconds;
    keyframes.retain(|candidate| candidate.at_seconds != to_seconds);
    keyframes.push(moved);
    set_effect_parameter_lane(
        project,
        item_id,
        effect_instance_id,
        parameter_key,
        keyframes,
    )
}

fn delete_effect_parameter_keyframe(
    project: &mut VideoProject,
    item_id: &str,
    effect_instance_id: &str,
    parameter_key: &str,
    at_seconds: f64,
) -> Result<(), ProjectActionError> {
    if !at_seconds.is_finite() {
        return Err(ProjectActionError::NegativeTime);
    }
    let (_, mut keyframes, _, _) =
        effect_parameter_lane(project, item_id, effect_instance_id, parameter_key)?;
    let original_len = keyframes.len();
    keyframes.retain(|candidate| candidate.at_seconds != at_seconds);
    if keyframes.len() == original_len {
        return Err(ProjectActionError::KeyframeNotFound {
            item_id: item_id.to_string(),
            property: format!("{effect_instance_id}.{parameter_key}"),
            at_seconds,
        });
    }
    set_effect_parameter_lane(
        project,
        item_id,
        effect_instance_id,
        parameter_key,
        keyframes,
    )
}

fn canonicalize_keyframes(
    keyframes: &mut [ProjectActionKeyframe],
) -> Result<(), ProjectActionError> {
    for keyframe in keyframes.iter_mut() {
        keyframe.easing = canonical_keyframe_easing(keyframe.easing.as_deref())?;
    }
    keyframes.sort_by(|left, right| left.at_seconds.total_cmp(&right.at_seconds));
    Ok(())
}

fn canonical_keyframe_easing(easing: Option<&str>) -> Result<Option<String>, ProjectActionError> {
    let Some(easing) = easing else {
        return Ok(None);
    };
    let canonical = match easing.trim() {
        "linear" => "linear",
        "hold" => "hold",
        "easeIn" => "easeIn",
        "easeOut" => "easeOut",
        "easeInOut" | "smooth" => "easeInOut",
        "" => {
            return Err(ProjectActionError::InvalidEffectParam(
                "keyframe easing cannot be blank".to_string(),
            ))
        }
        other => {
            return Err(ProjectActionError::InvalidEffectParam(format!(
                "unsupported keyframe easing `{other}`"
            )))
        }
    };
    Ok(Some(canonical.to_string()))
}

fn validate_keyframes(
    keyframes: &[ProjectActionKeyframe],
    duration_seconds: f64,
    min_value: f64,
    max_value: f64,
) -> Result<(), ProjectActionError> {
    let mut previous_at_seconds = None;
    for keyframe in keyframes {
        if !keyframe.at_seconds.is_finite()
            || keyframe.at_seconds < 0.0
            || keyframe.at_seconds > duration_seconds
        {
            return Err(ProjectActionError::NegativeTime);
        }
        if previous_at_seconds.is_some_and(|previous| keyframe.at_seconds <= previous) {
            return Err(ProjectActionError::NonPositiveDuration);
        }
        if !keyframe.value.is_finite() || !(min_value..=max_value).contains(&keyframe.value) {
            return Err(ProjectActionError::InvalidEffectParam(format!(
                "keyframe value must be between {} and {}",
                format_number_for_message(min_value),
                format_number_for_message(max_value)
            )));
        }
        if keyframe
            .easing
            .as_deref()
            .is_some_and(|easing| easing.trim().is_empty())
        {
            return Err(ProjectActionError::InvalidEffectParam(
                "keyframe easing cannot be blank".to_string(),
            ));
        }
        previous_at_seconds = Some(keyframe.at_seconds);
    }

    Ok(())
}

fn keyframe_property_name(property: &ProjectActionKeyframeProperty) -> &'static str {
    match property {
        ProjectActionKeyframeProperty::Opacity => "opacity",
        ProjectActionKeyframeProperty::VolumeDb => "volumeDb",
        ProjectActionKeyframeProperty::PositionX => "positionX",
        ProjectActionKeyframeProperty::PositionY => "positionY",
        ProjectActionKeyframeProperty::Scale => "scale",
        ProjectActionKeyframeProperty::ScaleX => "scaleX",
        ProjectActionKeyframeProperty::ScaleY => "scaleY",
        ProjectActionKeyframeProperty::RotationDegrees => "rotationDegrees",
        ProjectActionKeyframeProperty::CropTop => "cropTop",
        ProjectActionKeyframeProperty::CropRight => "cropRight",
        ProjectActionKeyframeProperty::CropBottom => "cropBottom",
        ProjectActionKeyframeProperty::CropLeft => "cropLeft",
    }
}

fn update_item_properties(
    project: &mut VideoProject,
    updates: &[ProjectActionItemPropertiesUpdate],
) -> Result<(), ProjectActionError> {
    if updates.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for update in updates {
        let (track_index, _) = find_item(&project.timeline, &update.item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(update.item_id.clone()))?;
        if project.timeline.tracks[track_index].locked {
            return Err(ProjectActionError::TrackLocked(
                project.timeline.tracks[track_index].id.clone(),
            ));
        }
        if update.set.keys().any(|key| key.trim().is_empty())
            || update.remove.iter().any(|key| key.trim().is_empty())
        {
            return Err(ProjectActionError::InvalidEffectParam(
                "item property keys must not be blank".to_string(),
            ));
        }
    }

    for update in updates {
        let (track_index, item_index) = find_item(&project.timeline, &update.item_id)
            .expect("item properties update prevalidated item");
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        for key in &update.remove {
            item.properties.remove(key);
        }
        for (key, value) in &update.set {
            if value.is_null() {
                item.properties.remove(key);
            } else {
                item.properties.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(())
}

fn link_items(
    project: &mut VideoProject,
    item_ids: &[String],
    link_group_id: &str,
) -> Result<(), ProjectActionError> {
    if link_group_id.trim().is_empty() || link_group_id.chars().count() > 128 {
        return Err(ProjectActionError::InvalidEffectParam(
            "link group id must be non-empty and at most 128 characters".to_string(),
        ));
    }
    let positions = linkable_item_positions(project, item_ids, 2)?;
    for (track_index, item_index) in positions {
        project.timeline.tracks[track_index].items[item_index]
            .properties
            .insert("linkGroupId".to_string(), serde_json::json!(link_group_id));
    }
    Ok(())
}

fn unlink_items(project: &mut VideoProject, item_ids: &[String]) -> Result<(), ProjectActionError> {
    let positions = linkable_item_positions(project, item_ids, 1)?;
    for (track_index, item_index) in positions {
        project.timeline.tracks[track_index].items[item_index]
            .properties
            .remove("linkGroupId");
    }
    Ok(())
}

fn linkable_item_positions(
    project: &VideoProject,
    item_ids: &[String],
    minimum_items: usize,
) -> Result<Vec<(usize, usize)>, ProjectActionError> {
    if item_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }
    let mut seen = BTreeSet::new();
    let mut positions = Vec::new();
    for item_id in item_ids {
        let item_id = item_id.trim();
        if item_id.is_empty() {
            return Err(ProjectActionError::ItemNotFound(item_id.to_string()));
        }
        if !seen.insert(item_id.to_string()) {
            continue;
        }
        let (track_index, item_index) = find_item(&project.timeline, item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
        if project.timeline.tracks[track_index].locked {
            return Err(ProjectActionError::TrackLocked(
                project.timeline.tracks[track_index].id.clone(),
            ));
        }
        positions.push((track_index, item_index));
    }
    if positions.len() < minimum_items {
        return Err(ProjectActionError::InvalidEffectParam(format!(
            "linking requires at least {minimum_items} distinct timeline items"
        )));
    }
    Ok(positions)
}

fn update_item_effects(
    project: &mut VideoProject,
    item_ids: &[String],
    mut effects: Vec<ProjectActionEffect>,
) -> Result<(), ProjectActionError> {
    canonicalize_project_action_effects(&mut effects)?;
    validate_effect_stack(&effects)?;
    let positions = validate_effect_item_positions(project, item_ids, &effects)?;
    let effects_value = serde_json::to_value(&effects)
        .map_err(|error| ProjectActionError::InvalidEffectStack(error.to_string()))?;

    for (track_index, item_index) in positions {
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        let retained_instance_ids = effects
            .iter()
            .map(|effect| effect.effect_instance_id.as_str())
            .collect::<BTreeSet<_>>();
        if let Some(mut lanes) = item
            .properties
            .get("effectParameterKeyframes")
            .and_then(serde_json::Value::as_object)
            .cloned()
        {
            lanes.retain(|instance_id, _| retained_instance_ids.contains(instance_id.as_str()));
            if lanes.is_empty() {
                item.properties.remove("effectParameterKeyframes");
            } else {
                item.properties.insert(
                    "effectParameterKeyframes".to_string(),
                    serde_json::Value::Object(lanes),
                );
            }
        }
        if effects.is_empty() {
            item.properties.remove("effects");
        } else {
            item.properties
                .insert("effects".to_string(), effects_value.clone());
        }
    }

    Ok(())
}

fn validate_effect_item_positions(
    project: &VideoProject,
    item_ids: &[String],
    effects: &[ProjectActionEffect],
) -> Result<Vec<(usize, usize)>, ProjectActionError> {
    if item_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let contains_audio_denoise = effects
        .iter()
        .any(|effect| effect.effect_type == "audio.denoise");
    let audio_denoise_only = contains_audio_denoise
        && effects
            .iter()
            .all(|effect| effect.effect_type == "audio.denoise");
    if contains_audio_denoise && !audio_denoise_only {
        return Err(ProjectActionError::InvalidEffectStack(
            "audio.denoise cannot be combined with visual effects".to_string(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut positions = Vec::new();
    for item_id in item_ids {
        let item_id = item_id.trim();
        if item_id.is_empty() {
            return Err(ProjectActionError::ItemNotFound(item_id.to_string()));
        }
        if !seen.insert(item_id.to_string()) {
            continue;
        }

        let (track_index, item_index) = find_item(&project.timeline, item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
        let track = &project.timeline.tracks[track_index];
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
        let item = &track.items[item_index];
        if audio_denoise_only {
            if item.kind != TimelineItemKind::AudioClip {
                return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
            }
        } else if effects.is_empty() {
            if !is_visual_clip_kind(&item.kind) && item.kind != TimelineItemKind::AudioClip {
                return Err(ProjectActionError::InvalidEffectStack(format!(
                    "timeline item cannot carry effects: {item_id}"
                )));
            }
        } else if !is_visual_clip_kind(&item.kind) {
            return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
        }
        positions.push((track_index, item_index));
    }

    Ok(positions)
}

fn update_item_color_grade(
    project: &mut VideoProject,
    item_ids: &[String],
    reset: bool,
    grade: ProjectActionColorGrade,
) -> Result<(), ProjectActionError> {
    validate_color_grade(&grade)?;
    let positions = validate_visual_item_positions(project, item_ids)?;

    for (track_index, item_index) in positions {
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        let mut grade_object = if reset {
            serde_json::Map::new()
        } else {
            item.properties
                .get("colorGrade")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default()
        };

        upsert_optional_grade_value(&mut grade_object, "exposure", grade.exposure);
        upsert_optional_grade_value(&mut grade_object, "contrast", grade.contrast);
        upsert_optional_grade_value(&mut grade_object, "saturation", grade.saturation);
        upsert_optional_grade_value(&mut grade_object, "temperature", grade.temperature);
        upsert_optional_grade_value(&mut grade_object, "tint", grade.tint);
        for (key, value) in &grade.extra {
            if value.is_null() {
                grade_object.remove(key);
            } else {
                grade_object.insert(key.clone(), value.clone());
            }
        }

        if grade_object.is_empty() {
            item.properties.remove("colorGrade");
        } else {
            item.properties.insert(
                "colorGrade".to_string(),
                serde_json::Value::Object(grade_object),
            );
        }
    }

    Ok(())
}

fn validate_visual_item_positions(
    project: &VideoProject,
    item_ids: &[String],
) -> Result<Vec<(usize, usize)>, ProjectActionError> {
    if item_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let mut seen = BTreeSet::new();
    let mut positions = Vec::new();
    for item_id in item_ids {
        let item_id = item_id.trim();
        if item_id.is_empty() {
            return Err(ProjectActionError::ItemNotFound(item_id.to_string()));
        }
        if !seen.insert(item_id.to_string()) {
            continue;
        }

        let (track_index, item_index) = find_item(&project.timeline, item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
        let track = &project.timeline.tracks[track_index];
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
        let item = &track.items[item_index];
        if !is_visual_clip_kind(&item.kind) {
            return Err(ProjectActionError::NotVisualClip(item_id.to_string()));
        }
        positions.push((track_index, item_index));
    }

    Ok(positions)
}

fn validate_effect_stack(effects: &[ProjectActionEffect]) -> Result<(), ProjectActionError> {
    for effect in effects {
        let effect_type = effect.effect_type.trim();
        if effect_type.is_empty() {
            return Err(ProjectActionError::InvalidEffectStack(
                "effectType must not be blank".to_string(),
            ));
        }
        let descriptor = effect_descriptor(effect_type).ok_or_else(|| {
            ProjectActionError::InvalidEffectStack(format!("unknown effect: {effect_type}"))
        })?;
        if effect_type.starts_with("color.") {
            return Err(ProjectActionError::InvalidEffectStack(format!(
                "{effect_type} is a color effect; use updateItemColorGrade"
            )));
        }
        for (key, value) in &effect.params {
            if key.trim().is_empty() {
                return Err(ProjectActionError::InvalidEffectStack(
                    "effect parameter keys must not be blank".to_string(),
                ));
            }
            let spec = descriptor
                .params
                .iter()
                .find(|param| param.key == key)
                .ok_or_else(|| {
                    ProjectActionError::InvalidEffectStack(format!(
                        "{effect_type} unknown effect parameter: {key}"
                    ))
                })?;
            let number = value.as_f64().ok_or_else(|| {
                ProjectActionError::InvalidEffectStack(format!(
                    "{effect_type} {key} must be numeric"
                ))
            })?;
            if !number.is_finite() || !(spec.min..=spec.max).contains(&number) {
                return Err(ProjectActionError::InvalidEffectStack(format!(
                    "{effect_type} {key} must be between {} and {}",
                    format_number_for_message(spec.min),
                    format_number_for_message(spec.max)
                )));
            }
        }
    }

    Ok(())
}

fn validate_color_grade(grade: &ProjectActionColorGrade) -> Result<(), ProjectActionError> {
    validate_optional_grade_range(grade.exposure, "exposure", -3.0, 3.0)?;
    validate_optional_grade_range(grade.contrast, "contrast", 0.5, 1.5)?;
    validate_optional_grade_range(grade.saturation, "saturation", 0.0, 2.0)?;
    validate_optional_grade_range(grade.temperature, "temperature", 2000.0, 11000.0)?;
    validate_optional_grade_range(grade.tint, "tint", -100.0, 100.0)?;
    validate_extra_color_grade(&grade.extra)?;
    Ok(())
}

fn validate_extra_color_grade(
    extra: &BTreeMap<String, serde_json::Value>,
) -> Result<(), ProjectActionError> {
    for (key, value) in extra {
        match key.as_str() {
            "vibrance" => validate_grade_value(value, key, -1.0, 1.0)?,
            "highlights" => validate_grade_value(value, key, -1.0, 1.0)?,
            "shadows" => validate_grade_value(value, key, -1.0, 1.0)?,
            "blacks" => validate_grade_value(value, key, -1.0, 1.0)?,
            "whites" => validate_grade_value(value, key, -1.0, 1.0)?,
            "shadowsHue" | "midsHue" | "highsHue" => validate_grade_value(value, key, 0.0, 360.0)?,
            "shadowsAmount" | "midsAmount" | "highsAmount" => {
                validate_grade_value(value, key, 0.0, 1.0)?
            }
            "shadowsLum" => validate_grade_value(value, key, -0.5, 0.5)?,
            "midsGamma" => validate_grade_value(value, key, 0.5, 2.0)?,
            "highsGain" => validate_grade_value(value, key, 0.5, 1.5)?,
            "masterCurve" | "redCurve" | "greenCurve" | "blueCurve" => {
                validate_curve_points(value, key)?
            }
            "hueCurves" => validate_hue_curves(value)?,
            "lut" => validate_lut_grade(value)?,
            _ => {
                return Err(ProjectActionError::InvalidColorGrade(format!(
                    "unknown color grade control {key}"
                )));
            }
        }
    }

    Ok(())
}

fn validate_grade_value(
    value: &serde_json::Value,
    label: &str,
    min: f64,
    max: f64,
) -> Result<(), ProjectActionError> {
    match value.as_f64() {
        Some(value) if value.is_finite() && (min..=max).contains(&value) => Ok(()),
        _ => Err(ProjectActionError::InvalidColorGrade(format!(
            "{label} must be between {} and {}",
            format_number_for_message(min),
            format_number_for_message(max)
        ))),
    }
}

fn validate_curve_points(value: &serde_json::Value, label: &str) -> Result<(), ProjectActionError> {
    let points = value.as_array().ok_or_else(|| {
        ProjectActionError::InvalidColorGrade(format!("{label} must be an array of [x, y] points"))
    })?;
    if points.len() < 2 {
        return Err(ProjectActionError::InvalidColorGrade(format!(
            "{label} must include at least two [x, y] points"
        )));
    }
    let mut previous_x = None;
    for point in points {
        let pair = point.as_array().ok_or_else(|| {
            ProjectActionError::InvalidColorGrade(format!(
                "{label} must be an array of [x, y] points"
            ))
        })?;
        if pair.len() != 2 {
            return Err(ProjectActionError::InvalidColorGrade(format!(
                "{label} points must include x and y"
            )));
        }
        validate_grade_value(&pair[0], label, 0.0, 1.0)?;
        validate_grade_value(&pair[1], label, 0.0, 1.0)?;
        let x = pair[0].as_f64().expect("validated curve x");
        if previous_x.is_some_and(|previous_x| x <= previous_x) {
            return Err(ProjectActionError::InvalidColorGrade(format!(
                "{label} x values must be strictly increasing"
            )));
        }
        previous_x = Some(x);
    }

    Ok(())
}

fn validate_hue_curves(value: &serde_json::Value) -> Result<(), ProjectActionError> {
    let targets = value
        .get("targets")
        .and_then(|targets| targets.as_array())
        .ok_or_else(|| {
            ProjectActionError::InvalidColorGrade("hueCurves.targets must be an array".to_string())
        })?;
    if targets.is_empty() {
        return Err(ProjectActionError::InvalidColorGrade(
            "hueCurves.targets must include at least one target".to_string(),
        ));
    }
    if targets.len() > 64 {
        return Err(ProjectActionError::InvalidColorGrade(
            "hueCurves.targets must include at most 64 targets".to_string(),
        ));
    }
    for target in targets {
        let target = target.as_object().ok_or_else(|| {
            ProjectActionError::InvalidColorGrade("hueCurves targets must be objects".to_string())
        })?;
        if let Some(key) = target
            .keys()
            .find(|key| !["targetHue", "hueShift", "satScale", "lumShift"].contains(&key.as_str()))
        {
            return Err(ProjectActionError::InvalidColorGrade(format!(
                "hueCurves target has unknown control {key}"
            )));
        }
        validate_grade_value(
            target.get("targetHue").ok_or_else(|| {
                ProjectActionError::InvalidColorGrade("hueCurves targetHue is required".to_string())
            })?,
            "targetHue",
            0.0,
            360.0,
        )?;
        if let Some(value) = target.get("hueShift") {
            validate_grade_value(value, "hueShift", -30.0, 30.0)?;
        }
        if let Some(value) = target.get("satScale") {
            validate_grade_value(value, "satScale", 0.0, 2.0)?;
        }
        if let Some(value) = target.get("lumShift") {
            validate_grade_value(value, "lumShift", -0.5, 0.5)?;
        }
    }

    Ok(())
}

fn validate_lut_grade(value: &serde_json::Value) -> Result<(), ProjectActionError> {
    let lut = value.as_object().ok_or_else(|| {
        ProjectActionError::InvalidColorGrade("lut must be an object".to_string())
    })?;
    match lut.get("path").and_then(serde_json::Value::as_str) {
        Some(path) if !path.trim().is_empty() => {}
        _ => {
            return Err(ProjectActionError::InvalidColorGrade(
                "lut.path must be a non-empty string".to_string(),
            ));
        }
    }
    if let Some(strength) = lut.get("strength") {
        validate_grade_value(strength, "lut.strength", 0.0, 1.0)?;
    }

    Ok(())
}

fn validate_optional_grade_range(
    value: Option<f64>,
    label: &str,
    min: f64,
    max: f64,
) -> Result<(), ProjectActionError> {
    if let Some(value) = value {
        if !value.is_finite() || !(min..=max).contains(&value) {
            return Err(ProjectActionError::InvalidColorGrade(format!(
                "{label} must be between {} and {}",
                format_number_for_message(min),
                format_number_for_message(max)
            )));
        }
    }

    Ok(())
}

fn format_number_for_message(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn upsert_optional_grade_value(
    grade_object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<f64>,
) {
    if let Some(value) = value {
        grade_object.insert(key.to_string(), serde_json::json!(value));
    }
}

fn apply_caption_repair(
    project: &mut VideoProject,
    repair: &ProjectActionCaptionRepair,
) -> Result<(), ProjectActionError> {
    let text = repair.text.trim();
    if text.is_empty() {
        return Err(ProjectActionError::EmptyCaptionText);
    }
    if !repair.start_seconds.is_finite() || repair.start_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !repair.end_seconds.is_finite() || repair.end_seconds <= repair.start_seconds {
        return Err(ProjectActionError::NonPositiveDuration);
    }

    let (track_index, item_index) = find_item(&project.timeline, &repair.caption_item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(repair.caption_item_id.clone()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }
    let caption_item = &project.timeline.tracks[track_index].items[item_index];
    if caption_item.kind != TimelineItemKind::Caption {
        return Err(ProjectActionError::NotCaptionItem(
            repair.caption_item_id.clone(),
        ));
    }
    if !matches!(caption_item.source, TimelineSource::Text { .. }) {
        return Err(ProjectActionError::NotCaptionItem(
            repair.caption_item_id.clone(),
        ));
    }

    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media.duration_seconds))
        .collect::<BTreeMap<_, _>>();
    let transcript_index = project
        .transcripts
        .iter()
        .position(|transcript| transcript.id == repair.transcript_id)
        .ok_or_else(|| ProjectActionError::TranscriptNotFound(repair.transcript_id.clone()))?;
    let transcript = &project.transcripts[transcript_index];
    let media_duration = media_by_id
        .get(transcript.media_id.as_str())
        .copied()
        .ok_or_else(|| ProjectActionError::MissingMedia(transcript.media_id.clone()))?;
    if repair.word_index >= transcript.words.len() {
        return Err(ProjectActionError::TranscriptWordNotFound {
            transcript_id: repair.transcript_id.clone(),
            word_index: repair.word_index,
        });
    }

    let before = transcript.words[repair.word_index].clone();
    let after = TranscriptWord {
        text: text.to_string(),
        start_seconds: repair.start_seconds,
        end_seconds: repair.end_seconds,
        confidence: before.confidence,
        speaker: before.speaker.clone(),
    };
    validate_transcript_word_range(&after, &transcript.media_id, media_duration)?;

    let mut transcript_after = transcript.clone();
    transcript_after.words[repair.word_index] = after.clone();
    validate_transcript_timing(&transcript_after)?;

    let caption_item = &mut project.timeline.tracks[track_index].items[item_index];
    caption_item.start_seconds = repair.start_seconds;
    caption_item.duration_seconds = repair.end_seconds - repair.start_seconds;
    caption_item.source = TimelineSource::Text {
        text: text.to_string(),
    };
    caption_item.properties.insert(
        "captionRepairId".to_string(),
        serde_json::json!(repair.repair_id),
    );
    caption_item.properties.insert(
        "transcriptId".to_string(),
        serde_json::json!(repair.transcript_id),
    );
    caption_item.properties.insert(
        "wordIndex".to_string(),
        serde_json::json!(repair.word_index),
    );
    caption_item
        .properties
        .insert("textEdited".to_string(), serde_json::json!(true));
    sort_track_items(&mut project.timeline.tracks[track_index]);

    let transcript = &mut project.transcripts[transcript_index];
    transcript.words[repair.word_index] = after.clone();
    transcript.repairs.push(TranscriptRepair {
        id: repair.repair_id.clone(),
        kind: transcript_repair_kind(&before, &after),
        word_index: repair.word_index,
        before,
        after,
        created_at: repair.created_at.clone(),
    });

    Ok(())
}

fn edit_transcript_words(
    project: &mut VideoProject,
    edits: &[ProjectActionTranscriptWordEdit],
) -> Result<(), ProjectActionError> {
    if edits.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let mut transcripts = project.transcripts.clone();
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media.duration_seconds))
        .collect::<BTreeMap<_, _>>();

    for edit in edits {
        let transcript_index = transcripts
            .iter()
            .position(|transcript| transcript.id == edit.transcript_id)
            .ok_or_else(|| ProjectActionError::TranscriptNotFound(edit.transcript_id.clone()))?;
        let transcript = &mut transcripts[transcript_index];
        let media_duration = media_by_id
            .get(transcript.media_id.as_str())
            .copied()
            .ok_or_else(|| ProjectActionError::MissingMedia(transcript.media_id.clone()))?;
        if edit.word_index >= transcript.words.len() {
            return Err(ProjectActionError::TranscriptWordNotFound {
                transcript_id: edit.transcript_id.clone(),
                word_index: edit.word_index,
            });
        }

        let before = transcript.words[edit.word_index].clone();
        let mut after = before.clone();
        if let Some(text) = &edit.text {
            let text = text.trim();
            if text.is_empty() {
                return Err(ProjectActionError::EmptyTranscriptText);
            }
            after.text = text.to_string();
        }
        if let Some(start_seconds) = edit.start_seconds {
            after.start_seconds = start_seconds;
        }
        if let Some(end_seconds) = edit.end_seconds {
            after.end_seconds = end_seconds;
        }

        validate_transcript_word_range(&after, &transcript.media_id, media_duration)?;
        transcript.words[edit.word_index] = after.clone();
        validate_transcript_timing(transcript)?;

        transcript.repairs.push(TranscriptRepair {
            id: edit.repair_id.clone(),
            kind: transcript_repair_kind(&before, &after),
            word_index: edit.word_index,
            before,
            after,
            created_at: edit.created_at.clone(),
        });
    }

    project.transcripts = transcripts;
    Ok(())
}

fn record_generated_asset(
    project: &mut VideoProject,
    asset: ProjectActionGeneratedAsset,
) -> Result<(), ProjectActionError> {
    validate_generated_asset(project, &asset)?;
    let target_folder_id = asset
        .target_folder_id
        .as_deref()
        .map(|folder_id| validate_existing_media_folder_id(project, folder_id))
        .transpose()?;

    for output in &asset.outputs {
        project.media.push(MediaAsset {
            id: output.media_id.clone(),
            name: None,
            relative_path: output.relative_path.clone(),
            kind: asset.kind.clone(),
            duration_seconds: output.duration_seconds,
            width: Some(output.width),
            height: Some(output.height),
            fps: (output.fps > 0.0).then_some(output.fps),
            folder_id: target_folder_id.clone(),
        });
    }

    project.generated_assets.push(GeneratedAsset {
        schema_version: super::split::SPLIT_GENERATED_ASSET_SCHEMA_VERSION,
        id: asset.id,
        kind: asset.kind,
        status: asset.status,
        name: asset.name.and_then(|name| {
            let trimmed = name.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        }),
        target_folder_id,
        placement_intent: asset.placement_intent.and_then(|intent| {
            let trimmed = intent.trim().to_string();
            (!trimmed.is_empty()).then_some(trimmed)
        }),
        prompt: asset.prompt.trim().to_string(),
        model: GenerationModel {
            provider: asset.model.provider.trim().to_string(),
            id: asset.model.id.trim().to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: asset.references.media_ids,
            source_video_media_ref: normalized_optional_media_ref(
                asset.references.source_video_media_ref,
            ),
            first_frame_media_id: asset.references.first_frame_media_id,
            last_frame_media_id: asset.references.last_frame_media_id,
            reference_image_media_refs: normalized_media_refs(
                asset.references.reference_image_media_refs,
            ),
            reference_video_media_refs: normalized_media_refs(
                asset.references.reference_video_media_refs,
            ),
            reference_audio_media_refs: normalized_media_refs(
                asset.references.reference_audio_media_refs,
            ),
            provider_input_urls: asset
                .references
                .provider_input_urls
                .into_iter()
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
                .collect(),
        },
        settings: GeneratedAssetSettings {
            width: asset.settings.width,
            height: asset.settings.height,
            duration_seconds: asset.settings.duration_seconds,
            fps: asset.settings.fps,
            aspect_ratio: asset
                .settings
                .aspect_ratio
                .map(|value| value.trim().to_string()),
            resolution: asset
                .settings
                .resolution
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            num_images: asset.settings.num_images,
            quality: asset
                .settings
                .quality
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            generate_audio: asset.settings.generate_audio,
            category: asset
                .settings
                .category
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            voice: asset
                .settings
                .voice
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            lyrics: asset
                .settings
                .lyrics
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            style_instructions: asset
                .settings
                .style_instructions
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            instrumental: asset.settings.instrumental,
            video_source_start_frame: asset.settings.video_source_start_frame,
            video_source_end_frame: asset.settings.video_source_end_frame,
            video_source_start_seconds: asset.settings.video_source_start_seconds,
            video_source_end_seconds: asset.settings.video_source_end_seconds,
            timeline_start_seconds: asset.settings.timeline_start_seconds,
        },
        outputs: asset
            .outputs
            .into_iter()
            .map(|output| GeneratedAssetOutput {
                media_id: output.media_id,
                relative_path: output.relative_path,
                source_url: normalized_provider_source_url(output.source_url),
                width: output.width,
                height: output.height,
                duration_seconds: output.duration_seconds,
                fps: output.fps,
            })
            .collect(),
        created_at: asset.created_at,
        parent_asset_id: asset.parent_asset_id,
        retry_of_asset_id: asset.retry_of_asset_id,
    });

    Ok(())
}

fn update_generated_asset_status(
    project: &mut VideoProject,
    asset_id: &str,
    status: GeneratedAssetStatus,
) -> Result<(), ProjectActionError> {
    let generated_asset = project
        .generated_assets
        .iter_mut()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| ProjectActionError::MissingGeneratedAssetReference(asset_id.to_string()))?;

    if generated_asset.status == GeneratedAssetStatus::Cancelled
        && status != GeneratedAssetStatus::Cancelled
    {
        return Err(ProjectActionError::TerminalGeneratedAssetStatus(
            asset_id.to_string(),
        ));
    }
    generated_asset.status = status;
    Ok(())
}

fn update_generated_asset_references(
    project: &mut VideoProject,
    asset_id: &str,
    references: ProjectActionGeneratedAssetReferences,
) -> Result<(), ProjectActionError> {
    validate_generated_asset_references(project, &references)?;

    let generated_asset = project
        .generated_assets
        .iter_mut()
        .find(|asset| asset.id == asset_id)
        .ok_or_else(|| ProjectActionError::MissingGeneratedAssetReference(asset_id.to_string()))?;

    generated_asset.references = GeneratedAssetReferences {
        media_ids: references.media_ids,
        source_video_media_ref: normalized_optional_media_ref(references.source_video_media_ref),
        first_frame_media_id: references.first_frame_media_id,
        last_frame_media_id: references.last_frame_media_id,
        reference_image_media_refs: normalized_media_refs(references.reference_image_media_refs),
        reference_video_media_refs: normalized_media_refs(references.reference_video_media_refs),
        reference_audio_media_refs: normalized_media_refs(references.reference_audio_media_refs),
        provider_input_urls: references
            .provider_input_urls
            .into_iter()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
            .collect(),
    };
    Ok(())
}

fn normalized_optional_media_ref(media_id: Option<String>) -> Option<String> {
    media_id
        .map(|media_id| media_id.trim().to_string())
        .filter(|media_id| !media_id.is_empty())
}

fn normalized_media_refs(media_ids: Vec<String>) -> Vec<String> {
    media_ids
        .into_iter()
        .map(|media_id| media_id.trim().to_string())
        .filter(|media_id| !media_id.is_empty())
        .collect()
}

fn complete_generated_asset(
    project: &mut VideoProject,
    asset_id: &str,
    outputs: Vec<ProjectActionGeneratedAssetOutput>,
    replacement: Option<ProjectActionReplaceGeneratedOutput>,
) -> Result<(), ProjectActionError> {
    if outputs.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }
    if let Some(replacement) = &replacement {
        if !outputs
            .iter()
            .any(|output| output.media_id == replacement.media_id)
        {
            return Err(ProjectActionError::CompletionReplacementOutputMismatch(
                replacement.media_id.clone(),
            ));
        }
    }
    let generated_asset_index = project
        .generated_assets
        .iter()
        .position(|asset| asset.id == asset_id)
        .ok_or_else(|| ProjectActionError::MissingGeneratedAssetReference(asset_id.to_string()))?;
    if project.generated_assets[generated_asset_index].status == GeneratedAssetStatus::Cancelled {
        return Err(ProjectActionError::TerminalGeneratedAssetStatus(
            asset_id.to_string(),
        ));
    }
    if project.generated_assets[generated_asset_index].status == GeneratedAssetStatus::Completed
        || !project.generated_assets[generated_asset_index]
            .outputs
            .is_empty()
    {
        return Err(ProjectActionError::GeneratedAssetAlreadyCompleted(
            asset_id.to_string(),
        ));
    }
    validate_generated_asset_outputs(project, asset_id, &outputs)?;
    let target_folder_id = project.generated_assets[generated_asset_index]
        .target_folder_id
        .clone();

    for output in &outputs {
        project.media.push(MediaAsset {
            id: output.media_id.clone(),
            name: None,
            relative_path: output.relative_path.clone(),
            kind: MediaKind::Generated,
            duration_seconds: output.duration_seconds,
            width: Some(output.width),
            height: Some(output.height),
            fps: (output.fps > 0.0).then_some(output.fps),
            folder_id: target_folder_id.clone(),
        });
    }

    let generated_asset = &mut project.generated_assets[generated_asset_index];
    generated_asset.status = GeneratedAssetStatus::Completed;
    generated_asset.outputs = outputs
        .into_iter()
        .map(|output| GeneratedAssetOutput {
            media_id: output.media_id,
            relative_path: output.relative_path,
            source_url: normalized_provider_source_url(output.source_url),
            width: output.width,
            height: output.height,
            duration_seconds: output.duration_seconds,
            fps: output.fps,
        })
        .collect();

    if let Some(replacement) = replacement {
        replace_timeline_item_with_generated_output(project, &replacement)?;
    }

    Ok(())
}

fn normalized_provider_source_url(source_url: Option<String>) -> Option<String> {
    source_url
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn replace_timeline_item_with_generated_output(
    project: &mut VideoProject,
    replacement: &ProjectActionReplaceGeneratedOutput,
) -> Result<(), ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, &replacement.item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(replacement.item_id.clone()))?;
    let track = &project.timeline.tracks[track_index];
    if track.locked {
        return Err(ProjectActionError::TrackLocked(track.id.clone()));
    }
    let item = &track.items[item_index];
    if !is_video_track_clip_kind(&item.kind) {
        return Err(ProjectActionError::TrackTypeMismatch {
            item_kind: item.kind.clone(),
            track_kind: TrackKind::Video,
        });
    }

    let output_media = project
        .media
        .iter()
        .find(|media| media.id == replacement.media_id)
        .ok_or_else(|| ProjectActionError::MissingGeneratedOutput(replacement.media_id.clone()))?;
    if output_media.kind != MediaKind::Generated {
        return Err(ProjectActionError::NotGeneratedOutput(
            replacement.media_id.clone(),
        ));
    }
    if !item.duration_seconds.is_finite() || item.duration_seconds <= 0.0 {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    let replacement_targets = linked_visual_replacement_targets(project, track_index, item_index)?;

    let generated_asset_id = project
        .generated_assets
        .iter()
        .find(|asset| {
            asset.status == GeneratedAssetStatus::Completed
                && asset
                    .outputs
                    .iter()
                    .any(|output| output.media_id == replacement.media_id)
        })
        .map(|asset| asset.id.clone())
        .ok_or_else(|| ProjectActionError::NotGeneratedOutput(replacement.media_id.clone()))?;

    for (target_track_index, target_item_index) in replacement_targets {
        let item = &mut project.timeline.tracks[target_track_index].items[target_item_index];
        let preserved_duration_seconds = item.duration_seconds;
        item.source = TimelineSource::Media {
            media_id: replacement.media_id.clone(),
        };
        item.properties
            .insert("sourceIn".to_string(), serde_json::json!(0.0));
        item.properties.insert(
            "sourceOut".to_string(),
            serde_json::json!(preserved_duration_seconds),
        );
        item.properties.insert(
            "reason".to_string(),
            serde_json::json!("generated replacement"),
        );
        item.properties.insert(
            "generatedAssetId".to_string(),
            serde_json::json!(generated_asset_id),
        );
        item.properties.insert(
            "generatedOutputMediaId".to_string(),
            serde_json::json!(replacement.media_id),
        );
    }
    for track_index in 0..project.timeline.tracks.len() {
        sort_track_items(&mut project.timeline.tracks[track_index]);
    }

    Ok(())
}

fn linked_visual_replacement_targets(
    project: &VideoProject,
    track_index: usize,
    item_index: usize,
) -> Result<Vec<(usize, usize)>, ProjectActionError> {
    let item = &project.timeline.tracks[track_index].items[item_index];
    let Some(link_group_id) = item
        .properties
        .get("linkGroupId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(vec![(track_index, item_index)]);
    };
    let TimelineSource::Media {
        media_id: source_media_id,
    } = &item.source
    else {
        return Ok(vec![(track_index, item_index)]);
    };

    let mut targets = Vec::new();
    for (candidate_track_index, track) in project.timeline.tracks.iter().enumerate() {
        if track.locked
            && track.items.iter().any(|candidate| {
                is_linked_visual_replacement_target(candidate, link_group_id, source_media_id)
            })
        {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
        for (candidate_item_index, candidate) in track.items.iter().enumerate() {
            if is_linked_visual_replacement_target(candidate, link_group_id, source_media_id) {
                if !candidate.duration_seconds.is_finite() || candidate.duration_seconds <= 0.0 {
                    return Err(ProjectActionError::NonPositiveDuration);
                }
                targets.push((candidate_track_index, candidate_item_index));
            }
        }
    }
    if targets.is_empty() {
        targets.push((track_index, item_index));
    }
    Ok(targets)
}

fn is_linked_visual_replacement_target(
    item: &TimelineItem,
    link_group_id: &str,
    source_media_id: &str,
) -> bool {
    if !is_video_track_clip_kind(&item.kind) {
        return false;
    }
    let Some(candidate_link_group_id) = item
        .properties
        .get("linkGroupId")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    if candidate_link_group_id != link_group_id {
        return false;
    }
    matches!(
        &item.source,
        TimelineSource::Media { media_id } if media_id == source_media_id
    )
}

fn assign_media_folder(
    project: &mut VideoProject,
    media_id: &str,
    folder_id: Option<String>,
) -> Result<(), ProjectActionError> {
    let media_index = project
        .media
        .iter()
        .position(|media| media.id == media_id)
        .ok_or_else(|| ProjectActionError::MissingMedia(media_id.to_string()))?;

    let folder_id = match folder_id {
        Some(value) => Some(validate_media_folder_id(&value)?),
        None => None,
    };
    if let Some(folder_id) = &folder_id {
        if !project
            .media_folders
            .iter()
            .any(|folder| folder.id == *folder_id)
        {
            return Err(ProjectActionError::MissingMediaFolder(folder_id.clone()));
        }
    }

    project.media[media_index].folder_id = folder_id;
    Ok(())
}

fn create_media_folder(
    project: &mut VideoProject,
    mut folder: MediaFolder,
) -> Result<(), ProjectActionError> {
    folder.id = validate_media_folder_id(&folder.id)?;
    folder.name = validate_media_folder_name(&folder.name)?;
    folder.parent_id = match folder.parent_id {
        Some(parent_id) => {
            let parent_id = validate_media_folder_id(&parent_id)?;
            if parent_id == folder.id
                || !project
                    .media_folders
                    .iter()
                    .any(|candidate| candidate.id == parent_id)
            {
                return Err(ProjectActionError::MissingMediaFolderParent(parent_id));
            }
            Some(parent_id)
        }
        None => None,
    };

    if project
        .media_folders
        .iter()
        .any(|candidate| candidate.id == folder.id)
    {
        return Err(ProjectActionError::DuplicateMediaFolderId(folder.id));
    }

    project.media_folders.push(folder);
    Ok(())
}

fn rename_media_folder(
    project: &mut VideoProject,
    folder_id: &str,
    name: &str,
) -> Result<(), ProjectActionError> {
    let folder_id = validate_media_folder_id(folder_id)?;
    let name = validate_media_folder_name(name)?;
    let folder = project
        .media_folders
        .iter_mut()
        .find(|folder| folder.id == folder_id)
        .ok_or_else(|| ProjectActionError::MissingMediaFolder(folder_id.clone()))?;

    folder.name = name;
    Ok(())
}

fn delete_media_folder(
    project: &mut VideoProject,
    folder_id: &str,
) -> Result<(), ProjectActionError> {
    let folder_id = validate_media_folder_id(folder_id)?;
    let folder_index = project
        .media_folders
        .iter()
        .position(|folder| folder.id == folder_id)
        .ok_or_else(|| ProjectActionError::MissingMediaFolder(folder_id.clone()))?;

    project.media_folders.remove(folder_index);
    for media in &mut project.media {
        if media.folder_id.as_deref() == Some(folder_id.as_str()) {
            media.folder_id = None;
        }
    }
    for generated_asset in &mut project.generated_assets {
        if generated_asset.target_folder_id.as_deref() == Some(folder_id.as_str()) {
            generated_asset.target_folder_id = None;
        }
    }
    for folder in &mut project.media_folders {
        if folder.parent_id.as_deref() == Some(folder_id.as_str()) {
            folder.parent_id = None;
        }
    }

    Ok(())
}

fn rename_media(
    project: &mut VideoProject,
    media_id: &str,
    name: &str,
) -> Result<(), ProjectActionError> {
    let name = validate_media_name(name)?;
    let media = project
        .media
        .iter_mut()
        .find(|media| media.id == media_id)
        .ok_or_else(|| ProjectActionError::MissingMedia(media_id.to_string()))?;

    media.name = Some(name);
    Ok(())
}

fn delete_media(
    project: &mut VideoProject,
    media_ids: &[String],
) -> Result<(), ProjectActionError> {
    if media_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let mut unique_media_ids = BTreeSet::new();
    for media_id in media_ids {
        let media_id = media_id.trim();
        if media_id.is_empty() {
            return Err(ProjectActionError::MissingMedia(media_id.to_string()));
        }
        if !project.media.iter().any(|media| media.id == media_id) {
            return Err(ProjectActionError::MissingMedia(media_id.to_string()));
        }
        if generated_assets_reference_media(project, media_id) {
            return Err(ProjectActionError::MediaInUse(media_id.to_string()));
        }
        unique_media_ids.insert(media_id.to_string());
    }

    for track in &mut project.timeline.tracks {
        track.items.retain(|item| match &item.source {
            TimelineSource::Media { media_id } => !unique_media_ids.contains(media_id),
            TimelineSource::Generated { .. }
            | TimelineSource::Timeline { .. }
            | TimelineSource::Text { .. } => true,
        });
    }
    project
        .media
        .retain(|media| !unique_media_ids.contains(&media.id));
    project
        .transcripts
        .retain(|transcript| !unique_media_ids.contains(&transcript.media_id));

    Ok(())
}

fn remove_tracks(
    project: &mut VideoProject,
    track_ids: &[String],
) -> Result<(), ProjectActionError> {
    if track_ids.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    let mut unique_track_ids = BTreeSet::new();
    for track_id in track_ids {
        let track_id = track_id.trim();
        if track_id.is_empty() {
            return Err(ProjectActionError::EmptyTrackId);
        }
        let track = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?;
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
        unique_track_ids.insert(track_id.to_string());
    }

    project
        .timeline
        .tracks
        .retain(|track| !unique_track_ids.contains(&track.id));

    Ok(())
}

fn update_render_settings(
    project: &mut VideoProject,
    settings: RenderSettings,
) -> Result<(), ProjectActionError> {
    validate_render_settings(&settings)?;
    project.render_settings = settings;
    Ok(())
}

fn update_project_settings(
    project: &mut VideoProject,
    name: String,
    render_settings: RenderSettings,
) -> Result<(), ProjectActionError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProjectActionError::InvalidRenderSettings(
            "project name cannot be empty".to_string(),
        ));
    }
    validate_project_settings_render_settings(&render_settings)?;

    project.name = name.to_string();
    project.render_settings = render_settings;
    Ok(())
}

fn generated_assets_reference_media(project: &VideoProject, target_media_id: &str) -> bool {
    project.generated_assets.iter().any(|asset| {
        asset
            .references
            .media_ids
            .iter()
            .any(|id| id == target_media_id)
            || asset.references.first_frame_media_id.as_deref() == Some(target_media_id)
            || asset.references.last_frame_media_id.as_deref() == Some(target_media_id)
            || asset
                .outputs
                .iter()
                .any(|output| output.media_id == target_media_id)
    })
}

fn validate_media_folder_id(folder_id: &str) -> Result<String, ProjectActionError> {
    let folder_id = folder_id.trim().to_string();
    if folder_id.is_empty() {
        return Err(ProjectActionError::EmptyMediaFolderId);
    }
    if !is_safe_path_segment(&folder_id) {
        return Err(ProjectActionError::InvalidMediaFolderId(folder_id));
    }

    Ok(folder_id)
}

fn validate_media_folder_name(name: &str) -> Result<String, ProjectActionError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ProjectActionError::EmptyMediaFolderName);
    }

    Ok(name)
}

fn validate_media_name(name: &str) -> Result<String, ProjectActionError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ProjectActionError::EmptyMediaName);
    }

    Ok(name)
}

fn validate_render_settings(settings: &RenderSettings) -> Result<(), ProjectActionError> {
    if settings.width == 0 {
        return Err(ProjectActionError::InvalidRenderSettings(
            "width must be greater than zero".to_string(),
        ));
    }
    if settings.height == 0 {
        return Err(ProjectActionError::InvalidRenderSettings(
            "height must be greater than zero".to_string(),
        ));
    }
    if !settings.fps.is_finite() || settings.fps <= 0.0 {
        return Err(ProjectActionError::InvalidRenderSettings(
            "fps must be finite and greater than zero".to_string(),
        ));
    }
    if !settings.loudness_lufs.is_finite() {
        return Err(ProjectActionError::InvalidRenderSettings(
            "loudnessLufs must be finite".to_string(),
        ));
    }

    Ok(())
}

fn validate_project_settings_render_settings(
    settings: &RenderSettings,
) -> Result<(), ProjectActionError> {
    const MIN_PROJECT_DIMENSION: u32 = 2;
    const MAX_PROJECT_DIMENSION: u32 = 16_384;
    const SUPPORTED_PROJECT_FPS: [f64; 8] = [23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0];

    if !(MIN_PROJECT_DIMENSION..=MAX_PROJECT_DIMENSION).contains(&settings.width)
        || !settings.width.is_multiple_of(2)
    {
        return Err(ProjectActionError::InvalidRenderSettings(
            "width must be an even value between 2 and 16384".to_string(),
        ));
    }
    if !(MIN_PROJECT_DIMENSION..=MAX_PROJECT_DIMENSION).contains(&settings.height)
        || !settings.height.is_multiple_of(2)
    {
        return Err(ProjectActionError::InvalidRenderSettings(
            "height must be an even value between 2 and 16384".to_string(),
        ));
    }
    if !settings.fps.is_finite()
        || !SUPPORTED_PROJECT_FPS
            .iter()
            .any(|fps| nearly_equal(settings.fps, *fps))
    {
        return Err(ProjectActionError::InvalidRenderSettings(
            "fps must be a supported canonical frame rate".to_string(),
        ));
    }
    validate_render_settings(settings)
}

fn validate_existing_media_folder_id(
    project: &VideoProject,
    folder_id: &str,
) -> Result<String, ProjectActionError> {
    let folder_id = validate_media_folder_id(folder_id)?;
    if !project
        .media_folders
        .iter()
        .any(|folder| folder.id == folder_id)
    {
        return Err(ProjectActionError::MissingMediaFolder(folder_id));
    }

    Ok(folder_id)
}

fn validate_generated_asset(
    project: &VideoProject,
    asset: &ProjectActionGeneratedAsset,
) -> Result<(), ProjectActionError> {
    if asset.id.trim().is_empty() {
        return Err(ProjectActionError::DuplicateGeneratedAssetId(
            asset.id.clone(),
        ));
    }
    if !is_safe_path_segment(&asset.id) {
        return Err(ProjectActionError::InvalidGeneratedAssetId(
            asset.id.clone(),
        ));
    }
    if project
        .generated_assets
        .iter()
        .any(|generated| generated.id == asset.id)
    {
        return Err(ProjectActionError::DuplicateGeneratedAssetId(
            asset.id.clone(),
        ));
    }
    if asset.kind != MediaKind::Generated {
        return Err(ProjectActionError::InvalidGeneratedAssetKind(
            asset.kind.clone(),
        ));
    }
    if asset.prompt.trim().is_empty()
        && !allows_promptless_source_video_audio_asset(&asset.references, &asset.settings)
    {
        return Err(ProjectActionError::EmptyGeneratedPrompt);
    }
    if asset.model.provider.trim().is_empty() || asset.model.id.trim().is_empty() {
        return Err(ProjectActionError::EmptyGeneratedModel);
    }
    if let Some(target_folder_id) = &asset.target_folder_id {
        validate_existing_media_folder_id(project, target_folder_id)?;
    }
    validate_generated_asset_settings(&asset.settings)?;
    if asset.outputs.is_empty() && asset.status == GeneratedAssetStatus::Completed {
        return Err(ProjectActionError::EmptyItems);
    }
    let generated_asset_ids = project
        .generated_assets
        .iter()
        .map(|generated| generated.id.as_str())
        .collect::<BTreeSet<_>>();
    for generated_asset_id in asset
        .parent_asset_id
        .iter()
        .chain(asset.retry_of_asset_id.iter())
    {
        if !generated_asset_ids.contains(generated_asset_id.as_str()) {
            return Err(ProjectActionError::MissingGeneratedAssetReference(
                generated_asset_id.clone(),
            ));
        }
    }

    validate_generated_asset_references(project, &asset.references)?;

    validate_generated_asset_outputs(project, &asset.id, &asset.outputs)?;

    Ok(())
}

fn allows_promptless_source_video_audio_asset(
    references: &ProjectActionGeneratedAssetReferences,
    settings: &ProjectActionGeneratedAssetSettings,
) -> bool {
    references
        .source_video_media_ref
        .as_deref()
        .is_some_and(|media_ref| !media_ref.trim().is_empty())
        && settings.width.is_none()
        && settings.height.is_none()
        && settings.fps.is_none()
        && settings.aspect_ratio.is_none()
}

fn validate_generated_asset_references(
    project: &VideoProject,
    references: &ProjectActionGeneratedAssetReferences,
) -> Result<(), ProjectActionError> {
    let media_ids = project
        .media
        .iter()
        .map(|media| media.id.as_str())
        .collect::<BTreeSet<_>>();
    let media_by_id = project
        .media
        .iter()
        .map(|media| (media.id.as_str(), media))
        .collect::<BTreeMap<_, _>>();
    for media_id in references
        .media_ids
        .iter()
        .chain(references.source_video_media_ref.iter())
        .chain(references.first_frame_media_id.iter())
        .chain(references.last_frame_media_id.iter())
        .chain(references.reference_image_media_refs.iter())
        .chain(references.reference_video_media_refs.iter())
        .chain(references.reference_audio_media_refs.iter())
    {
        if media_id.trim().is_empty() {
            return Err(ProjectActionError::EmptyGeneratedMediaReference);
        }
        if !media_ids.contains(media_id.as_str()) {
            return Err(ProjectActionError::MissingMedia(media_id.clone()));
        }
    }
    for media_id in references
        .first_frame_media_id
        .iter()
        .chain(references.last_frame_media_id.iter())
        .chain(references.reference_image_media_refs.iter())
    {
        if media_by_id
            .get(media_id.as_str())
            .is_some_and(|media| media.kind == MediaKind::Audio)
        {
            return Err(ProjectActionError::InvalidGeneratedFrameReference(
                media_id.clone(),
            ));
        }
    }
    for media_id in references.source_video_media_ref.iter() {
        if media_by_id
            .get(media_id.as_str())
            .is_some_and(|media| !media_is_video_reference(media))
        {
            return Err(ProjectActionError::InvalidGeneratedFrameReference(
                media_id.clone(),
            ));
        }
    }
    for media_id in &references.reference_video_media_refs {
        if media_by_id
            .get(media_id.as_str())
            .is_some_and(|media| !media_is_video_reference(media))
        {
            return Err(ProjectActionError::InvalidGeneratedFrameReference(
                media_id.clone(),
            ));
        }
    }
    for media_id in &references.reference_audio_media_refs {
        if media_by_id
            .get(media_id.as_str())
            .is_some_and(|media| media.kind != MediaKind::Audio)
        {
            return Err(ProjectActionError::InvalidGeneratedFrameReference(
                media_id.clone(),
            ));
        }
    }

    Ok(())
}

fn media_is_video_reference(media: &MediaAsset) -> bool {
    match media.kind {
        MediaKind::Video => true,
        MediaKind::Generated => {
            media.width.unwrap_or(0) > 0
                && media.height.unwrap_or(0) > 0
                && (media.duration_seconds > 0.0 || media.fps.unwrap_or(0.0) > 0.0)
        }
        _ => false,
    }
}

fn validate_generated_asset_settings(
    settings: &ProjectActionGeneratedAssetSettings,
) -> Result<(), ProjectActionError> {
    if settings.width == Some(0) {
        return Err(ProjectActionError::InvalidGeneratedAssetSettings(
            "width must be greater than zero".to_string(),
        ));
    }
    if settings.height == Some(0) {
        return Err(ProjectActionError::InvalidGeneratedAssetSettings(
            "height must be greater than zero".to_string(),
        ));
    }
    if settings.width.is_some() != settings.height.is_some() {
        return Err(ProjectActionError::InvalidGeneratedAssetSettings(
            "width and height must be set together".to_string(),
        ));
    }
    if settings
        .duration_seconds
        .is_some_and(|duration| !duration.is_finite() || duration <= 0.0)
    {
        return Err(ProjectActionError::InvalidGeneratedAssetSettings(
            "durationSeconds must be finite and greater than zero".to_string(),
        ));
    }
    if settings
        .fps
        .is_some_and(|fps| !fps.is_finite() || fps <= 0.0)
    {
        return Err(ProjectActionError::InvalidGeneratedAssetSettings(
            "fps must be finite and greater than zero".to_string(),
        ));
    }
    if settings
        .aspect_ratio
        .as_ref()
        .is_some_and(|aspect_ratio| !is_valid_generated_asset_aspect_ratio(aspect_ratio))
    {
        return Err(ProjectActionError::InvalidGeneratedAssetSettings(
            "aspectRatio must be a ratio such as 16:9 or auto".to_string(),
        ));
    }

    Ok(())
}

fn is_valid_generated_asset_aspect_ratio(aspect_ratio: &str) -> bool {
    let aspect_ratio = aspect_ratio.trim();
    !aspect_ratio.is_empty() && (aspect_ratio == "auto" || aspect_ratio.contains(':'))
}

fn validate_generated_asset_outputs(
    project: &VideoProject,
    asset_id: &str,
    outputs: &[ProjectActionGeneratedAssetOutput],
) -> Result<(), ProjectActionError> {
    let media_ids = project
        .media
        .iter()
        .map(|media| media.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut output_media_ids = BTreeSet::new();
    for output in outputs {
        if output.media_id.trim().is_empty() {
            return Err(ProjectActionError::EmptyGeneratedOutputMediaId);
        }
        if media_ids.contains(output.media_id.as_str())
            || !output_media_ids.insert(output.media_id.clone())
        {
            return Err(ProjectActionError::DuplicateMediaId(
                output.media_id.clone(),
            ));
        }
        if output.relative_path.trim().is_empty() {
            return Err(ProjectActionError::EmptyGeneratedOutputPath);
        }
        let relative_path = Path::new(&output.relative_path);
        let asset_directory = Path::new("generated").join(asset_id);
        let asset_sidecar = asset_directory.join("asset.json");
        let has_unsafe_component = relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
        if relative_path == asset_sidecar {
            return Err(ProjectActionError::GeneratedOutputReservedPath(
                output.relative_path.clone(),
            ));
        }
        if has_unsafe_component
            || relative_path == asset_directory
            || !relative_path.starts_with(&asset_directory)
        {
            return Err(ProjectActionError::GeneratedOutputOutsideAssetDirectory(
                output.relative_path.clone(),
            ));
        }
        if output.width == 0 || output.height == 0 {
            return Err(ProjectActionError::NonPositiveDuration);
        }
        if generated_output_is_still_image(&output.relative_path) {
            if !output.duration_seconds.is_finite() || output.duration_seconds < 0.0 {
                return Err(ProjectActionError::NonPositiveDuration);
            }
            if !output.fps.is_finite() || output.fps < 0.0 {
                return Err(ProjectActionError::NonPositiveDuration);
            }
        } else if !output.duration_seconds.is_finite()
            || output.duration_seconds <= 0.0
            || !output.fps.is_finite()
            || output.fps <= 0.0
        {
            return Err(ProjectActionError::NonPositiveDuration);
        }
    }
    Ok(())
}

fn generated_output_is_still_image(relative_path: &str) -> bool {
    Path::new(relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
        .unwrap_or(false)
}

fn is_safe_path_segment(id: &str) -> bool {
    let mut components = Path::new(id).components();
    let Some(component) = components.next() else {
        return false;
    };
    matches!(component, Component::Normal(_))
        && components.next().is_none()
        && !id.contains('/')
        && !id.contains('\\')
}

fn attach_render_report(
    project: &mut VideoProject,
    report: ProjectRenderReport,
) -> Result<(), ProjectActionError> {
    validate_render_report(project, &report)?;
    project.render_reports.push(report);

    Ok(())
}

fn validate_render_report(
    project: &VideoProject,
    report: &ProjectRenderReport,
) -> Result<(), ProjectActionError> {
    if project
        .render_reports
        .iter()
        .any(|existing| existing.id == report.id)
    {
        return Err(ProjectActionError::DuplicateRenderReportId(
            report.id.clone(),
        ));
    }
    if report.output_path.trim().is_empty() {
        return Err(ProjectActionError::EmptyRenderOutputPath);
    }
    if !report.duration_seconds.is_finite() || report.duration_seconds <= 0.0 {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    if !report.streams.video && !report.streams.audio {
        return Err(ProjectActionError::EmptyRenderStreams);
    }
    if report.artifacts.is_empty()
        || report
            .artifacts
            .iter()
            .any(|artifact| artifact.trim().is_empty())
        || !report
            .artifacts
            .iter()
            .any(|artifact| artifact == &report.output_path)
    {
        return Err(ProjectActionError::EmptyRenderArtifacts);
    }
    if let Some(comparison) = &report.preview_comparison {
        if comparison.status.trim().is_empty()
            || comparison.compared_frames.iter().any(|frame| {
                !frame.timeline_seconds.is_finite()
                    || frame.timeline_seconds < 0.0
                    || !frame.mismatch_ratio.is_finite()
                    || frame.mismatch_ratio < 0.0
                    || frame.mismatch_ratio > 1.0
                    || frame.preview_frame.trim().is_empty()
                    || frame.rendered_frame.trim().is_empty()
                    || !report
                        .artifacts
                        .iter()
                        .any(|artifact| artifact == &frame.preview_frame)
                    || !report
                        .artifacts
                        .iter()
                        .any(|artifact| artifact == &frame.rendered_frame)
                    || frame.diff_frame.as_ref().is_some_and(|diff_frame| {
                        diff_frame.trim().is_empty()
                            || !report
                                .artifacts
                                .iter()
                                .any(|artifact| artifact == diff_frame)
                    })
            })
        {
            return Err(ProjectActionError::EmptyRenderArtifacts);
        }
    }
    if report.log_path.trim().is_empty() {
        return Err(ProjectActionError::EmptyRenderLogPath);
    }
    for check in [
        "duration",
        "streams",
        "captionAlignment",
        "overlayTiming",
        "visualFrameEvidence",
        "artifactPaths",
        "logPath",
    ] {
        if !report.checks.contains_key(check) {
            return Err(ProjectActionError::MissingRenderCheck(check.to_string()));
        }
    }

    Ok(())
}

fn record_export_artifact(
    project: &mut VideoProject,
    artifact: ProjectExportArtifact,
) -> Result<(), ProjectActionError> {
    validate_export_artifact(project, &artifact)?;
    project.export_artifacts.push(artifact);

    Ok(())
}

fn validate_export_artifact(
    project: &VideoProject,
    artifact: &ProjectExportArtifact,
) -> Result<(), ProjectActionError> {
    if artifact.id.trim().is_empty() {
        return Err(ProjectActionError::EmptyExportArtifactId);
    }
    if !is_safe_path_segment(&artifact.id) {
        return Err(ProjectActionError::InvalidExportArtifactId(
            artifact.id.clone(),
        ));
    }
    if project
        .export_artifacts
        .iter()
        .any(|existing| existing.id == artifact.id)
    {
        return Err(ProjectActionError::DuplicateExportArtifactId(
            artifact.id.clone(),
        ));
    }
    if artifact.format.trim().is_empty() {
        return Err(ProjectActionError::EmptyExportArtifactFormat);
    }
    if artifact.path.trim().is_empty() {
        return Err(ProjectActionError::EmptyExportArtifactPath);
    }
    if Path::new(&artifact.path).is_absolute() {
        if !is_lexically_clean_absolute_path(&artifact.path) {
            return Err(ProjectActionError::UnsafeExportArtifactPath(
                artifact.path.clone(),
            ));
        }
        if !absolute_export_artifact_is_in_chosen_folder(project, artifact) {
            return Err(export_artifact_location_error(&artifact.path));
        }
    } else {
        if !is_safe_project_relative_path(&artifact.path) {
            return Err(ProjectActionError::UnsafeExportArtifactPath(
                artifact.path.clone(),
            ));
        }
        if !export_artifact_path_stays_under_exports(&artifact.path) {
            return Err(export_artifact_location_error(&artifact.path));
        }
    }
    if artifact.mime_type.trim().is_empty() {
        return Err(ProjectActionError::EmptyExportArtifactMimeType);
    }
    validate_export_artifact_profile_contract(artifact)?;
    if artifact.created_at.trim().is_empty() {
        return Err(ProjectActionError::EmptyExportArtifactCreatedAt);
    }

    Ok(())
}

fn export_artifact_location_error(path: &str) -> ProjectActionError {
    ProjectActionError::InvalidExportArtifactContract(format!(
        "export artifact path must stay under exports/ or in the folder its export chose: {path}"
    ))
}

fn is_lexically_clean_absolute_path(value: &str) -> bool {
    Path::new(value).components().all(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::Normal(_)
        )
    })
}

/// An absolute artifact must sit directly in the folder its export job recorded
/// in `export_settings`, so an agent-recorded artifact can't point anywhere.
fn absolute_export_artifact_is_in_chosen_folder(
    project: &VideoProject,
    artifact: &ProjectExportArtifact,
) -> bool {
    let Some(job_id) = artifact.job_id.as_deref() else {
        return false;
    };
    let Some(directory) = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .and_then(|job| job.export_settings.as_ref())
        .and_then(|settings| settings.output.as_ref())
        .and_then(|output| output.directory.as_deref())
    else {
        return false;
    };
    Path::new(&artifact.path).parent() == Some(Path::new(directory))
}

fn export_artifact_path_stays_under_exports(value: &str) -> bool {
    let path = Path::new(value);
    let mut components = path.components();
    let starts_with_exports = matches!(
        components.next(),
        Some(Component::Normal(component)) if component == "exports"
    );

    starts_with_exports && components.next().is_some()
}

fn validate_export_artifact_profile_contract(
    artifact: &ProjectExportArtifact,
) -> Result<(), ProjectActionError> {
    match artifact.format.as_str() {
        "mp4H264" | "mp4H265" => validate_export_artifact_exact_contract(
            artifact,
            ProjectExportArtifactKind::Mp4,
            "mp4",
            "video/mp4",
        ),
        "proResMov" => validate_export_artifact_exact_contract(
            artifact,
            ProjectExportArtifactKind::Mov,
            "mov",
            "video/quicktime",
        ),
        "palmierProject" => validate_export_artifact_exact_contract(
            artifact,
            ProjectExportArtifactKind::ProjectBundle,
            "palmier",
            "application/vnd.video-creater.project",
        ),
        _ => Ok(()),
    }
}

fn validate_export_artifact_exact_contract(
    artifact: &ProjectExportArtifact,
    expected_kind: ProjectExportArtifactKind,
    expected_extension: &str,
    expected_mime_type: &str,
) -> Result<(), ProjectActionError> {
    let actual_extension = Path::new(&artifact.path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();

    if artifact.kind != expected_kind
        || !actual_extension.eq_ignore_ascii_case(expected_extension)
        || artifact.mime_type != expected_mime_type
    {
        return Err(ProjectActionError::InvalidExportArtifactContract(format!(
            "{} export artifacts must use kind {:?}, .{} paths, and mimeType {}",
            artifact.format, expected_kind, expected_extension, expected_mime_type
        )));
    }

    Ok(())
}

fn is_safe_project_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn record_job(project: &mut VideoProject, job: JobSummary) -> Result<(), ProjectActionError> {
    validate_job_summary(project, &job)?;
    project.jobs.push(job);

    Ok(())
}

fn is_terminal_job_status(status: &JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
    )
}

fn update_job_status(
    project: &mut VideoProject,
    job_id: &str,
    status: JobStatus,
    updated_at: &str,
    run_id: Option<String>,
) -> Result<(), ProjectActionError> {
    validate_job_id(job_id)?;
    if updated_at.trim().is_empty() {
        return Err(ProjectActionError::EmptyJobUpdatedAt);
    }
    if run_id
        .as_ref()
        .is_some_and(|run_id| run_id.trim().is_empty())
    {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "runId".to_string(),
        ));
    }

    let job = project
        .jobs
        .iter_mut()
        .find(|job| job.id == job_id)
        .ok_or_else(|| ProjectActionError::JobNotFound(job_id.to_string()))?;

    if run_id.is_some() && job.workflow.is_none() {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "workflow".to_string(),
        ));
    }
    let mut explicit_local_render_retry = false;
    if let Some(attempt_id) = run_id
        .as_deref()
        .filter(|run_id| run_id.starts_with("render-attempt/"))
    {
        if !matches!(
            job.kind.as_str(),
            "render_draft" | "export_media" | "exportMedia" | "captureCanonicalPreviewFrame"
        ) {
            return Err(ProjectActionError::StaleRenderAttempt(job_id.to_string()));
        }
        let current_attempt = job
            .workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref());
        let incoming_terminal = matches!(
            status,
            JobStatus::Failed | JobStatus::Cancelled | JobStatus::Completed
        );
        let current_terminal = matches!(
            job.status,
            JobStatus::Failed | JobStatus::Cancelled | JobStatus::Completed
        );
        if incoming_terminal && current_attempt != Some(attempt_id) {
            return Err(ProjectActionError::StaleRenderAttempt(job_id.to_string()));
        }
        if incoming_terminal && current_terminal {
            return Err(ProjectActionError::TerminalJobStatus(job_id.to_string()));
        }
        if !incoming_terminal && current_terminal {
            if current_attempt == Some(attempt_id) {
                return Err(ProjectActionError::TerminalJobStatus(job_id.to_string()));
            }
            explicit_local_render_retry = true;
        }
    }
    // The editor records a Temporal run's "started" status after the start call returns, so it can
    // land after that run already finished. The run's result stands.
    let current_run_id = job
        .workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.as_deref());
    if run_id.is_some()
        && run_id.as_deref() == current_run_id
        && is_terminal_job_status(&job.status)
        && !is_terminal_job_status(&status)
    {
        return Ok(());
    }
    if job.status == JobStatus::Cancelled
        && status != JobStatus::Cancelled
        && !explicit_local_render_retry
    {
        return Err(ProjectActionError::TerminalJobStatus(job_id.to_string()));
    }

    if status != JobStatus::Failed {
        job.failure_reason = None;
    }
    job.status = status;
    job.updated_at = updated_at.to_string();
    if let Some(workflow_run_id) = run_id {
        if let Some(workflow) = &mut job.workflow {
            workflow.run_id = Some(workflow_run_id);
        }
    }

    Ok(())
}

fn record_job_failure(
    project: &mut VideoProject,
    job_id: &str,
    reason: &str,
    updated_at: &str,
    run_id: Option<String>,
) -> Result<(), ProjectActionError> {
    validate_job_id(job_id)?;
    if reason.trim().is_empty() {
        return Err(ProjectActionError::EmptyJobFailureReason);
    }
    if updated_at.trim().is_empty() {
        return Err(ProjectActionError::EmptyJobUpdatedAt);
    }
    if run_id
        .as_ref()
        .is_some_and(|run_id| run_id.trim().is_empty())
    {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "runId".to_string(),
        ));
    }

    let job = project
        .jobs
        .iter_mut()
        .find(|job| job.id == job_id)
        .ok_or_else(|| ProjectActionError::JobNotFound(job_id.to_string()))?;
    if !matches!(
        job.status,
        JobStatus::Queued | JobStatus::Running | JobStatus::Progress | JobStatus::Blocked
    ) {
        return Ok(());
    }

    job.status = JobStatus::Failed;
    job.updated_at = updated_at.to_string();
    job.failure_reason = Some(reason.trim().to_string());
    if let (Some(workflow_run_id), Some(workflow)) = (run_id, job.workflow.as_mut()) {
        workflow.run_id = Some(workflow_run_id);
    }

    Ok(())
}

fn update_job_provider_request(
    project: &mut VideoProject,
    job_id: &str,
    provider_request: JobProviderRequest,
) -> Result<(), ProjectActionError> {
    validate_job_id(job_id)?;
    validate_job_provider_request(&provider_request)?;

    let job = project
        .jobs
        .iter_mut()
        .find(|job| job.id == job_id)
        .ok_or_else(|| ProjectActionError::JobNotFound(job_id.to_string()))?;
    if job.status == JobStatus::Cancelled {
        return Err(ProjectActionError::TerminalJobStatus(job_id.to_string()));
    }

    job.provider_request = Some(provider_request);

    Ok(())
}

fn validate_job_summary(
    project: &VideoProject,
    job: &JobSummary,
) -> Result<(), ProjectActionError> {
    validate_job_id(&job.id)?;
    if project.jobs.iter().any(|existing| existing.id == job.id) {
        return Err(ProjectActionError::DuplicateJobId(job.id.clone()));
    }
    if job.kind.trim().is_empty() {
        return Err(ProjectActionError::EmptyJobKind);
    }
    if job.updated_at.trim().is_empty() {
        return Err(ProjectActionError::EmptyJobUpdatedAt);
    }
    if let Some(workflow) = &job.workflow {
        validate_job_workflow(workflow)?;
    }
    if let Some(start_request) = &job.start_request {
        validate_job_start_request(start_request)?;
        if let Some(workflow) = &job.workflow {
            validate_job_start_request_matches_workflow(start_request, workflow)?;
        }
    }
    if let Some(provider_request) = &job.provider_request {
        validate_job_provider_request(provider_request)?;
    }
    if let Some(export_settings) = &job.export_settings {
        validate_job_export_settings(export_settings)?;
    }

    Ok(())
}

fn validate_job_export_settings(
    settings: &crate::project::export_options::JobExportSettings,
) -> Result<(), ProjectActionError> {
    let invalid = |message: String| ProjectActionError::InvalidJobExportSettings(message);
    let options = settings
        .options
        .validated()
        .map_err(|error| invalid(error.to_string()))?;
    if let Some(output) = &settings.output {
        let (_, extension, _) =
            crate::project::export_destination::export_artifact_contract(options.profile)
                .ok_or_else(|| invalid("project bundles don't record export settings".into()))?;
        crate::project::export_destination::validate_export_output_request(output, extension)
            .map_err(|error| invalid(error.to_string()))?;
    }
    Ok(())
}

fn validate_job_id(job_id: &str) -> Result<(), ProjectActionError> {
    if job_id.trim().is_empty() {
        return Err(ProjectActionError::EmptyJobId);
    }
    if !is_safe_path_segment(job_id) {
        return Err(ProjectActionError::InvalidJobId(job_id.to_string()));
    }

    Ok(())
}

fn validate_job_provider_request(
    provider_request: &JobProviderRequest,
) -> Result<(), ProjectActionError> {
    for (field, value) in [
        ("provider", provider_request.provider.as_str()),
        ("requestId", provider_request.request_id.as_str()),
        ("statusUrl", provider_request.status_url.as_str()),
        ("responseUrl", provider_request.response_url.as_str()),
        ("cancelUrl", provider_request.cancel_url.as_str()),
        ("submittedAt", provider_request.submitted_at.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ProjectActionError::MissingJobProviderRequestMetadata(
                field.to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_job_workflow(workflow: &TemporalWorkflowMetadata) -> Result<(), ProjectActionError> {
    for (field, value) in [
        ("workflowId", workflow.workflow_id.as_str()),
        ("workflowType", workflow.workflow_type.as_str()),
        ("taskQueue", workflow.task_queue.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ProjectActionError::MissingJobWorkflowMetadata(
                field.to_string(),
            ));
        }
    }
    if workflow
        .run_id
        .as_ref()
        .is_some_and(|run_id| run_id.trim().is_empty())
    {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "runId".to_string(),
        ));
    }
    if workflow.activity_types.is_empty()
        || workflow
            .activity_types
            .iter()
            .any(|activity_type| activity_type.trim().is_empty())
    {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "activityTypes".to_string(),
        ));
    }

    Ok(())
}

fn validate_job_start_request(
    start_request: &TemporalWorkflowStartRequest,
) -> Result<(), ProjectActionError> {
    for (field, value) in [
        ("workflowId", start_request.workflow_id.as_str()),
        ("workflowType", start_request.workflow_type.as_str()),
        ("taskQueue", start_request.task_queue.as_str()),
        ("idReusePolicy", start_request.id_reuse_policy.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ProjectActionError::MissingJobWorkflowMetadata(format!(
                "startRequest.{field}"
            )));
        }
    }

    if !start_request.search_attributes.is_object() {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "startRequest.searchAttributes".to_string(),
        ));
    }

    if start_request.activity_types.is_empty()
        || start_request
            .activity_types
            .iter()
            .any(|activity_type| activity_type.trim().is_empty())
    {
        return Err(ProjectActionError::MissingJobWorkflowMetadata(
            "startRequest.activityTypes".to_string(),
        ));
    }

    Ok(())
}

fn validate_job_start_request_matches_workflow(
    start_request: &TemporalWorkflowStartRequest,
    workflow: &TemporalWorkflowMetadata,
) -> Result<(), ProjectActionError> {
    if start_request.workflow_id != workflow.workflow_id {
        return Err(ProjectActionError::MismatchedJobStartRequest(
            "workflowId".to_string(),
        ));
    }
    if start_request.workflow_type != workflow.workflow_type {
        return Err(ProjectActionError::MismatchedJobStartRequest(
            "workflowType".to_string(),
        ));
    }
    if start_request.task_queue != workflow.task_queue {
        return Err(ProjectActionError::MismatchedJobStartRequest(
            "taskQueue".to_string(),
        ));
    }
    if start_request.activity_types != workflow.activity_types {
        return Err(ProjectActionError::MismatchedJobStartRequest(
            "activityTypes".to_string(),
        ));
    }

    Ok(())
}

fn validate_transcript_word_range(
    word: &TranscriptWord,
    media_id: &str,
    media_duration: f64,
) -> Result<(), ProjectActionError> {
    if !word.start_seconds.is_finite() || word.start_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !word.end_seconds.is_finite() || word.end_seconds < word.start_seconds {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    if word.end_seconds > media_duration {
        return Err(ProjectActionError::SourceRangeOutsideMedia(
            media_id.to_string(),
        ));
    }

    Ok(())
}

fn validate_transcript_timing(transcript: &Transcript) -> Result<(), ProjectActionError> {
    let mut previous_end = None;
    for word in &transcript.words {
        if let Some(previous) = previous_end {
            if word.start_seconds < previous {
                return Err(ProjectActionError::NonMonotonicTranscriptTiming(
                    transcript.id.clone(),
                ));
            }
        }
        previous_end = Some(word.end_seconds);
    }

    Ok(())
}

fn transcript_repair_kind(before: &TranscriptWord, after: &TranscriptWord) -> TranscriptRepairKind {
    let text_changed = before.text != after.text;
    let timing_changed = !nearly_equal(before.start_seconds, after.start_seconds)
        || !nearly_equal(before.end_seconds, after.end_seconds);

    match (text_changed, timing_changed) {
        (true, true) => TranscriptRepairKind::WordTextAndTiming,
        (true, false) => TranscriptRepairKind::WordText,
        (false, true) | (false, false) => TranscriptRepairKind::WordTiming,
    }
}

const KEYFRAME_TIME_EPSILON: f64 = 0.000_000_1;

pub(crate) fn item_speed(item: &TimelineItem) -> Result<f64, ProjectActionError> {
    let speed = item
        .properties
        .get("speed")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(1.0);
    if speed.is_finite() && speed > 0.0 {
        Ok(speed)
    } else {
        Err(ProjectActionError::InvalidVisualClipSpeed(item.id.clone()))
    }
}

fn item_properties_for_subrange(
    properties: &BTreeMap<String, serde_json::Value>,
    start_seconds: f64,
    end_seconds: f64,
    original_duration_seconds: f64,
) -> Result<BTreeMap<String, serde_json::Value>, ProjectActionError> {
    if !start_seconds.is_finite()
        || !end_seconds.is_finite()
        || !original_duration_seconds.is_finite()
        || end_seconds <= start_seconds
        || original_duration_seconds <= 0.0
    {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    let mut next = properties.clone();
    if let Some(keyframes) = properties.get("keyframes") {
        next.insert(
            "keyframes".to_string(),
            partition_keyframe_lanes(
                keyframes,
                start_seconds,
                end_seconds,
                original_duration_seconds,
                "keyframes",
            )?,
        );
    }
    if let Some(effect_keyframes) = properties.get("effectParameterKeyframes") {
        let instances = effect_keyframes.as_object().ok_or_else(|| {
            ProjectActionError::InvalidEffectParam(
                "effectParameterKeyframes must be an object".to_string(),
            )
        })?;
        let mut next_instances = serde_json::Map::new();
        for (instance_id, parameters) in instances {
            next_instances.insert(
                instance_id.clone(),
                partition_keyframe_lanes(
                    parameters,
                    start_seconds,
                    end_seconds,
                    original_duration_seconds,
                    &format!("effectParameterKeyframes.{instance_id}"),
                )?,
            );
        }
        next.insert(
            "effectParameterKeyframes".to_string(),
            serde_json::Value::Object(next_instances),
        );
    }
    preserve_subrange_fades(
        &mut next,
        start_seconds,
        end_seconds,
        original_duration_seconds,
    );
    Ok(next)
}

fn partition_keyframe_lanes(
    lanes: &serde_json::Value,
    start_seconds: f64,
    end_seconds: f64,
    original_duration_seconds: f64,
    path: &str,
) -> Result<serde_json::Value, ProjectActionError> {
    let lanes = lanes.as_object().ok_or_else(|| {
        ProjectActionError::InvalidEffectParam(format!("{path} must be an object"))
    })?;
    let mut next = serde_json::Map::new();
    for (lane_name, lane) in lanes {
        let keyframes: Vec<ProjectActionKeyframe> =
            serde_json::from_value(lane.clone()).map_err(|_| {
                ProjectActionError::InvalidEffectParam(format!(
                    "{path}.{lane_name} must contain canonical numeric keyframes"
                ))
            })?;
        next.insert(
            lane_name.clone(),
            serde_json::to_value(partition_keyframe_lane(
                &keyframes,
                start_seconds,
                end_seconds,
                original_duration_seconds,
            )?)
            .expect("partitioned keyframes serialize"),
        );
    }
    Ok(serde_json::Value::Object(next))
}

fn partition_keyframe_lane(
    keyframes: &[ProjectActionKeyframe],
    start_seconds: f64,
    end_seconds: f64,
    original_duration_seconds: f64,
) -> Result<Vec<ProjectActionKeyframe>, ProjectActionError> {
    if keyframes.is_empty() {
        return Ok(Vec::new());
    }
    let mut canonical = keyframes.to_vec();
    canonicalize_keyframes(&mut canonical)?;
    validate_keyframes(&canonical, original_duration_seconds, f64::MIN, f64::MAX)?;

    let mut next = canonical
        .iter()
        .filter(|keyframe| {
            keyframe.at_seconds + KEYFRAME_TIME_EPSILON >= start_seconds
                && keyframe.at_seconds <= end_seconds + KEYFRAME_TIME_EPSILON
        })
        .cloned()
        .map(|mut keyframe| {
            keyframe.at_seconds =
                (keyframe.at_seconds - start_seconds).clamp(0.0, end_seconds - start_seconds);
            keyframe
        })
        .collect::<Vec<_>>();

    if start_seconds.abs() > KEYFRAME_TIME_EPSILON
        && !next
            .first()
            .is_some_and(|keyframe| keyframe.at_seconds <= KEYFRAME_TIME_EPSILON)
    {
        next.insert(
            0,
            sampled_boundary_keyframe(&canonical, start_seconds, 0.0)?,
        );
    }
    let next_duration = end_seconds - start_seconds;
    if end_seconds < original_duration_seconds - KEYFRAME_TIME_EPSILON
        && !next.last().is_some_and(|keyframe| {
            (keyframe.at_seconds - next_duration).abs() <= KEYFRAME_TIME_EPSILON
        })
    {
        next.push(sampled_boundary_keyframe(
            &canonical,
            end_seconds,
            next_duration,
        )?);
    }
    canonicalize_keyframes(&mut next)?;
    Ok(next)
}

fn sampled_boundary_keyframe(
    keyframes: &[ProjectActionKeyframe],
    sample_seconds: f64,
    output_seconds: f64,
) -> Result<ProjectActionKeyframe, ProjectActionError> {
    let numeric = keyframes
        .iter()
        .map(|keyframe| {
            Ok(NumericKeyframe {
                at_seconds: keyframe.at_seconds,
                value: keyframe.value,
                easing: keyframe_easing(keyframe.easing.as_deref())?,
            })
        })
        .collect::<Result<Vec<_>, ProjectActionError>>()?;
    let easing = keyframes
        .iter()
        .rev()
        .find(|keyframe| keyframe.at_seconds <= sample_seconds + KEYFRAME_TIME_EPSILON)
        .or_else(|| keyframes.first())
        .and_then(|keyframe| keyframe.easing.clone());
    Ok(ProjectActionKeyframe {
        at_seconds: output_seconds,
        value: NumericCurve {
            base: keyframes[0].value,
            keyframes: numeric,
        }
        .sample(sample_seconds),
        easing,
    })
}

fn keyframe_easing(easing: Option<&str>) -> Result<KeyframeEasing, ProjectActionError> {
    match canonical_keyframe_easing(easing)?.as_deref() {
        None | Some("linear") => Ok(KeyframeEasing::Linear),
        Some("hold") => Ok(KeyframeEasing::Hold),
        Some("easeIn") => Ok(KeyframeEasing::EaseIn),
        Some("easeOut") => Ok(KeyframeEasing::EaseOut),
        Some("easeInOut") => Ok(KeyframeEasing::EaseInOut),
        Some(_) => unreachable!("canonical_keyframe_easing returned a known value"),
    }
}

fn preserve_subrange_fades(
    properties: &mut BTreeMap<String, serde_json::Value>,
    start_seconds: f64,
    end_seconds: f64,
    original_duration_seconds: f64,
) {
    let duration = end_seconds - start_seconds;
    if start_seconds > KEYFRAME_TIME_EPSILON {
        properties.remove("fadeInSeconds");
    } else if let Some(fade_in) = properties
        .get("fadeInSeconds")
        .and_then(serde_json::Value::as_f64)
    {
        properties.insert(
            "fadeInSeconds".to_string(),
            serde_json::json!(fade_in.min(duration)),
        );
    }
    if end_seconds < original_duration_seconds - KEYFRAME_TIME_EPSILON {
        properties.remove("fadeOutSeconds");
    } else if let Some(fade_out) = properties
        .get("fadeOutSeconds")
        .and_then(serde_json::Value::as_f64)
    {
        properties.insert(
            "fadeOutSeconds".to_string(),
            serde_json::json!(fade_out.min(duration)),
        );
    }
}

pub(crate) fn item_source_range(item: &TimelineItem) -> Option<(f64, f64)> {
    if !matches!(item.source, TimelineSource::Media { .. }) {
        return None;
    }
    let source_in = item
        .properties
        .get("sourceIn")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    let source_out = item
        .properties
        .get("sourceOut")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| {
            source_in
                + item.duration_seconds
                    * item
                        .properties
                        .get("speed")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(1.0)
        });
    Some((source_in, source_out))
}

fn set_item_source_range(item: &mut TimelineItem, source_in: f64, source_out: f64) {
    item.properties
        .insert("sourceIn".to_string(), serde_json::json!(source_in));
    item.properties
        .insert("sourceOut".to_string(), serde_json::json!(source_out));
}

fn generated_ripple_item_id(
    item_id: &str,
    range_end_seconds: f64,
    existing_ids: &BTreeSet<String>,
    appended_ids: &BTreeSet<String>,
) -> String {
    let base = format!(
        "{}-ripple-{}",
        item_id,
        (range_end_seconds.max(0.0) * 1000.0).round() as u64
    );
    if !existing_ids.contains(&base) && !appended_ids.contains(&base) {
        return base;
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !existing_ids.contains(&candidate) && !appended_ids.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn generated_overwrite_item_id(
    item_id: &str,
    range_end_seconds: f64,
    existing_ids: &BTreeSet<String>,
    appended_ids: &BTreeSet<String>,
) -> String {
    let base = format!(
        "{}-overwrite-{}",
        item_id,
        (range_end_seconds.max(0.0) * 1000.0).round() as u64
    );
    if !existing_ids.contains(&base) && !appended_ids.contains(&base) {
        return base;
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !existing_ids.contains(&candidate) && !appended_ids.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn update_template_items(
    project: &mut VideoProject,
    updates: &[ProjectActionTemplateUpdate],
) -> Result<(), ProjectActionError> {
    if updates.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for update in updates {
        if !update.start_seconds.is_finite() || update.start_seconds < 0.0 {
            return Err(ProjectActionError::NegativeTime);
        }
        if !update.duration_seconds.is_finite() || update.duration_seconds <= 0.0 {
            return Err(ProjectActionError::NonPositiveDuration);
        }
        let (track_index, item_index) = find_item(&project.timeline, &update.item_id)
            .ok_or_else(|| ProjectActionError::ItemNotFound(update.item_id.clone()))?;
        if project.timeline.tracks[track_index].locked {
            return Err(ProjectActionError::TrackLocked(
                project.timeline.tracks[track_index].id.clone(),
            ));
        }
        let item = &project.timeline.tracks[track_index].items[item_index];
        if !is_template_item(item) {
            return Err(ProjectActionError::NotTemplateItem(update.item_id.clone()));
        }
    }

    for update in updates {
        let (track_index, item_index) =
            find_item(&project.timeline, &update.item_id).expect("template prevalidated item");
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        item.start_seconds = update.start_seconds;
        item.duration_seconds = update.duration_seconds;
        item.properties.insert(
            "templateFields".to_string(),
            serde_json::to_value(&update.template_fields).expect("serialize template fields"),
        );
        sort_track_items(&mut project.timeline.tracks[track_index]);
    }

    Ok(())
}

fn update_text_overlay_items(
    project: &mut VideoProject,
    updates: &[ProjectActionTextOverlayUpdate],
) -> Result<(), ProjectActionError> {
    if updates.is_empty() {
        return Err(ProjectActionError::EmptyItems);
    }

    for update in updates {
        validate_text_overlay_update(project, update)?;
    }

    for update in updates {
        let (track_index, item_index) =
            find_item(&project.timeline, &update.item_id).expect("overlay prevalidated item");
        let item = &mut project.timeline.tracks[track_index].items[item_index];
        let TimelineSource::Text { text } = &mut item.source else {
            unreachable!("text overlay prevalidated source");
        };

        let trimmed_text = update.text.trim().to_string();
        let trimmed_visual_treatment = update.visual_treatment.trim().to_string();
        let trimmed_motion = update.motion.trim().to_string();
        let trimmed_safe_zone = update.safe_zone.trim().to_string();
        let trimmed_avoid = update.avoid.trim().to_string();

        item.start_seconds = update.start_seconds;
        item.duration_seconds = update.duration_seconds;
        item.label = trimmed_text.clone();
        *text = trimmed_text.clone();
        item.properties
            .insert("text".to_string(), serde_json::json!(trimmed_text));
        item.properties.insert(
            "visualTreatment".to_string(),
            serde_json::json!(trimmed_visual_treatment),
        );
        item.properties
            .insert("motion".to_string(), serde_json::json!(trimmed_motion));
        item.properties
            .insert("safeZone".to_string(), serde_json::json!(trimmed_safe_zone));
        item.properties
            .insert("avoid".to_string(), serde_json::json!(trimmed_avoid));
        if let Some(font_name) = update.font_name.as_deref() {
            item.properties.insert(
                "fontName".to_string(),
                serde_json::json!(font_name.trim().to_string()),
            );
        }
        if let Some(font_size) = update.font_size {
            item.properties
                .insert("fontSize".to_string(), serde_json::json!(font_size));
        }
        if let Some(color) = update.color.as_deref() {
            item.properties.insert(
                "color".to_string(),
                serde_json::json!(color.trim().to_string()),
            );
        }
        if let Some(alignment) = update.alignment.as_deref() {
            item.properties.insert(
                "alignment".to_string(),
                serde_json::json!(alignment.trim().to_string()),
            );
        }
        item.properties
            .insert("textEdited".to_string(), serde_json::json!(true));
        sort_track_items(&mut project.timeline.tracks[track_index]);
    }

    Ok(())
}

fn validate_text_overlay_update(
    project: &VideoProject,
    update: &ProjectActionTextOverlayUpdate,
) -> Result<(), ProjectActionError> {
    if !update.start_seconds.is_finite() || update.start_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !update.duration_seconds.is_finite() || update.duration_seconds <= 0.0 {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    if update.text.trim().is_empty() {
        return Err(ProjectActionError::EmptyTextItemText);
    }
    for (field, value) in [
        ("visualTreatment", &update.visual_treatment),
        ("motion", &update.motion),
        ("safeZone", &update.safe_zone),
        ("avoid", &update.avoid),
    ] {
        if value.trim().is_empty() {
            return Err(ProjectActionError::MissingTextOverlayVisualMetadata(
                field.to_string(),
            ));
        }
    }
    for (field, value) in [
        ("fontName", update.font_name.as_deref()),
        ("color", update.color.as_deref()),
        ("alignment", update.alignment.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(ProjectActionError::MissingTextOverlayVisualMetadata(
                field.to_string(),
            ));
        }
    }
    if update
        .font_size
        .is_some_and(|font_size| !font_size.is_finite() || font_size <= 0.0)
    {
        return Err(ProjectActionError::MissingTextOverlayVisualMetadata(
            "fontSize".to_string(),
        ));
    }

    let (track_index, item_index) = find_item(&project.timeline, &update.item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(update.item_id.clone()))?;
    if project.timeline.tracks[track_index].locked {
        return Err(ProjectActionError::TrackLocked(
            project.timeline.tracks[track_index].id.clone(),
        ));
    }

    let item = &project.timeline.tracks[track_index].items[item_index];
    if item.kind != TimelineItemKind::Overlay || !matches!(item.source, TimelineSource::Text { .. })
    {
        return Err(ProjectActionError::NotTextOverlayItem(
            update.item_id.clone(),
        ));
    }

    Ok(())
}

fn update_template_override(
    project: &mut VideoProject,
    override_update: ProjectActionTemplateOverrideUpdate,
) -> Result<(), ProjectActionError> {
    validate_template_override_update(&override_update)?;

    let template_override = ProjectTemplateOverride {
        schema_version: super::split::SPLIT_TEMPLATE_SCHEMA_VERSION,
        template_id: override_update.template_id.trim().to_string(),
        name: override_update.name.trim().to_string(),
        fields: trim_template_fields(override_update.fields),
        style: override_update.style,
        visual_treatment: override_update.visual_treatment.trim().to_string(),
        motion: override_update.motion.trim().to_string(),
        safe_zone: override_update.safe_zone.trim().to_string(),
        avoid: override_update.avoid.trim().to_string(),
    };

    if let Some(existing) = project
        .template_overrides
        .iter_mut()
        .find(|existing| existing.template_id == template_override.template_id)
    {
        *existing = template_override;
    } else {
        project.template_overrides.push(template_override);
    }

    Ok(())
}

fn validate_template_override_update(
    override_update: &ProjectActionTemplateOverrideUpdate,
) -> Result<(), ProjectActionError> {
    if override_update.template_id.trim().is_empty() {
        return Err(ProjectActionError::EmptyTemplateId);
    }
    if override_update.name.trim().is_empty() {
        return Err(ProjectActionError::EmptyTemplateName);
    }
    for (field, value) in [
        ("visualTreatment", &override_update.visual_treatment),
        ("motion", &override_update.motion),
        ("safeZone", &override_update.safe_zone),
        ("avoid", &override_update.avoid),
    ] {
        if value.trim().is_empty() {
            return Err(ProjectActionError::MissingTemplateVisualMetadata(
                field.to_string(),
            ));
        }
    }

    Ok(())
}

fn trim_template_fields(fields: BTreeMap<String, String>) -> BTreeMap<String, String> {
    fields
        .into_iter()
        .map(|(key, value)| (key, value.trim().to_string()))
        .collect()
}

fn validate_item_for_add(
    project: &VideoProject,
    target_track: &TimelineTrack,
    item: &TimelineItem,
) -> Result<(), ProjectActionError> {
    if find_item(&project.timeline, &item.id).is_some() {
        return Err(ProjectActionError::DuplicateItemId(item.id.clone()));
    }
    if !item.start_seconds.is_finite() || item.start_seconds < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !item.duration_seconds.is_finite() || item.duration_seconds <= 0.0 {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    if !item_allowed_on_track(&item.kind, &target_track.kind) {
        return Err(ProjectActionError::TrackTypeMismatch {
            item_kind: item.kind.clone(),
            track_kind: target_track.kind.clone(),
        });
    }
    if let TimelineSource::Media { media_id } = &item.source {
        let media = project
            .media
            .iter()
            .find(|media| media.id == *media_id)
            .ok_or_else(|| ProjectActionError::MissingMedia(media_id.clone()))?;
        validate_generated_edit_source_range(item, media.duration_seconds)?;
    }

    Ok(())
}

fn is_video_track_clip_kind(kind: &TimelineItemKind) -> bool {
    matches!(
        kind,
        TimelineItemKind::VideoClip
            | TimelineItemKind::ImageClip
            | TimelineItemKind::LottieClip
            | TimelineItemKind::GeneratedClip
    )
}

fn is_visual_clip_kind(kind: &TimelineItemKind) -> bool {
    is_video_track_clip_kind(kind)
        || matches!(
            kind,
            TimelineItemKind::Overlay | TimelineItemKind::HyperframeScene
        )
}

fn validate_generated_edit_source_range(
    item: &TimelineItem,
    media_duration: f64,
) -> Result<(), ProjectActionError> {
    if !item
        .properties
        .get("generatedEdit")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(());
    }

    let source_in = item
        .properties
        .get("sourceIn")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| ProjectActionError::MissingGeneratedEditSourceRange(item.id.clone()))?;
    let source_out = item
        .properties
        .get("sourceOut")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| ProjectActionError::MissingGeneratedEditSourceRange(item.id.clone()))?;

    if !source_in.is_finite() || source_in < 0.0 {
        return Err(ProjectActionError::NegativeTime);
    }
    if !source_out.is_finite() || source_out <= source_in {
        return Err(ProjectActionError::NonPositiveDuration);
    }
    if source_out > media_duration {
        return Err(ProjectActionError::SourceRangeOutsideMedia(item.id.clone()));
    }
    let speed = item_speed(item)?;
    if !nearly_equal(source_out - source_in, item.duration_seconds * speed) {
        return Err(ProjectActionError::SourceRangeDurationMismatch(
            item.id.clone(),
        ));
    }
    if nearly_equal(source_in, 0.0) && nearly_equal(source_out, media_duration) {
        return Err(ProjectActionError::GeneratedEditFullSourcePassThrough(
            item.id.clone(),
        ));
    }

    Ok(())
}

fn is_template_item(item: &TimelineItem) -> bool {
    item.properties
        .get("templateId")
        .and_then(serde_json::Value::as_str)
        .is_some()
}

fn find_track(timeline: &Timeline, track_id: &str) -> Option<usize> {
    timeline
        .tracks
        .iter()
        .position(|track| track.id == track_id)
}

pub(super) fn find_item(timeline: &Timeline, item_id: &str) -> Option<(usize, usize)> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .items
                .iter()
                .position(|item| item.id == item_id)
                .map(|item_index| (track_index, item_index))
        })
}

fn item_allowed_on_track(item_kind: &TimelineItemKind, track_kind: &TrackKind) -> bool {
    matches!(
        (item_kind, track_kind),
        (TimelineItemKind::VideoClip, TrackKind::Video)
            | (TimelineItemKind::ImageClip, TrackKind::Video)
            | (TimelineItemKind::LottieClip, TrackKind::Video)
            | (TimelineItemKind::GeneratedClip, TrackKind::Video)
            | (
                TimelineItemKind::HyperframeScene,
                TrackKind::HyperframeScene
            )
            | (TimelineItemKind::Overlay, TrackKind::Overlay)
            | (TimelineItemKind::Caption, TrackKind::Caption)
            | (TimelineItemKind::AudioClip, TrackKind::Audio)
    )
}

fn sort_track_items(track: &mut TimelineTrack) {
    track.items.sort_by(|left, right| {
        left.start_seconds
            .partial_cmp(&right.start_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn nearly_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 0.000_001
}

fn recalculate_duration(timeline: &mut Timeline) {
    timeline.duration_seconds = timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.start_seconds + item.duration_seconds)
        .fold(0.0, f64::max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;

    #[test]
    fn local_render_terminal_updates_require_the_active_attempt_and_are_final() {
        let mut project = sample_project();
        project.jobs.push(JobSummary {
            id: "render-1".into(),
            kind: "render_draft".into(),
            status: JobStatus::Running,
            updated_at: "2026-09-12T00:00:00Z".into(),
            workflow: Some(TemporalWorkflowMetadata {
                workflow_id: "render-1".into(),
                workflow_type: "render".into(),
                task_queue: "render".into(),
                run_id: Some("render-attempt/current".into()),
                activity_types: vec!["render".into()],
            }),
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        });

        let stale = apply_project_action(
            &mut project,
            ProjectAction::UpdateJobStatus {
                job_id: "render-1".into(),
                status: JobStatus::Cancelled,
                updated_at: "2026-09-12T00:00:01Z".into(),
                run_id: Some("render-attempt/stale".into()),
            },
        )
        .expect_err("stale attempt must not finalize canonical state");
        assert!(matches!(stale, ProjectActionError::StaleRenderAttempt(_)));

        apply_project_action(
            &mut project,
            ProjectAction::UpdateJobStatus {
                job_id: "render-1".into(),
                status: JobStatus::Failed,
                updated_at: "2026-09-12T00:00:02Z".into(),
                run_id: Some("render-attempt/current".into()),
            },
        )
        .expect("active attempt can finalize");
        let overwrite = apply_project_action(
            &mut project,
            ProjectAction::UpdateJobStatus {
                job_id: "render-1".into(),
                status: JobStatus::Cancelled,
                updated_at: "2026-09-12T00:00:03Z".into(),
                run_id: Some("render-attempt/current".into()),
            },
        )
        .expect_err("terminal outcome must not be rewritten");
        assert!(matches!(
            overwrite,
            ProjectActionError::TerminalJobStatus(_)
        ));

        let implicit_retry = apply_project_action(
            &mut project,
            ProjectAction::UpdateJobStatus {
                job_id: "render-1".into(),
                status: JobStatus::Running,
                updated_at: "2026-09-12T00:00:04Z".into(),
                run_id: Some("render-attempt/current".into()),
            },
        )
        .expect_err("the same terminal attempt cannot restart");
        assert!(matches!(
            implicit_retry,
            ProjectActionError::TerminalJobStatus(_)
        ));

        apply_project_action(
            &mut project,
            ProjectAction::UpdateJobStatus {
                job_id: "render-1".into(),
                status: JobStatus::Running,
                updated_at: "2026-09-12T00:00:05Z".into(),
                run_id: Some("render-attempt/retry".into()),
            },
        )
        .expect("a new explicit attempt can retry a failed render");
    }

    #[test]
    fn repeated_local_render_terminal_updates_preserve_all_metadata() {
        for status in [
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ] {
            let mut project = sample_project();
            project.content_revision = 41;
            project.jobs.push(JobSummary {
                id: "render-1".into(),
                kind: "render_draft".into(),
                status: status.clone(),
                updated_at: "2026-09-12T00:00:00Z".into(),
                workflow: Some(TemporalWorkflowMetadata {
                    workflow_id: "render-1".into(),
                    workflow_type: "render".into(),
                    task_queue: "render".into(),
                    run_id: Some("render-attempt/current".into()),
                    activity_types: vec!["render".into()],
                }),
                start_request: None,
                provider_request: None,
                failure_reason: None,
                export_settings: None,
            });
            let before = project.clone();
            let error = apply_project_action(
                &mut project,
                ProjectAction::UpdateJobStatus {
                    job_id: "render-1".into(),
                    status,
                    updated_at: "2026-09-12T00:01:00Z".into(),
                    run_id: Some("render-attempt/current".into()),
                },
            )
            .expect_err("a terminal attempt cannot be finalized twice");
            assert!(matches!(error, ProjectActionError::TerminalJobStatus(_)));
            assert_eq!(project, before);
            assert_eq!(project.content_revision, 41);
        }
    }

    #[test]
    fn update_project_settings_changes_name_and_render_settings_atomically() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::UpdateProjectSettings {
                name: "Interview cut".into(),
                render_settings: RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
            },
        )
        .expect("valid project settings");

        assert_eq!(project.name, "Interview cut");
        assert_eq!(project.render_settings.width, 3840);
        assert_eq!(project.render_settings.height, 2160);
        assert_eq!(project.render_settings.fps, 24.0);
        assert_eq!(project.render_settings.loudness_lufs, -16.0);
        assert_eq!(project.render_settings.captions, CaptionRenderMode::Mux);
    }

    #[test]
    fn update_project_settings_rejects_blank_name_without_partial_mutation() {
        let mut project = sample_project();
        let before = project.clone();

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateProjectSettings {
                name: "  ".into(),
                render_settings: RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
            },
        );

        assert!(result.is_err());
        assert_eq!(project, before);
    }

    #[test]
    fn update_project_settings_rejects_invalid_format_without_partial_mutation() {
        for render_settings in [
            RenderSettings {
                width: 1919,
                height: 1080,
                fps: 24.0,
                loudness_lufs: -14.0,
                captions: CaptionRenderMode::BurnIn,
            },
            RenderSettings {
                width: 1920,
                height: 16_386,
                fps: 24.0,
                loudness_lufs: -14.0,
                captions: CaptionRenderMode::BurnIn,
            },
            RenderSettings {
                width: 1920,
                height: 1080,
                fps: 27.0,
                loudness_lufs: -14.0,
                captions: CaptionRenderMode::BurnIn,
            },
            RenderSettings {
                width: 1920,
                height: 1080,
                fps: 24.0,
                loudness_lufs: f64::NAN,
                captions: CaptionRenderMode::BurnIn,
            },
        ] {
            let mut project = sample_project();
            let before = project.clone();

            let result = apply_project_action(
                &mut project,
                ProjectAction::UpdateProjectSettings {
                    name: "Interview cut".into(),
                    render_settings,
                },
            );

            assert!(result.is_err());
            assert_eq!(project, before);
        }
    }

    #[test]
    fn update_project_settings_rejects_unknown_caption_mode_during_deserialization() {
        let result = serde_json::from_value::<ProjectAction>(serde_json::json!({
            "type": "updateProjectSettings",
            "name": "Interview cut",
            "renderSettings": {
                "width": 1920,
                "height": 1080,
                "fps": 24.0,
                "loudnessLufs": -14.0,
                "captions": "sidecar"
            }
        }));

        assert!(result.is_err());
    }

    fn project_with_adjacent_video_item() -> VideoProject {
        let mut project = sample_project();
        let video_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-video")
            .expect("video track");
        video_track.items.push(TimelineItem {
            id: "video-after".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 4.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Following clip".to_string(),
            properties: BTreeMap::new(),
        });
        project.timeline.duration_seconds = 8.0;
        project
    }

    #[test]
    fn move_items_reject_new_same_track_overlap() {
        let mut project = project_with_adjacent_video_item();
        let before = project.clone();

        let result = apply_project_action(
            &mut project,
            ProjectAction::MoveItems {
                moves: vec![ProjectActionMove {
                    item_id: "item-1".to_string(),
                    target_track_id: "track-video".to_string(),
                    start_seconds: 1.0,
                }],
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackItemOverlap {
                track_id: "track-video".to_string(),
                item_id: "item-1".to_string(),
                blocking_item_id: "video-after".to_string(),
            })
        );
        assert_eq!(project, before);
    }

    #[test]
    fn resize_items_reject_new_same_track_overlap() {
        let mut project = project_with_adjacent_video_item();
        let before = project.clone();

        let result = apply_project_action(
            &mut project,
            ProjectAction::ResizeItems {
                resizes: vec![ProjectActionResize {
                    item_id: "item-1".to_string(),
                    duration_seconds: 5.0,
                }],
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackItemOverlap {
                track_id: "track-video".to_string(),
                item_id: "item-1".to_string(),
                blocking_item_id: "video-after".to_string(),
            })
        );
        assert_eq!(project, before);
    }

    #[test]
    fn trim_items_reject_new_same_track_overlap() {
        let mut project = project_with_adjacent_video_item();
        let before = project.clone();

        let result = apply_project_action(
            &mut project,
            ProjectAction::TrimItems {
                trims: vec![ProjectActionTrim {
                    item_id: "item-1".to_string(),
                    start_seconds: 0.0,
                    duration_seconds: 5.0,
                    source_in: None,
                    source_out: None,
                }],
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackItemOverlap {
                track_id: "track-video".to_string(),
                item_id: "item-1".to_string(),
                blocking_item_id: "video-after".to_string(),
            })
        );
        assert_eq!(project, before);
    }

    #[test]
    fn move_items_allow_unchanged_legacy_same_track_overlap() {
        let mut project = sample_project();
        let caption_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-captions")
            .expect("caption track");
        caption_track.items.extend([
            TimelineItem {
                id: "caption-1".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source: TimelineSource::Text {
                    text: "First caption".to_string(),
                },
                label: "First caption".to_string(),
                properties: BTreeMap::new(),
            },
            TimelineItem {
                id: "caption-2".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 1.75,
                duration_seconds: 2.0,
                source: TimelineSource::Text {
                    text: "Second caption".to_string(),
                },
                label: "Second caption".to_string(),
                properties: BTreeMap::new(),
            },
        ]);

        apply_project_action(
            &mut project,
            ProjectAction::MoveItems {
                moves: vec![
                    ProjectActionMove {
                        item_id: "caption-1".to_string(),
                        target_track_id: "track-captions".to_string(),
                        start_seconds: 1.0,
                    },
                    ProjectActionMove {
                        item_id: "caption-2".to_string(),
                        target_track_id: "track-captions".to_string(),
                        start_seconds: 2.75,
                    },
                ],
            },
        )
        .expect("unchanged legacy overlap should remain loadable");

        let caption_track = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-captions")
            .expect("caption track");
        assert_eq!(caption_track.items[0].start_seconds, 1.0);
        assert_eq!(caption_track.items[1].start_seconds, 2.75);
    }

    fn overlay_item(
        id: &str,
        start_seconds: f64,
        duration_seconds: f64,
        template: bool,
    ) -> TimelineItem {
        let mut properties = BTreeMap::new();
        if template {
            properties.insert("templateId".to_string(), serde_json::json!("lower-third"));
        }
        TimelineItem {
            id: id.to_string(),
            kind: TimelineItemKind::Overlay,
            start_seconds,
            duration_seconds,
            source: TimelineSource::Text {
                text: id.to_string(),
            },
            label: id.to_string(),
            properties,
        }
    }

    #[test]
    fn geometry_updaters_reject_new_overlap_atomically() {
        let actions = [
            ProjectAction::UpdateTemplateItems {
                updates: vec![ProjectActionTemplateUpdate {
                    item_id: "target".to_string(),
                    start_seconds: 0.5,
                    duration_seconds: 1.0,
                    template_fields: BTreeMap::new(),
                }],
            },
            ProjectAction::UpdateTextOverlayItems {
                updates: vec![ProjectActionTextOverlayUpdate {
                    item_id: "target".to_string(),
                    start_seconds: 0.5,
                    duration_seconds: 1.0,
                    text: "Target".to_string(),
                    visual_treatment: "Lower third".to_string(),
                    motion: "Fade".to_string(),
                    safe_zone: "Lower safe area".to_string(),
                    avoid: "Faces".to_string(),
                    font_name: None,
                    font_size: None,
                    color: None,
                    alignment: None,
                }],
            },
        ];

        for action in actions {
            let mut project = sample_project();
            let overlay_track = project
                .timeline
                .tracks
                .iter_mut()
                .find(|track| track.id == "track-overlays")
                .expect("overlay track");
            overlay_track.items = vec![
                overlay_item("target", 0.0, 1.0, true),
                overlay_item("blocker", 1.25, 1.0, false),
            ];
            let before = project.clone();

            assert_eq!(
                apply_project_action(&mut project, action),
                Err(ProjectActionError::TrackItemOverlap {
                    track_id: "track-overlays".to_string(),
                    item_id: "blocker".to_string(),
                    blocking_item_id: "target".to_string(),
                })
            );
            assert_eq!(project, before);
        }
    }

    #[test]
    fn caption_repair_rejects_new_overlap_atomically() {
        let mut project = sample_project();
        let caption_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-captions")
            .expect("caption track");
        caption_track.items = vec![
            TimelineItem {
                id: "caption-target".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 0.0,
                duration_seconds: 1.0,
                source: TimelineSource::Text {
                    text: "Target".to_string(),
                },
                label: "Target".to_string(),
                properties: BTreeMap::new(),
            },
            TimelineItem {
                id: "caption-blocker".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 1.25,
                duration_seconds: 1.0,
                source: TimelineSource::Text {
                    text: "Blocker".to_string(),
                },
                label: "Blocker".to_string(),
                properties: BTreeMap::new(),
            },
        ];
        project.transcripts.push(Transcript {
            id: "transcript".to_string(),
            media_id: "media-1".to_string(),
            engine: None,
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: vec![TranscriptWord {
                text: "Target".to_string(),
                start_seconds: 0.0,
                end_seconds: 1.0,
                confidence: None,
                speaker: None,
            }],
        });
        let before = project.clone();

        assert_eq!(
            apply_project_action(
                &mut project,
                ProjectAction::ApplyCaptionRepair {
                    repair: ProjectActionCaptionRepair {
                        caption_item_id: "caption-target".to_string(),
                        transcript_id: "transcript".to_string(),
                        word_index: 0,
                        text: "Target".to_string(),
                        start_seconds: 0.5,
                        end_seconds: 1.5,
                        repair_id: "repair".to_string(),
                        created_at: "2026-07-19T00:00:00Z".to_string(),
                    },
                },
            ),
            Err(ProjectActionError::TrackItemOverlap {
                track_id: "track-captions".to_string(),
                item_id: "caption-blocker".to_string(),
                blocking_item_id: "caption-target".to_string(),
            })
        );
        assert_eq!(project, before);
    }

    #[test]
    fn split_allows_unchanged_legacy_overlap_with_new_right_id() {
        let mut project = project_with_adjacent_video_item();
        project.timeline.tracks[0].items[1].start_seconds = 2.0;

        apply_project_action(
            &mut project,
            ProjectAction::SplitItems {
                splits: vec![ProjectActionSplit {
                    item_id: "item-1".to_string(),
                    new_item_id: "item-1-right".to_string(),
                    split_seconds: 1.0,
                }],
            },
        )
        .expect("split should preserve, not create, the legacy overlap");
    }

    #[test]
    fn moving_a_legacy_overlap_between_tracks_is_allowed_but_worsening_it_is_rejected() {
        let mut project = sample_project();
        project.timeline.tracks[0].items.push(TimelineItem {
            id: "legacy-peer".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 3.0,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Legacy peer".to_string(),
            properties: BTreeMap::new(),
        });
        project.timeline.tracks.push(TimelineTrack::empty(
            "track-video-2",
            "Video 2",
            TrackKind::Video,
        ));

        apply_project_action(
            &mut project,
            ProjectAction::MoveItems {
                moves: vec![
                    ProjectActionMove {
                        item_id: "item-1".to_string(),
                        target_track_id: "track-video-2".to_string(),
                        start_seconds: 1.0,
                    },
                    ProjectActionMove {
                        item_id: "legacy-peer".to_string(),
                        target_track_id: "track-video-2".to_string(),
                        start_seconds: 4.0,
                    },
                ],
            },
        )
        .expect("moving an unchanged legacy overlap should remain allowed");

        let before = project.clone();
        assert!(matches!(
            apply_project_action(
                &mut project,
                ProjectAction::MoveItems {
                    moves: vec![ProjectActionMove {
                        item_id: "legacy-peer".to_string(),
                        target_track_id: "track-video-2".to_string(),
                        start_seconds: 3.5,
                    }],
                },
            ),
            Err(ProjectActionError::TrackItemOverlap { .. })
        ));
        assert_eq!(project, before);
    }

    fn timeline_with_legacy_caption_overlap() -> Timeline {
        let mut timeline = Timeline::default_editor_timeline();
        timeline.duration_seconds = 3.0;
        timeline.tracks[3].items = vec![
            TimelineItem {
                id: "legacy-caption-a".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source: TimelineSource::Text {
                    text: "A".to_string(),
                },
                label: "A".to_string(),
                properties: BTreeMap::new(),
            },
            TimelineItem {
                id: "legacy-caption-b".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 1.5,
                duration_seconds: 1.5,
                source: TimelineSource::Text {
                    text: "B".to_string(),
                },
                label: "B".to_string(),
                properties: BTreeMap::new(),
            },
        ];
        timeline
    }

    #[test]
    fn switching_to_an_existing_legacy_overlap_timeline_is_not_a_geometry_mutation() {
        let mut project = sample_project();
        project.timelines.push(ProjectTimeline {
            id: "legacy".to_string(),
            name: "Legacy".to_string(),
            timeline: timeline_with_legacy_caption_overlap(),
        });

        apply_project_action(
            &mut project,
            ProjectAction::SetActiveTimeline {
                timeline_id: "legacy".to_string(),
            },
        )
        .expect("timeline switching must not reinterpret stored overlap as a mutation");

        assert_eq!(project.active_timeline_id.as_deref(), Some("legacy"));
    }

    #[test]
    fn creating_from_a_legacy_overlap_snapshot_can_activate_the_new_timeline() {
        let mut project = sample_project();
        project.timelines.push(ProjectTimeline {
            id: "legacy".to_string(),
            name: "Legacy".to_string(),
            timeline: timeline_with_legacy_caption_overlap(),
        });

        apply_project_action(
            &mut project,
            ProjectAction::CreateTimeline {
                timeline_id: "legacy-copy".to_string(),
                name: "Legacy copy".to_string(),
                duplicate_active: true,
                source_timeline_id: Some("legacy".to_string()),
            },
        )
        .expect("creating from stored timeline geometry must not reject its legacy overlap");

        assert_eq!(project.active_timeline_id.as_deref(), Some("legacy-copy"));
    }

    #[test]
    fn deleting_the_active_timeline_can_reveal_a_stored_legacy_overlap() {
        let mut project = sample_project();
        let active_timeline = project.timeline.clone();
        project.timelines = vec![
            ProjectTimeline {
                id: "legacy".to_string(),
                name: "Legacy".to_string(),
                timeline: timeline_with_legacy_caption_overlap(),
            },
            ProjectTimeline {
                id: "delete-me".to_string(),
                name: "Delete me".to_string(),
                timeline: active_timeline.clone(),
            },
        ];
        project.active_timeline_id = Some("delete-me".to_string());
        project.timeline = active_timeline;

        apply_project_action(
            &mut project,
            ProjectAction::DeleteTimeline {
                timeline_id: "delete-me".to_string(),
            },
        )
        .expect("deleting active timeline must allow switching to stored legacy geometry");

        assert_eq!(project.active_timeline_id.as_deref(), Some("legacy"));
    }

    #[test]
    fn set_track_locked_updates_existing_track() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::SetTrackLocked {
                track_id: "track-video".to_string(),
                locked: true,
            },
        )
        .expect("track lock action should apply");

        let track = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-video")
            .expect("sample project should include video track");
        assert!(track.locked);
    }

    #[test]
    fn set_track_sync_locked_updates_existing_track() {
        let mut project = sample_project();
        apply_project_action(
            &mut project,
            ProjectAction::SetTrackSyncLocked {
                track_id: "track-video".to_string(),
                sync_locked: true,
            },
        )
        .expect("sync lock track");
        assert!(
            project
                .timeline
                .tracks
                .iter()
                .find(|track| track.id == "track-video")
                .expect("track")
                .sync_locked
        );
    }

    #[test]
    fn set_track_locked_rejects_unknown_track() {
        let mut project = sample_project();

        let result = apply_project_action(
            &mut project,
            ProjectAction::SetTrackLocked {
                track_id: "missing-track".to_string(),
                locked: true,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackNotFound(
                "missing-track".to_string()
            ))
        );
    }

    #[test]
    fn reorder_track_preserves_visual_audio_zones() {
        let mut project = sample_project();
        apply_project_action(
            &mut project,
            ProjectAction::ReorderTrack {
                track_id: "track-captions".to_string(),
                target_track_id: "track-video".to_string(),
                placement: ProjectActionTrackPlacement::Before,
            },
        )
        .expect("visual track reorder should apply");
        assert_eq!(
            project
                .timeline
                .tracks
                .iter()
                .map(|track| track.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "track-captions",
                "track-video",
                "track-scenes",
                "track-overlays",
                "track-audio",
            ]
        );

        let before = project.clone();
        let result = apply_project_action(
            &mut project,
            ProjectAction::ReorderTrack {
                track_id: "track-video".to_string(),
                target_track_id: "track-audio".to_string(),
                placement: ProjectActionTrackPlacement::After,
            },
        );
        assert_eq!(result, Err(ProjectActionError::InvalidTrackZone));
        assert_eq!(project, before);
    }

    #[test]
    fn set_track_enabled_updates_existing_track() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::SetTrackEnabled {
                track_id: "track-video".to_string(),
                enabled: false,
            },
        )
        .expect("track enabled action should apply");

        let track = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.id == "track-video")
            .expect("sample project should include video track");
        assert!(!track.enabled);
    }

    #[test]
    fn set_track_enabled_rejects_unknown_track() {
        let mut project = sample_project();

        let result = apply_project_action(
            &mut project,
            ProjectAction::SetTrackEnabled {
                track_id: "missing-track".to_string(),
                enabled: false,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackNotFound(
                "missing-track".to_string()
            ))
        );
    }

    #[test]
    fn update_audio_fade_out_sets_positive_value() {
        let mut project = sample_project_with_audio_clip();

        apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioFadeOut {
                item_id: "audio-1".to_string(),
                fade_out_seconds: 1.25,
            },
        )
        .expect("audio fade-out action should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "audio-1")
            .expect("audio item should exist");
        assert_eq!(
            item.properties.get("fadeOutSeconds"),
            Some(&serde_json::json!(1.25))
        );
    }

    #[test]
    fn update_audio_fade_out_removes_zero_value() {
        let mut project = sample_project_with_audio_clip();
        let audio_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-audio")
            .expect("audio track should exist");
        audio_track.items[0]
            .properties
            .insert("fadeOutSeconds".to_string(), serde_json::json!(0.75));

        apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioFadeOut {
                item_id: "audio-1".to_string(),
                fade_out_seconds: 0.0,
            },
        )
        .expect("audio fade-out action should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "audio-1")
            .expect("audio item should exist");
        assert!(!item.properties.contains_key("fadeOutSeconds"));
    }

    #[test]
    fn update_audio_fade_out_rejects_non_audio_item() {
        let mut project = sample_project_with_audio_clip();

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioFadeOut {
                item_id: "item-1".to_string(),
                fade_out_seconds: 1.25,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::NotAudioClip("item-1".to_string()))
        );
    }

    #[test]
    fn update_audio_fade_out_rejects_locked_track() {
        let mut project = sample_project_with_audio_clip();
        let audio_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-audio")
            .expect("audio track should exist");
        audio_track.locked = true;

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioFadeOut {
                item_id: "audio-1".to_string(),
                fade_out_seconds: 1.25,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackLocked("track-audio".to_string()))
        );
    }

    #[test]
    fn update_audio_volume_stores_valid_value() {
        let mut project = sample_project_with_audio_clip();

        apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioVolume {
                item_id: "audio-1".to_string(),
                volume_db: Some(-6.0),
            },
        )
        .expect("audio volume action should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "audio-1")
            .expect("audio item should exist");
        assert_eq!(
            item.properties.get("volumeDb"),
            Some(&serde_json::json!(-6.0))
        );
    }

    #[test]
    fn update_audio_volume_removes_blank_value() {
        let mut project = sample_project_with_audio_clip();
        let audio_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-audio")
            .expect("audio track should exist");
        audio_track.items[0]
            .properties
            .insert("volumeDb".to_string(), serde_json::json!(-3.0));

        apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioVolume {
                item_id: "audio-1".to_string(),
                volume_db: None,
            },
        )
        .expect("audio volume reset should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "audio-1")
            .expect("audio item should exist");
        assert!(!item.properties.contains_key("volumeDb"));
    }

    #[test]
    fn update_audio_volume_rejects_non_audio_item() {
        let mut project = sample_project_with_audio_clip();

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioVolume {
                item_id: "item-1".to_string(),
                volume_db: Some(-6.0),
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::NotAudioClip("item-1".to_string()))
        );
    }

    #[test]
    fn update_audio_volume_rejects_locked_track() {
        let mut project = sample_project_with_audio_clip();
        let audio_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-audio")
            .expect("audio track should exist");
        audio_track.locked = true;

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioVolume {
                item_id: "audio-1".to_string(),
                volume_db: Some(-6.0),
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackLocked("track-audio".to_string()))
        );
    }

    #[test]
    fn update_audio_volume_rejects_out_of_range_value() {
        let mut project = sample_project_with_audio_clip();

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateAudioVolume {
                item_id: "audio-1".to_string(),
                volume_db: Some(25.0),
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::InvalidAudioVolume(
                "audio-1".to_string()
            ))
        );
    }

    #[test]
    fn update_visual_clip_opacity_stores_sub_one_value() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::UpdateVisualClipOpacity {
                item_id: "item-1".to_string(),
                opacity: 0.45,
            },
        )
        .expect("visual opacity action should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "item-1")
            .expect("video item should exist");
        assert_eq!(
            item.properties.get("opacity"),
            Some(&serde_json::json!(0.45))
        );
    }

    #[test]
    fn update_visual_clip_opacity_removes_full_opacity_value() {
        let mut project = sample_project();
        let video_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-video")
            .expect("video track should exist");
        video_track.items[0]
            .properties
            .insert("opacity".to_string(), serde_json::json!(0.5));

        apply_project_action(
            &mut project,
            ProjectAction::UpdateVisualClipOpacity {
                item_id: "item-1".to_string(),
                opacity: 1.0,
            },
        )
        .expect("visual opacity reset should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "item-1")
            .expect("video item should exist");
        assert!(!item.properties.contains_key("opacity"));
    }

    #[test]
    fn update_visual_clip_opacity_rejects_non_visual_item() {
        let mut project = sample_project_with_audio_clip();

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateVisualClipOpacity {
                item_id: "audio-1".to_string(),
                opacity: 0.45,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::NotVisualClip("audio-1".to_string()))
        );
    }

    #[test]
    fn update_visual_clip_opacity_rejects_locked_track() {
        let mut project = sample_project();
        let video_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-video")
            .expect("video track should exist");
        video_track.locked = true;

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateVisualClipOpacity {
                item_id: "item-1".to_string(),
                opacity: 0.45,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::TrackLocked("track-video".to_string()))
        );
    }

    #[test]
    fn update_visual_clip_opacity_rejects_out_of_range_value() {
        let mut project = sample_project();

        let result = apply_project_action(
            &mut project,
            ProjectAction::UpdateVisualClipOpacity {
                item_id: "item-1".to_string(),
                opacity: 1.25,
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::InvalidVisualClipOpacity(
                "item-1".to_string()
            ))
        );
    }

    #[test]
    fn set_item_keyframes_stores_opacity_keyframes() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframes: vec![
                    ProjectActionKeyframe {
                        at_seconds: 0.0,
                        value: 0.0,
                        easing: Some("linear".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 1.0,
                        value: 1.0,
                        easing: Some("easeOut".to_string()),
                    },
                ],
            },
        )
        .expect("keyframes action should apply");

        let item = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "item-1")
            .expect("video item should exist");
        assert_eq!(
            item.properties
                .get("keyframes")
                .and_then(serde_json::Value::as_object)
                .and_then(|keyframes| keyframes.get("opacity"))
                .and_then(serde_json::Value::as_array)
                .and_then(|keyframes| keyframes.first())
                .and_then(|keyframe| keyframe.get("atSeconds")),
            Some(&serde_json::json!(0.0))
        );
    }

    #[test]
    fn keyframe_point_actions_are_ordered_replace_collisions_and_normalize_smooth() {
        let mut project = sample_project();
        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframes: vec![
                    ProjectActionKeyframe {
                        at_seconds: 2.0,
                        value: 0.2,
                        easing: Some("smooth".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 0.0,
                        value: 0.0,
                        easing: Some("linear".to_string()),
                    },
                ],
            },
        )
        .expect("unordered keyframes should canonicalize");
        apply_project_action(
            &mut project,
            ProjectAction::UpsertItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframe: ProjectActionKeyframe {
                    at_seconds: 2.0,
                    value: 0.8,
                    easing: Some("hold".to_string()),
                },
            },
        )
        .expect("upsert should replace the occupied timestamp");
        apply_project_action(
            &mut project,
            ProjectAction::UpsertItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframe: ProjectActionKeyframe {
                    at_seconds: 1.0,
                    value: 0.5,
                    easing: Some("smooth".to_string()),
                },
            },
        )
        .expect("upsert should insert in canonical order");

        let item = &project.timeline.tracks[0].items[0];
        let keyframes = item.properties["keyframes"]["opacity"]
            .as_array()
            .expect("opacity keyframes");
        assert_eq!(
            keyframes,
            &vec![
                serde_json::json!({"atSeconds": 0.0, "value": 0.0, "easing": "linear"}),
                serde_json::json!({"atSeconds": 1.0, "value": 0.5, "easing": "easeInOut"}),
                serde_json::json!({"atSeconds": 2.0, "value": 0.8, "easing": "hold"}),
            ]
        );

        apply_project_action(
            &mut project,
            ProjectAction::MoveItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                from_seconds: 0.0,
                to_seconds: 1.0,
            },
        )
        .expect("move should replace the occupied destination");
        let keyframes = project.timeline.tracks[0].items[0].properties["keyframes"]["opacity"]
            .as_array()
            .expect("moved opacity keyframes");
        assert_eq!(
            keyframes,
            &vec![
                serde_json::json!({"atSeconds": 1.0, "value": 0.0, "easing": "linear"}),
                serde_json::json!({"atSeconds": 2.0, "value": 0.8, "easing": "hold"}),
            ]
        );
    }

    #[test]
    fn delete_item_keyframe_removes_empty_property_and_failed_mutation_is_atomic() {
        let mut project = sample_project();
        apply_project_action(
            &mut project,
            ProjectAction::UpsertItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframe: ProjectActionKeyframe {
                    at_seconds: 1.0,
                    value: 0.5,
                    easing: None,
                },
            },
        )
        .expect("upsert keyframe");
        let before_failed_move = project.clone();
        let error = apply_project_action(
            &mut project,
            ProjectAction::UpsertItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframe: ProjectActionKeyframe {
                    at_seconds: 2.0,
                    value: 0.75,
                    easing: Some("bounce".to_string()),
                },
            },
        )
        .expect_err("unknown easing should fail");
        assert!(matches!(error, ProjectActionError::InvalidEffectParam(_)));
        assert_eq!(project, before_failed_move);
        let error = apply_project_action(
            &mut project,
            ProjectAction::MoveItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                from_seconds: 9.0,
                to_seconds: 2.0,
            },
        )
        .expect_err("missing source keyframe should fail");
        assert!(matches!(error, ProjectActionError::KeyframeNotFound { .. }));
        assert_eq!(project, before_failed_move);

        apply_project_action(
            &mut project,
            ProjectAction::DeleteItemKeyframe {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                at_seconds: 1.0,
            },
        )
        .expect("delete existing keyframe");
        assert!(!project.timeline.tracks[0].items[0]
            .properties
            .contains_key("keyframes"));
    }

    #[test]
    fn ripple_trim_propagates_linked_targets_and_sync_locked_followers() {
        let mut project = sample_ripple_trim_project();
        apply_project_action(
            &mut project,
            ProjectAction::RippleTrimItem {
                item_id: "item-1".to_string(),
                edge: ProjectActionTrimEdge::Right,
                delta_seconds: 1.0,
                propagate_linked: true,
                sync_locked_track_ids: vec!["track-captions".to_string()],
            },
        )
        .expect("ripple trim should apply atomically");

        let item = |id: &str| {
            project
                .timeline
                .tracks
                .iter()
                .flat_map(|track| track.items.iter())
                .find(|item| item.id == id)
                .expect("fixture item")
        };
        assert_eq!(item("item-1").duration_seconds, 5.0);
        assert_eq!(
            item("item-1").properties["sourceOut"],
            serde_json::json!(7.0)
        );
        assert_eq!(item("audio-1").duration_seconds, 5.0);
        assert_eq!(
            item("audio-1").properties["sourceOut"],
            serde_json::json!(6.0)
        );
        assert_eq!(item("video-after").start_seconds, 5.0);
        assert_eq!(item("audio-after").start_seconds, 5.0);
        assert_eq!(item("caption-after").start_seconds, 5.0);
        assert_eq!(project.timeline.duration_seconds, 7.0);
    }

    #[test]
    fn ripple_trim_rejects_sync_collision_and_invalid_source_atomically() {
        let mut project = sample_ripple_trim_project();
        let caption_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-captions")
            .expect("caption track");
        caption_track.items.insert(
            0,
            TimelineItem {
                id: "caption-obstacle".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 0.0,
                duration_seconds: 4.0,
                source: TimelineSource::Text {
                    text: "Obstacle".to_string(),
                },
                label: "Obstacle".to_string(),
                properties: BTreeMap::new(),
            },
        );
        project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-captions")
            .expect("caption track")
            .items
            .iter_mut()
            .find(|item| item.id == "caption-after")
            .expect("caption follower")
            .start_seconds = 5.0;
        let before = project.clone();
        let error = apply_project_action(
            &mut project,
            ProjectAction::RippleTrimItem {
                item_id: "item-1".to_string(),
                edge: ProjectActionTrimEdge::Right,
                delta_seconds: -2.0,
                propagate_linked: true,
                sync_locked_track_ids: vec!["track-captions".to_string()],
            },
        )
        .expect_err("sync-locked follower collision should reject");
        assert!(matches!(
            error,
            ProjectActionError::RippleTrimCollision { .. }
        ));
        assert_eq!(project, before);

        let error = apply_project_action(
            &mut project,
            ProjectAction::RippleTrimItem {
                item_id: "item-1".to_string(),
                edge: ProjectActionTrimEdge::Right,
                delta_seconds: 20.0,
                propagate_linked: true,
                sync_locked_track_ids: Vec::new(),
            },
        )
        .expect_err("source headroom must reject extension");
        assert!(matches!(
            error,
            ProjectActionError::SourceRangeOutsideMedia(_)
        ));
        assert_eq!(project, before);
    }

    fn automated_retimed_item() -> TimelineItem {
        TimelineItem {
            id: "retimed".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Retimed automated clip".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(10.0)),
                ("sourceOut".to_string(), serde_json::json!(18.0)),
                ("speed".to_string(), serde_json::json!(2.0)),
                ("fadeInSeconds".to_string(), serde_json::json!(0.5)),
                ("fadeOutSeconds".to_string(), serde_json::json!(0.75)),
                (
                    "keyframes".to_string(),
                    serde_json::json!({
                        "opacity": [
                            { "atSeconds": 0.0, "value": 0.0, "easing": "easeInOut" },
                            { "atSeconds": 2.0, "value": 1.0, "easing": "linear" },
                            { "atSeconds": 4.0, "value": 0.0, "easing": "hold" }
                        ]
                    }),
                ),
                (
                    "effectParameterKeyframes".to_string(),
                    serde_json::json!({
                        "contrast-1": {
                            "amount": [
                                { "atSeconds": 0.0, "value": 0.5, "easing": "linear" },
                                { "atSeconds": 4.0, "value": 1.5, "easing": "linear" }
                            ]
                        }
                    }),
                ),
            ]),
        }
    }

    fn project_with_automated_retimed_item() -> VideoProject {
        let mut project = sample_project();
        project.media[0].duration_seconds = 30.0;
        project.timeline.tracks[0].items = vec![automated_retimed_item()];
        project.timeline.duration_seconds = 4.0;
        project
    }

    #[test]
    fn split_retimed_item_partitions_source_fades_and_all_automation_lanes() {
        let mut project = project_with_automated_retimed_item();
        apply_project_action(
            &mut project,
            ProjectAction::SplitItems {
                splits: vec![ProjectActionSplit {
                    item_id: "retimed".to_string(),
                    new_item_id: "retimed-right".to_string(),
                    split_seconds: 1.5,
                }],
            },
        )
        .expect("retimed split");

        let left = &project.timeline.tracks[0].items[0];
        let right = &project.timeline.tracks[0].items[1];
        assert_eq!(left.properties["sourceIn"], serde_json::json!(10.0));
        assert_eq!(left.properties["sourceOut"], serde_json::json!(13.0));
        assert_eq!(right.properties["sourceIn"], serde_json::json!(13.0));
        assert_eq!(right.properties["sourceOut"], serde_json::json!(18.0));
        assert!(left.properties.contains_key("fadeInSeconds"));
        assert!(!left.properties.contains_key("fadeOutSeconds"));
        assert!(!right.properties.contains_key("fadeInSeconds"));
        assert!(right.properties.contains_key("fadeOutSeconds"));

        let left_opacity = left.properties["keyframes"]["opacity"]
            .as_array()
            .expect("left opacity");
        let right_opacity = right.properties["keyframes"]["opacity"]
            .as_array()
            .expect("right opacity");
        assert_eq!(
            left_opacity.last().unwrap()["atSeconds"],
            serde_json::json!(1.5)
        );
        assert!((left_opacity.last().unwrap()["value"].as_f64().unwrap() - 0.84375).abs() < 1e-9);
        assert_eq!(right_opacity[0]["atSeconds"], serde_json::json!(0.0));
        assert!((right_opacity[0]["value"].as_f64().unwrap() - 0.84375).abs() < 1e-9);
        assert_eq!(right_opacity[1]["atSeconds"], serde_json::json!(0.5));
        assert_eq!(
            right_opacity.last().unwrap()["atSeconds"],
            serde_json::json!(2.5)
        );

        let left_effect = left.properties["effectParameterKeyframes"]["contrast-1"]["amount"]
            .as_array()
            .expect("left effect lane");
        let right_effect = right.properties["effectParameterKeyframes"]["contrast-1"]["amount"]
            .as_array()
            .expect("right effect lane");
        assert_eq!(
            left_effect.last().unwrap()["value"],
            serde_json::json!(0.875)
        );
        assert_eq!(right_effect[0]["value"], serde_json::json!(0.875));
    }

    #[test]
    fn overwrite_and_ripple_delete_partition_retimed_source_and_automation() {
        let incoming = TimelineItem {
            id: "incoming".to_string(),
            start_seconds: 1.0,
            duration_seconds: 1.0,
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(0.0)),
                ("sourceOut".to_string(), serde_json::json!(1.0)),
            ]),
            ..automated_retimed_item()
        };
        let mut overwritten = project_with_automated_retimed_item();
        apply_project_action(
            &mut overwritten,
            ProjectAction::AddItems {
                target_track_id: "track-video".to_string(),
                items: vec![incoming],
            },
        )
        .expect("overwrite retimed clip");
        let left = &overwritten.timeline.tracks[0].items[0];
        let right = &overwritten.timeline.tracks[0].items[2];
        assert_eq!(left.properties["sourceOut"], serde_json::json!(12.0));
        assert_eq!(right.properties["sourceIn"], serde_json::json!(14.0));
        assert_eq!(right.properties["sourceOut"], serde_json::json!(18.0));
        assert!(!left.properties.contains_key("fadeOutSeconds"));
        assert!(!right.properties.contains_key("fadeInSeconds"));
        assert_eq!(
            right.properties["keyframes"]["opacity"][0]["atSeconds"],
            serde_json::json!(0.0)
        );

        let mut rippled = project_with_automated_retimed_item();
        apply_project_action(
            &mut rippled,
            ProjectAction::RippleDeleteRanges {
                ranges: vec![ProjectActionRippleDeleteRange {
                    start_seconds: 1.0,
                    end_seconds: 2.0,
                    track_ids: vec!["track-video".to_string()],
                }],
            },
        )
        .expect("ripple delete retimed clip");
        let left = &rippled.timeline.tracks[0].items[0];
        let right = &rippled.timeline.tracks[0].items[1];
        assert_eq!(left.properties["sourceOut"], serde_json::json!(12.0));
        assert_eq!(right.start_seconds, 1.0);
        assert_eq!(right.properties["sourceIn"], serde_json::json!(14.0));
        assert_eq!(right.properties["sourceOut"], serde_json::json!(18.0));
        assert!(!left.properties.contains_key("fadeOutSeconds"));
        assert!(!right.properties.contains_key("fadeInSeconds"));
    }

    #[test]
    fn ripple_trim_rebases_automation_at_the_changed_edge() {
        let mut project = project_with_automated_retimed_item();
        apply_project_action(
            &mut project,
            ProjectAction::RippleTrimItem {
                item_id: "retimed".to_string(),
                edge: ProjectActionTrimEdge::Left,
                delta_seconds: 1.0,
                propagate_linked: false,
                sync_locked_track_ids: Vec::new(),
            },
        )
        .expect("left ripple trim");
        let item = &project.timeline.tracks[0].items[0];
        assert_eq!(item.duration_seconds, 3.0);
        assert_eq!(item.properties["sourceIn"], serde_json::json!(12.0));
        assert!(!item.properties.contains_key("fadeInSeconds"));
        assert!(item.properties.contains_key("fadeOutSeconds"));
        assert_eq!(
            item.properties["keyframes"]["opacity"][0]["atSeconds"],
            serde_json::json!(0.0)
        );
        assert_eq!(
            item.properties["keyframes"]["opacity"][1]["atSeconds"],
            serde_json::json!(1.0)
        );
    }

    #[test]
    fn ripple_insert_splits_crossing_retimed_item_without_losing_automation() {
        let mut project = project_with_automated_retimed_item();
        let incoming = TimelineItem {
            id: "inserted".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 0.0,
            duration_seconds: 1.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Inserted".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(0.0)),
                ("sourceOut".to_string(), serde_json::json!(1.0)),
            ]),
        };
        apply_project_action(
            &mut project,
            ProjectAction::InsertItems {
                target_track_id: "track-video".to_string(),
                insert_seconds: 1.5,
                items: vec![incoming],
            },
        )
        .expect("ripple insert");

        let items = &project.timeline.tracks[0].items;
        assert_eq!(items[0].id, "retimed");
        assert_eq!(items[0].properties["sourceOut"], serde_json::json!(13.0));
        assert_eq!(items[1].id, "inserted");
        assert_eq!(items[2].start_seconds, 2.5);
        assert_eq!(items[2].properties["sourceIn"], serde_json::json!(13.0));
        assert_eq!(items[2].properties["sourceOut"], serde_json::json!(18.0));
        assert!(!items[0].properties.contains_key("fadeOutSeconds"));
        assert!(!items[2].properties.contains_key("fadeInSeconds"));
        assert_eq!(
            items[2].properties["keyframes"]["opacity"][0]["atSeconds"],
            serde_json::json!(0.0)
        );
    }

    #[test]
    fn set_item_keyframes_stores_visual_transform_keyframes() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::PositionX,
                keyframes: vec![
                    ProjectActionKeyframe {
                        at_seconds: 0.0,
                        value: -120.0,
                        easing: Some("linear".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 1.0,
                        value: 0.0,
                        easing: Some("easeOut".to_string()),
                    },
                ],
            },
        )
        .expect("position keyframes action should apply");
        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Scale,
                keyframes: vec![
                    ProjectActionKeyframe {
                        at_seconds: 0.0,
                        value: 0.85,
                        easing: Some("easeOut".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 1.0,
                        value: 1.0,
                        easing: Some("linear".to_string()),
                    },
                ],
            },
        )
        .expect("scale keyframes action should apply");
        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::ScaleY,
                keyframes: vec![ProjectActionKeyframe {
                    at_seconds: 0.0,
                    value: 0.5,
                    easing: Some("linear".to_string()),
                }],
            },
        )
        .expect("scaleY keyframes action should apply");
        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::CropLeft,
                keyframes: vec![ProjectActionKeyframe {
                    at_seconds: 0.0,
                    value: 0.25,
                    easing: Some("hold".to_string()),
                }],
            },
        )
        .expect("crop keyframes action should apply");
        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::RotationDegrees,
                keyframes: vec![
                    ProjectActionKeyframe {
                        at_seconds: 0.0,
                        value: -6.0,
                        easing: Some("easeOut".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 1.0,
                        value: 0.0,
                        easing: Some("linear".to_string()),
                    },
                ],
            },
        )
        .expect("rotation keyframes action should apply");

        let keyframes = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "item-1")
            .expect("video item should exist")
            .properties
            .get("keyframes")
            .and_then(serde_json::Value::as_object)
            .expect("keyframes should exist");

        assert_eq!(
            keyframes["positionX"][0]["value"],
            serde_json::json!(-120.0)
        );
        assert_eq!(keyframes["scale"][0]["value"], serde_json::json!(0.85));
        assert_eq!(keyframes["scaleY"][0]["value"], serde_json::json!(0.5));
        assert_eq!(keyframes["cropLeft"][0]["value"], serde_json::json!(0.25));
        assert_eq!(
            keyframes["rotationDegrees"][0]["value"],
            serde_json::json!(-6.0)
        );
    }

    #[test]
    fn set_item_keyframes_rejects_visual_transform_keyframes_on_audio_clip() {
        let mut project = sample_project_with_audio_clip();

        let result = apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "audio-1".to_string(),
                property: ProjectActionKeyframeProperty::PositionY,
                keyframes: vec![ProjectActionKeyframe {
                    at_seconds: 0.0,
                    value: 24.0,
                    easing: Some("linear".to_string()),
                }],
            },
        );

        assert_eq!(
            result,
            Err(ProjectActionError::NotVisualClip("audio-1".to_string()))
        );
    }

    #[test]
    fn effect_parameter_keyframes_are_collision_safe_and_mutable() {
        let mut project = sample_project();
        apply_project_action(
            &mut project,
            ProjectAction::UpdateItemEffects {
                item_ids: vec!["item-1".to_string()],
                effects: vec![
                    ProjectActionEffect {
                        effect_instance_id: "grain-a".to_string(),
                        effect_type: "stylize.grain".to_string(),
                        enabled: true,
                        params: BTreeMap::from([("amount".to_string(), serde_json::json!(0.2))]),
                    },
                    ProjectActionEffect {
                        effect_instance_id: "grain-b".to_string(),
                        effect_type: "stylize.grain".to_string(),
                        enabled: true,
                        params: BTreeMap::from([("amount".to_string(), serde_json::json!(0.4))]),
                    },
                ],
            },
        )
        .expect("effects should apply");

        for (instance, value) in [("grain-a", 0.3), ("grain-b", 0.7)] {
            apply_project_action(
                &mut project,
                ProjectAction::UpsertEffectParameterKeyframe {
                    item_id: "item-1".to_string(),
                    effect_instance_id: instance.to_string(),
                    parameter_key: "amount".to_string(),
                    keyframe: ProjectActionKeyframe {
                        at_seconds: 1.0,
                        value,
                        easing: Some("smooth".to_string()),
                    },
                },
            )
            .expect("effect keyframe should apply");
        }
        apply_project_action(
            &mut project,
            ProjectAction::MoveEffectParameterKeyframe {
                item_id: "item-1".to_string(),
                effect_instance_id: "grain-a".to_string(),
                parameter_key: "amount".to_string(),
                from_seconds: 1.0,
                to_seconds: 2.0,
            },
        )
        .expect("effect keyframe should move");
        apply_project_action(
            &mut project,
            ProjectAction::DeleteEffectParameterKeyframe {
                item_id: "item-1".to_string(),
                effect_instance_id: "grain-b".to_string(),
                parameter_key: "amount".to_string(),
                at_seconds: 1.0,
            },
        )
        .expect("effect keyframe should delete");

        let item = &project.timeline.tracks[0].items[0];
        let lanes = &item.properties["effectParameterKeyframes"];
        assert_eq!(
            lanes["grain-a"]["amount"][0]["atSeconds"],
            serde_json::json!(2.0)
        );
        assert_eq!(
            lanes["grain-a"]["amount"][0]["easing"],
            serde_json::json!("easeInOut")
        );
        assert!(lanes.get("grain-b").is_none());
    }

    #[test]
    fn legacy_effect_ids_are_deterministic_and_invalid_parameter_is_atomic() {
        let mut project = sample_project();
        let before = project.clone();
        apply_project_action(
            &mut project,
            ProjectAction::UpdateItemEffects {
                item_ids: vec!["item-1".to_string()],
                effects: vec![ProjectActionEffect {
                    effect_instance_id: String::new(),
                    effect_type: "stylize.grain".to_string(),
                    enabled: true,
                    params: BTreeMap::from([("amount".to_string(), serde_json::json!(0.2))]),
                }],
            },
        )
        .expect("legacy effect should canonicalize");
        let item = &project.timeline.tracks[0].items[0];
        assert_eq!(
            item.properties["effects"][0]["effectInstanceId"],
            serde_json::json!("legacy:stylize.grain:1")
        );

        let canonical = project.clone();
        let result = apply_project_action(
            &mut project,
            ProjectAction::UpsertEffectParameterKeyframe {
                item_id: "item-1".to_string(),
                effect_instance_id: "legacy:stylize.grain:1".to_string(),
                parameter_key: "missing".to_string(),
                keyframe: ProjectActionKeyframe {
                    at_seconds: 1.0,
                    value: 0.5,
                    easing: None,
                },
            },
        );
        assert!(matches!(
            result,
            Err(ProjectActionError::InvalidEffectParam(_))
        ));
        assert_eq!(project, canonical);
        assert_ne!(project, before);
    }

    #[test]
    fn resize_items_rescales_item_keyframes() {
        let mut project = sample_project();

        apply_project_action(
            &mut project,
            ProjectAction::SetItemKeyframes {
                item_id: "item-1".to_string(),
                property: ProjectActionKeyframeProperty::Opacity,
                keyframes: vec![
                    ProjectActionKeyframe {
                        at_seconds: 0.0,
                        value: 0.0,
                        easing: Some("linear".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 1.0,
                        value: 0.5,
                        easing: Some("easeInOut".to_string()),
                    },
                    ProjectActionKeyframe {
                        at_seconds: 4.0,
                        value: 1.0,
                        easing: Some("easeOut".to_string()),
                    },
                ],
            },
        )
        .expect("keyframes action should apply");

        apply_project_action(
            &mut project,
            ProjectAction::ResizeItems {
                resizes: vec![ProjectActionResize {
                    item_id: "item-1".to_string(),
                    duration_seconds: 2.0,
                }],
            },
        )
        .expect("resize action should apply");

        let opacity_keyframes = project
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.id == "item-1")
            .expect("video item should exist")
            .properties
            .get("keyframes")
            .and_then(serde_json::Value::as_object)
            .and_then(|keyframes| keyframes.get("opacity"))
            .and_then(serde_json::Value::as_array)
            .expect("opacity keyframes should exist");

        assert_eq!(
            opacity_keyframes
                .iter()
                .map(|keyframe| keyframe
                    .get("atSeconds")
                    .and_then(serde_json::Value::as_f64)
                    .expect("keyframe atSeconds should be numeric"))
                .collect::<Vec<_>>(),
            vec![0.0, 0.5, 2.0]
        );
        assert_eq!(
            opacity_keyframes[1].get("easing"),
            Some(&serde_json::json!("easeInOut"))
        );
    }

    fn sample_project_with_audio_clip() -> VideoProject {
        let mut project = sample_project();
        project.media.push(MediaAsset {
            id: "media-audio-1".to_string(),
            name: None,
            relative_path: "media/music.wav".to_string(),
            kind: MediaKind::Audio,
            duration_seconds: 12.0,
            width: None,
            height: None,
            fps: None,
            folder_id: None,
        });
        let audio_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-audio")
            .expect("audio track should exist");
        audio_track.items.push(TimelineItem {
            id: "audio-1".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "media-audio-1".to_string(),
            },
            label: "Music bed".to_string(),
            properties: BTreeMap::new(),
        });
        project
    }

    fn sample_ripple_trim_project() -> VideoProject {
        let mut project = sample_project_with_audio_clip();
        let video_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-video")
            .expect("video track");
        video_track.items[0].properties.extend([
            ("sourceIn".to_string(), serde_json::json!(2.0)),
            ("sourceOut".to_string(), serde_json::json!(6.0)),
            ("linkGroupId".to_string(), serde_json::json!("linked-av-1")),
        ]);
        video_track.items.push(TimelineItem {
            id: "video-after".to_string(),
            kind: TimelineItemKind::VideoClip,
            start_seconds: 4.0,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Video follower".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(6.0)),
                ("sourceOut".to_string(), serde_json::json!(8.0)),
            ]),
        });
        let audio_track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-audio")
            .expect("audio track");
        audio_track.items[0].properties.extend([
            ("sourceIn".to_string(), serde_json::json!(1.0)),
            ("sourceOut".to_string(), serde_json::json!(5.0)),
            ("linkGroupId".to_string(), serde_json::json!("linked-av-1")),
        ]);
        audio_track.items.push(TimelineItem {
            id: "audio-after".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 4.0,
            duration_seconds: 2.0,
            source: TimelineSource::Media {
                media_id: "media-audio-1".to_string(),
            },
            label: "Audio follower".to_string(),
            properties: BTreeMap::from([
                ("sourceIn".to_string(), serde_json::json!(5.0)),
                ("sourceOut".to_string(), serde_json::json!(7.0)),
            ]),
        });
        project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.id == "track-captions")
            .expect("caption track")
            .items
            .push(TimelineItem {
                id: "caption-after".to_string(),
                kind: TimelineItemKind::Caption,
                start_seconds: 4.0,
                duration_seconds: 1.0,
                source: TimelineSource::Text {
                    text: "Follower".to_string(),
                },
                label: "Caption follower".to_string(),
                properties: BTreeMap::new(),
            });
        project.timeline.duration_seconds = 6.0;
        project
    }
}
