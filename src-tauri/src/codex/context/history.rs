use super::media::{
    canonical_prefix, generated_asset_status_code, media_kind_code, push_truncation_marker,
};
use super::project_files::empty_context_label;
use super::timeline::compact_context_text;
use super::{
    MAX_CONTEXT_EXPORT_ARTIFACTS, MAX_CONTEXT_GENERATED_ASSETS, MAX_CONTEXT_GENERATED_OUTPUTS,
    MAX_CONTEXT_JOBS, MAX_CONTEXT_RENDER_REPORTS,
};
use crate::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetSettings, JobStatus, JobSummary,
    ProjectExportArtifact, ProjectExportArtifactKind, ProjectRenderReport, RenderReportCheckStatus,
    RenderReportStatus, VideoProject,
};
use std::collections::BTreeMap;

pub(super) fn generated_assets_summary(project: &VideoProject) -> String {
    generated_assets_summary_for(
        project,
        &canonical_prefix(&project.generated_assets, MAX_CONTEXT_GENERATED_ASSETS),
    )
}

pub(super) fn generated_assets_summary_for(
    project: &VideoProject,
    assets: &[&GeneratedAsset],
) -> String {
    let mut lines = vec![
        "Use generated asset ids for parentAssetId, retryOfAssetId, updateGeneratedAssetStatus assetId, and completeGeneratedAsset assetId; completeGeneratedAsset can include replacement { itemId, mediaId } when the completed output should swap a timeline clip, and replaceTimelineItemWithGeneratedOutput remains available for already-completed outputs.".to_string(),
    ];
    let mut output_count = 0usize;

    if project.generated_assets.is_empty() {
        lines.push("- none".to_string());
        return lines.join("\n");
    }

    for asset in assets {
        lines.push(generated_asset_summary_line(asset));
        for output in &asset.outputs {
            if output_count >= MAX_CONTEXT_GENERATED_OUTPUTS {
                break;
            }
            lines.push(generated_output_summary_line(output));
            output_count += 1;
        }
        if asset.outputs.is_empty() {
            lines.push(format!("- outputs {}: none", asset.id));
        }
    }

    push_truncation_marker(
        &mut lines,
        project.generated_assets.len(),
        assets.len(),
        "generated assets",
    );

    let total_outputs = project
        .generated_assets
        .iter()
        .map(|asset| asset.outputs.len())
        .sum::<usize>();
    if total_outputs > output_count {
        lines.push(format!(
            "- [{} generated outputs truncated]",
            total_outputs - output_count
        ));
    }

    lines.join("\n")
}

fn generated_asset_summary_line(asset: &GeneratedAsset) -> String {
    format!(
        "- asset {}: {} | {} | model: {}/{} | settings: {} | prompt: {} | firstFrame: {} | lastFrame: {} | references: {} | parent: {} | retryOf: {}",
        asset.id,
        media_kind_code(&asset.kind),
        generated_asset_status_code(&asset.status),
        compact_context_text(&asset.model.provider),
        compact_context_text(&asset.model.id),
        generated_settings_label(&asset.settings),
        compact_context_text(&asset.prompt),
        option_context_label(asset.references.first_frame_media_id.as_deref()),
        option_context_label(asset.references.last_frame_media_id.as_deref()),
        list_context_label(&asset.references.media_ids),
        option_context_label(asset.parent_asset_id.as_deref()),
        option_context_label(asset.retry_of_asset_id.as_deref())
    )
}

fn generated_settings_label(settings: &GeneratedAssetSettings) -> String {
    let dimensions = match (settings.width, settings.height) {
        (Some(width), Some(height)) => format!("{width}x{height}"),
        _ => "size none".to_string(),
    };
    let duration = settings
        .duration_seconds
        .map(|duration| format!("{duration:.3}s"))
        .unwrap_or_else(|| "duration none".to_string());
    let fps = settings
        .fps
        .map(|fps| format!("{fps:.3}fps"))
        .unwrap_or_else(|| "fps none".to_string());
    let aspect_ratio = settings
        .aspect_ratio
        .as_deref()
        .map(compact_context_text)
        .unwrap_or_else(|| "aspect none".to_string());

    format!("{dimensions}, {duration}, {fps}, aspect {aspect_ratio}")
}

