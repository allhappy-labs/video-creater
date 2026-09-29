//! Fail-closed risk classification for prepared conversation proposals.
//!
//! Only an explicit allowlist of local, recoverable timeline edits is `Safe`.
//! Known consequential classes get a specific reason code; any other variant,
//! including actions added after this allowlist was written, is `Review` with
//! `unclassifiedAction`.

use crate::project::action::ProjectAction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CodexProposalRiskLevel {
    Safe,
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexRiskReason {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalRisk {
    pub level: CodexProposalRiskLevel,
    pub reasons: Vec<CodexRiskReason>,
}

/// Classifies an exact action bundle. One review-level action makes the whole
/// bundle `Review`; reasons are de-duplicated by code in first-seen order.
pub fn classify_codex_proposal_risk(actions: &[ProjectAction]) -> CodexProposalRisk {
    let mut reasons: Vec<CodexRiskReason> = Vec::new();
    for (code, message) in actions.iter().filter_map(review_reason) {
        if reasons.iter().all(|reason| reason.code != code) {
            reasons.push(CodexRiskReason {
                code: code.to_string(),
                message: message.to_string(),
            });
        }
    }
    CodexProposalRisk {
        level: if reasons.is_empty() {
            CodexProposalRiskLevel::Safe
        } else {
            CodexProposalRiskLevel::Review
        },
        reasons,
    }
}

/// True only for local timeline edits that can apply without user review.
pub fn is_safe_local_action(action: &ProjectAction) -> bool {
    review_reason(action).is_none()
}

fn review_reason(action: &ProjectAction) -> Option<(&'static str, &'static str)> {
    match action {
        // Additions, placement, and local timing edits.
        ProjectAction::AddItems { .. }
        | ProjectAction::InsertItems { .. }
        | ProjectAction::MoveItems { .. }
        | ProjectAction::ReorderItems { .. }
        | ProjectAction::ResizeItems { .. }
        | ProjectAction::TrimItems { .. }
        | ProjectAction::RippleTrimItem { .. }
        | ProjectAction::RippleDeleteRanges { .. }
        | ProjectAction::SplitItems { .. }
        // Non-destructive track creation and track state.
        | ProjectAction::CreateTrack { .. }
        | ProjectAction::SetTrackLocked { .. }
        | ProjectAction::ReorderTrack { .. }
        | ProjectAction::SetTrackSyncLocked { .. }
        | ProjectAction::SetTrackEnabled { .. }
        // Caption, text, audio, visual, property, keyframe, effect, and color updates.
        | ProjectAction::EditCaptionText { .. }
        | ProjectAction::EditTextItem { .. }
        | ProjectAction::UpdateAudioFadeOut { .. }
        | ProjectAction::UpdateAudioFades { .. }
        | ProjectAction::UpdateAudioVolume { .. }
        | ProjectAction::UpdateAudioClipSpeed { .. }
        | ProjectAction::UpdateClipReverse { .. }
        | ProjectAction::DetachAudio { .. }
        | ProjectAction::UpdateAudioSync { .. }
        | ProjectAction::UpdateVisualClipOpacity { .. }
        | ProjectAction::UpdateVisualClipTransform { .. }
        | ProjectAction::UpdateVisualClipCrop { .. }
        | ProjectAction::UpdateVisualClipFades { .. }
        | ProjectAction::UpdateVisualClipSpeed { .. }
        | ProjectAction::SetItemKeyframes { .. }
        | ProjectAction::UpsertItemKeyframe { .. }
        | ProjectAction::MoveItemKeyframe { .. }
        | ProjectAction::DeleteItemKeyframe { .. }
        | ProjectAction::UpsertEffectParameterKeyframe { .. }
        | ProjectAction::MoveEffectParameterKeyframe { .. }
        | ProjectAction::DeleteEffectParameterKeyframe { .. }
        | ProjectAction::UpdateItemProperties { .. }
        | ProjectAction::UpdateItemEffects { .. }
        | ProjectAction::UpdateItemColorGrade { .. }
        // Links, repairs, and local text/template item updates.
        | ProjectAction::LinkItems { .. }
        | ProjectAction::UnlinkItems { .. }
        | ProjectAction::ApplyCaptionRepair { .. }
        | ProjectAction::EditTranscriptWords { .. }
        | ProjectAction::UpdateTemplateItems { .. }
        | ProjectAction::UpdateTextOverlayItems { .. }
        // Clip transitions: validated against adjacent clips, never delete media.
        | ProjectAction::AddTransition { .. }
        | ProjectAction::UpdateTransition { .. }
        | ProjectAction::RemoveTransition { .. } => None,

        ProjectAction::RemoveItems { .. } => Some((
            "deletesExistingItems",
            "Removes existing clips from the timeline.",
        )),
        ProjectAction::RemoveTracks { .. } => {
            Some(("removesTracks", "Removes whole tracks from the timeline."))
        }
        ProjectAction::DeleteTimeline { .. } => Some(("deletesTimeline", "Deletes a timeline.")),
        ProjectAction::DeleteMedia { .. } => Some((
            "deletesMedia",
            "Deletes media from the project library.",
        )),
        ProjectAction::DeleteMediaFolder { .. } => Some((
            "deletesMediaFolder",
            "Deletes a folder from the project library.",
        )),
        ProjectAction::DecomposeTimelineItem { .. } => Some((
            "decomposesItem",
            "Breaks a clip apart into separate layers.",
        )),
        ProjectAction::RecordGeneratedAsset { .. }
        | ProjectAction::UpdateGeneratedAssetStatus { .. }
        | ProjectAction::UpdateGeneratedAssetReferences { .. }
        | ProjectAction::CompleteGeneratedAsset { .. }
        | ProjectAction::ReplaceTimelineItemWithGeneratedOutput { .. } => Some((
            "changesGeneratedAssets",
            "Starts, changes, or places generated media.",
        )),
        ProjectAction::RecordJob { .. }
        | ProjectAction::UpdateJobStatus { .. }
        | ProjectAction::UpdateJobProviderRequest { .. }
        | ProjectAction::RecordJobFailure { .. } => Some((
            "changesJobs",
            "Starts or changes a background job or provider request.",
        )),
        ProjectAction::AttachRenderReport { .. } | ProjectAction::RecordExportArtifact { .. } => {
            Some((
                "changesRenderMetadata",
                "Changes render or export records.",
            ))
        }
        ProjectAction::UpdateProjectSettings { .. } | ProjectAction::UpdateRenderSettings { .. } => {
            Some((
                "changesProjectSettings",
                "Changes project or render settings.",
            ))
        }
        ProjectAction::UpdateTemplateOverride { .. } => Some((
            "changesTemplateOverride",
            "Changes a template for every place it is used.",
        )),
        // Unmatched and future actions fail closed.
        _ => Some((
            "unclassifiedAction",
            "Includes a change that needs your review.",
        )),
    }
}
