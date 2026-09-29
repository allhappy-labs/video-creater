//! Render plan expansion for clip transitions.

use super::transition_fixtures::*;
use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::edit::render_plan::RenderPlan;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::project_export::expand_project_nested_timelines_for_render;

#[test]
fn crossfade_extends_both_clips_into_their_handles_without_touching_the_project() {
    let project = crossfade_project();
    let canonical = project.clone();

    let plan = webm_plan(&project);

    assert_eq!(plan.clips.len(), 2);
    assert_eq!(timing(&plan.clips[0]), (0.0, 4.5, 2.0, 6.5));
    assert_eq!(handles(&plan.clips[0]), (None, Some(0.5)));
    assert_eq!(timing(&plan.clips[1]), (3.5, 4.5, 5.5, 10.0));
    assert_eq!(handles(&plan.clips[1]), (Some(0.5), None));
    assert_eq!(
        plan.transitions,
        vec![render_transition(TransitionKind::Crossfade, 3.5, 1.0, 0, 1)]
    );
    assert!(plan.audio_transitions.is_empty());
    assert_eq!(
        project, canonical,
        "the canonical project is never extended"
    );

    let wire = serde_json::to_value(&plan).expect("plan serializes");
    assert_eq!(
        wire["transitions"],
        json!([{
            "kind": "crossfade",
            "startSeconds": 3.5,
            "durationSeconds": 1.0,
            "leftClipIndex": 0,
            "rightClipIndex": 1
        }])
    );
    assert!(wire.get("audioTransitions").is_none());
    let decoded: RenderPlan = serde_json::from_value(wire).expect("plan round-trips");
    assert_eq!(decoded, plan);
}

#[test]
fn handle_extension_consumes_source_media_at_clip_speed() {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        // 2 s at 2x consumes source 0-4.
        clip(
            "fast",
            TimelineItemKind::VideoClip,
            "media-2",
            0.0,
            2.0,
            0.0,
            2.0,
        ),
        // 4 s at 0.5x consumes source 6-8.
        clip(
            "slow",
            TimelineItemKind::VideoClip,
            "media-2",
            2.0,
            4.0,
            6.0,
            0.5,
        ),
    ];
    project.timeline.duration_seconds = 6.0;
    add_transition(
        &mut project,
        "track-video",
        "speed-fade",
        ("fast", "slow"),
        TransitionKind::Crossfade,
        1.0,
    );

    let plan = webm_plan(&project);

    // Left: +0.5 s of timeline consumes 0.5 * 2 = 1 s of source.
    assert_eq!(timing(&plan.clips[0]), (0.0, 2.5, 0.0, 5.0));
    // Right: -0.5 s of timeline consumes 0.5 * 0.5 = 0.25 s of source.
    assert_eq!(timing(&plan.clips[1]), (1.5, 4.5, 5.75, 8.0));
    assert_eq!(
        plan.transitions,
        vec![render_transition(TransitionKind::Crossfade, 1.5, 1.0, 0, 1)]
    );
}

#[test]
fn range_renders_through_a_transition_keep_both_clips_and_range_relative_timing() {
    let project = crossfade_project();

    // The range starts before the cut, inside the window [3.5, 4.5].
    let plan = range_plan(&project, 3.75, 6.0);
    assert_eq!(plan.clips.len(), 2);
    assert_eq!(timing(&plan.clips[0]), (0.0, 0.75, 5.75, 6.5));
    assert_eq!(handles(&plan.clips[0]), (None, Some(0.5)));
    assert_eq!(timing(&plan.clips[1]), (0.0, 2.25, 5.75, 8.0));
    assert_eq!(handles(&plan.clips[1]), (Some(0.25), None));
    assert_eq!(
        plan.transitions,
        vec![render_transition(
            TransitionKind::Crossfade,
            -0.25,
            1.0,
            0,
            1
        )]
    );

    // The range starts after the cut, still inside the window.
    let plan = range_plan(&project, 4.25, 6.0);
    assert_eq!(plan.clips.len(), 2);
    assert_eq!(timing(&plan.clips[0]), (0.0, 0.25, 6.25, 6.5));
    assert_eq!(
        plan.transitions,
        vec![render_transition(
            TransitionKind::Crossfade,
            -0.75,
            1.0,
            0,
            1
        )]
    );

    // The range ends just inside the window's start.
    let plan = range_plan(&project, 1.0, 3.6);
    assert_eq!(plan.clips.len(), 2);
    assert_eq!(timing(&plan.clips[1]), (2.5, 0.1, 5.5, 5.6));
    assert_eq!(
        plan.transitions,
        vec![render_transition(TransitionKind::Crossfade, 2.5, 1.0, 0, 1)]
    );

    // A range after the window renders the right clip alone, with no transition.
    let plan = range_plan(&project, 4.75, 6.0);
    assert_eq!(plan.clips.len(), 1);
    assert_eq!(timing(&plan.clips[0]), (0.0, 1.25, 6.75, 8.0));
    assert!(plan.transitions.is_empty());
}

