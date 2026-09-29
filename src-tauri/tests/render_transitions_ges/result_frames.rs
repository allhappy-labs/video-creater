//! Agent result frames on Linux: `render_prepared_preview_frame_to_split_project_folder`
//! captures through the Rust canonical frame sampler instead of a one-frame native render.
//!
//! Evidence is retained under `output/linux-result-frames/`.
#![cfg(not(target_os = "macos"))]

use super::media::decode_frames;
use super::parity::{fixture_dir, load_fixture};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use video_creater_lib::edit::render_plan::RenderQualityProfile;
use video_creater_lib::precompose::{prepare_project_for_render, render_canonical_frame_rgba};
use video_creater_lib::project::model::*;
use video_creater_lib::project::split::{
    agent_undo_comparable_content, load_split_project, save_split_project, validate_split_project,
};
use video_creater_lib::render_pipeline::project_export::{
    render_prepared_preview_frame_to_split_project_folder, render_webm_to_split_project_folder,
};
use video_creater_lib::workflows::{temporal_job_summary, TemporalWorkflowKind};

const UPDATED_AT: &str = "2026-09-16T10:00:00Z";
/// The transition parity policy (`parity.rs`): mismatch ratio and per-channel tolerance.
const CAPTION_MISMATCH_THRESHOLD: f64 = 0.01;
const CAPTION_CHANNEL_THRESHOLD: u8 = 16;

fn evidence_dir(label: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../output/linux-result-frames")
        .join(label);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("evidence directory");
    dir
}

fn retain(dir: &Path, relative: &str, evidence: &Path) {
    let source = dir.join(relative);
    let file_name = source.file_name().expect("artifact file name");
    std::fs::copy(&source, evidence.join(file_name)).expect("retain evidence");
}

fn job<'a>(project: &'a VideoProject, id: &str) -> Option<&'a JobSummary> {
    project.jobs.iter().find(|job| job.id == id)
}

fn generated_asset(id: &str, status: GeneratedAssetStatus) -> GeneratedAsset {
    serde_json::from_value(json!({
        "schemaVersion": 1,
        "id": id,
        "kind": "video",
        "status": status,
        "prompt": "A warm abstract loop",
        "model": { "provider": "fal.ai", "id": "fal-ai/test-video" },
        "references": { "mediaIds": [], "firstFrameMediaId": null, "lastFrameMediaId": null },
        "outputs": [],
        "createdAt": UPDATED_AT,
        "parentAssetId": null,
        "retryOfAssetId": null
    }))
    .expect("generated asset")
}

#[test]
fn capture_writes_the_canonical_frame_and_records_its_job() {
    let dir = fixture_dir();
    let project = load_fixture();
    save_split_project(dir.path(), &project).expect("save split project");
    let saved = load_split_project(dir.path()).expect("saved project");

    let result = render_prepared_preview_frame_to_split_project_folder(
        dir.path(),
        2.0,
        "agent-result-frame-test",
        UPDATED_AT,
    )
    .unwrap_or_else(|errors| panic!("capture failed: {errors:?}"));

    assert_eq!(
        result.preview_frame,
        "renders/agent-result-frame-test/preview-qa/preview-frames/preview-0001.png"
    );
    let decoded = image::open(dir.path().join(&result.preview_frame))
        .expect("decode captured frame")
        .to_rgba8();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (
            project.render_settings.width,
            project.render_settings.height
        )
    );
    let prepared_project = prepare_project_for_render(dir.path(), &project)
        .expect("prepare project")
        .project;
    let sampled = render_canonical_frame_rgba(dir.path(), &prepared_project, 2.0, None)
        .expect("sampler frame");
    assert!(
        decoded.as_raw() == &sampled,
        "captured frame equals the sampler frame"
    );

    let evidence: Value = serde_json::from_slice(
        &std::fs::read(dir.path().join(&result.evidence_report)).expect("evidence report"),
    )
    .expect("evidence json");
    assert_eq!(evidence["backend"], "rust-canonical-sampler");
    assert_eq!(evidence["status"], "captured");
    assert_eq!(evidence["previewFrame"], result.preview_frame.as_str());
    assert_eq!(
        result.render_report.command.program,
        "rust-canonical-sampler"
    );
    assert!(dir
        .path()
        .join("renders/agent-result-frame-test/report.json")
        .is_file());
    assert!(dir
        .path()
        .join("renders/agent-result-frame-test/render.log")
        .is_file());

    let reloaded = load_split_project(dir.path()).expect("reload project");
    for recorded in [&result.project, &reloaded] {
        let capture_job = job(recorded, "agent-result-frame-test").expect("capture job");
        assert_eq!(capture_job.kind, "captureCanonicalPreviewFrame");
        assert_eq!(capture_job.status, JobStatus::Completed);
        let run_id = capture_job
            .workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref())
            .expect("capture run id");
        assert!(run_id.starts_with("render-attempt/"), "{run_id}");
        let report = recorded
            .render_reports
            .iter()
            .find(|report| report.id == "agent-result-frame-test")
            .expect("project render report");
        assert_eq!(report.status, RenderReportStatus::Completed);
    }
    assert!(validate_split_project(dir.path()).expect("validate").ok);
    assert_eq!(
        agent_undo_comparable_content(&reloaded, &[]),
        agent_undo_comparable_content(&saved, &[])
    );

    let evidence = evidence_dir("video-tracks");
    retain(dir.path(), &result.preview_frame, &evidence);
    retain(dir.path(), &result.evidence_report, &evidence);
}

