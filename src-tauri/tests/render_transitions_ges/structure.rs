//! GES timeline structure, inspected without rendering.

use super::fixtures::*;
use serde_json::json;
use std::path::Path;
use video_creater_lib::edit::render_plan::RenderPlan;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::gstreamer_backend::ges_timeline_summary_for_test;

pub(crate) fn summary(dir: &Path, plan: &RenderPlan) -> String {
    let prefix = dir.canonicalize().expect("canonical dir");
    ges_timeline_summary_for_test(plan, &[], &prefix.display().to_string())
        .expect("GES timeline summary")
}

/// Two video tracks and an audio track exercising opacity, fades, keyframes,
/// geometry, crop, rotation, colour grade and gain, without transitions.
fn project_without_transitions() -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        with_properties(
            clip("plain", TimelineItemKind::VideoClip, "red", 0.0, 1.5, 1.0),
            json!({
                "cropLeft": 0.1,
                "keyframes": {
                    "rotationDegrees": [
                        { "atSeconds": 0.0, "value": 0.0 },
                        { "atSeconds": 1.0, "value": 90.0 }
                    ]
                }
            }),
        ),
        with_properties(
            clip("styled", TimelineItemKind::VideoClip, "blue", 1.5, 2.0, 0.0),
            json!({
                "opacity": 0.8,
                "fadeInSeconds": 0.5,
                "fadeOutSeconds": 0.25,
                "keyframes": {
                    "opacity": [
                        { "atSeconds": 0.0, "value": 0.2 },
                        { "atSeconds": 1.0, "value": 0.9 }
                    ],
                    "positionX": [
                        { "atSeconds": 0.0, "value": 0.0 },
                        { "atSeconds": 2.0, "value": 10.0 }
                    ],
                    "scale": [
                        { "atSeconds": 0.0, "value": 1.0 },
                        { "atSeconds": 2.0, "value": 1.5 }
                    ]
                }
            }),
        ),
    ];
    let mut upper = TimelineTrack::empty("track-video-2", "Video 2", TrackKind::Video);
    upper.items = vec![with_properties(
        clip("overlay", TimelineItemKind::VideoClip, "red", 0.5, 1.0, 0.5),
        json!({
            "transform": { "centerX": 0.5, "centerY": 0.5, "width": 0.5, "height": 0.5 },
            "colorGrade": { "exposure": 0.5 }
        }),
    )];
    project.timeline.tracks.push(upper);
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![with_properties(
        clip("tone", TimelineItemKind::AudioClip, "tone-a", 0.0, 3.0, 0.5),
        json!({ "volumeDb": -6.0 }),
    )];
    project.timeline.duration_seconds = 3.5;
    project
}

#[test]
fn projects_without_transitions_build_the_same_ges_timeline() {
    let dir = project_dir();
    let project = project_without_transitions();
    let full = summary(dir.path(), &plan(dir.path(), &project, "full"));
    let range = summary(
        dir.path(),
        &range_plan(dir.path(), &project, "range", 0.25, 3.5),
    );
    let actual = format!("{full}---\n{range}");
    if let Ok(path) = std::env::var("RENDER_TRANSITIONS_GES_WRITE_SUMMARY") {
        std::fs::write(path, &actual).expect("write summary");
    }

    // Captured from the GES backend before it drew transitions. The only
    // change since is the `linear` interpolation mode: envelopes used to keep
    // GStreamer's default `none`, which steps between control points.
    assert_eq!(
        actual,
        include_str!("timeline_without_transitions.golden.txt")
    );
}

/// The `property` line reported under the first clip whose header contains `clip`.
fn property_line(summary: &str, clip: &str, property: &str) -> String {
    let block = summary
        .split("\n  GES")
        .find(|block| {
            block
                .lines()
                .next()
                .is_some_and(|header| header.contains(clip))
        })
        .unwrap_or_else(|| panic!("no clip `{clip}` in\n{summary}"));
    block
        .lines()
        .find(|line| line.trim_start().starts_with(&format!("{property}=")))
        .unwrap_or_else(|| panic!("no `{property}` under `{clip}` in\n{summary}"))
        .trim()
        .to_string()
}

fn layer_of(summary: &str, clip: &str) -> usize {
    summary
        .split("layer priority=")
        .skip(1)
        .position(|layer| layer.contains(clip))
        .unwrap_or_else(|| panic!("no clip `{clip}` in\n{summary}"))
}

fn assert_points(line: &str, points: &[&str]) {
    for point in points {
        assert!(line.contains(point), "`{point}` missing from `{line}`");
    }
}

const RED: &str = "media/red.webm";
const BLUE: &str = "media/blue.webm";

#[test]
fn crossfades_stack_the_incoming_clip_and_ramp_its_alpha_in_source_time() {
    let dir = project_dir();
    let summary = summary(
        dir.path(),
        &plan(
            dir.path(),
            &two_clip_project(Some(TransitionKind::Crossfade)),
            "crossfade",
        ),
    );
    assert_eq!(summary.matches("layer priority=").count(), 2, "{summary}");
    assert_eq!(layer_of(&summary, BLUE), 0);
    assert_eq!(layer_of(&summary, RED), 1);
    assert!(summary.contains("GESUriClip start=1.500000 inpoint=0.500000 duration=2.500000"));
    // Source envelopes start at the clip's in-point (0.5 s): progress 0.5 at the cut.
    let alpha = property_line(&summary, BLUE, "alpha");
    assert!(alpha.starts_with("alpha=1.0000 binding=absolute/linear[0.500000:0.0000,"));
    assert_points(
        &alpha,
        &["1.000000:0.5000", "1.500000:1.0000", "3.000000:1.0000]"],
    );
    assert_eq!(property_line(&summary, RED, "alpha"), "alpha=1.0000");
    assert!(!summary.contains("GESTestClip") && !summary.contains("GESEffect"));
}

