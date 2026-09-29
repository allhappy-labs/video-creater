//! Preset-free Codex conversation contract.
//!
//! A conversation request carries only the user's words and an internal focus.
//! Codex answers with a structured proposal; Rust validates and materializes it
//! against a cloned project before anything can mutate canonical state.

mod apply;
mod impact;
mod parse;
mod risk;
mod undo;

pub use apply::{
    apply_codex_conversation_proposal, CodexConversationApplyError, CodexConversationApplyRequest,
    CodexConversationApplyResult,
};
pub use impact::{codex_proposal_impact, CodexProposalImpact};
pub use parse::{conversation_proposal_from_text, conversation_proposal_from_value};
pub use risk::{
    classify_codex_proposal_risk, is_safe_local_action, CodexProposalRisk, CodexProposalRiskLevel,
    CodexRiskReason,
};
pub use undo::{undo_codex_conversation_edit, undo_codex_conversation_edit_with};

use super::proposal::{CodexProposalClip, CodexRenderReview};
use crate::project::action::{apply_project_action, ProjectAction, ProjectActionError};
use crate::project::model::{
    MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use crate::project::split::ProjectValidationIssue;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const TIMING_TOLERANCE_SECONDS: f64 = 1e-6;
const CONVERSATION_GENERATED_BY: &str = "codex-conversation-proposal";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationFocus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_media_id: Option<String>,
    #[serde(default)]
    pub media_ids: Vec<String>,
    #[serde(default)]
    pub timeline_item_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_range: Option<CodexConversationRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationEditRequest {
    pub prompt: String,
    #[serde(default)]
    pub focus: CodexConversationFocus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationEditProposal {
    pub summary: String,
    #[serde(default)]
    pub edl: Vec<CodexProposalClip>,
    #[serde(default)]
    pub project_actions: Vec<ProjectAction>,
    /// Serialized as `null` when absent so the bridge type stays `CodexRenderReview | null`.
    #[serde(default)]
    pub render_review: Option<CodexRenderReview>,
}

/// The exact, validated action bundle Rust derived from a proposal, with its
/// deterministic action IDs, fail-closed risk, and before/after impact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexPreparedProposal {
    pub actions: Vec<ProjectAction>,
    pub action_ids: Vec<String>,
    pub risk: CodexProposalRisk,
    pub impact: CodexProposalImpact,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexConversationTurnResult {
    pub thread_id: String,
    pub thread_response: Value,
    pub turn_response: Value,
    pub proposal: Option<CodexConversationEditProposal>,
    /// Present only when the proposal validated; carries the exact bundle.
    pub prepared_proposal: Option<CodexPreparedProposal>,
    pub proposal_validation_issues: Option<Vec<ProjectValidationIssue>>,
    /// Set when the configured model was replaced by a supported one.
    pub model_notice: Option<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum CodexConversationError {
    #[error("conversation prompt cannot be empty")]
    EmptyPrompt,
    #[error("focused media was not found: {0}")]
    FocusMediaNotFound(String),
    #[error("focused timeline item was not found: {0}")]
    FocusTimelineItemNotFound(String),
    #[error("focused timeline range is invalid: {0}")]
    InvalidTimelineRange(&'static str),
    #[error("proposal summary cannot be empty")]
    EmptySummary,
    #[error("proposal must contain at least one EDL range or project action")]
    EmptyActions,
    #[error("new primary video or audio items require a non-empty EDL")]
    MissingEdl,
    #[error("EDL media was not found: {0}")]
    EdlMediaNotFound(String),
    #[error("EDL range {index} is invalid: {reason}")]
    InvalidEdlRange { index: usize, reason: &'static str },
    #[error("render review criteria are required for an EDL or a new visual layer")]
    MissingRenderReview,
    #[error("render review duration is invalid: {0}")]
    InvalidRenderReviewDuration(&'static str),
    #[error("proposal materialization track was not found: {0:?}")]
    TrackNotFound(TrackKind),
    #[error("project action is invalid: {0}")]
    ProjectAction(ProjectActionError),
}

impl CodexConversationError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::EmptyPrompt => "codex.conversation.emptyPrompt",
            Self::FocusMediaNotFound(_) => "codex.conversation.focusMediaNotFound",
            Self::FocusTimelineItemNotFound(_) => "codex.conversation.focusTimelineItemNotFound",
            Self::InvalidTimelineRange(_) => "codex.conversation.invalidTimelineRange",
            Self::EmptySummary => "codex.conversation.emptySummary",
            Self::EmptyActions => "codex.conversation.emptyActions",
            Self::MissingEdl => "codex.conversation.missingEdl",
            Self::EdlMediaNotFound(_) => "codex.conversation.edlMediaNotFound",
            Self::InvalidEdlRange { .. } => "codex.conversation.invalidEdlRange",
            Self::MissingRenderReview => "codex.conversation.missingRenderReview",
            Self::InvalidRenderReviewDuration(_) => {
                "codex.conversation.invalidRenderReviewDuration"
            }
            Self::TrackNotFound(_) => "codex.conversation.trackNotFound",
            Self::ProjectAction(_) => "codex.conversation.invalidProjectAction",
        }
    }

    pub fn validation_issue(&self) -> ProjectValidationIssue {
        let (path, fix) = match self {
            Self::EmptyPrompt => ("prompt", "Describe the edit before sending the request"),
            Self::FocusMediaNotFound(_) => (
                "focus.mediaIds",
                "Mention media that still exists in the project library",
            ),
            Self::FocusTimelineItemNotFound(_) => (
                "focus.timelineItemIds",
                "Select timeline items that still exist on the active timeline",
            ),
            Self::InvalidTimelineRange(_) => (
                "focus.timelineRange",
                "Select an ordered range inside the active timeline",
            ),
            Self::EmptySummary => ("summary", "Describe the proposed edit in one sentence"),
            Self::EmptyActions => (
                "projectActions",
                "Return at least one project action or EDL range",
            ),
            Self::MissingEdl => (
                "edl",
                "Include an EDL before adding a new primary video or audio cut",
            ),
            Self::EdlMediaNotFound(_) | Self::InvalidEdlRange { .. } => (
                "edl",
                "Use existing video or audio media with sourceOut after sourceIn inside the source",
            ),
            Self::MissingRenderReview | Self::InvalidRenderReviewDuration(_) => (
                "renderReview",
                "Supply render-review criteria that match the EDL duration",
            ),
            Self::TrackNotFound(_) | Self::ProjectAction(_) => (
                "projectActions",
                "Use canonical unlocked targets and valid project actions",
            ),
        };
        ProjectValidationIssue {
            path: path.to_string(),
            message: self.to_string(),
            fix: fix.to_string(),
        }
    }
}

impl CodexConversationEditRequest {
    /// Validates the request against the project and returns a normalized copy
    /// whose focus IDs are trimmed and de-duplicated. The prompt is preserved
    /// exactly as the user wrote it.
    pub fn validate(&self, project: &VideoProject) -> Result<Self, CodexConversationError> {
        if self.prompt.trim().is_empty() {
            return Err(CodexConversationError::EmptyPrompt);
        }
        let media_exists = |id: &str| project.media.iter().any(|media| media.id == id);
        let mut seen_media = BTreeSet::new();
        let primary_media_id = normalized_id(self.focus.primary_media_id.as_deref());
        if let Some(id) = &primary_media_id {
            if !media_exists(id) {
                return Err(CodexConversationError::FocusMediaNotFound(id.clone()));
            }
            seen_media.insert(id.clone());
        }
        let mut media_ids = Vec::new();
        for id in self
            .focus
            .media_ids
            .iter()
            .filter_map(|id| normalized_id(Some(id)))
        {
            if !seen_media.insert(id.clone()) {
                continue;
            }
            if !media_exists(&id) {
                return Err(CodexConversationError::FocusMediaNotFound(id));
            }
            media_ids.push(id);
        }

        let mut seen_items = BTreeSet::new();
        let mut timeline_item_ids = Vec::new();
        for id in self
            .focus
            .timeline_item_ids
            .iter()
            .filter_map(|id| normalized_id(Some(id)))
        {
            if !seen_items.insert(id.clone()) {
                continue;
            }
            let exists = project
                .timeline
                .tracks
                .iter()
                .any(|track| track.items.iter().any(|item| item.id == id));
            if !exists {
                return Err(CodexConversationError::FocusTimelineItemNotFound(id));
            }
            timeline_item_ids.push(id);
        }

        if let Some(range) = &self.focus.timeline_range {
            validate_timeline_range(project, range)?;
        }

        Ok(Self {
            prompt: self.prompt.clone(),
            focus: CodexConversationFocus {
                primary_media_id,
                media_ids,
                timeline_item_ids,
                timeline_range: self.focus.timeline_range.clone(),
            },
            created_at: self.created_at.clone(),
        })
    }
}

fn normalized_id(id: Option<&str>) -> Option<String> {
    id.map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn validate_timeline_range(
    project: &VideoProject,
    range: &CodexConversationRange,
) -> Result<(), CodexConversationError> {
    if !range.start_seconds.is_finite() || !range.end_seconds.is_finite() {
        return Err(CodexConversationError::InvalidTimelineRange(
            "range bounds must be finite",
        ));
    }
    if range.start_seconds < 0.0 {
        return Err(CodexConversationError::InvalidTimelineRange(
            "range start cannot be negative",
        ));
    }
    if range.end_seconds <= range.start_seconds {
        return Err(CodexConversationError::InvalidTimelineRange(
            "range end must be after range start",
        ));
    }
    if range.end_seconds > timeline_end_seconds(project) + TIMING_TOLERANCE_SECONDS {
        return Err(CodexConversationError::InvalidTimelineRange(
            "range must end inside the timeline",
        ));
    }
    Ok(())
}

fn timeline_end_seconds(project: &VideoProject) -> f64 {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.start_seconds + item.duration_seconds)
        .fold(project.timeline.duration_seconds, f64::max)
}

/// Validates a conversation proposal, materializes it once, applies the exact
/// actions to a clone, and returns the bundle with its risk and impact.
/// Nothing here mutates the supplied project.
pub fn prepare_codex_conversation_proposal(
    project: &VideoProject,
    proposal: &CodexConversationEditProposal,
) -> Result<CodexPreparedProposal, CodexConversationError> {
    if proposal.summary.trim().is_empty() {
        return Err(CodexConversationError::EmptySummary);
    }
    if proposal.edl.is_empty() && proposal.project_actions.is_empty() {
        return Err(CodexConversationError::EmptyActions);
    }
    let edl_duration_seconds = validate_edl(project, &proposal.edl)?;
    let added_items = added_items(&proposal.project_actions);
    if proposal.edl.is_empty() && added_items.iter().any(|item| is_primary_media_item(item)) {
        return Err(CodexConversationError::MissingEdl);
    }
    let adds_visual_layer = added_items.iter().any(|item| !is_primary_media_item(item));
    validate_render_review(
        proposal.render_review.as_ref(),
        edl_duration_seconds,
        !proposal.edl.is_empty() || adds_visual_layer,
    )?;

    let actions = materialize_actions(project, proposal)?;
    if actions.is_empty() {
        return Err(CodexConversationError::EmptyActions);
    }
    let mut next_project = project.clone();
    for action in &actions {
        apply_project_action(&mut next_project, action.clone())
            .map_err(CodexConversationError::ProjectAction)?;
    }
    Ok(CodexPreparedProposal {
        action_ids: prepared_action_ids(&actions),
        risk: classify_codex_proposal_risk(&actions),
        impact: codex_proposal_impact(project, &next_project),
        actions,
    })
}

/// Deterministic IDs: the position plus a digest of the canonical action JSON,
/// so a recomputed bundle for the same proposal yields the same IDs.
fn prepared_action_ids(actions: &[ProjectAction]) -> Vec<String> {
    actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let canonical = serde_json::to_vec(action).unwrap_or_default();
            let digest = Sha256::digest(&canonical);
            let short = digest[..6]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("codex-action-{}-{short}", index + 1)
        })
        .collect()
}

fn validate_edl(
    project: &VideoProject,
    edl: &[CodexProposalClip],
) -> Result<f64, CodexConversationError> {
    let mut duration_seconds = 0.0;
    for (index, clip) in edl.iter().enumerate() {
        let media_id = clip.media_id.trim();
        let media = project
            .media
            .iter()
            .find(|media| media.id == media_id)
            .ok_or_else(|| CodexConversationError::EdlMediaNotFound(media_id.to_string()))?;
        if matches!(media.kind, MediaKind::Image | MediaKind::Lottie) {
            return Err(CodexConversationError::InvalidEdlRange {
                index,
                reason: "EDL media must be a video or audio source",
            });
        }
        if !clip.source_in.is_finite() || !clip.source_out.is_finite() || clip.source_in < 0.0 {
            return Err(CodexConversationError::InvalidEdlRange {
                index,
                reason: "source times must be finite and non-negative",
            });
        }
        if clip.source_out <= clip.source_in {
            return Err(CodexConversationError::InvalidEdlRange {
                index,
                reason: "sourceOut must be after sourceIn",
            });
        }
        if clip.source_out > media.duration_seconds + TIMING_TOLERANCE_SECONDS {
            return Err(CodexConversationError::InvalidEdlRange {
                index,
                reason: "sourceOut must stay inside the source duration",
            });
        }
        duration_seconds += clip.source_out - clip.source_in;
    }
    Ok(duration_seconds)
}

fn validate_render_review(
    render_review: Option<&CodexRenderReview>,
    edl_duration_seconds: f64,
    required: bool,
) -> Result<(), CodexConversationError> {
    let Some(render_review) = render_review else {
        return if required {
            Err(CodexConversationError::MissingRenderReview)
        } else {
            Ok(())
        };
    };
    let duration = render_review.duration_seconds;
    if !duration.is_finite() || duration <= 0.0 {
        return Err(CodexConversationError::InvalidRenderReviewDuration(
            "renderReview.durationSeconds must be finite and positive",
        ));
    }
    if edl_duration_seconds > 0.0
        && (duration - edl_duration_seconds).abs() > TIMING_TOLERANCE_SECONDS
    {
        return Err(CodexConversationError::InvalidRenderReviewDuration(
            "renderReview.durationSeconds must match the EDL duration",
        ));
    }
    Ok(())
}

fn added_items(actions: &[ProjectAction]) -> Vec<&TimelineItem> {
    actions
        .iter()
        .flat_map(|action| match action {
            ProjectAction::AddItems { items, .. } | ProjectAction::InsertItems { items, .. } => {
                items.iter().collect::<Vec<_>>()
            }
            ProjectAction::CreateTrack { track, .. } => track.items.iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn is_primary_media_item(item: &TimelineItem) -> bool {
    matches!(
        item.kind,
        TimelineItemKind::VideoClip | TimelineItemKind::AudioClip
    )
}

fn adds_items(action: &ProjectAction) -> bool {
    match action {
        ProjectAction::AddItems { .. } | ProjectAction::InsertItems { .. } => true,
        ProjectAction::CreateTrack { track, .. } => !track.items.is_empty(),
        _ => false,
    }
}

/// Places EDL-derived primary items after preparatory actions (removals, empty
/// track creation, local edits) and before the first action that adds items.
fn materialize_actions(
    project: &VideoProject,
    proposal: &CodexConversationEditProposal,
) -> Result<Vec<ProjectAction>, CodexConversationError> {
    if proposal.edl.is_empty() {
        return Ok(proposal.project_actions.clone());
    }
    let split_index = proposal
        .project_actions
        .iter()
        .position(adds_items)
        .unwrap_or(proposal.project_actions.len());
    let (leading, trailing) = proposal.project_actions.split_at(split_index);
    let mut staged = project.clone();
    for action in leading {
        apply_project_action(&mut staged, action.clone())
            .map_err(CodexConversationError::ProjectAction)?;
    }

    let mut used_ids = existing_item_ids(&staged);
    let mut video_items = Vec::new();
    let mut audio_items = Vec::new();
    let mut cursor = 0.0;
    for (index, clip) in proposal.edl.iter().enumerate() {
        let media_id = clip.media_id.trim().to_string();
        let duration = clip.source_out - clip.source_in;
        let properties = BTreeMap::from([
            ("sourceIn".to_string(), json!(clip.source_in)),
            ("sourceOut".to_string(), json!(clip.source_out)),
            ("reason".to_string(), json!(clip.reason)),
            ("generatedBy".to_string(), json!(CONVERSATION_GENERATED_BY)),
        ]);
        let is_audio_only = staged
            .media
            .iter()
            .any(|media| media.id == media_id && media.kind == MediaKind::Audio);
        let mut edl_item = |role: &str, kind: TimelineItemKind, label: String| TimelineItem {
            id: unique_item_id(&mut used_ids, role),
            kind,
            start_seconds: cursor,
            duration_seconds: duration,
            source: TimelineSource::Media {
                media_id: media_id.clone(),
            },
            label,
            properties: properties.clone(),
        };
        if !is_audio_only {
            video_items.push(edl_item(
                "video",
                TimelineItemKind::VideoClip,
                format!("Cut clip {}", index + 1),
            ));
        }
        audio_items.push(edl_item(
            "audio",
            TimelineItemKind::AudioClip,
            format!("Cut audio {}", index + 1),
        ));
        cursor += duration;
    }

    let mut actions = leading.to_vec();
    if !video_items.is_empty() {
        actions.push(ProjectAction::AddItems {
            target_track_id: first_track_id(&staged, TrackKind::Video)?,
            items: video_items,
        });
    }
    actions.push(ProjectAction::AddItems {
        target_track_id: first_track_id(&staged, TrackKind::Audio)?,
        items: audio_items,
    });
    actions.extend(trailing.iter().cloned());
    Ok(actions)
}

fn existing_item_ids(project: &VideoProject) -> BTreeSet<String> {
    project
        .timeline
        .tracks
        .iter()
        .chain(
            project
                .timelines
                .iter()
                .flat_map(|timeline| timeline.timeline.tracks.iter()),
        )
        .flat_map(|track| track.items.iter().map(|item| item.id.clone()))
        .collect()
}

fn unique_item_id(used_ids: &mut BTreeSet<String>, role: &str) -> String {
    let mut ordinal = 1usize;
    loop {
        let candidate = format!("codex-conversation-{role}-{ordinal}");
        if used_ids.insert(candidate.clone()) {
            return candidate;
        }
        ordinal += 1;
    }
}

fn first_track_id(
    project: &VideoProject,
    kind: TrackKind,
) -> Result<String, CodexConversationError> {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == kind)
        .map(|track| track.id.clone())
        .ok_or(CodexConversationError::TrackNotFound(kind))
}
