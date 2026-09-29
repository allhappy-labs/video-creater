//! Audio clip edits: `updateAudioClipSpeed` and `detachAudio`.

use super::assert_action_error_leaves_project_unchanged;
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::action::{
    apply_project_action, ProjectAction, ProjectActionError, ProjectActionResize,
    ProjectActionTrimEdge,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;

const AUDIO_TRACK: &str = "track-audio";

fn audio_clip(id: &str, start_seconds: f64, duration_seconds: f64, source_in: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Media {
            media_id: "music".to_string(),
        },
        label: id.to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(source_in)),
            ("sourceOut".to_string(), json!(source_in + duration_seconds)),
        ]),
    }
}

/// `sample_project` plus a 10 s music file and `left` (0-2 s, source 0-2)
/// and `right` (2-4 s, source 1-3) on the audio track.
fn audio_project() -> VideoProject {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "music".to_string(),
        name: None,
        relative_path: "media/music.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 10.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == AUDIO_TRACK)
        .expect("audio track");
    track.items = vec![
        audio_clip("left", 0.0, 2.0, 0.0),
        audio_clip("right", 2.0, 2.0, 1.0),
    ];
    project
}

fn audio_item<'a>(project: &'a VideoProject, id: &str) -> &'a TimelineItem {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == id)
        .expect("timeline item")
}

fn speed_action(item_id: &str, speed: f64) -> ProjectAction {
    ProjectAction::UpdateAudioClipSpeed {
        item_id: item_id.to_string(),
        speed,
    }
}

#[test]
fn update_audio_clip_speed_round_trips_its_wire_shape() {
    let value = json!({ "type": "updateAudioClipSpeed", "itemId": "right", "speed": 2.0 });
    let action: ProjectAction = serde_json::from_value(value.clone()).expect("decode");
    assert_eq!(action, speed_action("right", 2.0));
    assert_eq!(serde_json::to_value(&action).expect("encode"), value);
}

#[test]
fn update_audio_clip_speed_sets_and_clears_speed_without_resizing() {
    let mut project = audio_project();
    apply_project_action(&mut project, speed_action("right", 2.0)).expect("speed 2");
    let right = audio_item(&project, "right");
    assert_eq!(right.properties.get("speed"), Some(&json!(2.0)));
    assert_eq!(right.duration_seconds, 2.0);

    apply_project_action(&mut project, speed_action("right", 1.0)).expect("speed 1");
    assert_eq!(audio_item(&project, "right").properties.get("speed"), None);
}

#[test]
fn update_audio_clip_speed_rejects_out_of_range_speeds() {
    for speed in [0.05, 9.0, f64::NAN] {
        let mut project = audio_project();
        assert_action_error_leaves_project_unchanged(
            &mut project,
            speed_action("right", speed),
            ProjectActionError::InvalidAudioClipSpeed("right".to_string()),
        );
    }
}

#[test]
fn update_audio_clip_speed_rejects_visual_clips_locked_tracks_and_missing_items() {
    let mut project = audio_project();
    assert_action_error_leaves_project_unchanged(
        &mut project,
        speed_action("item-1", 2.0),
        ProjectActionError::NotAudioClip("item-1".to_string()),
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        speed_action("missing", 2.0),
        ProjectActionError::ItemNotFound("missing".to_string()),
    );
    for track in &mut project.timeline.tracks {
        if track.id == AUDIO_TRACK {
            track.locked = true;
        }
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        speed_action("right", 2.0),
        ProjectActionError::TrackLocked(AUDIO_TRACK.to_string()),
    );
}

