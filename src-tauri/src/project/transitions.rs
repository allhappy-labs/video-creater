//! Clip-to-clip transitions stored on a track and centered on a cut.
//!
//! The canonical timeline never overlaps the two clips of a transition. A
//! transition of `d` seconds instead needs `d / 2` seconds of unused source
//! media on each side of the cut, which renderers use to extend both clips.
//!
//! # Handle math
//!
//! A clip's *handles* are the unused source media beyond its selected range,
//! expressed in **timeline seconds**:
//!
//! - tail handle (left clip): `(sourceDuration - sourceOut) / speed`
//! - head handle (right clip): `sourceIn / speed`
//!
//! Reversed clips swap them (see [`super::reverse`]): their tail handle is
//! `sourceIn / speed` and their head handle `(sourceDuration - sourceOut) / speed`.
//!
//! Source seconds are divided by the clip speed because extending a clip by
//! `x` timeline seconds consumes `x * speed` source seconds. `sourceIn`
//! defaults to 0 and `sourceOut` to `sourceIn + duration * speed`, matching
//! the rest of the timeline model.
//!
//! Handles are unlimited (`None`) when the source has no finite duration to run
//! out of: image clips, still-image media, media with a zero or non-finite
//! duration, and sources that cannot be resolved to project media. Generated
//! sources resolve through their completed asset's first output. Audio clips use
//! their source media duration like video clips.
//!
//! The maximum duration is
//! `min(5, 2 * leftTailHandle, 2 * rightHeadHandle, left.duration, right.duration)`,
//! and the minimum is one frame at the project frame rate.

use super::action::{item_speed, ProjectActionError};
use super::model::*;
use super::reverse::{is_reversed, SourceWindow};

mod maintenance;

pub use maintenance::{maintain_transitions, retarget_transitions_to_split_tails};

/// The longest transition, in seconds.
pub const MAX_TRANSITION_SECONDS: f64 = 5.0;
const SECONDS_EPSILON: f64 = 0.000_001;

/// Which constraint sets a transition's maximum duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionLimit {
    /// The 5 s cap.
    Cap,
    /// Unused media after the left clip's `sourceOut`.
    LeftHandle,
    /// Unused media before the right clip's `sourceIn`.
    RightHandle,
    /// The left clip's timeline duration.
    LeftDuration,
    /// The right clip's timeline duration.
    RightDuration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionBounds {
    pub max_seconds: f64,
    pub limit: TransitionLimit,
}

/// One frame at the project frame rate (24 fps when the setting is invalid).
pub fn frame_seconds(project: &VideoProject) -> f64 {
    let fps = project.render_settings.fps;
    if fps.is_finite() && fps > 0.0 {
        1.0 / fps
    } else {
        1.0 / 24.0
    }
}

/// The maximum duration and its limiting constraint for a transition between
/// `left` and `right`. Ties report the first constraint in declaration order.
pub fn transition_bounds(
    project: &VideoProject,
    left: &TimelineItem,
    right: &TimelineItem,
) -> TransitionBounds {
    let doubled = |handle: Option<f64>| handle.map_or(f64::INFINITY, |seconds| 2.0 * seconds);
    [
        (MAX_TRANSITION_SECONDS, TransitionLimit::Cap),
        (
            doubled(item_tail_handle_seconds(project, left)),
            TransitionLimit::LeftHandle,
        ),
        (
            doubled(item_head_handle_seconds(project, right)),
            TransitionLimit::RightHandle,
        ),
        (left.duration_seconds, TransitionLimit::LeftDuration),
        (right.duration_seconds, TransitionLimit::RightDuration),
    ]
    .into_iter()
    .fold(
        TransitionBounds {
            max_seconds: f64::INFINITY,
            limit: TransitionLimit::Cap,
        },
        |best, (seconds, limit)| {
            let seconds = if seconds.is_finite() {
                seconds.max(0.0)
            } else if seconds == f64::INFINITY {
                seconds
            } else {
                0.0
            };
            if seconds < best.max_seconds - SECONDS_EPSILON {
                TransitionBounds {
                    max_seconds: seconds,
                    limit,
                }
            } else {
                best
            }
        },
    )
}

