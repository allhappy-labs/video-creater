//! Reversed clips: `updateClipReverse` and the reversed source mapping.
//!
//! The numbers match `src/lib/timeline-ops/reverse.test.ts`.

use super::assert_action_error_leaves_project_unchanged;
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::project::action::{
    apply_project_action, ProjectAction, ProjectActionError, ProjectActionRippleDeleteRange,
    ProjectActionSplit, ProjectActionTrimEdge,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;
use video_creater_lib::project::reverse::SourceWindow;
use video_creater_lib::project::transitions::{transition_bounds, TransitionLimit};

const VIDEO_TRACK: &str = "track-video";
const AUDIO_TRACK: &str = "track-audio";

fn media(id: &str, kind: MediaKind) -> MediaAsset {
    MediaAsset {
        id: id.to_string(),
        name: None,
        relative_path: format!("media/{id}"),
        kind,
        duration_seconds: 20.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    }
}

/// A clip of the 20 s `reel` video (or `tone` audio for audio clips).
fn clip(
    id: &str,
    kind: TimelineItemKind,
    (start, duration): (f64, f64),
    (source_in, source_out): (f64, f64),
    speed: f64,
    reverse: bool,
) -> TimelineItem {
    let media_id = if kind == TimelineItemKind::AudioClip {
        "tone"
    } else {
        "reel"
    };
    let mut properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(source_in)),
        ("sourceOut".to_string(), json!(source_out)),
    ]);
    if speed != 1.0 {
        properties.insert("speed".to_string(), json!(speed));
    }
    if reverse {
        properties.insert("reverse".to_string(), json!(true));
    }
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: id.to_string(),
        properties,
    }
}

/// `rev`: 10-14 s on the timeline, source 2-10 at speed 2, reversed.
fn reversed_clip() -> TimelineItem {
    clip(
        "rev",
        TimelineItemKind::VideoClip,
        (10.0, 4.0),
        (2.0, 10.0),
        2.0,
        true,
    )
}

fn project_with(video_items: Vec<TimelineItem>) -> VideoProject {
    let mut project = sample_project();
    project.media.extend([
        media("reel", MediaKind::Video),
        media("tone", MediaKind::Audio),
        media("still", MediaKind::Image),
    ]);
    project.timeline.tracks[0].items = video_items;
    project.timeline.duration_seconds = 20.0;
    project
}

fn item<'a>(project: &'a VideoProject, id: &str) -> &'a TimelineItem {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == id)
        .expect("timeline item")
}

fn source_range(item: &TimelineItem) -> (f64, f64) {
    (
        item.properties["sourceIn"].as_f64().expect("sourceIn"),
        item.properties["sourceOut"].as_f64().expect("sourceOut"),
    )
}

fn reverse_action(item_id: &str, reverse: bool) -> ProjectAction {
    ProjectAction::UpdateClipReverse {
        item_id: item_id.to_string(),
        reverse,
    }
}

fn window(reverse: bool) -> SourceWindow {
    SourceWindow {
        source_in: 2.0,
        source_out: 10.0,
        speed: 2.0,
        reverse,
    }
}

#[test]
fn update_clip_reverse_round_trips_its_wire_shape() {
    let value = json!({ "type": "updateClipReverse", "itemId": "rev", "reverse": true });
    let action: ProjectAction = serde_json::from_value(value.clone()).expect("decode");
    assert_eq!(action, reverse_action("rev", true));
    assert_eq!(serde_json::to_value(&action).expect("encode"), value);
}

#[test]
fn update_clip_reverse_sets_and_clears_reverse_on_video_and_audio_clips() {
    let mut project = project_with(vec![clip(
        "forward",
        TimelineItemKind::VideoClip,
        (0.0, 4.0),
        (2.0, 10.0),
        2.0,
        false,
    )]);
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == AUDIO_TRACK)
        .expect("audio track")
        .items = vec![clip(
        "sound",
        TimelineItemKind::AudioClip,
        (0.0, 4.0),
        (0.0, 4.0),
        1.0,
        false,
    )];
    let before = item(&project, "forward").clone();

    for id in ["forward", "sound"] {
        apply_project_action(&mut project, reverse_action(id, true)).expect("reverse");
        assert_eq!(
            item(&project, id).properties.get("reverse"),
            Some(&json!(true))
        );
    }
    let reversed = item(&project, "forward");
    assert_eq!(reversed.duration_seconds, before.duration_seconds);
    assert_eq!(source_range(reversed), source_range(&before));

    for id in ["forward", "sound"] {
        apply_project_action(&mut project, reverse_action(id, false)).expect("play forward");
        assert_eq!(item(&project, id).properties.get("reverse"), None);
    }
    assert_eq!(item(&project, "forward"), &before);
}

