//! Before/after impact of a prepared conversation proposal.
//!
//! Impact diffs the active timeline by canonical item ID. An item whose
//! footage keeps the same timeline-to-source mapping only reports the edges
//! that moved, so a trim or ripple delete highlights the cut rather than the
//! whole clip. The mapping honors clip speed and direction: a source span
//! covers `duration × speed`, and a reversed clip reads it from `sourceOut`
//! backwards. An added, changed, or removed transition reports the
//! two clips it joins and its window centered on their cut.

use super::CodexConversationRange;
use crate::project::action::{item_source_range, item_speed};
use crate::project::model::{Timeline, TimelineItem, TimelineTransition, VideoProject};
use crate::project::reverse::is_reversed;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const RANGE_TOLERANCE_SECONDS: f64 = 1e-6;
const FALLBACK_PREVIEW_FPS: f64 = 30.0;
const SOURCE_RANGE_KEYS: [&str; 2] = ["sourceIn", "sourceOut"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalImpact {
    pub summary: String,
    pub before_duration_seconds: f64,
    pub after_duration_seconds: f64,
    pub affected_item_ids: Vec<String>,
    pub affected_ranges: Vec<CodexConversationRange>,
    pub preview_timestamp: f64,
}

struct PlacedItem<'a> {
    track_id: &'a str,
    track_enabled: bool,
    item: &'a TimelineItem,
}

type Span = (f64, f64);

/// Computes the impact of turning `before` into `after`.
pub fn codex_proposal_impact(before: &VideoProject, after: &VideoProject) -> CodexProposalImpact {
    let before_items = placed_items(&before.timeline);
    let after_items = placed_items(&after.timeline);
    let after_by_id = after_items
        .iter()
        .map(|placed| (placed.item.id.as_str(), placed))
        .collect::<BTreeMap<_, _>>();
    let before_ids = before_items
        .iter()
        .map(|placed| placed.item.id.as_str())
        .collect::<BTreeSet<_>>();

    let mut affected_item_ids = Vec::new();
    let mut spans = Vec::new();
    let (mut added, mut changed, mut removed) = (0usize, 0usize, 0usize);
    for placed in &before_items {
        let item_spans = match after_by_id.get(placed.item.id.as_str()) {
            None => {
                removed += 1;
                vec![item_span(placed.item)]
            }
            Some(next) => {
                let item_spans = changed_spans(placed, next);
                if !item_spans.is_empty() {
                    changed += 1;
                }
                item_spans
            }
        };
        if !item_spans.is_empty() {
            affected_item_ids.push(placed.item.id.clone());
            spans.extend(item_spans);
        }
    }
    for placed in &after_items {
        if !before_ids.contains(placed.item.id.as_str()) {
            added += 1;
            affected_item_ids.push(placed.item.id.clone());
            spans.push(item_span(placed.item));
        }
    }

    // Transitions change how the clips around a cut render without moving them.
    for (item_ids, window) in changed_transition_windows(&before.timeline, &after.timeline) {
        for item_id in item_ids {
            if !affected_item_ids.iter().any(|affected| affected == item_id) {
                changed += 1;
                affected_item_ids.push(item_id.to_string());
            }
        }
        spans.extend(window);
    }

    let before_duration_seconds = item_extent_seconds(&before.timeline);
    let after_duration_seconds = item_extent_seconds(&after.timeline);
    let affected_ranges = normalized_ranges(spans);
    let preview_timestamp = preview_timestamp(
        &affected_ranges,
        after_duration_seconds,
        after.render_settings.fps,
    );
    CodexProposalImpact {
        summary: impact_summary(
            (added, changed, removed),
            before_duration_seconds,
            after_duration_seconds,
        ),
        before_duration_seconds,
        after_duration_seconds,
        affected_item_ids,
        affected_ranges,
        preview_timestamp,
    }
}

fn placed_items(timeline: &Timeline) -> Vec<PlacedItem<'_>> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| {
            track.items.iter().map(|item| PlacedItem {
                track_id: &track.id,
                track_enabled: track.enabled,
                item,
            })
        })
        .collect()
}

type PlacedTransition<'a> = (&'a str, &'a TimelineTransition, Option<Span>);

