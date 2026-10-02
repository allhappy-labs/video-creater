//! Canonical preview frame capture on platforms without the native AVFoundation compositor.
//!
//! macOS captures a one-frame native render. Elsewhere the capture samples the prepared
//! project's video tracks with the Rust canonical frame sampler
//! (`precompose::render_canonical_frame_rgba`), which is checked for parity against GES
//! renders. The capture records its job and render report exactly like a render does, and
//! writes the same `renders/<jobId>/preview-qa/` evidence layout.

use super::{
    claim_error_terminal, expand_project_nested_timelines_for_render, io_pipeline_error,
    register_project_render_attempt, render_job_start_actions, render_project_action_error,
    safe_path_segment, validate_render_project_write_path, PreparedPreviewFrameResult,
};
use crate::precompose::{prepare_project_for_render_cancellable, render_canonical_frame_rgba};
use crate::project::action::ProjectAction;
use crate::project::model::{
    GeneratedAssetStatus, JobStatus, JobSummary, MediaKind, ProjectRenderReport,
    RenderReportCheckStatus, RenderReportStatus, RenderReportStreams, TimelineSource, TrackKind,
    VideoProject,
};
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::split::{
    apply_project_bookkeeping_actions_to_split_project_with_lease, load_split_project,
    ProjectActionWriteResult,
};
use crate::render_pipeline::cancel::RenderCancellationToken;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::process::CommandSpec;
use crate::render_pipeline::report::{
    write_json_report, RenderReport, RenderReportStreams as PipelineRenderReportStreams,
    RenderReportSummary,
};
use crate::settings::storage::StorageMutationLease;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

#[path = "canonical_capture_graphics.rs"]
mod graphics;

const SAMPLER_BACKEND: &str = "rust-canonical-sampler";
const RENDER_LABEL: &str = "canonical preview";

struct CaptureIdentity {
    project_id: String,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl CaptureIdentity {
    fn capture(project_dir: &Path, project_id: &str) -> Result<Self, String> {
        let metadata = std::fs::metadata(project_dir).map_err(|error| error.to_string())?;
        if !metadata.is_dir() {
            return Err("canonical preview project is not a directory".into());
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            project_id: project_id.into(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        })
    }

    fn persist(
        &self,
        project_dir: &Path,
        job_id: &str,
        attempt_id: &str,
        actions: Vec<ProjectAction>,
        require_active_attempt: bool,
    ) -> Result<ProjectActionWriteResult, String> {
        let mutation = acquire_split_project_mutation_lease(project_dir)?;
        let actual = Self::capture(project_dir, &self.project_id)?;
        #[cfg(unix)]
        if (actual.device, actual.inode) != (self.device, self.inode) {
            return Err("canonical preview project package changed".into());
        }
        let current = load_split_project(project_dir).map_err(|error| error.to_string())?;
        if current.id != actual.project_id {
            return Err("canonical preview project identity changed".into());
        }
        if require_active_attempt
            && !current.jobs.iter().any(|job| {
                job.id == job_id
                    && matches!(
                        job.status,
                        JobStatus::Queued | JobStatus::Running | JobStatus::Progress
                    )
                    && job
                        .workflow
                        .as_ref()
                        .and_then(|workflow| workflow.run_id.as_deref())
                        == Some(attempt_id)
            })
        {
            return Err("canonical preview attempt is terminal or superseded".into());
        }
        apply_project_bookkeeping_actions_to_split_project_with_lease(
            project_dir,
            actions,
            &mutation,
        )
        .map_err(|error| error.to_string())
    }
}

/// Linux (non-macOS) canonical capture: the Rust sampler instead of a one-frame native render.
pub(super) fn capture_canonical_frame_with_sampler(
    project_dir: &Path,
    source_project: &VideoProject,
    playhead_seconds: f64,
    frame_end_seconds: f64,
    job: JobSummary,
    updated_at: &str,
    _lease: &StorageMutationLease,
) -> PipelineResult<PreparedPreviewFrameResult> {
    let identity = CaptureIdentity::capture(project_dir, &source_project.id).map_err(|error| {
        render_project_action_error(
            RENDER_LABEL,
            "Checking capture project identity failed.",
            error,
        )
    })?;
    let attempt = register_project_render_attempt(project_dir, &source_project.id, &job.id, None)?;
    let job_id = job.id.clone();
    let attempt_id = attempt.attempt_id.clone();
    identity
        .persist(
            project_dir,
            &job_id,
            &attempt_id,
            render_job_start_actions(source_project, job, updated_at, &attempt_id),
            false,
        )
        .map_err(|error| {
            render_project_action_error(RENDER_LABEL, "Persisting capture job start failed.", error)
        })?;
    let cancellation = attempt.guard.token();
    let capture = CaptureRequest {
        project_dir,
        source_project,
        playhead_seconds,
        frame_end_seconds,
        job_id: &job_id,
        attempt_id: &attempt_id,
        updated_at,
        cancellation: &cancellation,
        identity: &identity,
    };
    let result = capture_after_job_started(&capture);
    if result.is_err() {
        persist_capture_failure(&capture);
    }
    cancellation.mark_terminal();
    result
}

fn persist_capture_failure(capture: &CaptureRequest<'_>) {
    let status = claim_error_terminal(capture.cancellation);
    let _ = capture.identity.persist(
        capture.project_dir,
        capture.job_id,
        capture.attempt_id,
        vec![ProjectAction::UpdateJobStatus {
            job_id: capture.job_id.to_string(),
            status,
            updated_at: capture.updated_at.to_string(),
            run_id: Some(capture.attempt_id.to_string()),
        }],
        true,
    );
}

struct CaptureRequest<'a> {
    project_dir: &'a Path,
    source_project: &'a VideoProject,
    playhead_seconds: f64,
    frame_end_seconds: f64,
    job_id: &'a str,
    attempt_id: &'a str,
    updated_at: &'a str,
    cancellation: &'a RenderCancellationToken,
    identity: &'a CaptureIdentity,
}

