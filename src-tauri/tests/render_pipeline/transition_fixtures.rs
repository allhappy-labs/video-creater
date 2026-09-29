//! Fixtures for render plan transition tests.

use serde_json::json;
use std::collections::BTreeMap;
use video_creater_lib::edit::render_plan::{
    RenderClip, RenderPlan, RenderQualityProfile, RenderTransition,
};
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::project_export::{
    build_project_webm_render_plan, build_project_webm_render_plan_for_range,
};

pub(crate) fn media(
    id: &str,
    kind: MediaKind,
    duration_seconds: f64,
    relative_path: &str,
) -> MediaAsset {
    MediaAsset {
        id: id.to_string(),
        name: None,
        relative_path: relative_path.to_string(),
        kind,
        duration_seconds,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    }
}

/// A media-backed clip consuming `duration * speed` source seconds from `source_in`.
pub(crate) fn clip(
    id: &str,
    kind: TimelineItemKind,
    media_id: &str,
    start_seconds: f64,
    duration_seconds: f64,
    source_in: f64,
    speed: f64,
) -> TimelineItem {
    let mut properties = BTreeMap::from([
        ("sourceIn".to_string(), json!(source_in)),
        (
            "sourceOut".to_string(),
            json!(source_in + duration_seconds * speed),
        ),
    ]);
    if speed != 1.0 {
        properties.insert("speed".to_string(), json!(speed));
    }
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: id.to_string(),
        properties,
    }
}

pub(crate) fn video(id: &str, start: f64, duration: f64, source_in: f64) -> TimelineItem {
    clip(
        id,
        TimelineItemKind::VideoClip,
        "media-2",
        start,
        duration,
        source_in,
        1.0,
    )
}

/// Media, tracks, and clips shared by the transition fixtures. `media-2` is a
/// 60 s video, `music` a 30 s audio file, and `still` an image.
pub(crate) fn base_project() -> VideoProject {
    let mut project = sample_project();
    project.media.extend([
        media("media-2", MediaKind::Video, 60.0, "media/second.mp4"),
        media("music", MediaKind::Audio, 30.0, "media/music.wav"),
        media("still", MediaKind::Image, 0.0, "media/still.png"),
    ]);
    project.timeline.tracks[0].items.clear();
    project
}

pub(crate) const AUDIO_TRACK_INDEX: usize = 4;

pub(crate) fn add_transition(
    project: &mut VideoProject,
    track_id: &str,
    id: &str,
    (left, right): (&str, &str),
    kind: TransitionKind,
    duration_seconds: f64,
) {
    apply_project_action(
        project,
        ProjectAction::AddTransition {
            track_id: track_id.to_string(),
            transition: TimelineTransition {
                id: id.to_string(),
                left_item_id: left.to_string(),
                right_item_id: right.to_string(),
                kind,
                duration_seconds,
            },
        },
    )
    .expect("canonical transition is valid");
}

/// `clip-a` (0-4 s, source 2-6) meets `clip-b` (4-8 s, source 6-10) with a
/// 1 s crossfade.
pub(crate) fn crossfade_project() -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        video("clip-a", 0.0, 4.0, 2.0),
        video("clip-b", 4.0, 4.0, 6.0),
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
    project
}

pub(crate) fn webm_plan(project: &VideoProject) -> RenderPlan {
    let dir = tempfile::tempdir().expect("temp project dir");
    build_project_webm_render_plan(
        dir.path(),
        project,
        "transitions",
        RenderQualityProfile::DraftWebm,
    )
    .expect("render plan")
}

pub(crate) fn range_plan(project: &VideoProject, start: f64, end: f64) -> RenderPlan {
    let dir = tempfile::tempdir().expect("temp project dir");
    build_project_webm_render_plan_for_range(
        dir.path(),
        project,
        "transitions-range",
        RenderQualityProfile::DraftWebm,
        start,
        end,
    )
    .expect("range render plan")
}

/// `(timelineStartSeconds, timelineDurationSeconds, sourceIn, sourceOut)`.
pub(crate) fn timing(clip: &RenderClip) -> (f64, f64, f64, f64) {
    (
        clip.timeline_start_seconds.expect("clip start"),
        clip.properties["timelineDurationSeconds"]
            .as_f64()
            .expect("clip duration"),
        clip.source_in,
        clip.source_out,
    )
}

pub(crate) fn handles(clip: &RenderClip) -> (Option<f64>, Option<f64>) {
    let number = |key: &str| clip.properties.get(key).and_then(serde_json::Value::as_f64);
    (
        number("transitionHeadSeconds"),
        number("transitionTailSeconds"),
    )
}

pub(crate) fn render_transition(
    kind: TransitionKind,
    start_seconds: f64,
    duration_seconds: f64,
    left_clip_index: usize,
    right_clip_index: usize,
) -> RenderTransition {
    RenderTransition {
        kind,
        start_seconds,
        duration_seconds,
        left_clip_index,
        right_clip_index,
    }
}
