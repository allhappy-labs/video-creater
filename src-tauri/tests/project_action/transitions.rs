//! Clip transitions: model, wire contract, and validation.

use super::assert_action_error_leaves_project_unchanged;
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::action::{apply_project_action, ProjectAction, ProjectActionError};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;

pub(crate) const VIDEO_TRACK: &str = "track-video";
pub(crate) const AUDIO_TRACK: &str = "track-audio";

/// A media-backed clip whose source range starts at `source_in` and consumes
/// `duration * speed` source seconds.
pub(crate) fn clip(
    id: &str,
    label: &str,
    start_seconds: f64,
    duration_seconds: f64,
    source_in: f64,
) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: label.to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(source_in)),
            ("sourceOut".to_string(), json!(source_in + duration_seconds)),
        ]),
    }
}

/// Two 4 s clips of the 12 s `media-1` meeting at 4 s. Each side has 6 s of
/// unused media at the cut, so the maximum transition is limited to 4 s by the
/// clip durations.
pub(crate) fn cut_project() -> VideoProject {
    let mut project = sample_project();
    project.timeline.tracks[0].items = vec![
        clip("clip-a", "Opening shot", 0.0, 4.0, 2.0),
        clip("clip-b", "Closing shot", 4.0, 4.0, 6.0),
    ];
    project.timeline.duration_seconds = 8.0;
    project
}

pub(crate) fn transition(id: &str, left: &str, right: &str, duration: f64) -> TimelineTransition {
    TimelineTransition {
        id: id.to_string(),
        left_item_id: left.to_string(),
        right_item_id: right.to_string(),
        kind: TransitionKind::Crossfade,
        duration_seconds: duration,
    }
}

pub(crate) fn add_transition(
    project: &mut VideoProject,
    track_id: &str,
    transition: TimelineTransition,
) -> Result<(), ProjectActionError> {
    apply_project_action(
        project,
        ProjectAction::AddTransition {
            track_id: track_id.to_string(),
            transition,
        },
    )
}

/// `cut_project` with a 1 s crossfade `fade-1` on the clip-a | clip-b cut.
pub(crate) fn project_with_crossfade() -> VideoProject {
    let mut project = cut_project();
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", 1.0),
    )
    .expect("add crossfade");
    project
}

pub(crate) fn track<'a>(project: &'a VideoProject, track_id: &str) -> &'a TimelineTrack {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .expect("track")
}

fn invalid(message: &str) -> ProjectActionError {
    ProjectActionError::InvalidTransition(message.to_string())
}

fn assert_add_rejected(project: &mut VideoProject, transition: TimelineTransition, message: &str) {
    assert_action_error_leaves_project_unchanged(
        project,
        ProjectAction::AddTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition,
        },
        invalid(message),
    );
}

#[test]
fn transition_track_serializes_camel_case_and_omits_empty_transitions() {
    let mut track = TimelineTrack::empty(VIDEO_TRACK, "Video", TrackKind::Video);
    let empty = serde_json::to_value(&track).expect("encode empty track");
    assert!(empty.get("transitions").is_none());

    track.transitions = vec![
        TimelineTransition {
            kind: TransitionKind::DipToBlack,
            ..transition("dip", "clip-a", "clip-b", 0.5)
        },
        TimelineTransition {
            kind: TransitionKind::DipToWhite,
            ..transition("white", "clip-b", "clip-c", 0.25)
        },
        TimelineTransition {
            kind: TransitionKind::Wipe,
            ..transition("wipe", "clip-c", "clip-d", 1.0)
        },
        transition("fade", "clip-d", "clip-e", 2.0),
    ];
    let value = serde_json::to_value(&track).expect("encode track");
    assert_eq!(
        value["transitions"],
        json!([
            { "id": "dip", "leftItemId": "clip-a", "rightItemId": "clip-b", "kind": "dipToBlack", "durationSeconds": 0.5 },
            { "id": "white", "leftItemId": "clip-b", "rightItemId": "clip-c", "kind": "dipToWhite", "durationSeconds": 0.25 },
            { "id": "wipe", "leftItemId": "clip-c", "rightItemId": "clip-d", "kind": "wipe", "durationSeconds": 1.0 },
            { "id": "fade", "leftItemId": "clip-d", "rightItemId": "clip-e", "kind": "crossfade", "durationSeconds": 2.0 },
        ])
    );
    let decoded: TimelineTrack = serde_json::from_value(value).expect("decode track");
    assert_eq!(decoded, track);

    let legacy: TimelineTrack = serde_json::from_value(json!({
        "id": VIDEO_TRACK, "name": "Video", "kind": "video", "locked": false, "items": []
    }))
    .expect("decode legacy track");
    assert!(legacy.transitions.is_empty());
}

