use super::action::{apply_project_action, ProjectAction};
use super::model::{
    CaptionRenderMode, GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences,
    GeneratedAssetSettings, GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary,
    MediaAnalysisMoment, MediaAsset, MediaFolder, MediaKind, MediaSilenceRange,
    ProjectExportArtifact, ProjectExportArtifactKind, ProjectRenderReport, ProjectTemplateOverride,
    ProjectTimeline, RenderReportCheckStatus, RenderSettings, TemporalWorkflowMetadata,
    TemporalWorkflowStartRequest, Timeline, TimelineItem, TimelineItemKind, TimelineSource,
    TimelineTrack, TrackKind, Transcript, TranscriptRepair, VideoProject,
};
use super::mutation::{acquire_split_project_mutation_lease, SplitProjectMutationLease};
use crate::search::{project_search_index_path, rebuild_project_search_index};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

mod agent_batch;
mod agent_history_snapshot;
mod agent_undo_background;
mod agent_undo_content;

pub use agent_batch::{
    apply_agent_project_action_batch, legacy_agent_undo_refusal, undo_latest_agent_project_action,
    undo_latest_agent_project_batch, undo_latest_agent_project_batch_with_runs,
    ProjectAgentApplyResult, ProjectAgentUndoOutcome, UndoneGenerationRun,
};
pub use agent_history_snapshot::{AgentProjectSnapshot, MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES};
pub use agent_undo_content::{
    agent_project_content_hash, agent_undo_comparable_content, SplitAgentBookkeepingIds,
};

