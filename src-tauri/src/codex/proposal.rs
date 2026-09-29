use crate::edit::edl::{validate_one_click_edl, EdlClip, EdlError, RoughCutEdl};
use crate::edit::preset::{EditJobRequest, EditRequestError};
use crate::project::action::{apply_project_action, ProjectAction, ProjectActionError};
use crate::project::model::{
    TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexEditProposal {
    pub media_id: String,
    pub clips: Vec<CodexProposalClip>,
    #[serde(default)]
    pub captions: Vec<serde_json::Value>,
    #[serde(default)]
    pub overlays: Vec<serde_json::Value>,
    #[serde(default)]
    pub hyperframes: Vec<serde_json::Value>,
    #[serde(default)]
    pub gpu_visuals: Vec<serde_json::Value>,
    #[serde(default)]
    pub project_actions: Vec<ProjectAction>,
    pub render_review: CodexRenderReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalClip {
    #[serde(default)]
    pub media_id: String,
    pub source_in: f64,
    pub source_out: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexRenderReview {
    pub duration_seconds: f64,
    pub stream_check_required: bool,
    pub caption_alignment_required: bool,
    pub overlay_timing_required: bool,
    pub visual_frame_evidence_required: bool,
    pub artifact_paths_required: bool,
    pub log_reference_required: bool,
}

struct MotionTemplateSpec {
    id: &'static str,
    required_fields: &'static [&'static str],
}

const MOTION_TEMPLATE_SPECS: &[MotionTemplateSpec] = &[
    MotionTemplateSpec {
        id: "kinetic-lower-third-v1",
        required_fields: &["headline", "subline"],
    },
    MotionTemplateSpec {
        id: "punchy-caption-v1",
        required_fields: &["headline"],
    },
    MotionTemplateSpec {
        id: "metric-callout-v1",
        required_fields: &["headline", "subline"],
    },
    MotionTemplateSpec {
        id: "chapter-card-v1",
        required_fields: &["headline", "subline"],
    },
    MotionTemplateSpec {
        id: "tracking-highlight-v1",
        required_fields: &["headline"],
    },
    MotionTemplateSpec {
        id: "holographic-logo-cutout-v1",
        required_fields: &["logoAssetId"],
    },
    MotionTemplateSpec {
        id: "gradient-background-loop-v1",
        required_fields: &["headline"],
    },
];
const TEMPLATE_TIMING_TOLERANCE_SECONDS: f64 = 1e-6;
pub const CODEX_EDIT_PROPOSAL_VALIDATOR_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq)]
pub enum CodexProposalError {
    #[error("edit request is invalid: {0}")]
    InvalidRequest(EditRequestError),
    #[error("proposal media id does not match request media id")]
    MediaIdMismatch,
    #[error("media was not found: {0}")]
    MediaNotFound(String),
    #[error("proposal EDL is invalid: {0}")]
    Edl(EdlError),
    #[error(
        "render review must require duration, stream, caption, overlay, artifact, and log checks"
    )]
    IncompleteRenderReview,
    #[error("render review duration is invalid: {0}")]
    InvalidRenderReviewDuration(&'static str),
    #[error("unknown motion template: {0}")]
    UnknownTemplateId(String),
    #[error("template field {0} cannot be empty")]
    EmptyTemplateField(String),
    #[error("template overlay metadata is missing: {0}")]
    MissingTemplateMetadata(&'static str),
    #[error("hyperframe metadata is missing: {0}")]
    MissingHyperframeMetadata(&'static str),
    #[error("hyperframe kind is not supported: {0}")]
    UnsupportedHyperframeKind(String),
    #[error("hyperframe timing is invalid: {0}")]
    InvalidHyperframeTiming(&'static str),
    #[error("gpu visual timing is invalid: {0}")]
    InvalidGpuVisualTiming(&'static str),
    #[error("template overlay kind is not supported: {0}")]
    UnsupportedTemplateKind(String),
    #[error("template overlay timing is invalid: {0}")]
    InvalidTemplateTiming(&'static str),
    #[error("caption timing is invalid: {0}")]
    InvalidCaptionTiming(&'static str),
    #[error("overlay timing is invalid: {0}")]
    InvalidOverlayTiming(&'static str),
    #[error("{0} visual treatment is not allowed: {1}")]
    BannedVisualTreatment(&'static str, &'static str),
    #[error("project action is invalid: {0}")]
    ProjectAction(ProjectActionError),
    #[error("proposal materialization track was not found: {0:?}")]
    TrackNotFound(TrackKind),
}

impl CodexProposalError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "codex.proposal.invalidRequest",
            Self::MediaIdMismatch => "codex.proposal.mediaIdMismatch",
            Self::MediaNotFound(_) => "codex.proposal.mediaNotFound",
            Self::Edl(_) => "codex.proposal.invalidEdl",
            Self::IncompleteRenderReview => "codex.proposal.incompleteRenderReview",
            Self::InvalidRenderReviewDuration(_) => "codex.proposal.invalidRenderReviewDuration",
            Self::UnknownTemplateId(_) => "codex.proposal.unknownTemplateId",
            Self::EmptyTemplateField(_) => "codex.proposal.emptyTemplateField",
            Self::MissingTemplateMetadata(_) => "codex.proposal.missingTemplateMetadata",
            Self::MissingHyperframeMetadata(_) => "codex.proposal.missingHyperframeMetadata",
            Self::UnsupportedHyperframeKind(_) => "codex.proposal.unsupportedHyperframeKind",
            Self::InvalidHyperframeTiming(_) => "codex.proposal.invalidHyperframeTiming",
            Self::InvalidGpuVisualTiming(_) => "codex.proposal.invalidGpuVisualTiming",
            Self::UnsupportedTemplateKind(_) => "codex.proposal.unsupportedTemplateKind",
            Self::InvalidTemplateTiming(_) => "codex.proposal.invalidTemplateTiming",
            Self::InvalidCaptionTiming(_) => "codex.proposal.invalidCaptionTiming",
            Self::InvalidOverlayTiming(_) => "codex.proposal.invalidOverlayTiming",
            Self::BannedVisualTreatment(_, _) => "codex.proposal.bannedVisualTreatment",
            Self::ProjectAction(_) => "codex.proposal.invalidProjectAction",
            Self::TrackNotFound(_) => "codex.proposal.trackNotFound",
        }
    }
}

pub fn validate_codex_edit_proposal(
    project: &VideoProject,
    request: &EditJobRequest,
    proposal: &CodexEditProposal,
) -> Result<RoughCutEdl, CodexProposalError> {
    request
        .validate()
        .map_err(CodexProposalError::InvalidRequest)?;
    if proposal.media_id != request.media_id {
        return Err(CodexProposalError::MediaIdMismatch);
    }

    let media = project
        .media
        .iter()
        .find(|media| media.id == request.media_id)
        .ok_or_else(|| CodexProposalError::MediaNotFound(request.media_id.clone()))?;

    if !proposal.render_review.stream_check_required
        || !proposal.render_review.caption_alignment_required
        || !proposal.render_review.overlay_timing_required
        || !proposal.render_review.visual_frame_evidence_required
        || !proposal.render_review.artifact_paths_required
        || !proposal.render_review.log_reference_required
    {
        return Err(CodexProposalError::IncompleteRenderReview);
    }

    let mut source_durations_seconds =
        BTreeMap::from([(proposal.media_id.clone(), media.duration_seconds)]);
    for clip in &proposal.clips {
        let media_id = if clip.media_id.is_empty() {
            &proposal.media_id
        } else {
            &clip.media_id
        };
        let clip_media = project
            .media
            .iter()
            .find(|media| media.id == *media_id)
            .ok_or_else(|| CodexProposalError::MediaNotFound(media_id.clone()))?;
        source_durations_seconds.insert(media_id.clone(), clip_media.duration_seconds);
    }
    let edl = RoughCutEdl {
        media_id: proposal.media_id.clone(),
        source_duration_seconds: source_durations_seconds.values().sum(),
        source_durations_seconds,
        clips: proposal
            .clips
            .iter()
            .map(|clip| EdlClip {
                media_id: if clip.media_id.is_empty() {
                    proposal.media_id.clone()
                } else {
                    clip.media_id.clone()
                },
                source_in: clip.source_in,
                source_out: clip.source_out,
                reason: clip.reason.clone(),
                selection_reasons: vec![clip.reason.clone()],
                score: None,
            })
            .collect(),
    };

    validate_one_click_edl(&request.preset, &edl).map_err(CodexProposalError::Edl)?;
    let timeline_duration_seconds = edl.duration_seconds();
    validate_render_review_duration(
        proposal.render_review.duration_seconds,
        timeline_duration_seconds,
    )?;
    validate_captions(&proposal.captions, timeline_duration_seconds)?;
    validate_template_overlays(&proposal.overlays, timeline_duration_seconds)?;
    validate_overlay_visual_treatments(&proposal.overlays)?;
    validate_overlay_timing(&proposal.overlays, timeline_duration_seconds)?;
    validate_hyperframes(&proposal.hyperframes, timeline_duration_seconds)?;
    validate_gpu_visual_timing(&proposal.gpu_visuals, timeline_duration_seconds)?;
    validate_project_actions(project, &proposal.project_actions)?;
    Ok(edl)
}

pub fn materialize_codex_edit_proposal_actions(
    project: &VideoProject,
    request: &EditJobRequest,
    proposal: &CodexEditProposal,
) -> Result<Vec<ProjectAction>, CodexProposalError> {
    let edl = validate_codex_edit_proposal(project, request, proposal)?;
    let video_track_id = proposal_track_id(project, TrackKind::Video)?;
    let audio_track_id = proposal_track_id(project, TrackKind::Audio)?;
    let caption_track_id = proposal_track_id(project, TrackKind::Caption)?;
    let overlay_track_id = proposal_track_id(project, TrackKind::Overlay)?;
    let hyperframe_track_id = proposal_track_id(project, TrackKind::HyperframeScene)?;

    let mut actions = Vec::new();
    let mut video_items = Vec::new();
    let mut audio_items = Vec::new();
    let mut output_cursor = 0.0;
    for (index, clip) in edl.clips.iter().enumerate() {
        let duration = clip.source_out - clip.source_in;
        let mut properties = BTreeMap::new();
        properties.insert("sourceIn".to_string(), json!(clip.source_in));
        properties.insert("sourceOut".to_string(), json!(clip.source_out));
        properties.insert("reason".to_string(), json!(clip.reason));
        properties.insert("generatedBy".to_string(), json!("codex-edit-proposal"));

        video_items.push(TimelineItem {
            id: format!("codex-proposal-video-{}", index + 1),
            kind: TimelineItemKind::VideoClip,
            start_seconds: output_cursor,
            duration_seconds: duration,
            source: TimelineSource::Media {
                media_id: clip.media_id.clone(),
            },
            label: format!("Codex clip {}", index + 1),
            properties: properties.clone(),
        });
        audio_items.push(TimelineItem {
            id: format!("codex-proposal-audio-{}", index + 1),
            kind: TimelineItemKind::AudioClip,
            start_seconds: output_cursor,
            duration_seconds: duration,
            source: TimelineSource::Media {
                media_id: clip.media_id.clone(),
            },
            label: format!("Codex audio {}", index + 1),
            properties,
        });
        output_cursor += duration;
    }
    push_add_items_action(&mut actions, video_track_id, video_items);
    push_add_items_action(&mut actions, audio_track_id, audio_items);
    push_add_items_action(
        &mut actions,
        caption_track_id,
        proposal
            .captions
            .iter()
            .enumerate()
            .map(materialize_caption_item)
            .collect(),
    );
    push_add_items_action(
        &mut actions,
        overlay_track_id,
        proposal
            .overlays
            .iter()
            .enumerate()
            .map(materialize_overlay_item)
            .collect(),
    );
    push_add_items_action(
        &mut actions,
        hyperframe_track_id,
        proposal
            .hyperframes
            .iter()
            .enumerate()
            .map(materialize_hyperframe_item)
            .collect(),
    );
    actions.extend(proposal.project_actions.clone());

    validate_materialized_project_actions(project, &actions)?;
    Ok(actions)
}

fn validate_render_review_duration(
    render_review_duration_seconds: f64,
    timeline_duration_seconds: f64,
) -> Result<(), CodexProposalError> {
    if !render_review_duration_seconds.is_finite()
        || render_review_duration_seconds <= 0.0
        || (render_review_duration_seconds - timeline_duration_seconds).abs()
            > TEMPLATE_TIMING_TOLERANCE_SECONDS
    {
        return Err(CodexProposalError::InvalidRenderReviewDuration(
            "renderReview.durationSeconds must match selected EDL duration",
        ));
    }

    Ok(())
}

fn validate_project_actions(
    project: &VideoProject,
    actions: &[ProjectAction],
) -> Result<(), CodexProposalError> {
    let mut next_project = project.clone();
    for action in actions {
        apply_project_action(&mut next_project, action.clone())
            .map_err(CodexProposalError::ProjectAction)?;
    }
    Ok(())
}

fn validate_materialized_project_actions(
    project: &VideoProject,
    actions: &[ProjectAction],
) -> Result<(), CodexProposalError> {
    let mut next_project = project.clone();
    for action in actions {
        apply_project_action(&mut next_project, action.clone())
            .map_err(CodexProposalError::ProjectAction)?;
    }
    Ok(())
}

fn proposal_track_id(
    project: &VideoProject,
    kind: TrackKind,
) -> Result<String, CodexProposalError> {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == kind)
        .map(|track| track.id.clone())
        .ok_or(CodexProposalError::TrackNotFound(kind))
}

fn push_add_items_action(
    actions: &mut Vec<ProjectAction>,
    target_track_id: String,
    items: Vec<TimelineItem>,
) {
    if items.is_empty() {
        return;
    }
    actions.push(ProjectAction::AddItems {
        target_track_id,
        items,
    });
}

fn materialize_caption_item((index, caption): (usize, &serde_json::Value)) -> TimelineItem {
    let mut properties = BTreeMap::new();
    copy_optional_visual_property(caption, &mut properties, "visualTreatment");
    copy_optional_visual_property(caption, &mut properties, "motion");
    copy_optional_visual_property(caption, &mut properties, "safeZone");
    copy_optional_visual_property(caption, &mut properties, "avoid");
    properties.insert("generatedBy".to_string(), json!("codex-edit-proposal"));
    properties.insert("proposalCaptionIndex".to_string(), json!(index));
    if let Some(source_in) = numeric_property(caption, "sourceIn") {
        properties.insert("sourceIn".to_string(), json!(source_in));
    }
    if let Some(source_out) = numeric_property(caption, "sourceOut") {
        properties.insert("sourceOut".to_string(), json!(source_out));
    }
    let text = visible_text(caption).unwrap_or_else(|| format!("Caption {}", index + 1));
    properties.insert("textEdited".to_string(), json!(false));

    TimelineItem {
        id: optional_string_property(caption, "id")
            .unwrap_or_else(|| format!("codex-proposal-caption-{}", index + 1)),
        kind: TimelineItemKind::Caption,
        start_seconds: numeric_property(caption, "startSeconds").unwrap_or_default(),
        duration_seconds: numeric_property(caption, "durationSeconds").unwrap_or(1.0),
        source: TimelineSource::Text { text },
        label: format!("Codex caption {}", index + 1),
        properties,
    }
}

fn materialize_overlay_item((index, overlay): (usize, &serde_json::Value)) -> TimelineItem {
    let mut properties = BTreeMap::new();
    copy_optional_visual_property(overlay, &mut properties, "kind");
    copy_optional_visual_property(overlay, &mut properties, "templateId");
    copy_optional_visual_property(overlay, &mut properties, "visualTreatment");
    copy_optional_visual_property(overlay, &mut properties, "motion");
    copy_optional_visual_property(overlay, &mut properties, "safeZone");
    copy_optional_visual_property(overlay, &mut properties, "avoid");
    copy_optional_visual_property(overlay, &mut properties, "motionPresetId");
    properties.insert("previewVariant".to_string(), json!("template"));
    properties.insert("generatedBy".to_string(), json!("codex-edit-proposal"));
    properties.insert("proposalOverlayIndex".to_string(), json!(index));
    if let Some(fields) = overlay.get("fields").and_then(serde_json::Value::as_object) {
        properties.insert(
            "templateFields".to_string(),
            serde_json::Value::Object(fields.clone()),
        );
    }
    let text = visible_text(overlay).unwrap_or_else(|| format!("Overlay {}", index + 1));

    TimelineItem {
        id: optional_string_property(overlay, "id")
            .unwrap_or_else(|| format!("codex-proposal-overlay-{}", index + 1)),
        kind: TimelineItemKind::Overlay,
        start_seconds: numeric_property(overlay, "startSeconds").unwrap_or_default(),
        duration_seconds: numeric_property(overlay, "durationSeconds").unwrap_or(1.0),
        source: TimelineSource::Text { text: text.clone() },
        label: text,
        properties,
    }
}

fn materialize_hyperframe_item((index, hyperframe): (usize, &serde_json::Value)) -> TimelineItem {
    let kind = optional_string_property(hyperframe, "kind")
        .unwrap_or_else(|| "immersive_scene".to_string());
    let mut properties = BTreeMap::new();
    properties.insert("kind".to_string(), json!(kind.clone()));
    copy_optional_visual_property(hyperframe, &mut properties, "templateId");
    copy_optional_visual_property(hyperframe, &mut properties, "visualTreatment");
    copy_optional_visual_property(hyperframe, &mut properties, "motion");
    copy_optional_visual_property(hyperframe, &mut properties, "safeZone");
    copy_optional_visual_property(hyperframe, &mut properties, "avoid");
    copy_optional_visual_property(hyperframe, &mut properties, "motionPresetId");
    copy_optional_visual_property(hyperframe, &mut properties, "sourceBeat");
    properties.insert("previewVariant".to_string(), json!("hyperframe"));
    properties.insert("generatedBy".to_string(), json!("codex-edit-proposal"));
    properties.insert("proposalHyperframeIndex".to_string(), json!(index));
    if let Some(fields) = hyperframe
        .get("fields")
        .and_then(serde_json::Value::as_object)
    {
        properties.insert(
            "templateFields".to_string(),
            serde_json::Value::Object(fields.clone()),
        );
    }
    let text = visible_text(hyperframe).unwrap_or_else(|| format!("HyperFrame {}", index + 1));

    TimelineItem {
        id: optional_string_property(hyperframe, "id")
            .unwrap_or_else(|| format!("codex-proposal-hyperframe-{}", index + 1)),
        kind: TimelineItemKind::HyperframeScene,
        start_seconds: numeric_property(hyperframe, "startSeconds").unwrap_or_default(),
        duration_seconds: numeric_property(hyperframe, "durationSeconds").unwrap_or(1.0),
        source: TimelineSource::Text { text: text.clone() },
        label: text,
        properties,
    }
}

fn copy_optional_visual_property(
    value: &serde_json::Value,
    properties: &mut BTreeMap<String, serde_json::Value>,
    key: &str,
) {
    if let Some(text) = optional_string_property(value, key) {
        properties.insert(key.to_string(), json!(text));
    }
}

fn optional_string_property(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn numeric_property(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(serde_json::Value::as_f64)
}

fn visible_text(value: &serde_json::Value) -> Option<String> {
    for key in ["text", "brief", "headline", "title", "label"] {
        if let Some(text) = optional_string_property(value, key) {
            return Some(text);
        }
    }
    value
        .get("fields")
        .and_then(serde_json::Value::as_object)
        .and_then(|fields| {
            ["headline", "title", "label", "subline"]
                .iter()
                .find_map(|key| {
                    fields
                        .get(*key)
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                })
        })
}

fn validate_captions(
    captions: &[serde_json::Value],
    render_duration_seconds: f64,
) -> Result<(), CodexProposalError> {
    for caption in captions {
        let start_seconds = caption
            .get("startSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidCaptionTiming(
                "startSeconds must be finite and non-negative",
            ))?;
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(CodexProposalError::InvalidCaptionTiming(
                "startSeconds must be finite and non-negative",
            ));
        }

        let duration_seconds = caption
            .get("durationSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidCaptionTiming(
                "durationSeconds must be finite and positive",
            ))?;
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return Err(CodexProposalError::InvalidCaptionTiming(
                "durationSeconds must be finite and positive",
            ));
        }

        if start_seconds + duration_seconds
            > render_duration_seconds + TEMPLATE_TIMING_TOLERANCE_SECONDS
        {
            return Err(CodexProposalError::InvalidCaptionTiming(
                "caption must fit within render duration",
            ));
        }
    }

    Ok(())
}

