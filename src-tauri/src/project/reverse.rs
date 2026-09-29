//! Reversed clip playback: the `updateClipReverse` action and the source
//! mapping every trim, split, handle and render-range site shares.
//!
//! A clip plays its source window `[sourceIn, sourceOut]` at `speed`. Forward
//! clips read `sourceIn + t * speed` at clip-local timeline second `t`;
//! reversed clips (`properties.reverse == true`) read `sourceOut - t * speed`.
//! Everything else mirrors from that:
//!
//! - handles, in timeline seconds: forward head `sourceIn / speed` and tail
//!   `(mediaDuration - sourceOut) / speed`; reversed head
//!   `(mediaDuration - sourceOut) / speed` and tail `sourceIn / speed`;
//! - a sub-range `[start, end]` of the clip: forward
//!   `[sourceIn + start * speed, sourceIn + end * speed]`; reversed
//!   `[sourceOut - end * speed, sourceOut - start * speed]`, so the left part
//!   of a split keeps the original `sourceOut` and the right part `sourceIn`;
//! - moving an edge: the left edge moves `sourceIn` forward and `sourceOut`
//!   reversed; the right edge moves `sourceOut` forward and `sourceIn` reversed.
//!
//! Renderers play reversed clips from prepared, reversed intermediates. The
//! prepared clip is reverse-free, so a clip still marked `reverse` at render
//! time was never prepared.

use super::action::{find_item, item_source_range, ProjectActionError, ProjectActionTrimEdge};
use super::model::*;

/// The clip property that marks reversed playback.
pub const REVERSE_PROPERTY: &str = "reverse";

/// Whether `item` plays its source window backwards.
pub fn is_reversed(item: &TimelineItem) -> bool {
    item.properties
        .get(REVERSE_PROPERTY)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// A clip's source window, speed and direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceWindow {
    pub source_in: f64,
    pub source_out: f64,
    pub speed: f64,
    pub reverse: bool,
}

impl SourceWindow {
    /// The source second read at clip-local timeline second `local_seconds`.
    /// Negative times and times past the clip end continue into the handles.
    pub fn source_seconds_at(&self, local_seconds: f64) -> f64 {
        if self.reverse {
            self.source_out - local_seconds * self.speed
        } else {
            self.source_in + local_seconds * self.speed
        }
    }

    /// The clip-local second at which `source_seconds` plays: the inverse of
    /// [`Self::source_seconds_at`].
    pub fn local_seconds_for_source(&self, source_seconds: f64) -> f64 {
        if self.reverse {
            (self.source_out - source_seconds) / self.speed
        } else {
            (source_seconds - self.source_in) / self.speed
        }
    }

    /// The ordered timeline range over which source `[source_start, source_end]`
    /// plays in a clip starting at `clip_start`: forward
    /// `[start + (a - sourceIn) / speed, start + (b - sourceIn) / speed]`, reversed
    /// `[start + (sourceOut - b) / speed, start + (sourceOut - a) / speed]`.
    pub fn timeline_range_for_source(
        &self,
        clip_start: f64,
        source_start: f64,
        source_end: f64,
    ) -> (f64, f64) {
        let start = clip_start + self.local_seconds_for_source(source_start);
        let end = clip_start + self.local_seconds_for_source(source_end);
        (start.min(end), start.max(end))
    }

    /// The ordered source range read over clip-local `[start, end]`, as a range
    /// render maps the part of a clip it overlaps.
    pub fn source_range_for(&self, start_local: f64, end_local: f64) -> (f64, f64) {
        let start = self.source_seconds_at(start_local);
        let end = self.source_seconds_at(end_local);
        (start.min(end), start.max(end))
    }

    /// Whether a range from [`Self::source_range_for`] reads past the edge of
    /// the window it reads towards: `sourceOut` forward, `sourceIn` reversed.
    pub fn range_exceeds_window(&self, (source_in, source_out): (f64, f64)) -> bool {
        if self.reverse {
            source_in < self.source_in
        } else {
            source_out > self.source_out
        }
    }

    /// Unused media before the clip's start, in timeline seconds.
    pub fn head_handle_seconds(&self, media_duration: f64) -> f64 {
        let source_seconds = if self.reverse {
            media_duration - self.source_out
        } else {
            self.source_in
        };
        (source_seconds / self.speed).max(0.0)
    }

    /// Unused media after the clip's end, in timeline seconds.
    pub fn tail_handle_seconds(&self, media_duration: f64) -> f64 {
        let source_seconds = if self.reverse {
            self.source_in
        } else {
            media_duration - self.source_out
        };
        (source_seconds / self.speed).max(0.0)
    }