pub const SPLIT_PROJECT_SCHEMA_VERSION: u32 = 2;
pub const SPLIT_TIMELINE_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_MEDIA_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_TRANSCRIPT_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_TRANSCRIPT_INDEX_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_TEMPLATE_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_TEMPLATE_INDEX_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_GENERATED_ASSET_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_GENERATED_ASSET_INDEX_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_RENDER_REPORT_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_RENDER_REPORT_INDEX_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_WORKFLOW_JOB_INDEX_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_EXPORT_ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_EXPORT_ARTIFACT_INDEX_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_PROJECT_CONTEXT_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_AGENT_EDIT_HISTORY_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_APP_SERVER_CONVERSATION_SCHEMA_VERSION: u32 = 1;
pub const SPLIT_AGENT_SESSION_SCHEMA_VERSION: u32 = 1;
const SPLIT_TEMPLATE_INDEX_FILE_NAME: &str = "index.json";
const SPLIT_GENERATED_ASSET_INDEX_FILE_NAME: &str = "index.json";
const SPLIT_RENDER_REPORT_INDEX_FILE_NAME: &str = "index.json";
const SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME: &str = "jobs";
const SPLIT_WORKFLOW_JOB_INDEX_FILE_NAME: &str = "index.json";
const SPLIT_WORKFLOW_JOB_FILE_NAME: &str = "job.json";
const SPLIT_EXPORT_ARTIFACT_INDEX_DIR_NAME: &str = "exports";
const SPLIT_EXPORT_ARTIFACT_INDEX_FILE_NAME: &str = "index.json";
const SPLIT_EXPORT_ARTIFACT_FILE_NAME: &str = "artifact.json";
const SPLIT_PROJECT_CONTEXT_DIR_NAME: &str = "context";
const SPLIT_PROJECT_CONTEXT_FILE_NAME: &str = "project.json";
const SPLIT_AGENT_EDIT_HISTORY_FILE_NAME: &str = "agent-history.json";
const SPLIT_APP_SERVER_CONVERSATION_FILE_NAME: &str = "app-server-conversations.json";
const SPLIT_AGENT_SESSION_FILE_NAME: &str = "agent-sessions.json";
const SPLIT_TRANSCRIPT_INDEX_FILE_NAME: &str = "index.json";
const MAX_APP_SERVER_CONVERSATION_ENTRIES: usize = 100;
const SPLIT_PROJECT_TRANSACTION_SCHEMA_VERSION: u32 = 1;
const SPLIT_PROJECT_TRANSACTION_STAGE_PREFIX: &str = ".video-creater-settings-stage.";
const SPLIT_PROJECT_TRANSACTION_BACKUP_PREFIX: &str = ".video-creater-settings-backup.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SplitProjectLayout {
    Split,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub content_revision: u64,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub layout: SplitProjectLayout,
    pub files: SplitProjectFiles,
    pub render_settings: RenderSettings,
    pub codex_thread_id: Option<String>,
    #[serde(default)]
    pub jobs: Vec<JobSummary>,
    #[serde(default)]
    pub export_artifacts: Vec<ProjectExportArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectFiles {
    pub timeline: String,
    pub media: String,
    pub transcripts: String,
    pub templates: String,
    pub generated: String,
    pub renders: String,
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTimelineFile {
    pub schema_version: u32,
    pub duration_seconds: f64,
    pub tracks: Vec<super::model::TimelineTrack>,
    #[serde(default)]
    pub timelines: Vec<ProjectTimeline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_timeline_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitMediaIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub folders: Vec<super::model::MediaFolder>,
    #[serde(default)]
    pub assets: Vec<super::model::MediaAsset>,
    #[serde(default)]
    pub analysis: Vec<MediaAnalysisMoment>,
    #[serde(default)]
    pub silence_ranges: Vec<MediaSilenceRange>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierProjectFile {
    #[serde(default)]
    timelines: Vec<PalmierTimeline>,
    active_timeline_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierTimeline {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    fps: u32,
    width: u32,
    height: u32,
    #[serde(default)]
    tracks: Vec<PalmierTrack>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierTrack {
    #[serde(default)]
    id: String,
    #[serde(rename = "type")]
    kind: PalmierClipType,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    clips: Vec<PalmierClip>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum PalmierClipType {
    Video,
    Audio,
    Image,
    Text,
    Lottie,
    Sequence,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierClip {
    #[serde(default)]
    id: String,
    #[serde(default)]
    media_ref: String,
    #[serde(default)]
    media_type: Option<PalmierClipType>,
    #[serde(default)]
    source_clip_type: Option<PalmierClipType>,
    start_frame: i64,
    duration_frames: i64,
    #[serde(default)]
    trim_start_frame: i64,
    #[serde(default)]
    trim_end_frame: i64,
    #[serde(default = "default_palmier_clip_speed")]
    speed: f64,
    #[serde(default)]
    link_group_id: Option<String>,
    #[serde(default)]
    text_content: Option<String>,
}

fn default_palmier_clip_speed() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierMediaManifest {
    #[serde(default)]
    entries: Vec<PalmierMediaManifestEntry>,
    #[serde(default)]
    folders: Vec<PalmierMediaFolder>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierMediaFolder {
    id: String,
    name: String,
    #[serde(default, alias = "parentId")]
    parent_folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierMediaManifestEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    kind: PalmierClipType,
    source: PalmierMediaSource,
    #[serde(default)]
    duration: f64,
    generation_input: Option<PalmierGenerationInput>,
    #[serde(default)]
    source_width: Option<u32>,
    #[serde(default)]
    source_height: Option<u32>,
    #[serde(default)]
    source_fps: Option<f64>,
    #[serde(default)]
    folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
enum PalmierMediaSource {
    Project { relative_path: String },
    External { absolute_path: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PalmierGenerationInput {
    prompt: String,
    model: String,
    duration: u64,
    aspect_ratio: String,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    num_images: Option<u32>,
    #[serde(default)]
    voice: Option<String>,
    #[serde(default)]
    lyrics: Option<String>,
    #[serde(default)]
    style_instructions: Option<String>,
    #[serde(default)]
    instrumental: Option<bool>,
    #[serde(default)]
    generate_audio: Option<bool>,
    #[serde(default, rename = "imageURLs")]
    image_urls: Vec<String>,
    #[serde(default, rename = "referenceImageURLs")]
    reference_image_urls: Vec<String>,
    #[serde(default, rename = "referenceVideoURLs")]
    reference_video_urls: Vec<String>,
    #[serde(default, rename = "referenceAudioURLs")]
    reference_audio_urls: Vec<String>,
    #[serde(default)]
    image_url_asset_ids: Vec<String>,
    #[serde(default)]
    reference_image_asset_ids: Vec<String>,
    #[serde(default)]
    reference_video_asset_ids: Vec<String>,
    #[serde(default)]
    reference_audio_asset_ids: Vec<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    output_index: Option<usize>,
    #[serde(default, rename = "resultURLs")]
    result_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTranscriptFile {
    pub schema_version: u32,
    pub id: String,
    pub media_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_artifact_path: Option<String>,
    #[serde(default)]
    pub repairs: Vec<TranscriptRepair>,
    #[serde(default)]
    pub segments: Vec<super::model::TranscriptSegment>,
    #[serde(default)]
    pub words: Vec<super::model::TranscriptWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTranscriptIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub transcripts: Vec<SplitTranscriptIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTranscriptIndexEntry {
    pub transcript_id: String,
    pub media_id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_artifact_path: Option<String>,
    pub repair_count: usize,
    pub segment_count: usize,
    pub word_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTemplateIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub templates: Vec<SplitTemplateIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitTemplateIndexEntry {
    pub template_id: String,
    pub name: String,
    pub path: String,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitGeneratedAssetIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub assets: Vec<SplitGeneratedAssetIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitGeneratedAssetIndexEntry {
    pub asset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub path: String,
    pub status: String,
    pub model_provider: String,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_intent: Option<String>,
    pub output_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitRenderReportIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub reports: Vec<SplitRenderReportIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitRenderReportIndexEntry {
    pub report_id: String,
    pub path: String,
    pub status: String,
    pub output_path: String,
    pub duration_seconds: f64,
    pub video: bool,
    pub audio: bool,
    pub check_count: usize,
    pub failed_check_count: usize,
    pub artifact_count: usize,
    pub log_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitWorkflowJobIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub jobs: Vec<SplitWorkflowJobIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitWorkflowJobIndexEntry {
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_queue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub activity_count: usize,
    pub has_start_request: bool,
    pub start_request_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_reuse_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitExportArtifactIndexFile {
    pub schema_version: u32,
    #[serde(default)]
    pub artifacts: Vec<SplitExportArtifactIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitExportArtifactIndexEntry {
    pub artifact_id: String,
    pub kind: String,
    pub format: String,
    pub path: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectContextFile {
    pub schema_version: u32,
    pub project_id: String,
    pub name: String,
    pub updated_at: String,
    pub files: SplitProjectContextFiles,
    pub indexes: SplitProjectContextIndexes,
    pub counts: SplitProjectContextCounts,
    pub latest: SplitProjectContextLatest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectContextFiles {
    pub manifest: String,
    pub timeline: String,
    pub media: String,
    pub transcripts: String,
    pub templates: String,
    pub generated: String,
    pub renders: String,
    pub jobs: String,
    pub exports: String,
    pub logs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectContextIndexes {
    pub transcripts: String,
    pub templates: String,
    pub generated: String,
    pub renders: String,
    pub jobs: String,
    pub exports: String,
    #[serde(default = "default_search_index_path_string")]
    pub search: String,
}

fn default_search_index_path_string() -> String {
    "search/index.json".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectContextCounts {
    pub media: usize,
    pub media_folders: usize,
    pub timeline_tracks: usize,
    pub timeline_items: usize,
    pub transcripts: usize,
    pub template_overrides: usize,
    pub generated_assets: usize,
    pub render_reports: usize,
    pub workflow_jobs: usize,
    pub export_artifacts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectContextLatest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_job_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteReport {
    pub manifest_path: String,
    pub written_files: Vec<String>,
    pub removed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub recovery_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActionWriteResult {
    pub project: VideoProject,
    pub report: ProjectWriteReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAgentEditHistoryFile {
    pub schema_version: u32,
    #[serde(default)]
    pub entries: Vec<SplitAgentEditHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAgentEditHistoryEntry {
    pub id: String,
    pub action_count: usize,
    /// Pre-apply snapshot that Undo restores, stored compressed.
    pub before: AgentProjectSnapshot,
    /// Legacy full post-apply snapshot. Batch entries store `after_content_hash` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<AgentProjectSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_content_revision: Option<u64>,
    /// Content hash of the committed post-apply project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_content_hash: Option<String>,
    /// Rule `after_content_hash` was computed with. `Some(4)` is the version 3
    /// rule that also ignores the progress of `background_generated_asset_ids`;
    /// `Some(3)` is `agent_project_content_hash` scoped to
    /// `added_generated_asset_ids`, which also ignores the background progress
    /// of those generations; `Some(2)` is the same rule without generation
    /// scope, which ignores only job bookkeeping; `None` is the original rule
    /// that also hashed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_content_hash_version: Option<u32>,
    /// Job bookkeeping records the batch itself added; Undo removes exactly these.
    #[serde(default, skip_serializing_if = "SplitAgentBookkeepingIds::is_empty")]
    pub added_bookkeeping_ids: SplitAgentBookkeepingIds,
    /// Generated assets the batch itself recorded. Version 3 hashes ignore
    /// their background progress (status, outputs, placement over their
    /// placeholder), and Undo removes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_generated_asset_ids: Vec<String>,
    /// Generated assets recorded before the batch whose progress the batch left alone.
    /// Version 4 hashes ignore their progress (status, outputs, provider input URLs,
    /// created-at, unplaced completion media), and Undo keeps their current progress.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub background_generated_asset_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAppServerConversationFile {
    pub schema_version: u32,
    #[serde(default)]
    pub entries: Vec<SplitAppServerConversationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAppServerConversationEntry {
    pub id: String,
    pub project_id: String,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_status: Option<String>,
    pub prompt: String,
    /// The request exactly as submitted. Legacy entries hold an `EditJobRequest`;
    /// conversation entries hold a `CodexConversationEditRequest`.
    pub request: Value,
    pub has_proposal: bool,
    pub thread_response: Value,
    pub turn_response: Value,
    /// The chat session this turn was filed under. Entries written before 2026-09-16 have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAgentSessionManifest {
    pub schema_version: u32,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_session_id: Option<String>,
    #[serde(default)]
    pub sessions: Vec<SplitAgentSession>,
    #[serde(default)]
    pub deleted_sessions: Vec<SplitAgentSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAgentSession {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Which agent this chat last talked to. `None` on every chat written before backends
    /// were pluggable, which is why the field is defaulted rather than versioned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The provider's own session handle. Codex keeps using `thread_id`; Claude stores the
    /// UUID the app minted for `--session-id` and later passes to `--resume`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub turns: Vec<SplitAgentSessionTurn>,
    #[serde(default)]
    pub proposal_status: String,
    #[serde(default)]
    pub applied_action_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SplitAgentSessionTurn {
    pub id: String,
    pub created_at: String,
    pub full_turn: Value,
    #[serde(default)]
    pub tool_results: Vec<Value>,
    pub proposal_status: String,
    #[serde(default)]
    pub applied_action_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AgentSessionAction {
    Create {
        id: String,
        title: String,
        thread_id: Option<String>,
        timestamp: String,
    },
    Select {
        session_id: String,
    },
    Rename {
        session_id: String,
        title: String,
        timestamp: String,
    },
    Delete {
        session_id: String,
        timestamp: String,
    },
    Restore {
        session_id: String,
        timestamp: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAppServerConversationWriteResult {
    pub path: String,
    pub entry_count: usize,
    pub latest_entry_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentHistoryWriteResult {
    pub path: String,
    pub entry_count: usize,
    pub latest_entry_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentUndoResult {
    pub project: VideoProject,
    pub report: ProjectWriteReport,
    pub entry_id: String,
    pub action_count: usize,
    pub remaining_agent_history: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectValidationIssue {
    pub path: String,
    pub message: String,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectValidationReport {
    pub ok: bool,
    pub issues: Vec<ProjectValidationIssue>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SplitProjectError {
    #[error("project is already a schema-v2 split project")]
    AlreadySplitProject,
    #[error("manifest field {field} contains unsafe project path: {value}")]
    UnsafeManifestPath { field: String, value: String },
    #[error("project field {field} contains an unsafe sidecar ID: {value}")]
    UnsafeSidecarId { field: String, value: String },
    #[error(
        "project revision conflict: expected revision {expected}, but canonical revision is {actual}"
    )]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("split project contains a writable directory symlink at {path}")]
    UnsafeProjectSymlink { path: String },
    #[error("split project io error at {path}: {message}")]
    Io { path: String, message: String },
    #[error("split project json error at {path}: {message}")]
    Json { path: String, message: String },
    #[error("project action failed: {message}")]
    ProjectAction { message: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SplitProjectTransactionPhase {
    Prepared,
    OriginalMoved,
    Promoted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SplitProjectTransactionJournal {
    schema_version: u32,
    project_dir: PathBuf,
    staging_path: PathBuf,
    backup_path: PathBuf,
    phase: SplitProjectTransactionPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SplitProjectMetadataTransactionJournal {
    project_dir: PathBuf,
    staging_path: PathBuf,
    backup_path: PathBuf,
    relative_paths: Vec<PathBuf>,
    committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SplitProjectTransactionCheckpoint {
    Write(PathBuf),
    BeforePromotion,
    AfterPromotionBeforeParentSync,
    BeforePromotedJournal,
    AfterMetadataCommittedJournalBeforeParentSync,
    BeforeRollback,
    BeforeRestoreOriginal(PathBuf),
    AfterRestoreOriginalBeforeParentSync,
    BeforeCleanup(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitProjectRecoveryStatus {
    Clean,
    CleanupPending,
}

fn bool_is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationLog {
    schema_version: u32,
    project_id: String,
    migrated_at: String,
    source_schema_version: u32,
    target_schema_version: u32,
    written_files: Vec<String>,
}

pub fn default_split_manifest_for_project(project: &VideoProject) -> SplitProjectManifest {
    SplitProjectManifest {
        schema_version: SPLIT_PROJECT_SCHEMA_VERSION,
        content_revision: project.content_revision,
        id: project.id.clone(),
        name: project.name.clone(),
        created_at: project.created_at.clone(),
        updated_at: project.updated_at.clone(),
        layout: SplitProjectLayout::Split,
        files: SplitProjectFiles {
            timeline: "timeline.json".to_string(),
            media: "media/index.json".to_string(),
            transcripts: "transcripts".to_string(),
            templates: "templates".to_string(),
            generated: "generated".to_string(),
            renders: "renders".to_string(),
            logs: "logs".to_string(),
        },
        render_settings: project.render_settings.clone(),
        codex_thread_id: project.codex_thread_id.clone(),
        jobs: project.jobs.clone(),
        export_artifacts: project.export_artifacts.clone(),
    }
}

pub fn split_timeline_from_project(project: &VideoProject) -> SplitTimelineFile {
    let active_timeline_id = project
        .active_timeline_id
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let mut timelines = if project.timelines.is_empty() {
        vec![ProjectTimeline {
            id: active_timeline_id.clone(),
            name: "Timeline 1".to_string(),
            timeline: project.timeline.clone(),
        }]
    } else {
        project.timelines.clone()
    };
    if let Some(active) = timelines
        .iter_mut()
        .find(|timeline| timeline.id == active_timeline_id)
    {
        active.timeline = project.timeline.clone();
    } else {
        timelines.push(ProjectTimeline {
            id: active_timeline_id.clone(),
            name: "Timeline 1".to_string(),
            timeline: project.timeline.clone(),
        });
    }
    SplitTimelineFile {
        schema_version: SPLIT_TIMELINE_SCHEMA_VERSION,
        duration_seconds: project.timeline.duration_seconds,
        tracks: project.timeline.tracks.clone(),
        timelines,
        active_timeline_id: Some(active_timeline_id),
    }
}

pub fn split_media_index_from_project(project: &VideoProject) -> SplitMediaIndexFile {
    SplitMediaIndexFile {
        schema_version: SPLIT_MEDIA_SCHEMA_VERSION,
        folders: project.media_folders.clone(),
        assets: project.media.clone(),
        analysis: project.media_analysis.clone(),
        silence_ranges: project.media_silence_ranges.clone(),
    }
}

pub fn split_transcript_from_runtime(transcript: &Transcript) -> SplitTranscriptFile {
    SplitTranscriptFile {
        schema_version: SPLIT_TRANSCRIPT_SCHEMA_VERSION,
        id: transcript.id.clone(),
        media_id: transcript.media_id.clone(),
        engine: transcript.engine.clone(),
        raw_artifact_path: transcript.raw_artifact_path.clone(),
        repairs: transcript.repairs.clone(),
        segments: transcript.segments.clone(),
        words: transcript.words.clone(),
    }
}

pub fn split_transcript_index_from_project(
    project_dir: &Path,
    transcripts_dir: &Path,
    transcripts: &[SplitTranscriptFile],
) -> SplitTranscriptIndexFile {
    let mut transcripts = transcripts
        .iter()
        .map(|transcript| {
            let path = transcripts_dir.join(format!("{}.json", transcript.media_id));
            let (start_seconds, end_seconds) = transcript_time_bounds(transcript);
            SplitTranscriptIndexEntry {
                transcript_id: transcript.id.clone(),
                media_id: transcript.media_id.clone(),
                path: project_relative_path(project_dir, &path),
                engine: transcript.engine.clone(),
                raw_artifact_path: transcript.raw_artifact_path.clone(),
                repair_count: transcript.repairs.len(),
                segment_count: transcript.segments.len(),
                word_count: transcript.words.len(),
                start_seconds,
                end_seconds,
            }
        })
        .collect::<Vec<_>>();
    transcripts.sort_by(|left, right| left.media_id.cmp(&right.media_id));

    SplitTranscriptIndexFile {
        schema_version: SPLIT_TRANSCRIPT_INDEX_SCHEMA_VERSION,
        transcripts,
    }
}

pub fn split_template_index_from_project(
    project_dir: &Path,
    templates_dir: &Path,
    template_overrides: &[ProjectTemplateOverride],
) -> SplitTemplateIndexFile {
    let mut templates = template_overrides
        .iter()
        .map(|template| {
            let path = templates_dir.join(format!("{}.json", template.template_id));
            SplitTemplateIndexEntry {
                template_id: template.template_id.clone(),
                name: template.name.clone(),
                path: project_relative_path(project_dir, &path),
                visual_treatment: template.visual_treatment.clone(),
                motion: template.motion.clone(),
                safe_zone: template.safe_zone.clone(),
                avoid: template.avoid.clone(),
            }
        })
        .collect::<Vec<_>>();
    templates.sort_by(|left, right| left.template_id.cmp(&right.template_id));

    SplitTemplateIndexFile {
        schema_version: SPLIT_TEMPLATE_INDEX_SCHEMA_VERSION,
        templates,
    }
}

pub fn split_generated_asset_index_from_project(
    project_dir: &Path,
    generated_dir: &Path,
    generated_assets: &[GeneratedAsset],
) -> SplitGeneratedAssetIndexFile {
    let mut assets = generated_assets
        .iter()
        .map(|asset| {
            let path = generated_dir.join(&asset.id).join("asset.json");
            SplitGeneratedAssetIndexEntry {
                asset_id: asset.id.clone(),
                name: asset.name.clone(),
                path: project_relative_path(project_dir, &path),
                status: generated_asset_status_label(&asset.status).to_string(),
                model_provider: asset.model.provider.clone(),
                model_id: asset.model.id.clone(),
                target_folder_id: asset.target_folder_id.clone(),
                placement_intent: asset.placement_intent.clone(),
                output_count: asset.outputs.len(),
                created_at: asset.created_at.clone(),
            }
        })
        .collect::<Vec<_>>();
    assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));

    SplitGeneratedAssetIndexFile {
        schema_version: SPLIT_GENERATED_ASSET_INDEX_SCHEMA_VERSION,
        assets,
    }
}

pub fn split_render_report_index_from_project(
    project_dir: &Path,
    renders_dir: &Path,
    render_reports: &[ProjectRenderReport],
) -> SplitRenderReportIndexFile {
    let mut reports = render_reports
        .iter()
        .map(|report| {
            let path = renders_dir.join(&report.id).join("report.json");
            SplitRenderReportIndexEntry {
                report_id: report.id.clone(),
                path: project_relative_path(project_dir, &path),
                status: render_report_status_label(&report.status).to_string(),
                output_path: report.output_path.clone(),
                duration_seconds: report.duration_seconds,
                video: report.streams.video,
                audio: report.streams.audio,
                check_count: report.checks.len(),
                failed_check_count: report
                    .checks
                    .values()
                    .filter(|status| matches!(status, RenderReportCheckStatus::Failed))
                    .count(),
                artifact_count: report.artifacts.len(),
                log_path: report.log_path.clone(),
                created_at: report.created_at.clone(),
            }
        })
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| left.report_id.cmp(&right.report_id));

    SplitRenderReportIndexFile {
        schema_version: SPLIT_RENDER_REPORT_INDEX_SCHEMA_VERSION,
        reports,
    }
}

pub fn split_workflow_job_index_from_project(jobs: &[JobSummary]) -> SplitWorkflowJobIndexFile {
    let mut jobs = jobs
        .iter()
        .map(|job| {
            let workflow = job.workflow.as_ref();
            let start_request = job.start_request.as_ref();
            SplitWorkflowJobIndexEntry {
                job_id: job.id.clone(),
                kind: job.kind.clone(),
                status: job_status_label(&job.status).to_string(),
                updated_at: job.updated_at.clone(),
                workflow_id: workflow.map(|workflow| workflow.workflow_id.clone()),
                workflow_type: workflow.map(|workflow| workflow.workflow_type.clone()),
                task_queue: workflow.map(|workflow| workflow.task_queue.clone()),
                run_id: workflow.and_then(|workflow| workflow.run_id.clone()),
                activity_count: workflow
                    .map(|workflow| workflow.activity_types.len())
                    .unwrap_or(0),
                has_start_request: start_request.is_some(),
                start_request_ready: workflow.zip(start_request).is_some_and(
                    |(workflow, start_request)| {
                        workflow_start_request_ready(workflow, start_request)
                    },
                ),
                id_reuse_policy: start_request.map(|request| request.id_reuse_policy.clone()),
            }
        })
        .collect::<Vec<_>>();
    jobs.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.job_id.cmp(&right.job_id))
    });

    SplitWorkflowJobIndexFile {
        schema_version: SPLIT_WORKFLOW_JOB_INDEX_SCHEMA_VERSION,
        jobs,
    }
}

pub fn split_export_artifact_index_from_project(
    export_artifacts: &[ProjectExportArtifact],
) -> SplitExportArtifactIndexFile {
    let mut artifacts = export_artifacts
        .iter()
        .map(|artifact| SplitExportArtifactIndexEntry {
            artifact_id: artifact.id.clone(),
            kind: export_artifact_kind_label(&artifact.kind).to_string(),
            format: artifact.format.clone(),
            path: artifact.path.clone(),
            mime_type: artifact.mime_type.clone(),
            job_id: artifact.job_id.clone(),
            created_at: artifact.created_at.clone(),
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| left.artifact_id.cmp(&right.artifact_id))
    });

    SplitExportArtifactIndexFile {
        schema_version: SPLIT_EXPORT_ARTIFACT_INDEX_SCHEMA_VERSION,
        artifacts,
    }
}

pub fn split_project_context_from_project(
    project: &VideoProject,
    manifest: &SplitProjectManifest,
) -> SplitProjectContextFile {
    SplitProjectContextFile {
        schema_version: SPLIT_PROJECT_CONTEXT_SCHEMA_VERSION,
        project_id: project.id.clone(),
        name: project.name.clone(),
        updated_at: project.updated_at.clone(),
        files: SplitProjectContextFiles {
            manifest: super::storage::PROJECT_FILE_NAME.to_string(),
            timeline: manifest.files.timeline.clone(),
            media: manifest.files.media.clone(),
            transcripts: manifest.files.transcripts.clone(),
            templates: manifest.files.templates.clone(),
            generated: manifest.files.generated.clone(),
            renders: manifest.files.renders.clone(),
            jobs: SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME.to_string(),
            exports: SPLIT_EXPORT_ARTIFACT_INDEX_DIR_NAME.to_string(),
            logs: manifest.files.logs.clone(),
        },
        indexes: SplitProjectContextIndexes {
            transcripts: format!(
                "{}/{}",
                manifest.files.transcripts, SPLIT_TRANSCRIPT_INDEX_FILE_NAME
            ),
            templates: format!(
                "{}/{}",
                manifest.files.templates, SPLIT_TEMPLATE_INDEX_FILE_NAME
            ),
            generated: format!(
                "{}/{}",
                manifest.files.generated, SPLIT_GENERATED_ASSET_INDEX_FILE_NAME
            ),
            renders: format!(
                "{}/{}",
                manifest.files.renders, SPLIT_RENDER_REPORT_INDEX_FILE_NAME
            ),
            jobs: format!(
                "{}/{}",
                SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME, SPLIT_WORKFLOW_JOB_INDEX_FILE_NAME
            ),
            exports: format!(
                "{}/{}",
                SPLIT_EXPORT_ARTIFACT_INDEX_DIR_NAME, SPLIT_EXPORT_ARTIFACT_INDEX_FILE_NAME
            ),
            search: "search/index.json".to_string(),
        },
        counts: SplitProjectContextCounts {
            media: project.media.len(),
            media_folders: project.media_folders.len(),
            timeline_tracks: project.timeline.tracks.len(),
            timeline_items: project
                .timeline
                .tracks
                .iter()
                .map(|track| track.items.len())
                .sum(),
            transcripts: project.transcripts.len(),
            template_overrides: project.template_overrides.len(),
            generated_assets: project.generated_assets.len(),
            render_reports: project.render_reports.len(),
            workflow_jobs: project.jobs.len(),
            export_artifacts: project.export_artifacts.len(),
        },
        latest: SplitProjectContextLatest {
            generated_asset_id: latest_generated_asset_id(&project.generated_assets),
            render_report_id: latest_render_report_id(&project.render_reports),
            workflow_job_id: latest_workflow_job_id(&project.jobs),
            export_artifact_id: latest_export_artifact_id(&project.export_artifacts),
        },
    }
}

pub fn runtime_transcript_from_split(file: SplitTranscriptFile) -> Transcript {
    Transcript {
        id: file.id,
        media_id: file.media_id,
        engine: file.engine,
        raw_artifact_path: file.raw_artifact_path,
        repairs: file.repairs,
        segments: file.segments,
        words: file.words,
    }
}

pub struct SplitProjectParts {
    pub manifest: SplitProjectManifest,
    pub timeline: SplitTimelineFile,
    pub media: SplitMediaIndexFile,
    pub transcripts: Vec<SplitTranscriptFile>,
    pub template_overrides: Vec<ProjectTemplateOverride>,
    pub generated_assets: Vec<GeneratedAsset>,
    pub render_reports: Vec<ProjectRenderReport>,
    pub export_artifacts: Vec<ProjectExportArtifact>,
    pub jobs: Vec<JobSummary>,
}

pub fn runtime_project_from_split_parts(parts: SplitProjectParts) -> VideoProject {
    let SplitProjectParts {
        manifest,
        timeline,
        media,
        transcripts,
        template_overrides,
        generated_assets,
        render_reports,
        export_artifacts,
        jobs,
    } = parts;
    let legacy_timeline = Timeline {
        duration_seconds: timeline.duration_seconds,
        tracks: timeline.tracks,
    };
    let active_timeline_id = timeline
        .active_timeline_id
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let mut timelines = timeline.timelines;
    if timelines.is_empty() {
        timelines.push(ProjectTimeline {
            id: active_timeline_id.clone(),
            name: "Timeline 1".to_string(),
            timeline: legacy_timeline.clone(),
        });
    }
    let active_timeline = timelines
        .iter()
        .find(|entry| entry.id == active_timeline_id)
        .map(|entry| entry.timeline.clone())
        .unwrap_or(legacy_timeline);
    VideoProject {
        schema_version: manifest.schema_version,
        content_revision: manifest.content_revision,
        id: manifest.id,
        name: manifest.name,
        created_at: manifest.created_at,
        updated_at: manifest.updated_at,
        media: media.assets,
        media_analysis: media.analysis,
        media_silence_ranges: media.silence_ranges,
        media_folders: media.folders,
        template_overrides,
        generated_assets,
        render_reports,
        export_artifacts,
        transcripts: transcripts
            .into_iter()
            .map(runtime_transcript_from_split)
            .collect(),
        timelines,
        active_timeline_id: Some(active_timeline_id),
        timeline: active_timeline,
        render_settings: manifest.render_settings,
        codex_thread_id: manifest.codex_thread_id,
        jobs,
    }
}

fn load_native_palmier_package(project_dir: &Path) -> Result<VideoProject, SplitProjectError> {
    let project_path = project_dir.join("project.json");
    let media_path = project_dir.join("media.json");
    let project_file: PalmierProjectFile = read_json_file(&project_path)?;
    let media_manifest: PalmierMediaManifest = read_json_file(&media_path)?;
    let timeline = palmier_active_timeline(&project_file, &project_path)?;
    let media = palmier_media_assets(project_dir, &media_manifest)?;
    let media_kind_by_id = media
        .iter()
        .map(|asset| (asset.id.clone(), asset.kind.clone()))
        .collect::<BTreeMap<_, _>>();
    let generated_assets = palmier_generated_assets(&media_manifest, &media)?;
    let tracks = palmier_timeline_tracks(timeline, &media_kind_by_id);
    let duration_seconds = tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.start_seconds + item.duration_seconds)
        .filter(|value| value.is_finite())
        .fold(0.0, f64::max);
    let package_name = project_dir
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Palmier Project");
    let project_name = if timeline.name.trim().is_empty() {
        package_name.to_string()
    } else {
        timeline.name.trim().to_string()
    };

    let active_timeline = Timeline {
        duration_seconds,
        tracks,
    };
    Ok(VideoProject {
        schema_version: SPLIT_PROJECT_SCHEMA_VERSION,
        content_revision: 0,
        id: format!("palmier-{}", safe_import_id_segment(package_name)),
        name: project_name,
        created_at: "1970-01-01T00:00:00Z".to_string(),
        updated_at: "1970-01-01T00:00:00Z".to_string(),
        media,
        media_analysis: Vec::new(),
        media_silence_ranges: Vec::new(),
        media_folders: palmier_media_folders(&media_manifest),
        template_overrides: Vec::new(),
        generated_assets,
        render_reports: Vec::new(),
        export_artifacts: Vec::new(),
        transcripts: Vec::new(),
        timelines: vec![ProjectTimeline {
            id: "main".to_string(),
            name: timeline.name.clone(),
            timeline: active_timeline.clone(),
        }],
        active_timeline_id: Some("main".to_string()),
        timeline: active_timeline,
        render_settings: RenderSettings {
            width: timeline.width,
            height: timeline.height,
            fps: timeline.fps as f64,
            loudness_lufs: -14.0,
            captions: CaptionRenderMode::BurnIn,
        },
        codex_thread_id: None,
        jobs: Vec::new(),
    })
}

fn palmier_active_timeline<'a>(
    project_file: &'a PalmierProjectFile,
    project_path: &Path,
) -> Result<&'a PalmierTimeline, SplitProjectError> {
    if project_file.timelines.is_empty() {
        return Err(SplitProjectError::Json {
            path: project_path.display().to_string(),
            message: "native Palmier project has no timelines".to_string(),
        });
    }

    if let Some(active_timeline_id) = project_file
        .active_timeline_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(timeline) = project_file
            .timelines
            .iter()
            .find(|timeline| timeline.id == active_timeline_id)
        {
            return Ok(timeline);
        }
    }

    Ok(&project_file.timelines[0])
}

fn palmier_media_assets(
    project_dir: &Path,
    media_manifest: &PalmierMediaManifest,
) -> Result<Vec<MediaAsset>, SplitProjectError> {
    media_manifest
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let relative_path = palmier_media_source_path(project_dir, index, &entry.source)?;
            let kind = if entry.generation_input.is_some() {
                MediaKind::Generated
            } else {
                palmier_media_kind(&entry.kind)
            };
            Ok(MediaAsset {
                id: entry.id.clone(),
                name: Some(entry.name.clone()).filter(|name| !name.trim().is_empty()),
                relative_path,
                kind,
                duration_seconds: finite_non_negative(entry.duration).unwrap_or(0.0),
                width: entry.source_width,
                height: entry.source_height,
                fps: entry
                    .source_fps
                    .filter(|value| value.is_finite() && *value >= 0.0),
                folder_id: entry.folder_id.clone(),
            })
        })
        .collect()
}

fn palmier_media_folders(media_manifest: &PalmierMediaManifest) -> Vec<MediaFolder> {
    media_manifest
        .folders
        .iter()
        .map(|folder| MediaFolder {
            id: folder.id.clone(),
            name: folder.name.clone(),
            parent_id: folder.parent_folder_id.clone(),
        })
        .collect()
}

fn palmier_media_source_path(
    project_dir: &Path,
    index: usize,
    source: &PalmierMediaSource,
) -> Result<String, SplitProjectError> {
    match source {
        PalmierMediaSource::Project { relative_path } => {
            safe_join(
                project_dir,
                &format!("media.json entries[{index}].source"),
                relative_path,
            )?;
            Ok(relative_path.clone())
        }
        PalmierMediaSource::External { absolute_path } => {
            let path = Path::new(absolute_path);
            if path.is_absolute() && path.starts_with(project_dir) {
                Ok(project_relative_path(project_dir, path))
            } else {
                Err(SplitProjectError::UnsafeManifestPath {
                    field: format!("media.json entries[{index}].source.external.absolutePath"),
                    value: absolute_path.clone(),
                })
            }
        }
    }
}

fn palmier_generated_assets(
    media_manifest: &PalmierMediaManifest,
    media: &[MediaAsset],
) -> Result<Vec<GeneratedAsset>, SplitProjectError> {
    let media_by_id = media
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<BTreeMap<_, _>>();
    let mut generated_assets = Vec::new();

    for entry in &media_manifest.entries {
        let Some(input) = &entry.generation_input else {
            continue;
        };
        let Some(output_media) = media_by_id.get(entry.id.as_str()) else {
            continue;
        };
        let output_index = input.output_index.unwrap_or(0);
        let source_url = input
            .result_urls
            .get(output_index)
            .or_else(|| input.result_urls.first())
            .cloned();
        let provider_input_urls = palmier_provider_input_urls(input);
        let reference_image_media_refs = dedup_strings(
            input
                .reference_image_asset_ids
                .iter()
                .chain(&input.image_url_asset_ids),
        );
        let reference_video_media_refs = dedup_strings(input.reference_video_asset_ids.iter());
        let reference_audio_media_refs = dedup_strings(input.reference_audio_asset_ids.iter());
        let media_ids = dedup_strings(
            reference_image_media_refs
                .iter()
                .chain(&reference_video_media_refs)
                .chain(&reference_audio_media_refs),
        );
        let created_at = input
            .created_at
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

        generated_assets.push(GeneratedAsset {
            schema_version: SPLIT_GENERATED_ASSET_SCHEMA_VERSION,
            id: format!("palmier-{}", safe_import_id_segment(&entry.id)),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: Some(entry.name.clone()).filter(|name| !name.trim().is_empty()),
            target_folder_id: entry.folder_id.clone(),
            placement_intent: None,
            prompt: input.prompt.clone(),
            model: GenerationModel {
                provider: palmier_generation_provider(&input.model).to_string(),
                id: input.model.clone(),
            },
            references: GeneratedAssetReferences {
                media_ids,
                source_video_media_ref: reference_video_media_refs.first().cloned(),
                first_frame_media_id: input.image_url_asset_ids.first().cloned(),
                last_frame_media_id: None,
                reference_image_media_refs,
                reference_video_media_refs,
                reference_audio_media_refs,
                provider_input_urls,
            },
            settings: GeneratedAssetSettings {
                width: output_media.width,
                height: output_media.height,
                duration_seconds: Some(input.duration as f64),
                fps: output_media.fps,
                aspect_ratio: Some(input.aspect_ratio.clone())
                    .filter(|value| !value.trim().is_empty()),
                resolution: input.resolution.clone(),
                num_images: input.num_images,
                quality: input.quality.clone(),
                generate_audio: input.generate_audio,
                voice: input.voice.clone(),
                lyrics: input.lyrics.clone(),
                style_instructions: input.style_instructions.clone(),
                instrumental: input.instrumental,
                ..GeneratedAssetSettings::default()
            },
            outputs: vec![GeneratedAssetOutput {
                media_id: output_media.id.clone(),
                relative_path: output_media.relative_path.clone(),
                source_url,
                width: output_media.width.unwrap_or(0),
                height: output_media.height.unwrap_or(0),
                duration_seconds: output_media.duration_seconds,
                fps: output_media.fps.unwrap_or(0.0),
            }],
            created_at,
            parent_asset_id: None,
            retry_of_asset_id: None,
        });
    }

    Ok(generated_assets)
}

fn palmier_provider_input_urls(input: &PalmierGenerationInput) -> Vec<String> {
    dedup_strings(
        input
            .image_urls
            .iter()
            .chain(&input.reference_image_urls)
            .chain(&input.reference_video_urls)
            .chain(&input.reference_audio_urls),
    )
}

fn palmier_timeline_tracks(
    timeline: &PalmierTimeline,
    media_kind_by_id: &BTreeMap<String, MediaKind>,
) -> Vec<TimelineTrack> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let kind = palmier_track_kind(&track.kind);
            let fps = timeline.fps.max(1) as f64;
            let items = track
                .clips
                .iter()
                .enumerate()
                .filter_map(|(clip_index, clip)| {
                    palmier_timeline_item(clip, clip_index, fps, media_kind_by_id)
                })
                .collect::<Vec<_>>();
            TimelineTrack {
                transitions: Vec::new(),
                id: if track.id.trim().is_empty() {
                    format!("palmier-track-{index}")
                } else {
                    track.id.clone()
                },
                name: palmier_track_name(&track.kind, index),
                kind,
                locked: false,
                sync_locked: false,
                enabled: !track.hidden && !track.muted,
                items,
            }
        })
        .collect()
}

fn palmier_timeline_item(
    clip: &PalmierClip,
    clip_index: usize,
    fps: f64,
    media_kind_by_id: &BTreeMap<String, MediaKind>,
) -> Option<TimelineItem> {
    let duration_seconds = finite_non_negative(clip.duration_frames as f64 / fps)?;
    if duration_seconds <= 0.0 {
        return None;
    }
    let start_seconds = finite_non_negative(clip.start_frame as f64 / fps).unwrap_or(0.0);
    let clip_type = clip
        .media_type
        .as_ref()
        .or(clip.source_clip_type.as_ref())
        .cloned()
        .unwrap_or(PalmierClipType::Video);
    let id = if clip.id.trim().is_empty() {
        format!("palmier-clip-{clip_index}")
    } else {
        clip.id.clone()
    };
    let label = if clip_type == PalmierClipType::Text {
        clip.text_content
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Text".to_string())
    } else {
        clip.media_ref.clone()
    };
    let source = if clip_type == PalmierClipType::Text {
        TimelineSource::Text {
            text: clip.text_content.clone().unwrap_or_default(),
        }
    } else if media_kind_by_id.get(&clip.media_ref) == Some(&MediaKind::Generated) {
        TimelineSource::Generated {
            artifact_id: clip.media_ref.clone(),
        }
    } else {
        TimelineSource::Media {
            media_id: clip.media_ref.clone(),
        }
    };
    let mut properties = BTreeMap::new();
    if clip_type != PalmierClipType::Text {
        let speed = if clip.speed.is_finite() && clip.speed > 0.0 {
            clip.speed
        } else {
            default_palmier_clip_speed()
        };
        let source_start_frame = clip.trim_start_frame.max(0) as f64;
        let source_frames_consumed = ((clip.duration_frames.max(0) as f64) * speed)
            .round()
            .max(0.0);
        let source_in = source_start_frame / fps;
        let source_out = (source_start_frame + source_frames_consumed) / fps;
        properties.insert("sourceIn".to_string(), serde_json::json!(source_in));
        properties.insert("sourceOut".to_string(), serde_json::json!(source_out));
        if clip.trim_start_frame > 0 {
            properties.insert(
                "trimStartFrame".to_string(),
                serde_json::json!(clip.trim_start_frame),
            );
        }
        if (speed - 1.0).abs() > f64::EPSILON {
            properties.insert("speed".to_string(), serde_json::json!(speed));
        }
    }
    if let Some(link_group_id) = clip
        .link_group_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        properties.insert("linkGroupId".to_string(), serde_json::json!(link_group_id));
    }
    if let Some(trim_end_frame) = (clip.trim_end_frame > 0).then_some(clip.trim_end_frame) {
        properties.insert(
            "trimEndFrame".to_string(),
            serde_json::json!(trim_end_frame),
        );
    }
    if media_kind_by_id.get(&clip.media_ref) == Some(&MediaKind::Generated) {
        properties.insert(
            "generatedAssetId".to_string(),
            serde_json::json!(format!(
                "palmier-{}",
                safe_import_id_segment(&clip.media_ref)
            )),
        );
        properties.insert(
            "generatedOutputMediaId".to_string(),
            serde_json::json!(clip.media_ref.clone()),
        );
    }

    Some(TimelineItem {
        id,
        kind: palmier_timeline_item_kind(&clip_type, media_kind_by_id.get(&clip.media_ref)),
        start_seconds,
        duration_seconds,
        source,
        label,
        properties,
    })
}

fn palmier_media_kind(kind: &PalmierClipType) -> MediaKind {
    match kind {
        PalmierClipType::Audio => MediaKind::Audio,
        PalmierClipType::Image => MediaKind::Image,
        PalmierClipType::Lottie => MediaKind::Lottie,
        PalmierClipType::Video | PalmierClipType::Sequence | PalmierClipType::Text => {
            MediaKind::Video
        }
    }
}

fn palmier_track_kind(kind: &PalmierClipType) -> TrackKind {
    match kind {
        PalmierClipType::Audio => TrackKind::Audio,
        PalmierClipType::Text => TrackKind::Overlay,
        PalmierClipType::Video
        | PalmierClipType::Image
        | PalmierClipType::Lottie
        | PalmierClipType::Sequence => TrackKind::Video,
    }
}

fn palmier_timeline_item_kind(
    clip_type: &PalmierClipType,
    media_kind: Option<&MediaKind>,
) -> TimelineItemKind {
    if media_kind == Some(&MediaKind::Generated) {
        return TimelineItemKind::GeneratedClip;
    }
    match clip_type {
        PalmierClipType::Audio => TimelineItemKind::AudioClip,
        PalmierClipType::Image => TimelineItemKind::ImageClip,
        PalmierClipType::Lottie => TimelineItemKind::LottieClip,
        PalmierClipType::Text => TimelineItemKind::Overlay,
        PalmierClipType::Video | PalmierClipType::Sequence => TimelineItemKind::VideoClip,
    }
}

fn palmier_track_name(kind: &PalmierClipType, index: usize) -> String {
    let label = match kind {
        PalmierClipType::Audio => "Audio",
        PalmierClipType::Image => "Image",
        PalmierClipType::Text => "Text",
        PalmierClipType::Lottie => "Lottie",
        PalmierClipType::Sequence => "Sequence",
        PalmierClipType::Video => "Video",
    };
    format!("{label} {}", index + 1)
}

fn palmier_generation_provider(model_id: &str) -> &'static str {
    let normalized = model_id.trim().to_ascii_lowercase();
    if normalized.starts_with("fal-ai/")
        || normalized.starts_with("fal/")
        || normalized.contains("wan")
        || normalized.contains("kling")
        || normalized.contains("sonilo")
        || normalized.contains("mirelo")
    {
        "fal.ai"
    } else if normalized.starts_with("openai/")
        || normalized.contains("gpt-image")
        || normalized.contains("gpt-4o")
    {
        "openai"
    } else if normalized.starts_with("xai/") || normalized.contains("grok") {
        "xai"
    } else if normalized.starts_with("google/")
        || normalized.contains("veo")
        || normalized.contains("gemini")
        || normalized.contains("lyria")
    {
        "google"
    } else if normalized.starts_with("elevenlabs/") || normalized.contains("eleven") {
        "elevenlabs"
    } else if normalized.starts_with("minimax/") || normalized.contains("minimax") {
        "minimax"
    } else if normalized.starts_with("black-forest-labs/")
        || normalized.starts_with("bytedance/")
        || normalized.contains("seedance")
    {
        "replicate"
    } else {
        "palmier"
    }
}

fn finite_non_negative(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn dedup_strings<'a>(values: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

fn safe_import_id_segment(value: &str) -> String {
    let segment = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if segment.is_empty() {
        "import".to_string()
    } else {
        segment
    }
}

pub fn resolve_project_relative_path(
    project_dir: &Path,
    value: &str,
) -> Result<PathBuf, SplitProjectError> {
    safe_join(project_dir, "path", value)
}

pub fn validate_split_project_write_path(
    project_dir: &Path,
    path: &Path,
) -> Result<(), SplitProjectError> {
    let relative =
        path.strip_prefix(project_dir)
            .map_err(|_| SplitProjectError::UnsafeManifestPath {
                field: "path".to_string(),
                value: path.display().to_string(),
            })?;
    let mut current = project_dir.to_path_buf();
    reject_split_project_symlink(&current)?;
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(SplitProjectError::UnsafeManifestPath {
                field: "path".to_string(),
                value: path.display().to_string(),
            });
        };
        current.push(component);
        reject_split_project_symlink(&current)?;
    }
    Ok(())
}

fn reject_split_project_symlink(path: &Path) -> Result<(), SplitProjectError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(SplitProjectError::UnsafeProjectSymlink {
                path: path.display().to_string(),
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        }),
    }
}

pub fn split_project_manifest_path(project_dir: &Path) -> PathBuf {
    project_dir.join(super::storage::PROJECT_FILE_NAME)
}

fn split_project_mutation_lease(
    project_dir: &Path,
) -> Result<SplitProjectMutationLease, SplitProjectError> {
    acquire_split_project_mutation_lease(project_dir).map_err(|message| SplitProjectError::Io {
        path: project_dir.display().to_string(),
        message,
    })
}

pub fn save_split_project(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let _ = recover_pending_split_project_transaction(project_dir)?;
    let existing = existing_project_manifest(project_dir)?;
    let mut next_project = project.clone();
    next_project.content_revision = existing.next_content_revision(project_dir)?;
    canonicalize_project_for_split_save(&mut next_project);
    validate_project_sidecar_ids(&next_project)?;
    // A schema-v1 single-file project shares the manifest file name. It has no split sidecars to
    // back up, so it takes the whole-package transaction that migration relies on.
    let report = if matches!(existing, ExistingProjectManifest::Split { .. }) {
        save_split_project_metadata_transactionally(project_dir, &next_project, &mut |_| Ok(()))
    } else {
        save_split_project_transactionally(project_dir, &next_project, &mut |_| Ok(()))
    }?;
    Ok(ProjectActionWriteResult {
        project: next_project,
        report,
    })
}

fn canonicalize_project_for_split_save(project: &mut VideoProject) {
    project.schema_version = SPLIT_PROJECT_SCHEMA_VERSION;
    let timeline = split_timeline_from_project(project);
    let active_timeline_id = timeline
        .active_timeline_id
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let active_timeline = timeline
        .timelines
        .iter()
        .find(|entry| entry.id == active_timeline_id)
        .map(|entry| entry.timeline.clone())
        .unwrap_or_else(|| Timeline {
            duration_seconds: timeline.duration_seconds,
            tracks: timeline.tracks.clone(),
        });
    project.timelines = timeline.timelines;
    project.active_timeline_id = Some(active_timeline_id);
    project.timeline = active_timeline;
}

pub fn replace_split_project_if_revision(
    project_dir: &Path,
    mut replacement: VideoProject,
    expected_revision: u64,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let _ = recover_pending_split_project_transaction(project_dir)?;
    let manifest_path = split_project_manifest_path(project_dir);
    let canonical = if manifest_path.exists() {
        Some(load_split_project_without_recovery(project_dir)?)
    } else {
        None
    };
    let actual_revision = canonical
        .as_ref()
        .map(|project| project.content_revision)
        .unwrap_or(0);
    if actual_revision != expected_revision {
        return Err(SplitProjectError::RevisionConflict {
            expected: expected_revision,
            actual: actual_revision,
        });
    }
    if let Some(canonical) = &canonical {
        preserve_worker_owned_state(canonical, &mut replacement);
    }
    replacement.content_revision =
        actual_revision
            .checked_add(1)
            .ok_or_else(|| SplitProjectError::Io {
                path: split_project_manifest_path(project_dir)
                    .display()
                    .to_string(),
                message: "project content revision is exhausted".to_string(),
            })?;
    canonicalize_project_for_split_save(&mut replacement);
    validate_project_sidecar_ids(&replacement)?;
    let report = if canonical.is_some() {
        save_split_project_metadata_transactionally(project_dir, &replacement, &mut |_| Ok(()))?
    } else {
        save_split_project_transactionally(project_dir, &replacement, &mut |_| Ok(()))?
    };
    Ok(ProjectActionWriteResult {
        project: replacement,
        report,
    })
}

fn preserve_worker_owned_state(canonical: &VideoProject, replacement: &mut VideoProject) {
    replacement.media_analysis = canonical.media_analysis.clone();
    replacement.media_silence_ranges = canonical.media_silence_ranges.clone();
    replacement.generated_assets = canonical.generated_assets.clone();
    replacement.render_reports = canonical.render_reports.clone();
    replacement.export_artifacts = canonical.export_artifacts.clone();
    replacement.jobs = canonical.jobs.clone();

    let generated_media_ids = canonical
        .generated_assets
        .iter()
        .flat_map(|asset| asset.outputs.iter().map(|output| output.media_id.as_str()))
        .collect::<BTreeSet<_>>();
    for media in &canonical.media {
        if generated_media_ids.contains(media.id.as_str()) {
            if let Some(existing) = replacement
                .media
                .iter_mut()
                .find(|entry| entry.id == media.id)
            {
                *existing = media.clone();
            } else {
                replacement.media.push(media.clone());
            }
        }
    }
    for transcript in &canonical.transcripts {
        if let Some(existing) = replacement
            .transcripts
            .iter_mut()
            .find(|entry| entry.media_id == transcript.media_id)
        {
            *existing = transcript.clone();
        } else {
            replacement.transcripts.push(transcript.clone());
        }
    }
}

/// What currently occupies a package's `video-creater.project.json`.
enum ExistingProjectManifest {
    Missing,
    /// A schema-v1 single-file project awaiting migration to the split layout.
    SingleFile {
        content_revision: u64,
    },
    Split {
        content_revision: u64,
    },
}

impl ExistingProjectManifest {
    fn next_content_revision(&self, project_dir: &Path) -> Result<u64, SplitProjectError> {
        let current = match self {
            Self::Missing => 0,
            Self::SingleFile { content_revision } | Self::Split { content_revision } => {
                *content_revision
            }
        };
        current.checked_add(1).ok_or_else(|| SplitProjectError::Io {
            path: split_project_manifest_path(project_dir)
                .display()
                .to_string(),
            message: "project content revision is exhausted".to_string(),
        })
    }
}

fn existing_project_manifest(
    project_dir: &Path,
) -> Result<ExistingProjectManifest, SplitProjectError> {
    let manifest_path = split_project_manifest_path(project_dir);
    if !manifest_path.exists() {
        return Ok(ExistingProjectManifest::Missing);
    }
    let json = std::fs::read_to_string(&manifest_path).map_err(|error| SplitProjectError::Io {
        path: manifest_path.display().to_string(),
        message: error.to_string(),
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&json).map_err(|error| {
        SplitProjectError::Json {
            path: manifest_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    if !is_split_manifest_value(&value) {
        return Ok(ExistingProjectManifest::SingleFile {
            content_revision: value
                .get("contentRevision")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        });
    }
    let manifest: SplitProjectManifest =
        serde_json::from_value(value).map_err(|error| SplitProjectError::Json {
            path: manifest_path.display().to_string(),
            message: error.to_string(),
        })?;
    Ok(ExistingProjectManifest::Split {
        content_revision: manifest.content_revision,
    })
}

fn is_split_manifest_value(value: &serde_json::Value) -> bool {
    value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        == Some(SPLIT_PROJECT_SCHEMA_VERSION as u64)
        && value.get("layout").and_then(serde_json::Value::as_str) == Some("split")
}

fn advance_project_content_revision(
    project_dir: &Path,
    project: &mut VideoProject,
) -> Result<(), SplitProjectError> {
    project.content_revision =
        project
            .content_revision
            .checked_add(1)
            .ok_or_else(|| SplitProjectError::Io {
                path: split_project_manifest_path(project_dir)
                    .display()
                    .to_string(),
                message: "project content revision is exhausted".to_string(),
            })?;
    Ok(())
}

fn validate_project_sidecar_ids(project: &VideoProject) -> Result<(), SplitProjectError> {
    for (field, value) in project
        .transcripts
        .iter()
        .map(|entry| ("transcripts[].mediaId", entry.media_id.as_str()))
        .chain(
            project
                .template_overrides
                .iter()
                .map(|entry| ("templateOverrides[].templateId", entry.template_id.as_str())),
        )
        .chain(
            project
                .generated_assets
                .iter()
                .map(|entry| ("generatedAssets[].id", entry.id.as_str())),
        )
        .chain(
            project
                .render_reports
                .iter()
                .map(|entry| ("renderReports[].id", entry.id.as_str())),
        )
        .chain(
            project
                .jobs
                .iter()
                .map(|entry| ("jobs[].id", entry.id.as_str())),
        )
        .chain(
            project
                .export_artifacts
                .iter()
                .map(|entry| ("exportArtifacts[].id", entry.id.as_str())),
        )
    {
        validate_sidecar_id_segment(field, value)?;
    }
    Ok(())
}

fn validate_sidecar_id_segment(field: &str, value: &str) -> Result<(), SplitProjectError> {
    let path = Path::new(value);
    let mut components = path.components();
    let safe = !value.trim().is_empty()
        && !value.contains(['/', '\\'])
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none();
    if safe {
        Ok(())
    } else {
        Err(SplitProjectError::UnsafeSidecarId {
            field: field.to_string(),
            value: value.to_string(),
        })
    }
}

/// Writes the split package files for `project` under `project_dir`, keeping the manifest file
/// layout declared by the package at `layout_dir` (the same directory unless staging elsewhere).
fn save_split_project_with_write_checkpoint(
    project_dir: &Path,
    layout_dir: &Path,
    project: &VideoProject,
    write_checkpoint: &mut dyn FnMut(&Path) -> Result<(), SplitProjectError>,
) -> Result<ProjectWriteReport, SplitProjectError> {
    validate_project_sidecar_ids(project)?;
    let manifest = save_manifest_for_project(layout_dir, project)?;
    let timeline = split_timeline_from_project(project);
    let media = split_media_index_from_project(project);
    let transcript_files = project
        .transcripts
        .iter()
        .map(split_transcript_from_runtime)
        .collect::<Vec<_>>();

    create_split_dirs(project_dir, &manifest)?;

    let manifest_path = split_project_manifest_path(project_dir);
    let timeline_path = safe_join(project_dir, "files.timeline", &manifest.files.timeline)?;
    let media_path = safe_join(project_dir, "files.media", &manifest.files.media)?;
    let transcripts_dir = safe_join(
        project_dir,
        "files.transcripts",
        &manifest.files.transcripts,
    )?;
    let templates_dir = safe_join(project_dir, "files.templates", &manifest.files.templates)?;
    let generated_dir = safe_join(project_dir, "files.generated", &manifest.files.generated)?;
    let renders_dir = safe_join(project_dir, "files.renders", &manifest.files.renders)?;
    let workflow_jobs_dir = project_dir.join(SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME);
    let export_artifacts_dir = project_dir.join(SPLIT_EXPORT_ARTIFACT_INDEX_DIR_NAME);
    let project_context_dir = project_dir.join(SPLIT_PROJECT_CONTEXT_DIR_NAME);
    let search_index_path = project_search_index_path(project_dir);

    let mut written_files = Vec::new();
    let mut expected_transcript_paths = BTreeSet::new();
    let mut expected_template_paths = BTreeSet::new();
    let mut expected_generated_paths = BTreeSet::new();
    let mut expected_render_report_paths = BTreeSet::new();
    let mut expected_export_artifact_paths = BTreeSet::new();
    let mut expected_workflow_job_paths = BTreeSet::new();
    write_json_file_and_checkpoint(&manifest_path, &manifest, write_checkpoint)?;
    written_files.push(manifest_path.display().to_string());
    write_json_file_and_checkpoint(&timeline_path, &timeline, write_checkpoint)?;
    written_files.push(timeline_path.display().to_string());
    write_json_file_and_checkpoint(&media_path, &media, write_checkpoint)?;
    written_files.push(media_path.display().to_string());

    for transcript in &transcript_files {
        let path = transcripts_dir.join(format!("{}.json", transcript.media_id));
        validate_split_project_write_path(project_dir, &path)?;
        write_json_file_and_checkpoint(&path, transcript, write_checkpoint)?;
        expected_transcript_paths.insert(path.clone());
        written_files.push(path.display().to_string());
    }
    let transcript_index_path = transcripts_dir.join(SPLIT_TRANSCRIPT_INDEX_FILE_NAME);
    let transcript_index =
        split_transcript_index_from_project(project_dir, &transcripts_dir, &transcript_files);
    write_json_file_and_checkpoint(&transcript_index_path, &transcript_index, write_checkpoint)?;
    expected_transcript_paths.insert(transcript_index_path.clone());
    written_files.push(transcript_index_path.display().to_string());
    for template_override in &project.template_overrides {
        let path = templates_dir.join(format!("{}.json", template_override.template_id));
        validate_split_project_write_path(project_dir, &path)?;
        write_json_file_and_checkpoint(&path, template_override, write_checkpoint)?;
        expected_template_paths.insert(path.clone());
        written_files.push(path.display().to_string());
    }
    let template_index_path = templates_dir.join(SPLIT_TEMPLATE_INDEX_FILE_NAME);
    let template_index =
        split_template_index_from_project(project_dir, &templates_dir, &project.template_overrides);
    write_json_file_and_checkpoint(&template_index_path, &template_index, write_checkpoint)?;
    expected_template_paths.insert(template_index_path.clone());
    written_files.push(template_index_path.display().to_string());
    for generated_asset in &project.generated_assets {
        let path = generated_dir.join(&generated_asset.id).join("asset.json");
        validate_split_project_write_path(project_dir, &path)?;
        write_json_file_and_checkpoint(&path, generated_asset, write_checkpoint)?;
        expected_generated_paths.insert(path.clone());
        written_files.push(path.display().to_string());
    }
    let generated_index_path = generated_dir.join(SPLIT_GENERATED_ASSET_INDEX_FILE_NAME);
    let generated_index = split_generated_asset_index_from_project(
        project_dir,
        &generated_dir,
        &project.generated_assets,
    );
    write_json_file_and_checkpoint(&generated_index_path, &generated_index, write_checkpoint)?;
    written_files.push(generated_index_path.display().to_string());
    for render_report in &project.render_reports {
        let path = renders_dir.join(&render_report.id).join("report.json");
        validate_split_project_write_path(project_dir, &path)?;
        write_json_file_and_checkpoint(&path, render_report, write_checkpoint)?;
        expected_render_report_paths.insert(path.clone());
        written_files.push(path.display().to_string());
    }
    let render_index_path = renders_dir.join(SPLIT_RENDER_REPORT_INDEX_FILE_NAME);
    let render_index =
        split_render_report_index_from_project(project_dir, &renders_dir, &project.render_reports);
    write_json_file_and_checkpoint(&render_index_path, &render_index, write_checkpoint)?;
    written_files.push(render_index_path.display().to_string());
    create_dir(&workflow_jobs_dir)?;
    for job in &project.jobs {
        let path = workflow_jobs_dir
            .join(&job.id)
            .join(SPLIT_WORKFLOW_JOB_FILE_NAME);
        validate_split_project_write_path(project_dir, &path)?;
        write_json_file_and_checkpoint(&path, job, write_checkpoint)?;
        expected_workflow_job_paths.insert(path.clone());
        written_files.push(path.display().to_string());
    }
    let workflow_job_index_path = workflow_jobs_dir.join(SPLIT_WORKFLOW_JOB_INDEX_FILE_NAME);
    let workflow_job_index = split_workflow_job_index_from_project(&project.jobs);
    write_json_file_and_checkpoint(
        &workflow_job_index_path,
        &workflow_job_index,
        write_checkpoint,
    )?;
    written_files.push(workflow_job_index_path.display().to_string());
    create_dir(&export_artifacts_dir)?;
    for export_artifact in &project.export_artifacts {
        let path = export_artifacts_dir
            .join(&export_artifact.id)
            .join(SPLIT_EXPORT_ARTIFACT_FILE_NAME);
        validate_split_project_write_path(project_dir, &path)?;
        write_json_file_and_checkpoint(&path, export_artifact, write_checkpoint)?;
        expected_export_artifact_paths.insert(path.clone());
        written_files.push(path.display().to_string());
    }
    let export_artifact_index_path =
        export_artifacts_dir.join(SPLIT_EXPORT_ARTIFACT_INDEX_FILE_NAME);
    let export_artifact_index = split_export_artifact_index_from_project(&project.export_artifacts);
    write_json_file_and_checkpoint(
        &export_artifact_index_path,
        &export_artifact_index,
        write_checkpoint,
    )?;
    written_files.push(export_artifact_index_path.display().to_string());
    rebuild_project_search_index(project_dir, project).map_err(|error| SplitProjectError::Io {
        path: search_index_path.display().to_string(),
        message: error.to_string(),
    })?;
    write_checkpoint(&search_index_path)?;
    written_files.push(search_index_path.display().to_string());
    create_dir(&project_context_dir)?;
    let project_context_path = project_context_dir.join(SPLIT_PROJECT_CONTEXT_FILE_NAME);
    let project_context = split_project_context_from_project(project, &manifest);
    write_json_file_and_checkpoint(&project_context_path, &project_context, write_checkpoint)?;
    written_files.push(project_context_path.display().to_string());
    let mut removed_files =
        remove_stale_transcript_files(&transcripts_dir, &expected_transcript_paths)?;
    removed_files.extend(remove_stale_template_files(
        &templates_dir,
        &expected_template_paths,
    )?);
    removed_files.extend(remove_stale_generated_asset_files(
        &generated_dir,
        &expected_generated_paths,
    )?);
    removed_files.extend(remove_stale_render_report_files(
        &renders_dir,
        &expected_render_report_paths,
    )?);
    removed_files.extend(remove_stale_workflow_job_files(
        &workflow_jobs_dir,
        &expected_workflow_job_paths,
    )?);
    removed_files.extend(remove_stale_export_artifact_files(
        &export_artifacts_dir,
        &expected_export_artifact_paths,
    )?);

    Ok(ProjectWriteReport {
        manifest_path: split_project_manifest_path(project_dir)
            .display()
            .to_string(),
        written_files,
        removed_files,
        recovery_pending: false,
    })
}

pub fn load_split_project(project_dir: &Path) -> Result<VideoProject, SplitProjectError> {
    load_split_project_with_cleanup_hook(project_dir, &mut |_| Ok(()))
}

fn load_split_project_with_cleanup_hook(
    project_dir: &Path,
    cleanup_hook: &mut dyn FnMut(&Path) -> Result<(), SplitProjectError>,
) -> Result<VideoProject, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let _ = recover_pending_split_project_transaction_with_cleanup_hook(project_dir, cleanup_hook)?;
    load_split_project_without_recovery(project_dir)
}

fn load_split_project_without_recovery(
    project_dir: &Path,
) -> Result<VideoProject, SplitProjectError> {
    let manifest_path = split_project_manifest_path(project_dir);
    if !manifest_path.exists()
        && project_dir.join("project.json").exists()
        && project_dir.join("media.json").exists()
    {
        return load_native_palmier_package(project_dir);
    }

    let manifest: SplitProjectManifest = read_json_file(&manifest_path)?;
    let timeline_path = safe_join(project_dir, "files.timeline", &manifest.files.timeline)?;
    let media_path = safe_join(project_dir, "files.media", &manifest.files.media)?;
    let transcripts_dir = safe_join(
        project_dir,
        "files.transcripts",
        &manifest.files.transcripts,
    )?;
    let templates_dir = safe_join(project_dir, "files.templates", &manifest.files.templates)?;
    let generated_dir = safe_join(project_dir, "files.generated", &manifest.files.generated)?;
    let renders_dir = safe_join(project_dir, "files.renders", &manifest.files.renders)?;
    let export_artifacts_dir = project_dir.join(SPLIT_EXPORT_ARTIFACT_INDEX_DIR_NAME);
    let workflow_jobs_dir = project_dir.join(SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME);

    let timeline: SplitTimelineFile = read_json_file(&timeline_path)?;
    let media: SplitMediaIndexFile = read_json_file(&media_path)?;
    let mut transcripts = Vec::new();
    let mut template_overrides = Vec::new();
    let mut generated_assets = Vec::new();
    let mut render_reports = Vec::new();
    let mut export_artifacts = Vec::new();
    let mut jobs = Vec::new();

    if transcripts_dir.exists() {
        let mut transcript_paths = std::fs::read_dir(&transcripts_dir)
            .map_err(|error| SplitProjectError::Io {
                path: transcripts_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: transcripts_dir.display().to_string(),
                message: error.to_string(),
            })?;
        transcript_paths.sort();

        for path in transcript_paths {
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                if is_transcript_index_path(&path) {
                    continue;
                }
                transcripts.push(read_json_file::<SplitTranscriptFile>(&path)?);
            }
        }
    }
    if templates_dir.exists() {
        let mut template_paths = std::fs::read_dir(&templates_dir)
            .map_err(|error| SplitProjectError::Io {
                path: templates_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: templates_dir.display().to_string(),
                message: error.to_string(),
            })?;
        template_paths.sort();

        for path in template_paths {
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                if is_template_index_path(&path) {
                    continue;
                }
                template_overrides.push(read_json_file::<ProjectTemplateOverride>(&path)?);
            }
        }
    }
    if generated_dir.exists() {
        let mut generated_paths = std::fs::read_dir(&generated_dir)
            .map_err(|error| SplitProjectError::Io {
                path: generated_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path().join("asset.json")))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: generated_dir.display().to_string(),
                message: error.to_string(),
            })?;
        generated_paths.sort();

        for path in generated_paths {
            if path.exists() {
                generated_assets.push(read_json_file::<GeneratedAsset>(&path)?);
            }
        }
    }
    if renders_dir.exists() {
        let mut report_paths = std::fs::read_dir(&renders_dir)
            .map_err(|error| SplitProjectError::Io {
                path: renders_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path().join("report.json")))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: renders_dir.display().to_string(),
                message: error.to_string(),
            })?;
        report_paths.sort();

        for path in report_paths {
            if path.exists() {
                render_reports.push(read_json_file::<ProjectRenderReport>(&path)?);
            }
        }
    }
    if export_artifacts_dir.exists() {
        let mut export_artifact_paths = std::fs::read_dir(&export_artifacts_dir)
            .map_err(|error| SplitProjectError::Io {
                path: export_artifacts_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path().join(SPLIT_EXPORT_ARTIFACT_FILE_NAME)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: export_artifacts_dir.display().to_string(),
                message: error.to_string(),
            })?;
        export_artifact_paths.sort();

        for path in export_artifact_paths {
            if path.exists() {
                export_artifacts.push(read_json_file::<ProjectExportArtifact>(&path)?);
            }
        }
    }
    if export_artifacts.is_empty() {
        export_artifacts = manifest.export_artifacts.clone();
    }
    if workflow_jobs_dir.exists() {
        let mut workflow_job_paths = std::fs::read_dir(&workflow_jobs_dir)
            .map_err(|error| SplitProjectError::Io {
                path: workflow_jobs_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path().join(SPLIT_WORKFLOW_JOB_FILE_NAME)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: workflow_jobs_dir.display().to_string(),
                message: error.to_string(),
            })?;
        workflow_job_paths.sort();

        for path in workflow_job_paths {
            if path.exists() {
                jobs.push(read_json_file::<JobSummary>(&path)?);
            }
        }
    }
    if jobs.is_empty() {
        jobs = manifest.jobs.clone();
    }

    Ok(runtime_project_from_split_parts(SplitProjectParts {
        manifest,
        timeline,
        media,
        transcripts,
        template_overrides,
        generated_assets,
        render_reports,
        export_artifacts,
        jobs,
    }))
}

pub fn apply_project_action_to_split_project(
    project_dir: &Path,
    action: ProjectAction,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    apply_project_actions_to_split_project(project_dir, vec![action])
}

pub fn update_project_settings_in_split_project(
    project_dir: &Path,
    name: String,
    render_settings: RenderSettings,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let mut project = load_split_project(project_dir)?;
    apply_project_action(
        &mut project,
        ProjectAction::UpdateProjectSettings {
            name,
            render_settings,
        },
    )
    .map_err(|error| SplitProjectError::ProjectAction {
        message: error.to_string(),
    })?;
    advance_project_content_revision(project_dir, &mut project)?;
    let report =
        save_split_project_metadata_transactionally(project_dir, &project, &mut |_| Ok(()))?;
    Ok(ProjectActionWriteResult { project, report })
}

fn split_project_metadata_transaction_journal_path(
    project_dir: &Path,
) -> Result<PathBuf, SplitProjectError> {
    let parent = project_dir.parent().ok_or_else(|| SplitProjectError::Io {
        path: project_dir.display().to_string(),
        message: "project folder has no parent for metadata transaction".to_string(),
    })?;
    let name = project_dir
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: "project folder has no UTF-8 name".to_string(),
        })?;
    Ok(parent.join(format!(".{name}.metadata-transaction.json")))
}

fn report_relative_paths(
    report: &ProjectWriteReport,
    root: &Path,
) -> Result<BTreeSet<PathBuf>, SplitProjectError> {
    report
        .written_files
        .iter()
        .map(|path| {
            Path::new(path)
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .map_err(|_| SplitProjectError::Io {
                    path: path.clone(),
                    message: "staged metadata path escaped its transaction root".to_string(),
                })
        })
        .collect()
}

fn rollback_split_project_metadata_transaction(
    journal_path: &Path,
    journal: &SplitProjectMetadataTransactionJournal,
) -> Result<(), SplitProjectError> {
    for relative in &journal.relative_paths {
        let target = journal.project_dir.join(relative);
        if target.exists() {
            std::fs::remove_file(&target).map_err(|error| SplitProjectError::Io {
                path: target.display().to_string(),
                message: format!("metadata rollback remove failed: {error}"),
            })?;
        }
        let backup = journal.backup_path.join(relative);
        if backup.exists() {
            if let Some(parent) = target.parent() {
                create_dir(parent)?;
            }
            std::fs::copy(&backup, &target)
                .and_then(|_| std::fs::File::open(&target)?.sync_all())
                .map_err(|error| SplitProjectError::Io {
                    path: target.display().to_string(),
                    message: format!("metadata rollback restore failed: {error}"),
                })?;
        }
    }
    sync_metadata_parent_dirs(&journal.project_dir, &journal.relative_paths)?;
    remove_transaction_path(&journal.staging_path)?;
    remove_transaction_path(&journal.backup_path)?;
    std::fs::remove_file(journal_path).map_err(|error| SplitProjectError::Io {
        path: journal_path.display().to_string(),
        message: error.to_string(),
    })?;
    if let Some(parent) = journal_path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn sync_metadata_parent_dirs(
    project_dir: &Path,
    relative_paths: &[PathBuf],
) -> Result<(), SplitProjectError> {
    let mut directories = BTreeSet::new();
    directories.insert(project_dir.to_path_buf());
    for relative in relative_paths {
        let mut current = project_dir.join(relative).parent().map(Path::to_path_buf);
        while let Some(directory) = current {
            if !directory.starts_with(project_dir) {
                break;
            }
            directories.insert(directory.clone());
            if directory == project_dir {
                break;
            }
            current = directory.parent().map(Path::to_path_buf);
        }
    }
    let mut directories = directories.into_iter().collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        if directory.is_dir() {
            sync_directory(&directory)?;
        }
    }
    Ok(())
}

fn recover_pending_split_project_metadata_transaction(
    project_dir: &Path,
) -> Result<(), SplitProjectError> {
    let journal_path = split_project_metadata_transaction_journal_path(project_dir)?;
    if !journal_path.exists() {
        return Ok(());
    }
    let journal: SplitProjectMetadataTransactionJournal = read_json_file(&journal_path)?;
    if journal.project_dir != project_dir {
        return Err(SplitProjectError::Io {
            path: journal_path.display().to_string(),
            message: "metadata transaction journal targets another project".to_string(),
        });
    }
    let parent = project_dir
        .parent()
        .expect("metadata journal path has a parent");
    for (label, path, prefix) in [
        (
            "staging",
            &journal.staging_path,
            ".video-creater-metadata-stage-",
        ),
        (
            "backup",
            &journal.backup_path,
            ".video-creater-metadata-backup-",
        ),
    ] {
        let safe = path.parent() == Some(parent)
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with(prefix));
        if !safe {
            return Err(SplitProjectError::Io {
                path: journal_path.display().to_string(),
                message: format!("metadata transaction {label} path is unsafe"),
            });
        }
    }
    if journal.relative_paths.iter().any(|path| {
        path.as_os_str().is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
    }) {
        return Err(SplitProjectError::Io {
            path: journal_path.display().to_string(),
            message: "metadata transaction contains an unsafe relative path".to_string(),
        });
    }
    if journal.committed {
        remove_transaction_path(&journal.staging_path)?;
        remove_transaction_path(&journal.backup_path)?;
        std::fs::remove_file(&journal_path).map_err(|error| SplitProjectError::Io {
            path: journal_path.display().to_string(),
            message: error.to_string(),
        })?;
        return Ok(());
    }
    rollback_split_project_metadata_transaction(&journal_path, &journal)
}

fn save_split_project_metadata_transactionally(
    project_dir: &Path,
    project: &VideoProject,
    transaction_hook: &mut dyn FnMut(
        SplitProjectTransactionCheckpoint,
    ) -> Result<(), SplitProjectError>,
) -> Result<ProjectWriteReport, SplitProjectError> {
    validate_project_sidecar_ids(project)?;
    recover_pending_split_project_metadata_transaction(project_dir)?;
    let parent = project_dir.parent().ok_or_else(|| SplitProjectError::Io {
        path: project_dir.display().to_string(),
        message: "project folder has no parent for metadata transaction".to_string(),
    })?;
    let staging = tempfile::Builder::new()
        .prefix(".video-creater-metadata-stage-")
        .tempdir_in(parent)
        .map_err(|error| SplitProjectError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    // The staging directory starts empty, so both staged trees take the file layout from the
    // canonical package; otherwise a custom manifest layout would be reset on every metadata save.
    let staged_report = save_split_project_with_write_checkpoint(
        staging.path(),
        project_dir,
        project,
        &mut |path| transaction_hook(SplitProjectTransactionCheckpoint::Write(path.to_path_buf())),
    )?;
    let new_paths = report_relative_paths(&staged_report, staging.path())?;

    let old_project = load_split_project_without_recovery(project_dir)?;
    let old_stage = tempfile::Builder::new()
        .prefix(".video-creater-metadata-old-")
        .tempdir_in(parent)
        .map_err(|error| SplitProjectError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    let old_report = save_split_project_with_write_checkpoint(
        old_stage.path(),
        project_dir,
        &old_project,
        &mut |_| Ok(()),
    )?;
    let old_paths = report_relative_paths(&old_report, old_stage.path())?;
    let relative_paths = old_paths.union(&new_paths).cloned().collect::<Vec<_>>();

    let backup = tempfile::Builder::new()
        .prefix(".video-creater-metadata-backup-")
        .tempdir_in(parent)
        .map_err(|error| SplitProjectError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    for relative in &relative_paths {
        let target = project_dir.join(relative);
        validate_split_project_write_path(project_dir, &target)?;
        if target.is_file() {
            let backup_path = backup.path().join(relative);
            if let Some(parent) = backup_path.parent() {
                create_dir(parent)?;
            }
            std::fs::copy(&target, &backup_path).map_err(|error| SplitProjectError::Io {
                path: backup_path.display().to_string(),
                message: format!("metadata backup failed: {error}"),
            })?;
            std::fs::File::open(&backup_path)
                .and_then(|file| file.sync_all())
                .map_err(|error| SplitProjectError::Io {
                    path: backup_path.display().to_string(),
                    message: format!("metadata backup sync failed: {error}"),
                })?;
        }
    }
    sync_split_project_tree(staging.path())?;
    sync_split_project_tree(backup.path())?;

    let staging_path = staging.path().to_path_buf();
    let backup_path = backup.path().to_path_buf();
    let journal_path = split_project_metadata_transaction_journal_path(project_dir)?;
    let mut journal = SplitProjectMetadataTransactionJournal {
        project_dir: project_dir.to_path_buf(),
        staging_path: staging_path.clone(),
        backup_path: backup_path.clone(),
        relative_paths,
        committed: false,
    };
    write_json_file(&journal_path, &journal)?;
    let staging_path = staging.keep();
    let backup_path = backup.keep();
    sync_directory(parent)?;

    let apply =
        transaction_hook(SplitProjectTransactionCheckpoint::BeforePromotion).and_then(|()| {
            for relative in &journal.relative_paths {
                let target = project_dir.join(relative);
                if target.exists() {
                    std::fs::remove_file(&target).map_err(|error| SplitProjectError::Io {
                        path: target.display().to_string(),
                        message: format!("metadata replacement remove failed: {error}"),
                    })?;
                }
                let staged = staging_path.join(relative);
                if staged.exists() {
                    if let Some(parent) = target.parent() {
                        create_dir(parent)?;
                    }
                    std::fs::rename(&staged, &target).map_err(|error| SplitProjectError::Io {
                        path: target.display().to_string(),
                        message: format!("metadata promotion failed: {error}"),
                    })?;
                }
            }
            transaction_hook(SplitProjectTransactionCheckpoint::AfterPromotionBeforeParentSync)?;
            sync_metadata_parent_dirs(project_dir, &journal.relative_paths)?;
            sync_directory(parent)?;
            transaction_hook(SplitProjectTransactionCheckpoint::BeforePromotedJournal)?;
            journal.committed = true;
            write_json_file(&journal_path, &journal)?;
            transaction_hook(
                SplitProjectTransactionCheckpoint::AfterMetadataCommittedJournalBeforeParentSync,
            )?;
            sync_directory(parent)
        });
    if let Err(error) = apply {
        let rollback = rollback_split_project_metadata_transaction(&journal_path, &journal);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(SplitProjectError::Io {
                path: project_dir.display().to_string(),
                message: format!(
                    "metadata transaction failed: {error}; rollback failed: {rollback_error}"
                ),
            }),
        };
    }

    let mut report = remap_write_report(staged_report, &staging_path, project_dir);
    report.removed_files = old_paths
        .difference(&new_paths)
        .map(|relative| project_dir.join(relative).display().to_string())
        .collect();
    let cleanup = transaction_hook(SplitProjectTransactionCheckpoint::BeforeCleanup(
        backup_path.clone(),
    ))
    .and_then(|()| recover_pending_split_project_metadata_transaction(project_dir));
    report.recovery_pending = cleanup.is_err();
    Ok(report)
}

fn save_split_project_transactionally(
    project_dir: &Path,
    project: &VideoProject,
    transaction_hook: &mut dyn FnMut(
        SplitProjectTransactionCheckpoint,
    ) -> Result<(), SplitProjectError>,
) -> Result<ProjectWriteReport, SplitProjectError> {
    validate_project_sidecar_ids(project)?;
    let parent = project_dir.parent().ok_or_else(|| SplitProjectError::Io {
        path: project_dir.display().to_string(),
        message: "project folder has no parent for transactional persistence".to_string(),
    })?;
    create_dir(parent)?;
    if !project_dir.exists() {
        create_dir(project_dir)?;
    }
    let recovery = recover_pending_split_project_transaction(project_dir)?;
    let pending_journal_path = split_project_transaction_journal_path(project_dir)?;
    if recovery == SplitProjectRecoveryStatus::CleanupPending || pending_journal_path.exists() {
        return Err(SplitProjectError::Io {
            path: pending_journal_path.display().to_string(),
            message: "prior project transaction cleanup is still pending; reopen the project and retry settings after cleanup succeeds".to_string(),
        });
    }
    let staging = tempfile::Builder::new()
        .prefix(SPLIT_PROJECT_TRANSACTION_STAGE_PREFIX)
        .tempdir_in(parent)
        .map_err(|error| SplitProjectError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    clone_project_tree(project_dir, staging.path())?;
    let staged_report = save_split_project_with_write_checkpoint(
        staging.path(),
        staging.path(),
        project,
        &mut |path| transaction_hook(SplitProjectTransactionCheckpoint::Write(path.to_path_buf())),
    )?;
    sync_split_project_tree(staging.path())?;
    sync_directory(parent)?;

    let backup_slot = tempfile::Builder::new()
        .prefix(SPLIT_PROJECT_TRANSACTION_BACKUP_PREFIX)
        .tempdir_in(parent)
        .map_err(|error| SplitProjectError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    let backup_path = backup_slot.path().to_path_buf();
    backup_slot.close().map_err(|error| SplitProjectError::Io {
        path: backup_path.display().to_string(),
        message: error.to_string(),
    })?;
    let staging_path = staging.path().to_path_buf();
    let journal_path = split_project_transaction_journal_path(project_dir)?;
    let mut journal = SplitProjectTransactionJournal {
        schema_version: SPLIT_PROJECT_TRANSACTION_SCHEMA_VERSION,
        project_dir: project_dir.to_path_buf(),
        staging_path: staging_path.clone(),
        backup_path: backup_path.clone(),
        phase: SplitProjectTransactionPhase::Prepared,
    };
    write_split_project_transaction_journal(&journal_path, &journal)?;
    let staging_path = staging.keep();

    if let Err(error) = std::fs::rename(project_dir, &backup_path) {
        let _ = std::fs::remove_dir_all(&staging_path);
        let _ = std::fs::remove_file(&journal_path);
        let _ = sync_directory(parent);
        return Err(SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: format!("could not start project settings transaction: {error}"),
        });
    }
    sync_directory(parent)?;
    journal.phase = SplitProjectTransactionPhase::OriginalMoved;
    write_split_project_transaction_journal(&journal_path, &journal)?;

    let promotion =
        transaction_hook(SplitProjectTransactionCheckpoint::BeforePromotion).and_then(|()| {
            std::fs::rename(&staging_path, project_dir).map_err(|error| SplitProjectError::Io {
                path: project_dir.display().to_string(),
                message: format!("could not promote staged project settings: {error}"),
            })
        });
    if let Err(promotion_error) = promotion {
        let rollback = transaction_hook(SplitProjectTransactionCheckpoint::BeforeRollback)
            .and_then(|()| {
                std::fs::rename(&backup_path, project_dir).map_err(|error| SplitProjectError::Io {
                    path: project_dir.display().to_string(),
                    message: format!("could not restore original project folder: {error}"),
                })
            })
            .and_then(|()| {
                transaction_hook(
                    SplitProjectTransactionCheckpoint::AfterRestoreOriginalBeforeParentSync,
                )
            })
            .and_then(|()| sync_directory(parent));
        if rollback.is_ok() {
            let _ = std::fs::remove_dir_all(&staging_path);
            let _ = std::fs::remove_file(&journal_path);
            let _ = sync_directory(parent);
        }
        return Err(SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: match rollback {
                Ok(()) => format!(
                    "could not commit project settings transaction: {promotion_error}"
                ),
                Err(rollback_error) => format!(
                    "could not commit project settings transaction: {promotion_error}; rollback failed and open-time recovery is required: {rollback_error}"
                ),
            },
        });
    }
    let post_promotion =
        transaction_hook(SplitProjectTransactionCheckpoint::AfterPromotionBeforeParentSync)
            .and_then(|()| sync_directory(parent))
            .and_then(|()| {
                transaction_hook(SplitProjectTransactionCheckpoint::BeforePromotedJournal)
            })
            .and_then(|()| {
                journal.phase = SplitProjectTransactionPhase::Promoted;
                write_split_project_transaction_journal(&journal_path, &journal)
            });
    if let Err(post_promotion_error) = post_promotion {
        let park_promoted = transaction_hook(SplitProjectTransactionCheckpoint::BeforeRollback)
            .and_then(|()| {
                std::fs::rename(project_dir, &staging_path).map_err(|error| SplitProjectError::Io {
                    path: project_dir.display().to_string(),
                    message: format!(
                        "could not park promoted project during durability rollback: {error}"
                    ),
                })
            });
        if let Err(rollback_error) = park_promoted {
            let recovery = recover_pending_split_project_transaction_with_cleanup_hook(
                project_dir,
                &mut |path| {
                    transaction_hook(SplitProjectTransactionCheckpoint::BeforeCleanup(
                        path.to_path_buf(),
                    ))
                },
            )?;
            if split_project_transaction_candidate_is_complete(project_dir) {
                let mut report = remap_write_report(staged_report, &staging_path, project_dir);
                report.recovery_pending = recovery == SplitProjectRecoveryStatus::CleanupPending;
                return Ok(report);
            }
            return Err(SplitProjectError::Io {
                path: project_dir.display().to_string(),
                message: format!(
                    "post-promotion durability failed: {post_promotion_error}; promoted rollback failed: {rollback_error}"
                ),
            });
        }

        let restore_original = transaction_hook(
            SplitProjectTransactionCheckpoint::BeforeRestoreOriginal(backup_path.clone()),
        )
        .and_then(|()| {
            std::fs::rename(&backup_path, project_dir).map_err(|error| SplitProjectError::Io {
                path: project_dir.display().to_string(),
                message: format!("could not restore original project after promotion: {error}"),
            })
        });
        if let Err(restore_error) = restore_original {
            let recovery_will_finalize_promoted_candidate =
                !split_project_transaction_candidate_is_complete(&backup_path)
                    && split_project_transaction_candidate_is_complete(&staging_path);
            let recovery = recover_pending_split_project_transaction(project_dir)?;
            if recovery_will_finalize_promoted_candidate {
                let mut report = remap_write_report(staged_report, &staging_path, project_dir);
                report.recovery_pending = recovery == SplitProjectRecoveryStatus::CleanupPending;
                return Ok(report);
            }
            return Err(SplitProjectError::Io {
                path: project_dir.display().to_string(),
                message: format!(
                    "project settings were not committed because post-promotion durability failed: {post_promotion_error}; original restore failed: {restore_error}"
                ),
            });
        } else {
            let _ = sync_directory(parent);
            let _ = recover_pending_split_project_transaction(project_dir);
        }
        return Err(SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: format!(
                "project settings were not committed because post-promotion durability failed: {post_promotion_error}"
            ),
        });
    }
    let recovery =
        recover_pending_split_project_transaction_with_cleanup_hook(project_dir, &mut |path| {
            transaction_hook(SplitProjectTransactionCheckpoint::BeforeCleanup(
                path.to_path_buf(),
            ))
        })?;
    let mut report = remap_write_report(staged_report, &staging_path, project_dir);
    report.recovery_pending = recovery == SplitProjectRecoveryStatus::CleanupPending;
    Ok(report)
}

fn clone_project_tree(source: &Path, destination: &Path) -> Result<(), SplitProjectError> {
    let mut entries = std::fs::read_dir(source)
        .map_err(|error| SplitProjectError::Io {
            path: source.display().to_string(),
            message: error.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: source.display().to_string(),
            message: error.to_string(),
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| SplitProjectError::Io {
            path: source_path.display().to_string(),
            message: error.to_string(),
        })?;
        if file_type.is_dir() {
            create_dir(&destination_path)?;
            clone_project_tree(&source_path, &destination_path)?;
        } else if file_type.is_symlink() {
            clone_project_symlink(&source_path, &destination_path)?;
        } else {
            clone_project_file(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn clone_project_file(source: &Path, destination: &Path) -> Result<(), SplitProjectError> {
    let is_json = source
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    let result = if is_json {
        std::fs::copy(source, destination).map(|_| ())
    } else {
        std::fs::hard_link(source, destination)
            .or_else(|_| std::fs::copy(source, destination).map(|_| ()))
    };
    result.map_err(|error| SplitProjectError::Io {
        path: destination.display().to_string(),
        message: error.to_string(),
    })
}

fn clone_project_symlink(source: &Path, destination: &Path) -> Result<(), SplitProjectError> {
    let target_metadata = std::fs::metadata(source).map_err(|error| SplitProjectError::Io {
        path: source.display().to_string(),
        message: error.to_string(),
    })?;
    if target_metadata.is_dir() {
        return Err(SplitProjectError::UnsafeProjectSymlink {
            path: source.display().to_string(),
        });
    }
    std::fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| SplitProjectError::Io {
            path: destination.display().to_string(),
            message: error.to_string(),
        })
}

fn split_project_transaction_journal_path(
    project_dir: &Path,
) -> Result<PathBuf, SplitProjectError> {
    let parent = project_dir.parent().ok_or_else(|| SplitProjectError::Io {
        path: project_dir.display().to_string(),
        message: "project folder has no parent for transaction recovery".to_string(),
    })?;
    let digest = Sha256::digest(project_dir.as_os_str().to_string_lossy().as_bytes());
    let digest = format!("{digest:x}");
    Ok(parent.join(format!(
        ".video-creater-project-transaction-{}.json",
        &digest[..24]
    )))
}

fn write_split_project_transaction_journal(
    journal_path: &Path,
    journal: &SplitProjectTransactionJournal,
) -> Result<(), SplitProjectError> {
    write_json_file(journal_path, journal)?;
    sync_directory(journal_path.parent().ok_or_else(|| SplitProjectError::Io {
        path: journal_path.display().to_string(),
        message: "transaction journal has no parent".to_string(),
    })?)
}

fn recover_pending_split_project_transaction(
    project_dir: &Path,
) -> Result<SplitProjectRecoveryStatus, SplitProjectError> {
    recover_pending_split_project_transaction_with_cleanup_hook(project_dir, &mut |_| Ok(()))
}

fn recover_pending_split_project_transaction_with_cleanup_hook(
    project_dir: &Path,
    cleanup_hook: &mut dyn FnMut(&Path) -> Result<(), SplitProjectError>,
) -> Result<SplitProjectRecoveryStatus, SplitProjectError> {
    recover_pending_split_project_metadata_transaction(project_dir)?;
    let journal_path = split_project_transaction_journal_path(project_dir)?;
    if !journal_path.exists() {
        return Ok(SplitProjectRecoveryStatus::Clean);
    }
    let journal: SplitProjectTransactionJournal = read_json_file(&journal_path)?;
    validate_split_project_transaction_journal(project_dir, &journal_path, &journal)?;
    let parent = project_dir.parent().expect("validated transaction parent");

    let canonical_complete = split_project_transaction_candidate_is_complete(project_dir);
    let recovery_candidate = match journal.phase {
        SplitProjectTransactionPhase::Prepared
        | SplitProjectTransactionPhase::OriginalMoved
        | SplitProjectTransactionPhase::Promoted
            if !canonical_complete =>
        {
            [
                journal.backup_path.as_path(),
                journal.staging_path.as_path(),
            ]
            .into_iter()
            .find(|candidate| split_project_transaction_candidate_is_complete(candidate))
        }
        _ => None,
    };
    if !canonical_complete {
        let candidate = recovery_candidate.ok_or_else(|| SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: format!(
                "transaction journal phase {:?} has no complete canonical, backup, or staged project",
                journal.phase
            ),
        })?;
        remove_transaction_path(project_dir)?;
        std::fs::rename(candidate, project_dir).map_err(|error| SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: format!("open-time project transaction recovery failed: {error}"),
        })?;
        if sync_directory(parent).is_err() {
            return Ok(SplitProjectRecoveryStatus::CleanupPending);
        }
    }

    if sync_directory(parent).is_err() {
        return Ok(SplitProjectRecoveryStatus::CleanupPending);
    }

    for cleanup_path in [&journal.staging_path, &journal.backup_path] {
        if cleanup_hook(cleanup_path).is_err() || remove_transaction_path(cleanup_path).is_err() {
            return Ok(SplitProjectRecoveryStatus::CleanupPending);
        }
    }
    if sync_directory(parent).is_err() || cleanup_hook(&journal_path).is_err() {
        return Ok(SplitProjectRecoveryStatus::CleanupPending);
    }
    if std::fs::remove_file(&journal_path).is_err() {
        return Ok(SplitProjectRecoveryStatus::CleanupPending);
    }
    let _ = sync_directory(parent);
    Ok(SplitProjectRecoveryStatus::Clean)
}

fn split_project_transaction_candidate_is_complete(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && load_split_project_without_recovery(path).is_ok()
    })
}

fn validate_split_project_transaction_journal(
    project_dir: &Path,
    journal_path: &Path,
    journal: &SplitProjectTransactionJournal,
) -> Result<(), SplitProjectError> {
    let parent = project_dir.parent();
    let valid_sibling = |path: &Path, prefix: &str| {
        path.parent() == parent
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
    };
    if journal.schema_version != SPLIT_PROJECT_TRANSACTION_SCHEMA_VERSION
        || journal.project_dir != project_dir
        || !valid_sibling(
            &journal.staging_path,
            SPLIT_PROJECT_TRANSACTION_STAGE_PREFIX,
        )
        || !valid_sibling(
            &journal.backup_path,
            SPLIT_PROJECT_TRANSACTION_BACKUP_PREFIX,
        )
    {
        return Err(SplitProjectError::Json {
            path: journal_path.display().to_string(),
            message: "transaction journal paths or schema are invalid".to_string(),
        });
    }
    Ok(())
}

fn remove_transaction_path(path: &Path) -> Result<(), SplitProjectError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(SplitProjectError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            })
        }
    };
    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn sync_split_project_tree(path: &Path) -> Result<(), SplitProjectError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(SplitProjectError::UnsafeProjectSymlink {
            path: path.display().to_string(),
        });
    }
    if metadata.is_file() {
        return std::fs::File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|error| SplitProjectError::Io {
                path: path.display().to_string(),
                message: format!("staged file sync failed: {error}"),
            });
    }

    let entries = std::fs::read_dir(path)
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    for entry in entries {
        sync_split_project_tree(&entry.path())?;
    }
    sync_directory(path)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), SplitProjectError> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: format!("directory sync failed: {error}"),
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), SplitProjectError> {
    Ok(())
}

fn remap_write_report(
    report: ProjectWriteReport,
    staging_path: &Path,
    project_dir: &Path,
) -> ProjectWriteReport {
    let remap = |value: String| {
        Path::new(&value)
            .strip_prefix(staging_path)
            .map(|relative| project_dir.join(relative).display().to_string())
            .unwrap_or(value)
    };
    ProjectWriteReport {
        manifest_path: remap(report.manifest_path),
        written_files: report.written_files.into_iter().map(remap).collect(),
        removed_files: report.removed_files.into_iter().map(remap).collect(),
        recovery_pending: report.recovery_pending,
    }
}

#[cfg(test)]
fn update_project_settings_in_split_project_with_transaction_hook(
    project_dir: &Path,
    name: String,
    render_settings: RenderSettings,
    transaction_hook: &mut dyn FnMut(
        SplitProjectTransactionCheckpoint,
    ) -> Result<(), SplitProjectError>,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let mut project = load_split_project(project_dir)?;
    apply_project_action(
        &mut project,
        ProjectAction::UpdateProjectSettings {
            name,
            render_settings,
        },
    )
    .map_err(|error| SplitProjectError::ProjectAction {
        message: error.to_string(),
    })?;
    advance_project_content_revision(project_dir, &mut project)?;
    let report = save_split_project_transactionally(project_dir, &project, transaction_hook)?;
    Ok(ProjectActionWriteResult { project, report })
}

pub fn apply_project_actions_to_split_project(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let lease = split_project_mutation_lease(project_dir)?;
    apply_project_actions_to_split_project_with_lease(project_dir, actions, &lease)
}

#[cfg(test)]
fn apply_project_actions_to_split_project_with_transaction_hook(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    transaction_hook: &mut dyn FnMut(
        SplitProjectTransactionCheckpoint,
    ) -> Result<(), SplitProjectError>,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let lease = split_project_mutation_lease(project_dir)?;
    apply_project_actions_to_split_project_with_lease_and_transaction_hook(
        project_dir,
        actions,
        &lease,
        transaction_hook,
    )
}

pub(crate) fn apply_project_actions_to_split_project_with_lease(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    _lease: &SplitProjectMutationLease,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let mut project = load_split_project(project_dir)?;
    for action in actions {
        apply_project_action(&mut project, action).map_err(|error| {
            SplitProjectError::ProjectAction {
                message: error.to_string(),
            }
        })?;
    }
    advance_project_content_revision(project_dir, &mut project)?;
    let report =
        save_split_project_metadata_transactionally(project_dir, &project, &mut |_| Ok(()))?;
    Ok(ProjectActionWriteResult { project, report })
}

#[cfg(test)]
fn apply_project_actions_to_split_project_with_lease_and_transaction_hook(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    _lease: &SplitProjectMutationLease,
    transaction_hook: &mut dyn FnMut(
        SplitProjectTransactionCheckpoint,
    ) -> Result<(), SplitProjectError>,
) -> Result<ProjectActionWriteResult, SplitProjectError> {
    let mut project = load_split_project(project_dir)?;
    for action in actions {
        apply_project_action(&mut project, action).map_err(|error| {
            SplitProjectError::ProjectAction {
                message: error.to_string(),
            }
        })?;
    }
    advance_project_content_revision(project_dir, &mut project)?;
    let report = save_split_project_transactionally(project_dir, &project, transaction_hook)?;

    Ok(ProjectActionWriteResult { project, report })
}

pub fn record_agent_project_action_batch(
    project_dir: &Path,
    before: VideoProject,
    after: VideoProject,
    action_count: usize,
) -> Result<ProjectAgentHistoryWriteResult, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let mut history = read_agent_edit_history(project_dir)?;
    let latest_entry_id =
        agent_batch::agent_edit_entry_id(&history.entries, after.content_revision);
    // A hashed entry, like conversation applies: a legacy full `after` snapshot would make the
    // background progress of the batch's generations block Undo.
    history.entries.push(agent_batch::batch_history_entry(
        latest_entry_id.clone(),
        &before,
        &after,
        action_count,
        Vec::new(),
        None,
    )?);
    agent_history_snapshot::retain_agent_edit_history_budget(
        &mut history.entries,
        MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES,
    );
    let path = write_agent_edit_history(project_dir, &history)?;

    Ok(ProjectAgentHistoryWriteResult {
        path: path.display().to_string(),
        entry_count: history.entries.len(),
        latest_entry_id,
    })
}

#[derive(Debug, Clone)]
pub struct AppServerConversationTurn {
    pub turn_id: Option<String>,
    pub turn_status: Option<String>,
    /// Denormalized prompt used for the session title and history display.
    pub prompt: String,
    /// Submission timestamp used for session and turn timestamps.
    pub created_at: String,
    /// Forward-compatible request JSON (legacy edit job or conversation request).
    pub request: Value,
    pub thread_response: Value,
    pub turn_response: Value,
    pub has_proposal: bool,
    /// Which agent produced this turn, when it was not the Codex app-server.
    pub provider: Option<String>,
    /// The provider session this turn belongs to, when the backend has one of its own.
    pub provider_session_id: Option<String>,
}

pub fn record_app_server_conversation_turn(
    project_dir: &Path,
    project_id: &str,
    thread_id: &str,
    turn: AppServerConversationTurn,
) -> Result<ProjectAppServerConversationWriteResult, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let AppServerConversationTurn {
        turn_id,
        turn_status,
        prompt,
        created_at,
        request,
        thread_response,
        turn_response,
        has_proposal,
        provider,
        provider_session_id,
    } = turn;
    // Resolve (or create) the session first so the history entry can name it. The history
    // file is still written before the session manifest.
    let mut sessions = read_agent_session_manifest(project_dir, project_id)?;
    // A turn that carries a provider session handle names its chat exactly, so that match
    // comes first: two backends can share a project and their handles never collide.
    let session_index = provider_session_id
        .as_deref()
        .and_then(|handle| {
            sessions
                .sessions
                .iter()
                .position(|session| session.provider_session_id.as_deref() == Some(handle))
        })
        .or_else(|| {
            sessions
                .sessions
                .iter()
                .position(|session| session.thread_id.as_deref() == Some(thread_id))
        })
        .or_else(|| {
            sessions.active_session_id.as_deref().and_then(|active_id| {
                sessions
                    .sessions
                    .iter()
                    .position(|session| session.id == active_id)
            })
        });
    let session_index = if let Some(index) = session_index {
        index
    } else {
        let id = format!(
            "agent-session-{}",
            sessions.sessions.len() + sessions.deleted_sessions.len() + 1
        );
        sessions.sessions.push(SplitAgentSession {
            id: id.clone(),
            title: prompt.chars().take(48).collect(),
            thread_id: Some(thread_id.to_string()),
            provider: provider.clone(),
            provider_session_id: provider_session_id.clone(),
            created_at: created_at.clone(),
            updated_at: created_at.clone(),
            turns: Vec::new(),
            proposal_status: "none".to_string(),
            applied_action_ids: Vec::new(),
        });
        sessions.active_session_id = Some(id);
        sessions.sessions.len() - 1
    };
    let mut history = read_app_server_conversation_history(project_dir)?;
    let latest_entry_id = format!("app-server-turn-{}", history.entries.len() + 1);
    history.entries.push(SplitAppServerConversationEntry {
        id: latest_entry_id.clone(),
        project_id: project_id.to_string(),
        thread_id: thread_id.to_string(),
        turn_id: turn_id.clone(),
        turn_status: turn_status.clone(),
        prompt: prompt.clone(),
        request: request.clone(),
        has_proposal,
        thread_response: thread_response.clone(),
        turn_response: turn_response.clone(),
        session_id: Some(sessions.sessions[session_index].id.clone()),
    });
    if history.entries.len() > MAX_APP_SERVER_CONVERSATION_ENTRIES {
        let excess = history.entries.len() - MAX_APP_SERVER_CONVERSATION_ENTRIES;
        history.entries.drain(0..excess);
    }
    let path = write_app_server_conversation_history(project_dir, &history)?;
    let proposal_status = if has_proposal { "proposed" } else { "none" }.to_string();
    let tool_results = turn_response
        .get("toolResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let session = &mut sessions.sessions[session_index];
    session.thread_id = Some(thread_id.to_string());
    // Record which agent produced the turn, so history never lies about it — including when a
    // chat that started on one backend continues on another.
    if provider.is_some() {
        session.provider = provider;
    }
    if provider_session_id.is_some() {
        session.provider_session_id = provider_session_id;
    }
    session.updated_at = created_at.clone();
    session.proposal_status = proposal_status.clone();
    session.turns.push(SplitAgentSessionTurn {
        id: turn_id.clone().unwrap_or_else(|| latest_entry_id.clone()),
        created_at,
        full_turn: serde_json::json!({
            "request": request,
            "threadResponse": thread_response,
            "turnResponse": turn_response,
            "turnStatus": turn_status,
        }),
        tool_results,
        proposal_status,
        applied_action_ids: Vec::new(),
    });
    write_agent_session_manifest(project_dir, &sessions)?;

    Ok(ProjectAppServerConversationWriteResult {
        path: path.display().to_string(),
        entry_count: history.entries.len(),
        latest_entry_id,
    })
}

pub fn load_app_server_conversation_history(
    project_dir: &Path,
) -> Result<SplitAppServerConversationFile, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    read_app_server_conversation_history(project_dir)
}

/// The Codex thread the project's next agent turn continues.
///
/// Each chat owns its own thread, so the active chat's thread is resumed and a chat without one
/// (a chat the user just created) starts a fresh thread. A project saved before chats existed has
/// no chat manifest, so it keeps resuming `project.codex_thread_id`; a thread whose only chat was
/// deleted is not resumed.
pub fn codex_thread_for_next_turn(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<Option<String>, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let manifest = read_agent_session_manifest(project_dir, &project.id)?;
    Ok(codex_thread_for_active_session(
        &manifest,
        project.codex_thread_id.as_deref(),
    ))
}

fn codex_thread_for_active_session(
    manifest: &SplitAgentSessionManifest,
    project_thread_id: Option<&str>,
) -> Option<String> {
    let active = manifest.active_session_id.as_deref().and_then(|active_id| {
        manifest
            .sessions
            .iter()
            .find(|session| session.id == active_id)
    });
    if let Some(active) = active {
        return active.thread_id.clone();
    }
    let project_thread_id = project_thread_id?;
    let owned_by_deleted_chat = manifest
        .deleted_sessions
        .iter()
        .any(|session| session.thread_id.as_deref() == Some(project_thread_id));
    (!owned_by_deleted_chat).then(|| project_thread_id.to_string())
}

pub fn load_agent_session_manifest(
    project_dir: &Path,
) -> Result<SplitAgentSessionManifest, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let project = load_split_project(project_dir)?;
    read_agent_session_manifest(project_dir, &project.id)
}

pub fn apply_agent_session_action(
    project_dir: &Path,
    project_id: &str,
    action: AgentSessionAction,
) -> Result<SplitAgentSessionManifest, SplitProjectError> {
    apply_agent_session_action_with_hook(project_dir, project_id, action, &mut || {})
}

fn apply_agent_session_action_with_hook(
    project_dir: &Path,
    project_id: &str,
    action: AgentSessionAction,
    after_project_load: &mut dyn FnMut(),
) -> Result<SplitAgentSessionManifest, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let project = load_split_project(project_dir)?;
    after_project_load();
    if project.id != project_id {
        return Err(SplitProjectError::ProjectAction {
            message: "agent session project id does not match the split project".to_string(),
        });
    }
    let mut manifest = read_agent_session_manifest(project_dir, project_id)?;
    match action {
        AgentSessionAction::Create {
            id,
            title,
            thread_id,
            timestamp,
        } => {
            if id.trim().is_empty()
                || title.trim().is_empty()
                || manifest
                    .sessions
                    .iter()
                    .chain(&manifest.deleted_sessions)
                    .any(|session| session.id == id)
            {
                return Err(SplitProjectError::ProjectAction {
                    message: "agent session id/title is invalid or already exists".to_string(),
                });
            }
            manifest.sessions.push(SplitAgentSession {
                id: id.clone(),
                title: title.trim().to_string(),
                thread_id,
                provider: None,
                provider_session_id: None,
                created_at: timestamp.clone(),
                updated_at: timestamp,
                turns: Vec::new(),
                proposal_status: "none".to_string(),
                applied_action_ids: Vec::new(),
            });
            manifest.active_session_id = Some(id);
        }
        AgentSessionAction::Select { session_id } => {
            if !manifest
                .sessions
                .iter()
                .any(|session| session.id == session_id)
            {
                return Err(SplitProjectError::ProjectAction {
                    message: "agent session was not found".to_string(),
                });
            }
            manifest.active_session_id = Some(session_id);
        }
        AgentSessionAction::Rename {
            session_id,
            title,
            timestamp,
        } => {
            let session = manifest
                .sessions
                .iter_mut()
                .find(|session| session.id == session_id)
                .ok_or_else(|| SplitProjectError::ProjectAction {
                    message: "agent session was not found".to_string(),
                })?;
            if title.trim().is_empty() {
                return Err(SplitProjectError::ProjectAction {
                    message: "agent session title cannot be empty".to_string(),
                });
            }
            session.title = title.trim().to_string();
            session.updated_at = timestamp;
        }
        AgentSessionAction::Delete {
            session_id,
            timestamp,
        } => {
            let index = manifest
                .sessions
                .iter()
                .position(|session| session.id == session_id)
                .ok_or_else(|| SplitProjectError::ProjectAction {
                    message: "agent session was not found".to_string(),
                })?;
            let mut session = manifest.sessions.remove(index);
            session.updated_at = timestamp;
            manifest.deleted_sessions.push(session);
            if manifest.active_session_id.as_deref() == Some(session_id.as_str()) {
                manifest.active_session_id =
                    manifest.sessions.first().map(|session| session.id.clone());
            }
        }
        AgentSessionAction::Restore {
            session_id,
            timestamp,
        } => {
            let index = manifest
                .deleted_sessions
                .iter()
                .position(|session| session.id == session_id)
                .ok_or_else(|| SplitProjectError::ProjectAction {
                    message: "deleted agent session was not found".to_string(),
                })?;
            let mut session = manifest.deleted_sessions.remove(index);
            session.updated_at = timestamp;
            manifest.active_session_id = Some(session.id.clone());
            manifest.sessions.push(session);
        }
    }
    write_agent_session_manifest(project_dir, &manifest)?;
    Ok(manifest)
}

pub fn validate_split_project(
    project_dir: &Path,
) -> Result<ProjectValidationReport, SplitProjectError> {
    let mut issues = Vec::new();
    let manifest_path = split_project_manifest_path(project_dir);

    if !manifest_path.exists() {
        issues.push(issue(
            super::storage::PROJECT_FILE_NAME,
            "manifest file is missing",
            "Create video-creater.project.json before opening the split project.",
        ));
        return Ok(ProjectValidationReport { ok: false, issues });
    }

    let manifest: SplitProjectManifest = match read_json_file(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            issues.push(issue(
                super::storage::PROJECT_FILE_NAME,
                error.to_string(),
                "Repair the project manifest JSON.",
            ));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };

    validate_workflow_job_files(project_dir, &manifest, &mut issues)?;

    let timeline_path = match safe_join(project_dir, "files.timeline", &manifest.files.timeline) {
        Ok(path) => path,
        Err(error) => {
            issues.push(issue(
                "video-creater.project.json files.timeline",
                error.to_string(),
                "Use a project-relative timeline path.",
            ));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };
    let media_path = match safe_join(project_dir, "files.media", &manifest.files.media) {
        Ok(path) => path,
        Err(error) => {
            issues.push(issue(
                "video-creater.project.json files.media",
                error.to_string(),
                "Use a project-relative media index path.",
            ));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };

    if !timeline_path.exists() {
        issues.push(issue(
            &manifest.files.timeline,
            "timeline file is missing",
            "Create timeline.json or update the manifest files.timeline path.",
        ));
    }
    if !media_path.exists() {
        issues.push(issue(
            &manifest.files.media,
            "media index file is missing",
            "Create media/index.json or update the manifest files.media path.",
        ));
    }
    if !issues.is_empty() {
        return Ok(ProjectValidationReport { ok: false, issues });
    }

    let timeline: SplitTimelineFile = match read_json_file(&timeline_path) {
        Ok(timeline) => timeline,
        Err(error) => {
            issues.push(issue(
                &manifest.files.timeline,
                error.to_string(),
                "Repair timeline JSON.",
            ));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };
    let media: SplitMediaIndexFile = match read_json_file(&media_path) {
        Ok(media) => media,
        Err(error) => {
            issues.push(issue(
                &manifest.files.media,
                error.to_string(),
                "Repair media index JSON.",
            ));
            return Ok(ProjectValidationReport { ok: false, issues });
        }
    };

    let generated_assets = collect_generated_asset_summaries(project_dir, &manifest)?;

    validate_media_index(project_dir, &media, &mut issues);
    validate_timeline(&timeline, &media, &generated_assets, &mut issues);
    validate_timeline_library(&timeline, &mut issues);
    validate_transcript_files(project_dir, &manifest, &media, &mut issues)?;
    validate_template_override_files(project_dir, &manifest, &mut issues)?;
    validate_generated_asset_files(project_dir, &manifest, &media, &mut issues)?;
    validate_generated_asset_index_file(project_dir, &manifest, &mut issues)?;
    validate_render_report_files(project_dir, &manifest, &mut issues)?;
    validate_export_artifact_files(project_dir, &manifest, &mut issues)?;

    Ok(ProjectValidationReport {
        ok: issues.is_empty(),
        issues,
    })
}

fn validate_manifest_jobs(
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let mut job_ids = BTreeSet::new();

    for (index, job) in manifest.jobs.iter().enumerate() {
        let base_path = format!("video-creater.project.json jobs[{index}]");
        validate_job_summary(&base_path, job, issues);
        if !job.id.trim().is_empty()
            && is_safe_manifest_segment(&job.id)
            && !job_ids.insert(job.id.clone())
        {
            issues.push(issue(
                format!("{base_path}.id"),
                format!("duplicate job id `{}`", job.id),
                "Use unique job ids in video-creater.project.json.",
            ));
        }
    }
}

fn validate_workflow_job_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let workflow_jobs_dir = project_dir.join(SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME);
    if !workflow_jobs_dir.exists() {
        validate_manifest_jobs(manifest, issues);
        return Ok(());
    }

    let mut job_paths = std::fs::read_dir(&workflow_jobs_dir)
        .map_err(|error| SplitProjectError::Io {
            path: workflow_jobs_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path().join(SPLIT_WORKFLOW_JOB_FILE_NAME)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: workflow_jobs_dir.display().to_string(),
            message: error.to_string(),
        })?;
    job_paths.sort();

    if !job_paths.iter().any(|path| path.exists()) {
        validate_manifest_jobs(manifest, issues);
        return Ok(());
    }

    let mut job_ids = BTreeSet::new();
    let mut sidecar_jobs = BTreeMap::new();
    for path in job_paths {
        if !path.exists() {
            continue;
        }
        let relative_path = project_relative_path(project_dir, &path);
        let job: JobSummary = match read_json_file(&path) {
            Ok(job) => job,
            Err(error) => {
                issues.push(issue(
                    relative_path,
                    error.to_string(),
                    "Repair workflow job sidecar JSON.",
                ));
                continue;
            }
        };

        validate_job_summary(&relative_path, &job, issues);
        if !job.id.trim().is_empty() {
            sidecar_jobs.insert(job.id.clone(), job.clone());
        }
        if path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            != Some(job.id.as_str())
        {
            issues.push(issue(
                format!("{relative_path} id"),
                "job id must match containing directory",
                "Rename the job directory or update the job id.",
            ));
        }
        if !job.id.trim().is_empty()
            && is_safe_manifest_segment(&job.id)
            && !job_ids.insert(job.id.clone())
        {
            issues.push(issue(
                format!("{relative_path} id"),
                format!("duplicate job id `{}`", job.id),
                "Use unique workflow job ids.",
            ));
        }
    }

    validate_workflow_job_index_file(project_dir, &sidecar_jobs, issues)?;

    Ok(())
}

fn validate_workflow_job_index_file(
    project_dir: &Path,
    sidecar_jobs: &BTreeMap<String, JobSummary>,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let workflow_jobs_dir = project_dir.join(SPLIT_WORKFLOW_JOB_INDEX_DIR_NAME);
    let index_path = workflow_jobs_dir.join(SPLIT_WORKFLOW_JOB_INDEX_FILE_NAME);
    if !index_path.exists() {
        return Ok(());
    }

    let index: SplitWorkflowJobIndexFile = match read_json_file(&index_path) {
        Ok(index) => index,
        Err(error) => {
            issues.push(issue(
                "jobs/index.json",
                error.to_string(),
                "Repair jobs/index.json or regenerate it from workflow job sidecars.",
            ));
            return Ok(());
        }
    };
    let mut indexed_job_ids = BTreeSet::new();

    for entry in &index.jobs {
        let entry_path = workflow_job_index_entry_path(&entry.job_id);
        if entry.job_id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.jobId"),
                "workflow job index jobId is required",
                "Regenerate jobs/index.json from workflow job sidecars.",
            ));
            continue;
        }
        if !indexed_job_ids.insert(entry.job_id.clone()) {
            issues.push(issue(
                format!("{entry_path}.jobId"),
                format!("duplicate workflow job index jobId `{}`", entry.job_id),
                "Regenerate jobs/index.json from workflow job sidecars.",
            ));
        }

        let Some(job) = sidecar_jobs.get(&entry.job_id) else {
            issues.push(issue(
                format!("{entry_path}.jobId"),
                format!(
                    "workflow job index entry references missing job sidecar `{}`",
                    entry.job_id
                ),
                "Regenerate jobs/index.json from workflow job sidecars.",
            ));
            continue;
        };

        let workflow = job.workflow.as_ref();
        let start_request = job.start_request.as_ref();
        validate_workflow_job_index_field(&entry_path, "kind", &entry.kind, &job.kind, issues);
        validate_workflow_job_index_field(
            &entry_path,
            "status",
            &entry.status,
            job_status_label(&job.status),
            issues,
        );
        validate_workflow_job_index_field(
            &entry_path,
            "updatedAt",
            &entry.updated_at,
            &job.updated_at,
            issues,
        );
        validate_workflow_job_index_optional_field(
            &entry_path,
            "workflowId",
            entry.workflow_id.as_deref(),
            workflow.map(|workflow| workflow.workflow_id.as_str()),
            issues,
        );
        validate_workflow_job_index_optional_field(
            &entry_path,
            "workflowType",
            entry.workflow_type.as_deref(),
            workflow.map(|workflow| workflow.workflow_type.as_str()),
            issues,
        );
        validate_workflow_job_index_optional_field(
            &entry_path,
            "taskQueue",
            entry.task_queue.as_deref(),
            workflow.map(|workflow| workflow.task_queue.as_str()),
            issues,
        );
        validate_workflow_job_index_optional_field(
            &entry_path,
            "runId",
            entry.run_id.as_deref(),
            workflow.and_then(|workflow| workflow.run_id.as_deref()),
            issues,
        );
        let expected_activity_count = workflow
            .map(|workflow| workflow.activity_types.len())
            .unwrap_or(0);
        if entry.activity_count != expected_activity_count {
            issues.push(issue(
                format!("{entry_path}.activityCount"),
                format!(
                    "workflow job index activityCount `{}` must match job sidecar activity count `{expected_activity_count}`",
                    entry.activity_count
                ),
                "Regenerate jobs/index.json from workflow job sidecars.",
            ));
        }
        validate_workflow_job_index_bool_field(
            &entry_path,
            "hasStartRequest",
            entry.has_start_request,
            start_request.is_some(),
            issues,
        );
        validate_workflow_job_index_bool_field(
            &entry_path,
            "startRequestReady",
            entry.start_request_ready,
            workflow
                .zip(start_request)
                .is_some_and(|(workflow, start_request)| {
                    workflow_start_request_ready(workflow, start_request)
                }),
            issues,
        );
        validate_workflow_job_index_optional_field(
            &entry_path,
            "idReusePolicy",
            entry.id_reuse_policy.as_deref(),
            start_request.map(|request| request.id_reuse_policy.as_str()),
            issues,
        );
    }

    for job_id in sidecar_jobs.keys() {
        if !indexed_job_ids.contains(job_id) {
            issues.push(issue(
                format!("jobs/index.json jobs[{job_id}]"),
                format!("missing workflow job index entry for `{job_id}`"),
                "Regenerate jobs/index.json from workflow job sidecars.",
            ));
        }
    }

    Ok(())
}

fn workflow_job_index_entry_path(job_id: &str) -> String {
    if job_id.trim().is_empty() {
        "jobs/index.json jobs[]".to_string()
    } else {
        format!("jobs/index.json jobs[{job_id}]")
    }
}

fn validate_workflow_job_index_field(
    entry_path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!("workflow job index {field} `{actual}` must match job sidecar `{expected}`"),
            "Regenerate jobs/index.json from workflow job sidecars.",
        ));
    }
}

