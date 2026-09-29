//! Canonical preview sampling against GES renders of every transition kind.
//!
//! The fixture project (`tests/fixtures/transitions/preview-render-parity.json`)
//! chains five 2 s clips at speed 1 through a crossfade, a dip to black, a dip
//! to white and a wipe, each 1 s. Its media change colour every 7 frames, so a
//! frame drawn from the wrong source time shows the wrong colour.
//!
//! At each transition's quarter, midpoint and three-quarter points the
//! canonical frame (`render_canonical_frame_rgba`, the sampler behind prepared
//! preview frames) is compared with the GES-rendered frame through
//! `scripts/compare-preview-render-frames.mjs`. Evidence is retained under
//! `output/transition-preview-parity/`.

use super::fixtures::{add_transition, base_project, clip, render, start_runtime};
use super::media::{decode_frames, write_segmented_video, FPS, HEIGHT, WIDTH};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::precompose::render_canonical_frame_rgba;
use video_creater_lib::project::model::*;
use video_creater_lib::render_pipeline::project_export::build_project_webm_render_plan;

const FIXTURE: &str = "tests/fixtures/transitions/preview-render-parity.json";
const SEGMENT_FRAMES: u32 = 7;
const SEGMENTS: usize = 14;
/// Mismatch ratio allowed per frame: the `visual:qa:preview-render` default.
const MISMATCH_THRESHOLD: f64 = 0.01;
/// Per-channel difference ignored per pixel. The tooling defaults to 0, which
/// no lossy render can meet; VP8 re-encoding of the GES output moves flat
/// colours by up to this much.
const CHANNEL_THRESHOLD: u8 = 16;

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

pub(crate) fn segment_colours(warm: bool) -> Vec<u32> {
    (0..SEGMENTS)
        .map(|index| {
            let ramp = 16 * index as u32;
            if warm {
                0xff00_0000 | 0xf0 << 16 | ramp << 8 | 0x20
            } else {
                0xff00_0000 | 0x20 << 16 | ramp << 8 | 0xf0
            }
        })
        .collect()
}

/// Five clips on the video track, each reading source 1-3 s of a 4 s medium.
fn fixture_project() -> VideoProject {
    let mut project = base_project();
    project.id = "transition-preview-render-parity".to_string();
    project.name = "Transition preview/render parity".to_string();
    for media in &mut project.media {
        match media.id.as_str() {
            "red" => {
                media.id = "warm".to_string();
                media.name = Some("warm".to_string());
                media.relative_path = "media/warm.webm".to_string();
            }
            "blue" => {
                media.id = "cool".to_string();
                media.name = Some("cool".to_string());
                media.relative_path = "media/cool.webm".to_string();
            }
            _ => {}
        }
    }
    project.media.retain(|media| media.kind == MediaKind::Video);
    project.timeline.tracks[0].items = (0..5)
        .map(|index| {
            let media = if index % 2 == 0 { "warm" } else { "cool" };
            clip(
                &format!("clip-{index}"),
                TimelineItemKind::VideoClip,
                media,
                2.0 * index as f64,
                2.0,
                1.0,
            )
        })
        .collect();
    project.timeline.duration_seconds = 10.0;
    for (index, kind) in [
        TransitionKind::Crossfade,
        TransitionKind::DipToBlack,
        TransitionKind::DipToWhite,
        TransitionKind::Wipe,
    ]
    .into_iter()
    .enumerate()
    {
        add_transition(
            &mut project,
            "track-video",
            (&format!("clip-{index}"), &format!("clip-{}", index + 1)),
            kind,
            1.0,
        );
    }
    project
}

#[test]
fn fixture_project_matches_its_builder() {
    let built = fixture_project();
    let path = manifest_path(FIXTURE);
    if std::env::var_os("UPDATE_TRANSITION_PARITY_FIXTURE").is_some() {
        std::fs::create_dir_all(path.parent().expect("fixture directory")).expect("fixture dir");
        let json = serde_json::to_string_pretty(&built).expect("serialize fixture");
        std::fs::write(&path, format!("{json}\n")).expect("write fixture");
    }
    let committed: VideoProject =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read fixture"))
            .expect("fixture project");
    assert_eq!(committed, built);
    let transitions = &committed.timeline.tracks[0].transitions;
    assert_eq!(transitions.len(), 4, "one transition of each kind");
}

fn write_png(path: &Path, rgba: &[u8]) {
    image::RgbaImage::from_raw(WIDTH as u32, HEIGHT as u32, rgba.to_vec())
        .expect("frame dimensions")
        .save(path)
        .expect("write frame png");
}

