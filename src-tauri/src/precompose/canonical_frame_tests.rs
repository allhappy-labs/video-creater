//! Canonical sampling of clip transitions, at the timestamps and with the
//! expectations of the editor preview tests (`src/lib/timeline-preview.test.ts`).

use super::*;
use crate::frame_compositor::TransitionRole;
use crate::project::fixtures::sample_project;
use crate::project::model::{TimelineItem, TimelineItemKind, TimelineTransition};
use serde_json::{json, Value};
use std::collections::BTreeMap;

// Two 4 s cuts of the 12 s clip meeting at 4 s: "left" uses source 2-6 s and
// "right" uses source 5-9 s. A 1 s transition spans [3.5, 4.5).
fn cut_clip(id: &str, start: f64, source_in: f64, properties: Value) -> TimelineItem {
    let mut item_properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(source_in)),
        ("sourceOut".to_string(), json!(source_in + 4.0)),
    ]);
    for (key, value) in properties.as_object().expect("properties object") {
        item_properties.insert(key.clone(), value.clone());
    }
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: start,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: id.to_string(),
        properties: item_properties,
    }
}

fn cut_project(kind: TransitionKind, items: Vec<TimelineItem>) -> VideoProject {
    let mut project = sample_project();
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 8.0;
    let track = &mut project.timeline.tracks[0];
    track.items = items;
    track.transitions = vec![TimelineTransition {
        id: "cut".to_string(),
        left_item_id: "left".to_string(),
        right_item_id: "right".to_string(),
        kind,
        duration_seconds: 1.0,
    }];
    project
}

fn default_cut(kind: TransitionKind) -> VideoProject {
    cut_project(
        kind,
        vec![
            cut_clip("left", 0.0, 2.0, json!({})),
            cut_clip("right", 4.0, 5.0, json!({})),
        ],
    )
}

fn frame_at(project: &VideoProject, seconds: f64) -> CanonicalFrameSample {
    sample_canonical_frame(project, seconds).expect("canonical sample")
}

fn layer<'a>(frame: &'a CanonicalFrameSample, item_id: &str) -> &'a CanonicalLayerSample {
    frame
        .layers
        .iter()
        .find(|layer| layer.item_id == item_id)
        .unwrap_or_else(|| panic!("expected layer {item_id}"))
}

fn ids(frame: &CanonicalFrameSample) -> Vec<&str> {
    frame
        .layers
        .iter()
        .map(|layer| layer.item_id.as_str())
        .collect()
}

fn opacities(frame: &CanonicalFrameSample) -> Vec<f64> {
    frame.layers.iter().map(|layer| layer.opacity).collect()
}

#[test]
fn draws_both_clips_at_the_midpoint_outgoing_beneath_incoming_sampling_handles() {
    // The right clip listed first still draws on top.
    let project = cut_project(
        TransitionKind::Crossfade,
        vec![
            cut_clip("right", 4.0, 5.0, json!({})),
            cut_clip("left", 0.0, 2.0, json!({})),
        ],
    );
    let midpoint = frame_at(&project, 4.0);
    assert_eq!(ids(&midpoint), ["left", "right"]);
    assert_eq!(layer(&midpoint, "left").source_seconds, 6.0);
    assert_eq!(layer(&midpoint, "right").source_seconds, 5.0);
    let left = layer(&midpoint, "left")
        .transition
        .expect("outgoing transition");
    assert_eq!(
        (left.kind, left.role, left.progress),
        (TransitionKind::Crossfade, TransitionRole::Outgoing, 0.5)
    );
    assert!(midpoint.solids.is_empty());

    let project = default_cut(TransitionKind::Crossfade);
    // Past the left clip's canonical end: sourceOut + (t - leftEnd) * speed.
    assert_eq!(
        layer(&frame_at(&project, 4.25), "left").source_seconds,
        6.25
    );
    // Before the right clip's canonical start: sourceIn - (rightStart - t) * speed.
    assert_eq!(
        layer(&frame_at(&project, 3.75), "right").source_seconds,
        4.75
    );
}

#[test]
fn crossfades_with_the_incoming_clip_at_opacity_p_over_the_outgoing_clip() {
    let project = default_cut(TransitionKind::Crossfade);
    let midpoint = frame_at(&project, 4.0);
    assert_eq!(layer(&midpoint, "left").opacity, 1.0);
    let right = layer(&midpoint, "right");
    assert_eq!(right.opacity, 0.5);
    let incoming = right.transition.expect("incoming transition");
    assert_eq!(
        (incoming.role, incoming.progress, incoming.opacity),
        (TransitionRole::Incoming, 0.5, 0.5)
    );
    assert_eq!(layer(&frame_at(&project, 3.75), "right").opacity, 0.25);
    assert_eq!(layer(&frame_at(&project, 4.25), "left").opacity, 1.0);
}

#[test]
fn dips_through_a_solid_with_both_clips_transparent_at_the_cut() {
    let project = default_cut(TransitionKind::DipToBlack);
    let cut = frame_at(&project, 4.0);
    assert_eq!(ids(&cut), ["left", "right"]);
    assert_eq!(opacities(&cut), [0.0, 0.0]);
    assert_eq!(
        cut.solids,
        [CanonicalSolidSample {
            track_index: 0,
            kind: TransitionKind::DipToBlack,
            progress: 0.5,
            rgba: [0, 0, 0, 255],
        }]
    );
    assert_eq!(opacities(&frame_at(&project, 3.75)), [0.5, 0.0]);
    assert_eq!(opacities(&frame_at(&project, 4.25)), [0.0, 0.5]);
    let start = frame_at(&project, 3.5);
    assert_eq!(start.solids.len(), 1);
    assert_eq!(start.solids[0].progress, 0.0);
    let white = frame_at(&default_cut(TransitionKind::DipToWhite), 4.0);
    assert_eq!(
        (white.solids[0].kind, white.solids[0].rgba),
        (TransitionKind::DipToWhite, [255, 255, 255, 255])
    );
}