fn validate_workflow_job_index_optional_field(
    entry_path: &str,
    field: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "workflow job index {field} `{}` must match job sidecar `{}`",
                actual.unwrap_or("null"),
                expected.unwrap_or("null")
            ),
            "Regenerate jobs/index.json from workflow job sidecars.",
        ));
    }
}

fn validate_workflow_job_index_bool_field(
    entry_path: &str,
    field: &str,
    actual: bool,
    expected: bool,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!("workflow job index {field} `{actual}` must match job sidecar `{expected}`"),
            "Regenerate jobs/index.json from workflow job sidecars.",
        ));
    }
}

fn validate_job_summary(
    base_path: &str,
    job: &JobSummary,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if job.id.trim().is_empty() {
        issues.push(issue(
            job_field_path(base_path, "id"),
            "job id is required",
            "Set a stable job id.",
        ));
    } else if !is_safe_manifest_segment(&job.id) {
        issues.push(issue(
            job_field_path(base_path, "id"),
            format!("job id `{}` must be a safe path segment", job.id),
            "Use a job id without slashes, backslashes, or parent path components.",
        ));
    }

    if job.kind.trim().is_empty() {
        issues.push(issue(
            job_field_path(base_path, "kind"),
            "job kind is required",
            "Set the workflow job kind such as render_draft or generate_media.",
        ));
    }
    if job.updated_at.trim().is_empty() {
        issues.push(issue(
            job_field_path(base_path, "updatedAt"),
            "job updatedAt is required",
            "Record the last workflow status timestamp.",
        ));
    }

    if let Some(workflow) = &job.workflow {
        validate_job_workflow(base_path, workflow, issues);
    }
    if let Some(start_request) = &job.start_request {
        validate_job_start_request(base_path, start_request, issues);
    }
    if let (Some(workflow), Some(start_request)) = (&job.workflow, &job.start_request) {
        if !workflow_start_request_ready(workflow, start_request) {
            issues.push(issue(
                job_field_path(base_path, "startRequest"),
                "start request must match workflow metadata",
                "Keep workflow id, workflow type, task queue, and activity types aligned.",
            ));
        }
    }
}