#[test]
fn dip_and_wipe_kinds_carry_through_with_their_clip_pairs() {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        video("a", 0.0, 4.0, 2.0),
        video("b", 4.0, 4.0, 10.0),
        video("c", 8.0, 4.0, 20.0),
        video("d", 12.0, 4.0, 30.0),
    ];
    project.timeline.duration_seconds = 16.0;
    for (id, pair, kind, duration) in [
        ("dip-black", ("a", "b"), TransitionKind::DipToBlack, 1.0),
        ("wipe", ("b", "c"), TransitionKind::Wipe, 2.0),
        ("dip-white", ("c", "d"), TransitionKind::DipToWhite, 0.5),
    ] {
        add_transition(&mut project, "track-video", id, pair, kind, duration);
    }

    let plan = webm_plan(&project);

    assert_eq!(
        plan.transitions,
        vec![
            render_transition(TransitionKind::DipToBlack, 3.5, 1.0, 0, 1),
            render_transition(TransitionKind::Wipe, 7.0, 2.0, 1, 2),
            render_transition(TransitionKind::DipToWhite, 11.75, 0.5, 2, 3),
        ]
    );
    // `b` and `c` sit between two transitions and extend on both sides.
    assert_eq!(timing(&plan.clips[1]), (3.5, 5.5, 9.5, 15.0));
    assert_eq!(handles(&plan.clips[1]), (Some(0.5), Some(1.0)));
    assert_eq!(timing(&plan.clips[2]), (7.0, 5.25, 19.0, 24.25));
    assert_eq!(handles(&plan.clips[2]), (Some(1.0), Some(0.25)));
}

#[test]
fn audio_clip_pairs_produce_audio_transitions() {
    let mut project = crossfade_project();
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![
        clip(
            "music-a",
            TimelineItemKind::AudioClip,
            "music",
            0.0,
            3.0,
            1.0,
            1.0,
        ),
        clip(
            "music-b",
            TimelineItemKind::AudioClip,
            "music",
            3.0,
            3.0,
            10.0,
            1.0,
        ),
    ];
    add_transition(
        &mut project,
        "track-audio",
        "music-fade",
        ("music-a", "music-b"),
        TransitionKind::Crossfade,
        2.0,
    );

    let plan = webm_plan(&project);

    assert_eq!(timing(&plan.audio_clips[0]), (0.0, 4.0, 1.0, 5.0));
    assert_eq!(handles(&plan.audio_clips[0]), (None, Some(1.0)));
    assert_eq!(timing(&plan.audio_clips[1]), (2.0, 4.0, 9.0, 13.0));
    assert_eq!(handles(&plan.audio_clips[1]), (Some(1.0), None));
    assert_eq!(
        plan.audio_transitions,
        vec![render_transition(TransitionKind::Crossfade, 2.0, 2.0, 0, 1)]
    );
    // The video pair's transition stays in `transitions`; video clips carry no
    // embedded audio in the plan, so it has no audio counterpart.
    assert_eq!(
        plan.transitions,
        vec![render_transition(TransitionKind::Crossfade, 3.5, 1.0, 0, 1)]
    );
}

#[test]
fn nested_sequence_transitions_carry_through_expansion_at_playback_speed() {
    let mut project = base_project();
    let mut nested = Timeline::default_editor_timeline();
    nested.duration_seconds = 8.0;
    nested.tracks[0].items = vec![
        video("inner-a", 0.0, 4.0, 2.0),
        video("inner-b", 4.0, 4.0, 6.0),
    ];
    nested.tracks[0].transitions = vec![TimelineTransition {
        id: "inner-fade".to_string(),
        left_item_id: "inner-a".to_string(),
        right_item_id: "inner-b".to_string(),
        kind: TransitionKind::Wipe,
        duration_seconds: 1.0,
    }];
    project.timelines.push(ProjectTimeline {
        id: "nested".to_string(),
        name: "Nested".to_string(),
        timeline: nested,
    });
    project.timeline.tracks[0].items = vec![TimelineItem {
        id: "wrapper".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 2.0,
        duration_seconds: 8.0,
        source: TimelineSource::Timeline {
            timeline_id: "nested".to_string(),
        },
        label: "Nested".to_string(),
        properties: BTreeMap::from([("speed".to_string(), json!(2.0))]),
    }];
    project.timeline.duration_seconds = 6.0;

    let expanded = expand_project_nested_timelines_for_render(&project).expect("expand nested");
    let carried = expanded
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.transitions.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        carried,
        vec![&TimelineTransition {
            id: "root:0:wrapper:inner-fade".to_string(),
            left_item_id: "root:0:wrapper:inner-a".to_string(),
            right_item_id: "root:0:wrapper:inner-b".to_string(),
            kind: TransitionKind::Wipe,
            duration_seconds: 0.5,
        }]
    );

    // Media exports expand once before precompose and again in the plan builder.
    let twice = expand_project_nested_timelines_for_render(&expanded).expect("expand again");
    assert_eq!(
        twice
            .timeline
            .tracks
            .iter()
            .flat_map(|track| track.transitions.iter())
            .collect::<Vec<_>>(),
        carried
    );

    let plan = webm_plan(&project);

    // At 2x the children span 2-4 s and 4-6 s. The 1 s nested wipe lasts
    // 0.5 s of parent time; each 0.25 s extension consumes 0.5 s of source.
    assert_eq!(timing(&plan.clips[0]), (2.0, 2.25, 2.0, 6.5));
    assert_eq!(timing(&plan.clips[1]), (3.75, 2.25, 5.5, 10.0));
    assert_eq!(
        plan.transitions,
        vec![render_transition(TransitionKind::Wipe, 3.75, 0.5, 0, 1)]
    );
    assert_eq!(
        project.timelines.last().expect("nested").timeline.tracks[0].items[0].duration_seconds,
        4.0
    );
}