fn assert_refused(dir: &Path, playhead_seconds: f64, job_id: &str, names: &str) {
    let errors = render_prepared_preview_frame_to_split_project_folder(
        dir,
        playhead_seconds,
        job_id,
        UPDATED_AT,
    )
    .expect_err("capture is refused");
    let error = errors
        .iter()
        .find(|error| error.path == "canonicalPreview.unsupportedSource")
        .unwrap_or_else(|| panic!("unsupported source refusal: {errors:?}"));
    assert!(error.message.contains(names), "{}", error.message);
    let project = load_split_project(dir).expect("reload project");
    assert_eq!(
        job(&project, job_id).expect("refused capture job").status,
        JobStatus::Failed
    );
    assert!(!dir
        .join(format!(
            "renders/{job_id}/preview-qa/preview-frames/preview-0001.png"
        ))
        .exists());
}

#[test]
fn capture_refuses_image_clips_and_pending_generations_plainly() {
    let dir = fixture_dir();
    let mut project = load_fixture();
    image::RgbaImage::from_pixel(
        project.render_settings.width,
        project.render_settings.height,
        image::Rgba([10, 200, 30, 255]),
    )
    .save(dir.path().join("media/still.png"))
    .expect("write still image");
    project.media.push(MediaAsset {
        id: "still".to_string(),
        name: Some("still".to_string()),
        relative_path: "media/still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(project.render_settings.width),
        height: Some(project.render_settings.height),
        fps: None,
        folder_id: None,
    });
    let track = &mut project.timeline.tracks[0];
    track.transitions.clear();
    track.items[0].kind = TimelineItemKind::ImageClip;
    track.items[0].source = TimelineSource::Media {
        media_id: "still".to_string(),
    };
    project.generated_assets.push(generated_asset(
        "pending-generation",
        GeneratedAssetStatus::Running,
    ));
    track.items[2].kind = TimelineItemKind::GeneratedClip;
    track.items[2].source = TimelineSource::Generated {
        artifact_id: "pending-generation".to_string(),
    };
    save_split_project(dir.path(), &project).expect("save split project");

    assert_refused(dir.path(), 1.0, "refuse-image", "image clips");
    assert_refused(dir.path(), 5.0, "refuse-pending", "unfinished generations");

    let errors = render_prepared_preview_frame_to_split_project_folder(
        dir.path(),
        project.timeline.duration_seconds,
        "refuse-end",
        UPDATED_AT,
    )
    .expect_err("playhead at the end is refused");
    assert_eq!(errors[0].path, "playheadSeconds");
    assert!(job(
        &load_split_project(dir.path()).expect("reload"),
        "refuse-end"
    )
    .is_none());
}

/// A burned-in caption on the caption track covering 1.5-2.5 s.
fn captioned_fixture() -> VideoProject {
    let mut project = load_fixture();
    project.render_settings.captions = CaptionRenderMode::BurnIn;
    let caption_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("caption track");
    caption_track.items.push(TimelineItem {
        id: "result-caption".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 1.5,
        duration_seconds: 1.0,
        source: TimelineSource::Text {
            text: "RESULT FRAME".to_string(),
        },
        label: "Result caption".to_string(),
        properties: std::collections::BTreeMap::from([
            (
                "visualTreatment".to_string(),
                json!("transparent editorial caption with strong hierarchy"),
            ),
            ("motion".to_string(), json!("hold")),
            (
                "safeZone".to_string(),
                json!("keep text inside 10% margins"),
            ),
            ("avoid".to_string(), json!("full-width opaque black slabs")),
        ]),
    });
    project
}

fn capture_rgba(dir: &Path, relative: &str) -> Vec<u8> {
    image::open(dir.join(relative))
        .expect("decode captured frame")
        .to_rgba8()
        .into_raw()
}