fn validate_job_workflow(
    base_path: &str,
    workflow: &super::model::TemporalWorkflowMetadata,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    for (field, value) in [
        ("workflowId", workflow.workflow_id.as_str()),
        ("workflowType", workflow.workflow_type.as_str()),
        ("taskQueue", workflow.task_queue.as_str()),
    ] {
        if value.trim().is_empty() {
            issues.push(issue(
                job_field_path(base_path, &format!("workflow.{field}")),
                format!("workflow {field} is required"),
                "Record complete Temporal workflow metadata for this job.",
            ));
        }
    }

    if workflow
        .run_id
        .as_ref()
        .is_some_and(|run_id| run_id.trim().is_empty())
    {
        issues.push(issue(
            job_field_path(base_path, "workflow.runId"),
            "workflow runId cannot be empty",
            "Use null until Temporal returns a run id, or record the non-empty run id.",
        ));
    }

    if workflow.activity_types.is_empty() {
        issues.push(issue(
            job_field_path(base_path, "workflow.activityTypes"),
            "workflow activityTypes must include at least one activity type",
            "List the Temporal activities this workflow is expected to run.",
        ));
    }
    for (index, activity_type) in workflow.activity_types.iter().enumerate() {
        if activity_type.trim().is_empty() {
            issues.push(issue(
                job_field_path(base_path, &format!("workflow.activityTypes[{index}]")),
                "workflow activity type cannot be empty",
                "Remove the empty activity type or replace it with the Temporal activity name.",
            ));
        }
    }
}

fn validate_job_start_request(
    base_path: &str,
    start_request: &TemporalWorkflowStartRequest,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    for (field, value) in [
        ("workflowId", start_request.workflow_id.as_str()),
        ("workflowType", start_request.workflow_type.as_str()),
        ("taskQueue", start_request.task_queue.as_str()),
        ("idReusePolicy", start_request.id_reuse_policy.as_str()),
    ] {
        if value.trim().is_empty() {
            issues.push(issue(
                job_field_path(base_path, &format!("startRequest.{field}")),
                format!("start request {field} is required"),
                "Record complete Temporal workflow start request metadata for this job.",
            ));
        }
    }

    if start_request.activity_types.is_empty() {
        issues.push(issue(
            job_field_path(base_path, "startRequest.activityTypes"),
            "start request activityTypes must include at least one activity type",
            "List the Temporal activities this workflow is expected to run.",
        ));
    }
    for (index, activity_type) in start_request.activity_types.iter().enumerate() {
        if activity_type.trim().is_empty() {
            issues.push(issue(
                job_field_path(base_path, &format!("startRequest.activityTypes[{index}]")),
                "start request activity type cannot be empty",
                "Remove the empty activity type or replace it with the Temporal activity name.",
            ));
        }
    }
}

fn job_field_path(base_path: &str, field: &str) -> String {
    if base_path.starts_with("video-creater.project.json ") {
        format!("{base_path}.{field}")
    } else {
        format!("{base_path} {field}")
    }
}