/// Transitions added, removed, or changed between the timelines, with the
/// clips they join and their window centered on the cut.
fn changed_transition_windows<'a>(
    before: &'a Timeline,
    after: &'a Timeline,
) -> Vec<([&'a str; 2], Option<Span>)> {
    let (before, after) = (placed_transitions(before), placed_transitions(after));
    let only_in = |side: &[PlacedTransition<'a>], other: &[PlacedTransition<'a>]| {
        side.iter()
            .filter(|placed| !other.contains(placed))
            .map(|(_, transition, window)| {
                (
                    [
                        transition.left_item_id.as_str(),
                        transition.right_item_id.as_str(),
                    ],
                    *window,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut changed = only_in(&before, &after);
    changed.extend(only_in(&after, &before));
    changed
}

fn placed_transitions(timeline: &Timeline) -> Vec<PlacedTransition<'_>> {
    timeline
        .tracks
        .iter()
        .flat_map(|track| {
            track.transitions.iter().map(move |transition| {
                let window = track
                    .items
                    .iter()
                    .find(|item| item.id == transition.right_item_id)
                    .map(|right| {
                        let half = transition.duration_seconds / 2.0;
                        (
                            (right.start_seconds - half).max(0.0),
                            right.start_seconds + half,
                        )
                    });
                (track.id.as_str(), transition, window)
            })
        })
        .collect()
}

fn item_span(item: &TimelineItem) -> Span {
    (
        item.start_seconds,
        item.start_seconds + item.duration_seconds,
    )
}

/// Timeline spans where an item present before and after renders differently.
fn changed_spans(before: &PlacedItem<'_>, after: &PlacedItem<'_>) -> Vec<Span> {
    let (old, new) = (item_span(before.item), item_span(after.item));
    if before.track_id != after.track_id || before.track_enabled != after.track_enabled {
        return vec![old, new];
    }
    if before.item == after.item {
        return Vec::new();
    }
    if !same_content(before.item, after.item) || !same_source_mapping(before.item, after.item) {
        return vec![old, new];
    }
    let overlaps = old.0.max(new.0) < old.1.min(new.1) - RANGE_TOLERANCE_SECONDS;
    if !overlaps {
        return vec![old, new];
    }
    // The footage stays in place where the spans overlap; only moved edges change.
    let mut spans = Vec::new();
    if !nearly_equal(old.0, new.0) {
        spans.push((old.0.min(new.0), old.0.max(new.0)));
    }
    if !nearly_equal(old.1, new.1) {
        spans.push((old.1.min(new.1), old.1.max(new.1)));
    }
    spans
}

/// Everything except timing and the source range must match.
fn same_content(before: &TimelineItem, after: &TimelineItem) -> bool {
    fn without_source_range(item: &TimelineItem) -> impl Iterator<Item = (&String, &Value)> {
        item.properties
            .iter()
            .filter(|(key, _)| !SOURCE_RANGE_KEYS.contains(&key.as_str()))
    }
    before.kind == after.kind
        && before.source == after.source
        && before.label == after.label
        && without_source_range(before).eq(without_source_range(after))
}

/// True when every timeline instant both spans share shows the same source
/// instant: `sourceIn - start × speed` is unchanged, or `sourceOut + start ×
/// speed` for a reversed clip. Items without a media source are anchored at
/// their start.
fn same_source_mapping(before: &TimelineItem, after: &TimelineItem) -> bool {
    match (item_source_range(before), item_source_range(after)) {
        (Some(before_range), Some(after_range)) => {
            let (Ok(before_speed), Ok(after_speed)) = (item_speed(before), item_speed(after))
            else {
                return false;
            };
            is_reversed(before) == is_reversed(after)
                && nearly_equal(before_speed, after_speed)
                && nearly_equal(
                    source_anchor(before, before_range, before_speed),
                    source_anchor(after, after_range, after_speed),
                )
        }
        (None, None) => nearly_equal(before.start_seconds, after.start_seconds),
        _ => false,
    }
}

/// The source second the clip would read at timeline second zero.
fn source_anchor(item: &TimelineItem, (source_in, source_out): (f64, f64), speed: f64) -> f64 {
    if is_reversed(item) {
        source_out + item.start_seconds * speed
    } else {
        source_in - item.start_seconds * speed
    }
}

fn nearly_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= RANGE_TOLERANCE_SECONDS
}

fn item_extent_seconds(timeline: &Timeline) -> f64 {
    timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .map(|item| item.start_seconds + item.duration_seconds)
        .fold(0.0, f64::max)
}

fn normalized_ranges(mut spans: Vec<Span>) -> Vec<CodexConversationRange> {
    spans.retain(|(start, end)| start.is_finite() && end.is_finite() && end - start > 0.0);
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut merged: Vec<Span> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 + RANGE_TOLERANCE_SECONDS => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
        .into_iter()
        .map(|(start_seconds, end_seconds)| CodexConversationRange {
            start_seconds,
            end_seconds,
        })
        .collect()
}

/// The first affected instant, kept at least one frame before the end of the
/// resulting timeline so a preview frame exists.
fn preview_timestamp(ranges: &[CodexConversationRange], after_duration: f64, fps: f64) -> f64 {
    let first = ranges.first().map_or(0.0, |range| range.start_seconds);
    let fps = if fps.is_finite() && fps > 0.0 {
        fps
    } else {
        FALLBACK_PREVIEW_FPS
    };
    let latest = (after_duration - 1.0 / fps).max(0.0);
    first.clamp(0.0, latest)
}

fn impact_summary(counts: (usize, usize, usize), before: f64, after: f64) -> String {
    let (added, changed, removed) = counts;
    let items = |count: usize| {
        if count == 1 {
            "1 item".to_string()
        } else {
            format!("{count} items")
        }
    };
    let parts = [("changes", changed), ("adds", added), ("removes", removed)]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(verb, count)| format!("{verb} {}", items(count)))
        .collect::<Vec<_>>();
    let edits = match parts.split_first() {
        None => "No timeline items change".to_string(),
        Some((first, rest)) => {
            let mut sentence = first[..1].to_uppercase() + &first[1..];
            for part in rest {
                sentence.push_str(", ");
                sentence.push_str(part);
            }
            sentence
        }
    };
    let duration = if nearly_equal(before, after) {
        format!("the timeline stays {before:.1}s")
    } else {
        format!("the timeline goes from {before:.1}s to {after:.1}s")
    };
    format!("{edits}; {duration}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reversed_clip(
        start: f64,
        duration: f64,
        source_in: f64,
        source_out: f64,
    ) -> PlacedItem<'static> {
        let item: TimelineItem = serde_json::from_value(serde_json::json!({
            "id": "clip",
            "kind": "video_clip",
            "startSeconds": start,
            "durationSeconds": duration,
            "source": { "type": "media", "mediaId": "media" },
            "label": "Clip",
            "properties": { "sourceIn": source_in, "sourceOut": source_out, "reverse": true },
        }))
        .unwrap();
        PlacedItem {
            track_id: "track",
            track_enabled: true,
            item: Box::leak(Box::new(item)),
        }
    }

    #[test]
    fn trimming_a_reversed_clip_reports_only_the_moved_edge() {
        // A reversed clip reads `sourceOut - t`: trimming its left edge lowers `sourceOut`,
        // and trimming its right edge raises `sourceIn`; the footage that stays keeps its place.
        let before = reversed_clip(0.0, 4.0, 0.0, 4.0);
        assert_eq!(
            changed_spans(&before, &reversed_clip(1.0, 3.0, 0.0, 3.0)),
            vec![(0.0, 1.0)]
        );
        assert_eq!(
            changed_spans(&before, &reversed_clip(0.0, 3.0, 1.0, 4.0)),
            vec![(3.0, 4.0)]
        );
    }

    #[test]
    fn summary_counts_items_and_duration() {
        assert_eq!(
            impact_summary((2, 2, 0), 10.0, 8.0),
            "Changes 2 items, adds 2 items; the timeline goes from 10.0s to 8.0s."
        );
        assert_eq!(
            impact_summary((0, 1, 0), 10.0, 10.0),
            "Changes 1 item; the timeline stays 10.0s."
        );
    }

    #[test]
    fn ranges_merge_touching_spans_and_drop_empty_ones() {
        let ranges = normalized_ranges(vec![(6.0, 10.0), (4.0, 6.0), (2.0, 2.0), (12.0, 13.0)]);
        let spans = ranges
            .iter()
            .map(|range| (range.start_seconds, range.end_seconds))
            .collect::<Vec<_>>();
        assert_eq!(spans, vec![(4.0, 10.0), (12.0, 13.0)]);
    }

    #[test]
    fn preview_timestamp_stays_inside_an_empty_or_short_timeline() {
        assert_eq!(preview_timestamp(&[], 0.0, 30.0), 0.0);
        let ranges = normalized_ranges(vec![(9.0, 10.0)]);
        assert!(preview_timestamp(&ranges, 6.0, 25.0) < 6.0);
        assert_eq!(preview_timestamp(&ranges, 6.0, f64::NAN), 6.0 - 1.0 / 30.0);
    }
}
