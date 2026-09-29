use super::action::{
    apply_project_action, ProjectAction, ProjectActionError, ProjectActionMove,
    ProjectActionResize, ProjectActionTrim,
};
use super::model::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TimelinePatch {
    MoveItem {
        item_id: String,
        target_track_id: String,
        start_seconds: f64,
    },
    ResizeItem {
        item_id: String,
        duration_seconds: f64,
    },
    TrimItem {
        item_id: String,
        start_seconds: f64,
        duration_seconds: f64,
        source_in: Option<f64>,
        source_out: Option<f64>,
    },
    EditCaptionText {
        item_id: String,
        text: String,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum TimelinePatchError {
    #[error("time values cannot be negative")]
    NegativeTime,
    #[error("duration must be greater than zero")]
    NonPositiveDuration,
    #[error("timeline item was not found: {0}")]
    ItemNotFound(String),
    #[error("timeline track was not found: {0}")]
    TrackNotFound(String),
    #[error("track is locked: {0}")]
    TrackLocked(String),
    #[error("item kind {item_kind:?} is incompatible with track kind {track_kind:?}")]
    TrackTypeMismatch {
        item_kind: TimelineItemKind,
        track_kind: TrackKind,
    },
    #[error("caption text cannot be empty")]
    EmptyCaptionText,
    #[error("timeline item is not a caption: {0}")]
    NotCaptionItem(String),
    #[error("timeline patch is invalid: {0}")]
    InvalidPatch(String),
}

pub fn apply_timeline_patch(
    project: &mut VideoProject,
    patch: TimelinePatch,
) -> Result<(), TimelinePatchError> {
    let action = match patch {
        TimelinePatch::MoveItem {
            item_id,
            target_track_id,
            start_seconds,
        } => ProjectAction::MoveItems {
            moves: vec![ProjectActionMove {
                item_id,
                target_track_id,
                start_seconds,
            }],
        },
        TimelinePatch::ResizeItem {
            item_id,
            duration_seconds,
        } => ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id,
                duration_seconds,
            }],
        },
        TimelinePatch::TrimItem {
            item_id,
            start_seconds,
            duration_seconds,
            source_in,
            source_out,
        } => ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id,
                start_seconds,
                duration_seconds,
                source_in,
                source_out,
            }],
        },
        TimelinePatch::EditCaptionText { item_id, text } => {
            ProjectAction::EditCaptionText { item_id, text }
        }
    };

    apply_project_action(project, action).map_err(map_action_error)
}