fn save_manifest_for_project(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<SplitProjectManifest, SplitProjectError> {
    let mut manifest = default_split_manifest_for_project(project);

    if let Some(files) = existing_split_manifest_files(project_dir)? {
        manifest.files = files;
    }

    Ok(manifest)
}

fn existing_split_manifest_files(
    project_dir: &Path,
) -> Result<Option<SplitProjectFiles>, SplitProjectError> {
    let manifest_path = split_project_manifest_path(project_dir);
    if !manifest_path.exists() {
        return Ok(None);
    }

    let json = std::fs::read_to_string(&manifest_path).map_err(|error| SplitProjectError::Io {
        path: manifest_path.display().to_string(),
        message: error.to_string(),
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&json).map_err(|error| {
        SplitProjectError::Json {
            path: manifest_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    if !is_split_manifest_value(&value) {
        return Ok(None);
    }

    let manifest: SplitProjectManifest =
        serde_json::from_value(value).map_err(|error| SplitProjectError::Json {
            path: manifest_path.display().to_string(),
            message: error.to_string(),
        })?;

    validate_manifest_file_paths(project_dir, &manifest.files)?;
    Ok(Some(manifest.files))
}

fn validate_manifest_file_paths(
    project_dir: &Path,
    files: &SplitProjectFiles,
) -> Result<(), SplitProjectError> {
    for (field, value) in [
        ("files.timeline", files.timeline.as_str()),
        ("files.media", files.media.as_str()),
        ("files.transcripts", files.transcripts.as_str()),
        ("files.templates", files.templates.as_str()),
        ("files.generated", files.generated.as_str()),
        ("files.renders", files.renders.as_str()),
        ("files.logs", files.logs.as_str()),
    ] {
        safe_join(project_dir, field, value)?;
    }

    Ok(())
}

pub fn migrate_single_file_project(
    project_dir: &Path,
) -> Result<ProjectWriteReport, SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    reject_already_split_project(project_dir)?;

    let mut project =
        super::storage::load_project(project_dir).map_err(|error| SplitProjectError::Io {
            path: split_project_manifest_path(project_dir)
                .display()
                .to_string(),
            message: error.to_string(),
        })?;
    let source_schema_version = project.schema_version;

    project.schema_version = SPLIT_PROJECT_SCHEMA_VERSION;
    let mut save_result = save_split_project(project_dir, &project)?;
    let migration_log = MigrationLog {
        schema_version: 1,
        project_id: project.id,
        migrated_at: chrono::Utc::now().to_rfc3339(),
        source_schema_version,
        target_schema_version: SPLIT_PROJECT_SCHEMA_VERSION,
        written_files: save_result.report.written_files.clone(),
    };
    let migration_log_path = project_dir.join("logs").join(format!(
        "project-migration-{}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));

    write_json_file(&migration_log_path, &migration_log)?;
    save_result
        .report
        .written_files
        .push(migration_log_path.display().to_string());

    Ok(save_result.report)
}

fn reject_already_split_project(project_dir: &Path) -> Result<(), SplitProjectError> {
    let manifest_path = split_project_manifest_path(project_dir);
    if !manifest_path.exists() {
        return Ok(());
    }

    let json = std::fs::read_to_string(&manifest_path).map_err(|error| SplitProjectError::Io {
        path: manifest_path.display().to_string(),
        message: error.to_string(),
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&json).map_err(|error| {
        SplitProjectError::Json {
            path: manifest_path.display().to_string(),
            message: error.to_string(),
        }
    })?;

    if is_split_manifest_value(&value) {
        return Err(SplitProjectError::AlreadySplitProject);
    }

    Ok(())
}

fn validate_media_index(
    project_dir: &Path,
    media: &SplitMediaIndexFile,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let mut folder_ids = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeSet::new();

    for folder in &media.folders {
        if folder.id.trim().is_empty() {
            issues.push(issue(
                "media/index.json folders[].id",
                "media folder id is required",
                "Set a stable media folder id.",
            ));
        } else if !is_safe_manifest_segment(&folder.id) {
            issues.push(issue(
                format!("media/index.json folders[{}].id", folder.id),
                format!(
                    "media folder id `{}` must be a safe path segment",
                    folder.id
                ),
                "Use a folder id without slashes, backslashes, or parent path components.",
            ));
        } else if !folder_ids.insert(folder.id.clone()) {
            issues.push(issue(
                "media/index.json folders",
                format!("duplicate media folder id `{}`", folder.id),
                "Use unique media folder ids.",
            ));
        }
    }

    for folder in &media.folders {
        let Some(parent_id) = &folder.parent_id else {
            continue;
        };
        let parent_path = format!("media/index.json folders[{}].parentId", folder.id);
        if parent_id == &folder.id {
            issues.push(issue(
                parent_path,
                "media folder parentId cannot reference itself",
                "Set parentId to another folder id or null.",
            ));
        } else if !folder_ids.contains(parent_id) {
            issues.push(issue(
                parent_path,
                format!("media folder references missing parent `{parent_id}`"),
                "Add the parent folder or set parentId to null.",
            ));
        }
    }
    let parent_by_folder_id = media
        .folders
        .iter()
        .filter(|folder| !folder.id.trim().is_empty())
        .map(|folder| (folder.id.as_str(), folder.parent_id.as_deref()))
        .collect::<std::collections::BTreeMap<_, _>>();

    for folder in &media.folders {
        if folder.id.trim().is_empty() {
            continue;
        }

        let mut seen = std::collections::BTreeSet::new();
        let mut current_parent = folder.parent_id.as_deref();
        while let Some(parent_id) = current_parent {
            if !seen.insert(parent_id) {
                issues.push(issue(
                    format!("media/index.json folders[{}].parentId", folder.id),
                    "media folder parent chain contains a cycle",
                    "Break the folder cycle by setting one parentId to null or another folder.",
                ));
                break;
            }
            current_parent = parent_by_folder_id.get(parent_id).copied().flatten();
        }
    }

    for asset in &media.assets {
        if asset.id.trim().is_empty() {
            issues.push(issue(
                "media/index.json assets[].id",
                "media asset id is required",
                "Set a stable media asset id.",
            ));
        } else if !is_safe_manifest_segment(&asset.id) {
            issues.push(issue(
                format!("media/index.json assets[{}].id", asset.id),
                format!("media asset id `{}` must be a safe path segment", asset.id),
                "Use a media id without slashes, backslashes, or parent path components.",
            ));
        } else if !ids.insert(asset.id.clone()) {
            issues.push(issue(
                "media/index.json assets",
                format!("duplicate media id `{}`", asset.id),
                "Use unique media asset ids.",
            ));
        }

        if !asset.duration_seconds.is_finite() || asset.duration_seconds < 0.0 {
            issues.push(issue(
                format!("media/index.json assets[{}].durationSeconds", asset.id),
                "media duration must be finite and non-negative",
                "Probe the media again or repair the duration.",
            ));
        }

        if let Some(folder_id) = &asset.folder_id {
            if folder_id.trim().is_empty() {
                issues.push(issue(
                    format!("media/index.json assets[{}].folderId", asset.id),
                    "media folderId cannot be empty",
                    "Set folderId to an existing media folder id or remove the field.",
                ));
            } else if !folder_ids.contains(folder_id) {
                issues.push(issue(
                    format!("media/index.json assets[{}].folderId", asset.id),
                    format!("media asset references missing folder `{folder_id}`"),
                    "Add the media folder or update the asset folderId.",
                ));
            }
        }

        let path = format!("media/index.json assets[{}].relativePath", asset.id);
        if asset.relative_path.trim().is_empty() {
            issues.push(issue(
                path,
                "media relative path is required",
                "Record the media file path inside the project folder.",
            ));
        } else {
            validate_project_relative_path_value(
                project_dir,
                &path,
                &path,
                &asset.relative_path,
                "Keep media file paths inside the project folder.",
                issues,
            );
        }
    }
}

fn validate_timeline(
    timeline: &SplitTimelineFile,
    media: &SplitMediaIndexFile,
    generated_assets: &BTreeMap<String, GeneratedAssetSummary>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let media_ids = media
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let media_by_id = media
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut item_ids = std::collections::BTreeSet::new();
    let mut latest_item_end: Option<(f64, String)> = None;

    if !timeline.duration_seconds.is_finite() || timeline.duration_seconds < 0.0 {
        issues.push(issue(
            "timeline.json durationSeconds",
            "timeline duration must be finite and non-negative",
            "Recalculate timeline duration from item end times.",
        ));
    }

    let mut track_ids = std::collections::BTreeSet::new();
    for (track_index, track) in timeline.tracks.iter().enumerate() {
        if !track_ids.insert(track.id.clone()) {
            issues.push(issue(
                format!("timeline.json tracks[{track_index}].id"),
                format!("duplicate track id `{}`", track.id),
                "Use unique timeline track ids.",
            ));
        }

        let mut valid_item_ranges = Vec::new();
        for (item_index, item) in track.items.iter().enumerate() {
            let item_path = format!("timeline.json tracks[{track_index}].items[{item_index}]");

            if !item_ids.insert(item.id.clone()) {
                issues.push(issue(
                    format!("{item_path}.id"),
                    format!("duplicate timeline item id `{}`", item.id),
                    "Use unique timeline item ids.",
                ));
            }

            if !item.start_seconds.is_finite() || item.start_seconds < 0.0 {
                issues.push(issue(
                    format!("{item_path}.startSeconds"),
                    "item start must be finite and non-negative",
                    "Move the item to a valid timeline time.",
                ));
            }
            if !item.duration_seconds.is_finite() || item.duration_seconds <= 0.0 {
                issues.push(issue(
                    format!("{item_path}.durationSeconds"),
                    "item duration must be finite and greater than zero",
                    "Set a positive item duration.",
                ));
            }
            if item.start_seconds.is_finite()
                && item.start_seconds >= 0.0
                && item.duration_seconds.is_finite()
                && item.duration_seconds > 0.0
            {
                let item_end = item.start_seconds + item.duration_seconds;
                valid_item_ranges.push((
                    item.start_seconds,
                    item_end,
                    item.id.as_str(),
                    item_index,
                ));
                if latest_item_end
                    .as_ref()
                    .map(|(latest_end, _)| item_end > *latest_end)
                    .unwrap_or(true)
                {
                    latest_item_end = Some((item_end, item.id.clone()));
                }
            }

            if !item_allowed_on_track(&item.kind, &track.kind) {
                issues.push(issue(
                    format!("{item_path}.kind"),
                    format!(
                        "{} item is incompatible with {} track",
                        item_kind_name(&item.kind),
                        track_kind_name(&track.kind)
                    ),
                    "Move the item to a compatible track or update its kind.",
                ));
            }
            if let Some(source_issue) = item_source_issue_for_kind(&item.kind, &item.source) {
                issues.push(issue(
                    format!("{}{}", item_path, source_issue.path_suffix),
                    source_issue.message,
                    source_issue.suggestion,
                ));
            }
            validate_generated_timeline_metadata(&item_path, item, generated_assets, issues);

            match &item.source {
                super::model::TimelineSource::Media { media_id } => {
                    if !media_ids.contains(media_id.as_str()) {
                        issues.push(issue(
                            format!("{item_path}.source.mediaId"),
                            format!("timeline item references missing media `{media_id}`"),
                            "Add the media asset or update the timeline item source.",
                        ));
                    } else if let Some(asset) = media_by_id.get(media_id.as_str()) {
                        if !media_allowed_for_item_kind(&item.kind, &asset.kind) {
                            issues.push(issue(
                                format!("{item_path}.source.mediaId"),
                                format!(
                                    "{} item cannot reference {} media `{media_id}`",
                                    item_kind_name(&item.kind),
                                    media_kind_name(&asset.kind)
                                ),
                                "Use media whose kind matches the timeline item kind.",
                            ));
                        }
                        validate_source_range(&item_path, item, asset.duration_seconds, issues);
                    }
                }
                super::model::TimelineSource::Generated { artifact_id } => {
                    if let Some(generated_asset) = generated_assets.get(artifact_id) {
                        if generated_asset.status != GeneratedAssetStatus::Completed {
                            issues.push(issue(
                                format!("{item_path}.source.artifactId"),
                                format!(
                                    "timeline item references generated asset `{artifact_id}` that is not completed"
                                ),
                                "Wait for generation to complete or reference a completed generated asset.",
                            ));
                        }
                        if generated_asset.output_count == 0 {
                            issues.push(issue(
                                format!("{item_path}.source.artifactId"),
                                format!(
                                    "timeline item references generated asset `{artifact_id}` with no outputs"
                                ),
                                "Record at least one generated output before placing this asset on the timeline.",
                            ));
                        } else if let Some(max_output_duration) =
                            generated_asset.max_output_duration_seconds
                        {
                            if item.duration_seconds > max_output_duration
                                && !nearly_equal(item.duration_seconds, max_output_duration)
                            {
                                issues.push(issue(
                                    format!("{item_path}.durationSeconds"),
                                    format!(
                                        "timeline item duration {:.3}s exceeds generated output duration {:.3}s for `{artifact_id}`",
                                        item.duration_seconds, max_output_duration
                                    ),
                                    "Shorten the timeline item or regenerate an output with enough duration.",
                                ));
                            }
                        }
                    } else {
                        issues.push(issue(
                            format!("{item_path}.source.artifactId"),
                            format!(
                                "timeline item references missing generated asset `{artifact_id}`"
                            ),
                            "Add the generated asset file or update the timeline item source.",
                        ));
                    }
                }
                super::model::TimelineSource::Timeline { .. } => {}
                super::model::TimelineSource::Text { .. } => {}
            }
        }
        validate_track_item_order(track_index, &valid_item_ranges, issues);
        validate_track_item_overlaps(track_index, &mut valid_item_ranges, issues);
    }

    if timeline.duration_seconds.is_finite() && timeline.duration_seconds >= 0.0 {
        if let Some((latest_end, item_id)) = latest_item_end {
            if latest_end > timeline.duration_seconds
                && !nearly_equal(latest_end, timeline.duration_seconds)
            {
                issues.push(issue(
                    "timeline.json durationSeconds",
                    format!(
                        "timeline duration must cover item `{item_id}` ending at {latest_end:.3}s"
                    ),
                    "Recalculate timeline duration from item end times.",
                ));
            }
        }
    }
}

fn validate_generated_timeline_metadata(
    item_path: &str,
    item: &super::model::TimelineItem,
    generated_assets: &BTreeMap<String, GeneratedAssetSummary>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let generated_asset_id = string_item_property(item, "generatedAssetId");
    let generated_output_media_id = string_item_property(item, "generatedOutputMediaId");

    let Some(generated_asset_id) = generated_asset_id else {
        if let Some(generated_output_media_id) = generated_output_media_id {
            issues.push(issue(
                format!("{item_path}.properties.generatedOutputMediaId"),
                format!(
                    "timeline generatedOutputMediaId `{generated_output_media_id}` requires generatedAssetId"
                ),
                "Set generatedAssetId to the generated asset that owns this output.",
            ));
        }
        return;
    };
    let Some(generated_asset) = generated_assets.get(generated_asset_id) else {
        issues.push(issue(
            format!("{item_path}.properties.generatedAssetId"),
            format!("timeline item references missing generated asset `{generated_asset_id}`"),
            "Add the generated asset file or update generatedAssetId.",
        ));
        return;
    };

    if generated_asset.status != GeneratedAssetStatus::Completed {
        issues.push(issue(
            format!("{item_path}.properties.generatedAssetId"),
            format!(
                "timeline item references generated asset `{generated_asset_id}` that is not completed"
            ),
            "Wait for generation to complete or update generatedAssetId.",
        ));
    }

    if let Some(generated_output_media_id) = generated_output_media_id {
        if !generated_asset
            .output_media_ids
            .contains(generated_output_media_id)
        {
            issues.push(issue(
                format!("{item_path}.properties.generatedOutputMediaId"),
                format!(
                    "timeline item references generated output `{generated_output_media_id}` that is not recorded on generated asset `{generated_asset_id}`"
                ),
                "Use an output media id listed in the generated asset sidecar.",
            ));
        }
        if let super::model::TimelineSource::Media { media_id } = &item.source {
            if media_id != generated_output_media_id {
                issues.push(issue(
                    format!("{item_path}.properties.generatedOutputMediaId"),
                    format!(
                        "timeline generatedOutputMediaId `{generated_output_media_id}` does not match source media `{media_id}`"
                    ),
                    "Keep generatedOutputMediaId aligned with the timeline item's media source.",
                ));
            }
        }
    } else if let super::model::TimelineSource::Media { media_id } = &item.source {
        if generated_asset.output_media_ids.contains(media_id) {
            issues.push(issue(
                format!("{item_path}.properties.generatedOutputMediaId"),
                format!(
                    "timeline item source media `{media_id}` is a generated output and requires generatedOutputMediaId when generatedAssetId is set"
                ),
                "Set generatedOutputMediaId to the generated output media id that this timeline item uses.",
            ));
        }
    }
}

fn string_item_property<'a>(item: &'a super::model::TimelineItem, key: &str) -> Option<&'a str> {
    item.properties
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn media_allowed_for_item_kind(item_kind: &TimelineItemKind, media_kind: &MediaKind) -> bool {
    match item_kind {
        TimelineItemKind::VideoClip => {
            matches!(media_kind, MediaKind::Video | MediaKind::Generated)
        }
        TimelineItemKind::ImageClip => {
            matches!(media_kind, MediaKind::Image | MediaKind::Generated)
        }
        TimelineItemKind::LottieClip => {
            matches!(media_kind, MediaKind::Lottie | MediaKind::Generated)
        }
        TimelineItemKind::GeneratedClip => {
            matches!(media_kind, MediaKind::Generated)
        }
        TimelineItemKind::AudioClip => {
            matches!(
                media_kind,
                MediaKind::Audio | MediaKind::Video | MediaKind::Generated
            )
        }
        _ => true,
    }
}

fn media_kind_name(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
        MediaKind::Image => "image",
        MediaKind::Lottie => "lottie",
        MediaKind::Generated => "generated",
    }
}

struct TimelineSourceKindIssue {
    path_suffix: &'static str,
    message: &'static str,
    suggestion: &'static str,
}

fn item_source_issue_for_kind(
    item_kind: &TimelineItemKind,
    source: &super::model::TimelineSource,
) -> Option<TimelineSourceKindIssue> {
    match item_kind {
        TimelineItemKind::Caption => match source {
            super::model::TimelineSource::Text { text } => {
                if text.trim().is_empty() {
                    Some(TimelineSourceKindIssue {
                        path_suffix: ".source.text",
                        message: "caption text is required",
                        suggestion: "Set non-empty caption text.",
                    })
                } else {
                    None
                }
            }
            _ => Some(TimelineSourceKindIssue {
                path_suffix: ".source",
                message: "caption item requires a text source",
                suggestion: "Set caption item sources to source.type = text.",
            }),
        },
        TimelineItemKind::VideoClip => {
            if matches!(
                source,
                super::model::TimelineSource::Media { .. }
                    | super::model::TimelineSource::Generated { .. }
                    | super::model::TimelineSource::Timeline { .. }
            ) {
                None
            } else {
                Some(TimelineSourceKindIssue {
                    path_suffix: ".source",
                    message: "video item requires a media, generated, or timeline source",
                    suggestion:
                        "Set video item sources to source.type = media, generated, or timeline.",
                })
            }
        }
        TimelineItemKind::ImageClip
        | TimelineItemKind::LottieClip
        | TimelineItemKind::GeneratedClip
        | TimelineItemKind::AudioClip => {
            if matches!(
                source,
                super::model::TimelineSource::Media { .. }
                    | super::model::TimelineSource::Generated { .. }
            ) {
                None
            } else {
                Some(TimelineSourceKindIssue {
                    path_suffix: ".source",
                    message: match item_kind {
                        TimelineItemKind::ImageClip => {
                            "image item requires a media or generated source"
                        }
                        TimelineItemKind::LottieClip => {
                            "lottie item requires a media or generated source"
                        }
                        TimelineItemKind::GeneratedClip => {
                            "generated item requires a media or generated source"
                        }
                        TimelineItemKind::AudioClip => {
                            "audio item requires a media or generated source"
                        }
                        _ => unreachable!(),
                    },
                    suggestion: "Set media item sources to source.type = media or generated.",
                })
            }
        }
        _ => None,
    }
}

fn validate_timeline_library(
    timeline: &SplitTimelineFile,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let mut timeline_ids = std::collections::BTreeSet::new();
    let mut edges = std::collections::BTreeMap::<String, Vec<(String, String)>>::new();

    for (timeline_index, entry) in timeline.timelines.iter().enumerate() {
        let entry_path = format!("timeline.json timelines[{timeline_index}]");
        if entry.id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.id"),
                "timeline id cannot be empty",
                "Give each timeline a stable non-empty id.",
            ));
            continue;
        }
        if !timeline_ids.insert(entry.id.clone()) {
            issues.push(issue(
                format!("{entry_path}.id"),
                format!("duplicate timeline id `{}`", entry.id),
                "Use unique timeline ids for alternate cuts and sequences.",
            ));
            continue;
        }
        let entry_edges = entry
            .timeline
            .tracks
            .iter()
            .enumerate()
            .flat_map(|(track_index, track)| {
                let entry_path = entry_path.clone();
                track.items.iter().enumerate().filter_map(move |(item_index, item)| {
                    let super::model::TimelineSource::Timeline { timeline_id } = &item.source else {
                        return None;
                    };
                    Some((
                        timeline_id.clone(),
                        format!(
                            "{entry_path}.tracks[{track_index}].items[{item_index}].source.timelineId"
                        ),
                    ))
                })
            })
            .collect::<Vec<_>>();
        edges.insert(entry.id.clone(), entry_edges);
    }

    for references in edges.values() {
        for (target_id, path) in references {
            if target_id.trim().is_empty() || !timeline_ids.contains(target_id) {
                issues.push(issue(
                    path.clone(),
                    format!("timeline item references missing nested timeline `{target_id}`"),
                    "Choose an existing timeline before nesting it.",
                ));
            }
        }
    }

    let mut visited = std::collections::BTreeSet::new();
    let mut visiting = std::collections::BTreeSet::new();
    let mut reported_paths = std::collections::BTreeSet::new();
    for timeline_id in &timeline_ids {
        validate_timeline_dependency_cycles(
            timeline_id,
            &edges,
            &timeline_ids,
            &mut visited,
            &mut visiting,
            &mut reported_paths,
            issues,
        );
    }
}

fn validate_timeline_dependency_cycles(
    timeline_id: &str,
    edges: &std::collections::BTreeMap<String, Vec<(String, String)>>,
    timeline_ids: &std::collections::BTreeSet<String>,
    visited: &mut std::collections::BTreeSet<String>,
    visiting: &mut std::collections::BTreeSet<String>,
    reported_paths: &mut std::collections::BTreeSet<String>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if visited.contains(timeline_id) || !visiting.insert(timeline_id.to_string()) {
        return;
    }
    for (target_id, path) in edges.get(timeline_id).into_iter().flatten() {
        if !timeline_ids.contains(target_id) {
            continue;
        }
        if visiting.contains(target_id) {
            if reported_paths.insert(path.clone()) {
                issues.push(issue(
                    path.clone(),
                    format!("timeline nesting cycle includes `{target_id}`"),
                    "Remove the nested timeline reference that closes the cycle.",
                ));
            }
            continue;
        }
        validate_timeline_dependency_cycles(
            target_id,
            edges,
            timeline_ids,
            visited,
            visiting,
            reported_paths,
            issues,
        );
    }
    visiting.remove(timeline_id);
    visited.insert(timeline_id.to_string());
}

fn validate_track_item_order(
    track_index: usize,
    ranges: &[(f64, f64, &str, usize)],
    issues: &mut Vec<ProjectValidationIssue>,
) {
    for pair in ranges.windows(2) {
        let (previous_start, _, previous_id, _) = pair[0];
        let (current_start, _, current_id, current_index) = pair[1];
        if current_start < previous_start && !nearly_equal(current_start, previous_start) {
            issues.push(issue(
                format!("timeline.json tracks[{track_index}].items[{current_index}].startSeconds"),
                format!(
                    "timeline item `{current_id}` must be in chronological order after `{previous_id}`"
                ),
                "Sort each track's items by startSeconds so timeline files match editor order.",
            ));
        }
    }
}

fn validate_track_item_overlaps(
    track_index: usize,
    ranges: &mut [(f64, f64, &str, usize)],
    issues: &mut Vec<ProjectValidationIssue>,
) {
    ranges.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for pair in ranges.windows(2) {
        let (_, previous_end, previous_id, _) = pair[0];
        let (current_start, _, current_id, current_index) = pair[1];
        if previous_end > current_start && !nearly_equal(previous_end, current_start) {
            issues.push(issue(
                format!("timeline.json tracks[{track_index}].items[{current_index}].startSeconds"),
                format!("timeline item `{current_id}` overlaps `{previous_id}` on the same track"),
                "Move one item to another track or adjust its start/duration so same-track items do not overlap.",
            ));
        }
    }
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

fn item_kind_name(kind: &TimelineItemKind) -> &'static str {
    match kind {
        TimelineItemKind::VideoClip => "video",
        TimelineItemKind::ImageClip => "image",
        TimelineItemKind::LottieClip => "lottie",
        TimelineItemKind::GeneratedClip => "generated",
        TimelineItemKind::HyperframeScene => "hyperframe scene",
        TimelineItemKind::Overlay => "overlay",
        TimelineItemKind::Caption => "caption",
        TimelineItemKind::AudioClip => "audio",
    }
}

fn track_kind_name(kind: &TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "video",
        TrackKind::HyperframeScene => "hyperframe scene",
        TrackKind::Overlay => "overlay",
        TrackKind::Caption => "caption",
        TrackKind::Audio => "audio",
    }
}

/// Playback speed for validation reports. Invalid speeds fall back to 1 so a
/// malformed speed never aborts project validation.
fn validation_item_speed(item: &super::model::TimelineItem) -> f64 {
    item.properties
        .get("speed")
        .and_then(serde_json::Value::as_f64)
        .filter(|speed| speed.is_finite() && *speed > 0.0)
        .unwrap_or(1.0)
}

fn validate_source_range(
    item_path: &str,
    item: &super::model::TimelineItem,
    media_duration: f64,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let generated_edit = item
        .properties
        .get("generatedEdit")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let source_in = item
        .properties
        .get("sourceIn")
        .and_then(serde_json::Value::as_f64);
    let source_out = item
        .properties
        .get("sourceOut")
        .and_then(serde_json::Value::as_f64);

    if generated_edit && (source_in.is_none() || source_out.is_none()) {
        issues.push(issue(
            format!("{item_path}.properties.sourceIn"),
            "generated edit video clips require sourceIn and sourceOut",
            "Build generated edits from selected source ranges before adding captions or overlays.",
        ));
        return;
    }

    if let Some(value) = source_in {
        if !value.is_finite() || value < 0.0 || value > media_duration {
            issues.push(issue(
                format!("{item_path}.properties.sourceIn"),
                "sourceIn must be finite, non-negative, and inside media duration",
                "Set sourceIn inside the source media duration.",
            ));
        }
    }

    if let Some(value) = source_out {
        if !value.is_finite() || value < 0.0 || value > media_duration {
            issues.push(issue(
                format!("{item_path}.properties.sourceOut"),
                "sourceOut must be finite, non-negative, and inside media duration",
                "Set sourceOut inside the source media duration.",
            ));
        }
    }

    if let (Some(source_in), Some(source_out)) = (source_in, source_out) {
        if source_out <= source_in {
            issues.push(issue(
                format!("{item_path}.properties.sourceOut"),
                "sourceOut must be greater than sourceIn",
                "Increase sourceOut or decrease sourceIn.",
            ));
        } else if !nearly_equal(
            source_out - source_in,
            item.duration_seconds * validation_item_speed(item),
        ) {
            issues.push(issue(
                format!("{item_path}.properties.sourceOut"),
                "source range duration must match timeline duration",
                "Make sourceOut minus sourceIn equal the timeline item duration times its playback speed.",
            ));
        } else if generated_edit
            && nearly_equal(source_in, 0.0)
            && nearly_equal(source_out, media_duration)
        {
            issues.push(issue(
                format!("{item_path}.properties.sourceOut"),
                "generated edit video clips cannot pass through the full source",
                "Select a smaller source range for generated edits or mark the clip as a manual full-length sequence.",
            ));
        }
    }
}

fn validate_transcript_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    media: &SplitMediaIndexFile,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let transcripts_dir = safe_join(
        project_dir,
        "files.transcripts",
        &manifest.files.transcripts,
    )?;
    if !transcripts_dir.exists() {
        return Ok(());
    }

    let media_ids = media
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut transcript_ids = std::collections::BTreeSet::new();
    let mut transcript_media_ids = std::collections::BTreeSet::new();
    let mut sidecar_transcripts = BTreeMap::new();
    let mut transcript_paths = std::fs::read_dir(&transcripts_dir)
        .map_err(|error| SplitProjectError::Io {
            path: transcripts_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: transcripts_dir.display().to_string(),
            message: error.to_string(),
        })?;
    transcript_paths.sort();

    for path in transcript_paths {
        if is_transcript_index_path(&path) {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let relative_path = path
            .strip_prefix(project_dir)
            .unwrap_or(path.as_path())
            .display()
            .to_string();
        let transcript: SplitTranscriptFile = match read_json_file(&path) {
            Ok(transcript) => transcript,
            Err(error) => {
                issues.push(issue(
                    relative_path,
                    error.to_string(),
                    "Repair transcript JSON.",
                ));
                continue;
            }
        };

        if !transcript.media_id.trim().is_empty() {
            sidecar_transcripts.insert(
                transcript.media_id.clone(),
                (relative_path.clone(), transcript.clone()),
            );
        }

        if transcript.id.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} id"),
                "transcript id is required",
                "Set a stable transcript id.",
            ));
        } else if !transcript_ids.insert(transcript.id.clone()) {
            issues.push(issue(
                format!("{relative_path} id"),
                format!("duplicate transcript id `{}`", transcript.id),
                "Use unique transcript ids.",
            ));
        }

        if !transcript.media_id.trim().is_empty()
            && !transcript_media_ids.insert(transcript.media_id.clone())
        {
            issues.push(issue(
                format!("{relative_path} mediaId"),
                format!("duplicate transcript mediaId `{}`", transcript.media_id),
                "Use only one transcript sidecar per media asset.",
            ));
        }

        if let Some(file_media_id) = path.file_stem().and_then(|stem| stem.to_str()) {
            if file_media_id != transcript.media_id {
                issues.push(issue(
                    format!("{relative_path} mediaId"),
                    format!(
                        "transcript mediaId `{}` must match filename `{file_media_id}`",
                        transcript.media_id
                    ),
                    "Rename the transcript file or update mediaId to match.",
                ));
            }
        }

        if !media_ids.contains(transcript.media_id.as_str()) {
            issues.push(issue(
                format!("{relative_path} mediaId"),
                format!(
                    "transcript references missing media `{}`",
                    transcript.media_id
                ),
                "Add the media asset or update the transcript mediaId.",
            ));
        }
        if let Some(raw_artifact_path) = &transcript.raw_artifact_path {
            validate_project_relative_path_value(
                project_dir,
                &format!("{relative_path} rawArtifactPath"),
                &format!("{relative_path} rawArtifactPath"),
                raw_artifact_path,
                "Keep transcript raw artifact paths inside the project folder.",
                issues,
            );
        }

        validate_transcript_repairs(&relative_path, &transcript, issues);
        validate_transcript_words(&relative_path, &transcript, issues);
    }

    validate_transcript_index_file(project_dir, &transcripts_dir, &sidecar_transcripts, issues)?;

    Ok(())
}

fn validate_transcript_index_file(
    project_dir: &Path,
    transcripts_dir: &Path,
    sidecar_transcripts: &BTreeMap<String, (String, SplitTranscriptFile)>,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let index_path = transcripts_dir.join(SPLIT_TRANSCRIPT_INDEX_FILE_NAME);
    if !index_path.exists() {
        return Ok(());
    }

    let index: SplitTranscriptIndexFile = match read_json_file(&index_path) {
        Ok(index) => index,
        Err(error) => {
            issues.push(issue(
                "transcripts/index.json",
                error.to_string(),
                "Repair transcripts/index.json or regenerate it from transcript sidecars.",
            ));
            return Ok(());
        }
    };
    let mut indexed_media_ids = BTreeSet::new();

    for entry in &index.transcripts {
        let entry_path = transcript_index_entry_path(&entry.media_id);
        if entry.media_id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.mediaId"),
                "transcript index mediaId is required",
                "Regenerate transcripts/index.json from transcript sidecars.",
            ));
            continue;
        }
        if !indexed_media_ids.insert(entry.media_id.clone()) {
            issues.push(issue(
                format!("{entry_path}.mediaId"),
                format!("duplicate transcript index mediaId `{}`", entry.media_id),
                "Regenerate transcripts/index.json from transcript sidecars.",
            ));
        }
        validate_project_relative_path_value(
            project_dir,
            &format!("{entry_path}.path"),
            &format!("{entry_path}.path"),
            &entry.path,
            "Regenerate transcripts/index.json with project-relative transcript sidecar paths.",
            issues,
        );

        let Some((sidecar_path, transcript)) = sidecar_transcripts.get(&entry.media_id) else {
            issues.push(issue(
                format!("{entry_path}.path"),
                format!(
                    "transcript index entry references missing transcript sidecar `{}`",
                    entry.path
                ),
                "Regenerate transcripts/index.json from transcript sidecars.",
            ));
            continue;
        };
        let (start_seconds, end_seconds) = transcript_time_bounds(transcript);

        validate_transcript_index_field(
            &entry_path,
            "transcriptId",
            &entry.transcript_id,
            &transcript.id,
            issues,
        );
        validate_transcript_index_field(&entry_path, "path", &entry.path, sidecar_path, issues);
        validate_transcript_index_optional_field(
            &entry_path,
            "engine",
            entry.engine.as_deref(),
            transcript.engine.as_deref(),
            issues,
        );
        validate_transcript_index_optional_field(
            &entry_path,
            "rawArtifactPath",
            entry.raw_artifact_path.as_deref(),
            transcript.raw_artifact_path.as_deref(),
            issues,
        );
        validate_transcript_index_usize_field(
            &entry_path,
            "repairCount",
            entry.repair_count,
            transcript.repairs.len(),
            issues,
        );
        validate_transcript_index_usize_field(
            &entry_path,
            "segmentCount",
            entry.segment_count,
            transcript.segments.len(),
            issues,
        );
        validate_transcript_index_usize_field(
            &entry_path,
            "wordCount",
            entry.word_count,
            transcript.words.len(),
            issues,
        );
        validate_transcript_index_optional_seconds_field(
            &entry_path,
            "startSeconds",
            entry.start_seconds,
            start_seconds,
            issues,
        );
        validate_transcript_index_optional_seconds_field(
            &entry_path,
            "endSeconds",
            entry.end_seconds,
            end_seconds,
            issues,
        );
    }

    for media_id in sidecar_transcripts.keys() {
        if !indexed_media_ids.contains(media_id) {
            issues.push(issue(
                format!("transcripts/index.json transcripts[{media_id}]"),
                format!("missing transcript index entry for `{media_id}`"),
                "Regenerate transcripts/index.json from transcript sidecars.",
            ));
        }
    }

    Ok(())
}

fn transcript_index_entry_path(media_id: &str) -> String {
    if media_id.trim().is_empty() {
        "transcripts/index.json transcripts[]".to_string()
    } else {
        format!("transcripts/index.json transcripts[{media_id}]")
    }
}

fn validate_transcript_index_field(
    entry_path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "transcript index {field} `{actual}` must match transcript sidecar `{expected}`"
            ),
            "Regenerate transcripts/index.json from transcript sidecars.",
        ));
    }
}

fn validate_transcript_index_optional_field(
    entry_path: &str,
    field: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "transcript index {field} `{}` must match transcript sidecar `{}`",
                actual.unwrap_or("null"),
                expected.unwrap_or("null")
            ),
            "Regenerate transcripts/index.json from transcript sidecars.",
        ));
    }
}

fn validate_transcript_index_usize_field(
    entry_path: &str,
    field: &str,
    actual: usize,
    expected: usize,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "transcript index {field} `{actual}` must match transcript sidecar `{expected}`"
            ),
            "Regenerate transcripts/index.json from transcript sidecars.",
        ));
    }
}

fn validate_transcript_index_optional_seconds_field(
    entry_path: &str,
    field: &str,
    actual: Option<f64>,
    expected: Option<f64>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let matches = match (actual, expected) {
        (Some(actual), Some(expected)) => nearly_equal(actual, expected),
        (None, None) => true,
        _ => false,
    };
    if !matches {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "transcript index {field} `{}` must match transcript sidecar `{}`",
                optional_seconds_label(actual),
                optional_seconds_label(expected)
            ),
            "Regenerate transcripts/index.json from transcript sidecars.",
        ));
    }
}

fn optional_seconds_label(value: Option<f64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn validate_transcript_repairs(
    path: &str,
    transcript: &SplitTranscriptFile,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let mut repair_ids = std::collections::BTreeSet::new();
    for (index, repair) in transcript.repairs.iter().enumerate() {
        let repair_path = format!("{path} repairs[{index}]");
        if repair.id.trim().is_empty() {
            issues.push(issue(
                format!("{repair_path}.id"),
                "transcript repair id is required",
                "Set a stable transcript repair id.",
            ));
        } else if !repair_ids.insert(repair.id.clone()) {
            issues.push(issue(
                format!("{repair_path}.id"),
                format!("duplicate transcript repair id `{}`", repair.id),
                "Use unique transcript repair ids within each transcript.",
            ));
        }
        if repair.word_index >= transcript.words.len() {
            issues.push(issue(
                format!("{repair_path}.wordIndex"),
                "transcript repair wordIndex is outside transcript words",
                "Point the repair at an existing transcript word.",
            ));
        }
    }
}

fn validate_transcript_words(
    path: &str,
    transcript: &SplitTranscriptFile,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let mut previous_end = None;

    for (index, word) in transcript.words.iter().enumerate() {
        if !word.start_seconds.is_finite() || word.start_seconds < 0.0 {
            issues.push(issue(
                format!("{path} words[{index}].startSeconds"),
                "word start must be finite and non-negative",
                "Repair transcript word timing.",
            ));
        }
        if !word.end_seconds.is_finite() || word.end_seconds < word.start_seconds {
            issues.push(issue(
                format!("{path} words[{index}].endSeconds"),
                "word end must be finite and greater than or equal to start",
                "Repair transcript word timing.",
            ));
        }
        if let Some(previous) = previous_end {
            if word.start_seconds < previous {
                issues.push(issue(
                    format!("{path} words[{index}].startSeconds"),
                    "transcript word timestamps must be monotonic",
                    "Sort or repair transcript word timing.",
                ));
            }
        }

        previous_end = Some(word.end_seconds);
    }
}

fn nearly_equal(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-6
}

#[derive(Debug, Clone)]
struct GeneratedAssetSummary {
    status: GeneratedAssetStatus,
    output_count: usize,
    max_output_duration_seconds: Option<f64>,
    output_media_ids: BTreeSet<String>,
}

fn collect_generated_asset_summaries(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
) -> Result<BTreeMap<String, GeneratedAssetSummary>, SplitProjectError> {
    let generated_dir = safe_join(project_dir, "files.generated", &manifest.files.generated)?;
    if !generated_dir.exists() {
        return Ok(BTreeMap::new());
    }

    let mut generated_paths = std::fs::read_dir(&generated_dir)
        .map_err(|error| SplitProjectError::Io {
            path: generated_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path().join("asset.json")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: generated_dir.display().to_string(),
            message: error.to_string(),
        })?;
    generated_paths.sort();

    let mut generated_assets = BTreeMap::new();
    for path in generated_paths {
        if !path.exists() {
            continue;
        }
        let Ok(generated_asset) = read_json_file::<GeneratedAsset>(&path) else {
            continue;
        };
        if !generated_asset.id.trim().is_empty() {
            let max_output_duration_seconds = generated_asset
                .outputs
                .iter()
                .filter_map(|output| {
                    if output.duration_seconds.is_finite() && output.duration_seconds > 0.0 {
                        Some(output.duration_seconds)
                    } else {
                        None
                    }
                })
                .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
            generated_assets.insert(
                generated_asset.id,
                GeneratedAssetSummary {
                    status: generated_asset.status,
                    output_count: generated_asset.outputs.len(),
                    max_output_duration_seconds,
                    output_media_ids: generated_asset
                        .outputs
                        .iter()
                        .map(|output| output.media_id.clone())
                        .collect(),
                },
            );
        }
    }

    Ok(generated_assets)
}

fn validate_generated_asset_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    media: &SplitMediaIndexFile,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let generated_dir = safe_join(project_dir, "files.generated", &manifest.files.generated)?;
    let media_ids = media
        .assets
        .iter()
        .map(|asset| asset.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let folder_ids = media
        .folders
        .iter()
        .map(|folder| folder.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let media_by_id = media
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut generated_ids = std::collections::BTreeSet::new();
    let mut generated_lineage_references = Vec::new();
    let mut generated_output_media_ids = std::collections::BTreeSet::new();
    let mut generated_output_owner_by_media_id =
        std::collections::BTreeMap::<String, String>::new();
    let mut generated_paths = if generated_dir.exists() {
        std::fs::read_dir(&generated_dir)
            .map_err(|error| SplitProjectError::Io {
                path: generated_dir.display().to_string(),
                message: error.to_string(),
            })?
            .map(|entry| entry.map(|entry| entry.path().join("asset.json")))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SplitProjectError::Io {
                path: generated_dir.display().to_string(),
                message: error.to_string(),
            })?
    } else {
        Vec::new()
    };
    generated_paths.sort();

    for path in generated_paths {
        if !path.exists() {
            continue;
        }

        let relative_path = project_relative_path(project_dir, &path);
        let generated_asset: GeneratedAsset = match read_json_file(&path) {
            Ok(generated_asset) => generated_asset,
            Err(error) => {
                issues.push(issue(
                    relative_path,
                    error.to_string(),
                    "Repair generated asset JSON.",
                ));
                continue;
            }
        };

        if generated_asset.schema_version != SPLIT_GENERATED_ASSET_SCHEMA_VERSION {
            issues.push(issue(
                format!("{relative_path} schemaVersion"),
                format!(
                    "generated asset schemaVersion must be {}",
                    SPLIT_GENERATED_ASSET_SCHEMA_VERSION
                ),
                "Update the generated asset file to the supported schema version.",
            ));
        }
        if generated_asset.kind != MediaKind::Generated {
            issues.push(issue(
                format!("{relative_path} kind"),
                "generated asset kind must be generated",
                "Set generated asset sidecars to kind generated.",
            ));
        }
        if generated_asset.id.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} id"),
                "generated asset id is required",
                "Set a stable generated asset id.",
            ));
        } else {
            if let Some(directory_id) = path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
            {
                if directory_id != generated_asset.id {
                    issues.push(issue(
                        format!("{relative_path} id"),
                        format!(
                            "generated asset id `{}` must match directory `{directory_id}`",
                            generated_asset.id
                        ),
                        "Move the generated asset file or update its id to match the directory.",
                    ));
                }
            }
            if !generated_ids.insert(generated_asset.id.clone()) {
                issues.push(issue(
                    format!("{relative_path} id"),
                    format!("duplicate generated asset id `{}`", generated_asset.id),
                    "Use unique generated asset ids.",
                ));
            }
        }
        if generated_asset.prompt.trim().is_empty()
            && !allows_promptless_source_video_audio_asset(
                &generated_asset.references,
                &generated_asset.settings,
            )
        {
            issues.push(issue(
                format!("{relative_path} prompt"),
                "generated asset prompt is required",
                "Record the prompt used to generate the asset.",
            ));
        }
        if generated_asset.model.provider.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} model.provider"),
                "generated asset model provider is required",
                "Record the generation provider.",
            ));
        }
        if generated_asset.model.id.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} model.id"),
                "generated asset model id is required",
                "Record the generation model id.",
            ));
        }
        if let Some(target_folder_id) = &generated_asset.target_folder_id {
            if target_folder_id.trim().is_empty() {
                issues.push(issue(
                    format!("{relative_path} targetFolderId"),
                    "generated asset targetFolderId cannot be empty",
                    "Set targetFolderId to an existing media folder id or remove the field.",
                ));
            } else if !folder_ids.contains(target_folder_id.as_str()) {
                issues.push(issue(
                    format!("{relative_path} targetFolderId"),
                    format!(
                        "generated asset references missing target folder `{target_folder_id}`"
                    ),
                    "Add the media folder or update the generated asset targetFolderId.",
                ));
            }
        }
        validate_generated_settings(&relative_path, &generated_asset.settings, issues);
        generated_lineage_references.push((
            relative_path.clone(),
            generated_asset.id.clone(),
            generated_asset.parent_asset_id.clone(),
            generated_asset.retry_of_asset_id.clone(),
        ));

        for (index, media_id) in generated_asset.references.media_ids.iter().enumerate() {
            validate_generated_media_reference(
                &relative_path,
                &format!("references.mediaIds[{index}]"),
                media_id,
                &media_ids,
                issues,
            );
        }
        if let Some(media_id) = &generated_asset.references.first_frame_media_id {
            validate_generated_frame_reference(
                &relative_path,
                "references.firstFrameMediaId",
                media_id,
                &media_ids,
                &media_by_id,
                issues,
            );
        }
        if let Some(media_id) = &generated_asset.references.last_frame_media_id {
            validate_generated_frame_reference(
                &relative_path,
                "references.lastFrameMediaId",
                media_id,
                &media_ids,
                &media_by_id,
                issues,
            );
        }
        if let Some(media_id) = &generated_asset.references.source_video_media_ref {
            validate_generated_video_reference(
                &relative_path,
                "references.sourceVideoMediaRef",
                media_id,
                &media_ids,
                &media_by_id,
                issues,
            );
        }
        for (index, media_id) in generated_asset
            .references
            .reference_image_media_refs
            .iter()
            .enumerate()
        {
            validate_generated_frame_reference(
                &relative_path,
                &format!("references.referenceImageMediaRefs[{index}]"),
                media_id,
                &media_ids,
                &media_by_id,
                issues,
            );
        }
        for (index, media_id) in generated_asset
            .references
            .reference_video_media_refs
            .iter()
            .enumerate()
        {
            validate_generated_video_reference(
                &relative_path,
                &format!("references.referenceVideoMediaRefs[{index}]"),
                media_id,
                &media_ids,
                &media_by_id,
                issues,
            );
        }
        for (index, media_id) in generated_asset
            .references
            .reference_audio_media_refs
            .iter()
            .enumerate()
        {
            validate_generated_audio_reference(
                &relative_path,
                &format!("references.referenceAudioMediaRefs[{index}]"),
                media_id,
                &media_ids,
                &media_by_id,
                issues,
            );
        }

        if generated_asset.outputs.is_empty()
            && generated_asset.status == GeneratedAssetStatus::Completed
        {
            issues.push(issue(
                format!("{relative_path} outputs"),
                "completed generated asset must include at least one output",
                "Record the generated media outputs or mark the asset as pending or failed.",
            ));
        }
        let generated_asset_directory = path
            .parent()
            .map(|directory| project_relative_path(project_dir, directory));
        let mut output_media_ids = std::collections::BTreeSet::new();
        for (index, output) in generated_asset.outputs.iter().enumerate() {
            let output_path = format!("{relative_path} outputs[{index}]");
            if output.media_id.trim().is_empty() {
                issues.push(issue(
                    format!("{output_path}.mediaId"),
                    "generated output media id is required",
                    "Set the generated media id for this output.",
                ));
            } else if !output_media_ids.insert(output.media_id.clone()) {
                issues.push(issue(
                    format!("{output_path}.mediaId"),
                    format!("duplicate generated output media id `{}`", output.media_id),
                    "Use unique output media ids within a generated asset.",
                ));
            } else {
                if let Some(previous_asset_id) = generated_output_owner_by_media_id
                    .insert(output.media_id.clone(), generated_asset.id.clone())
                {
                    if previous_asset_id != generated_asset.id {
                        issues.push(issue(
                            format!("{output_path}.mediaId"),
                            format!(
                                "generated output media id `{}` is already used by generated asset `{previous_asset_id}`",
                                output.media_id
                            ),
                            "Use a unique generated output media id for each generated asset output.",
                        ));
                    }
                }
                generated_output_media_ids.insert(output.media_id.clone());

                if !media_ids.contains(output.media_id.as_str()) {
                    issues.push(issue(
                        format!("{output_path}.mediaId"),
                        format!(
                            "generated output references missing media `{}`",
                            output.media_id
                        ),
                        "Add the generated output to media/index.json.",
                    ));
                } else if let Some(media_asset) = media_by_id.get(output.media_id.as_str()) {
                    if media_asset.kind != MediaKind::Generated {
                        issues.push(issue(
                            format!("{output_path}.mediaId"),
                            format!(
                                "generated output media id `{}` must reference generated media",
                                output.media_id
                            ),
                            "Update the generated output media id or mark the media entry as generated.",
                        ));
                    }
                    if media_asset.relative_path != output.relative_path {
                        issues.push(issue(
                            format!("{output_path}.relativePath"),
                            format!(
                                "generated output path for media `{}` must match media/index.json",
                                output.media_id
                            ),
                            "Keep generated output paths consistent with media/index.json.",
                        ));
                    }
                    if !nearly_equal(media_asset.duration_seconds, output.duration_seconds) {
                        issues.push(issue(
                            format!("{output_path}.durationSeconds"),
                            format!(
                                "generated output duration for media `{}` must match media/index.json",
                                output.media_id
                            ),
                            "Keep generated output duration consistent with media/index.json.",
                        ));
                    }
                    if media_asset.width != Some(output.width) {
                        issues.push(issue(
                            format!("{output_path}.width"),
                            format!(
                                "generated output width for media `{}` must match media/index.json",
                                output.media_id
                            ),
                            "Keep generated output width consistent with media/index.json.",
                        ));
                    }
                    if media_asset.height != Some(output.height) {
                        issues.push(issue(
                            format!("{output_path}.height"),
                            format!(
                                "generated output height for media `{}` must match media/index.json",
                                output.media_id
                            ),
                            "Keep generated output height consistent with media/index.json.",
                        ));
                    }
                    if !media_asset
                        .fps
                        .is_some_and(|fps| nearly_equal(fps, output.fps))
                    {
                        issues.push(issue(
                            format!("{output_path}.fps"),
                            format!(
                                "generated output fps for media `{}` must match media/index.json",
                                output.media_id
                            ),
                            "Keep generated output fps consistent with media/index.json.",
                        ));
                    }
                }
            }
            if output.relative_path.trim().is_empty() {
                issues.push(issue(
                    format!("{output_path}.relativePath"),
                    "generated output relative path is required",
                    "Record where the generated output file is stored.",
                ));
            } else {
                validate_project_relative_path_value(
                    project_dir,
                    &format!("{output_path}.relativePath"),
                    &format!("{output_path}.relativePath"),
                    &output.relative_path,
                    "Keep generated output paths inside the project folder.",
                    issues,
                );
                if let Some(generated_asset_directory) = &generated_asset_directory {
                    let output_relative_path = Path::new(&output.relative_path);
                    let generated_asset_directory_path = Path::new(generated_asset_directory);
                    let generated_asset_sidecar_path =
                        generated_asset_directory_path.join("asset.json");
                    if output_relative_path == generated_asset_directory_path
                        || !output_relative_path.starts_with(generated_asset_directory_path)
                    {
                        issues.push(issue(
                            format!("{output_path}.relativePath"),
                            format!(
                                "generated output path must stay under `{generated_asset_directory}`"
                            ),
                            "Move the output into the generated asset directory or update the asset sidecar.",
                        ));
                    } else if output_relative_path == generated_asset_sidecar_path {
                        issues.push(issue(
                            format!("{output_path}.relativePath"),
                            "generated output path cannot use reserved asset sidecar `asset.json`",
                            "Move the media output to a separate file under the generated asset directory.",
                        ));
                    }
                }
            }
            if output.width == 0 {
                issues.push(issue(
                    format!("{output_path}.width"),
                    "generated output width must be greater than zero",
                    "Probe the generated output or repair its dimensions.",
                ));
            }
            if output.height == 0 {
                issues.push(issue(
                    format!("{output_path}.height"),
                    "generated output height must be greater than zero",
                    "Probe the generated output or repair its dimensions.",
                ));
            }
            if !output.duration_seconds.is_finite() || output.duration_seconds <= 0.0 {
                issues.push(issue(
                    format!("{output_path}.durationSeconds"),
                    "generated output duration must be finite and greater than zero",
                    "Probe the generated output or repair its duration.",
                ));
            }
            if !output.fps.is_finite() || output.fps <= 0.0 {
                issues.push(issue(
                    format!("{output_path}.fps"),
                    "generated output fps must be finite and greater than zero",
                    "Probe the generated output or repair its frame rate.",
                ));
            }
        }
    }

    for media_asset in &media.assets {
        if media_asset.kind == MediaKind::Generated
            && !generated_output_media_ids.contains(media_asset.id.as_str())
        {
            issues.push(issue(
                format!("media/index.json assets[{}].kind", media_asset.id),
                format!(
                    "generated media `{}` is not backed by a generated asset output",
                    media_asset.id
                ),
                "Add a generated asset output for this media or mark the media as imported.",
            ));
        }
    }

    let mut generated_lineage_edges =
        std::collections::BTreeMap::<String, Vec<(&'static str, String)>>::new();
    for (_, asset_id, parent_asset_id, retry_of_asset_id) in &generated_lineage_references {
        if asset_id.trim().is_empty() {
            continue;
        }
        if let Some(parent_asset_id) = parent_asset_id {
            if generated_ids.contains(parent_asset_id) {
                generated_lineage_edges
                    .entry(asset_id.clone())
                    .or_default()
                    .push(("parentAssetId", parent_asset_id.clone()));
            }
        }
        if let Some(retry_of_asset_id) = retry_of_asset_id {
            if generated_ids.contains(retry_of_asset_id) {
                generated_lineage_edges
                    .entry(asset_id.clone())
                    .or_default()
                    .push(("retryOfAssetId", retry_of_asset_id.clone()));
            }
        }
    }

    for (relative_path, asset_id, parent_asset_id, retry_of_asset_id) in
        generated_lineage_references
    {
        validate_generated_asset_lineage_reference(
            &relative_path,
            "parentAssetId",
            parent_asset_id.as_deref(),
            &generated_ids,
            issues,
        );
        validate_generated_asset_lineage_reference(
            &relative_path,
            "retryOfAssetId",
            retry_of_asset_id.as_deref(),
            &generated_ids,
            issues,
        );
        validate_generated_asset_lineage_cycle(
            &relative_path,
            "parentAssetId",
            &asset_id,
            parent_asset_id.as_deref(),
            &generated_lineage_edges,
            issues,
        );
        validate_generated_asset_lineage_cycle(
            &relative_path,
            "retryOfAssetId",
            &asset_id,
            retry_of_asset_id.as_deref(),
            &generated_lineage_edges,
            issues,
        );
    }

    Ok(())
}