fn generated_output_summary_line(output: &GeneratedAssetOutput) -> String {
    format!(
        "- output {}: {} | {}x{} | {:.3}s | fps: {:.3}",
        output.media_id,
        output.relative_path,
        output.width,
        output.height,
        output.duration_seconds,
        output.fps
    )
}

pub(super) fn render_reports_summary(project: &VideoProject) -> String {
    render_reports_summary_for(
        project,
        &canonical_prefix(&project.render_reports, MAX_CONTEXT_RENDER_REPORTS),
    )
}

pub(super) fn render_reports_summary_for(
    project: &VideoProject,
    reports: &[&ProjectRenderReport],
) -> String {
    let mut lines = vec![
        "Use attachRenderReport for new render-review artifacts; preserve outputPath, streams, checks, artifact paths, and logPath.".to_string(),
    ];

    if project.render_reports.is_empty() {
        lines.push("- none".to_string());
        return lines.join("\n");
    }

    for report in reports {
        lines.push(render_report_summary_line(report));
    }
    push_truncation_marker(
        &mut lines,
        project.render_reports.len(),
        reports.len(),
        "render reports",
    );

    lines.join("\n")
}

pub(super) fn workflow_jobs_summary(project: &VideoProject) -> String {
    workflow_jobs_summary_for(project, &canonical_prefix(&project.jobs, MAX_CONTEXT_JOBS))
}

pub(super) fn workflow_jobs_summary_for(project: &VideoProject, jobs: &[&JobSummary]) -> String {
    let mut lines = vec![
        "Use recordJob before starting durable work, updateJobStatus as Temporal workflows progress, and updateGeneratedAssetStatus when generation assets move between queued, running, failed, or completed.".to_string(),
    ];

    if project.jobs.is_empty() {
        lines.push("- none".to_string());
        return lines.join("\n");
    }

    for job in jobs {
        lines.push(workflow_job_summary_line(job));
    }
    push_truncation_marker(&mut lines, project.jobs.len(), jobs.len(), "workflow jobs");

    lines.join("\n")
}

pub(super) fn export_artifacts_summary(project: &VideoProject) -> String {
    export_artifacts_summary_for(
        project,
        &canonical_prefix(&project.export_artifacts, MAX_CONTEXT_EXPORT_ARTIFACTS),
    )
}

pub(super) fn export_artifacts_summary_for(
    project: &VideoProject,
    artifacts: &[&ProjectExportArtifact],
) -> String {
    let mut lines = vec![
        "Use recordExportArtifact after creating durable export outputs; keep paths project-relative under exports/.".to_string(),
    ];

    if project.export_artifacts.is_empty() {
        lines.push("- none".to_string());
        return lines.join("\n");
    }

    for artifact in artifacts {
        lines.push(export_artifact_summary_line(artifact));
    }
    push_truncation_marker(
        &mut lines,
        project.export_artifacts.len(),
        artifacts.len(),
        "export artifacts",
    );

    lines.join("\n")
}

pub(super) fn export_capabilities_summary() -> String {
    [
        "Supported exports: draft WebM, final WebM, Premiere XMEML, DaVinci FCPXML.",
        "NLE XML command: export_nle_xml_to_split_project_folder(format: premiereXmeml | davinciFcpxml).",
        "Temporal workflow: export_nle_xml uses VideoCreaterExportNleXmlWorkflow on video-creater-workflows.",
        "NLE XML workflow activities: BuildNleXml, ValidateNleXml, WriteExportArtifact, AttachExportReport.",
        NATIVE_DELIVERY_EXPORT_GUIDANCE,
    ]
    .join("\n")
}

#[cfg(target_os = "macos")]
const NATIVE_DELIVERY_EXPORT_GUIDANCE: &str = "MP4/H.264/H.265/ProRes: use the bundled macOS AVFoundation exporter when available; reviewed GStreamer factories remain the compatibility fallback.";

#[cfg(not(target_os = "macos"))]
const NATIVE_DELIVERY_EXPORT_GUIDANCE: &str = "MP4/H.264/H.265/ProRes: use the reviewed GStreamer encoders; H.264 uses a VA-API hardware encoder or OpenH264 with FFmpeg AAC audio, ProRes uses the FFmpeg ProRes encoder with PCM audio, and H.265 is available only when a VA-API hardware encoder is present. Check the export profile availability report before proposing MP4 or ProRes delivery.";

