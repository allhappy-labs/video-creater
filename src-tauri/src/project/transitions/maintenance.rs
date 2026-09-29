//! Keeps stored transitions valid after any project action.
//!
//! `apply_project_action` runs, in order, after every action and before the
//! timeline duration is recalculated:
//!
//! 1. [`retarget_transitions_to_split_tails`], when the action did not switch
//!    the active timeline. Splits, ripple deletes, inserts and overwrites keep
//!    the original id on the left-hand piece of a cut clip and give the
//!    right-hand piece a new id. A transition on the clip's end therefore moves
//!    to that new right-hand piece, which still ends at the cut. The transition
//!    keeps its own id.
//! 2. [`maintain_transitions`] on the active timeline and every timeline in the
//!    library. It drops transitions whose clips are missing from the track
//!    (removed or moved to another track), no longer adjacent, no longer an
//!    eligible pair, already covered by an earlier transition on the same cut,
//!    or without room for one frame. Otherwise it clamps the duration into
//!    `[one frame, maximum]`.

use super::{
    frame_seconds, items_are_adjacent, transition_max_duration, validate_transition_pair,
    SECONDS_EPSILON,
};
use crate::project::model::*;
use crate::project::reverse::is_reversed;
use std::collections::BTreeSet;

/// Moves a transition's left clip to the new right-hand piece of that clip
/// when this action cut it in two.
///
/// A candidate piece is an item on the same track that did not exist in
/// `before`, has the original clip's kind, source and direction, ends at the
/// same source edge (`sourceOut`, or `sourceIn` for reversed clips), and is
/// adjacent to the transition's right clip.
pub fn retarget_transitions_to_split_tails(before: &Timeline, project: &mut VideoProject) {
    let frame = frame_seconds(project);
    let before_ids = before
        .tracks
        .iter()
        .flat_map(|track| track.items.iter().map(|item| item.id.as_str()))
        .collect::<BTreeSet<_>>();
    for track in &mut project.timeline.tracks {
        let Some(before_track) = before.tracks.iter().find(|entry| entry.id == track.id) else {
            continue;
        };
        for index in 0..track.transitions.len() {
            let transition = &track.transitions[index];
            let find = |item_id: &str| track.items.iter().find(|item| item.id == item_id);
            let Some(right) = find(&transition.right_item_id) else {
                continue;
            };
            if find(&transition.left_item_id)
                .is_some_and(|left| items_are_adjacent(left, right, frame))
            {
                continue;
            }
            let Some(original) = before_track
                .items
                .iter()
                .find(|item| item.id == transition.left_item_id)
            else {
                continue;
            };
            let tail = track.items.iter().find(|item| {
                !before_ids.contains(item.id.as_str())
                    && item.kind == original.kind
                    && item.source == original.source
                    && is_reversed(item) == is_reversed(original)
                    && same_end_source(item, original)
                    && items_are_adjacent(item, right, frame)
            });
            if let Some(tail_id) = tail.map(|item| item.id.clone()) {
                track.transitions[index].left_item_id = tail_id;
            }
        }
    }
}

/// Whether `item` ends at the source second `original` ends at.
fn same_end_source(item: &TimelineItem, original: &TimelineItem) -> bool {
    let key = if is_reversed(original) {
        "sourceIn"
    } else {
        "sourceOut"
    };
    let source_out =
        |item: &TimelineItem| item.properties.get(key).and_then(serde_json::Value::as_f64);
    match (source_out(item), source_out(original)) {
        (Some(item_out), Some(original_out)) => (item_out - original_out).abs() <= SECONDS_EPSILON,
        (None, None) => true,
        _ => false,
    }
}

/// Drops or clamps transitions that edits made invalid. See the module docs.
pub fn maintain_transitions(project: &mut VideoProject) {
    for track_index in 0..project.timeline.tracks.len() {
        let track = &project.timeline.tracks[track_index];
        if track.transitions.is_empty() {
            continue;
        }
        let maintained = maintained_track_transitions(project, track);
        project.timeline.tracks[track_index].transitions = maintained;
    }
    for timeline_index in 0..project.timelines.len() {
        for track_index in 0..project.timelines[timeline_index].timeline.tracks.len() {
            let track = &project.timelines[timeline_index].timeline.tracks[track_index];
            if track.transitions.is_empty() {
                continue;
            }
            let maintained = maintained_track_transitions(project, track);
            project.timelines[timeline_index].timeline.tracks[track_index].transitions = maintained;
        }
    }
}

fn maintained_track_transitions(
    project: &VideoProject,
    track: &TimelineTrack,
) -> Vec<TimelineTransition> {
    let frame = frame_seconds(project);
    let mut used_left = BTreeSet::new();
    let mut used_right = BTreeSet::new();
    let mut maintained = Vec::with_capacity(track.transitions.len());
    for transition in &track.transitions {
        let find = |item_id: &str| track.items.iter().find(|item| item.id == item_id);
        let (Some(left), Some(right)) = (
            find(&transition.left_item_id),
            find(&transition.right_item_id),
        ) else {
            continue;
        };
        if validate_transition_pair(left, right, frame).is_err()
            || used_left.contains(&left.id)
            || used_right.contains(&right.id)
            || !transition.duration_seconds.is_finite()
        {
            continue;
        }
        let max = transition_max_duration(project, left, right);
        if max < frame - SECONDS_EPSILON {
            continue;
        }
        let mut transition = transition.clone();
        if transition.duration_seconds > max + SECONDS_EPSILON {
            transition.duration_seconds = max;
        } else if transition.duration_seconds < frame - SECONDS_EPSILON {
            transition.duration_seconds = frame;
        }
        used_left.insert(left.id.clone());
        used_right.insert(right.id.clone());
        maintained.push(transition);
    }
    maintained
}