/// `min(5, 2 * leftHandle, 2 * rightHandle, left.duration, right.duration)`.
pub fn transition_max_duration(
    project: &VideoProject,
    left: &TimelineItem,
    right: &TimelineItem,
) -> f64 {
    transition_bounds(project, left, right).max_seconds
}

/// Unused source media after the clip's end, in timeline seconds; `None` is unlimited.
pub fn item_tail_handle_seconds(project: &VideoProject, item: &TimelineItem) -> Option<f64> {
    let source_duration = item_source_media_duration(project, item)?;
    Some(item_source_window(item).tail_handle_seconds(source_duration))
}

/// Unused source media before the clip's start, in timeline seconds; `None` is unlimited.
pub fn item_head_handle_seconds(project: &VideoProject, item: &TimelineItem) -> Option<f64> {
    let source_duration = item_source_media_duration(project, item)?;
    Some(item_source_window(item).head_handle_seconds(source_duration))
}

fn item_source_window(item: &TimelineItem) -> SourceWindow {
    let speed = item_speed(item).unwrap_or(1.0);
    let number = |key: &str| {
        item.properties
            .get(key)
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
    };
    let source_in = number("sourceIn").unwrap_or(0.0);
    let source_out = number("sourceOut").unwrap_or(source_in + item.duration_seconds * speed);
    SourceWindow {
        source_in,
        source_out,
        speed,
        reverse: is_reversed(item),
    }
}

/// The finite source duration a clip can run out of, or `None` when unlimited.
fn item_source_media_duration(project: &VideoProject, item: &TimelineItem) -> Option<f64> {
    if item.kind == TimelineItemKind::ImageClip {
        return None;
    }
    let (media_id, output_duration) = match &item.source {
        TimelineSource::Media { media_id } => (media_id.as_str(), None),
        TimelineSource::Generated { artifact_id } => {
            let output = project
                .generated_assets
                .iter()
                .find(|asset| {
                    asset.id == *artifact_id && asset.status == GeneratedAssetStatus::Completed
                })?
                .outputs
                .first()?;
            (output.media_id.as_str(), Some(output.duration_seconds))
        }
        TimelineSource::Timeline { .. } | TimelineSource::Text { .. } => return None,
    };
    let duration = match project.media.iter().find(|media| media.id == media_id) {
        Some(media) if media.kind == MediaKind::Image => return None,
        Some(media) => media.duration_seconds,
        None => output_duration?,
    };
    (duration.is_finite() && duration > 0.0).then_some(duration)
}

fn is_visual_transition_kind(kind: &TimelineItemKind) -> bool {
    matches!(
        kind,
        TimelineItemKind::VideoClip | TimelineItemKind::ImageClip | TimelineItemKind::GeneratedClip
    )
}

fn is_transition_item_kind(kind: &TimelineItemKind) -> bool {
    is_visual_transition_kind(kind) || *kind == TimelineItemKind::AudioClip
}

fn invalid(message: impl Into<String>) -> ProjectActionError {
    ProjectActionError::InvalidTransition(message.into())
}

fn label(item: &TimelineItem) -> &str {
    let label = item.label.trim();
    if label.is_empty() {
        "this clip"
    } else {
        label
    }
}

/// Seconds for messages: `1.0`, `0.4`, `0.25`.
fn format_seconds(seconds: f64) -> String {
    let hundredths = (seconds * 100.0).round();
    if hundredths % 10.0 == 0.0 {
        format!("{:.1}", hundredths / 100.0)
    } else {
        format!("{:.2}", hundredths / 100.0)
    }
}