fn validate_overlay_timing(
    overlays: &[serde_json::Value],
    render_duration_seconds: f64,
) -> Result<(), CodexProposalError> {
    for overlay in overlays {
        let start_seconds = overlay
            .get("startSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidOverlayTiming(
                "startSeconds must be finite and non-negative",
            ))?;
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(CodexProposalError::InvalidOverlayTiming(
                "startSeconds must be finite and non-negative",
            ));
        }

        let duration_seconds = overlay
            .get("durationSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidOverlayTiming(
                "durationSeconds must be finite and positive",
            ))?;
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return Err(CodexProposalError::InvalidOverlayTiming(
                "durationSeconds must be finite and positive",
            ));
        }

        if start_seconds + duration_seconds
            > render_duration_seconds + TEMPLATE_TIMING_TOLERANCE_SECONDS
        {
            return Err(CodexProposalError::InvalidOverlayTiming(
                "overlay must fit within render duration",
            ));
        }
    }

    Ok(())
}

fn validate_overlay_visual_treatments(
    overlays: &[serde_json::Value],
) -> Result<(), CodexProposalError> {
    for overlay in overlays {
        if let Some(visual_treatment) = overlay
            .get("visualTreatment")
            .and_then(serde_json::Value::as_str)
        {
            reject_banned_visual_treatment("overlay", visual_treatment)?;
        }
    }

    Ok(())
}