fn allows_promptless_source_video_audio_asset(
    references: &GeneratedAssetReferences,
    settings: &GeneratedAssetSettings,
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

fn validate_generated_asset_index_file(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let generated_dir = safe_join(project_dir, "files.generated", &manifest.files.generated)?;
    let index_path = generated_dir.join(SPLIT_GENERATED_ASSET_INDEX_FILE_NAME);
    if !index_path.exists() {
        return Ok(());
    }

    let index: SplitGeneratedAssetIndexFile = match read_json_file(&index_path) {
        Ok(index) => index,
        Err(error) => {
            issues.push(issue(
                "generated/index.json",
                error.to_string(),
                "Repair generated/index.json or regenerate it from generated asset sidecars.",
            ));
            return Ok(());
        }
    };
    let sidecars = collect_generated_asset_sidecars_for_index(project_dir, &generated_dir)?;
    let mut indexed_asset_ids = BTreeSet::new();

    for entry in &index.assets {
        let entry_path = generated_index_entry_path(&entry.asset_id);
        if entry.asset_id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.assetId"),
                "generated index assetId is required",
                "Regenerate generated/index.json from generated asset sidecars.",
            ));
            continue;
        }
        if !indexed_asset_ids.insert(entry.asset_id.clone()) {
            issues.push(issue(
                format!("{entry_path}.assetId"),
                format!("duplicate generated index assetId `{}`", entry.asset_id),
                "Regenerate generated/index.json from generated asset sidecars.",
            ));
        }
        validate_project_relative_path_value(
            project_dir,
            &format!("{entry_path}.path"),
            &format!("{entry_path}.path"),
            &entry.path,
            "Regenerate generated/index.json with project-relative generated asset sidecar paths.",
            issues,
        );

        let Some((sidecar_path, asset)) = sidecars.get(&entry.asset_id) else {
            issues.push(issue(
                format!("{entry_path}.path"),
                format!(
                    "generated index entry references missing generated asset sidecar `{}`",
                    entry.path
                ),
                "Regenerate generated/index.json from generated asset sidecars.",
            ));
            continue;
        };

        validate_generated_index_field(&entry_path, "path", &entry.path, sidecar_path, issues);
        validate_generated_index_field(
            &entry_path,
            "status",
            &entry.status,
            generated_asset_status_label(&asset.status),
            issues,
        );
        validate_generated_index_optional_field(
            &entry_path,
            "name",
            entry.name.as_deref(),
            asset.name.as_deref(),
            issues,
        );
        validate_generated_index_field(
            &entry_path,
            "modelProvider",
            &entry.model_provider,
            &asset.model.provider,
            issues,
        );
        validate_generated_index_field(
            &entry_path,
            "modelId",
            &entry.model_id,
            &asset.model.id,
            issues,
        );
        validate_generated_index_optional_field(
            &entry_path,
            "targetFolderId",
            entry.target_folder_id.as_deref(),
            asset.target_folder_id.as_deref(),
            issues,
        );
        validate_generated_index_optional_field(
            &entry_path,
            "placementIntent",
            entry.placement_intent.as_deref(),
            asset.placement_intent.as_deref(),
            issues,
        );
        if entry.output_count != asset.outputs.len() {
            issues.push(issue(
                format!("{entry_path}.outputCount"),
                format!(
                    "generated index outputCount `{}` must match generated asset output count `{}`",
                    entry.output_count,
                    asset.outputs.len()
                ),
                "Regenerate generated/index.json from generated asset sidecars.",
            ));
        }
        validate_generated_index_field(
            &entry_path,
            "createdAt",
            &entry.created_at,
            &asset.created_at,
            issues,
        );
    }

    for asset_id in sidecars.keys() {
        if !indexed_asset_ids.contains(asset_id) {
            issues.push(issue(
                format!("generated/index.json assets[{asset_id}]"),
                format!("missing generated asset index entry for `{asset_id}`"),
                "Regenerate generated/index.json from generated asset sidecars.",
            ));
        }
    }

    Ok(())
}

fn collect_generated_asset_sidecars_for_index(
    project_dir: &Path,
    generated_dir: &Path,
) -> Result<BTreeMap<String, (String, GeneratedAsset)>, SplitProjectError> {
    if !generated_dir.exists() {
        return Ok(BTreeMap::new());
    }

    let mut generated_paths = std::fs::read_dir(generated_dir)
        .map_err(|error| SplitProjectError::Io {
            path: generated_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path().join("asset.json")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: generated_dir.display().to_string(),
            message: error.to_string(),
        })?;
    generated_paths.sort();

    let mut sidecars = BTreeMap::new();
    for path in generated_paths {
        if !path.exists() {
            continue;
        }
        let Ok(asset) = read_json_file::<GeneratedAsset>(&path) else {
            continue;
        };
        if asset.id.trim().is_empty() {
            continue;
        }
        sidecars.insert(
            asset.id.clone(),
            (project_relative_path(project_dir, &path), asset),
        );
    }

    Ok(sidecars)
}

fn generated_index_entry_path(asset_id: &str) -> String {
    if asset_id.trim().is_empty() {
        "generated/index.json assets[]".to_string()
    } else {
        format!("generated/index.json assets[{asset_id}]")
    }
}

fn validate_generated_index_field(
    entry_path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "generated index {field} `{actual}` must match generated asset sidecar `{expected}`"
            ),
            "Regenerate generated/index.json from generated asset sidecars.",
        ));
    }
}

fn validate_generated_index_optional_field(
    entry_path: &str,
    field: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "generated index {field} `{}` must match generated asset sidecar `{}`",
                actual.unwrap_or("null"),
                expected.unwrap_or("null")
            ),
            "Regenerate generated/index.json from generated asset sidecars.",
        ));
    }
}

fn validate_template_override_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let templates_dir = safe_join(project_dir, "files.templates", &manifest.files.templates)?;
    if !templates_dir.exists() {
        return Ok(());
    }

    let mut template_ids = std::collections::BTreeSet::new();
    let mut sidecar_templates = BTreeMap::new();
    let mut template_paths = std::fs::read_dir(&templates_dir)
        .map_err(|error| SplitProjectError::Io {
            path: templates_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: templates_dir.display().to_string(),
            message: error.to_string(),
        })?;
    template_paths.sort();

    for path in template_paths {
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if is_template_index_path(&path) {
            continue;
        }

        let relative_path = project_relative_path(project_dir, &path);
        let template_override: ProjectTemplateOverride = match read_json_file(&path) {
            Ok(template_override) => template_override,
            Err(error) => {
                issues.push(issue(
                    relative_path,
                    error.to_string(),
                    "Repair template override JSON.",
                ));
                continue;
            }
        };

        if !template_override.template_id.trim().is_empty() {
            sidecar_templates.insert(
                template_override.template_id.clone(),
                (relative_path.clone(), template_override.clone()),
            );
        }

        if template_override.schema_version != SPLIT_TEMPLATE_SCHEMA_VERSION {
            issues.push(issue(
                format!("{relative_path} schemaVersion"),
                format!(
                    "template override schemaVersion must be {}",
                    SPLIT_TEMPLATE_SCHEMA_VERSION
                ),
                "Update the template override file to the supported schema version.",
            ));
        }
        if template_override.template_id.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} templateId"),
                "template override templateId is required",
                "Set the template id this override customizes.",
            ));
        } else {
            if let Some(file_template_id) = path.file_stem().and_then(|stem| stem.to_str()) {
                if file_template_id != template_override.template_id {
                    issues.push(issue(
                        format!("{relative_path} templateId"),
                        format!(
                            "template override id `{}` must match filename `{file_template_id}`",
                            template_override.template_id
                        ),
                        "Rename the template override file or update templateId to match.",
                    ));
                }
            }
            if !template_ids.insert(template_override.template_id.clone()) {
                issues.push(issue(
                    format!("{relative_path} templateId"),
                    format!(
                        "duplicate template override id `{}`",
                        template_override.template_id
                    ),
                    "Use unique template override ids.",
                ));
            }
        }
        if template_override.name.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} name"),
                "template override name is required",
                "Give the project template override a human-readable name.",
            ));
        }
        for (field, value) in [
            ("visualTreatment", &template_override.visual_treatment),
            ("motion", &template_override.motion),
            ("safeZone", &template_override.safe_zone),
            ("avoid", &template_override.avoid),
        ] {
            if value.trim().is_empty() {
                issues.push(issue(
                    format!("{relative_path} {field}"),
                    "required template visual metadata is missing",
                    "Record visualTreatment, motion, safeZone, and avoid for every template override.",
                ));
            }
        }
    }

    validate_template_index_file(project_dir, &templates_dir, &sidecar_templates, issues)?;

    Ok(())
}

fn validate_template_index_file(
    project_dir: &Path,
    templates_dir: &Path,
    sidecar_templates: &BTreeMap<String, (String, ProjectTemplateOverride)>,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let index_path = templates_dir.join(SPLIT_TEMPLATE_INDEX_FILE_NAME);
    if !index_path.exists() {
        return Ok(());
    }

    let index: SplitTemplateIndexFile = match read_json_file(&index_path) {
        Ok(index) => index,
        Err(error) => {
            issues.push(issue(
                "templates/index.json",
                error.to_string(),
                "Repair templates/index.json or regenerate it from template override sidecars.",
            ));
            return Ok(());
        }
    };
    let mut indexed_template_ids = BTreeSet::new();

    for entry in &index.templates {
        let entry_path = template_index_entry_path(&entry.template_id);
        if entry.template_id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.templateId"),
                "template index templateId is required",
                "Regenerate templates/index.json from template override sidecars.",
            ));
            continue;
        }
        if !indexed_template_ids.insert(entry.template_id.clone()) {
            issues.push(issue(
                format!("{entry_path}.templateId"),
                format!(
                    "duplicate template index templateId `{}`",
                    entry.template_id
                ),
                "Regenerate templates/index.json from template override sidecars.",
            ));
        }
        validate_project_relative_path_value(
            project_dir,
            &format!("{entry_path}.path"),
            &format!("{entry_path}.path"),
            &entry.path,
            "Regenerate templates/index.json with project-relative template override paths.",
            issues,
        );

        let Some((sidecar_path, template)) = sidecar_templates.get(&entry.template_id) else {
            issues.push(issue(
                format!("{entry_path}.path"),
                format!(
                    "template index entry references missing template override sidecar `{}`",
                    entry.path
                ),
                "Regenerate templates/index.json from template override sidecars.",
            ));
            continue;
        };

        validate_template_index_field(&entry_path, "name", &entry.name, &template.name, issues);
        validate_template_index_field(&entry_path, "path", &entry.path, sidecar_path, issues);
        validate_template_index_field(
            &entry_path,
            "visualTreatment",
            &entry.visual_treatment,
            &template.visual_treatment,
            issues,
        );
        validate_template_index_field(
            &entry_path,
            "motion",
            &entry.motion,
            &template.motion,
            issues,
        );
        validate_template_index_field(
            &entry_path,
            "safeZone",
            &entry.safe_zone,
            &template.safe_zone,
            issues,
        );
        validate_template_index_field(&entry_path, "avoid", &entry.avoid, &template.avoid, issues);
    }

    for template_id in sidecar_templates.keys() {
        if !indexed_template_ids.contains(template_id) {
            issues.push(issue(
                format!("templates/index.json templates[{template_id}]"),
                format!("missing template index entry for `{template_id}`"),
                "Regenerate templates/index.json from template override sidecars.",
            ));
        }
    }

    Ok(())
}

fn template_index_entry_path(template_id: &str) -> String {
    if template_id.trim().is_empty() {
        "templates/index.json templates[]".to_string()
    } else {
        format!("templates/index.json templates[{template_id}]")
    }
}

fn validate_template_index_field(
    entry_path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "template index {field} `{actual}` must match template override sidecar `{expected}`"
            ),
            "Regenerate templates/index.json from template override sidecars.",
        ));
    }
}

fn validate_project_relative_path_value(
    project_dir: &Path,
    issue_path: &str,
    field: &str,
    value: &str,
    fix: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if let Err(error) = safe_join(project_dir, field, value) {
        issues.push(issue(issue_path, error.to_string(), fix));
    }
}

/// Export artifacts are project-relative, or absolute when the export was saved
/// to a folder outside the project (action validation ties that folder to the
/// export job). Absolute paths are checked lexically only.
fn validate_export_artifact_path_value(
    project_dir: &Path,
    issue_path: &str,
    value: &str,
    fix: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let path = Path::new(value);
    if !path.is_absolute() {
        validate_project_relative_path_value(
            project_dir,
            issue_path,
            issue_path,
            value,
            fix,
            issues,
        );
        return;
    }
    let lexically_clean = path.components().all(|component| {
        matches!(
            component,
            std::path::Component::RootDir
                | std::path::Component::Prefix(_)
                | std::path::Component::Normal(_)
        )
    });
    if !lexically_clean {
        issues.push(issue(
            issue_path,
            "Export artifact paths outside the project must be absolute without parent components.",
            fix,
        ));
    }
}

fn validate_generated_media_reference(
    path: &str,
    field: &str,
    media_id: &str,
    media_ids: &std::collections::BTreeSet<&str>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if media_id.trim().is_empty() {
        issues.push(issue(
            format!("{path} {field}"),
            "generated asset media reference is required",
            "Set a valid media id or remove the empty reference.",
        ));
    } else if !media_ids.contains(media_id) {
        issues.push(issue(
            format!("{path} {field}"),
            format!("generated asset references missing media `{media_id}`"),
            "Add the referenced media asset or update the generated asset reference.",
        ));
    }
}

fn validate_generated_frame_reference(
    path: &str,
    field: &str,
    media_id: &str,
    media_ids: &std::collections::BTreeSet<&str>,
    media_by_id: &std::collections::BTreeMap<&str, &super::model::MediaAsset>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    validate_generated_media_reference(path, field, media_id, media_ids, issues);

    let Some(media_asset) = media_by_id.get(media_id) else {
        return;
    };
    if media_asset.kind == MediaKind::Audio {
        issues.push(issue(
            format!("{path} {field}"),
            format!("generated frame reference `{media_id}` must point to visual media"),
            "Use an image, video, or generated visual media reference for first and last frames.",
        ));
    }
}

fn validate_generated_video_reference(
    path: &str,
    field: &str,
    media_id: &str,
    media_ids: &std::collections::BTreeSet<&str>,
    media_by_id: &std::collections::BTreeMap<&str, &super::model::MediaAsset>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    validate_generated_media_reference(path, field, media_id, media_ids, issues);

    let Some(media_asset) = media_by_id.get(media_id) else {
        return;
    };
    if media_asset.kind != MediaKind::Video {
        issues.push(issue(
            format!("{path} {field}"),
            format!("generated video reference `{media_id}` must point to video media"),
            "Use a video media asset for source video and video reference fields.",
        ));
    }
}

fn validate_generated_audio_reference(
    path: &str,
    field: &str,
    media_id: &str,
    media_ids: &std::collections::BTreeSet<&str>,
    media_by_id: &std::collections::BTreeMap<&str, &super::model::MediaAsset>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    validate_generated_media_reference(path, field, media_id, media_ids, issues);

    let Some(media_asset) = media_by_id.get(media_id) else {
        return;
    };
    if media_asset.kind != MediaKind::Audio {
        issues.push(issue(
            format!("{path} {field}"),
            format!("generated audio reference `{media_id}` must point to audio media"),
            "Use an audio media asset for audio reference fields.",
        ));
    }
}

fn validate_generated_settings(
    path: &str,
    settings: &GeneratedAssetSettings,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if settings.width == Some(0) {
        issues.push(issue(
            format!("{path} settings.width"),
            "generated asset settings width must be greater than zero",
            "Set a positive generated width or remove the width setting.",
        ));
    }
    if settings.height == Some(0) {
        issues.push(issue(
            format!("{path} settings.height"),
            "generated asset settings height must be greater than zero",
            "Set a positive generated height or remove the height setting.",
        ));
    }
    if settings.width.is_some() != settings.height.is_some() {
        issues.push(issue(
            format!("{path} settings"),
            "generated asset settings width and height must be set together",
            "Set both width and height, or remove both settings.",
        ));
    }
    if settings
        .duration_seconds
        .is_some_and(|duration| !duration.is_finite() || duration <= 0.0)
    {
        issues.push(issue(
            format!("{path} settings.durationSeconds"),
            "generated asset settings durationSeconds must be finite and greater than zero",
            "Set a positive generation duration or remove the duration setting.",
        ));
    }
    if settings
        .fps
        .is_some_and(|fps| !fps.is_finite() || fps <= 0.0)
    {
        issues.push(issue(
            format!("{path} settings.fps"),
            "generated asset settings fps must be finite and greater than zero",
            "Set a positive generation fps or remove the fps setting.",
        ));
    }
    if settings
        .aspect_ratio
        .as_ref()
        .is_some_and(|aspect_ratio| !is_valid_generated_asset_aspect_ratio(aspect_ratio))
    {
        issues.push(issue(
            format!("{path} settings.aspectRatio"),
            "generated asset settings aspectRatio must be a ratio such as 16:9 or auto",
            "Set a valid aspect ratio, use auto, or remove the aspectRatio setting.",
        ));
    }
}

fn is_valid_generated_asset_aspect_ratio(aspect_ratio: &str) -> bool {
    let aspect_ratio = aspect_ratio.trim();
    !aspect_ratio.is_empty() && (aspect_ratio == "auto" || aspect_ratio.contains(':'))
}

fn validate_generated_asset_lineage_reference(
    path: &str,
    field: &str,
    reference_id: Option<&str>,
    generated_ids: &std::collections::BTreeSet<String>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let Some(reference_id) = reference_id else {
        return;
    };
    if !generated_ids.contains(reference_id) {
        issues.push(issue(
            format!("{path} {field}"),
            format!("generated asset lineage references missing asset `{reference_id}`"),
            "Add the referenced generated asset or update the lineage reference.",
        ));
    }
}

fn validate_generated_asset_lineage_cycle(
    path: &str,
    field: &str,
    asset_id: &str,
    reference_id: Option<&str>,
    lineage_edges: &std::collections::BTreeMap<String, Vec<(&'static str, String)>>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let Some(reference_id) = reference_id else {
        return;
    };
    let mut visited = std::collections::BTreeSet::new();
    let mut stack = vec![reference_id];

    while let Some(current_id) = stack.pop() {
        if current_id == asset_id {
            issues.push(issue(
                format!("{path} {field}"),
                format!("generated asset lineage cycle includes `{reference_id}`"),
                "Remove or repair the generated asset lineage reference that creates the cycle.",
            ));
            return;
        }
        if !visited.insert(current_id) {
            continue;
        }
        if let Some(edges) = lineage_edges.get(current_id) {
            for (_, next_id) in edges {
                stack.push(next_id);
            }
        }
    }
}

fn validate_render_report_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let renders_dir = safe_join(project_dir, "files.renders", &manifest.files.renders)?;
    if !renders_dir.exists() {
        return Ok(());
    }

    let mut render_report_ids = std::collections::BTreeSet::new();
    let mut sidecar_reports = BTreeMap::new();
    let mut report_paths = std::fs::read_dir(&renders_dir)
        .map_err(|error| SplitProjectError::Io {
            path: renders_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path().join("report.json")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: renders_dir.display().to_string(),
            message: error.to_string(),
        })?;
    report_paths.sort();

    for path in report_paths {
        if !path.exists() {
            continue;
        }

        let relative_path = project_relative_path(project_dir, &path);
        let render_report: ProjectRenderReport = match read_json_file(&path) {
            Ok(render_report) => render_report,
            Err(error) => {
                issues.push(issue(
                    relative_path,
                    error.to_string(),
                    "Repair render report JSON.",
                ));
                continue;
            }
        };
        if !render_report.id.trim().is_empty() {
            sidecar_reports.insert(
                render_report.id.clone(),
                (relative_path.clone(), render_report.clone()),
            );
        }

        if render_report.schema_version != SPLIT_RENDER_REPORT_SCHEMA_VERSION {
            issues.push(issue(
                format!("{relative_path} schemaVersion"),
                format!(
                    "render report schemaVersion must be {}",
                    SPLIT_RENDER_REPORT_SCHEMA_VERSION
                ),
                "Update the render report file to the supported schema version.",
            ));
        }
        if render_report.id.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} id"),
                "render report id is required",
                "Set a stable render report id.",
            ));
        } else {
            if let Some(directory_id) = path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
            {
                if directory_id != render_report.id {
                    issues.push(issue(
                        format!("{relative_path} id"),
                        format!(
                            "render report id `{}` must match directory `{directory_id}`",
                            render_report.id
                        ),
                        "Move the render report file or update its id to match the directory.",
                    ));
                }
            }
            if !render_report_ids.insert(render_report.id.clone()) {
                issues.push(issue(
                    format!("{relative_path} id"),
                    format!("duplicate render report id `{}`", render_report.id),
                    "Use unique render report ids.",
                ));
            }
        }
        if render_report.output_path.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} outputPath"),
                "render output path is required",
                "Record the rendered MP4 output path.",
            ));
        } else {
            let render_report_directory = path
                .parent()
                .map(|directory| project_relative_path(project_dir, directory));
            validate_project_relative_path_value(
                project_dir,
                &format!("{relative_path} outputPath"),
                &format!("{relative_path} outputPath"),
                &render_report.output_path,
                "Keep render output paths inside the project folder.",
                issues,
            );
            if let Some(render_report_directory) = &render_report_directory {
                if !Path::new(&render_report.output_path)
                    .starts_with(Path::new(render_report_directory))
                {
                    issues.push(issue(
                        format!("{relative_path} outputPath"),
                        format!("render output path must stay under `{render_report_directory}`"),
                        "Move the render output into the render report directory or update the report.",
                    ));
                }
            }
        }
        if !render_report.duration_seconds.is_finite() || render_report.duration_seconds <= 0.0 {
            issues.push(issue(
                format!("{relative_path} durationSeconds"),
                "render duration must be finite and greater than zero",
                "Probe the draft MP4 and record its duration.",
            ));
        }
        if !render_report.streams.video && !render_report.streams.audio {
            issues.push(issue(
                format!("{relative_path} streams"),
                "render report must record at least one output stream",
                "Probe the draft MP4 and record video/audio stream presence.",
            ));
        }
        for required_check in [
            "duration",
            "streams",
            "captionAlignment",
            "overlayTiming",
            "artifactPaths",
            "logPath",
        ] {
            if !render_report.checks.contains_key(required_check) {
                issues.push(issue(
                    format!("{relative_path} checks.{required_check}"),
                    "render report is missing a required review check",
                    "Record the duration, stream, caption alignment, overlay timing, artifact path, and log path checks.",
                ));
            }
        }
        if render_report.artifacts.is_empty() {
            issues.push(issue(
                format!("{relative_path} artifacts"),
                "render report must include artifact paths",
                "Record generated MP4, log, and sidecar artifact paths.",
            ));
        } else if !render_report
            .artifacts
            .iter()
            .any(|artifact| artifact == &render_report.output_path)
        {
            issues.push(issue(
                format!("{relative_path} artifacts"),
                format!(
                    "render artifacts must include outputPath `{}`",
                    render_report.output_path
                ),
                "Add the rendered output path to artifacts.",
            ));
        }
        for (index, artifact) in render_report.artifacts.iter().enumerate() {
            if artifact.trim().is_empty() {
                issues.push(issue(
                    format!("{relative_path} artifacts[{index}]"),
                    "render artifact path must not be empty",
                    "Remove the empty artifact entry or record its path.",
                ));
            } else {
                let render_report_directory = path
                    .parent()
                    .map(|directory| project_relative_path(project_dir, directory));
                validate_project_relative_path_value(
                    project_dir,
                    &format!("{relative_path} artifacts[{index}]"),
                    &format!("{relative_path} artifacts[{index}]"),
                    artifact,
                    "Keep render artifact paths inside the project folder.",
                    issues,
                );
                if let Some(render_report_directory) = &render_report_directory {
                    if !Path::new(artifact).starts_with(Path::new(render_report_directory)) {
                        issues.push(issue(
                            format!("{relative_path} artifacts[{index}]"),
                            format!(
                                "render artifact path must stay under `{render_report_directory}`"
                            ),
                            "Move the render artifact into the render report directory or update the report.",
                        ));
                    }
                }
            }
        }
        if render_report.log_path.trim().is_empty() {
            issues.push(issue(
                format!("{relative_path} logPath"),
                "render log path is required",
                "Record the ffmpeg/job log path for troubleshooting.",
            ));
        } else {
            validate_project_relative_path_value(
                project_dir,
                &format!("{relative_path} logPath"),
                &format!("{relative_path} logPath"),
                &render_report.log_path,
                "Keep render log paths inside the project folder.",
                issues,
            );
        }
    }

    validate_render_report_index_file(project_dir, &renders_dir, &sidecar_reports, issues)?;

    Ok(())
}

fn validate_render_report_index_file(
    project_dir: &Path,
    renders_dir: &Path,
    sidecar_reports: &BTreeMap<String, (String, ProjectRenderReport)>,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let index_path = renders_dir.join(SPLIT_RENDER_REPORT_INDEX_FILE_NAME);
    if !index_path.exists() {
        return Ok(());
    }

    let index: SplitRenderReportIndexFile = match read_json_file(&index_path) {
        Ok(index) => index,
        Err(error) => {
            issues.push(issue(
                "renders/index.json",
                error.to_string(),
                "Repair renders/index.json or regenerate it from render report sidecars.",
            ));
            return Ok(());
        }
    };
    let mut indexed_report_ids = BTreeSet::new();

    for entry in &index.reports {
        let entry_path = render_report_index_entry_path(&entry.report_id);
        if entry.report_id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.reportId"),
                "render report index reportId is required",
                "Regenerate renders/index.json from render report sidecars.",
            ));
            continue;
        }
        if !indexed_report_ids.insert(entry.report_id.clone()) {
            issues.push(issue(
                format!("{entry_path}.reportId"),
                format!(
                    "duplicate render report index reportId `{}`",
                    entry.report_id
                ),
                "Regenerate renders/index.json from render report sidecars.",
            ));
        }
        validate_project_relative_path_value(
            project_dir,
            &format!("{entry_path}.path"),
            &format!("{entry_path}.path"),
            &entry.path,
            "Regenerate renders/index.json with project-relative render report sidecar paths.",
            issues,
        );

        let Some((sidecar_path, report)) = sidecar_reports.get(&entry.report_id) else {
            issues.push(issue(
                format!("{entry_path}.path"),
                format!(
                    "render report index entry references missing render report sidecar `{}`",
                    entry.path
                ),
                "Regenerate renders/index.json from render report sidecars.",
            ));
            continue;
        };

        validate_render_report_index_field(&entry_path, "path", &entry.path, sidecar_path, issues);
        validate_render_report_index_field(
            &entry_path,
            "status",
            &entry.status,
            render_report_status_label(&report.status),
            issues,
        );
        validate_render_report_index_field(
            &entry_path,
            "outputPath",
            &entry.output_path,
            &report.output_path,
            issues,
        );
        validate_render_report_index_f64_field(
            &entry_path,
            "durationSeconds",
            entry.duration_seconds,
            report.duration_seconds,
            issues,
        );
        validate_render_report_index_bool_field(
            &entry_path,
            "video",
            entry.video,
            report.streams.video,
            issues,
        );
        validate_render_report_index_bool_field(
            &entry_path,
            "audio",
            entry.audio,
            report.streams.audio,
            issues,
        );
        validate_render_report_index_usize_field(
            &entry_path,
            "checkCount",
            entry.check_count,
            report.checks.len(),
            issues,
        );
        validate_render_report_index_usize_field(
            &entry_path,
            "failedCheckCount",
            entry.failed_check_count,
            report
                .checks
                .values()
                .filter(|status| matches!(status, RenderReportCheckStatus::Failed))
                .count(),
            issues,
        );
        validate_render_report_index_usize_field(
            &entry_path,
            "artifactCount",
            entry.artifact_count,
            report.artifacts.len(),
            issues,
        );
        validate_render_report_index_field(
            &entry_path,
            "logPath",
            &entry.log_path,
            &report.log_path,
            issues,
        );
        validate_render_report_index_field(
            &entry_path,
            "createdAt",
            &entry.created_at,
            &report.created_at,
            issues,
        );
    }

    for report_id in sidecar_reports.keys() {
        if !indexed_report_ids.contains(report_id) {
            issues.push(issue(
                format!("renders/index.json reports[{report_id}]"),
                format!("missing render report index entry for `{report_id}`"),
                "Regenerate renders/index.json from render report sidecars.",
            ));
        }
    }

    Ok(())
}