fn capture_after_job_started(
    capture: &CaptureRequest<'_>,
) -> PipelineResult<PreparedPreviewFrameResult> {
    let project_dir = capture.project_dir;
    let expanded = expand_project_nested_timelines_for_render(capture.source_project)?;
    let prepared =
        prepare_project_for_render_cancellable(project_dir, &expanded, Some(capture.cancellation))?;
    let normalized = normalize_capture_sources(
        &prepared.project,
        capture.playhead_seconds,
        capture.frame_end_seconds,
    )?;
    let mut rgba = render_canonical_frame_rgba(
        project_dir,
        &normalized,
        capture.playhead_seconds,
        Some(capture.cancellation),
    )?;
    let (width, height) = (
        normalized.render_settings.width,
        normalized.render_settings.height,
    );

    let render_dir = format!("renders/{}", safe_path_segment(capture.job_id));
    let capture_graphics = graphics::render_capture_graphics(
        project_dir,
        &prepared.project,
        capture.playhead_seconds,
        capture.frame_end_seconds,
        &render_dir,
        capture.cancellation,
    )?;
    graphics::composite_graphics_frames(&mut rgba, width, height, &capture_graphics.frames)?;
    let preview_frame = format!("{render_dir}/preview-qa/preview-frames/preview-0001.png");
    let evidence_report = format!("{render_dir}/preview-qa/canonical-preview-frame.json");
    // `report.json` is the split project's render report, written when it is attached.
    let json_report = format!("{render_dir}/pipeline-report.json");
    let log_path = format!("{render_dir}/render.log");
    for relative in [&preview_frame, &evidence_report, &json_report, &log_path] {
        let path = project_dir.join(relative);
        validate_render_project_write_path(project_dir, &path)?;
        std::fs::create_dir_all(path.parent().unwrap_or(project_dir)).map_err(|error| {
            io_pipeline_error(
                "canonicalPreview.dir",
                "Create preview evidence directory failed.",
                error,
            )
        })?;
    }
    let image = image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
        vec![capture_error(
            "canonicalPreview.frame",
            "The sampled frame does not match the project size.",
        )]
    })?;
    image
        .save(project_dir.join(&preview_frame))
        .map_err(|error| {
            vec![capture_error(
                "canonicalPreview.frame",
                &format!("Write canonical preview frame failed: {error}"),
            )]
        })?;

    std::fs::write(
        project_dir.join(&log_path),
        format!(
            "{SAMPLER_BACKEND}: sampled the prepared project at {:.3}s ({width}x{height}) and drew {} graphics layer(s) over it.\n",
            capture.playhead_seconds,
            capture_graphics.frames.len()
        ),
    )
    .map_err(|error| io_pipeline_error("renders.log", "Write render log failed.", error))?;

    let evidence = json!({
        "schemaVersion": 1,
        "status": "captured",
        "source": "exact-split-project",
        "backend": SAMPLER_BACKEND,
        "playheadSeconds": capture.playhead_seconds,
        "frameDurationSeconds": capture.frame_end_seconds - capture.playhead_seconds,
        "previewFrame": preview_frame,
        "sourceOutput": preview_frame,
        "renderReport": format!("{render_dir}/report.json"),
        "policy": { "failOnMismatch": true }
    });
    let evidence_bytes = serde_json::to_vec_pretty(&evidence).map_err(|error| {
        vec![capture_error(
            "canonicalPreview.report",
            &format!("Canonical preview evidence could not be encoded: {error}"),
        )]
    })?;
    std::fs::write(project_dir.join(&evidence_report), evidence_bytes).map_err(|error| {
        io_pipeline_error(
            "canonicalPreview.report",
            "Write canonical preview evidence failed.",
            error,
        )
    })?;

    let mut artifacts = vec![
        preview_frame.clone(),
        evidence_report.clone(),
        json_report.clone(),
        log_path.clone(),
    ];
    artifacts.extend(capture_graphics.artifacts);
    let render_report = RenderReport {
        job_id: capture.job_id.to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(capture.frame_end_seconds - capture.playhead_seconds),
            output_path: Some(preview_frame.clone()),
            quality: None,
            requested_width: Some(width),
            requested_height: Some(height),
            actual_width: Some(width),
            actual_height: Some(height),
            container: Some("png".to_string()),
            video_codec: Some("png".to_string()),
            audio_codec: None,
        },
        command: CommandSpec::new(SAMPLER_BACKEND)
            .arg(format!("--playhead-seconds={}", capture.playhead_seconds))
            .arg(format!("--output={preview_frame}")),
        stdout: String::new(),
        stderr: String::new(),
        streams: Some(PipelineRenderReportStreams {
            video: true,
            audio: false,
        }),
        errors: Vec::new(),
        artifacts: artifacts.clone(),
        graphics: capture_graphics.reports,
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };
    write_json_report(&project_dir.join(&json_report), &render_report)?;

    let project_render_report = ProjectRenderReport {
        schema_version: 1,
        id: capture.job_id.to_string(),
        status: RenderReportStatus::Completed,
        output_path: preview_frame.clone(),
        duration_seconds: capture.frame_end_seconds - capture.playhead_seconds,
        quality: None,
        requested_width: Some(width),
        requested_height: Some(height),
        actual_width: Some(width),
        actual_height: Some(height),
        streams: RenderReportStreams {
            video: true,
            audio: false,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Skipped),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Skipped,
            ),
            (
                "overlayTiming".to_string(),
                RenderReportCheckStatus::Skipped,
            ),
            (
                "visualFrameEvidence".to_string(),
                RenderReportCheckStatus::Passed,
            ),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts,
        preview_comparison_request: None,
        preview_comparison: None,
        log_path,
        created_at: capture.updated_at.to_string(),
    };
    if !capture.cancellation.begin_completion() {
        return Err(vec![capture_error(
            "canonicalPreview.cancelled",
            "The canonical preview capture was cancelled.",
        )]);
    }
    let write = capture
        .identity
        .persist(
            project_dir,
            capture.job_id,
            capture.attempt_id,
            vec![
                ProjectAction::AttachRenderReport {
                    report: project_render_report.clone(),
                },
                ProjectAction::UpdateJobStatus {
                    job_id: capture.job_id.to_string(),
                    status: JobStatus::Completed,
                    updated_at: capture.updated_at.to_string(),
                    run_id: Some(capture.attempt_id.to_string()),
                },
            ],
            true,
        )
        .map_err(|error| {
            render_project_action_error(
                RENDER_LABEL,
                "Persisting capture completion failed.",
                error,
            )
        })?;

    Ok(PreparedPreviewFrameResult {
        project: write.project,
        playhead_seconds: capture.playhead_seconds,
        source_output: preview_frame.clone(),
        preview_frame,
        evidence_report,
        render_report,
        project_render_report,
    })
}