#[test]
fn wipes_the_incoming_clip_in_from_the_left() {
    let project = default_cut(TransitionKind::Wipe);
    let midpoint = frame_at(&project, 4.0);
    let left = layer(&midpoint, "left");
    assert_eq!(left.opacity, 1.0);
    assert_eq!(
        left.transition
            .map(|state| (state.role, state.wipe_inset_right)),
        Some((TransitionRole::Outgoing, None))
    );
    let right = layer(&midpoint, "right");
    assert_eq!(right.opacity, 1.0);
    assert_eq!(
        right
            .transition
            .map(|state| (state.role, state.wipe_inset_right)),
        Some((TransitionRole::Incoming, Some(0.5)))
    );
    assert_eq!(
        layer(&frame_at(&project, 3.75), "right")
            .transition
            .and_then(|state| state.wipe_inset_right),
        Some(0.75)
    );
}

#[test]
fn draws_only_one_clip_outside_the_window_with_exact_edges() {
    let project = default_cut(TransitionKind::Crossfade);
    for (seconds, item_id) in [(3.4, "left"), (4.6, "right"), (4.5, "right")] {
        let frame = frame_at(&project, seconds);
        assert_eq!(ids(&frame), [item_id], "at {seconds}");
        assert_eq!(frame.layers[0].transition, None);
        assert_eq!(frame.layers[0].opacity, 1.0);
        assert!(frame.solids.is_empty());
    }
    let start = frame_at(&project, 3.5);
    assert_eq!(ids(&start), ["left", "right"]);
    assert_eq!(opacities(&start), [1.0, 0.0]);
    assert_eq!(
        layer(&start, "right")
            .transition
            .map(|state| state.progress),
        Some(0.0)
    );
    assert_eq!(layer(&frame_at(&project, 4.5), "right").source_seconds, 5.5);
}

#[test]
fn multiplies_transition_opacity_with_clip_opacity_keyframes_and_fades() {
    let left = cut_clip(
        "left",
        0.0,
        2.0,
        json!({ "keyframes": { "opacity": [
            { "atSeconds": 0.0, "value": 1.0 },
            { "atSeconds": 4.0, "value": 0.6 }
        ] } }),
    );
    let right = cut_clip(
        "right",
        4.0,
        5.0,
        json!({ "opacity": 0.8, "fadeInSeconds": 1.0 }),
    );
    let crossfade = cut_project(TransitionKind::Crossfade, vec![left.clone(), right]);
    // Keyframes hold their last value in the tail handle; the fade-in reads 0 before the canonical start.
    assert!((layer(&frame_at(&crossfade, 4.25), "left").opacity - 0.6).abs() < 1e-12);
    assert_eq!(layer(&frame_at(&crossfade, 3.75), "right").opacity, 0.0);
    assert!(
        (layer(&frame_at(&crossfade, 4.25), "right").opacity - 0.8 * 0.25 * 0.75).abs() < 1e-10
    );
    let dip = cut_project(
        TransitionKind::DipToBlack,
        vec![left, cut_clip("right", 4.0, 5.0, json!({ "opacity": 0.8 }))],
    );
    // Keyframed 0.625 at 3.75 s, halved by the dip.
    assert!((layer(&frame_at(&dip, 3.75), "left").opacity - 0.625 * 0.5).abs() < 1e-10);
    assert!((layer(&frame_at(&dip, 4.25), "right").opacity - 0.8 * 0.5).abs() < 1e-10);
}

#[test]
fn clamps_the_window_to_the_available_handles_and_skips_transitions_that_cannot_draw() {
    // 0.2 s of media before the right clip allows 0.4 s.
    let short = cut_project(
        TransitionKind::Crossfade,
        vec![
            cut_clip("left", 0.0, 2.0, json!({})),
            cut_clip("right", 4.0, 0.2, json!({})),
        ],
    );
    assert_eq!(ids(&frame_at(&short, 3.75)), ["left"]);
    assert_eq!(ids(&frame_at(&short, 3.85)), ["left", "right"]);
    let apart = cut_project(
        TransitionKind::Crossfade,
        vec![
            cut_clip("left", 0.0, 2.0, json!({})),
            cut_clip("right", 5.0, 5.0, json!({})),
        ],
    );
    assert!(frame_at(&apart, 4.25).layers.is_empty());
}

#[test]
fn samples_projects_without_transitions_identically() {
    let mut plain = default_cut(TransitionKind::Crossfade);
    plain.timeline.tracks[0].transitions.clear();
    for seconds in [0.0, 1.0, 3.5, 3.99, 4.0, 4.5, 6.5, 7.99] {
        let frame = frame_at(&plain, seconds);
        let (item_id, source_seconds) = if seconds < 4.0 {
            ("left", 2.0 + seconds)
        } else {
            ("right", 1.0 + seconds)
        };
        assert_eq!(
            frame,
            CanonicalFrameSample {
                layers: vec![CanonicalLayerSample {
                    track_index: 0,
                    item_id: item_id.to_string(),
                    source_seconds,
                    opacity: 1.0,
                    transition: None,
                }],
                solids: Vec::new(),
            },
            "at {seconds}"
        );
    }
    assert!(frame_at(&plain, 8.0).layers.is_empty());
}
