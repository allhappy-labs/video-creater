//! Lottie clips through transitions: render preparation bakes the Lottie
//! worker's frames into an intermediate that covers the clip's transition
//! handles, and GES renders it like canonical frames.
//!
//! The fixture (`tests/fixtures/transitions/lottie-colour-steps.json`) is a
//! 4 s, 24 fps, render-sized animation whose fill holds a new colour every 7
//! frames, so a frame's colour identifies its source time.

use super::media::{FPS, HEIGHT, WIDTH};
use super::parity::{assert_render_parity, fixture_dir, load_fixture, transition_sample_times};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use video_creater_lib::precompose::{
    decode_video_frames_rgba, prepare_project_for_render, FrameDecodeSpec, PreparedProject,
};
use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::transition_plan::plan_clip_transitions;

const FIXTURE: &str = "tests/fixtures/transitions/lottie-colour-steps.json";
const STEP_FRAMES: usize = 7;
const LOTTIE_SECONDS: f64 = 4.0;

/// The Lottie worker `precompose_worker_path` resolves for this test binary.
fn require_lottie_worker() {
    let worker = std::env::var_os("VIDEO_CREATER_PRECOMPOSE_WORKER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let exe = std::env::current_exe().expect("test executable");
            let mut dir = exe.parent().expect("test directory").to_path_buf();
            if dir.file_name().and_then(|name| name.to_str()) == Some("deps") {
                dir.pop();
            }
            dir.join("video-creater-precompose-worker")
        });
    if !worker.is_file() {
        panic!(
            "Lottie worker {} is missing. Build the Lottie worker with `pnpm build:precompose-sidecar:dev` or set VIDEO_CREATER_PRECOMPOSE_WORKER.",
            worker.display()
        );
    }
    eprintln!("Lottie worker: {}", worker.display());
}

/// The fill colour of each step, as RGB bytes, read from the fixture.
fn step_colours() -> Vec<[u8; 3]> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let lottie: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json");
    lottie["layers"][0]["shapes"][1]["c"]["k"]
        .as_array()
        .expect("colour keyframes")
        .iter()
        .map(|keyframe| {
            let channel = |index: usize| {
                (keyframe["s"][index].as_f64().expect("channel") * 255.0).round() as u8
            };
            [channel(0), channel(1), channel(2)]
        })
        .collect()
}

fn centre(frame: &[u8]) -> [u8; 3] {
    let offset = (HEIGHT / 2 * WIDTH + WIDTH / 2) * 4;
    [frame[offset], frame[offset + 1], frame[offset + 2]]
}

fn assert_colour(label: &str, actual: [u8; 3], expected: [u8; 3]) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(left, right)| left.abs_diff(right) <= 8),
        "{label}: {actual:?} is not {expected:?}"
    );
}

fn media_clip(
    id: &str,
    media: &str,
    start: f64,
    duration: f64,
    source_in: f64,
    speed: f64,
) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: media.to_string(),
        },
        label: id.to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(source_in)),
            ("sourceOut".to_string(), json!(source_in + duration * speed)),
            ("speed".to_string(), json!(speed)),
        ]),
    }
}

/// Cool video, then the Lottie clip at `speed` over `lottie_duration` from
/// source `source_in`, then warm video, joined by a 1 s crossfade and a 1 s
/// dip to black.
fn lottie_project(dir: &Path, lottie_duration: f64, source_in: f64, speed: f64) -> VideoProject {
    let mut project = lottie_cuts(dir, lottie_duration, source_in, speed);
    for (id, left, right, kind) in [
        ("fade", "cool", "steps", TransitionKind::Crossfade),
        ("dip", "steps", "warm", TransitionKind::DipToBlack),
    ] {
        apply_project_action(
            &mut project,
            ProjectAction::AddTransition {
                track_id: "track-video".to_string(),
                transition: TimelineTransition {
                    id: id.to_string(),
                    left_item_id: left.to_string(),
                    right_item_id: right.to_string(),
                    kind,
                    duration_seconds: 1.0,
                },
            },
        )
        .unwrap_or_else(|error| panic!("{id} transition: {error:?}"));
    }
    project
}

/// The clips of `lottie_project`, joined by cuts.
fn lottie_cuts(dir: &Path, lottie_duration: f64, source_in: f64, speed: f64) -> VideoProject {
    require_lottie_worker();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE),
        dir.join("media").join("colour-steps.json"),
    )
    .expect("copy Lottie fixture");
    let mut project = load_fixture();
    project.media.push(MediaAsset {
        id: "steps".to_string(),
        name: Some("steps".to_string()),
        relative_path: "media/colour-steps.json".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: LOTTIE_SECONDS,
        width: Some(WIDTH as u32),
        height: Some(HEIGHT as u32),
        fps: Some(FPS),
        folder_id: None,
    });
    let warm_start = 2.0 + lottie_duration;
    let track = &mut project.timeline.tracks[0];
    track.transitions.clear();
    track.items = vec![
        media_clip("cool", "cool", 0.0, 2.0, 1.0, 1.0),
        media_clip("steps", "steps", 2.0, lottie_duration, source_in, speed),
        media_clip("warm", "warm", warm_start, 2.0, 1.0, 1.0),
    ];
    project.timeline.duration_seconds = warm_start + 2.0;
    project
}

