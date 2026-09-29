//! Reversed clips in render planning: mirrored range and handle mapping, and
//! fail-closed rendering until reversed media is prepared.

use super::transition_fixtures::*;
use serde_json::json;
use video_creater_lib::precompose::prepare_project_for_render;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::avfoundation_backend::AvFoundationRenderBackend;
use video_creater_lib::render_pipeline::backend::RenderBackend;
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;

fn reversed(mut item: TimelineItem) -> TimelineItem {
    item.properties.insert("reverse".to_string(), json!(true));
    item
}

/// `rev`: 10-14 s on the timeline, source 2-10 at speed 2, reversed, on the
/// video track and (as `rev-audio`) on the audio track.
fn reversed_video_and_audio_project() -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![reversed(clip(
        "rev",
        TimelineItemKind::VideoClip,
        "media-2",
        10.0,
        4.0,
        2.0,
        2.0,
    ))];
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![reversed(clip(
        "rev-audio",
        TimelineItemKind::AudioClip,
        "music",
        10.0,
        4.0,
        2.0,
        2.0,
    ))];
    project.timeline.duration_seconds = 14.0;
    project
}

#[test]
fn range_renders_map_reversed_clips_to_the_mirrored_source_window() {
    let project = reversed_video_and_audio_project();

    // Timeline 11-12.5 s is clip-local 1-2.5 s: source 10 - 2.5 * 2 to 10 - 1 * 2.
    let plan = range_plan(&project, 11.0, 12.5);

    assert_eq!(plan.clips.len(), 1);
    assert_eq!(timing(&plan.clips[0]), (0.0, 1.5, 5.0, 8.0));
    assert_eq!(plan.clips[0].properties.get("reverse"), Some(&json!(true)));
    assert_eq!(plan.audio_clips.len(), 1);
    assert_eq!(timing(&plan.audio_clips[0]), (0.0, 1.5, 5.0, 8.0));
    assert_eq!(
        plan.audio_clips[0].properties.get("reverse"),
        Some(&json!(true))
    );
}

#[test]
fn transition_handles_extend_reversed_clips_through_the_mirrored_source_edges() {
    let mut project = base_project();
    // clip-a (0-4 s) plays source 6 down to 2; clip-b (4-8 s) plays 10 down to 6.
    project.timeline.tracks[0].items = vec![
        reversed(video("clip-a", 0.0, 4.0, 2.0)),
        reversed(video("clip-b", 4.0, 4.0, 6.0)),
    ];
    project.timeline.duration_seconds = 8.0;
    add_transition(
        &mut project,
        "track-video",
        "fade",
        ("clip-a", "clip-b"),
        TransitionKind::Crossfade,
        1.0,
    );

    let plan = webm_plan(&project);

    // The left clip's tail reads below sourceIn; the right clip's head reads
    // above sourceOut.
    assert_eq!(timing(&plan.clips[0]), (0.0, 4.5, 1.5, 6.0));
    assert_eq!(handles(&plan.clips[0]), (None, Some(0.5)));
    assert_eq!(timing(&plan.clips[1]), (3.5, 4.5, 6.0, 10.5));
    assert_eq!(handles(&plan.clips[1]), (Some(0.5), None));
}

#[test]
fn render_backends_reject_reversed_clips_that_were_not_prepared() {
    let project = reversed_video_and_audio_project();
    let plan = range_plan(&project, 10.0, 14.0);

    let error = GstreamerGesRenderBackend::new()
        .build_command(&plan, &[])
        .expect_err("GES fails closed on reversed clips");
    assert_eq!(error[0].path, "renderPlan.clips[0].properties.reverse");
    assert!(
        error[0].message.contains("Reversed clips"),
        "{}",
        error[0].message
    );

    let mut audio_only = plan.clone();
    audio_only.clips[0].properties.remove("reverse");
    let error = GstreamerGesRenderBackend::new()
        .build_command(&audio_only, &[])
        .expect_err("GES fails closed on reversed audio clips");
    assert_eq!(error[0].path, "renderPlan.audioClips[0].properties.reverse");

    let error = AvFoundationRenderBackend::new()
        .build_request("job-reverse", &plan, &[])
        .expect_err("AVFoundation fails closed on reversed clips");
    assert_eq!(error[0].path, "renderPlan.clips[0].properties.reverse");
}

#[test]
fn render_preparation_rejects_reversed_clips_it_cannot_reverse() {
    let mut project = base_project();
    // Only video clips of video media and audio clips play in reverse.
    project.timeline.tracks[0].items = vec![reversed(clip(
        "rev-still",
        TimelineItemKind::ImageClip,
        "still",
        0.0,
        2.0,
        0.0,
        1.0,
    ))];
    project.timeline.duration_seconds = 2.0;
    let dir = tempfile::tempdir().expect("temp project dir");

    let error = prepare_project_for_render(dir.path(), &project)
        .expect_err("a reversed still is not prepared");

    assert_eq!(
        error[0].path,
        "timeline.tracks[0].items[0].properties.reverse"
    );
    assert!(
        error[0].message.contains("Reversed clip"),
        "{}",
        error[0].message
    );
    assert_eq!(
        error[0].details.get("itemId"),
        Some(&"rev-still".to_string())
    );
}
