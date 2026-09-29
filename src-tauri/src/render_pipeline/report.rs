use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::process::CommandSpec;
use crate::edit::render_plan::RenderQuality;

const MARKDOWN_ARTIFACT_LIMIT: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderReport {
    pub job_id: String,
    pub summary: RenderReportSummary,
    pub command: CommandSpec,
    pub stdout: String,
    pub stderr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streams: Option<RenderReportStreams>,
    pub errors: Vec<PipelineError>,
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub graphics: Vec<RenderGraphicsReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<RenderPerformanceSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_comparison_request: Option<RenderPreviewComparisonRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_comparison: Option<RenderPreviewComparison>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderReportSummary {
    pub status: String,
    pub duration_seconds: Option<f64>,
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<RenderQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_codec: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderReportStreams {
    pub video: bool,
    pub audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraphicsReport {
    pub layer_id: String,
    pub renderer: String,
    pub quality_profile: Option<String>,
    pub template_id: Option<String>,
    pub motion_preset_id: Option<String>,
    pub visual_qa_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_status: Option<String>,
    #[serde(default)]
    pub sampled_frames: Vec<String>,
    #[serde(default)]
    pub qa_metrics: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderPerformanceSummary {
    pub total_duration_ms: u128,
    pub stages: Vec<RenderStageReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderPreviewComparison {
    pub status: String,
    pub compared_frames: Vec<RenderPreviewComparisonFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderPreviewComparisonRequest {
    pub status: String,
    pub project_dir: String,
    pub project_report_id: String,
    pub render_report_path: String,
    pub rendered_video: String,
    pub duration_seconds: f64,
    pub frame_time_seconds: f64,
    pub rendered_frames: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fail_on_mismatch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderPreviewComparisonFrame {
    pub timeline_seconds: f64,
    pub preview_frame: String,
    pub rendered_frame: String,
    pub diff_frame: Option<String>,
    pub mismatch_ratio: f64,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderStageReport {
    pub name: String,
    pub status: String,
    pub duration_ms: u128,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

pub fn write_json_report(path: &Path, report: &RenderReport) -> PipelineResult<()> {
    let json = serde_json::to_string_pretty(report).map_err(|error| {
        report_write_error(
            path,
            "Serialize render report JSON failed.",
            "Fix non-serializable report fields and retry writing the report.",
            error,
        )
    })?;

    std::fs::write(path, json).map_err(|error| {
        report_write_error(
            path,
            "Write render report JSON failed.",
            "Ensure the report directory exists and is writable.",
            error,
        )
    })
}

pub fn write_markdown_report(path: &Path, report: &RenderReport) -> PipelineResult<()> {
    std::fs::write(path, markdown_report(report)).map_err(|error| {
        report_write_error(
            path,
            "Write render report Markdown failed.",
            "Ensure the report directory exists and is writable.",
            error,
        )
    })
}

fn markdown_report(report: &RenderReport) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# Render Report: {}\n\n", report.job_id));
    markdown.push_str("## Summary\n\n");
    markdown.push_str(&format!("- Status: {}\n", report.summary.status));
    if let Some(duration_seconds) = report.summary.duration_seconds {
        markdown.push_str(&format!("- Duration: {:.3}s\n", duration_seconds));
    }
    if let Some(output_path) = &report.summary.output_path {
        markdown.push_str(&format!("- Output: {output_path}\n"));
    }
    if let Some(quality) = report.summary.quality {
        markdown.push_str(&format!("- Quality: {}\n", render_quality_name(quality)));
    }
    if let (Some(width), Some(height)) = (
        report.summary.requested_width,
        report.summary.requested_height,
    ) {
        markdown.push_str(&format!("- Requested dimensions: {width}x{height}\n"));
    }
    if let (Some(width), Some(height)) = (report.summary.actual_width, report.summary.actual_height)
    {
        markdown.push_str(&format!("- Actual dimensions: {width}x{height}\n"));
    }

    markdown.push_str("\n## Command\n\n");
    markdown.push_str(&format!("`{}`\n", report.command.display()));

    if let Some(performance) = &report.performance {
        markdown.push_str("\n## Performance\n\n");
        markdown.push_str(&format!("- Total: {}ms\n\n", performance.total_duration_ms));
        markdown.push_str("| Stage | Status | Duration | Details |\n");
        markdown.push_str("| --- | --- | ---: | --- |\n");
        for stage in &performance.stages {
            let details = stage
                .details
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", ");
            markdown.push_str(&format!(
                "| {} | {} | {}ms | {} |\n",
                stage.name, stage.status, stage.duration_ms, details
            ));
        }
    }

    if !report.artifacts.is_empty() {
        markdown.push_str("\n## Artifacts\n\n");
        for artifact in report.artifacts.iter().take(MARKDOWN_ARTIFACT_LIMIT) {
            markdown.push_str(&format!("- {artifact}\n"));
        }
        if report.artifacts.len() > MARKDOWN_ARTIFACT_LIMIT {
            let omitted = report.artifacts.len() - MARKDOWN_ARTIFACT_LIMIT;
            markdown.push_str(&format!(
                "- ... {omitted} more artifacts omitted from Markdown; see JSON report for the complete list.\n"
            ));
        }
    }

    if let Some(streams) = &report.streams {
        markdown.push_str("\n## Streams\n\n");
        markdown.push_str(&format!("- video={}\n", streams.video));
        markdown.push_str(&format!("- audio={}\n", streams.audio));
    }

    if !report.graphics.is_empty() {
        markdown.push_str("\n## Graphics\n\n");
        for graphics in &report.graphics {
            markdown.push_str(&format!(
                "- {}: renderer={} profile={} template={} preset={} qa={}\n",
                graphics.layer_id,
                graphics.renderer,
                graphics.quality_profile.as_deref().unwrap_or("none"),
                graphics.template_id.as_deref().unwrap_or("none"),
                graphics.motion_preset_id.as_deref().unwrap_or("none"),
                graphics.visual_qa_status.as_deref().unwrap_or("not-run")
            ));
            if let Some(cache_status) = &graphics.cache_status {
                markdown.push_str(&format!("  - cache={cache_status}\n"));
            }
            if !graphics.sampled_frames.is_empty() {
                markdown.push_str(&format!(
                    "  - samples={}\n",
                    graphics.sampled_frames.join(", ")
                ));
            }
            if !graphics.qa_metrics.is_empty() {
                let metrics = graphics
                    .qa_metrics
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                markdown.push_str(&format!("  - qaMetrics={metrics}\n"));
            }
        }
    }

    if let Some(comparison) = &report.preview_comparison {
        markdown.push_str("\n## Preview/Render Comparison\n\n");
        let matched = comparison
            .compared_frames
            .iter()
            .filter(|frame| frame.passed)
            .count();
        markdown.push_str(&format!("- Status: {}\n", comparison.status));
        markdown.push_str(&format!(
            "- Matched Frames: {matched}/{}\n",
            comparison.compared_frames.len()
        ));
        for frame in &comparison.compared_frames {
            let diff = frame.diff_frame.as_deref().unwrap_or("none");
            markdown.push_str(&format!(
                "- {:.3}s mismatch={:.6} preview={} rendered={} diff={}\n",
                frame.timeline_seconds,
                frame.mismatch_ratio,
                frame.preview_frame,
                frame.rendered_frame,
                diff
            ));
        }
    }

    if let Some(request) = &report.preview_comparison_request {
        markdown.push_str("\n## Preview/Render QA Request\n\n");
        markdown.push_str(&format!("- Status: {}\n", request.status));
        markdown.push_str(&format!("- Project: {}\n", request.project_dir));
        markdown.push_str(&format!("- Report: {}\n", request.render_report_path));
        markdown.push_str(&format!(
            "- Frame Time: {:.3}s\n",
            request.frame_time_seconds
        ));
        if !request.rendered_frames.is_empty() {
            markdown.push_str(&format!(
                "- Rendered Frames: {}\n",
                request.rendered_frames.join(", ")
            ));
        }
        markdown.push_str("- Runner: bundled native preview review\n");
    }

    if !report.errors.is_empty() {
        markdown.push_str("\n## Errors\n\n");
        for error in &report.errors {
            markdown.push_str(&format!(
                "- {:?}: {} Fix: {}\n",
                error.code, error.message, error.fix
            ));
        }
    }

    if !report.stderr.trim().is_empty() {
        markdown.push_str("\n## Stderr\n\n```text\n");
        markdown.push_str(&report.stderr);
        markdown.push_str("\n```\n");
    }

    if !report.stdout.trim().is_empty() {
        markdown.push_str("\n## Stdout\n\n```text\n");
        markdown.push_str(&report.stdout);
        markdown.push_str("\n```\n");
    }

    markdown
}

fn render_quality_name(quality: RenderQuality) -> &'static str {
    match quality {
        RenderQuality::Draft => "draft",
        RenderQuality::Final => "final",
    }
}

fn report_write_error(
    path: &Path,
    message: impl Into<String>,
    fix: impl Into<String>,
    error: impl std::fmt::Display,
) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineReportWriteFailed,
        "renderReport.path",
        message,
        fix,
    )
    .with_detail("path", path.display().to_string())
    .with_detail("ioError", error.to_string())]
}
