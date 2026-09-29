//! Clip transitions stay valid, in the same action, as their clips are edited.

use super::generated_asset_action;
use super::transitions::{
    add_transition, cut_project, project_with_crossfade, set_source_range, track, transition,
    VIDEO_TRACK,
};
use video_creater_lib::project::action::{
    apply_project_action, ProjectAction, ProjectActionMove, ProjectActionReplaceGeneratedOutput,
    ProjectActionResize, ProjectActionRippleDeleteRange, ProjectActionSplit, ProjectActionTrim,
    ProjectActionTrimEdge,
};
use video_creater_lib::project::model::*;

fn apply(project: &mut VideoProject, action: ProjectAction) {
    apply_project_action(project, action).expect("apply action");
}

fn transitions(project: &VideoProject) -> &[TimelineTransition] {
    &track(project, VIDEO_TRACK).transitions
}

fn assert_seconds(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.000_001,
        "expected {expected}, got {actual}"
    );
}

/// `cut_project` with a crossfade `fade-1` of `duration` on the cut.
fn project_with_crossfade_of(duration: f64) -> VideoProject {
    let mut project = cut_project();
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", duration),
    )
    .expect("add crossfade");
    project
}

fn move_item(item_id: &str, track_id: &str, start_seconds: f64) -> ProjectActionMove {
    ProjectActionMove {
        item_id: item_id.to_string(),
        target_track_id: track_id.to_string(),
        start_seconds,
    }
}

#[test]
fn moving_the_right_clip_away_drops_the_transition() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![move_item("clip-b", VIDEO_TRACK, 6.0)],
        },
    );
    assert!(transitions(&project).is_empty());
    let active = project
        .timelines
        .iter()
        .find(|entry| Some(&entry.id) == project.active_timeline_id.as_ref())
        .expect("active timeline entry");
    assert!(active.timeline.tracks[0].transitions.is_empty());
}

#[test]
fn moving_both_clips_together_keeps_the_transition() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![
                move_item("clip-b", VIDEO_TRACK, 14.0),
                move_item("clip-a", VIDEO_TRACK, 10.0),
            ],
        },
    );
    assert_eq!(
        transitions(&project),
        [transition("fade-1", "clip-a", "clip-b", 1.0)]
    );
}

#[test]
fn moving_a_clip_to_another_track_drops_the_transition() {
    let mut project = project_with_crossfade();
    project.timeline.tracks.push(TimelineTrack::empty(
        "track-video-2",
        "Video 2",
        TrackKind::Video,
    ));
    apply(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![move_item("clip-b", "track-video-2", 4.0)],
        },
    );
    assert!(transitions(&project).is_empty());
    assert!(track(&project, "track-video-2").transitions.is_empty());
}

#[test]
fn trimming_the_left_source_out_clamps_the_duration() {
    let mut project = project_with_crossfade_of(2.0);
    apply(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "clip-a".to_string(),
                start_seconds: 0.0,
                duration_seconds: 4.0,
                source_in: Some(7.75),
                source_out: Some(11.75),
            }],
        },
    );
    assert_eq!(transitions(&project).len(), 1);
    assert_seconds(transitions(&project)[0].duration_seconds, 0.5);
}

#[test]
fn ripple_trimming_the_left_clip_keeps_adjacency_and_clamps_the_duration() {
    let mut project = project_with_crossfade_of(2.0);
    apply(
        &mut project,
        ProjectAction::RippleTrimItem {
            item_id: "clip-a".to_string(),
            edge: ProjectActionTrimEdge::Right,
            delta_seconds: 5.5,
            propagate_linked: false,
            sync_locked_track_ids: Vec::new(),
        },
    );
    // clip-a now ends at source 11.5 of 12: 0.5 s of tail handle allows 1.0 s.
    assert_seconds(track(&project, VIDEO_TRACK).items[1].start_seconds, 9.5);
    assert_eq!(transitions(&project).len(), 1);
    assert_seconds(transitions(&project)[0].duration_seconds, 1.0);
}

#[test]
fn resizing_the_left_clip_away_from_the_cut_drops_the_transition() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id: "clip-a".to_string(),
                duration_seconds: 3.0,
            }],
        },
    );
    assert!(transitions(&project).is_empty());
}

#[test]
fn splitting_the_left_clip_retargets_the_transition_to_the_right_hand_piece() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "clip-a".to_string(),
                new_item_id: "clip-a-tail".to_string(),
                split_seconds: 2.0,
            }],
        },
    );
    assert_eq!(
        transitions(&project),
        [transition("fade-1", "clip-a-tail", "clip-b", 1.0)]
    );
}