#[test]
fn transition_actions_accept_camel_case_wire_contract() {
    let add: ProjectAction = serde_json::from_value(json!({
        "type": "addTransition",
        "trackId": VIDEO_TRACK,
        "transition": {
            "id": "fade-1",
            "leftItemId": "clip-a",
            "rightItemId": "clip-b",
            "kind": "crossfade",
            "durationSeconds": 1.0
        }
    }))
    .expect("decode addTransition");
    assert_eq!(
        add,
        ProjectAction::AddTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition: transition("fade-1", "clip-a", "clip-b", 1.0),
        }
    );

    let update: ProjectAction = serde_json::from_value(json!({
        "type": "updateTransition",
        "trackId": VIDEO_TRACK,
        "transitionId": "fade-1",
        "kind": "wipe",
        "durationSeconds": 0.5
    }))
    .expect("decode updateTransition");
    assert_eq!(
        update,
        ProjectAction::UpdateTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
            kind: Some(TransitionKind::Wipe),
            duration_seconds: Some(0.5),
        }
    );

    let kind_only = ProjectAction::UpdateTransition {
        track_id: VIDEO_TRACK.to_string(),
        transition_id: "fade-1".to_string(),
        kind: Some(TransitionKind::DipToWhite),
        duration_seconds: None,
    };
    let kind_only_json = json!({
        "type": "updateTransition",
        "trackId": VIDEO_TRACK,
        "transitionId": "fade-1",
        "kind": "dipToWhite"
    });
    assert_eq!(
        serde_json::to_value(&kind_only).expect("encode"),
        kind_only_json
    );
    assert_eq!(
        serde_json::from_value::<ProjectAction>(kind_only_json).expect("decode"),
        kind_only
    );

    let remove: ProjectAction = serde_json::from_value(json!({
        "type": "removeTransition",
        "trackId": VIDEO_TRACK,
        "transitionId": "fade-1"
    }))
    .expect("decode removeTransition");
    assert_eq!(
        remove,
        ProjectAction::RemoveTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
        }
    );
    assert_eq!(
        serde_json::from_value::<ProjectAction>(serde_json::to_value(&add).expect("encode"))
            .expect("round trip"),
        add
    );
}

#[test]
fn add_transition_succeeds_for_adjacent_video_clips_with_enough_handles() {
    let project = project_with_crossfade();

    assert_eq!(
        track(&project, VIDEO_TRACK).transitions,
        vec![transition("fade-1", "clip-a", "clip-b", 1.0)]
    );
    let active = project
        .timelines
        .iter()
        .find(|entry| Some(&entry.id) == project.active_timeline_id.as_ref())
        .expect("active timeline entry");
    assert_eq!(active.timeline, project.timeline);
}

#[test]
fn add_transition_accepts_up_to_one_frame_of_gap_and_rejects_more() {
    let mut project = cut_project();
    project.timeline.tracks[0].items[1].start_seconds = 4.0 + 1.0 / 24.0;
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", 1.0),
    )
    .expect("a one-frame gap still counts as a cut");

    let mut project = cut_project();
    project.timeline.tracks[0].items[1].start_seconds = 4.1;
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Opening shot must end where Closing shot starts to add a transition.",
    );

    let mut project = cut_project();
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-b", "clip-a", 1.0),
        "Closing shot must end where Opening shot starts to add a transition.",
    );
}