#[test]
fn capture_draws_captions_over_the_sampled_video() {
    let dir = fixture_dir();
    let project = captioned_fixture();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = render_prepared_preview_frame_to_split_project_folder(
        dir.path(),
        2.0,
        "caption-result-frame",
        UPDATED_AT,
    )
    .unwrap_or_else(|errors| panic!("capture failed: {errors:?}"));

    let captured = capture_rgba(dir.path(), &result.preview_frame);
    let prepared = prepare_project_for_render(dir.path(), &project)
        .expect("prepare project")
        .project;
    let sampled =
        render_canonical_frame_rgba(dir.path(), &prepared, 2.0, None).expect("sampler frame");
    let graphics_frame = result
        .render_report
        .artifacts
        .iter()
        .find(|artifact| artifact.contains("/graphics/") && artifact.ends_with("frame-000000.png"))
        .unwrap_or_else(|| {
            panic!(
                "graphics frame artifact: {:?}",
                result.render_report.artifacts
            )
        });
    let caption = capture_rgba(dir.path(), graphics_frame);
    assert_eq!(result.render_report.graphics.len(), 1);

    let mut inside_changed = 0;
    for ((captured, sampled), caption) in captured
        .chunks_exact(4)
        .zip(sampled.chunks_exact(4))
        .zip(caption.chunks_exact(4))
    {
        if caption[3] == 0 {
            assert_eq!(
                captured, sampled,
                "pixels outside the caption are the sampler's"
            );
        } else if captured != sampled {
            inside_changed += 1;
        }
    }
    assert!(inside_changed > 0, "the caption is drawn over the video");

    let evidence = evidence_dir("caption");
    retain(dir.path(), &result.preview_frame, &evidence);
    retain(dir.path(), graphics_frame, &evidence);
}

#[test]
fn capture_matches_the_ges_render_with_a_caption() {
    let dir = fixture_dir();
    let project = captioned_fixture();
    save_split_project(dir.path(), &project).expect("save split project");

    let capture = render_prepared_preview_frame_to_split_project_folder(
        dir.path(),
        2.0,
        "caption-parity-frame",
        UPDATED_AT,
    )
    .unwrap_or_else(|errors| panic!("capture failed: {errors:?}"));
    let mut render_job = temporal_job_summary(
        TemporalWorkflowKind::RenderDraft,
        &project.id,
        "caption-parity-render",
        JobStatus::Queued,
        UPDATED_AT,
    );
    render_job.kind = "render_draft".to_string();
    let render = render_webm_to_split_project_folder(
        dir.path(),
        &project.id,
        RenderQualityProfile::FinalWebm,
        render_job,
        UPDATED_AT,
        None,
        None,
    )
    .unwrap_or_else(|errors| panic!("GES render failed: {errors:?}"));
    let rendered = decode_frames(&dir.path().join(&render.output_path));
    let ges_frame = rendered.get(&48).expect("GES frame 48");

    let evidence = evidence_dir("caption-parity");
    let preview_path = evidence.join("capture-048.png");
    let rendered_path = evidence.join("ges-048.png");
    std::fs::copy(dir.path().join(&capture.preview_frame), &preview_path).expect("copy capture");
    image::RgbaImage::from_raw(
        project.render_settings.width,
        project.render_settings.height,
        ges_frame.clone(),
    )
    .expect("GES frame size")
    .save(&rendered_path)
    .expect("write GES frame");
    let comparison_path = evidence.join("preview-comparison.json");
    let output = Command::new("node")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
        .arg("scripts/compare-preview-render-frames.mjs")
        .args(["--out", &comparison_path.display().to_string()])
        .args(["--diff-dir", &evidence.join("diffs").display().to_string()])
        .args(["--threshold", &CAPTION_MISMATCH_THRESHOLD.to_string()])
        .args([
            "--channel-threshold",
            &CAPTION_CHANNEL_THRESHOLD.to_string(),
        ])
        .args([
            "--frame",
            &format!("2:{}:{}", preview_path.display(), rendered_path.display()),
        ])
        .output()
        .expect("run the comparison tooling");
    assert_eq!(
        output.status.code(),
        Some(0),
        "comparison tooling failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let comparison: Value =
        serde_json::from_slice(&std::fs::read(&comparison_path).expect("comparison report"))
            .expect("comparison json");
    eprintln!(
        "caption parity mismatch ratio {}",
        comparison["comparedFrames"][0]["mismatchRatio"]
    );
    assert_eq!(
        comparison["status"],
        "passed",
        "capture/GES caption parity failed; inspect {}",
        comparison_path.display()
    );
}

#[test]
fn capture_draws_a_completed_generated_video_clip() {
    let dir = fixture_dir();
    let mut project = load_fixture();
    let mut generated_media = project.media[0].clone();
    generated_media.id = "generated-warm".to_string();
    generated_media.kind = MediaKind::Generated;
    generated_media.fps = Some(24.0);
    project.media.push(generated_media);
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "generated-warm".to_string(),
    };
    save_split_project(dir.path(), &project).expect("save split project");

    let result = render_prepared_preview_frame_to_split_project_folder(
        dir.path(),
        1.0,
        "generated-result-frame",
        UPDATED_AT,
    )
    .unwrap_or_else(|errors| panic!("capture failed: {errors:?}"));

    let mut as_video = project.clone();
    as_video
        .media
        .iter_mut()
        .find(|media| media.id == "generated-warm")
        .expect("generated media")
        .kind = MediaKind::Video;
    let prepared = prepare_project_for_render(dir.path(), &as_video)
        .expect("prepare project")
        .project;
    let sampled =
        render_canonical_frame_rgba(dir.path(), &prepared, 1.0, None).expect("sampler frame");
    assert!(capture_rgba(dir.path(), &result.preview_frame) == sampled);
}