fn map_action_error(error: ProjectActionError) -> TimelinePatchError {
    // TimelinePatch is a compatibility surface over the richer ProjectAction API. Keep the
    // common patch errors typed, but never let a newly reachable action validation error abort the
    // app; preserve its actionable message as a recoverable patch error instead.
    let action_error_message = error.to_string();
    match error {
        ProjectActionError::NegativeTime => TimelinePatchError::NegativeTime,
        ProjectActionError::NonPositiveDuration => TimelinePatchError::NonPositiveDuration,
        ProjectActionError::TrackNotFound(track_id) => TimelinePatchError::TrackNotFound(track_id),
        ProjectActionError::ItemNotFound(item_id) => TimelinePatchError::ItemNotFound(item_id),
        ProjectActionError::TrackLocked(track_id) => TimelinePatchError::TrackLocked(track_id),
        ProjectActionError::EmptyCaptionText => TimelinePatchError::EmptyCaptionText,
        ProjectActionError::NotCaptionItem(item_id) => TimelinePatchError::NotCaptionItem(item_id),
        ProjectActionError::TrackTypeMismatch {
            item_kind,
            track_kind,
        } => TimelinePatchError::TrackTypeMismatch {
            item_kind,
            track_kind,
        },
        ProjectActionError::EmptyItems
        | ProjectActionError::InvalidTrackZone
        | ProjectActionError::DuplicateItemId(_)
        | ProjectActionError::DuplicateTrackId(_)
        | ProjectActionError::EmptyTrackId
        | ProjectActionError::EmptyTrackName
        | ProjectActionError::EmptyMediaName
        | ProjectActionError::DuplicateMediaId(_)
        | ProjectActionError::MediaInUse(_)
        | ProjectActionError::TrackNotEmpty(_)
        | ProjectActionError::DuplicateGeneratedAssetId(_)
        | ProjectActionError::InvalidGeneratedAssetId(_)
        | ProjectActionError::MissingMedia(_)
        | ProjectActionError::MissingMediaFolder(_)
        | ProjectActionError::EmptyMediaFolderId
        | ProjectActionError::DuplicateMediaFolderId(_)
        | ProjectActionError::InvalidMediaFolderId(_)
        | ProjectActionError::EmptyMediaFolderName
        | ProjectActionError::MissingMediaFolderParent(_)
        | ProjectActionError::EmptyTextItemText
        | ProjectActionError::NotTextItem(_)
        | ProjectActionError::NotAudioClip(_)
        | ProjectActionError::InvalidAudioVolume(_)
        | ProjectActionError::InvalidAudioFades(_)
        | ProjectActionError::InvalidAudioClipSpeed(_)
        | ProjectActionError::NoDetachableAudio(_)
        | ProjectActionError::AudioAlreadyDetached(_)
        | ProjectActionError::NotReversibleClip(_)
        | ProjectActionError::InvalidAudioSync(_)
        | ProjectActionError::NotVisualClip(_)
        | ProjectActionError::InvalidVisualClipOpacity(_)
        | ProjectActionError::InvalidVisualClipTransform(_)
        | ProjectActionError::InvalidVisualClipCrop(_)
        | ProjectActionError::InvalidVisualClipFades(_)
        | ProjectActionError::InvalidVisualClipSpeed(_)
        | ProjectActionError::NotTextOverlayItem(_)
        | ProjectActionError::MissingTextOverlayVisualMetadata(_)
        | ProjectActionError::NotTemplateItem(_)
        | ProjectActionError::EmptyTemplateId
        | ProjectActionError::EmptyTemplateName
        | ProjectActionError::MissingTemplateVisualMetadata(_)
        | ProjectActionError::TranscriptNotFound(_)
        | ProjectActionError::TranscriptWordNotFound { .. }
        | ProjectActionError::EmptyTranscriptText
        | ProjectActionError::NonMonotonicTranscriptTiming(_)
        | ProjectActionError::EmptyGeneratedPrompt
        | ProjectActionError::EmptyGeneratedModel
        | ProjectActionError::InvalidGeneratedAssetSettings(_)
        | ProjectActionError::InvalidGeneratedAssetKind(_)
        | ProjectActionError::EmptyGeneratedMediaReference
        | ProjectActionError::InvalidGeneratedFrameReference(_)
        | ProjectActionError::EmptyGeneratedOutputMediaId
        | ProjectActionError::EmptyGeneratedOutputPath
        | ProjectActionError::GeneratedOutputReservedPath(_)
        | ProjectActionError::GeneratedOutputOutsideAssetDirectory(_)
        | ProjectActionError::MissingGeneratedAssetReference(_)
        | ProjectActionError::GeneratedAssetAlreadyCompleted(_)
        | ProjectActionError::CompletionReplacementOutputMismatch(_)
        | ProjectActionError::MissingGeneratedOutput(_)
        | ProjectActionError::NotGeneratedOutput(_)
        | ProjectActionError::DuplicateRenderReportId(_)
        | ProjectActionError::EmptyRenderOutputPath
        | ProjectActionError::EmptyRenderStreams
        | ProjectActionError::EmptyRenderArtifacts
        | ProjectActionError::EmptyRenderLogPath
        | ProjectActionError::MissingRenderCheck(_)
        | ProjectActionError::EmptyExportArtifactId
        | ProjectActionError::InvalidExportArtifactId(_)
        | ProjectActionError::DuplicateExportArtifactId(_)
        | ProjectActionError::EmptyExportArtifactFormat
        | ProjectActionError::EmptyExportArtifactPath
        | ProjectActionError::UnsafeExportArtifactPath(_)
        | ProjectActionError::InvalidExportArtifactContract(_)
        | ProjectActionError::EmptyExportArtifactMimeType
        | ProjectActionError::EmptyExportArtifactCreatedAt
        | ProjectActionError::InvalidRenderSettings(_)
        | ProjectActionError::InvalidEffectStack(_)
        | ProjectActionError::InvalidEffectParam(_)
        | ProjectActionError::KeyframeNotFound { .. }
        | ProjectActionError::EffectInstanceNotFound { .. }
        | ProjectActionError::RippleTrimCollision { .. }
        | ProjectActionError::TrackItemOverlap { .. }
        | ProjectActionError::InvalidColorGrade(_)
        | ProjectActionError::EmptyJobId
        | ProjectActionError::InvalidJobId(_)
        | ProjectActionError::DuplicateJobId(_)
        | ProjectActionError::JobNotFound(_)
        | ProjectActionError::TerminalJobStatus(_)
        | ProjectActionError::StaleRenderAttempt(_)
        | ProjectActionError::TerminalGeneratedAssetStatus(_)
        | ProjectActionError::EmptyJobKind
        | ProjectActionError::EmptyJobUpdatedAt
        | ProjectActionError::EmptyJobFailureReason
        | ProjectActionError::MissingJobProviderRequestMetadata(_)
        | ProjectActionError::InvalidJobExportSettings(_)
        | ProjectActionError::MissingJobWorkflowMetadata(_)
        | ProjectActionError::MismatchedJobStartRequest(_)
        | ProjectActionError::SplitPointOutsideItem(_)
        | ProjectActionError::SourceRangeDurationMismatch(_)
        | ProjectActionError::SourceRangeOutsideMedia(_)
        | ProjectActionError::MissingGeneratedEditSourceRange(_)
        | ProjectActionError::GeneratedEditFullSourcePassThrough(_)
        | ProjectActionError::InvalidTransition(_)
        | ProjectActionError::TransitionNotFound(_) => {
            TimelinePatchError::InvalidPatch(action_error_message)
        }
    }
}