#[test]
fn add_transition_rejects_clips_on_different_tracks() {
    let mut project = cut_project();
    let right = project.timeline.tracks[0].items.remove(1);
    let mut second_track = TimelineTrack::empty("track-video-2", "Video 2", TrackKind::Video);
    second_track.items.push(right);
    project.timeline.tracks.push(second_track);

    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Opening shot and Closing shot must be on the same track to add a transition.",
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition: transition("fade-1", "clip-a", "missing", 1.0),
        },
        ProjectActionError::ItemNotFound("missing".to_string()),
    );
}

#[test]
fn add_transition_rejects_mixed_audio_and_visual_pair() {
    let mut project = cut_project();
    project.timeline.tracks[0].items[1].kind = TimelineItemKind::AudioClip;

    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Transitions need two visual clips or two audio clips, not a mix.",
    );
}

#[test]
fn add_transition_rejects_items_that_cannot_transition() {
    let mut project = cut_project();
    project.timeline.tracks[0].items[1].kind = TimelineItemKind::LottieClip;
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Transitions only work between video, image, generated, or audio clips.",
    );

    let mut project = cut_project();
    project.timeline.tracks[0].items[1].source = TimelineSource::Timeline {
        timeline_id: "main".to_string(),
    };
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Transitions don't support nested sequences yet.",
    );
}

#[test]
fn add_transition_rejects_a_duplicate_on_the_same_cut_or_id() {
    let mut project = project_with_crossfade();
    assert_add_rejected(
        &mut project,
        TimelineTransition {
            kind: TransitionKind::Wipe,
            ..transition("wipe-1", "clip-a", "clip-b", 0.5)
        },
        "Opening shot and Closing shot already have a transition.",
    );

    project.timeline.tracks[0]
        .items
        .push(clip("clip-c", "Outro", 8.0, 2.0, 10.0));
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-b", "clip-c", 0.5),
        "A transition with id fade-1 already exists.",
    );
    assert_add_rejected(
        &mut project,
        transition(" ", "clip-b", "clip-c", 0.5),
        "Transition id cannot be empty.",
    );
}

#[test]
fn add_transition_rejects_short_handles_and_reports_the_maximum() {
    let mut project = cut_project();
    set_source_range(&mut project.timeline.tracks[0].items[1], 0.2, 4.2);
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Not enough unused media before Closing shot for a 1.0s transition. Maximum is 0.4s.",
    );

    let mut project = cut_project();
    set_source_range(&mut project.timeline.tracks[0].items[0], 7.8, 11.8);
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Not enough unused media after Opening shot for a 1.0s transition. Maximum is 0.4s.",
    );
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", 0.4),
    )
    .expect("the reported maximum is accepted");
}

#[test]
fn add_transition_rejects_clips_shorter_than_the_transition() {
    let mut project = sample_project();
    project.timeline.tracks[0].items = vec![
        clip("clip-a", "Opening shot", 0.0, 0.5, 2.0),
        clip("clip-b", "Closing shot", 0.5, 0.75, 6.0),
    ];

    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.0),
        "Opening shot is too short for a 1.0s transition. Maximum is 0.5s.",
    );
}

#[test]
fn add_transition_handles_are_measured_in_timeline_seconds_at_clip_speed() {
    // clip-b plays at 2x, so its 1 s of source before sourceIn covers 0.5 s of
    // timeline: the transition can reach 0.5 s into it, 1.0 s in total.
    let mut project = cut_project();
    let right = &mut project.timeline.tracks[0].items[1];
    right.properties.insert("speed".to_string(), json!(2.0));
    set_source_range(right, 1.0, 9.0);

    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 1.2),
        "Not enough unused media before Closing shot for a 1.2s transition. Maximum is 1.0s.",
    );
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", 1.0),
    )
    .expect("speed-scaled maximum is accepted");
}

