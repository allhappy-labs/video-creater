use super::*;
use crate::edit::render_plan::{RenderOutputProfile, RenderQuality};
use serde_json::json;
use std::collections::BTreeMap;

fn render_clip(start: f64, duration: f64, head: f64, tail: f64) -> RenderClip {
    let mut properties = BTreeMap::from([("timelineDurationSeconds".to_string(), json!(duration))]);
    if head > 0.0 {
        properties.insert(TRANSITION_HEAD_SECONDS_PROPERTY.to_string(), json!(head));
    }
    if tail > 0.0 {
        properties.insert(TRANSITION_TAIL_SECONDS_PROPERTY.to_string(), json!(tail));
    }
    RenderClip {
        source_path: None,
        timeline_start_seconds: Some(start),
        properties,
        timeline_track_index: 0,
        source_in: 1.0,
        source_out: 1.0 + duration,
    }
}

fn transition(kind: TransitionKind, start: f64, left: usize, right: usize) -> RenderTransition {
    RenderTransition {
        kind,
        start_seconds: start,
        duration_seconds: 1.0,
        left_clip_index: left,
        right_clip_index: right,
    }
}

/// Red 0-2 s and blue 2-4 s extended into a 1 s window `[1.5, 2.5]`.
fn plan(kind: TransitionKind) -> RenderPlan {
    RenderPlan {
        input_path: "red.webm".to_string(),
        output_path: "out.webm".to_string(),
        width: 128,
        height: 72,
        fps: 24.0,
        quality: RenderQuality::Final,
        output_profile: RenderOutputProfile::WebPreview,
        encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
        clips: vec![
            render_clip(0.0, 2.5, 0.0, 0.5),
            render_clip(1.5, 2.5, 0.5, 0.0),
        ],
        audio_clips: Vec::new(),
        transitions: vec![transition(kind, 1.5, 0, 1)],
        audio_transitions: Vec::new(),
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-6
}

#[test]
fn incoming_clips_stack_above_outgoing_clips_and_dips_add_solids() {
    let layout = GesTransitionLayout::new(&plan(TransitionKind::DipToWhite)).expect("layout");
    assert_eq!(layout.clips[0].depth, 0);
    assert_eq!(layout.clips[1].depth, 1);
    assert_eq!(
        layout.max_video_depth(&plan(TransitionKind::Crossfade), 0),
        1
    );
    assert_eq!(
        layout.solids,
        vec![GesTransitionSolid {
            track_index: 0,
            start_seconds: 1.5,
            duration_seconds: 1.0,
            white: true,
        }]
    );
    assert!(GesTransitionLayout::new(&plan(TransitionKind::Crossfade))
        .expect("layout")
        .solids
        .is_empty());

    let mut chain = plan(TransitionKind::Crossfade);
    chain.clips[1] = render_clip(1.5, 3.0, 0.5, 0.5);
    chain.clips.push(render_clip(3.5, 2.5, 0.5, 0.0));
    chain
        .transitions
        .insert(0, transition(TransitionKind::Wipe, 3.5, 1, 2));
    let layout = GesTransitionLayout::new(&chain).expect("chained layout");
    assert_eq!(
        layout
            .clips
            .iter()
            .map(|clip| clip.depth)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn projects_without_transitions_keep_a_flat_layout() {
    let mut flat = plan(TransitionKind::Crossfade);
    flat.transitions.clear();
    flat.clips = vec![
        render_clip(0.0, 2.0, 0.0, 0.0),
        render_clip(2.0, 2.0, 0.0, 0.0),
    ];
    let layout = GesTransitionLayout::new(&flat).expect("layout");
    assert!(layout.solids.is_empty());
    assert!(layout
        .clips
        .iter()
        .all(|clip| clip.depth == 0 && !clip.is_transitioned()));
}

#[test]
fn invalid_render_transitions_fail_before_ges_mutation() {
    for (left, right) in [(0, 0), (0, 5)] {
        let mut invalid = plan(TransitionKind::Crossfade);
        invalid.transitions[0].left_clip_index = left;
        invalid.transitions[0].right_clip_index = right;
        let error = GesTransitionLayout::new(&invalid).expect_err("invalid indices");
        assert_eq!(error[0].path, "renderPlan.transitions[0]");
    }
    let mut other_track = plan(TransitionKind::Crossfade);
    other_track.clips[1].timeline_track_index = 1;
    assert!(GesTransitionLayout::new(&other_track).is_err());
    let mut zero = plan(TransitionKind::Crossfade);
    zero.transitions[0].duration_seconds = 0.0;
    assert!(GesTransitionLayout::new(&zero).is_err());
}

#[test]
fn transition_factors_match_the_preview_at_window_positions() {
    let factors = |kind| {
        let layout = GesTransitionLayout::new(&plan(kind)).expect("layout");
        // Output time 2.0 s is the midpoint; 1.75 s is a quarter of the way in.
        let (outgoing, incoming) = (layout.clips[0], layout.clips[1]);
        [1.75, 2.0].map(|output| {
            (
                outgoing.visual_alpha_factor(output - outgoing.clip_start_seconds),
                incoming.visual_alpha_factor(output - incoming.clip_start_seconds),
            )
        })
    };
    let [quarter, midpoint] = factors(TransitionKind::Crossfade);
    assert!(close(quarter.0, 1.0) && close(quarter.1, 0.25));
    assert!(close(midpoint.0, 1.0) && close(midpoint.1, 0.5));
    for kind in [TransitionKind::DipToBlack, TransitionKind::DipToWhite] {
        let [quarter, midpoint] = factors(kind);
        assert!(close(quarter.0, 0.5) && close(quarter.1, 0.0));
        assert!(close(midpoint.0, 0.0) && close(midpoint.1, 0.0));
    }
    let [quarter, midpoint] = factors(TransitionKind::Wipe);
    assert!(close(quarter.0, 1.0) && close(quarter.1, 1.0));
    assert!(close(midpoint.0, 1.0) && close(midpoint.1, 1.0));

    let wipe = GesTransitionLayout::new(&plan(TransitionKind::Wipe)).expect("layout");
    assert_eq!(wipe.clips[1].wipe_reveal_pixels(0.25), Some(32.0));
    assert_eq!(wipe.clips[1].visual_alpha_factor(0.0), 0.0);
    assert_eq!(wipe.clips[0].wipe_reveal_pixels(1.0), None);
}

#[test]
fn range_renders_keep_partial_progress_when_the_window_starts_before_the_output() {
    let mut range = plan(TransitionKind::Crossfade);
    // A range starting at the cut: the window is [-0.5, 0.5].
    range.clips = vec![
        render_clip(0.0, 0.5, 0.0, 0.5),
        render_clip(0.0, 2.0, 0.0, 0.0),
    ];
    range.clips[1]
        .properties
        .insert(TRANSITION_HEAD_SECONDS_PROPERTY.to_string(), json!(0.0));
    range.transitions[0].start_seconds = -0.5;
    let layout = GesTransitionLayout::new(&range).expect("layout");
    let incoming = layout.clips[1];
    assert!(close(incoming.visual_alpha_factor(0.0), 0.5));
    assert!(close(incoming.visual_alpha_factor(0.25), 0.75));
    let points = transition_alpha_control_points(1.0, 2.0, None, &[], &incoming, 24.0);
    assert!(close(points[0].value, 0.5), "{points:?}");
    assert!(points
        .iter()
        .any(|point| close(point.seconds, 0.5) && close(point.value, 1.0)));
    assert_eq!(
        layout.solids,
        Vec::<GesTransitionSolid>::new(),
        "crossfades draw no solid"
    );
}

#[test]
fn alpha_envelopes_clamp_fades_to_the_canonical_clip_and_sample_every_frame() {
    let layout = GesTransitionLayout::new(&plan(TransitionKind::Crossfade)).expect("layout");
    let incoming = layout.clips[1];
    let fade = GesVisualFadeEnvelope {
        opacity: 1.0,
        duration_seconds: 2.0,
        fade_in_seconds: 0.5,
        fade_out_seconds: 0.0,
    };
    let points = transition_alpha_control_points(1.0, 2.5, Some(fade), &[], &incoming, 24.0);
    // The handle and the canonical start hold the fade-in at zero.
    for point in points.iter().filter(|point| point.seconds <= 0.5 + 1e-9) {
        assert!(close(point.value, 0.0), "{point:?}");
    }
    // Midway through the fade (canonical 0.25 s) at transition progress 0.75.
    let quarter = points
        .iter()
        .find(|point| close(point.seconds, 0.75))
        .expect("frame sample");
    assert!(close(quarter.value, 0.5 * 0.75), "{quarter:?}");
    assert!(close(points.last().expect("end").value, 1.0));
    let frame_samples = points
        .iter()
        .filter(|point| (0.0..=1.0).contains(&point.seconds))
        .count();
    assert!(frame_samples >= 25, "{points:?}");
}

#[test]
fn audio_gain_envelopes_are_equal_power_and_end_at_the_clip_end() {
    let mut audio = plan(TransitionKind::Crossfade);
    audio.audio_clips = std::mem::take(&mut audio.clips);
    audio.audio_transitions = std::mem::take(&mut audio.transitions);
    let layout = GesTransitionLayout::new(&audio).expect("layout");
    let (outgoing, incoming) = (layout.audio_clips[0], layout.audio_clips[1]);
    let midpoint_power =
        outgoing.audio_gain_factor(2.0).powi(2) + incoming.audio_gain_factor(0.5).powi(2);
    assert!(close(midpoint_power, 1.0));
    assert!(close(
        outgoing.audio_gain_factor(2.0),
        std::f64::consts::FRAC_1_SQRT_2
    ));

    let points = audio_gain_control_points(0.5, 2.5, None, &incoming);
    assert!(close(points[0].value, 0.0));
    let end = points[points.len() - 1];
    assert!(close(end.seconds, 2.5) && close(end.value, 0.5));
    assert!(points.len() >= AUDIO_CURVE_SEGMENTS + 2);

    let fade = GesVisualFadeEnvelope {
        opacity: 1.0,
        duration_seconds: 2.0,
        fade_in_seconds: 0.5,
        fade_out_seconds: 0.0,
    };
    let plain = GesClipTransitions {
        duration_seconds: 2.0,
        ..GesClipTransitions::default()
    };
    let faded = audio_gain_control_points(1.0, 2.0, Some(fade), &plain);
    assert_eq!(
        faded
            .iter()
            .map(|point| (point.seconds, point.value))
            .collect::<Vec<_>>(),
        vec![(0.0, 0.0), (0.5, 1.0), (2.0, 1.0)]
    );
}

#[test]
fn wipe_masks_crop_and_narrow_the_incoming_clip_in_canvas_space() {
    let layout = GesTransitionLayout::new(&plan(TransitionKind::Wipe)).expect("layout");
    let incoming = layout.clips[1];
    let full = |_: f64| 128.0;
    let left = |_: f64| 0.0;
    let mask = WipeMask {
        transitions: &incoming,
        box_left: &left,
        box_width: &full,
        frame_width: &full,
    };
    let (widths, crops) = mask.points(incoming.visual_event_seconds(24.0));
    let at = |points: &[GesTimedControlPoint], seconds: f64| {
        points
            .iter()
            .find(|point| close(point.seconds, seconds))
            .map(|point| point.value)
    };
    assert_eq!(at(&widths, 0.5), Some(64.0));
    assert_eq!(at(&crops, 0.5), Some(64.0));
    assert_eq!(at(&widths, 0.0), Some(1.0));
    assert_eq!(at(&crops, 0.0), Some(127.0));
    assert_eq!(at(&widths, 2.5), Some(128.0));
    assert_eq!(at(&crops, 2.5), Some(0.0));

    // A half-width box centred on the canvas is revealed from its own left edge.
    let offset = |_: f64| 32.0;
    let half = |_: f64| 64.0;
    let boxed = WipeMask {
        transitions: &incoming,
        box_left: &offset,
        box_width: &half,
        frame_width: &full,
    };
    let (widths, crops) = boxed.points(incoming.visual_event_seconds(24.0));
    assert_eq!(at(&widths, 0.5), Some(32.0));
    assert_eq!(at(&crops, 0.5), Some(64.0));
    assert_eq!(held_control_value(&widths, 10.0, 0.0), 64.0);
}