fn export_artifact_summary_line(artifact: &ProjectExportArtifact) -> String {
    format!(
        "- export {}: {} | {} | path: {} | mimeType: {} | job: {} | created: {}",
        artifact.id,
        export_artifact_kind_code(&artifact.kind),
        compact_context_text(&artifact.format),
        artifact.path,
        artifact.mime_type,
        option_context_label(artifact.job_id.as_deref()),
        artifact.created_at
    )
}

fn export_artifact_kind_code(kind: &ProjectExportArtifactKind) -> &'static str {
    match kind {
        ProjectExportArtifactKind::NleXml => "nle_xml",
        ProjectExportArtifactKind::Webm => "webm",
        ProjectExportArtifactKind::Mp4 => "mp4",
        ProjectExportArtifactKind::Mov => "mov",
        ProjectExportArtifactKind::ProjectBundle => "project_bundle",
    }
}

fn workflow_job_summary_line(job: &JobSummary) -> String {
    let workflow = job
        .workflow
        .as_ref()
        .map(|workflow| {
            format!(
                "{} @ {} | workflowId: {} | runId: {} | activities: {}",
                workflow.workflow_type,
                workflow.task_queue,
                workflow.workflow_id,
                workflow.run_id.as_deref().unwrap_or("none"),
                workflow.activity_types.join(", ")
            )
        })
        .unwrap_or_else(|| "none".to_string());

    format!(
        "- job {}: {} | {} | updated: {} | workflow: {}",
        job.id,
        job.kind,
        job_status_code(&job.status),
        job.updated_at,
        workflow
    )
}

fn job_status_code(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Progress => "progress",
        JobStatus::Blocked => "blocked",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
        JobStatus::Completed => "completed",
    }
}

fn render_report_summary_line(report: &ProjectRenderReport) -> String {
    format!(
        "- report {}: {} | output: {} | duration: {:.3}s | streams: video={},audio={} | checks: {} | visualEvidence: {} | artifacts: {} | log: {}",
        report.id,
        render_report_status_code(&report.status),
        report.output_path,
        report.duration_seconds,
        report.streams.video,
        report.streams.audio,
        render_checks_context_label(&report.checks),
        render_visual_evidence_context_label(&report.artifacts),
        list_context_label(&report.artifacts),
        empty_context_label(&report.log_path)
    )
}

fn render_checks_context_label(checks: &BTreeMap<String, RenderReportCheckStatus>) -> String {
    if checks.is_empty() {
        return "none".to_string();
    }

    checks
        .iter()
        .map(|(key, status)| {
            format!(
                "{}={}",
                compact_context_text(key),
                render_report_check_status_code(status)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_report_status_code(status: &RenderReportStatus) -> &'static str {
    match status {
        RenderReportStatus::Queued => "queued",
        RenderReportStatus::Running => "running",
        RenderReportStatus::Failed => "failed",
        RenderReportStatus::Completed => "completed",
    }
}

fn render_report_check_status_code(status: &RenderReportCheckStatus) -> &'static str {
    match status {
        RenderReportCheckStatus::Passed => "passed",
        RenderReportCheckStatus::Failed => "failed",
        RenderReportCheckStatus::Skipped => "skipped",
    }
}

fn render_visual_evidence_context_label(artifacts: &[String]) -> String {
    let frame_artifacts = artifacts
        .iter()
        .filter(|artifact| is_render_frame_artifact(artifact))
        .count();
    if frame_artifacts == 0 {
        "frameArtifacts=0".to_string()
    } else {
        format!("frameArtifacts={frame_artifacts}")
    }
}

fn is_render_frame_artifact(artifact: &str) -> bool {
    let normalized = artifact.replace('\\', "/");
    normalized.contains("/frames/frame-") && normalized.ends_with(".png")
}

fn option_context_label(value: Option<&str>) -> String {
    value
        .map(compact_context_text)
        .unwrap_or_else(|| "none".to_string())
}

fn list_context_label(values: &[String]) -> String {
    if values.is_empty() {
        return "none".to_string();
    }

    values
        .iter()
        .map(|value| compact_context_text(value))
        .collect::<Vec<_>>()
        .join(", ")
}
