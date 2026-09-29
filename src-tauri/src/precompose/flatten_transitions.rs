//! Clip transitions in canonical (flattened) frame sampling.
//!
//! Uses the render plan's transition rechecks
//! ([`plan_clip_transitions`]) and the shared window math in
//! [`crate::frame_compositor::transition`], so a flattened composite draws a
//! transition exactly where GES and the editor preview do:
//!
//! - both clips extend into their handles: the outgoing clip stays active `d/2`
//!   past its canonical end and the incoming clip `d/2` before its start, each
//!   sampling source `sourceIn + (t - start) * speed` (so `sourceOut + (t -
//!   end) * speed` past the end);
//! - frame programs sample canonical clip time clamped to the clip, so
//!   keyframes hold their edge values and fades read 0 in the handles;
//! - on a track the outgoing clip draws beneath the incoming clip, and a dip's
//!   solid draws beneath both.
//!
//! A flattened group that touches a transition window is widened to the whole
//! window, so the transition is baked into the intermediate and never split
//! between prepared and native rendering. Only pairs of media-backed video
//! clips can be flattened; other transitions keep rendering natively.

use crate::frame_compositor::{
    transition_solid_rgba8, TransitionFrameState, TransitionRole, TransitionWindow,
};
use crate::project::model::{MediaKind, TimelineItem, TimelineSource, TrackKind, VideoProject};
use crate::render_pipeline::transition_plan::{plan_clip_transitions, PlannedClipTransition};
use serde::Serialize;
use std::collections::BTreeMap;

const GROUP_EPSILON_SECONDS: f64 = 1e-9;

/// The transition windows that extend one clip.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemTransitions {
    /// The transition this clip is the incoming side of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) head: Option<TransitionWindow>,
    /// The transition this clip is the outgoing side of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tail: Option<TransitionWindow>,
}

impl ItemTransitions {
    pub(crate) fn is_empty(&self) -> bool {
        self.head.is_none() && self.tail.is_none()
    }

    pub(crate) fn head_seconds(&self) -> f64 {
        self.head
            .map_or(0.0, |window| window.duration_seconds / 2.0)
    }

    pub(crate) fn tail_seconds(&self) -> f64 {
        self.tail
            .map_or(0.0, |window| window.duration_seconds / 2.0)
    }

    /// The span `item` draws: its canonical span extended by its handles.
    pub(crate) fn active_span(&self, item: &TimelineItem) -> (f64, f64) {
        (
            item.start_seconds - self.head_seconds(),
            item.start_seconds + item.duration_seconds + self.tail_seconds(),
        )
    }

    pub(crate) fn is_active(&self, item: &TimelineItem, seconds: f64) -> bool {
        let (start, end) = self.active_span(item);
        seconds >= start && seconds < end
    }

    /// The transition the clip takes part in at `seconds`.
    pub(crate) fn state_at(&self, seconds: f64) -> Option<TransitionFrameState> {
        self.head
            .and_then(|window| TransitionFrameState::at(&window, TransitionRole::Incoming, seconds))
            .or_else(|| {
                self.tail.and_then(|window| {
                    TransitionFrameState::at(&window, TransitionRole::Outgoing, seconds)
                })
            })
    }
}

/// A dip's opaque solid, drawn in its track's slot beneath the track's clips.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransitionSolid {
    pub(crate) track_index: usize,
    pub(crate) window: TransitionWindow,
    pub(crate) rgba: [u8; 4],
}

/// The visual transitions canonical sampling can draw for a project.
#[derive(Debug, Default)]
pub(crate) struct FlattenTransitions {
    planned: Vec<PlannedClipTransition>,
    by_item: BTreeMap<(usize, String), ItemTransitions>,
}

impl FlattenTransitions {
    pub(crate) fn plan(project: &VideoProject) -> Self {
        let mut transitions = Self::default();
        for planned in plan_clip_transitions(project) {
            if planned.audio || !pair_is_flattenable(project, &planned) {
                continue;
            }
            let window = window_of(&planned);
            transitions
                .by_item
                .entry((planned.track_index, planned.left_item_id.clone()))
                .or_default()
                .tail = Some(window);
            transitions
                .by_item
                .entry((planned.track_index, planned.right_item_id.clone()))
                .or_default()
                .head = Some(window);
            transitions.planned.push(planned);
        }
        transitions
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.planned.is_empty()
    }

    pub(crate) fn for_item(&self, track_index: usize, item_id: &str) -> ItemTransitions {
        self.by_item
            .get(&(track_index, item_id.to_string()))
            .copied()
            .unwrap_or_default()
    }

