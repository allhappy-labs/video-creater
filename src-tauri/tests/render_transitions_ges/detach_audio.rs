//! Detached sound renders: a video clip never sounds on the timeline, and
//! `detachAudio` adds an audio clip that plays the video file's own sound.

use super::fixtures::*;
use super::media::{
    decode_audio, dominant_frequency, rms, write_video_with_tone, FPS, RED, SAMPLE_RATE,
};
use serde_json::json;
use std::path::Path;
use video_creater_lib::edit::render_plan::RenderPlan;
use video_creater_lib::precompose::prepare_project_for_render;
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::model::*;

const CAMERA_SECONDS: f64 = 4.0;

/// A 4 s red camera clip with a 440 Hz tone, played over 0-4 s at `speed`
/// from source 0.
fn camera_project(dir: &Path, speed: f64) -> VideoProject {
    write_video_with_tone(
        &dir.join("media").join("camera.webm"),
        RED,
        440.0,
        CAMERA_SECONDS,
    );
    let mut project = base_project();
    project.media.push(MediaAsset {
        id: "camera".to_string(),
        name: Some("camera".to_string()),
        relative_path: "media/camera.webm".to_string(),
        kind: MediaKind::Video,
        duration_seconds: CAMERA_SECONDS,
        width: Some(super::media::WIDTH as u32),
        height: Some(super::media::HEIGHT as u32),
        fps: Some(FPS),
        folder_id: None,
    });
    let duration = CAMERA_SECONDS / speed;
    project.timeline.tracks[0].items = vec![with_properties(
        clip(
            "camera-clip",
            TimelineItemKind::VideoClip,
            "camera",
            0.0,
            duration,
            0.0,
        ),
        json!({ "sourceOut": CAMERA_SECONDS, "speed": speed }),
    )];
    project.timeline.duration_seconds = duration;
    project
}

fn detach(project: &mut VideoProject) {
    apply_project_action(
        project,
        ProjectAction::DetachAudio {
            item_id: "camera-clip".to_string(),
            audio_item_id: "camera-clip-audio".to_string(),
            target_track_id: "track-audio".to_string(),
            link_group_id: "link-camera-clip".to_string(),
        },
    )
    .expect("detach audio");
}

fn render_audio(dir: &Path, project: &VideoProject, job: &str) -> Vec<f32> {
    let prepared = prepare_project_for_render(dir, project).expect("prepare render");
    decode_audio(&render(&plan(dir, &prepared.project, job)))
}

fn audio_clip_of(plan: &RenderPlan) -> &video_creater_lib::edit::render_plan::RenderClip {
    assert_eq!(plan.audio_clips.len(), 1, "{:?}", plan.audio_clips);
    &plan.audio_clips[0]
}

#[test]
fn detached_audio_joins_the_render_plan_from_the_video_file() {
    let dir = project_dir();
    let mut project = camera_project(dir.path(), 1.0);
    assert!(plan(dir.path(), &project, "before-detach")
        .audio_clips
        .is_empty());

    detach(&mut project);
    let plan = plan(dir.path(), &project, "after-detach");
    let audio = audio_clip_of(&plan);
    let camera = dir.path().join("media").join("camera.webm");
    assert_eq!(
        audio.source_path.as_deref().map(Path::new),
        Some(camera.as_path())
    );
    assert_eq!((audio.source_in, audio.source_out), (0.0, CAMERA_SECONDS));
    assert_eq!(
        audio.properties.get("timelineDurationSeconds"),
        Some(&json!(CAMERA_SECONDS))
    );
}

#[test]
fn detached_audio_sounds_and_the_video_alone_is_silent() {
    let dir = project_dir();
    let mut project = camera_project(dir.path(), 1.0);
    let silent = render_audio(dir.path(), &project, "camera-silent");
    let silent_rms = if silent.is_empty() {
        0.0
    } else {
        rms(&silent, 1.0, 3.0)
    };

    detach(&mut project);
    let samples = render_audio(dir.path(), &project, "camera-detached");
    let steady = rms(&samples, 1.0, 3.0);
    let frequency = dominant_frequency(&samples, 1.0, 3.0);
    eprintln!(
        "camera before detach: {} samples, rms {silent_rms:.4}; after: rms {steady:.4}, {frequency:.1} Hz",
        silent.len()
    );
    assert!(
        silent_rms < 0.01,
        "the video clip alone is silent: {silent_rms}"
    );
    assert!(steady > 0.3, "the detached sound plays: {steady}");
    assert!(
        (frequency - 440.0).abs() <= 440.0 * 0.03,
        "the detached sound is the camera tone: {frequency}"
    );
}

#[test]
fn detached_audio_at_speed_two_keeps_pitch_and_length() {
    let dir = project_dir();
    let mut project = camera_project(dir.path(), 2.0);
    detach(&mut project);
    let samples = render_audio(dir.path(), &project, "camera-detached-fast");
    let seconds = samples.len() as f64 / SAMPLE_RATE as f64;
    let frequency = dominant_frequency(&samples, 0.3, 1.7);
    eprintln!("detached camera at speed 2: {seconds:.4} s, {frequency:.1} Hz");
    assert!(
        (seconds - 2.0).abs() <= 1.0 / FPS,
        "output length {seconds}"
    );
    assert!(
        (frequency - 440.0).abs() <= 440.0 * 0.03,
        "pitch is kept at 440 Hz: {frequency}"
    );
}