#[test]
fn update_clip_reverse_rejects_other_clips_locked_tracks_and_missing_items() {
    let mut still = clip(
        "still-1",
        TimelineItemKind::ImageClip,
        (4.0, 2.0),
        (0.0, 2.0),
        1.0,
        false,
    );
    still.source = TimelineSource::Media {
        media_id: "still".to_string(),
    };
    let mut image_backed_video = still.clone();
    image_backed_video.id = "image-video".to_string();
    image_backed_video.kind = TimelineItemKind::VideoClip;
    image_backed_video.start_seconds = 6.0;
    let mut generated_sound = clip(
        "generated-sound",
        TimelineItemKind::AudioClip,
        (0.0, 2.0),
        (0.0, 2.0),
        1.0,
        false,
    );
    generated_sound.source = TimelineSource::Generated {
        artifact_id: "artifact-1".to_string(),
    };
    let mut project = project_with(vec![reversed_clip(), still, image_backed_video]);
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == AUDIO_TRACK)
        .expect("audio track")
        .items = vec![generated_sound];

    for id in ["still-1", "image-video", "generated-sound"] {
        assert_action_error_leaves_project_unchanged(
            &mut project,
            reverse_action(id, true),
            ProjectActionError::NotReversibleClip(id.to_string()),
        );
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        reverse_action("missing", true),
        ProjectActionError::ItemNotFound("missing".to_string()),
    );
    project.timeline.tracks[0].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        reverse_action("rev", false),
        ProjectActionError::TrackLocked(VIDEO_TRACK.to_string()),
    );
}

#[test]
fn reversed_clips_read_source_backwards_from_source_out() {
    let forward = window(false);
    let reversed = window(true);
    assert_eq!(forward.source_seconds_at(1.0), 4.0);
    assert_eq!(reversed.source_seconds_at(0.0), 10.0);
    assert_eq!(reversed.source_seconds_at(1.0), 8.0);
    assert_eq!(reversed.source_seconds_at(4.0), 2.0);
}

#[test]
fn reversed_clips_map_source_seconds_back_to_the_mirrored_timeline() {
    let ten_seconds = |speed: f64, reverse: bool| SourceWindow {
        source_in: 0.0,
        source_out: 10.0,
        speed,
        reverse,
    };
    assert_eq!(ten_seconds(1.0, false).local_seconds_for_source(1.0), 1.0);
    assert_eq!(ten_seconds(1.0, true).local_seconds_for_source(1.0), 9.0);
    assert_eq!(ten_seconds(2.0, true).local_seconds_for_source(1.0), 4.5);
    assert_eq!(window(true).local_seconds_for_source(8.0), 1.0);
    // Source 1-2 s of a 0-10 s window plays at 1-2 s forward and 8-9 s reversed.
    assert_eq!(
        ten_seconds(1.0, false).timeline_range_for_source(0.0, 1.0, 2.0),
        (1.0, 2.0)
    );
    assert_eq!(
        ten_seconds(1.0, true).timeline_range_for_source(0.0, 1.0, 2.0),
        (8.0, 9.0)
    );
    assert_eq!(
        ten_seconds(1.0, true).timeline_range_for_source(10.0, 1.0, 2.0),
        (18.0, 19.0)
    );
    assert_eq!(
        ten_seconds(2.0, false).timeline_range_for_source(0.0, 1.0, 2.0),
        (0.5, 1.0)
    );
    assert_eq!(
        ten_seconds(2.0, true).timeline_range_for_source(0.0, 1.0, 2.0),
        (4.0, 4.5)
    );
}

#[test]
fn reversed_clip_handles_swap_head_and_tail() {
    assert_eq!(window(false).head_handle_seconds(20.0), 1.0);
    assert_eq!(window(false).tail_handle_seconds(20.0), 5.0);
    assert_eq!(window(true).head_handle_seconds(20.0), 5.0);
    assert_eq!(window(true).tail_handle_seconds(20.0), 1.0);
}

#[test]
fn reversed_range_renders_read_the_mirrored_source_window() {
    // Timeline 11-12.5 s is clip-local 1-2.5 s.
    assert_eq!(window(false).source_range_for(1.0, 2.5), (4.0, 7.0));
    assert_eq!(window(true).source_range_for(1.0, 2.5), (5.0, 8.0));
}

#[test]
fn splitting_a_reversed_clip_keeps_source_out_on_the_left_part() {
    let mut project = project_with(vec![reversed_clip()]);
    apply_project_action(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "rev".to_string(),
                new_item_id: "rev-2".to_string(),
                split_seconds: 11.5,
            }],
        },
    )
    .expect("split");

    let left = item(&project, "rev");
    let right = item(&project, "rev-2");
    assert_eq!((left.start_seconds, left.duration_seconds), (10.0, 1.5));
    assert_eq!(source_range(left), (7.0, 10.0));
    assert_eq!((right.start_seconds, right.duration_seconds), (11.5, 2.5));
    assert_eq!(source_range(right), (2.0, 7.0));
    assert_eq!(right.properties.get("reverse"), Some(&json!(true)));
}

#[test]
fn ripple_deleting_inside_a_reversed_clip_mirrors_both_parts() {
    let mut project = project_with(vec![reversed_clip()]);
    apply_project_action(
        &mut project,
        ProjectAction::RippleDeleteRanges {
            ranges: vec![ProjectActionRippleDeleteRange {
                start_seconds: 11.0,
                end_seconds: 12.0,
                track_ids: vec![VIDEO_TRACK.to_string()],
            }],
        },
    )
    .expect("ripple delete");

    let items = &project.timeline.tracks[0].items;
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(source_range(&items[0]), (8.0, 10.0));
    assert_eq!(source_range(&items[1]), (2.0, 6.0));
}