/// A maximum rounded down so the reported value is always accepted.
fn format_max_seconds(seconds: f64) -> String {
    format_seconds((seconds * 100.0 + SECONDS_EPSILON).floor() / 100.0)
}

fn find_in_timeline<'a>(timeline: &'a Timeline, item_id: &str) -> Option<&'a TimelineItem> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == item_id)
}

/// Whether `left` ends where `right` starts, within one frame.
pub fn items_are_adjacent(left: &TimelineItem, right: &TimelineItem, frame_seconds: f64) -> bool {
    left.start_seconds < right.start_seconds
        && (left.start_seconds + left.duration_seconds - right.start_seconds).abs()
            <= frame_seconds + SECONDS_EPSILON
}

/// Validates a transition against `track`: both items on the track, compatible
/// kinds, adjacency, no other transition on the cut, and duration bounds.
/// Transition id uniqueness is checked by the actions.
pub fn validate_transition(
    project: &VideoProject,
    track: &TimelineTrack,
    transition: &TimelineTransition,
) -> Result<(), ProjectActionError> {
    if transition.left_item_id == transition.right_item_id {
        return Err(invalid("A transition needs two different clips."));
    }
    let on_track = |item_id: &str| track.items.iter().find(|item| item.id == item_id);
    let anywhere = |item_id: &str| {
        on_track(item_id)
            .or_else(|| find_in_timeline(&project.timeline, item_id))
            .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))
    };
    let (Some(left), Some(right)) = (
        on_track(&transition.left_item_id),
        on_track(&transition.right_item_id),
    ) else {
        let left = anywhere(&transition.left_item_id)?;
        let right = anywhere(&transition.right_item_id)?;
        return Err(invalid(format!(
            "{} and {} must be on the same track to add a transition.",
            label(left),
            label(right)
        )));
    };
    validate_transition_items(project, track, transition, left, right)
}

/// Structural checks shared by validation and maintenance: eligible kinds and
/// sources, no visual/audio mix, and adjacency within one frame.
fn validate_transition_pair(
    left: &TimelineItem,
    right: &TimelineItem,
    frame_seconds: f64,
) -> Result<(), ProjectActionError> {
    if !is_transition_item_kind(&left.kind) || !is_transition_item_kind(&right.kind) {
        return Err(invalid(
            "Transitions only work between video, image, generated, or audio clips.",
        ));
    }
    for item in [left, right] {
        match item.source {
            TimelineSource::Media { .. } | TimelineSource::Generated { .. } => {}
            TimelineSource::Timeline { .. } => {
                return Err(invalid("Transitions don't support nested sequences yet."))
            }
            TimelineSource::Text { .. } => {
                return Err(invalid(
                    "Transitions only work between video, image, generated, or audio clips.",
                ))
            }
        }
    }
    if is_visual_transition_kind(&left.kind) != is_visual_transition_kind(&right.kind) {
        return Err(invalid(
            "Transitions need two visual clips or two audio clips, not a mix.",
        ));
    }
    if !items_are_adjacent(left, right, frame_seconds) {
        return Err(invalid(format!(
            "{} must end where {} starts to add a transition.",
            label(left),
            label(right)
        )));
    }
    Ok(())
}

