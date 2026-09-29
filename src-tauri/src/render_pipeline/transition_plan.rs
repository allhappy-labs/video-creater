//! Render-only expansion of clip transitions.
//!
//! The canonical timeline never overlaps the two clips of a transition. For
//! rendering, a transition of `d` timeline seconds centered on the cut (the
//! right clip's start) extends both clips into their unused source media:
//!
//! - the left clip gains `d / 2` at its tail: `duration + d/2` and
//!   `sourceOut + d/2 * speed`;
//! - the right clip gains `d / 2` at its head: `start - d/2`,
//!   `sourceIn - d/2 * speed` and `duration + d/2`.
//!
//! Reversed clips mirror the source edges: the left clip's tail reads
//! `sourceIn - d/2 * speed` and the right clip's head `sourceOut + d/2 * speed`.
//!
//! Source seconds scale with clip speed, matching the handle math in
//! [`crate::project::transitions`]. The window is `[cut - d/2, cut + d/2]`,
//! the same span the editor preview uses.
//!
//! Expansion runs on the nested-expanded, precomposed project the render plan
//! is built from, so it re-checks every transition against that project:
//! transitions whose clips are missing, no longer adjacent, or not renderable
//! as a pair are skipped, and durations are clamped to the handles that remain.
//! LUT and Lottie intermediates include their clips' transition handles, and
//! flattened composites bake the transitions they cover (see
//! `precompose/flatten_transitions.rs`). The canonical project is never
//! mutated.

use crate::edit::render_plan::RenderTransition;
use crate::project::action::ProjectActionTrimEdge;
use crate::project::model::{
    TimelineItem, TimelineItemKind, TimelineSource, TimelineTrack, TimelineTransition, TrackKind,
    TransitionKind, VideoProject,
};
use crate::project::reverse::{is_reversed, SourceWindow};
use crate::project::transitions::{frame_seconds, items_are_adjacent, transition_max_duration};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Render clip property: timeline seconds of transition handle before the
/// clip's canonical start that the render clip actually includes.
pub const TRANSITION_HEAD_SECONDS_PROPERTY: &str = "transitionHeadSeconds";
/// Render clip property: timeline seconds of transition handle after the
/// clip's canonical end that the render clip actually includes.
pub const TRANSITION_TAIL_SECONDS_PROPERTY: &str = "transitionTailSeconds";

/// Keeps extended source ranges strictly inside the media when a transition
/// uses every available handle, so float rounding cannot push `sourceIn`
/// below zero or `sourceOut` past the media duration.
const HANDLE_MARGIN_SECONDS: f64 = 1e-9;
const FLOAT_SLACK_SECONDS: f64 = 1e-12;
const SECONDS_EPSILON: f64 = 1e-6;

/// A clip on a track of the render project: `(track index, item id)`.
pub(super) type ClipKey = (usize, String);

#[derive(Debug, Clone, Copy)]
struct CanonicalSpan {
    start: f64,
    end: f64,
}

#[derive(Debug, Clone)]
struct PlannedTransition {
    audio: bool,
    left: ClipKey,
    right: ClipKey,
    kind: TransitionKind,
    start_seconds: f64,
    duration_seconds: f64,
}

/// The transitions a render plan draws, and the canonical spans of the clips
/// they extend.
#[derive(Debug, Default)]
pub(super) struct TransitionHandlePlan {
    spans: BTreeMap<ClipKey, CanonicalSpan>,
    transitions: Vec<PlannedTransition>,
}

/// A clip transition the renderers draw, after the render plan's rechecks:
/// the pair is adjacent on one enabled track, source-backed, and its duration
/// is clamped to the remaining handles. A clip end extended by one transition
/// is not extended again.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedClipTransition {
    pub track_index: usize,
    pub left_item_id: String,
    pub right_item_id: String,
    pub kind: TransitionKind,
    /// Whether the pair is two audio clips on an audio track.
    pub audio: bool,
    /// Window start: the right clip's start minus half the duration.
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

/// The transitions of `project` that renderers draw, in track then declaration
/// order. Shared by the render plan and canonical preview sampling.
pub fn plan_clip_transitions(project: &VideoProject) -> Vec<PlannedClipTransition> {
    let mut planned_transitions = Vec::new();
    let mut claimed_tails = std::collections::BTreeSet::<ClipKey>::new();
    let mut claimed_heads = std::collections::BTreeSet::<ClipKey>::new();
    let frame = frame_seconds(project);
    for (track_index, track) in project.timeline.tracks.iter().enumerate() {
        for transition in &track.transitions {
            let Some(planned) = plan_transition(project, track_index, track, transition, frame)
            else {
                continue;
            };
            if claimed_tails.contains(&planned.left) || claimed_heads.contains(&planned.right) {
                continue;
            }
            claimed_tails.insert(planned.left.clone());
            claimed_heads.insert(planned.right.clone());
            planned_transitions.push(PlannedClipTransition {
                track_index,
                left_item_id: planned.left.1,
                right_item_id: planned.right.1,
                kind: planned.kind,
                audio: planned.audio,
                start_seconds: planned.start_seconds,
                duration_seconds: planned.duration_seconds,
            });
        }
    }
    planned_transitions
}

