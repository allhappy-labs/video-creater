use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::edit::render_plan::RenderQuality;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoProject {
    pub schema_version: u32,
    #[serde(default)]
    pub content_revision: u64,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub media: Vec<MediaAsset>,
    #[serde(default)]
    pub media_analysis: Vec<MediaAnalysisMoment>,
    #[serde(default)]
    pub media_silence_ranges: Vec<MediaSilenceRange>,
    #[serde(default)]
    pub media_folders: Vec<MediaFolder>,
    #[serde(default)]
    pub template_overrides: Vec<ProjectTemplateOverride>,
    #[serde(default)]
    pub generated_assets: Vec<GeneratedAsset>,
    #[serde(default)]
    pub render_reports: Vec<ProjectRenderReport>,
    #[serde(default)]
    pub export_artifacts: Vec<ProjectExportArtifact>,
    pub transcripts: Vec<Transcript>,
    /// The persisted alternate-cut library. `timeline` remains the active working
    /// projection while actions and callers migrate to explicit timeline IDs.
    #[serde(default)]
    pub timelines: Vec<ProjectTimeline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_timeline_id: Option<String>,
    pub timeline: Timeline,
    pub render_settings: RenderSettings,
    pub codex_thread_id: Option<String>,
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTimeline {
    pub id: String,
    pub name: String,
    pub timeline: Timeline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaAsset {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub relative_path: String,
    pub kind: MediaKind,
    pub duration_seconds: f64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaAnalysisMoment {
    pub media_id: String,
    pub source_in: f64,
    pub source_out: f64,
    #[serde(default)]
    pub visual_action_score: f64,
    #[serde(default)]
    pub audio_energy_score: f64,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaSilenceRange {
    pub media_id: String,
    pub source_in: f64,
    pub source_out: f64,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Video,
    Audio,
    Image,
    Lottie,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplateOverride {
    pub schema_version: u32,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    #[serde(default)]
    pub style: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub visual_treatment: String,
    #[serde(default)]
    pub motion: String,
    #[serde(default)]
    pub safe_zone: String,
    #[serde(default)]
    pub avoid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedAsset {
    pub schema_version: u32,
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
    pub model: GenerationModel,
    pub references: GeneratedAssetReferences,
    #[serde(default)]
    pub settings: GeneratedAssetSettings,
    pub outputs: Vec<GeneratedAssetOutput>,
    pub created_at: String,
    pub parent_asset_id: Option<String>,
    pub retry_of_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedAssetStatus {
    Queued,
    Running,
    Cancelled,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GenerationModel {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedAssetReferences {
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
pub struct GeneratedAssetSettings {
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
pub struct GeneratedAssetOutput {
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
pub struct ProjectRenderReport {
    pub schema_version: u32,
    pub id: String,
    pub status: RenderReportStatus,
    pub output_path: String,
    pub duration_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<RenderQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_height: Option<u32>,
    pub streams: RenderReportStreams,
    pub checks: BTreeMap<String, RenderReportCheckStatus>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_comparison_request: Option<ProjectRenderPreviewComparisonRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_comparison: Option<ProjectRenderPreviewComparison>,
    pub log_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRenderPreviewComparisonRequest {
    pub status: String,
    pub project_dir: String,
    pub project_report_id: String,
    pub render_report_path: String,
    pub rendered_video: String,
    pub duration_seconds: f64,
    pub frame_time_seconds: f64,
    pub rendered_frames: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fail_on_mismatch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRenderPreviewComparison {
    pub status: String,
    pub compared_frames: Vec<ProjectRenderPreviewComparisonFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRenderPreviewComparisonFrame {
    pub timeline_seconds: f64,
    pub preview_frame: String,
    pub rendered_frame: String,
    pub diff_frame: Option<String>,
    pub mismatch_ratio: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RenderReportStatus {
    Queued,
    Running,
    Failed,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderReportStreams {
    pub video: bool,
    pub audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RenderReportCheckStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportArtifact {
    pub schema_version: u32,
    pub id: String,
    pub kind: ProjectExportArtifactKind,
    pub format: String,
    pub path: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectExportArtifactKind {
    NleXml,
    Webm,
    Mp4,
    Mov,
    ProjectBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: String,
    pub media_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_artifact_path: Option<String>,
    #[serde(default)]
    pub repairs: Vec<TranscriptRepair>,
    #[serde(default)]
    pub segments: Vec<TranscriptSegment>,
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRepair {
    pub id: String,
    pub kind: TranscriptRepairKind,
    pub word_index: usize,
    pub before: TranscriptWord,
    pub after: TranscriptWord,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptRepairKind {
    WordText,
    WordTiming,
    WordTextAndTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptWord {
    pub text: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub confidence: Option<f64>,
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub duration_seconds: f64,
    pub tracks: Vec<TimelineTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineTrack {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub locked: bool,
    #[serde(default)]
    pub sync_locked: bool,
    #[serde(default = "default_track_enabled")]
    pub enabled: bool,
    pub items: Vec<TimelineItem>,
    /// Clip-to-clip transitions centered on cuts between adjacent items on this
    /// track. The canonical timeline never overlaps the two clips; renderers
    /// extend both clips into their unused source media instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TimelineTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineTransition {
    pub id: String,
    pub left_item_id: String,
    pub right_item_id: String,
    pub kind: TransitionKind,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransitionKind {
    Crossfade,
    DipToBlack,
    DipToWhite,
    Wipe,
}

fn default_track_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    HyperframeScene,
    Overlay,
    Caption,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub id: String,
    pub kind: TimelineItemKind,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub source: TimelineSource,
    pub label: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineItemKind {
    VideoClip,
    ImageClip,
    LottieClip,
    GeneratedClip,
    HyperframeScene,
    Overlay,
    Caption,
    AudioClip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TimelineSource {
    Media { media_id: String },
    Generated { artifact_id: String },
    Timeline { timeline_id: String },
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub loudness_lufs: f64,
    pub captions: CaptionRenderMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CaptionRenderMode {
    BurnIn,
    Mux,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    pub id: String,
    pub kind: String,
    pub status: JobStatus,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<TemporalWorkflowMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_request: Option<TemporalWorkflowStartRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_request: Option<JobProviderRequest>,
    /// Plain-language reason a failed job stopped, set by `recordJobFailure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Export options, file name and folder of an export job, for Retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_settings: Option<crate::project::export_options::JobExportSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Progress,
    Blocked,
    Failed,
    Cancelled,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JobProviderRequest {
    pub provider: String,
    pub request_id: String,
    pub status_url: String,
    pub response_url: String,
    pub cancel_url: String,
    pub submitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkflowMetadata {
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub run_id: Option<String>,
    pub activity_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWorkflowStartRequest {
    pub workflow_id: String,
    pub workflow_type: String,
    pub task_queue: String,
    pub input: Value,
    pub search_attributes: Value,
    pub activity_types: Vec<String>,
    pub id_reuse_policy: String,
}

impl VideoProject {
    pub fn new_empty(id: String, name: String, now: String) -> Self {
        let timeline = Timeline::default_editor_timeline();
        Self {
            schema_version: 1,
            content_revision: 0,
            id,
            name,
            created_at: now.clone(),
            updated_at: now,
            media: Vec::new(),
            media_analysis: Vec::new(),
            media_silence_ranges: Vec::new(),
            media_folders: Vec::new(),
            template_overrides: Vec::new(),
            generated_assets: Vec::new(),
            render_reports: Vec::new(),
            export_artifacts: Vec::new(),
            transcripts: Vec::new(),
            timelines: vec![ProjectTimeline {
                id: "main".to_string(),
                name: "Timeline 1".to_string(),
                timeline: timeline.clone(),
            }],
            active_timeline_id: Some("main".to_string()),
            timeline,
            render_settings: RenderSettings {
                width: 1920,
                height: 1080,
                fps: 24.0,
                loudness_lufs: -14.0,
                captions: CaptionRenderMode::BurnIn,
            },
            codex_thread_id: None,
            jobs: Vec::new(),
        }
    }

    /// Return a project projection whose working timeline is the requested
    /// alternate cut. `active` names the existing active projection.
    pub fn projected_for_timeline(&self, timeline_id: &str) -> Option<Self> {
        let timeline_id = timeline_id.trim();
        if timeline_id.is_empty() || timeline_id.eq_ignore_ascii_case("active") {
            return Some(self.clone());
        }
        let active_timeline_id = self.active_timeline_id.as_deref().unwrap_or("main");
        if timeline_id == active_timeline_id {
            return Some(self.clone());
        }
        let timeline = self
            .timelines
            .iter()
            .find(|entry| entry.id == timeline_id)?
            .timeline
            .clone();
        let mut projected = self.clone();
        projected.timeline = timeline;
        projected.active_timeline_id = Some(timeline_id.to_string());
        Some(projected)
    }
}

impl Timeline {
    /// The standard editable track layout for a new, non-duplicated timeline.
    pub fn default_editor_timeline() -> Self {
        Self {
            duration_seconds: 0.0,
            tracks: vec![
                TimelineTrack::empty("track-video", "Video", TrackKind::Video),
                TimelineTrack::empty("track-scenes", "HyperFrames", TrackKind::HyperframeScene),
                TimelineTrack::empty("track-overlays", "Overlays", TrackKind::Overlay),
                TimelineTrack::empty("track-captions", "Captions", TrackKind::Caption),
                TimelineTrack::empty("track-audio", "Audio", TrackKind::Audio),
            ],
        }
    }
}

impl TimelineTrack {
    pub fn empty(id: &str, name: &str, kind: TrackKind) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            kind,
            locked: false,
            sync_locked: false,
            enabled: true,
            items: Vec::new(),
            transitions: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TimelineSource, TimelineTrack};
    use serde_json::json;

    #[test]
    fn timeline_source_media_serializes_with_camel_case_fields() {
        let source = TimelineSource::Media {
            media_id: "media-1".to_string(),
        };

        let serialized = serde_json::to_value(source).expect("serialize timeline source");

        assert_eq!(
            serialized,
            json!({
                "type": "media",
                "mediaId": "media-1",
            })
        );
    }

    #[test]
    fn timeline_track_without_sync_lock_defaults_unlocked_and_round_trips() {
        let track: TimelineTrack = serde_json::from_value(json!({
            "id": "track-video",
            "name": "Video",
            "kind": "video",
            "locked": false,
            "enabled": true,
            "items": []
        }))
        .expect("decode legacy track");
        assert!(!track.sync_locked);
        let value = serde_json::to_value(track).expect("encode track");
        assert_eq!(value["syncLocked"], json!(false));
    }

    #[test]
    fn timeline_track_defaults_enabled_when_field_is_missing() {
        let track: TimelineTrack = serde_json::from_value(json!({
            "id": "track-video",
            "name": "Video",
            "kind": "video",
            "locked": false,
            "items": [],
        }))
        .expect("deserialize timeline track without enabled");

        assert!(track.enabled);
    }
}