#[test]
fn add_transition_treats_still_images_as_unlimited_handles() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "still-1".to_string(),
        name: None,
        relative_path: "media/still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1920),
        height: Some(1080),
        fps: None,
        folder_id: None,
    });
    let still = |id: &str, start: f64| TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::ImageClip,
        start_seconds: start,
        duration_seconds: 6.0,
        source: TimelineSource::Media {
            media_id: "still-1".to_string(),
        },
        label: "Still".to_string(),
        properties: BTreeMap::new(),
    };
    project.timeline.tracks[0].items = vec![still("still-a", 0.0), still("still-b", 6.0)];

    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "still-a", "still-b", 5.0),
    )
    .expect("stills can always be extended");
}

#[test]
fn add_transition_accepts_adjacent_audio_clips() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "music".to_string(),
        name: None,
        relative_path: "media/music.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 30.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let audio = |id: &str, start: f64, source_in: f64| TimelineItem {
        kind: TimelineItemKind::AudioClip,
        source: TimelineSource::Media {
            media_id: "music".to_string(),
        },
        ..clip(id, "Music", start, 5.0, source_in)
    };
    project.timeline.tracks[4].items =
        vec![audio("music-a", 0.0, 0.0), audio("music-b", 5.0, 10.0)];

    add_transition(
        &mut project,
        AUDIO_TRACK,
        transition("audio-fade", "music-a", "music-b", 2.0),
    )
    .expect("audio crossfade");
    assert_eq!(track(&project, AUDIO_TRACK).transitions.len(), 1);
}

#[test]
fn add_transition_rejects_zero_and_over_five_second_durations() {
    let mut project = cut_project();
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 0.0),
        "Transition duration must be at least one frame (0.04s).",
    );
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", f64::NAN),
        "Transition duration must be at least one frame (0.04s).",
    );
    assert_add_rejected(
        &mut project,
        transition("fade-1", "clip-a", "clip-b", 5.5),
        "Transition duration cannot be longer than 5.0s.",
    );
}

#[test]
fn add_transition_rejects_locked_track() {
    let mut project = cut_project();
    project.timeline.tracks[0].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition: transition("fade-1", "clip-a", "clip-b", 1.0),
        },
        ProjectActionError::TrackLocked(VIDEO_TRACK.to_string()),
    );
}

#[test]
fn update_transition_changes_kind_and_duration_without_clamping() {
    let mut project = project_with_crossfade();
    apply_project_action(
        &mut project,
        ProjectAction::UpdateTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
            kind: Some(TransitionKind::DipToBlack),
            duration_seconds: None,
        },
    )
    .expect("update kind");
    assert_eq!(
        track(&project, VIDEO_TRACK).transitions,
        vec![TimelineTransition {
            kind: TransitionKind::DipToBlack,
            ..transition("fade-1", "clip-a", "clip-b", 1.0)
        }]
    );

    apply_project_action(
        &mut project,
        ProjectAction::UpdateTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
            kind: None,
            duration_seconds: Some(4.0),
        },
    )
    .expect("update duration to the maximum");
    assert_eq!(
        track(&project, VIDEO_TRACK).transitions[0].duration_seconds,
        4.0
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
            kind: None,
            duration_seconds: Some(4.5),
        },
        invalid("Opening shot is too short for a 4.5s transition. Maximum is 4.0s."),
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "missing".to_string(),
            kind: Some(TransitionKind::Wipe),
            duration_seconds: None,
        },
        ProjectActionError::TransitionNotFound("missing".to_string()),
    );
}

#[test]
fn remove_transition_removes_it() {
    let mut project = project_with_crossfade();
    apply_project_action(
        &mut project,
        ProjectAction::RemoveTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
        },
    )
    .expect("remove transition");
    assert!(track(&project, VIDEO_TRACK).transitions.is_empty());

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RemoveTransition {
            track_id: VIDEO_TRACK.to_string(),
            transition_id: "fade-1".to_string(),
        },
        ProjectActionError::TransitionNotFound("fade-1".to_string()),
    );
}

pub(crate) fn set_source_range(item: &mut TimelineItem, source_in: f64, source_out: f64) {
    item.properties
        .insert("sourceIn".to_string(), json!(source_in));
    item.properties
        .insert("sourceOut".to_string(), json!(source_out));
}