#[test]
fn splitting_the_left_clip_near_the_cut_retargets_and_clamps() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "clip-a".to_string(),
                new_item_id: "clip-a-tail".to_string(),
                split_seconds: 3.75,
            }],
        },
    );
    assert_eq!(transitions(&project).len(), 1);
    assert_eq!(transitions(&project)[0].left_item_id, "clip-a-tail");
    assert_seconds(transitions(&project)[0].duration_seconds, 0.25);
}

#[test]
fn splitting_the_right_clip_keeps_the_transition_and_clamps_to_its_head() {
    let mut project = project_with_crossfade_of(2.0);
    apply(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "clip-b".to_string(),
                new_item_id: "clip-b-tail".to_string(),
                split_seconds: 5.5,
            }],
        },
    );
    assert_eq!(transitions(&project).len(), 1);
    assert_eq!(transitions(&project)[0].right_item_id, "clip-b");
    assert_seconds(transitions(&project)[0].duration_seconds, 1.5);
}

#[test]
fn removing_either_clip_drops_the_transition() {
    for removed in ["clip-a", "clip-b"] {
        let mut project = project_with_crossfade();
        apply(
            &mut project,
            ProjectAction::RemoveItems {
                item_ids: vec![removed.to_string()],
            },
        );
        assert!(transitions(&project).is_empty(), "removed {removed}");
    }
}

#[test]
fn ripple_delete_that_keeps_adjacency_keeps_the_transition() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::RippleDeleteRanges {
            ranges: vec![ProjectActionRippleDeleteRange {
                start_seconds: 3.0,
                end_seconds: 4.0,
                track_ids: vec![VIDEO_TRACK.to_string()],
            }],
        },
    );
    assert_seconds(track(&project, VIDEO_TRACK).items[1].start_seconds, 3.0);
    assert_eq!(
        transitions(&project),
        [transition("fade-1", "clip-a", "clip-b", 1.0)]
    );
}

#[test]
fn ripple_delete_inside_the_left_clip_retargets_to_its_remaining_tail() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::RippleDeleteRanges {
            ranges: vec![ProjectActionRippleDeleteRange {
                start_seconds: 1.0,
                end_seconds: 2.0,
                track_ids: vec![VIDEO_TRACK.to_string()],
            }],
        },
    );
    let items = &track(&project, VIDEO_TRACK).items;
    assert_eq!(items.len(), 3);
    assert_eq!(
        transitions(&project),
        [transition("fade-1", &items[1].id, "clip-b", 1.0)]
    );
    assert_ne!(items[1].id, "clip-a");
}

#[test]
fn inserting_into_the_left_clip_retargets_to_the_shifted_tail() {
    let mut project = project_with_crossfade();
    let inserted = super::transitions::clip("inserted", "Insert", 0.0, 1.0, 0.0);
    apply(
        &mut project,
        ProjectAction::InsertItems {
            target_track_id: VIDEO_TRACK.to_string(),
            insert_seconds: 2.0,
            items: vec![inserted],
        },
    );
    let items = &track(&project, VIDEO_TRACK).items;
    let tail = items
        .iter()
        .find(|item| item.id.starts_with("clip-a-") && item.start_seconds == 3.0)
        .expect("shifted tail of clip-a");
    assert_eq!(
        transitions(&project),
        [transition("fade-1", &tail.id, "clip-b", 1.0)]
    );
}

#[test]
fn speed_change_clamps_the_duration_to_the_speed_scaled_handle() {
    let mut project = cut_project();
    set_source_range(&mut project.timeline.tracks[0].items[0], 7.0, 11.0);
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", 2.0),
    )
    .expect("1 s of tail handle allows 2 s");

    apply(
        &mut project,
        ProjectAction::UpdateVisualClipSpeed {
            item_id: "clip-a".to_string(),
            speed: 4.0,
        },
    );
    // 1 s of source at 4x is 0.25 s of timeline on the left side.
    assert_eq!(transitions(&project).len(), 1);
    assert_seconds(transitions(&project)[0].duration_seconds, 0.5);
}

#[test]
fn replacing_a_clip_with_a_generated_output_without_handles_drops_the_transition() {
    let mut project = project_with_crossfade();
    apply(&mut project, generated_asset_action());
    apply(
        &mut project,
        ProjectAction::ReplaceTimelineItemWithGeneratedOutput {
            replacement: ProjectActionReplaceGeneratedOutput {
                item_id: "clip-b".to_string(),
                media_id: "generated-shot-1-output".to_string(),
            },
        },
    );
    assert!(transitions(&project).is_empty());
}