fn validate_transition_items(
    project: &VideoProject,
    track: &TimelineTrack,
    transition: &TimelineTransition,
    left: &TimelineItem,
    right: &TimelineItem,
) -> Result<(), ProjectActionError> {
    let frame = frame_seconds(project);
    validate_transition_pair(left, right, frame)?;
    if track.transitions.iter().any(|other| {
        other.id != transition.id
            && (other.left_item_id == left.id || other.right_item_id == right.id)
    }) {
        return Err(invalid(format!(
            "{} and {} already have a transition.",
            label(left),
            label(right)
        )));
    }

    let duration = transition.duration_seconds;
    if !duration.is_finite() || duration < frame - SECONDS_EPSILON {
        return Err(invalid(format!(
            "Transition duration must be at least one frame ({}s).",
            format_seconds(frame)
        )));
    }
    if duration > MAX_TRANSITION_SECONDS + SECONDS_EPSILON {
        return Err(invalid(format!(
            "Transition duration cannot be longer than {}s.",
            format_seconds(MAX_TRANSITION_SECONDS)
        )));
    }
    let bounds = transition_bounds(project, left, right);
    if duration > bounds.max_seconds + SECONDS_EPSILON {
        let requested = format_seconds(duration);
        let max = format_max_seconds(bounds.max_seconds);
        return Err(invalid(match bounds.limit {
            TransitionLimit::LeftHandle => format!(
                "Not enough unused media after {} for a {requested}s transition. Maximum is {max}s.",
                label(left)
            ),
            TransitionLimit::RightHandle => format!(
                "Not enough unused media before {} for a {requested}s transition. Maximum is {max}s.",
                label(right)
            ),
            TransitionLimit::LeftDuration | TransitionLimit::Cap => format!(
                "{} is too short for a {requested}s transition. Maximum is {max}s.",
                label(left)
            ),
            TransitionLimit::RightDuration => format!(
                "{} is too short for a {requested}s transition. Maximum is {max}s.",
                label(right)
            ),
        }));
    }
    Ok(())
}

fn editable_track_index(
    project: &VideoProject,
    track_id: &str,
) -> Result<usize, ProjectActionError> {
    let index = project
        .timeline
        .tracks
        .iter()
        .position(|track| track.id == track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(track_id.to_string()))?;
    if project.timeline.tracks[index].locked {
        return Err(ProjectActionError::TrackLocked(track_id.to_string()));
    }
    Ok(index)
}

fn transition_index(
    track: &TimelineTrack,
    transition_id: &str,
) -> Result<usize, ProjectActionError> {
    track
        .transitions
        .iter()
        .position(|transition| transition.id == transition_id)
        .ok_or_else(|| ProjectActionError::TransitionNotFound(transition_id.to_string()))
}

pub(crate) fn add_transition(
    project: &mut VideoProject,
    track_id: &str,
    transition: TimelineTransition,
) -> Result<(), ProjectActionError> {
    let track_index = editable_track_index(project, track_id)?;
    if transition.id.trim().is_empty() {
        return Err(invalid("Transition id cannot be empty."));
    }
    if project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.transitions.iter())
        .any(|existing| existing.id == transition.id)
    {
        return Err(invalid(format!(
            "A transition with id {} already exists.",
            transition.id
        )));
    }
    validate_transition(project, &project.timeline.tracks[track_index], &transition)?;
    project.timeline.tracks[track_index]
        .transitions
        .push(transition);
    Ok(())
}

pub(crate) fn update_transition(
    project: &mut VideoProject,
    track_id: &str,
    transition_id: &str,
    kind: Option<TransitionKind>,
    duration_seconds: Option<f64>,
) -> Result<(), ProjectActionError> {
    let track_index = editable_track_index(project, track_id)?;
    let track = &project.timeline.tracks[track_index];
    let index = transition_index(track, transition_id)?;
    let mut updated = track.transitions[index].clone();
    if let Some(kind) = kind {
        updated.kind = kind;
    }
    if let Some(duration_seconds) = duration_seconds {
        updated.duration_seconds = duration_seconds;
    }
    validate_transition(project, track, &updated)?;
    project.timeline.tracks[track_index].transitions[index] = updated;
    Ok(())
}

pub(crate) fn remove_transition(
    project: &mut VideoProject,
    track_id: &str,
    transition_id: &str,
) -> Result<(), ProjectActionError> {
    let track_index = editable_track_index(project, track_id)?;
    let index = transition_index(&project.timeline.tracks[track_index], transition_id)?;
    project.timeline.tracks[track_index]
        .transitions
        .remove(index);
    Ok(())
}