/// `(max channel difference, 99th percentile of per-pixel max channel difference)`.
fn channel_differences(canonical: &[u8], rendered: &[u8]) -> (u8, u8) {
    let mut per_pixel = canonical
        .chunks_exact(4)
        .zip(rendered.chunks_exact(4))
        .map(|(left, right)| {
            (0..4)
                .map(|channel| left[channel].abs_diff(right[channel]))
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    per_pixel.sort_unstable();
    let max = per_pixel.last().copied().unwrap_or(0);
    let p99 = per_pixel[((per_pixel.len() as f64 * 0.99).ceil() as usize).min(per_pixel.len()) - 1];
    (max, p99)
}

/// The committed fixture project.
pub(crate) fn load_fixture() -> VideoProject {
    serde_json::from_str(&std::fs::read_to_string(manifest_path(FIXTURE)).expect("fixture"))
        .expect("fixture project")
}

/// A temporary project folder holding the fixture's colour-segmented media.
pub(crate) fn fixture_dir() -> tempfile::TempDir {
    start_runtime();
    let dir = tempfile::tempdir().expect("project directory");
    std::fs::create_dir_all(dir.path().join("media")).expect("media directory");
    for (file, warm) in [("warm.webm", true), ("cool.webm", false)] {
        write_segmented_video(
            &dir.path().join("media").join(file),
            &segment_colours(warm),
            SEGMENT_FRAMES,
        );
    }
    dir
}

/// Renders `render_project` with GES, then compares its frames at `times`
/// with canonical frames of `canonical_project` through the comparison
/// tooling. Evidence goes to `output/transition-preview-parity/<label>`.
pub(crate) fn assert_render_parity(
    label: &str,
    dir: &Path,
    canonical_project: &VideoProject,
    render_project: &VideoProject,
    times: &[f64],
) -> Value {
    let plan =
        build_project_webm_render_plan(dir, render_project, label, RenderQualityProfile::FinalWebm)
            .expect("render plan");
    let rendered = decode_frames(&render(&plan));

    let evidence = manifest_path("../output/transition-preview-parity").join(label);
    let _ = std::fs::remove_dir_all(&evidence);
    let preview_dir = evidence.join("preview-frames");
    let rendered_dir = evidence.join("rendered-frames");
    std::fs::create_dir_all(&preview_dir).expect("preview frame directory");
    std::fs::create_dir_all(&rendered_dir).expect("rendered frame directory");

    let mut command = Command::new("node");
    let comparison_path = evidence.join("preview-comparison.json");
    command
        .current_dir(manifest_path(".."))
        .arg("scripts/compare-preview-render-frames.mjs")
        .args(["--out", &comparison_path.display().to_string()])
        .args(["--diff-dir", &evidence.join("diffs").display().to_string()])
        .args(["--threshold", &MISMATCH_THRESHOLD.to_string()])
        .args(["--channel-threshold", &CHANNEL_THRESHOLD.to_string()]);
    for seconds in times {
        let frame_index = (seconds * FPS).round() as i64;
        let canonical = render_canonical_frame_rgba(dir, canonical_project, *seconds, None)
            .unwrap_or_else(|errors| panic!("{label}: canonical frame at {seconds}: {errors:?}"));
        let ges = rendered
            .get(&frame_index)
            .unwrap_or_else(|| panic!("{label}: rendered frame {frame_index}"));
        let (max, p99) = channel_differences(&canonical, ges);
        let centre = (HEIGHT / 2 * WIDTH + WIDTH / 2) * 4;
        eprintln!(
            "{label} {seconds:>5.2}s: max channel diff {max}, p99 {p99}, centre canonical {:?} ges {:?}",
            &canonical[centre..centre + 4],
            &ges[centre..centre + 4]
        );
        let preview_path = preview_dir.join(format!("canonical-{frame_index:03}.png"));
        let rendered_path = rendered_dir.join(format!("ges-{frame_index:03}.png"));
        write_png(&preview_path, &canonical);
        write_png(&rendered_path, ges);
        command.args([
            "--frame",
            &format!(
                "{seconds}:{}:{}",
                preview_path.display(),
                rendered_path.display()
            ),
        ]);
    }
    let output = command.output().expect("run the comparison tooling");
    assert!(
        output.status.code() == Some(0),
        "{label}: comparison tooling failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let comparison: Value =
        serde_json::from_slice(&std::fs::read(&comparison_path).expect("comparison report"))
            .expect("comparison json");
    for frame in comparison["comparedFrames"]
        .as_array()
        .expect("compared frames")
    {
        eprintln!(
            "{label} compared {}s: mismatch ratio {}",
            frame["timelineSeconds"], frame["mismatchRatio"]
        );
    }
    assert_eq!(
        comparison["status"],
        "passed",
        "{label}: preview/render parity failed; inspect {}",
        comparison_path.display()
    );
    comparison
}

/// Quarter, midpoint and three quarters of each fixture transition window.
pub(crate) fn transition_sample_times(cuts: &[f64]) -> Vec<f64> {
    cuts.iter()
        .flat_map(|cut| [cut - 0.25, *cut, cut + 0.25])
        .collect()
}

#[test]
fn canonical_preview_frames_match_ges_renders_at_every_transition_kind() {
    let project = load_fixture();
    let dir = fixture_dir();
    let plan = build_project_webm_render_plan(
        dir.path(),
        &project,
        "transition-parity-plan",
        RenderQualityProfile::FinalWebm,
    )
    .expect("render plan");
    assert_eq!(plan.transitions.len(), 4, "every transition renders");
    // A frame outside every window, then each kind in timeline order.
    let mut times = vec![1.0];
    times.extend(transition_sample_times(&[2.0, 4.0, 6.0, 8.0]));
    assert_render_parity("every-kind", dir.path(), &project, &project, &times);
}