#[test]
fn deleting_the_track_media_of_a_clip_drops_the_transition() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::DeleteMedia {
            media_ids: vec!["media-1".to_string()],
        },
    );
    assert!(transitions(&project).is_empty());
}

#[test]
fn raising_the_frame_rate_drops_transitions_across_a_one_frame_gap() {
    let mut project = cut_project();
    project.timeline.tracks[0].items[1].start_seconds = 4.04;
    add_transition(
        &mut project,
        VIDEO_TRACK,
        transition("fade-1", "clip-a", "clip-b", 1.0),
    )
    .expect("0.04 s gap is within one frame at 24 fps");

    let mut settings = project.render_settings.clone();
    settings.fps = 60.0;
    apply(
        &mut project,
        ProjectAction::UpdateRenderSettings { settings },
    );
    assert!(transitions(&project).is_empty());
}

#[test]
fn decomposing_a_nested_sequence_carries_its_transitions_with_remapped_ids() {
    let mut project = project_with_crossfade();
    project.timelines.push(ProjectTimeline {
        id: "nested".to_string(),
        name: "Nested".to_string(),
        timeline: project.timeline.clone(),
    });
    let wrapper = TimelineItem {
        id: "wrapper".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 8.0,
        source: TimelineSource::Timeline {
            timeline_id: "nested".to_string(),
        },
        label: "Nested".to_string(),
        properties: Default::default(),
    };
    project.timeline.tracks[0].items = vec![wrapper];
    project.timeline.tracks[0].transitions.clear();

    apply(
        &mut project,
        ProjectAction::DecomposeTimelineItem {
            item_id: "wrapper".to_string(),
        },
    );

    let decomposed = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.id.starts_with("decomposed-wrapper-track-video"))
        .expect("decomposed video track");
    let item_ids = decomposed
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(item_ids, ["wrapper-clip-a", "wrapper-clip-b"]);
    assert_eq!(
        decomposed.transitions,
        [transition(
            "wrapper-fade-1",
            "wrapper-clip-a",
            "wrapper-clip-b",
            1.0
        )]
    );
    let nested = project
        .timelines
        .iter()
        .find(|entry| entry.id == "nested")
        .expect("nested timeline");
    assert_eq!(
        nested.timeline.tracks[0].transitions,
        [transition("fade-1", "clip-a", "clip-b", 1.0)]
    );
}

#[test]
fn decomposing_a_clipped_nested_sequence_clamps_carried_transitions() {
    let mut project = project_with_crossfade();
    project.timelines.push(ProjectTimeline {
        id: "nested".to_string(),
        name: "Nested".to_string(),
        timeline: project.timeline.clone(),
    });
    project.timeline.tracks[0].items = vec![TimelineItem {
        id: "wrapper".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 4.5,
        source: TimelineSource::Timeline {
            timeline_id: "nested".to_string(),
        },
        label: "Nested".to_string(),
        properties: Default::default(),
    }];
    project.timeline.tracks[0].transitions.clear();

    apply(
        &mut project,
        ProjectAction::DecomposeTimelineItem {
            item_id: "wrapper".to_string(),
        },
    );

    // The wrapper keeps only 0.5 s of clip-b, which caps the transition.
    let decomposed = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.id.starts_with("decomposed-wrapper-track-video"))
        .expect("decomposed video track");
    assert_eq!(decomposed.transitions.len(), 1);
    assert_seconds(decomposed.transitions[0].duration_seconds, 0.5);
}

#[test]
fn duplicating_a_timeline_copies_its_transitions() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::CreateTimeline {
            timeline_id: "copy".to_string(),
            name: "Copy".to_string(),
            duplicate_active: true,
            source_timeline_id: None,
        },
    );
    assert_eq!(project.active_timeline_id.as_deref(), Some("copy"));
    // Duplicated timelines keep item ids, so transition references stay valid.
    assert_eq!(
        transitions(&project),
        [transition("fade-1", "clip-a", "clip-b", 1.0)]
    );
    let original = project
        .timelines
        .iter()
        .find(|entry| entry.id == "main")
        .expect("original timeline");
    assert_eq!(original.timeline.tracks[0].transitions.len(), 1);
    assert_eq!(project.timeline.tracks[0].items[0].id, "clip-a");
}

#[test]
fn unrelated_edits_leave_transitions_untouched() {
    let mut project = project_with_crossfade();
    apply(
        &mut project,
        ProjectAction::UpdateVisualClipOpacity {
            item_id: "clip-a".to_string(),
            opacity: 0.5,
        },
    );
    assert_eq!(
        transitions(&project),
        [transition("fade-1", "clip-a", "clip-b", 1.0)]
    );
}
