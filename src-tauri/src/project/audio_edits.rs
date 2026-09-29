//! Audio clip edits that have their own project actions: `updateAudioClipSpeed`
//! and `detachAudio`.
//!
//! Speed is the same `speed` clip property that trim, split, transition-handle,
//! render-plan and NLE code already read for every clip kind. Changing it does
//! not resize the clip; callers pair the action with `resizeItems`, exactly
//! like `updateVisualClipSpeed`.
//!
//! Timeline video never sounds: GES adds video clips as video-only tracks and the
//! preview mutes video layers, so a video clip's sound plays from the audio clip
//! sharing its `linkGroupId`. Detaching audio creates that clip from the video's
//! own media range and moves the clip-level sound properties onto it.

use super::action::{find_item, ProjectActionError};
use super::model::*;
use serde_json::Value;

/// The supported clip speed range, matching visual clips.
pub(crate) const SPEED_RANGE: std::ops::RangeInclusive<f64> = 0.1..=8.0;

/// Sets an audio clip's playback speed; a speed of 1 removes the property.
pub(crate) fn update_audio_clip_speed(
    project: &mut VideoProject,
    item_id: &str,
    speed: f64,
) -> Result<(), ProjectActionError> {
    if !speed.is_finite() || !SPEED_RANGE.contains(&speed) {
        return Err(ProjectActionError::InvalidAudioClipSpeed(
            item_id.to_string(),
        ));
    }
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let track = &mut project.timeline.tracks[track_index];
    if track.locked {
        return Err(ProjectActionError::TrackLocked(track.id.clone()));
    }
    let item = &mut track.items[item_index];
    if item.kind != TimelineItemKind::AudioClip {
        return Err(ProjectActionError::NotAudioClip(item_id.to_string()));
    }
    if (speed - 1.0).abs() < f64::EPSILON {
        item.properties.remove("speed");
    } else {
        item.properties
            .insert("speed".to_string(), serde_json::json!(speed));
    }
    Ok(())
}

const OVERLAP_EPSILON_SECONDS: f64 = 0.000_001;
/// Clip-level sound properties that move from the video clip to its audio clip.
const MOVED_AUDIO_PROPERTIES: [&str; 1] = ["volumeDb"];
/// Source range properties the audio clip copies so it plays the same media
/// in the same direction.
const COPIED_RANGE_PROPERTIES: [&str; 4] = ["sourceIn", "sourceOut", "speed", "reverse"];

/// Creates a linked audio clip from a media-backed video clip's own sound.
///
/// The new `audio_item_id` clip spans the video clip on `target_track_id` with
/// the same source range and speed. `volumeDb`, the `volumeDb` keyframe lane
/// and `audio.*` effects move to it; visual fades stay on the video, where they
/// fade opacity. The video gains `link_group_id` (an existing group must match)
/// and `audioDetached: true`, which keeps NLE export from writing its source
/// audio a second time.
pub(crate) fn detach_audio(
    project: &mut VideoProject,
    item_id: &str,
    audio_item_id: &str,
    target_track_id: &str,
    link_group_id: &str,
) -> Result<(), ProjectActionError> {
    let (track_index, item_index) = find_item(&project.timeline, item_id)
        .ok_or_else(|| ProjectActionError::ItemNotFound(item_id.to_string()))?;
    let item = &project.timeline.tracks[track_index].items[item_index];
    if !has_detachable_audio(project, item) {
        return Err(ProjectActionError::NoDetachableAudio(item_id.to_string()));
    }
    let existing_group = item
        .properties
        .get("linkGroupId")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(group) = existing_group.as_deref() {
        let linked_audio = project.timeline.tracks.iter().any(|track| {
            track.items.iter().any(|candidate| {
                candidate.kind == TimelineItemKind::AudioClip
                    && candidate
                        .properties
                        .get("linkGroupId")
                        .and_then(Value::as_str)
                        == Some(group)
            })
        });
        if linked_audio {
            return Err(ProjectActionError::AudioAlreadyDetached(
                item_id.to_string(),
            ));
        }
    }
    let target_index = project
        .timeline
        .tracks
        .iter()
        .position(|track| track.id == target_track_id)
        .ok_or_else(|| ProjectActionError::TrackNotFound(target_track_id.to_string()))?;
    let target = &project.timeline.tracks[target_index];
    if target.kind != TrackKind::Audio {
        return Err(ProjectActionError::TrackTypeMismatch {
            item_kind: TimelineItemKind::AudioClip,
            track_kind: target.kind.clone(),
        });
    }
    for index in [track_index, target_index] {
        let track = &project.timeline.tracks[index];
        if track.locked {
            return Err(ProjectActionError::TrackLocked(track.id.clone()));
        }
    }
    if link_group_id.trim().is_empty()
        || link_group_id.chars().count() > 128
        || existing_group
            .as_deref()
            .is_some_and(|group| group != link_group_id)
    {
        return Err(ProjectActionError::InvalidEffectParam(format!(
            "link group id must be 1 to 128 characters and match the clip's existing link group: {link_group_id}"
        )));
    }
    if find_item(&project.timeline, audio_item_id).is_some() {
        return Err(ProjectActionError::DuplicateItemId(
            audio_item_id.to_string(),
        ));
    }
    let start = item.start_seconds;
    let end = item.start_seconds + item.duration_seconds;
    if let Some(blocking) = target.items.iter().find(|candidate| {
        candidate.start_seconds < end - OVERLAP_EPSILON_SECONDS
            && candidate.start_seconds + candidate.duration_seconds
                > start + OVERLAP_EPSILON_SECONDS
    }) {
        return Err(ProjectActionError::TrackItemOverlap {
            track_id: target.id.clone(),
            item_id: audio_item_id.to_string(),
            blocking_item_id: blocking.id.clone(),
        });
    }

    let audio_item = detached_audio_item(item, audio_item_id, link_group_id);
    let video = &mut project.timeline.tracks[track_index].items[item_index];
    strip_detached_audio(video, link_group_id);
    let target_items = &mut project.timeline.tracks[target_index].items;
    let position = target_items
        .iter()
        .position(|candidate| candidate.start_seconds > audio_item.start_seconds)
        .unwrap_or(target_items.len());
    target_items.insert(position, audio_item);
    Ok(())
}