#[test]
fn retimed_audio_crossfades_revalidate_against_speed_scaled_handles() {
    let mut project = audio_project();
    apply_project_action(
        &mut project,
        ProjectAction::AddTransition {
            track_id: AUDIO_TRACK.to_string(),
            transition: TimelineTransition {
                id: "fade".to_string(),
                left_item_id: "left".to_string(),
                right_item_id: "right".to_string(),
                kind: TransitionKind::Crossfade,
                duration_seconds: 1.2,
            },
        },
    )
    .expect("1.2 s crossfade fits the 1 s head handle at speed 1");

    // At speed 2, sourceIn 1 leaves a 0.5 s head handle: at most a 1 s transition.
    apply_project_action(&mut project, speed_action("right", 2.0)).expect("speed 2");
    apply_project_action(
        &mut project,
        ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id: "right".to_string(),
                duration_seconds: 1.0,
            }],
        },
    )
    .expect("resize to the retimed duration");

    let right = audio_item(&project, "right");
    assert_eq!(right.duration_seconds, 1.0);
    let track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.id == AUDIO_TRACK)
        .expect("audio track");
    assert_eq!(track.transitions.len(), 1);
    assert!(
        (track.transitions[0].duration_seconds - 1.0).abs() < 1e-9,
        "clamped to the speed-scaled handle: {:?}",
        track.transitions[0]
    );
}

/// `sample_project`'s `item-1` (0-4 s of the 12 s `media-1` video) with a
/// source range, volume, a volume lane, an audio effect and a visual fade.
fn detach_project() -> VideoProject {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(1.0)),
        ("sourceOut".to_string(), json!(5.0)),
        ("volumeDb".to_string(), json!(-6.0)),
        ("fadeInSeconds".to_string(), json!(0.5)),
        (
            "keyframes".to_string(),
            json!({ "volumeDb": [{ "atSeconds": 0.0, "value": -12.0 }], "opacity": [{ "atSeconds": 0.0, "value": 1.0 }] }),
        ),
        (
            "effects".to_string(),
            json!([{ "effectType": "audio.denoise", "enabled": true }, { "effectType": "blur.gaussian", "enabled": true }]),
        ),
    ]);
    project
}

fn detach_action(target_track_id: &str) -> ProjectAction {
    ProjectAction::DetachAudio {
        item_id: "item-1".to_string(),
        audio_item_id: "item-1-audio".to_string(),
        target_track_id: target_track_id.to_string(),
        link_group_id: "link-item-1".to_string(),
    }
}

#[test]
fn detach_audio_round_trips_its_wire_shape() {
    let value = json!({
        "type": "detachAudio", "itemId": "item-1", "audioItemId": "item-1-audio",
        "targetTrackId": "track-audio", "linkGroupId": "link-item-1",
    });
    let action: ProjectAction = serde_json::from_value(value.clone()).expect("decode");
    assert_eq!(action, detach_action(AUDIO_TRACK));
    assert_eq!(serde_json::to_value(&action).expect("encode"), value);
}

#[test]
fn detach_audio_creates_a_linked_clip_and_moves_the_sound_properties() {
    let mut project = detach_project();
    apply_project_action(&mut project, detach_action(AUDIO_TRACK)).expect("detach audio");

    let audio = audio_item(&project, "item-1-audio");
    assert_eq!(audio.kind, TimelineItemKind::AudioClip);
    assert_eq!(audio.label, "Opening clip audio");
    assert_eq!((audio.start_seconds, audio.duration_seconds), (0.0, 4.0));
    assert_eq!(
        audio.source,
        TimelineSource::Media {
            media_id: "media-1".to_string()
        }
    );
    assert_eq!(
        serde_json::to_value(&audio.properties).expect("audio properties"),
        json!({
            "sourceIn": 1.0, "sourceOut": 5.0, "volumeDb": -6.0,
            "keyframes": { "volumeDb": [{ "atSeconds": 0.0, "value": -12.0 }] },
            "effects": [{ "effectType": "audio.denoise", "enabled": true }],
            "linkGroupId": "link-item-1", "sourceClipType": "audio",
        })
    );
    let video = audio_item(&project, "item-1");
    assert_eq!(
        serde_json::to_value(&video.properties).expect("video properties"),
        json!({
            "sourceIn": 1.0, "sourceOut": 5.0, "fadeInSeconds": 0.5,
            "keyframes": { "opacity": [{ "atSeconds": 0.0, "value": 1.0 }] },
            "effects": [{ "effectType": "blur.gaussian", "enabled": true }],
            "linkGroupId": "link-item-1", "audioDetached": true,
        })
    );
}

