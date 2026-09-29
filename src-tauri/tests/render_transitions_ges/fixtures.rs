//! Projects for the GES transition renders: two 2 s clips with 1 s handles.

use super::media::{write_solid_video, write_tone, BLUE, FPS, HEIGHT, RED, WIDTH};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;
use video_creater_lib::edit::render_plan::{RenderPlan, RenderQualityProfile};
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::gstreamer_backend::GstreamerGesRenderBackend;
use video_creater_lib::render_pipeline::process::SystemProcessRunner;
use video_creater_lib::render_pipeline::project_export::{
    build_project_webm_render_plan, build_project_webm_render_plan_for_range,
};
use video_creater_lib::render_runtime::start_render_process_runtime;

pub(crate) const AUDIO_TRACK_INDEX: usize = 4;
const MEDIA_SECONDS: f64 = 4.0;

pub(crate) fn start_runtime() {
    static START: Once = Once::new();
    START.call_once(|| {
        start_render_process_runtime().expect(
            "GES render runtime starts (set VIDEO_CREATER_RENDER_RUNTIME_ROOT to a staged runtime)",
        );
    });
}

/// A temporary project folder with red and blue videos and two tones.
pub(crate) fn project_dir() -> tempfile::TempDir {
    start_runtime();
    let dir = tempfile::tempdir().expect("temporary project");
    let media = dir.path().join("media");
    std::fs::create_dir_all(&media).expect("media directory");
    write_solid_video(&media.join("red.webm"), RED, MEDIA_SECONDS);
    write_solid_video(&media.join("blue.webm"), BLUE, MEDIA_SECONDS);
    write_tone(&media.join("tone-a.webm"), 440.0, MEDIA_SECONDS);
    write_tone(&media.join("tone-b.webm"), 660.0, MEDIA_SECONDS);
    dir
}

fn media(id: &str, kind: MediaKind, file: &str) -> MediaAsset {
    let visual = kind == MediaKind::Video;
    MediaAsset {
        id: id.to_string(),
        name: Some(id.to_string()),
        relative_path: format!("media/{file}"),
        kind,
        duration_seconds: MEDIA_SECONDS,
        width: visual.then_some(WIDTH as u32),
        height: visual.then_some(HEIGHT as u32),
        fps: visual.then_some(FPS),
        folder_id: None,
    }
}

/// A clip at `start` for `duration` timeline seconds reading from `source_in`.
pub(crate) fn clip(
    id: &str,
    kind: TimelineItemKind,
    media_id: &str,
    start: f64,
    duration: f64,
    source_in: f64,
) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: id.to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(source_in)),
            ("sourceOut".to_string(), json!(source_in + duration)),
        ]),
    }
}

pub(crate) fn with_properties(mut item: TimelineItem, properties: Value) -> TimelineItem {
    for (key, value) in properties.as_object().expect("property object") {
        item.properties.insert(key.clone(), value.clone());
    }
    item
}

/// Red, blue and tone media with every timeline track empty.
pub(crate) fn base_project() -> VideoProject {
    let mut project = sample_project();
    project.render_settings.width = WIDTH as u32;
    project.render_settings.height = HEIGHT as u32;
    project.render_settings.fps = FPS;
    project.media = vec![
        media("red", MediaKind::Video, "red.webm"),
        media("blue", MediaKind::Video, "blue.webm"),
        media("tone-a", MediaKind::Audio, "tone-a.webm"),
        media("tone-b", MediaKind::Audio, "tone-b.webm"),
    ];
    for track in &mut project.timeline.tracks {
        track.items.clear();
    }
    project.timelines.clear();
    project.active_timeline_id = None;
    project
}

pub(crate) fn add_transition(
    project: &mut VideoProject,
    track_id: &str,
    (left, right): (&str, &str),
    kind: TransitionKind,
    duration_seconds: f64,
) {
    apply_project_action(
        project,
        ProjectAction::AddTransition {
            track_id: track_id.to_string(),
            transition: TimelineTransition {
                id: format!("{left}-{right}"),
                left_item_id: left.to_string(),
                right_item_id: right.to_string(),
                kind,
                duration_seconds,
            },
        },
    )
    .expect("transition is valid");
}

/// Red (0-2 s, source 1-3) cuts to blue (2-4 s, source 1-3) with a 1 s
/// transition window `[1.5, 2.5]`.
pub(crate) fn two_clip_project(kind: Option<TransitionKind>) -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![
        clip(
            "red-clip",
            TimelineItemKind::VideoClip,
            "red",
            0.0,
            2.0,
            1.0,
        ),
        clip(
            "blue-clip",
            TimelineItemKind::VideoClip,
            "blue",
            2.0,
            2.0,
            1.0,
        ),
    ];
    project.timeline.duration_seconds = 4.0;
    if let Some(kind) = kind {
        add_transition(
            &mut project,
            "track-video",
            ("red-clip", "blue-clip"),
            kind,
            1.0,
        );
    }
    project
}

/// Tone A (0-2 s) crossfades into tone B (2-4 s) over red video.
pub(crate) fn audio_crossfade_project() -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![clip(
        "red-clip",
        TimelineItemKind::VideoClip,
        "red",
        0.0,
        4.0,
        0.0,
    )];
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![
        clip(
            "tone-a-clip",
            TimelineItemKind::AudioClip,
            "tone-a",
            0.0,
            2.0,
            1.0,
        ),
        clip(
            "tone-b-clip",
            TimelineItemKind::AudioClip,
            "tone-b",
            2.0,
            2.0,
            1.0,
        ),
    ];
    project.timeline.duration_seconds = 4.0;
    add_transition(
        &mut project,
        "track-audio",
        ("tone-a-clip", "tone-b-clip"),
        TransitionKind::Crossfade,
        1.0,
    );
    project
}

/// A 2 s tone that fades in over 0.5 s, reading from source second 1.
pub(crate) fn audio_fade_project() -> VideoProject {
    let mut project = base_project();
    project.timeline.tracks[0].items = vec![clip(
        "red-clip",
        TimelineItemKind::VideoClip,
        "red",
        0.0,
        2.0,
        0.0,
    )];
    project.timeline.tracks[AUDIO_TRACK_INDEX].items = vec![with_properties(
        clip(
            "tone-a-clip",
            TimelineItemKind::AudioClip,
            "tone-a",
            0.0,
            2.0,
            1.0,
        ),
        json!({ "fadeInSeconds": 0.5 }),
    )];
    project.timeline.duration_seconds = 2.0;
    project
}

pub(crate) fn plan(dir: &Path, project: &VideoProject, job: &str) -> RenderPlan {
    build_project_webm_render_plan(dir, project, job, RenderQualityProfile::FinalWebm)
        .expect("render plan")
}

pub(crate) fn range_plan(
    dir: &Path,
    project: &VideoProject,
    job: &str,
    start: f64,
    end: f64,
) -> RenderPlan {
    build_project_webm_render_plan_for_range(
        dir,
        project,
        job,
        RenderQualityProfile::FinalWebm,
        start,
        end,
    )
    .expect("range render plan")
}

/// Renders `plan` with GES and returns the output path.
pub(crate) fn render(plan: &RenderPlan) -> PathBuf {
    let output = PathBuf::from(&plan.output_path);
    std::fs::create_dir_all(output.parent().expect("output parent")).expect("output directory");
    GstreamerGesRenderBackend::new()
        .render_cancellable(
            &SystemProcessRunner,
            plan,
            &[],
            Duration::from_secs(120),
            None,
        )
        .expect("GES render");
    output
}