fn has_detachable_audio(project: &VideoProject, item: &TimelineItem) -> bool {
    if item.kind != TimelineItemKind::VideoClip {
        return false;
    }
    let TimelineSource::Media { media_id } = &item.source else {
        return false;
    };
    project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .is_some_and(|media| matches!(media.kind, MediaKind::Video | MediaKind::Generated))
}

fn is_audio_effect(effect: &Value) -> bool {
    effect
        .get("effectType")
        .and_then(Value::as_str)
        .is_some_and(|effect_type| effect_type.starts_with("audio."))
}

fn detached_audio_item(
    video: &TimelineItem,
    audio_item_id: &str,
    link_group_id: &str,
) -> TimelineItem {
    let mut properties = std::collections::BTreeMap::new();
    for key in COPIED_RANGE_PROPERTIES
        .iter()
        .chain(MOVED_AUDIO_PROPERTIES.iter())
    {
        if let Some(value) = video.properties.get(*key) {
            properties.insert((*key).to_string(), value.clone());
        }
    }
    if let Some(lane) = video
        .properties
        .get("keyframes")
        .and_then(|keyframes| keyframes.get("volumeDb"))
    {
        properties.insert(
            "keyframes".to_string(),
            serde_json::json!({ "volumeDb": lane }),
        );
    }
    let audio_effects = video
        .properties
        .get("effects")
        .and_then(Value::as_array)
        .map(|effects| {
            effects
                .iter()
                .filter(|effect| is_audio_effect(effect))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !audio_effects.is_empty() {
        properties.insert("effects".to_string(), Value::Array(audio_effects));
    }
    properties.insert("linkGroupId".to_string(), serde_json::json!(link_group_id));
    properties.insert("sourceClipType".to_string(), serde_json::json!("audio"));
    TimelineItem {
        id: audio_item_id.to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: video.start_seconds,
        duration_seconds: video.duration_seconds,
        source: video.source.clone(),
        label: format!("{} audio", video.label),
        properties,
    }
}

fn strip_detached_audio(video: &mut TimelineItem, link_group_id: &str) {
    for key in MOVED_AUDIO_PROPERTIES {
        video.properties.remove(key);
    }
    if let Some(Value::Object(lanes)) = video.properties.get_mut("keyframes") {
        lanes.remove("volumeDb");
        if lanes.is_empty() {
            video.properties.remove("keyframes");
        }
    }
    if let Some(Value::Array(effects)) = video.properties.get_mut("effects") {
        effects.retain(|effect| !is_audio_effect(effect));
        if effects.is_empty() {
            video.properties.remove("effects");
        }
    }
    video
        .properties
        .insert("linkGroupId".to_string(), serde_json::json!(link_group_id));
    video
        .properties
        .insert("audioDetached".to_string(), Value::Bool(true));
}