    /// Windows on tracks at or below `top_track_index`.
    fn windows_up_to(&self, top_track_index: usize) -> impl Iterator<Item = TransitionWindow> + '_ {
        self.planned
            .iter()
            .filter(move |planned| planned.track_index <= top_track_index)
            .map(window_of)
    }

    /// Dip solids on tracks at or below `top_track_index` overlapping `[start, end)`.
    pub(crate) fn solids(
        &self,
        top_track_index: usize,
        start_seconds: f64,
        end_seconds: f64,
    ) -> Vec<TransitionSolid> {
        self.planned
            .iter()
            .filter(|planned| planned.track_index <= top_track_index)
            .filter_map(|planned| {
                let window = window_of(planned);
                let rgba = transition_solid_rgba8(planned.kind)?;
                let overlaps =
                    window.start_seconds < end_seconds && window.end_seconds() > start_seconds;
                overlaps.then_some(TransitionSolid {
                    track_index: planned.track_index,
                    window,
                    rgba,
                })
            })
            .collect()
    }
}

fn window_of(planned: &PlannedClipTransition) -> TransitionWindow {
    TransitionWindow {
        kind: planned.kind,
        start_seconds: planned.start_seconds,
        duration_seconds: planned.duration_seconds,
    }
}

fn pair_is_flattenable(project: &VideoProject, planned: &PlannedClipTransition) -> bool {
    let Some(track) = project.timeline.tracks.get(planned.track_index) else {
        return false;
    };
    if track.kind != TrackKind::Video {
        return false;
    }
    [&planned.left_item_id, &planned.right_item_id]
        .into_iter()
        .all(|id| {
            track.items.iter().any(|item| {
                item.id == *id
                    && matches!(&item.source, TimelineSource::Media { media_id }
                        if project.media.iter().any(|media| media.id == *media_id && media.kind == MediaKind::Video))
            })
        })
}

/// A flattened interval and the track it is composited up to.
pub(crate) trait GroupSpan {
    fn span(&self) -> (f64, f64);
    fn top_track_index(&self) -> usize;
    fn set_span(&mut self, start_seconds: f64, end_seconds: f64);
    /// Absorbs an overlapping group, keeping the higher top track.
    fn absorb(&mut self, other: Self);
}

/// Widens every group to the whole of each transition window it touches on
/// its tracks, merging groups that come to overlap, until stable.
pub(crate) fn widen_groups_over_transitions<G: GroupSpan>(
    mut groups: Vec<G>,
    transitions: &FlattenTransitions,
) -> Vec<G> {
    if transitions.is_empty() {
        return groups;
    }
    loop {
        let mut changed = false;
        for group in &mut groups {
            for window in transitions.windows_up_to(group.top_track_index()) {
                let (start, end) = group.span();
                if window.start_seconds < end - GROUP_EPSILON_SECONDS
                    && window.end_seconds() > start + GROUP_EPSILON_SECONDS
                    && (window.start_seconds < start || window.end_seconds() > end)
                {
                    group.set_span(
                        start.min(window.start_seconds),
                        end.max(window.end_seconds()),
                    );
                    changed = true;
                }
            }
        }
        groups.sort_by(|left, right| left.span().0.total_cmp(&right.span().0));
        let mut merged: Vec<G> = Vec::with_capacity(groups.len());
        for group in groups {
            match merged.last_mut() {
                Some(previous) if group.span().0 < previous.span().1 - GROUP_EPSILON_SECONDS => {
                    previous.absorb(group);
                    changed = true;
                }
                _ => merged.push(group),
            }
        }
        groups = merged;
        if !changed {
            return groups;
        }
    }
}

/// Retargets a track's transitions after flattening split its items.
/// `segments` maps an original item id to the ids of the segments that kept
/// its canonical start and end (`None` when that edge was flattened). A
/// transition survives only when its outgoing clip kept its end and its
/// incoming clip kept its start, so baked transitions are dropped.
pub(crate) fn retarget_split_transitions(
    track: &mut crate::project::model::TimelineTrack,
    segments: &BTreeMap<String, (Option<String>, Option<String>)>,
) {
    if track.transitions.is_empty() {
        return;
    }
    track.transitions.retain_mut(|transition| {
        let left = match segments.get(&transition.left_item_id) {
            Some((_, end)) => end.clone(),
            None => Some(transition.left_item_id.clone()),
        };
        let right = match segments.get(&transition.right_item_id) {
            Some((start, _)) => start.clone(),
            None => Some(transition.right_item_id.clone()),
        };
        match (left, right) {
            (Some(left), Some(right)) => {
                transition.left_item_id = left;
                transition.right_item_id = right;
                true
            }
            _ => false,
        }
    });
}