#[test]
fn detach_audio_copies_speed_and_keeps_an_existing_link_group() {
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 2.0;
    item.properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(0.0)),
        ("sourceOut".to_string(), json!(4.0)),
        ("speed".to_string(), json!(2.0)),
        ("linkGroupId".to_string(), json!("link-old")),
    ]);
    let mismatched = project.clone();
    let mut action = detach_action(AUDIO_TRACK);
    if let ProjectAction::DetachAudio { link_group_id, .. } = &mut action {
        *link_group_id = "link-old".to_string();
    }
    apply_project_action(&mut project, action).expect("detach retimed clip");
    let audio = audio_item(&project, "item-1-audio");
    assert_eq!(audio.properties.get("speed"), Some(&json!(2.0)));
    assert_eq!(
        audio.properties.get("linkGroupId"),
        Some(&json!("link-old"))
    );

    let mut mismatched = mismatched;
    let before = mismatched.clone();
    assert!(apply_project_action(&mut mismatched, detach_action(AUDIO_TRACK)).is_err());
    assert_eq!(mismatched, before);
}

#[test]
fn detach_audio_rejects_linked_clips_and_media_without_sound() {
    let mut linked = detach_project();
    apply_project_action(&mut linked, detach_action(AUDIO_TRACK)).expect("first detach");
    let mut again = detach_action(AUDIO_TRACK);
    if let ProjectAction::DetachAudio { audio_item_id, .. } = &mut again {
        *audio_item_id = "item-1-audio-2".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut linked,
        again,
        ProjectActionError::AudioAlreadyDetached("item-1".to_string()),
    );

    let mut image = sample_project();
    image.media[0].kind = MediaKind::Image;
    image.timeline.tracks[0].items[0].kind = TimelineItemKind::ImageClip;
    assert_action_error_leaves_project_unchanged(
        &mut image,
        detach_action(AUDIO_TRACK),
        ProjectActionError::NoDetachableAudio("item-1".to_string()),
    );

    let mut audio_media = sample_project();
    audio_media.media[0].kind = MediaKind::Audio;
    assert_action_error_leaves_project_unchanged(
        &mut audio_media,
        detach_action(AUDIO_TRACK),
        ProjectActionError::NoDetachableAudio("item-1".to_string()),
    );
}

#[test]
fn detach_audio_rejects_overlaps_locked_tracks_and_the_wrong_track_kind() {
    let mut overlap = audio_project();
    assert_action_error_leaves_project_unchanged(
        &mut overlap,
        detach_action(AUDIO_TRACK),
        ProjectActionError::TrackItemOverlap {
            track_id: AUDIO_TRACK.to_string(),
            item_id: "item-1-audio".to_string(),
            blocking_item_id: "left".to_string(),
        },
    );

    let mut locked = sample_project();
    for track in &mut locked.timeline.tracks {
        if track.id == AUDIO_TRACK {
            track.locked = true;
        }
    }
    assert_action_error_leaves_project_unchanged(
        &mut locked,
        detach_action(AUDIO_TRACK),
        ProjectActionError::TrackLocked(AUDIO_TRACK.to_string()),
    );

    let mut wrong_kind = sample_project();
    assert_action_error_leaves_project_unchanged(
        &mut wrong_kind,
        detach_action("track-video"),
        ProjectActionError::TrackTypeMismatch {
            item_kind: TimelineItemKind::AudioClip,
            track_kind: TrackKind::Video,
        },
    );
    assert_action_error_leaves_project_unchanged(
        &mut wrong_kind,
        detach_action("missing"),
        ProjectActionError::TrackNotFound("missing".to_string()),
    );
}

#[test]
fn detached_audio_follows_linked_ripple_trims() {
    let mut project = detach_project();
    apply_project_action(&mut project, detach_action(AUDIO_TRACK)).expect("detach audio");
    apply_project_action(
        &mut project,
        ProjectAction::RippleTrimItem {
            item_id: "item-1".to_string(),
            edge: ProjectActionTrimEdge::Right,
            delta_seconds: -1.0,
            propagate_linked: true,
            sync_locked_track_ids: Vec::new(),
        },
    )
    .expect("ripple trim the video");
    assert_eq!(audio_item(&project, "item-1").duration_seconds, 3.0);
    assert_eq!(audio_item(&project, "item-1-audio").duration_seconds, 3.0);
}