fn render_report_index_entry_path(report_id: &str) -> String {
    if report_id.trim().is_empty() {
        "renders/index.json reports[]".to_string()
    } else {
        format!("renders/index.json reports[{report_id}]")
    }
}

fn validate_render_report_index_field(
    entry_path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "render report index {field} `{actual}` must match render report sidecar `{expected}`"
            ),
            "Regenerate renders/index.json from render report sidecars.",
        ));
    }
}

fn validate_render_report_index_usize_field(
    entry_path: &str,
    field: &str,
    actual: usize,
    expected: usize,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "render report index {field} `{actual}` must match render report sidecar `{expected}`"
            ),
            "Regenerate renders/index.json from render report sidecars.",
        ));
    }
}

fn validate_render_report_index_bool_field(
    entry_path: &str,
    field: &str,
    actual: bool,
    expected: bool,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "render report index {field} `{actual}` must match render report sidecar `{expected}`"
            ),
            "Regenerate renders/index.json from render report sidecars.",
        ));
    }
}

fn validate_render_report_index_f64_field(
    entry_path: &str,
    field: &str,
    actual: f64,
    expected: f64,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if !nearly_equal(actual, expected) {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "render report index {field} `{actual}` must match render report sidecar `{expected}`"
            ),
            "Regenerate renders/index.json from render report sidecars.",
        ));
    }
}

fn validate_export_artifact_index_file(
    project_dir: &Path,
    export_artifacts_dir: &Path,
    sidecar_artifacts: &BTreeMap<String, (String, ProjectExportArtifact)>,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let index_path = export_artifacts_dir.join(SPLIT_EXPORT_ARTIFACT_INDEX_FILE_NAME);
    if !index_path.exists() {
        return Ok(());
    }

    let index: SplitExportArtifactIndexFile = match read_json_file(&index_path) {
        Ok(index) => index,
        Err(error) => {
            issues.push(issue(
                "exports/index.json",
                error.to_string(),
                "Repair exports/index.json or regenerate it from export artifact sidecars.",
            ));
            return Ok(());
        }
    };
    let mut indexed_artifact_ids = BTreeSet::new();

    for entry in &index.artifacts {
        let entry_path = export_artifact_index_entry_path(&entry.artifact_id);
        if entry.artifact_id.trim().is_empty() {
            issues.push(issue(
                format!("{entry_path}.artifactId"),
                "export artifact index artifactId is required",
                "Regenerate exports/index.json from export artifact sidecars.",
            ));
            continue;
        }
        if !indexed_artifact_ids.insert(entry.artifact_id.clone()) {
            issues.push(issue(
                format!("{entry_path}.artifactId"),
                format!(
                    "duplicate export artifact index artifactId `{}`",
                    entry.artifact_id
                ),
                "Regenerate exports/index.json from export artifact sidecars.",
            ));
        }
        validate_export_artifact_path_value(
            project_dir,
            &format!("{entry_path}.path"),
            &entry.path,
            "Regenerate exports/index.json with project-relative export artifact paths.",
            issues,
        );

        let Some((_sidecar_path, artifact)) = sidecar_artifacts.get(&entry.artifact_id) else {
            issues.push(issue(
                format!("{entry_path}.path"),
                format!(
                    "export artifact index entry references missing export artifact sidecar `{}`",
                    entry.artifact_id
                ),
                "Regenerate exports/index.json from export artifact sidecars.",
            ));
            continue;
        };

        validate_export_artifact_index_field(
            &entry_path,
            "kind",
            &entry.kind,
            export_artifact_kind_label(&artifact.kind),
            issues,
        );
        validate_export_artifact_index_field(
            &entry_path,
            "format",
            &entry.format,
            &artifact.format,
            issues,
        );
        validate_export_artifact_index_field(
            &entry_path,
            "path",
            &entry.path,
            &artifact.path,
            issues,
        );
        validate_export_artifact_index_field(
            &entry_path,
            "mimeType",
            &entry.mime_type,
            &artifact.mime_type,
            issues,
        );
        validate_export_artifact_index_optional_field(
            &entry_path,
            "jobId",
            &entry.job_id,
            &artifact.job_id,
            issues,
        );
        validate_export_artifact_index_field(
            &entry_path,
            "createdAt",
            &entry.created_at,
            &artifact.created_at,
            issues,
        );
    }

    for artifact_id in sidecar_artifacts.keys() {
        if !indexed_artifact_ids.contains(artifact_id) {
            issues.push(issue(
                format!("exports/index.json artifacts[{artifact_id}]"),
                format!("missing export artifact index entry for `{artifact_id}`"),
                "Regenerate exports/index.json from export artifact sidecars.",
            ));
        }
    }

    Ok(())
}

fn export_artifact_index_entry_path(artifact_id: &str) -> String {
    if artifact_id.trim().is_empty() {
        "exports/index.json artifacts[]".to_string()
    } else {
        format!("exports/index.json artifacts[{artifact_id}]")
    }
}

fn validate_export_artifact_index_field(
    entry_path: &str,
    field: &str,
    actual: &str,
    expected: &str,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "export artifact index {field} `{actual}` must match export artifact sidecar `{expected}`"
            ),
            "Regenerate exports/index.json from export artifact sidecars.",
        ));
    }
}

fn validate_export_artifact_index_optional_field(
    entry_path: &str,
    field: &str,
    actual: &Option<String>,
    expected: &Option<String>,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if actual != expected {
        let actual = actual.as_deref().unwrap_or("null");
        let expected = expected.as_deref().unwrap_or("null");
        issues.push(issue(
            format!("{entry_path}.{field}"),
            format!(
                "export artifact index {field} `{actual}` must match export artifact sidecar `{expected}`"
            ),
            "Regenerate exports/index.json from export artifact sidecars.",
        ));
    }
}

fn validate_export_artifact_files(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) -> Result<(), SplitProjectError> {
    let export_artifacts_dir = project_dir.join(SPLIT_EXPORT_ARTIFACT_INDEX_DIR_NAME);
    if !export_artifacts_dir.exists() {
        validate_manifest_export_artifacts(project_dir, manifest, issues);
        return Ok(());
    }

    let mut artifact_paths = std::fs::read_dir(&export_artifacts_dir)
        .map_err(|error| SplitProjectError::Io {
            path: export_artifacts_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path().join(SPLIT_EXPORT_ARTIFACT_FILE_NAME)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: export_artifacts_dir.display().to_string(),
            message: error.to_string(),
        })?;
    artifact_paths.sort();

    if !artifact_paths.iter().any(|path| path.exists()) {
        validate_manifest_export_artifacts(project_dir, manifest, issues);
        return Ok(());
    }

    let mut artifact_ids = BTreeSet::new();
    let mut sidecar_artifacts = BTreeMap::new();
    for path in artifact_paths {
        if !path.exists() {
            continue;
        }
        let relative_path = project_relative_path(project_dir, &path);
        let artifact: ProjectExportArtifact = match read_json_file(&path) {
            Ok(artifact) => artifact,
            Err(error) => {
                issues.push(issue(
                    relative_path,
                    error.to_string(),
                    "Repair export artifact sidecar JSON.",
                ));
                continue;
            }
        };

        validate_export_artifact(project_dir, &relative_path, &artifact, issues);
        if path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            != Some(artifact.id.as_str())
        {
            issues.push(issue(
                format!("{relative_path} id"),
                "export artifact id must match containing directory",
                "Rename the export artifact directory or update the artifact id.",
            ));
        }
        if !artifact.id.trim().is_empty() && is_safe_manifest_segment(&artifact.id) {
            if artifact_ids.insert(artifact.id.clone()) {
                sidecar_artifacts.insert(
                    artifact.id.clone(),
                    (relative_path.clone(), artifact.clone()),
                );
            } else {
                issues.push(issue(
                    format!("{relative_path} id"),
                    format!("duplicate export artifact id `{}`", artifact.id),
                    "Use unique export artifact ids.",
                ));
            }
        }
    }

    validate_export_artifact_index_file(
        project_dir,
        &export_artifacts_dir,
        &sidecar_artifacts,
        issues,
    )?;

    Ok(())
}

fn validate_manifest_export_artifacts(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    let mut artifact_ids = BTreeSet::new();

    for (index, artifact) in manifest.export_artifacts.iter().enumerate() {
        let base_path = format!("video-creater.project.json exportArtifacts[{index}]");
        validate_export_artifact(project_dir, &base_path, artifact, issues);
        if !artifact.id.trim().is_empty()
            && is_safe_manifest_segment(&artifact.id)
            && !artifact_ids.insert(artifact.id.clone())
        {
            issues.push(issue(
                format!("{base_path}.id"),
                format!("duplicate export artifact id `{}`", artifact.id),
                "Use unique export artifact ids in video-creater.project.json.",
            ));
        }
    }
}

fn validate_export_artifact(
    project_dir: &Path,
    base_path: &str,
    artifact: &ProjectExportArtifact,
    issues: &mut Vec<ProjectValidationIssue>,
) {
    if artifact.schema_version != SPLIT_EXPORT_ARTIFACT_SCHEMA_VERSION {
        issues.push(issue(
            export_artifact_field_path(base_path, "schemaVersion"),
            format!(
                "export artifact schemaVersion must be {}",
                SPLIT_EXPORT_ARTIFACT_SCHEMA_VERSION
            ),
            "Update the export artifact metadata to the supported schema version.",
        ));
    }
    if artifact.id.trim().is_empty() {
        issues.push(issue(
            export_artifact_field_path(base_path, "id"),
            "export artifact id is required",
            "Set a stable export artifact id.",
        ));
    } else if !is_safe_manifest_segment(&artifact.id) {
        issues.push(issue(
            export_artifact_field_path(base_path, "id"),
            format!(
                "export artifact id `{}` must be a safe path segment",
                artifact.id
            ),
            "Use an artifact id without slashes, backslashes, or parent path components.",
        ));
    }
    if artifact.format.trim().is_empty() {
        issues.push(issue(
            export_artifact_field_path(base_path, "format"),
            "export artifact format is required",
            "Record the export format such as h264, prores, or premiereXmeml.",
        ));
    }
    if artifact.path.trim().is_empty() {
        issues.push(issue(
            export_artifact_field_path(base_path, "path"),
            "export artifact path is required",
            "Record the project-relative exported artifact path.",
        ));
    } else {
        validate_export_artifact_path_value(
            project_dir,
            &export_artifact_field_path(base_path, "path"),
            &artifact.path,
            "Keep export artifact paths inside the project folder.",
            issues,
        );
    }
    if artifact.mime_type.trim().is_empty() {
        issues.push(issue(
            export_artifact_field_path(base_path, "mimeType"),
            "export artifact mimeType is required",
            "Record the exported artifact MIME type.",
        ));
    }
    if artifact.created_at.trim().is_empty() {
        issues.push(issue(
            export_artifact_field_path(base_path, "createdAt"),
            "export artifact createdAt is required",
            "Record when the export artifact was created.",
        ));
    }
    if artifact
        .job_id
        .as_ref()
        .is_some_and(|job_id| job_id.trim().is_empty())
    {
        issues.push(issue(
            export_artifact_field_path(base_path, "jobId"),
            "export artifact jobId cannot be empty",
            "Use null until a job id exists, or record the non-empty job id.",
        ));
    }
}

fn export_artifact_field_path(base_path: &str, field: &str) -> String {
    if base_path.starts_with("video-creater.project.json ") {
        format!("{base_path}.{field}")
    } else {
        format!("{base_path} {field}")
    }
}

fn project_relative_path(project_dir: &Path, path: &Path) -> String {
    path.strip_prefix(project_dir)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn latest_generated_asset_id(generated_assets: &[GeneratedAsset]) -> Option<String> {
    generated_assets
        .iter()
        .max_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|asset| asset.id.clone())
}

fn latest_render_report_id(render_reports: &[ProjectRenderReport]) -> Option<String> {
    render_reports
        .iter()
        .max_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|report| report.id.clone())
}

fn latest_workflow_job_id(jobs: &[JobSummary]) -> Option<String> {
    jobs.iter()
        .max_by(|left, right| {
            left.updated_at
                .cmp(&right.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|job| job.id.clone())
}

fn latest_export_artifact_id(export_artifacts: &[ProjectExportArtifact]) -> Option<String> {
    export_artifacts
        .iter()
        .max_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|artifact| artifact.id.clone())
}

fn generated_asset_status_label(status: &GeneratedAssetStatus) -> &'static str {
    match status {
        GeneratedAssetStatus::Queued => "queued",
        GeneratedAssetStatus::Running => "running",
        GeneratedAssetStatus::Cancelled => "cancelled",
        GeneratedAssetStatus::Failed => "failed",
        GeneratedAssetStatus::Completed => "completed",
    }
}

fn render_report_status_label(status: &super::model::RenderReportStatus) -> &'static str {
    match status {
        super::model::RenderReportStatus::Queued => "queued",
        super::model::RenderReportStatus::Running => "running",
        super::model::RenderReportStatus::Failed => "failed",
        super::model::RenderReportStatus::Completed => "completed",
    }
}

fn job_status_label(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Progress => "progress",
        JobStatus::Blocked => "blocked",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
        JobStatus::Completed => "completed",
    }
}

fn export_artifact_kind_label(kind: &ProjectExportArtifactKind) -> &'static str {
    match kind {
        ProjectExportArtifactKind::NleXml => "nle_xml",
        ProjectExportArtifactKind::Webm => "webm",
        ProjectExportArtifactKind::Mp4 => "mp4",
        ProjectExportArtifactKind::Mov => "mov",
        ProjectExportArtifactKind::ProjectBundle => "project_bundle",
    }
}

fn workflow_start_request_ready(
    workflow: &TemporalWorkflowMetadata,
    start_request: &TemporalWorkflowStartRequest,
) -> bool {
    start_request.workflow_id == workflow.workflow_id
        && start_request.workflow_type == workflow.workflow_type
        && start_request.task_queue == workflow.task_queue
        && start_request.activity_types == workflow.activity_types
}

fn transcript_time_bounds(transcript: &SplitTranscriptFile) -> (Option<f64>, Option<f64>) {
    let mut starts = transcript
        .words
        .iter()
        .map(|word| word.start_seconds)
        .chain(
            transcript
                .segments
                .iter()
                .map(|segment| segment.start_seconds),
        )
        .filter(|value| value.is_finite());
    let mut ends = transcript
        .words
        .iter()
        .map(|word| word.end_seconds)
        .chain(
            transcript
                .segments
                .iter()
                .map(|segment| segment.end_seconds),
        )
        .filter(|value| value.is_finite());

    let start_seconds = starts.next().map(|first| starts.fold(first, f64::min));
    let end_seconds = ends.next().map(|first| ends.fold(first, f64::max));

    (start_seconds, end_seconds)
}

fn is_template_index_path(path: &Path) -> bool {
    path.file_name().and_then(|value| value.to_str()) == Some(SPLIT_TEMPLATE_INDEX_FILE_NAME)
}

fn is_transcript_index_path(path: &Path) -> bool {
    path.file_name().and_then(|value| value.to_str()) == Some(SPLIT_TRANSCRIPT_INDEX_FILE_NAME)
}

fn is_safe_manifest_segment(value: &str) -> bool {
    let mut components = Path::new(value).components();
    let Some(component) = components.next() else {
        return false;
    };

    matches!(component, Component::Normal(_))
        && components.next().is_none()
        && !value.contains('/')
        && !value.contains('\\')
}

fn issue(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> ProjectValidationIssue {
    ProjectValidationIssue {
        path: path.into(),
        message: message.into(),
        fix: fix.into(),
    }
}

fn create_split_dirs(
    project_dir: &Path,
    manifest: &SplitProjectManifest,
) -> Result<(), SplitProjectError> {
    create_dir(project_dir)?;

    for path in [
        &manifest.files.transcripts,
        &manifest.files.templates,
        &manifest.files.generated,
        &manifest.files.renders,
        &manifest.files.logs,
    ] {
        let dir = safe_join(project_dir, "files.directory", path)?;
        create_dir(&dir)?;
    }

    if let Some(parent) = safe_join(project_dir, "files.media", &manifest.files.media)?.parent() {
        create_dir(parent)?;
    }

    Ok(())
}

fn remove_stale_transcript_files(
    transcripts_dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<Vec<String>, SplitProjectError> {
    if !transcripts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut removed_files = Vec::new();
    let mut paths = std::fs::read_dir(transcripts_dir)
        .map_err(|error| SplitProjectError::Io {
            path: transcripts_dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: transcripts_dir.display().to_string(),
            message: error.to_string(),
        })?;
    paths.sort();

    for path in paths {
        if expected_paths.contains(&path) || !is_managed_transcript_file(&path) {
            continue;
        }

        std::fs::remove_file(&path).map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        removed_files.push(path.display().to_string());
    }

    Ok(removed_files)
}

fn is_managed_transcript_file(path: &Path) -> bool {
    if is_transcript_index_path(path) {
        return true;
    }
    let Ok(transcript) = read_json_file::<SplitTranscriptFile>(path) else {
        return false;
    };
    let expected_file_name = format!("{}.json", transcript.media_id);

    path.file_name().and_then(|value| value.to_str()) == Some(expected_file_name.as_str())
}

fn remove_stale_template_files(
    templates_dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<Vec<String>, SplitProjectError> {
    remove_stale_flat_files(templates_dir, expected_paths, |path| {
        let Ok(template) = read_json_file::<ProjectTemplateOverride>(path) else {
            return false;
        };
        let expected_file_name = format!("{}.json", template.template_id);

        path.file_name().and_then(|value| value.to_str()) == Some(expected_file_name.as_str())
    })
}

fn remove_stale_generated_asset_files(
    generated_dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<Vec<String>, SplitProjectError> {
    remove_stale_nested_files(generated_dir, expected_paths, "asset.json", |path| {
        let Ok(generated_asset) = read_json_file::<GeneratedAsset>(path) else {
            return false;
        };
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            == Some(generated_asset.id.as_str())
    })
}

fn remove_stale_render_report_files(
    renders_dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<Vec<String>, SplitProjectError> {
    remove_stale_nested_files(renders_dir, expected_paths, "report.json", |path| {
        let Ok(render_report) = read_json_file::<ProjectRenderReport>(path) else {
            return false;
        };
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            == Some(render_report.id.as_str())
    })
}

fn remove_stale_workflow_job_files(
    workflow_jobs_dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<Vec<String>, SplitProjectError> {
    remove_stale_nested_files(
        workflow_jobs_dir,
        expected_paths,
        SPLIT_WORKFLOW_JOB_FILE_NAME,
        |path| {
            let Ok(job) = read_json_file::<JobSummary>(path) else {
                return false;
            };
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|value| value.to_str())
                == Some(job.id.as_str())
        },
    )
}

fn remove_stale_export_artifact_files(
    export_artifacts_dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
) -> Result<Vec<String>, SplitProjectError> {
    remove_stale_nested_files(
        export_artifacts_dir,
        expected_paths,
        SPLIT_EXPORT_ARTIFACT_FILE_NAME,
        |path| {
            let Ok(export_artifact) = read_json_file::<ProjectExportArtifact>(path) else {
                return false;
            };
            path.parent()
                .and_then(|parent| parent.file_name())
                .and_then(|value| value.to_str())
                == Some(export_artifact.id.as_str())
        },
    )
}

fn remove_stale_flat_files(
    dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
    is_managed_file: impl Fn(&Path) -> bool,
) -> Result<Vec<String>, SplitProjectError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut removed_files = Vec::new();
    let mut paths = std::fs::read_dir(dir)
        .map_err(|error| SplitProjectError::Io {
            path: dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: dir.display().to_string(),
            message: error.to_string(),
        })?;
    paths.sort();

    for path in paths {
        if expected_paths.contains(&path) || !is_managed_file(&path) {
            continue;
        }

        std::fs::remove_file(&path).map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        removed_files.push(path.display().to_string());
    }

    Ok(removed_files)
}

fn remove_stale_nested_files(
    dir: &Path,
    expected_paths: &BTreeSet<PathBuf>,
    managed_file_name: &str,
    is_managed_file: impl Fn(&Path) -> bool,
) -> Result<Vec<String>, SplitProjectError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut removed_files = Vec::new();
    let mut paths = std::fs::read_dir(dir)
        .map_err(|error| SplitProjectError::Io {
            path: dir.display().to_string(),
            message: error.to_string(),
        })?
        .map(|entry| entry.map(|entry| entry.path().join(managed_file_name)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SplitProjectError::Io {
            path: dir.display().to_string(),
            message: error.to_string(),
        })?;
    paths.sort();

    for path in paths {
        if expected_paths.contains(&path) || !path.exists() || !is_managed_file(&path) {
            continue;
        }

        std::fs::remove_file(&path).map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        removed_files.push(path.display().to_string());
        remove_empty_parent_dir(&path)?;
    }

    Ok(removed_files)
}

fn remove_empty_parent_dir(path: &Path) -> Result<(), SplitProjectError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    match std::fs::remove_dir(parent) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SplitProjectError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        }),
    }
}