fn validate_gpu_visual_timing(
    gpu_visuals: &[serde_json::Value],
    render_duration_seconds: f64,
) -> Result<(), CodexProposalError> {
    for gpu_visual in gpu_visuals {
        let start_seconds = gpu_visual
            .get("startSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidGpuVisualTiming(
                "startSeconds must be finite and non-negative",
            ))?;
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(CodexProposalError::InvalidGpuVisualTiming(
                "startSeconds must be finite and non-negative",
            ));
        }

        let duration_seconds = gpu_visual
            .get("durationSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidGpuVisualTiming(
                "durationSeconds must be finite and positive",
            ))?;
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return Err(CodexProposalError::InvalidGpuVisualTiming(
                "durationSeconds must be finite and positive",
            ));
        }

        if start_seconds + duration_seconds
            > render_duration_seconds + TEMPLATE_TIMING_TOLERANCE_SECONDS
        {
            return Err(CodexProposalError::InvalidGpuVisualTiming(
                "gpu visual must fit within render duration",
            ));
        }
    }

    Ok(())
}

fn reject_banned_visual_treatment(
    layer_kind: &'static str,
    visual_treatment: &str,
) -> Result<(), CodexProposalError> {
    let normalized = visual_treatment.to_ascii_lowercase();
    for banned_phrase in [
        "full-width opaque black",
        "opaque black caption slab",
        "opaque black slab",
        "centered text on plain box",
        "static text-only card",
        "default-font template",
        "generic visual layer",
        "plain text box",
    ] {
        if normalized.contains(banned_phrase) {
            return Err(CodexProposalError::BannedVisualTreatment(
                layer_kind,
                banned_phrase,
            ));
        }
    }

    Ok(())
}