#[test]
fn image_clips_extend_into_unlimited_handles() {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        clip(
            "still-a",
            TimelineItemKind::ImageClip,
            "still",
            0.0,
            3.0,
            0.0,
            1.0,
        ),
        clip(
            "still-b",
            TimelineItemKind::ImageClip,
            "still",
            3.0,
            3.0,
            0.0,
            1.0,
        ),
    ];
    project.timeline.duration_seconds = 6.0;
    add_transition(
        &mut project,
        "track-video",
        "still-fade",
        ("still-a", "still-b"),
        TransitionKind::DipToBlack,
        2.0,
    );

    let plan = webm_plan(&project);

    assert_eq!(timing(&plan.clips[0]), (0.0, 4.0, 0.0, 4.0));
    assert_eq!(timing(&plan.clips[1]), (2.0, 4.0, 0.0, 4.0));
    assert_eq!(
        plan.transitions,
        vec![render_transition(
            TransitionKind::DipToBlack,
            2.0,
            2.0,
            0,
            1
        )]
    );
}

#[test]
fn stale_transitions_are_clamped_to_remaining_handles_or_skipped() {
    // The right clip now starts 0.5 s into its media, as if its handles shrank
    // behind a 3 s transition, so the render clamps to 1 s.
    let mut project = crossfade_project();
    project.timeline.tracks[0].transitions[0].duration_seconds = 3.0;
    let right = &mut project.timeline.tracks[0].items[1];
    right.properties.insert("sourceIn".to_string(), json!(0.5));
    right.properties.insert("sourceOut".to_string(), json!(4.5));
    let plan = webm_plan(&project);
    assert_eq!(timing(&plan.clips[1]), (3.5, 4.5, 0.0, 4.5));
    assert_eq!(
        plan.transitions,
        vec![render_transition(TransitionKind::Crossfade, 3.5, 1.0, 0, 1)]
    );

    // No handle at all: render the hard cut.
    let right = &mut project.timeline.tracks[0].items[1];
    right.properties.insert("sourceIn".to_string(), json!(0.0));
    right.properties.insert("sourceOut".to_string(), json!(4.0));
    let plan = webm_plan(&project);
    assert_eq!(timing(&plan.clips[1]), (4.0, 4.0, 0.0, 4.0));
    assert!(plan.transitions.is_empty());

    // Clips that no longer meet at the cut.
    let mut project = crossfade_project();
    project.timeline.tracks[0].items[1].start_seconds = 5.0;
    let plan = webm_plan(&project);
    assert_eq!(timing(&plan.clips[0]), (0.0, 4.0, 2.0, 6.0));
    assert!(plan.transitions.is_empty());
}

#[test]
fn transitions_using_the_full_handle_stay_inside_the_source_media() {
    let mut project = base_project();
    project.media[1].duration_seconds = 12.0;
    // At 7x, `left` has 0.8 / 7 s of tail handle and `right` 0.9 / 7 s of head
    // handle. Validation accepts durations up to a micro-second past the
    // maximum, which would read past the media without clamping.
    let accepted_past_max = 2.0 * 0.8 / 7.0 + 5e-7;
    project.timeline.tracks[0].items = vec![
        clip(
            "left",
            TimelineItemKind::VideoClip,
            "media-2",
            0.0,
            1.0,
            4.2,
            7.0,
        ),
        clip(
            "right",
            TimelineItemKind::VideoClip,
            "media-2",
            1.0,
            1.0,
            0.9,
            7.0,
        ),
    ];
    project.timeline.duration_seconds = 2.0;
    add_transition(
        &mut project,
        "track-video",
        "tight",
        ("left", "right"),
        TransitionKind::Crossfade,
        accepted_past_max,
    );

    let plan = webm_plan(&project);
    assert_eq!(timing(&plan.clips[0]), (0.0, 1.114, 4.2, 12.0));
    assert_eq!(timing(&plan.clips[1]), (0.886, 1.114, 0.1, 7.9));
    assert_eq!(
        plan.transitions,
        vec![render_transition(
            TransitionKind::Crossfade,
            0.886,
            0.229,
            0,
            1
        )]
    );

    // Range renders recompute source ends from the extended starts.
    let plan = range_plan(&project, 0.5, 2.0);
    assert_eq!(timing(&plan.clips[0]), (0.0, 0.614, 7.7, 12.0));
    assert_eq!(timing(&plan.clips[1]), (0.386, 1.114, 0.1, 7.9));
    assert_eq!(plan.transitions.len(), 1);
}