fn create_dir(path: &Path) -> Result<(), SplitProjectError> {
    std::fs::create_dir_all(path).map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), SplitProjectError> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }

    let json = serde_json::to_string_pretty(value).map_err(|error| SplitProjectError::Json {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let temp_parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp_file = tempfile::NamedTempFile::with_prefix_in(
        format!(
            ".{}.tmp.",
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("split-project")
        ),
        temp_parent,
    )
    .map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;

    temp_file
        .write_all(json.as_bytes())
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    temp_file
        .as_file()
        .sync_all()
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    temp_file
        .persist(path)
        .map_err(|error| SplitProjectError::Io {
            path: path.display().to_string(),
            message: error.error.to_string(),
        })?;

    Ok(())
}

fn write_json_file_and_checkpoint<T: Serialize>(
    path: &Path,
    value: &T,
    write_checkpoint: &mut dyn FnMut(&Path) -> Result<(), SplitProjectError>,
) -> Result<(), SplitProjectError> {
    write_json_file(path, value)?;
    write_checkpoint(path)
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, SplitProjectError> {
    let json = std::fs::read_to_string(path).map_err(|error| SplitProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&json).map_err(|error| SplitProjectError::Json {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn agent_edit_history_path(project_dir: &Path) -> PathBuf {
    project_dir
        .join(SPLIT_PROJECT_CONTEXT_DIR_NAME)
        .join(SPLIT_AGENT_EDIT_HISTORY_FILE_NAME)
}

fn app_server_conversation_history_path(project_dir: &Path) -> PathBuf {
    project_dir
        .join(SPLIT_PROJECT_CONTEXT_DIR_NAME)
        .join(SPLIT_APP_SERVER_CONVERSATION_FILE_NAME)
}

fn agent_session_manifest_path(project_dir: &Path) -> PathBuf {
    project_dir
        .join(SPLIT_PROJECT_CONTEXT_DIR_NAME)
        .join(SPLIT_AGENT_SESSION_FILE_NAME)
}

fn read_agent_session_manifest(
    project_dir: &Path,
    project_id: &str,
) -> Result<SplitAgentSessionManifest, SplitProjectError> {
    let path = agent_session_manifest_path(project_dir);
    if !path.exists() {
        return Ok(SplitAgentSessionManifest {
            schema_version: SPLIT_AGENT_SESSION_SCHEMA_VERSION,
            project_id: project_id.to_string(),
            active_session_id: None,
            sessions: Vec::new(),
            deleted_sessions: Vec::new(),
        });
    }
    let manifest: SplitAgentSessionManifest = read_json_file(&path)?;
    if manifest.schema_version != SPLIT_AGENT_SESSION_SCHEMA_VERSION
        || manifest.project_id != project_id
    {
        return Err(SplitProjectError::Json {
            path: path.display().to_string(),
            message: "agent session manifest schema/project mismatch".to_string(),
        });
    }
    Ok(manifest)
}

fn write_agent_session_manifest(
    project_dir: &Path,
    manifest: &SplitAgentSessionManifest,
) -> Result<PathBuf, SplitProjectError> {
    let path = agent_session_manifest_path(project_dir);
    write_json_file(&path, manifest)?;
    Ok(path)
}

fn read_agent_edit_history(
    project_dir: &Path,
) -> Result<SplitAgentEditHistoryFile, SplitProjectError> {
    let path = agent_edit_history_path(project_dir);
    if !path.exists() {
        return Ok(SplitAgentEditHistoryFile {
            schema_version: SPLIT_AGENT_EDIT_HISTORY_SCHEMA_VERSION,
            entries: Vec::new(),
        });
    }

    let history: SplitAgentEditHistoryFile = read_json_file(&path)?;
    if history.schema_version != SPLIT_AGENT_EDIT_HISTORY_SCHEMA_VERSION {
        return Err(SplitProjectError::Json {
            path: path.display().to_string(),
            message: format!(
                "unsupported agent edit history schema version {}",
                history.schema_version
            ),
        });
    }

    Ok(history)
}

fn read_app_server_conversation_history(
    project_dir: &Path,
) -> Result<SplitAppServerConversationFile, SplitProjectError> {
    let path = app_server_conversation_history_path(project_dir);
    if !path.exists() {
        return Ok(SplitAppServerConversationFile {
            schema_version: SPLIT_APP_SERVER_CONVERSATION_SCHEMA_VERSION,
            entries: Vec::new(),
        });
    }

    let history: SplitAppServerConversationFile = read_json_file(&path)?;
    if history.schema_version != SPLIT_APP_SERVER_CONVERSATION_SCHEMA_VERSION {
        return Err(SplitProjectError::Json {
            path: path.display().to_string(),
            message: format!(
                "unsupported app-server conversation schema version {}",
                history.schema_version
            ),
        });
    }

    Ok(history)
}

fn write_agent_edit_history(
    project_dir: &Path,
    history: &SplitAgentEditHistoryFile,
) -> Result<PathBuf, SplitProjectError> {
    let path = agent_edit_history_path(project_dir);
    write_json_file(&path, history)?;
    Ok(path)
}

fn write_app_server_conversation_history(
    project_dir: &Path,
    history: &SplitAppServerConversationFile,
) -> Result<PathBuf, SplitProjectError> {
    let path = app_server_conversation_history_path(project_dir);
    write_json_file(&path, history)?;
    Ok(path)
}

fn safe_join(project_dir: &Path, field: &str, value: &str) -> Result<PathBuf, SplitProjectError> {
    let path = Path::new(value);
    let unsafe_path = path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });

    if value.trim().is_empty() || unsafe_path {
        return Err(SplitProjectError::UnsafeManifestPath {
            field: field.to_string(),
            value: value.to_string(),
        });
    }

    Ok(project_dir.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    fn start_paused_settings_transaction(
        project_dir: &Path,
    ) -> (
        mpsc::SyncSender<()>,
        std::thread::JoinHandle<Result<ProjectActionWriteResult, SplitProjectError>>,
    ) {
        let settings_project_dir = project_dir.to_path_buf();
        let (staged_tx, staged_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let settings = std::thread::spawn(move || {
            update_project_settings_in_split_project_with_transaction_hook(
                &settings_project_dir,
                "Interview cut".to_string(),
                RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
                &mut |checkpoint| {
                    if matches!(
                        checkpoint,
                        SplitProjectTransactionCheckpoint::BeforePromotion
                    ) {
                        staged_tx.send(()).expect("settings staged");
                        release_rx.recv().expect("release settings promotion");
                    }
                    Ok(())
                },
            )
        });
        staged_rx.recv().expect("settings reaches promotion");
        (release_tx, settings)
    }

    fn file_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            let mut entries = fs::read_dir(current)
                .expect("read project directory")
                .map(|entry| entry.expect("project entry").path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.insert(
                        path.strip_prefix(root)
                            .expect("relative project path")
                            .to_path_buf(),
                        fs::read(&path).expect("read project file"),
                    );
                }
            }
        }

        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    #[test]
    fn split_project_rejects_every_unsafe_sidecar_id_before_writing() {
        for (field, value) in [
            ("transcripts[].mediaId", "../transcript"),
            ("templateOverrides[].templateId", "nested/template"),
            ("generatedAssets[].id", "/absolute-generated"),
            ("renderReports[].id", ".."),
            ("jobs[].id", "nested\\job"),
            ("exportArtifacts[].id", "."),
        ] {
            let error = validate_sidecar_id_segment(field, value)
                .expect_err("unsafe sidecar ID must be rejected");
            assert!(matches!(error, SplitProjectError::UnsafeSidecarId { .. }));
        }

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save valid fixture");
        let before_files = file_snapshot(&project_dir);
        let escaped_path = temp.path().join("escaped.json");
        fs::write(&escaped_path, b"sentinel").expect("write escape sentinel");
        let mut unsafe_project = load_split_project(&project_dir).expect("load fixture");
        unsafe_project.transcripts.push(Transcript {
            id: "transcript-escape".to_string(),
            media_id: "../escaped".to_string(),
            engine: None,
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: Vec::new(),
        });

        let error = save_split_project(&project_dir, &unsafe_project)
            .expect_err("unsafe save must fail before staging any project file");
        assert!(matches!(error, SplitProjectError::UnsafeSidecarId { .. }));
        assert_eq!(file_snapshot(&project_dir), before_files);
        assert_eq!(fs::read(&escaped_path).unwrap(), b"sentinel");
    }

    #[test]
    fn action_transaction_failure_keeps_the_complete_previous_project() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let before_files = file_snapshot(&project_dir);
        let before_project = load_split_project(&project_dir).expect("load fixture");
        let mut writes = 0;

        let result = apply_project_actions_to_split_project_with_transaction_hook(
            &project_dir,
            vec![ProjectAction::UpdateProjectSettings {
                name: "Atomic action".to_string(),
                render_settings: RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
            }],
            &mut |checkpoint| {
                if matches!(checkpoint, SplitProjectTransactionCheckpoint::Write(_)) {
                    writes += 1;
                    if writes == 2 {
                        return Err(SplitProjectError::Io {
                            path: "injected-action-write".to_string(),
                            message: "forced action persistence failure".to_string(),
                        });
                    }
                }
                Ok(())
            },
        );

        assert!(result.is_err());
        assert_eq!(writes, 2);
        assert_eq!(file_snapshot(&project_dir), before_files);
        assert_eq!(load_split_project(&project_dir).unwrap(), before_project);
    }

    #[test]
    fn metadata_action_does_not_traverse_or_copy_bulk_artifacts() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let bulk_dir = project_dir.join("cache/bulk");
        fs::create_dir_all(&bulk_dir).expect("bulk directory");
        for index in 0..128 {
            fs::write(
                bulk_dir.join(format!("frame-{index:04}.bin")),
                vec![index as u8; 8192],
            )
            .expect("bulk artifact");
        }
        #[cfg(unix)]
        {
            let outside = temp.path().join("outside-directory");
            fs::create_dir(&outside).expect("outside directory");
            std::os::unix::fs::symlink(&outside, project_dir.join("cache/unrelated-link"))
                .expect("unrelated bulk symlink");
        }
        let first = bulk_dir.join("frame-0000.bin");
        #[cfg(unix)]
        let inode_before = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&first).unwrap());

        apply_project_action_to_split_project(
            &project_dir,
            ProjectAction::UpdateProjectSettings {
                name: "Bounded metadata edit".to_string(),
                render_settings: sample_project().render_settings,
            },
        )
        .expect("metadata-only action ignores bulk tree");

        assert_eq!(fs::read(&first).unwrap(), vec![0; 8192]);
        assert_eq!(fs::read_dir(&bulk_dir).unwrap().count(), 128);
        #[cfg(unix)]
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(&fs::metadata(&first).unwrap()),
            inode_before,
            "bulk artifact must remain in place"
        );
    }

    #[test]
    fn metadata_transaction_failure_restores_removed_and_replaced_sidecars() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let mut original = sample_project();
        original.transcripts.push(Transcript {
            id: "transcript-before".to_string(),
            media_id: original.media[0].id.clone(),
            engine: Some("worker".to_string()),
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: Vec::new(),
        });
        save_split_project(&project_dir, &original).expect("save fixture");
        let before = file_snapshot(&project_dir);
        let mut replacement = load_split_project(&project_dir).expect("load fixture");
        replacement.name = "replacement".to_string();
        replacement.transcripts.clear();
        advance_project_content_revision(&project_dir, &mut replacement).unwrap();

        let error = save_split_project_metadata_transactionally(
            &project_dir,
            &replacement,
            &mut |checkpoint| {
                if matches!(
                    checkpoint,
                    SplitProjectTransactionCheckpoint::BeforePromotedJournal
                ) {
                    return Err(SplitProjectError::Io {
                        path: "injected-metadata-commit".to_string(),
                        message: "forced metadata commit failure".to_string(),
                    });
                }
                Ok(())
            },
        )
        .expect_err("metadata transaction must roll back");

        assert!(error.to_string().contains("forced metadata commit failure"));
        assert_eq!(file_snapshot(&project_dir), before);
        assert_eq!(
            load_split_project(&project_dir).unwrap().transcripts.len(),
            1
        );
    }

    #[test]
    fn metadata_transaction_open_recovery_keeps_committed_state() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let mut replacement = load_split_project(&project_dir).expect("load fixture");
        replacement.name = "Committed metadata".to_string();
        advance_project_content_revision(&project_dir, &mut replacement).unwrap();

        let report = save_split_project_metadata_transactionally(
            &project_dir,
            &replacement,
            &mut |checkpoint| {
                if matches!(
                    checkpoint,
                    SplitProjectTransactionCheckpoint::BeforeCleanup(_)
                ) {
                    return Err(SplitProjectError::Io {
                        path: "injected-metadata-cleanup".to_string(),
                        message: "forced cleanup deferral".to_string(),
                    });
                }
                Ok(())
            },
        )
        .expect("committed metadata is successful");
        assert!(report.recovery_pending);
        assert!(
            split_project_metadata_transaction_journal_path(&project_dir)
                .unwrap()
                .exists()
        );

        let recovered = load_split_project(&project_dir).expect("recover committed metadata");
        assert_eq!(recovered.name, "Committed metadata");
        assert!(
            !split_project_metadata_transaction_journal_path(&project_dir)
                .unwrap()
                .exists()
        );
    }

    #[test]
    fn metadata_commit_marker_requires_parent_sync_before_success() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let before = file_snapshot(&project_dir);
        let mut replacement = load_split_project(&project_dir).expect("load fixture");
        replacement.name = "Must not report success".to_string();
        advance_project_content_revision(&project_dir, &mut replacement).unwrap();

        let error = save_split_project_metadata_transactionally(
            &project_dir,
            &replacement,
            &mut |checkpoint| {
                if matches!(
                    checkpoint,
                    SplitProjectTransactionCheckpoint::AfterMetadataCommittedJournalBeforeParentSync
                ) {
                    return Err(SplitProjectError::Io {
                        path: "injected-journal-parent-sync".to_string(),
                        message: "forced failure before committed journal directory sync"
                            .to_string(),
                    });
                }
                Ok(())
            },
        )
        .expect_err("commit marker without a parent sync must not report success");

        assert!(error
            .to_string()
            .contains("forced failure before committed journal directory sync"));
        assert_eq!(file_snapshot(&project_dir), before);
        assert_eq!(
            load_split_project(&project_dir).unwrap().name,
            sample_project().name
        );
    }

    #[test]
    fn stale_snapshot_replacement_is_rejected_and_keeps_newer_canonical_state() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let stale = load_split_project(&project_dir).expect("capture stale project");

        update_project_settings_in_split_project(
            &project_dir,
            "Newer canonical edit".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
        )
        .expect("commit newer project");

        let error =
            replace_split_project_if_revision(&project_dir, stale.clone(), stale.content_revision)
                .expect_err("stale snapshot must conflict");

        assert!(matches!(error, SplitProjectError::RevisionConflict { .. }));
        let persisted = load_split_project(&project_dir).expect("newer project remains");
        assert_eq!(persisted.name, "Newer canonical edit");
        assert_eq!(persisted.render_settings.width, 3840);
    }

    #[test]
    fn successful_save_returns_the_exact_committed_project_for_the_next_cas_save() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();

        let result = save_split_project(&project_dir, &project).expect("save project");
        let persisted = load_split_project(&project_dir).expect("load committed project");

        assert_eq!(result.project, persisted);
        assert_eq!(result.project.content_revision, 1);
        let replacement = replace_split_project_if_revision(
            &project_dir,
            result.project.clone(),
            result.project.content_revision,
        )
        .expect("subsequent CAS save");
        assert_eq!(replacement.project.content_revision, 2);
    }

    #[test]
    fn agent_undo_returns_the_exact_committed_project_for_the_next_cas_save() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let before = save_split_project(&project_dir, &sample_project())
            .expect("save before")
            .project;
        let mut changed = before.clone();
        changed.name = "Agent edit".to_string();
        let after = save_split_project(&project_dir, &changed)
            .expect("save after")
            .project;
        record_agent_project_action_batch(&project_dir, before, after.clone(), 1)
            .expect("record agent history");

        let result = undo_latest_agent_project_action(&project_dir).expect("undo agent edit");
        let persisted = load_split_project(&project_dir).expect("load undone project");

        assert_eq!(result.project, persisted);
        replace_split_project_if_revision(
            &project_dir,
            result.project.clone(),
            result.project.content_revision,
        )
        .expect("subsequent CAS save");
    }

    #[test]
    fn snapshot_replacement_preserves_newer_same_media_transcript() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let mut project = sample_project();
        project.transcripts.push(Transcript {
            id: "transcript-media-1".to_string(),
            media_id: project.media[0].id.clone(),
            engine: Some("old-engine".to_string()),
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: Vec::new(),
        });
        save_split_project(&project_dir, &project).expect("save fixture");
        let stale_snapshot = load_split_project(&project_dir).expect("history snapshot");
        let mut worker_project = stale_snapshot.clone();
        worker_project.transcripts[0].engine = Some("new-worker-engine".to_string());
        save_split_project(&project_dir, &worker_project).expect("worker transcript update");
        let current = load_split_project(&project_dir).expect("current canonical project");

        replace_split_project_if_revision(&project_dir, stale_snapshot, current.content_revision)
            .expect("undo replacement");

        let persisted = load_split_project(&project_dir).expect("load replacement");
        assert_eq!(
            persisted.transcripts[0].engine.as_deref(),
            Some("new-worker-engine")
        );
    }

    #[test]
    fn update_project_settings_persists_manifest_and_split_sidecars() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path();
        save_split_project(project_dir, &sample_project()).expect("save fixture");

        let result = update_project_settings_in_split_project(
            project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
        )
        .expect("update project settings");

        let loaded = load_split_project(project_dir).expect("reload updated project");
        assert_eq!(loaded, result.project);
        assert_eq!(loaded.name, "Interview cut");
        assert_eq!(loaded.render_settings.width, 3840);
        assert_eq!(loaded.render_settings.captions, CaptionRenderMode::Mux);
        assert!(result
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("timeline.json")));
        assert!(result
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("media/index.json")));
        assert!(result
            .report
            .written_files
            .iter()
            .any(|path| path.ends_with("context/project.json")));
    }

    #[test]
    fn update_project_settings_rejects_invalid_input_without_writing_any_file() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path();
        save_split_project(project_dir, &sample_project()).expect("save fixture");
        let before = file_snapshot(project_dir);

        let result = update_project_settings_in_split_project(
            project_dir,
            "  ".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
        );

        assert!(result.is_err());
        assert_eq!(file_snapshot(project_dir), before);
    }

    #[test]
    fn update_project_settings_rolls_back_a_mid_write_failure_without_mixed_sidecars() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path();
        save_split_project(project_dir, &sample_project()).expect("save fixture");
        let before = file_snapshot(project_dir);
        let loaded_before = load_split_project(project_dir).expect("load original project");
        let mut writes = 0;

        let result = update_project_settings_in_split_project_with_transaction_hook(
            project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| {
                if matches!(checkpoint, SplitProjectTransactionCheckpoint::Write(_)) {
                    writes += 1;
                    if writes == 2 {
                        return Err(SplitProjectError::Io {
                            path: "injected-mid-write".to_string(),
                            message: "forced settings persistence failure".to_string(),
                        });
                    }
                }
                Ok(())
            },
        );

        assert!(result.is_err());
        assert_eq!(writes, 2);
        assert_eq!(file_snapshot(project_dir), before);
        assert_eq!(
            load_split_project(project_dir).expect("reload original project"),
            loaded_before
        );
    }

    #[test]
    fn project_open_recovers_when_promotion_and_immediate_rollback_both_fail() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let loaded_before = load_split_project(&project_dir).expect("load original project");
        let journal_path =
            split_project_transaction_journal_path(&project_dir).expect("transaction journal path");

        let result = update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::BeforePromotion
                | SplitProjectTransactionCheckpoint::BeforeRollback => Err(SplitProjectError::Io {
                    path: "injected-promotion".to_string(),
                    message: "forced rename failure".to_string(),
                }),
                _ => Ok(()),
            },
        );

        assert!(result.is_err());
        assert!(
            !project_dir.exists(),
            "failed rollback leaves recovery journal in control"
        );
        assert!(
            journal_path.is_file(),
            "durable journal survives the failed attempt"
        );

        let recovered = load_split_project(&project_dir).expect("open-time transaction recovery");
        assert_eq!(recovered, loaded_before);
        assert!(project_dir.is_dir());
        assert!(!journal_path.exists());
    }

    #[test]
    fn project_open_restores_the_backup_when_a_promoted_tree_is_incomplete() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let loaded_before = load_split_project(&project_dir).expect("load original project");
        let journal_path =
            split_project_transaction_journal_path(&project_dir).expect("transaction journal path");

        update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::BeforePromotion
                | SplitProjectTransactionCheckpoint::BeforeRollback => Err(SplitProjectError::Io {
                    path: "injected-promotion".to_string(),
                    message: "forced rename failure".to_string(),
                }),
                _ => Ok(()),
            },
        )
        .expect_err("promotion and rollback fail");

        let mut journal: SplitProjectTransactionJournal =
            read_json_file(&journal_path).expect("read recovery journal");
        fs::rename(&journal.staging_path, &project_dir).expect("simulate staged promotion");
        fs::remove_file(split_project_manifest_path(&project_dir))
            .expect("simulate an incomplete promoted tree");
        journal.phase = SplitProjectTransactionPhase::Promoted;
        write_split_project_transaction_journal(&journal_path, &journal)
            .expect("record promoted phase");

        let recovered = load_split_project(&project_dir).expect("restore complete backup");
        assert_eq!(recovered, loaded_before);
        assert!(!journal_path.exists());
    }

    #[test]
    fn post_promotion_durability_failures_restore_original_before_returning_error() {
        for failure_checkpoint in [
            SplitProjectTransactionCheckpoint::AfterPromotionBeforeParentSync,
            SplitProjectTransactionCheckpoint::BeforePromotedJournal,
        ] {
            let temp = tempfile::tempdir().expect("temporary project");
            let project_dir = temp.path().join("project");
            save_split_project(&project_dir, &sample_project()).expect("save fixture");
            let before = file_snapshot(&project_dir);
            let loaded_before = load_split_project(&project_dir).expect("original project");

            let result = update_project_settings_in_split_project_with_transaction_hook(
                &project_dir,
                "Interview cut".to_string(),
                RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
                &mut |checkpoint| {
                    if checkpoint == failure_checkpoint {
                        return Err(SplitProjectError::Io {
                            path: "injected-post-promotion".to_string(),
                            message: "forced post-promotion durability failure".to_string(),
                        });
                    }
                    Ok(())
                },
            );

            assert!(result.is_err(), "failed durability must not report success");
            assert_eq!(file_snapshot(&project_dir), before);
            assert_eq!(
                load_split_project(&project_dir).expect("original remains open"),
                loaded_before
            );
            assert!(
                !split_project_transaction_journal_path(&project_dir)
                    .expect("journal path")
                    .exists(),
                "rolled-back transaction leaves no recovery marker"
            );
        }
    }

    #[test]
    fn post_promotion_failure_reports_success_when_rollback_cannot_start_and_recovery_commits() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");

        let result = update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::AfterPromotionBeforeParentSync
                | SplitProjectTransactionCheckpoint::BeforeRollback => Err(SplitProjectError::Io {
                    path: "injected-post-promotion".to_string(),
                    message: "forced durability and rollback failure".to_string(),
                }),
                _ => Ok(()),
            },
        )
        .expect("validated promoted canonical is an honest success");

        assert_eq!(result.project.name, "Interview cut");
        assert_eq!(result.project.render_settings.width, 3840);
        assert_eq!(
            load_split_project(&project_dir)
                .expect("committed project")
                .name,
            "Interview cut"
        );
        assert!(!split_project_transaction_journal_path(&project_dir)
            .expect("journal path")
            .exists());
    }

    #[test]
    fn post_promotion_failure_reports_success_when_original_restore_fails_and_recovery_commits() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");

        let result = update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::AfterPromotionBeforeParentSync => {
                    Err(SplitProjectError::Io {
                        path: "injected-post-promotion".to_string(),
                        message: "forced durability failure".to_string(),
                    })
                }
                SplitProjectTransactionCheckpoint::BeforeRestoreOriginal(backup_path) => {
                    fs::remove_dir_all(&backup_path).expect("make original restore impossible");
                    Ok(())
                }
                _ => Ok(()),
            },
        )
        .expect("recovered promoted project is an honest success");

        assert_eq!(result.project.name, "Interview cut");
        assert_eq!(result.project.render_settings.width, 3840);
        let reopened = load_split_project(&project_dir).expect("committed project");
        assert_eq!(reopened.name, "Interview cut");
        assert_eq!(reopened.render_settings.width, 3840);
        assert!(!split_project_transaction_journal_path(&project_dir)
            .expect("journal path")
            .exists());
    }

    #[test]
    fn failed_promotion_keeps_recovery_evidence_until_restored_parent_syncs() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let original = load_split_project(&project_dir).expect("load original fixture");
        let journal_path =
            split_project_transaction_journal_path(&project_dir).expect("journal path");

        let result = update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::BeforePromotion => Err(SplitProjectError::Io {
                    path: "injected-promotion".to_string(),
                    message: "forced promotion failure".to_string(),
                }),
                SplitProjectTransactionCheckpoint::AfterRestoreOriginalBeforeParentSync => {
                    Err(SplitProjectError::Io {
                        path: "injected-parent-sync".to_string(),
                        message: "forced restored-parent sync failure".to_string(),
                    })
                }
                _ => Ok(()),
            },
        );

        assert!(result.is_err());
        assert!(journal_path.exists(), "recovery journal must survive");
        let journal: SplitProjectTransactionJournal =
            read_json_file(&journal_path).expect("retained journal");
        assert!(
            journal.staging_path.exists(),
            "staged candidate must survive"
        );
        assert_eq!(
            load_split_project(&project_dir).expect("crash recovery completes"),
            original
        );
        assert!(
            !journal_path.exists(),
            "recovery clears evidence after sync"
        );
    }

    #[test]
    fn promoted_recovery_success_does_not_depend_on_runtime_vector_order() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let mut project = sample_project();
        for id in ["job-a", "job-z"] {
            project.jobs.push(JobSummary {
                id: id.to_string(),
                kind: "render".to_string(),
                status: JobStatus::Completed,
                updated_at: "2026-07-19T12:00:00Z".to_string(),
                workflow: None,
                start_request: None,
                provider_request: None,
                failure_reason: None,
                export_settings: None,
            });
        }
        save_split_project(&project_dir, &project).expect("save fixture");
        let journal_path =
            split_project_transaction_journal_path(&project_dir).expect("journal path");

        let result = update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::AfterPromotionBeforeParentSync => {
                    Err(SplitProjectError::Io {
                        path: "injected-post-promotion".to_string(),
                        message: "forced durability failure".to_string(),
                    })
                }
                SplitProjectTransactionCheckpoint::BeforeRestoreOriginal(backup_path) => {
                    fs::remove_dir_all(&backup_path).expect("make original restore impossible");
                    let journal: SplitProjectTransactionJournal =
                        read_json_file(&journal_path).expect("read journal");
                    let jobs = journal.staging_path.join("jobs");
                    fs::rename(jobs.join("job-a"), jobs.join("zz-job-a"))
                        .expect("move first job sidecar");
                    fs::rename(jobs.join("job-z"), jobs.join("aa-job-z"))
                        .expect("move second job sidecar");
                    Ok(())
                }
                _ => Ok(()),
            },
        )
        .expect("promoted candidate identity determines success");

        assert_eq!(result.project.name, "Interview cut");
        let reopened = load_split_project(&project_dir).expect("committed project");
        assert_eq!(reopened.name, "Interview cut");
        assert_eq!(
            reopened
                .jobs
                .iter()
                .map(|job| job.id.as_str())
                .collect::<Vec<_>>(),
            vec!["job-z", "job-a"]
        );
    }

    #[test]
    fn valid_canonical_project_opens_while_cleanup_remains_pending_then_retries_cleanup() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let journal_path =
            split_project_transaction_journal_path(&project_dir).expect("transaction journal path");

        update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| match checkpoint {
                SplitProjectTransactionCheckpoint::BeforePromotion
                | SplitProjectTransactionCheckpoint::BeforeRollback => Err(SplitProjectError::Io {
                    path: "injected-promotion".to_string(),
                    message: "forced rename failure".to_string(),
                }),
                _ => Ok(()),
            },
        )
        .expect_err("leave recovery candidates");
        let mut journal: SplitProjectTransactionJournal =
            read_json_file(&journal_path).expect("read recovery journal");
        fs::rename(&journal.staging_path, &project_dir).expect("simulate promotion");
        journal.phase = SplitProjectTransactionPhase::Promoted;
        write_split_project_transaction_journal(&journal_path, &journal)
            .expect("record promoted phase");
        let backup_path = journal.backup_path.clone();

        let opened = load_split_project_with_cleanup_hook(&project_dir, &mut |path| {
            if path == backup_path {
                return Err(SplitProjectError::Io {
                    path: path.display().to_string(),
                    message: "forced persistent cleanup failure".to_string(),
                });
            }
            Ok(())
        })
        .expect("valid canonical opens despite cleanup failure");

        assert_eq!(opened.name, "Interview cut");
        assert!(journal_path.is_file(), "cleanup journal remains retryable");
        assert!(backup_path.is_dir(), "failed cleanup candidate remains");

        let reopened = load_split_project(&project_dir).expect("later open retries cleanup");
        assert_eq!(reopened.name, "Interview cut");
        assert!(!journal_path.exists());
        assert!(!backup_path.exists());
    }

    #[test]
    fn committed_settings_report_cleanup_pending_without_returning_an_ordinary_failure() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let mut failed_cleanup = false;

        let result = update_project_settings_in_split_project_with_transaction_hook(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
            &mut |checkpoint| {
                if matches!(
                    checkpoint,
                    SplitProjectTransactionCheckpoint::BeforeCleanup(_)
                ) && !failed_cleanup
                {
                    failed_cleanup = true;
                    return Err(SplitProjectError::Io {
                        path: "injected-cleanup".to_string(),
                        message: "forced committed cleanup failure".to_string(),
                    });
                }
                Ok(())
            },
        )
        .expect("committed settings return success with pending status");

        assert!(result.report.recovery_pending);
        assert_eq!(result.project.name, "Interview cut");
        assert_eq!(
            load_split_project(&project_dir)
                .expect("later open cleans and loads")
                .name,
            "Interview cut"
        );
        assert!(!split_project_transaction_journal_path(&project_dir)
            .expect("journal path")
            .exists());
    }

    /// Deadlock guard for writers released after a concurrent settings transaction. Each queued
    /// transaction fsyncs every staged file, backup, and parent directory; that costs well over a
    /// second on ext4 under parallel test load (APFS fsync is far cheaper), so this bound only
    /// needs to distinguish "blocked forever" from "durable I/O finished".
    const DURABLE_TRANSACTION_DEADLOCK_TIMEOUT: Duration = Duration::from_secs(30);

    #[test]
    fn project_mutation_lease_preserves_a_concurrent_artifact_and_canonical_action() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        let settings_project_dir = project_dir.clone();
        let (staged_tx, staged_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let settings = std::thread::spawn(move || {
            update_project_settings_in_split_project_with_transaction_hook(
                &settings_project_dir,
                "Interview cut".to_string(),
                RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
                &mut |checkpoint| {
                    if matches!(
                        checkpoint,
                        SplitProjectTransactionCheckpoint::BeforePromotion
                    ) {
                        staged_tx.send(()).expect("settings staged");
                        release_rx.recv().expect("release settings promotion");
                    }
                    Ok(())
                },
            )
        });
        staged_rx
            .recv()
            .expect("settings reaches promotion boundary");

        let writer_project_dir = project_dir.clone();
        let (writer_done_tx, writer_done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let lease = acquire_split_project_mutation_lease(&writer_project_dir)
                .expect("render-style project mutation lease");
            let artifact = writer_project_dir.join("renders/concurrent/pipeline-report.json");
            fs::create_dir_all(artifact.parent().expect("artifact parent"))
                .expect("create artifact parent");
            fs::write(&artifact, br#"{"status":"completed"}"#).expect("write artifact");
            let result = apply_project_actions_to_split_project_with_lease(
                &writer_project_dir,
                vec![ProjectAction::CreateMediaFolder {
                    folder: MediaFolder {
                        id: "concurrent-render".to_string(),
                        name: "Concurrent render".to_string(),
                        parent_id: None,
                    },
                }],
                &lease,
            );
            writer_done_tx.send(()).expect("writer completed");
            result
        });

        assert!(matches!(
            writer_done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        release_tx.send(()).expect("release settings transaction");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        writer_done_rx
            .recv_timeout(DURABLE_TRANSACTION_DEADLOCK_TIMEOUT)
            .expect("writer proceeds after settings");
        writer
            .join()
            .expect("writer thread")
            .expect("writer mutation");

        let loaded = load_split_project(&project_dir).expect("reload merged project");
        assert_eq!(loaded.name, "Interview cut");
        assert_eq!(loaded.render_settings.width, 3840);
        assert!(loaded
            .media_folders
            .iter()
            .any(|folder| folder.id == "concurrent-render"));
        assert_eq!(
            fs::read_to_string(project_dir.join("renders/concurrent/pipeline-report.json"))
                .expect("concurrent artifact"),
            r#"{"status":"completed"}"#
        );
    }

    #[test]
    fn agent_edit_history_write_waits_for_settings_and_persists_after_promotion() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save fixture");
        let (release_tx, settings) = start_paused_settings_transaction(&project_dir);

        let writer_project_dir = project_dir.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result =
                record_agent_project_action_batch(&writer_project_dir, project.clone(), project, 1);
            done_tx.send(()).expect("history writer completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        writer
            .join()
            .expect("writer thread")
            .expect("history write");
        let history = read_agent_edit_history(&project_dir).expect("history after promotion");
        assert_eq!(history.entries.len(), 1);
        assert_eq!(
            load_split_project(&project_dir)
                .expect("updated project")
                .name,
            "Interview cut"
        );
    }

    #[test]
    fn app_server_conversation_write_waits_for_settings_and_persists_after_promotion() {
        use crate::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save fixture");
        let (release_tx, settings) = start_paused_settings_transaction(&project_dir);

        let writer_project_dir = project_dir.clone();
        let project_id = project.id.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = record_app_server_conversation_turn(
                &writer_project_dir,
                &project_id,
                "thread-concurrent",
                AppServerConversationTurn {
                    turn_id: Some("turn-concurrent".to_string()),
                    turn_status: Some("completed".to_string()),
                    prompt: "Build a concise cut".to_string(),
                    created_at: "2026-07-19T13:00:00Z".to_string(),
                    request: serde_json::to_value(EditJobRequest {
                        media_id: "media-1".to_string(),
                        preset: EditPreset::TrailerCut,
                        prompt: "Build a concise cut".to_string(),
                        target_duration_seconds: Some(45.0),
                        language_mode: LanguageMode::Auto,
                        caption_style: CaptionStyle::Bold,
                        created_at: "2026-07-19T13:00:00Z".to_string(),
                    })
                    .expect("legacy request json"),
                    thread_response: serde_json::json!({ "thread": "thread-concurrent" }),
                    turn_response: serde_json::json!({ "status": "completed" }),
                    has_proposal: true,
                    provider: None,
                    provider_session_id: None,
                },
            );
            done_tx.send(()).expect("conversation writer completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        writer
            .join()
            .expect("writer thread")
            .expect("conversation write");
        let history = read_app_server_conversation_history(&project_dir)
            .expect("conversation after promotion");
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].thread_id, "thread-concurrent");
        let sessions = read_agent_session_manifest(&project_dir, &project.id)
            .expect("session manifest after promotion");
        assert_eq!(sessions.sessions.len(), 1);
        assert_eq!(
            load_split_project(&project_dir)
                .expect("updated project")
                .name,
            "Interview cut"
        );
    }

    #[test]
    fn agent_session_action_waits_for_settings_and_persists_after_promotion() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save fixture");
        let (release_tx, settings) = start_paused_settings_transaction(&project_dir);

        let writer_project_dir = project_dir.clone();
        let project_id = project.id.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = apply_agent_session_action(
                &writer_project_dir,
                &project_id,
                AgentSessionAction::Create {
                    id: "session-concurrent".to_string(),
                    title: "Concurrent session".to_string(),
                    thread_id: Some("thread-concurrent".to_string()),
                    timestamp: "2026-07-19T13:00:00Z".to_string(),
                },
            );
            done_tx.send(()).expect("session writer completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        let manifest = writer
            .join()
            .expect("writer thread")
            .expect("session write");
        assert_eq!(
            manifest.active_session_id.as_deref(),
            Some("session-concurrent")
        );
        assert_eq!(
            read_agent_session_manifest(&project_dir, &project.id)
                .expect("session after promotion")
                .active_session_id
                .as_deref(),
            Some("session-concurrent")
        );
        assert_eq!(
            load_split_project(&project_dir)
                .expect("updated project")
                .name,
            "Interview cut"
        );
    }

    #[test]
    fn media_import_waits_for_settings_and_merges_into_promoted_project() {
        use crate::project::import::import_media_files;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save fixture");
        let source = temp.path().join("concurrent.mp4");
        fs::write(&source, b"test media").expect("write import source");
        let (release_tx, settings) = start_paused_settings_transaction(&project_dir);

        let writer_project_dir = project_dir.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = import_media_files(&writer_project_dir, project, &[source]);
            done_tx.send(()).expect("import writer completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        let imported = writer.join().expect("writer thread").expect("media import");
        let persisted = load_split_project(&project_dir).expect("merged project");
        assert_eq!(persisted.name, "Interview cut");
        assert_eq!(imported.imported.len(), 1);
        assert!(persisted
            .media
            .iter()
            .any(|media| media.id == imported.imported[0].id));
    }

    #[test]
    fn matte_creation_waits_for_settings_and_merges_into_promoted_project() {
        use crate::project::matte::{create_matte_for_project, MatteRequest};

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save fixture");
        let (release_tx, settings) = start_paused_settings_transaction(&project_dir);

        let writer_project_dir = project_dir.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = create_matte_for_project(
                &writer_project_dir,
                &project,
                MatteRequest {
                    hex: "#112233".to_string(),
                    aspect_ratio: "Project".to_string(),
                    name: Some("Concurrent matte".to_string()),
                    folder_id: None,
                },
            );
            done_tx.send(()).expect("matte writer completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        let matte = writer
            .join()
            .expect("writer thread")
            .expect("matte creation");
        let persisted = load_split_project(&project_dir).expect("merged project");
        assert_eq!(persisted.name, "Interview cut");
        assert!(persisted
            .media
            .iter()
            .any(|media| media.id == matte.media.id));
    }

    #[test]
    fn temporal_export_artifact_waits_for_settings_and_persists_file_and_record() {
        use crate::workflows::temporal_export_media_write_artifact_activity_value;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let mut project = sample_project();
        project.jobs.push(JobSummary {
            id: "export-concurrent".to_string(),
            kind: "export_media".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-07-19T12:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        });
        save_split_project(&project_dir, &project).expect("save fixture");
        let rendered_path = project_dir.join("renders/export-concurrent/output.mp4");
        fs::create_dir_all(rendered_path.parent().expect("render parent"))
            .expect("create render directory");
        fs::write(&rendered_path, b"validated render").expect("write validated render");
        let (release_tx, settings) = start_paused_settings_transaction(&project_dir);

        let writer_project_dir = project_dir.clone();
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = temporal_export_media_write_artifact_activity_value(serde_json::json!({
                "projectId": "project-test",
                "projectDir": writer_project_dir.display().to_string(),
                "jobId": "export-concurrent",
                "profile": "mp4H264",
                "quality": "final",
                "outputPath": "exports/concurrent.mp4",
                "createdAt": "2026-07-19T13:00:00Z",
                "overwrite": true,
                "validation": {
                    "container": "mp4",
                    "extension": "mp4",
                    "mimeType": "video/mp4",
                    "videoCodec": "h264",
                    "audioCodec": "aac",
                    "requireVideoStream": true,
                    "requireAudioStreamWhenTimelineHasAudio": true
                },
                "validationOutput": {
                    "status": "validated",
                    "renderReport": {
                        "summary": { "outputPath": "renders/export-concurrent/output.mp4" }
                    }
                }
            }));
            done_tx.send(()).expect("export writer completed");
            result
        });
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        writer
            .join()
            .expect("writer thread")
            .expect("export artifact write");
        let persisted = load_split_project(&project_dir).expect("merged project");
        assert_eq!(persisted.name, "Interview cut");
        assert_eq!(
            fs::read(project_dir.join("exports/concurrent.mp4")).expect("export artifact"),
            b"validated render"
        );
        assert!(persisted
            .export_artifacts
            .iter()
            .any(|artifact| artifact.path == "exports/concurrent.mp4"));
    }

    #[test]
    fn agent_session_action_holds_the_lease_from_project_load_through_manifest_write() {
        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let project = sample_project();
        save_split_project(&project_dir, &project).expect("save fixture");

        let writer_project_dir = project_dir.clone();
        let project_id = project.id.clone();
        let (loaded_tx, loaded_rx) = mpsc::sync_channel(1);
        let (release_writer_tx, release_writer_rx) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            apply_agent_session_action_with_hook(
                &writer_project_dir,
                &project_id,
                AgentSessionAction::Create {
                    id: "session-owned".to_string(),
                    title: "Owned session".to_string(),
                    thread_id: Some("thread-owned".to_string()),
                    timestamp: "2026-07-19T13:00:00Z".to_string(),
                },
                &mut || {
                    loaded_tx.send(()).expect("session project loaded");
                    release_writer_rx.recv().expect("release session writer");
                },
            )
        });
        loaded_rx.recv().expect("writer loaded project");

        let settings_project_dir = project_dir.clone();
        let (settings_done_tx, settings_done_rx) = mpsc::sync_channel(1);
        let settings = std::thread::spawn(move || {
            let result = update_project_settings_in_split_project(
                &settings_project_dir,
                "Interview cut".to_string(),
                RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
            );
            settings_done_tx.send(()).expect("settings completed");
            result
        });
        assert!(matches!(
            settings_done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_writer_tx.send(()).expect("release session writer");
        writer
            .join()
            .expect("writer thread")
            .expect("session action");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        assert_eq!(
            read_agent_session_manifest(&project_dir, &project.id)
                .expect("session persisted")
                .active_session_id
                .as_deref(),
            Some("session-owned")
        );
        assert_eq!(
            load_split_project(&project_dir)
                .expect("settings persisted")
                .name,
            "Interview cut"
        );
    }

    #[test]
    fn interrupted_render_recovery_waits_for_settings_and_preserves_both_results() {
        use crate::render_pipeline::project_export::recover_interrupted_local_render_jobs;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let mut project = sample_project();
        project.jobs.push(JobSummary {
            id: "render-race".to_string(),
            kind: "render_draft".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-07-19T12:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        });
        save_split_project(&project_dir, &project).expect("save fixture");
        let render_dir = project_dir.join("renders/render-race");
        fs::create_dir_all(&render_dir).expect("create render directory");
        fs::write(render_dir.join("output.mp4"), b"partial-render")
            .expect("write interrupted render");

        let settings_project_dir = project_dir.clone();
        let (staged_tx, staged_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let settings = std::thread::spawn(move || {
            update_project_settings_in_split_project_with_transaction_hook(
                &settings_project_dir,
                "Interview cut".to_string(),
                RenderSettings {
                    width: 3840,
                    height: 2160,
                    fps: 24.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::Mux,
                },
                &mut |checkpoint| {
                    if matches!(
                        checkpoint,
                        SplitProjectTransactionCheckpoint::BeforePromotion
                    ) {
                        staged_tx.send(()).expect("settings staged");
                        release_rx.recv().expect("release settings promotion");
                    }
                    Ok(())
                },
            )
        });
        staged_rx.recv().expect("settings reaches promotion");

        let recovery_project_dir = project_dir.clone();
        let (recovery_done_tx, recovery_done_rx) = mpsc::sync_channel(1);
        let recovery = std::thread::spawn(move || {
            let result = recover_interrupted_local_render_jobs(
                &recovery_project_dir,
                "2026-07-19T12:01:00Z",
            );
            recovery_done_tx.send(()).expect("recovery completed");
            result
        });
        assert!(matches!(
            recovery_done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_tx.send(()).expect("release settings transaction");
        settings
            .join()
            .expect("settings thread")
            .expect("settings transaction");
        recovery_done_rx
            .recv_timeout(DURABLE_TRANSACTION_DEADLOCK_TIMEOUT)
            .expect("recovery proceeds after settings");
        let recovered = recovery
            .join()
            .expect("recovery thread")
            .expect("interrupted render recovery");

        assert_eq!(recovered.recovered_job_ids, vec!["render-race"]);
        assert!(project_dir
            .join("renders/render-race/output.mp4.interrupted")
            .is_file());
        assert!(project_dir
            .join("renders/render-race/recovery.log")
            .is_file());
        let loaded = load_split_project(&project_dir).expect("reload merged project");
        assert_eq!(loaded.name, "Interview cut");
        assert_eq!(loaded.render_settings.width, 3840);
        assert_eq!(
            loaded
                .jobs
                .iter()
                .find(|job| job.id == "render-race")
                .expect("render job")
                .status,
            JobStatus::Failed
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_settings_reject_a_writable_directory_symlink_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let external_dir = temp.path().join("external-context");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        fs::remove_dir_all(project_dir.join("context")).expect("remove managed context");
        fs::create_dir_all(&external_dir).expect("external directory");
        fs::write(external_dir.join("sentinel.txt"), b"outside").expect("external sentinel");
        symlink(&external_dir, project_dir.join("context")).expect("external directory symlink");
        let external_before = file_snapshot(&external_dir);

        let result = update_project_settings_in_split_project(
            &project_dir,
            "Interview cut".to_string(),
            RenderSettings {
                width: 3840,
                height: 2160,
                fps: 24.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::Mux,
            },
        );

        assert!(matches!(
            result,
            Err(SplitProjectError::UnsafeProjectSymlink { .. })
        ));
        assert_eq!(file_snapshot(&external_dir), external_before);
        assert_eq!(
            fs::read(external_dir.join("sentinel.txt")).unwrap(),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_render_recovery_rejects_a_symlink_escape_without_touching_its_target() {
        use crate::render_pipeline::project_export::recover_interrupted_local_render_jobs;
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let external_dir = temp.path().join("external-renders");
        let mut project = sample_project();
        project.jobs.push(JobSummary {
            id: "render-escape".to_string(),
            kind: "render_draft".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-07-19T12:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        });
        save_split_project(&project_dir, &project).expect("save fixture");
        fs::remove_dir_all(project_dir.join("renders")).expect("remove managed renders");
        fs::create_dir_all(external_dir.join("render-escape"))
            .expect("create external render directory");
        fs::write(
            external_dir.join("render-escape/output.mp4"),
            b"outside-render",
        )
        .expect("write external render");
        symlink(&external_dir, project_dir.join("renders"))
            .expect("external render directory symlink");
        let external_before = file_snapshot(&external_dir);

        let result = recover_interrupted_local_render_jobs(&project_dir, "2026-07-19T12:01:00Z");

        assert!(result.is_err());
        assert_eq!(file_snapshot(&external_dir), external_before);
    }

    #[cfg(unix)]
    #[test]
    fn project_relative_read_resolution_keeps_file_symlinks_readable() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let external_file = temp.path().join("external.mov");
        save_split_project(&project_dir, &sample_project()).expect("save fixture");
        fs::write(&external_file, b"linked-source").expect("external source");
        symlink(&external_file, project_dir.join("media/linked.mov")).expect("linked source media");

        assert_eq!(
            resolve_project_relative_path(&project_dir, "media/linked.mov")
                .expect("read resolver remains lexical"),
            project_dir.join("media/linked.mov")
        );
        assert_eq!(
            fs::read(project_dir.join("media/linked.mov")).expect("read linked media"),
            b"linked-source"
        );
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_render_recovery_rejects_a_nested_artifact_symlink() {
        use crate::render_pipeline::project_export::recover_interrupted_local_render_jobs;
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary project");
        let project_dir = temp.path().join("project");
        let external_log = temp.path().join("external-recovery.log");
        let mut project = sample_project();
        project.jobs.push(JobSummary {
            id: "render-nested-escape".to_string(),
            kind: "render_draft".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-07-19T12:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        });
        save_split_project(&project_dir, &project).expect("save fixture");
        let render_dir = project_dir.join("renders/render-nested-escape");
        fs::create_dir_all(&render_dir).expect("render directory");
        fs::write(&external_log, b"outside-log").expect("external log");
        symlink(&external_log, render_dir.join("recovery.log"))
            .expect("nested recovery-log symlink");
        let external_before = fs::read(&external_log).expect("external snapshot");

        let result = recover_interrupted_local_render_jobs(&project_dir, "2026-07-19T12:01:00Z");

        assert!(result.is_err());
        assert_eq!(
            fs::read(&external_log).expect("external log remains"),
            external_before
        );
    }
}