impl TransitionHandlePlan {
    /// Extends the clips of every renderable transition in `project` into
    /// their handles. Projects without transitions are left untouched.
    pub(super) fn extend_project(project: &mut VideoProject) -> Self {
        let mut plan = Self::default();
        let mut extensions = BTreeMap::<ClipKey, (f64, f64)>::new();
        for planned in plan_clip_transitions(project) {
            let left = (planned.track_index, planned.left_item_id);
            let right = (planned.track_index, planned.right_item_id);
            let half = planned.duration_seconds / 2.0;
            extensions.entry(left.clone()).or_default().1 = half;
            extensions.entry(right.clone()).or_default().0 = half;
            plan.transitions.push(PlannedTransition {
                audio: planned.audio,
                left,
                right,
                kind: planned.kind,
                start_seconds: planned.start_seconds,
                duration_seconds: planned.duration_seconds,
            });
        }
        for ((track_index, item_id), (head, tail)) in extensions {
            let Some(item) = project.timeline.tracks[track_index]
                .items
                .iter_mut()
                .find(|item| item.id == item_id)
            else {
                continue;
            };
            plan.spans.insert(
                (track_index, item_id),
                CanonicalSpan {
                    start: item.start_seconds,
                    end: item.start_seconds + item.duration_seconds,
                },
            );
            extend_item(item, head, tail);
        }
        plan.transitions.sort_by(|left, right| {
            left.start_seconds
                .total_cmp(&right.start_seconds)
                .then_with(|| left.left.cmp(&right.left))
        });
        plan
    }

    /// Records how much transition handle a render clip spanning
    /// `[clip_start, clip_end]` (absolute timeline seconds) includes.
    pub(super) fn annotate_clip(
        &self,
        properties: &mut BTreeMap<String, Value>,
        track_index: usize,
        item_id: &str,
        clip_start: f64,
        clip_end: f64,
    ) {
        let Some(span) = self.spans.get(&(track_index, item_id.to_string())) else {
            return;
        };
        for (key, seconds) in [
            (TRANSITION_HEAD_SECONDS_PROPERTY, span.start - clip_start),
            (TRANSITION_TAIL_SECONDS_PROPERTY, clip_end - span.end),
        ] {
            if seconds > SECONDS_EPSILON {
                properties.insert(key.to_string(), json!(round_seconds(seconds)));
            }
        }
    }

    /// Resolves planned transitions to plan clip indices. `clip_keys` and
    /// `audio_clip_keys` list the clip behind each plan clip, in plan order.
    /// A transition is dropped when a range render excludes either clip.
    pub(super) fn render_transitions(
        &self,
        clip_keys: &[ClipKey],
        audio_clip_keys: &[ClipKey],
        range_start_seconds: f64,
    ) -> (Vec<RenderTransition>, Vec<RenderTransition>) {
        let mut video = Vec::new();
        let mut audio = Vec::new();
        for planned in &self.transitions {
            let keys = if planned.audio {
                audio_clip_keys
            } else {
                clip_keys
            };
            let index_of = |key: &ClipKey| keys.iter().position(|candidate| candidate == key);
            let (Some(left_clip_index), Some(right_clip_index)) =
                (index_of(&planned.left), index_of(&planned.right))
            else {
                continue;
            };
            let transition = RenderTransition {
                kind: planned.kind,
                start_seconds: round_seconds(planned.start_seconds - range_start_seconds),
                duration_seconds: round_seconds(planned.duration_seconds),
                left_clip_index,
                right_clip_index,
            };
            if planned.audio {
                audio.push(transition);
            } else {
                video.push(transition);
            }
        }
        (video, audio)
    }
}