    /// The source window of the clip-local part `[start, end]` of a clip that
    /// lasted `original_duration`. The edge a part shares with the original clip
    /// keeps the stored value.
    pub fn subrange(&self, start_local: f64, end_local: f64, original_duration: f64) -> (f64, f64) {
        let keeps_end = end_local >= original_duration;
        if self.reverse {
            let source_in = if keeps_end {
                self.source_in
            } else {
                self.source_out - end_local * self.speed
            };
            (source_in, self.source_out - start_local * self.speed)
        } else {
            let source_out = if keeps_end {
                self.source_out
            } else {
                self.source_in + end_local * self.speed
            };
            (self.source_in + start_local * self.speed, source_out)
        }
    }

    /// The source window after `edge` moves so the clip gains
    /// `duration_delta` timeline seconds (negative shrinks it).
    pub fn trimmed(&self, edge: ProjectActionTrimEdge, duration_delta: f64) -> (f64, f64) {
        let source_delta = duration_delta * self.speed;
        match (edge, self.reverse) {
            (ProjectActionTrimEdge::Left, false) => {
                (self.source_in - source_delta, self.source_out)
            }
            (ProjectActionTrimEdge::Right, false) => {
                (self.source_in, self.source_out + source_delta)
            }
            (ProjectActionTrimEdge::Left, true) => (self.source_in, self.source_out + source_delta),
            (ProjectActionTrimEdge::Right, true) => {
                (self.source_in - source_delta, self.source_out)
            }
        }
    }
}

/// The source window of a media-backed clip played at `speed`, with the
/// timeline model's defaults (`sourceIn` 0, `sourceOut` `sourceIn + duration * speed`).
pub(crate) fn media_source_window(item: &TimelineItem, speed: f64) -> Option<SourceWindow> {
    item_source_range(item).map(|(source_in, source_out)| SourceWindow {
        source_in,
        source_out,
        speed,
        reverse: is_reversed(item),
    })
}

/// The source property a clip that keeps only its first `kept_duration`
/// timeline seconds rewrites, and its value: `sourceOut` for forward clips,
/// `sourceIn` for reversed ones. `None` when the clip has no `sourceIn`.
pub fn truncated_tail_source_property(
    item: &TimelineItem,
    speed: f64,
    original_duration: f64,
    kept_duration: f64,
) -> Option<(&'static str, f64)> {
    let number = |key: &str| item.properties.get(key).and_then(serde_json::Value::as_f64);
    let source_in = number("sourceIn")?;
    if is_reversed(item) {
        let source_out = number("sourceOut").unwrap_or(source_in + original_duration * speed);
        Some(("sourceIn", source_out - kept_duration * speed))
    } else {
        Some(("sourceOut", source_in + kept_duration * speed))
    }
}

/// Whether `item` can play reversed: a video clip of video media or an audio
/// clip of project media (the clips render preparation can reverse). Mirrors TS
/// `isReversibleItem`.
pub fn is_reversible_clip(project: &VideoProject, item: &TimelineItem) -> bool {
    let TimelineSource::Media { media_id } = &item.source else {
        return false;
    };
    match item.kind {
        TimelineItemKind::AudioClip => true,
        TimelineItemKind::VideoClip => project
            .media
            .iter()
            .any(|media| media.id == *media_id && media.kind == MediaKind::Video),
        _ => false,
    }
}

/// Sets or clears reversed playback on a clip [`is_reversible_clip`] accepts.
/// Duration and source range are unchanged.
pub(crate) fn update_clip_reverse(
    project: &mut VideoProject,
    item_id: &str,
    reverse: bool,
) -> Result<(), ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let track = &project.timeline.tracks[track_index];
    if track.locked {
        return Err(ProjectActionError::TrackLocked(track.id.clone()));
    }
    let item = &track.items[item_index];
    if let (TimelineItemKind::VideoClip, TimelineSource::Media { media_id }) =
        (&item.kind, &item.source)
    {
        if !project.media.iter().any(|media| media.id == *media_id) {
            return Err(ProjectActionError::MissingMedia(media_id.clone()));
        }
    }
    if !is_reversible_clip(project, item) {
        return Err(ProjectActionError::NotReversibleClip(item_id.to_string()));
    }
    let properties = &mut project.timeline.tracks[track_index].items[item_index].properties;
    if reverse {
        properties.insert(REVERSE_PROPERTY.to_string(), serde_json::json!(true));
    } else {
        properties.remove(REVERSE_PROPERTY);
    }
    Ok(())
}