#[test]
fn dips_fade_both_clips_through_a_solid_beneath_them() {
    let dir = project_dir();
    for (kind, pattern) in [
        (TransitionKind::DipToBlack, "Black"),
        (TransitionKind::DipToWhite, "White"),
    ] {
        let summary = summary(
            dir.path(),
            &plan(dir.path(), &two_clip_project(Some(kind)), "dip"),
        );
        assert_eq!(layer_of(&summary, BLUE), 0);
        assert_eq!(layer_of(&summary, RED), 1);
        assert_eq!(layer_of(&summary, "GESTestClip"), 2, "{summary}");
        assert!(summary.contains(&format!(
            "TestClip start=1.500000 inpoint=0.000000 duration=1.000000 vpattern={pattern}"
        )));
        assert_points(
            &property_line(&summary, RED, "alpha"),
            &[
                "[1.000000:1.0000, 2.500000:1.0000,",
                "2.750000:0.5000",
                "3.000000:0.0000",
                "3.500000:0.0000]",
            ],
        );
        assert_points(
            &property_line(&summary, BLUE, "alpha"),
            &[
                "[0.500000:0.0000,",
                "1.000000:0.0000",
                "1.250000:0.5000",
                "1.500000:1.0000",
            ],
        );
    }
}

#[test]
fn wipes_mask_the_incoming_clip_with_a_top_crop_and_a_narrower_box() {
    let dir = project_dir();
    let project = two_clip_project(Some(TransitionKind::Wipe));
    let summary = summary(dir.path(), &plan(dir.path(), &project, "wipe"));
    assert_eq!(layer_of(&summary, BLUE), 0);
    assert_eq!(property_line(&summary, BLUE, "height"), "height=72");
    // Frame positioner width in source time, crop in clip-local time.
    assert_points(
        &property_line(&summary, BLUE, "width"),
        &[
            "[0.500000:1.0000,",
            "0.750000:32.0000",
            "1.000000:64.0000",
            "1.500000:128.0000",
        ],
    );
    assert!(
        summary.contains("effect GESEffect track=TrackType(VIDEO) bin=Some(\"videocrop right=0\")")
    );
    assert_points(
        &property_line(&summary, BLUE, "right"),
        &[
            "[0.000000:127.0000,",
            "0.250000:96.0000",
            "0.500000:64.0000",
            "1.000000:0.0000",
        ],
    );
    assert_points(
        &property_line(&summary, BLUE, "alpha"),
        &["[0.500000:0.0000, 0.507812:1.0000,"],
    );
    assert!(!property_line(&summary, RED, "width").contains("binding"));

    let range = summary_for_range(dir.path(), &project);
    assert_points(
        &property_line(&range, BLUE, "right"),
        &["[0.000000:64.0000,", "0.500000:0.0000"],
    );
    assert_points(
        &property_line(&range, BLUE, "width"),
        &["[1.000000:64.0000,", "1.500000:128.0000"],
    );
}

fn summary_for_range(dir: &Path, project: &VideoProject) -> String {
    let range = range_plan(dir, project, "wipe-range", 2.0, 3.0);
    assert_eq!(range.transitions[0].start_seconds, -0.5);
    summary(dir, &range)
}

#[test]
fn audio_transitions_and_fades_bind_linear_volume_envelopes() {
    let dir = project_dir();
    let summary = summary(
        dir.path(),
        &plan(dir.path(), &audio_crossfade_project(), "audio"),
    );
    let outgoing = property_line(&summary, "tone-a.webm", "volume");
    assert_points(
        &outgoing,
        &["[1.000000:1.0000, 2.500000:1.0000,", "3.000000:0.7071"],
    );
    // GES re-interpolates the out-point, which can print a negative zero.
    assert!(outgoing.ends_with(" 3.500000:0.0000]") || outgoing.ends_with(" 3.500000:-0.0000]"));
    assert_points(
        &property_line(&summary, "tone-b.webm", "volume"),
        &[
            "[0.500000:0.0000,",
            "1.000000:0.7071",
            "1.500000:1.0000",
            "3.000000:1.0000]",
        ],
    );

    let fade = summary_of_fade(dir.path());
    assert_eq!(
        property_line(&fade, "tone-a.webm", "volume"),
        "volume=1.0000 binding=absolute/linear[1.000000:0.0000, 1.500000:1.0000, 3.000000:1.0000]"
    );
}

fn summary_of_fade(dir: &Path) -> String {
    let mut project = audio_fade_project();
    let tone = &mut project.timeline.tracks[AUDIO_TRACK_INDEX].items[0];
    tone.properties.insert("volumeDb".to_string(), json!(0.0));
    summary(dir, &plan(dir, &project, "fade"))
}