/// Carries `track`'s transitions onto its nested-expanded copy. Item and
/// transition ids gain the expansion namespace, and durations are retimed by
/// the nested playback speed like the items. Transitions whose clips were
/// clipped away by the wrapper are dropped.
pub(super) fn carry_transitions_through_expansion(
    track: &TimelineTrack,
    expanded_items: &[TimelineItem],
    namespace: &str,
    playback_speed: f64,
) -> Vec<TimelineTransition> {
    let expanded_id = |id: &str| {
        if namespace == "root" {
            id.to_string()
        } else {
            format!("{namespace}:{id}")
        }
    };
    track
        .transitions
        .iter()
        .filter_map(|transition| {
            let left_item_id = expanded_id(&transition.left_item_id);
            let right_item_id = expanded_id(&transition.right_item_id);
            let present = |id: &str| expanded_items.iter().any(|item| item.id == id);
            (present(&left_item_id) && present(&right_item_id)).then(|| TimelineTransition {
                id: expanded_id(&transition.id),
                left_item_id,
                right_item_id,
                kind: transition.kind,
                duration_seconds: transition.duration_seconds / playback_speed,
            })
        })
        .collect()
}

fn plan_transition(
    project: &VideoProject,
    track_index: usize,
    track: &TimelineTrack,
    transition: &TimelineTransition,
    frame: f64,
) -> Option<PlannedTransition> {
    if !track.enabled {
        return None;
    }
    let find = |id: &str| track.items.iter().find(|item| item.id == id);
    let left = find(&transition.left_item_id)?;
    let right = find(&transition.right_item_id)?;
    let audio = match track.kind {
        TrackKind::Video if is_visual_pair_kind(&left.kind) && is_visual_pair_kind(&right.kind) => {
            false
        }
        TrackKind::Audio
            if left.kind == TimelineItemKind::AudioClip
                && right.kind == TimelineItemKind::AudioClip =>
        {
            true
        }
        _ => return None,
    };
    if !is_source_backed(left)
        || !is_source_backed(right)
        || !items_are_adjacent(left, right, frame)
    {
        return None;
    }
    let max_seconds = transition_max_duration(project, left, right) - HANDLE_MARGIN_SECONDS;
    let duration_seconds = transition.duration_seconds.min(max_seconds);
    if !duration_seconds.is_finite() || duration_seconds < frame - SECONDS_EPSILON {
        return None;
    }
    Some(PlannedTransition {
        audio,
        left: (track_index, left.id.clone()),
        right: (track_index, right.id.clone()),
        kind: transition.kind,
        start_seconds: right.start_seconds - duration_seconds / 2.0,
        duration_seconds,
    })
}

fn is_visual_pair_kind(kind: &TimelineItemKind) -> bool {
    matches!(
        kind,
        TimelineItemKind::VideoClip | TimelineItemKind::ImageClip | TimelineItemKind::GeneratedClip
    )
}

fn is_source_backed(item: &TimelineItem) -> bool {
    matches!(
        item.source,
        TimelineSource::Media { .. } | TimelineSource::Generated { .. }
    )
}

/// Extends `item` by `head` timeline seconds before its start and `tail`
/// after its end, consuming `seconds * speed` of source media on each side.
fn extend_item(item: &mut TimelineItem, head: f64, tail: f64) {
    let number = |key: &str| {
        item.properties
            .get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
    };
    let speed = number("speed").filter(|speed| *speed > 0.0).unwrap_or(1.0);
    let source_in = number("sourceIn").unwrap_or(0.0);
    let source_out = number("sourceOut").unwrap_or(source_in + item.duration_seconds * speed);
    item.start_seconds -= head;
    item.duration_seconds += head + tail;
    if is_reversed(item) {
        let window = SourceWindow {
            source_in,
            source_out,
            speed,
            reverse: true,
        };
        let extended_out = window.trimmed(ProjectActionTrimEdge::Left, head).1;
        let extended_in = window
            .trimmed(ProjectActionTrimEdge::Right, tail)
            .0
            .max(0.0);
        // Mirror of the forward slack below: a range render recomputing the
        // source start from `sourceOut - duration * speed` stays inside `sourceIn`.
        let spanned_in = extended_out - item.duration_seconds * speed;
        let extended_in = extended_in
            .min(spanned_in.max(extended_in - FLOAT_SLACK_SECONDS))
            .max(0.0);
        item.properties
            .insert("sourceIn".to_string(), json!(extended_in));
        item.properties
            .insert("sourceOut".to_string(), json!(extended_out));
        return;
    }
    let extended_in = (source_in - head * speed).max(0.0);
    let spanned_out = extended_in + item.duration_seconds * speed;
    let extended_out = if item.kind == TimelineItemKind::ImageClip {
        // Stills have unlimited handles; keep the range matching the duration.
        spanned_out
    } else {
        // Tolerate float noise so range renders that recompute the source end
        // from `sourceIn + duration * speed` stay inside `sourceOut`.
        let extended_out = source_out + tail * speed;
        extended_out.max(spanned_out.min(extended_out + FLOAT_SLACK_SECONDS))
    };
    item.properties
        .insert("sourceIn".to_string(), json!(extended_in));
    item.properties
        .insert("sourceOut".to_string(), json!(extended_out));
}

fn round_seconds(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