fn ripple_trim(project: &mut VideoProject, edge: ProjectActionTrimEdge, delta_seconds: f64) {
    apply_project_action(
        project,
        ProjectAction::RippleTrimItem {
            item_id: "rev".to_string(),
            edge,
            delta_seconds,
            propagate_linked: false,
            sync_locked_track_ids: Vec::new(),
        },
    )
    .expect("ripple trim");
}

#[test]
fn trimming_the_left_edge_of_a_reversed_clip_moves_source_out() {
    let mut project = project_with(vec![reversed_clip()]);
    ripple_trim(&mut project, ProjectActionTrimEdge::Left, 0.5);
    assert_eq!(item(&project, "rev").duration_seconds, 3.5);
    assert_eq!(source_range(item(&project, "rev")), (2.0, 9.0));

    assert_eq!(
        window(true).trimmed(ProjectActionTrimEdge::Left, 0.5),
        (2.0, 11.0)
    );
}

#[test]
fn trimming_the_right_edge_of_a_reversed_clip_moves_source_in() {
    let mut project = project_with(vec![reversed_clip()]);
    ripple_trim(&mut project, ProjectActionTrimEdge::Right, -0.5);
    assert_eq!(item(&project, "rev").duration_seconds, 3.5);
    assert_eq!(source_range(item(&project, "rev")), (3.0, 10.0));

    assert_eq!(
        window(true).trimmed(ProjectActionTrimEdge::Right, 0.5),
        (1.0, 10.0)
    );
}

#[test]
fn transition_maximums_use_reversed_handles() {
    // A reversed left clip's tail handle is sourceIn / speed = 1 s.
    let project = project_with(vec![
        clip(
            "a",
            TimelineItemKind::VideoClip,
            (0.0, 4.0),
            (2.0, 10.0),
            2.0,
            true,
        ),
        clip(
            "b",
            TimelineItemKind::VideoClip,
            (4.0, 4.0),
            (8.0, 12.0),
            1.0,
            false,
        ),
    ]);
    let items = &project.timeline.tracks[0].items;
    let bounds = transition_bounds(&project, &items[0], &items[1]);
    assert_eq!(bounds.max_seconds, 2.0);
    assert_eq!(bounds.limit, TransitionLimit::LeftHandle);

    // A reversed right clip's head handle is (20 - sourceOut) / speed = 1 s.
    let project = project_with(vec![
        clip(
            "c",
            TimelineItemKind::VideoClip,
            (0.0, 4.0),
            (0.0, 4.0),
            1.0,
            false,
        ),
        clip(
            "d",
            TimelineItemKind::VideoClip,
            (4.0, 4.0),
            (15.0, 19.0),
            1.0,
            true,
        ),
    ]);
    let items = &project.timeline.tracks[0].items;
    let bounds = transition_bounds(&project, &items[0], &items[1]);
    assert_eq!(bounds.max_seconds, 2.0);
    assert_eq!(bounds.limit, TransitionLimit::RightHandle);
}

#[test]
fn splitting_a_reversed_left_clip_keeps_its_transition_on_the_right_part() {
    let mut project = project_with(vec![
        clip(
            "a",
            TimelineItemKind::VideoClip,
            (0.0, 4.0),
            (2.0, 10.0),
            2.0,
            true,
        ),
        clip(
            "b",
            TimelineItemKind::VideoClip,
            (4.0, 4.0),
            (8.0, 12.0),
            1.0,
            false,
        ),
    ]);
    project.timeline.tracks[0].transitions = vec![TimelineTransition {
        id: "fade".to_string(),
        left_item_id: "a".to_string(),
        right_item_id: "b".to_string(),
        kind: TransitionKind::Crossfade,
        duration_seconds: 1.0,
    }];
    apply_project_action(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "a".to_string(),
                new_item_id: "a-2".to_string(),
                split_seconds: 2.0,
            }],
        },
    )
    .expect("split");

    assert_eq!(source_range(item(&project, "a-2")), (2.0, 6.0));
    let transitions = &project.timeline.tracks[0].transitions;
    assert_eq!(transitions.len(), 1, "{transitions:?}");
    assert_eq!(transitions[0].left_item_id, "a-2");
    assert_eq!(transitions[0].duration_seconds, 1.0);
}

#[test]
fn detaching_audio_from_a_reversed_clip_keeps_the_sound_reversed() {
    let mut project = project_with(vec![reversed_clip()]);
    apply_project_action(
        &mut project,
        ProjectAction::DetachAudio {
            item_id: "rev".to_string(),
            audio_item_id: "rev-audio".to_string(),
            target_track_id: AUDIO_TRACK.to_string(),
            link_group_id: "link-rev".to_string(),
        },
    )
    .expect("detach audio");

    let audio = item(&project, "rev-audio");
    assert_eq!(source_range(audio), (2.0, 10.0));
    assert_eq!(audio.properties.get("reverse"), Some(&json!(true)));
}