fn validate_template_overlays(
    overlays: &[serde_json::Value],
    render_duration_seconds: f64,
) -> Result<(), CodexProposalError> {
    for overlay in overlays {
        let Some(template_id) = overlay
            .get("templateId")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };

        let Some(template_spec) = MOTION_TEMPLATE_SPECS
            .iter()
            .find(|template| template.id == template_id)
        else {
            return Err(CodexProposalError::UnknownTemplateId(
                template_id.to_string(),
            ));
        };

        let kind = overlay
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or(CodexProposalError::MissingTemplateMetadata("kind"))?;
        if kind != "lower_third" && kind != "overlay" {
            return Err(CodexProposalError::UnsupportedTemplateKind(
                kind.to_string(),
            ));
        }

        let start_seconds = overlay
            .get("startSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidTemplateTiming(
                "startSeconds must be finite and non-negative",
            ))?;
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(CodexProposalError::InvalidTemplateTiming(
                "startSeconds must be finite and non-negative",
            ));
        }

        let duration_seconds = overlay
            .get("durationSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidTemplateTiming(
                "durationSeconds must be finite and greater than zero",
            ))?;
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return Err(CodexProposalError::InvalidTemplateTiming(
                "durationSeconds must be finite and greater than zero",
            ));
        }

        if !render_duration_seconds.is_finite()
            || start_seconds + duration_seconds - render_duration_seconds
                > TEMPLATE_TIMING_TOLERANCE_SECONDS
        {
            return Err(CodexProposalError::InvalidTemplateTiming(
                "overlay must fit within render duration",
            ));
        }

        for key in ["visualTreatment", "motion", "safeZone", "avoid"] {
            if overlay
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                return Err(CodexProposalError::MissingTemplateMetadata(key));
            }
        }
        let fields = overlay
            .get("fields")
            .and_then(serde_json::Value::as_object)
            .ok_or(CodexProposalError::MissingTemplateMetadata("fields"))?;

        for field_name in template_spec.required_fields {
            if fields
                .get(*field_name)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                return Err(CodexProposalError::EmptyTemplateField(
                    (*field_name).to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_hyperframes(
    hyperframes: &[serde_json::Value],
    render_duration_seconds: f64,
) -> Result<(), CodexProposalError> {
    for hyperframe in hyperframes {
        let kind = hyperframe
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(CodexProposalError::MissingHyperframeMetadata("kind"))?;
        if !is_supported_hyperframe_kind(kind) {
            return Err(CodexProposalError::UnsupportedHyperframeKind(
                kind.to_string(),
            ));
        }

        let start_seconds = hyperframe
            .get("startSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidHyperframeTiming(
                "startSeconds must be finite and non-negative",
            ))?;
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(CodexProposalError::InvalidHyperframeTiming(
                "startSeconds must be finite and non-negative",
            ));
        }

        let duration_seconds = hyperframe
            .get("durationSeconds")
            .and_then(serde_json::Value::as_f64)
            .ok_or(CodexProposalError::InvalidHyperframeTiming(
                "durationSeconds must be finite and greater than zero",
            ))?;
        if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
            return Err(CodexProposalError::InvalidHyperframeTiming(
                "durationSeconds must be finite and greater than zero",
            ));
        }

        if !render_duration_seconds.is_finite()
            || start_seconds + duration_seconds - render_duration_seconds
                > TEMPLATE_TIMING_TOLERANCE_SECONDS
        {
            return Err(CodexProposalError::InvalidHyperframeTiming(
                "hyperframe must fit within render duration",
            ));
        }

        for key in ["visualTreatment", "motion", "safeZone", "avoid"] {
            if hyperframe
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                return Err(CodexProposalError::MissingHyperframeMetadata(key));
            }
        }
        reject_banned_visual_treatment(
            "hyperframe",
            hyperframe
                .get("visualTreatment")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default(),
        )?;
    }

    Ok(())
}

fn is_supported_hyperframe_kind(kind: &str) -> bool {
    matches!(
        kind,
        "template_overlay"
            | "title_card"
            | "lower_third"
            | "diagram"
            | "transition"
            | "immersive_scene"
    )
}