/// A capture-local copy of the prepared project the sampler can draw. It is never saved.
///
/// - Completed generations on video tracks read their first output media, as GES does.
/// - Generated media with a frame rate is video; without one it is an image.
/// - Image clips, text sources, unfinished generations and unexpanded nested timelines
///   active at the playhead are refused plainly.
fn normalize_capture_sources(
    project: &VideoProject,
    playhead_seconds: f64,
    frame_end_seconds: f64,
) -> PipelineResult<VideoProject> {
    let mut normalized = project.clone();
    for media in &mut normalized.media {
        if media.kind == MediaKind::Generated && media.fps.is_some() {
            media.kind = MediaKind::Video;
        }
    }
    let media = normalized.media.clone();
    let generated_assets = normalized.generated_assets.clone();
    for track in &mut normalized.timeline.tracks {
        if !track.enabled || track.kind != TrackKind::Video {
            continue;
        }
        let transitions = track.transitions.clone();
        for item in &mut track.items {
            let head = transitions
                .iter()
                .filter(|transition| transition.right_item_id == item.id)
                .map(|transition| transition.duration_seconds / 2.0)
                .fold(0.0, f64::max);
            let tail = transitions
                .iter()
                .filter(|transition| transition.left_item_id == item.id)
                .map(|transition| transition.duration_seconds / 2.0)
                .fold(0.0, f64::max);
            let active = item.start_seconds - head < frame_end_seconds
                && item.start_seconds + item.duration_seconds + tail > playhead_seconds;
            if let TimelineSource::Generated { artifact_id } = &item.source {
                let output = generated_assets
                    .iter()
                    .find(|asset| asset.id == *artifact_id)
                    .filter(|asset| asset.status == GeneratedAssetStatus::Completed)
                    .and_then(|asset| asset.outputs.first());
                match output {
                    Some(output) => {
                        item.source = TimelineSource::Media {
                            media_id: output.media_id.clone(),
                        }
                    }
                    None if active => return Err(vec![unsupported_source_error(&item.id)]),
                    None => {}
                }
            }
            if !active {
                continue;
            }
            let supported = match &item.source {
                TimelineSource::Media { media_id } => media
                    .iter()
                    .find(|asset| asset.id == *media_id)
                    .is_none_or(|asset| {
                        !matches!(asset.kind, MediaKind::Image | MediaKind::Generated)
                    }),
                TimelineSource::Generated { .. }
                | TimelineSource::Text { .. }
                | TimelineSource::Timeline { .. } => false,
            };
            if !supported {
                return Err(vec![unsupported_source_error(&item.id)]);
            }
        }
    }
    Ok(normalized)
}

fn unsupported_source_error(item_id: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "canonicalPreview.unsupportedSource",
        "Result frames on Linux can't show image clips, text sources or unfinished generations yet.",
        "Open the viewer to check this moment.",
    )
    .with_detail("itemId", item_id.to_string())
}

fn capture_error(path: &str, message: &str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        path,
        message,
        "Open the viewer to check this moment, then retry the capture.",
    )
}

#[cfg(test)]
#[path = "canonical_capture_tests.rs"]
mod tests;