/// The prepared Lottie item and its intermediate media.
fn prepared_lottie(prepared: &PreparedProject) -> (&TimelineItem, &MediaAsset) {
    let item = prepared.project.timeline.tracks[0]
        .items
        .iter()
        .find(|item| item.id == "steps")
        .expect("prepared Lottie item");
    let TimelineSource::Media { media_id } = &item.source else {
        panic!("prepared Lottie clip is media-backed");
    };
    assert!(media_id.starts_with("precompose-"), "{media_id}");
    let media = prepared
        .project
        .media
        .iter()
        .find(|media| media.id == *media_id)
        .expect("prepared Lottie media");
    (item, media)
}

/// Centre colours of every frame of a prepared intermediate.
fn intermediate_centres(dir: &Path, media: &MediaAsset) -> Vec<[u8; 3]> {
    let frame_count = (media.duration_seconds * FPS).round() as u32;
    let mut centres = Vec::new();
    decode_video_frames_rgba(
        &dir.join(&media.relative_path),
        FrameDecodeSpec {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            fps_numerator: FPS as u32,
            fps_denominator: 1,
            source_start_micros: 0,
            source_stop_micros: (media.duration_seconds * 1_000_000.0).round() as u64,
            playback_rate_micros: 1_000_000,
            frame_count,
        },
        Duration::from_secs(60),
        |_, rgba| {
            centres.push(centre(rgba));
            Ok(())
        },
    )
    .expect("decode Lottie intermediate");
    centres
}

fn assert_transitions_kept(project: &VideoProject) {
    let planned = plan_clip_transitions(project);
    assert_eq!(planned.len(), 2, "the Lottie clip keeps both transitions");
    assert!(planned
        .iter()
        .all(|transition| (transition.duration_seconds - 1.0).abs() < 1e-6));
}

#[test]
fn lottie_fixture_steps_colour_every_seven_frames() {
    let dir = fixture_dir();
    let mut project = lottie_cuts(dir.path(), 2.0, 0.0, 1.0);
    project.timeline.tracks[0]
        .items
        .retain(|item| item.id == "steps");
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare Lottie");
    let (_, media) = prepared_lottie(&prepared);
    let centres = intermediate_centres(dir.path(), media);
    let colours = step_colours();
    for frame in [0, 7, 14] {
        assert_colour(
            &format!("intermediate frame {frame}"),
            centres[frame],
            colours[frame / STEP_FRAMES],
        );
    }
}

#[test]
fn lottie_clip_keeps_both_transition_handles() {
    let dir = fixture_dir();
    let project = lottie_project(dir.path(), 2.0, 1.0, 1.0);
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare Lottie");
    let (item, media) = prepared_lottie(&prepared);
    assert_eq!(item.properties["sourceIn"], json!(0.5));
    assert_eq!(
        media.duration_seconds, 3.0,
        "0.5 s head + 2 s clip + 0.5 s tail"
    );
    assert_transitions_kept(&prepared.project);

    // Prepared 0.25 s is in the head handle: Lottie time 1.0 - 0.5 + 0.25.
    let centres = intermediate_centres(dir.path(), media);
    assert_colour(
        "head handle at prepared 0.25 s",
        centres[(0.25 * FPS) as usize],
        step_colours()[(0.75 * FPS) as usize / STEP_FRAMES],
    );
}

#[test]
fn lottie_handles_render_like_canonical_frames() {
    let dir = fixture_dir();
    let project = lottie_project(dir.path(), 2.0, 1.0, 1.0);
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare Lottie");
    let comparison = assert_render_parity(
        "lottie-handles",
        dir.path(),
        &prepared.project,
        &prepared.project,
        &transition_sample_times(&[2.0, 4.0]),
    );
    eprintln!(
        "lottie-handles mismatch ratios: {:?}",
        comparison["comparedFrames"]
            .as_array()
            .expect("compared frames")
            .iter()
            .map(|frame| frame["mismatchRatio"].clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn sped_up_lottie_clip_keeps_handles() {
    let dir = fixture_dir();
    // Speed 1.5 over 1.5 s from Lottie 1.0 s: the 0.5 s handles read Lottie
    // 0.25-1.0 s and 3.25-4.0 s.
    let project = lottie_project(dir.path(), 1.5, 1.0, 1.5);
    let prepared = prepare_project_for_render(dir.path(), &project).expect("prepare Lottie");
    let (item, media) = prepared_lottie(&prepared);
    assert_eq!(item.properties["sourceIn"], json!(0.5));
    assert!(
        (media.duration_seconds - 2.5).abs() < 1e-6,
        "0.5 s head + 1.5 s clip + 0.5 s tail: {}",
        media.duration_seconds
    );
    assert_transitions_kept(&prepared.project);

    // Prepared frame n samples Lottie frame 6 + 1.5 n (Lottie time
    // 0.25 + t * 1.5); these frames sit away from colour step boundaries.
    let centres = intermediate_centres(dir.path(), media);
    let colours = step_colours();
    for (prepared_frame, lottie_frame) in [(3, 10.5), (6, 15.0), (25, 43.5), (54, 87.0)] {
        assert_colour(
            &format!("prepared frame {prepared_frame}"),
            centres[prepared_frame],
            colours[lottie_frame as usize / STEP_FRAMES],
        );
    }

    let mut times = transition_sample_times(&[2.0, 3.5]);
    times.sort_by(f64::total_cmp);
    assert_render_parity(
        "lottie-handles-speed",
        dir.path(),
        &prepared.project,
        &prepared.project,
        &times,
    );
}
