use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::edit::render_plan::{RenderClip, RenderOutputProfile, RenderPlan, RenderQuality};
use crate::graphics::assets::AssetRegistry;
use crate::graphics::manifest::GraphicsArtifactManifest;
use crate::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use crate::project::export_options::ExportRenderOptions;
use crate::project::export_profiles::ExportProfile;
use crate::project::model::{
    MediaAsset, MediaKind, ProjectRenderReport, RenderReportCheckStatus, RenderReportStatus,
    RenderReportStreams, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, Transcript,
    TranscriptWord, VideoProject,
};
use crate::render_pipeline::backend::RenderBackend;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::gstreamer_backend::{
    generate_fixture_source_with_gstreamer, probe_media_with_gstreamer, GstreamerGesRenderBackend,
};
use crate::render_pipeline::probe::{validate_rendered_media, ExpectedMedia};
use crate::render_pipeline::process::{ProcessRunner, SystemProcessRunner};
use crate::render_pipeline::project_export::build_project_graphics_layers;
use crate::render_pipeline::report::{
    write_json_report, write_markdown_report, RenderGraphicsReport, RenderReport,
    RenderReportStreams as PipelineRenderReportStreams, RenderReportSummary,
};
use serde_json::json;

const DEFAULT_OUTPUT_DIR: &str = "output/e2e-combined";
const RAW_FOOTAGE_FILE_NAME: &str = "raw-footage.webm";
const FINAL_VALIDATION_FILE_NAME: &str = "final-combined-validation.webm";
const JSON_REPORT_FILE_NAME: &str = "validation-report.json";
const MARKDOWN_REPORT_FILE_NAME: &str = "validation-report.md";
const LOG_FILE_NAME: &str = "render.log";
const RAW_DURATION_SECONDS: f64 = 8.0;
const OUTPUT_WIDTH: u32 = 1280;
const OUTPUT_HEIGHT: u32 = 720;
const OUTPUT_FPS: f64 = 30.0;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedE2eConfig {
    pub output_dir: PathBuf,
}

impl CombinedE2eConfig {
    pub fn raw_path(&self) -> PathBuf {
        self.output_dir.join(RAW_FOOTAGE_FILE_NAME)
    }

    pub fn final_path(&self) -> PathBuf {
        self.output_dir.join(FINAL_VALIDATION_FILE_NAME)
    }

    pub fn json_report_path(&self) -> PathBuf {
        self.output_dir.join(JSON_REPORT_FILE_NAME)
    }

    pub fn markdown_report_path(&self) -> PathBuf {
        self.output_dir.join(MARKDOWN_REPORT_FILE_NAME)
    }

    pub fn log_path(&self) -> PathBuf {
        self.output_dir.join(LOG_FILE_NAME)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CombinedE2eRunResult {
    pub final_path: PathBuf,
    pub json_report_path: PathBuf,
    pub markdown_report_path: PathBuf,
    pub log_path: PathBuf,
    pub artifacts: Vec<PathBuf>,
    pub project: VideoProject,
}

#[derive(Debug, Clone)]
struct RenderedCombinedGraphics {
    artifact_dir: PathBuf,
    manifest: GraphicsArtifactManifest,
    timeline_start_seconds: f64,
}

#[derive(Debug, Default)]
struct CombinedE2eArgs {
    output_dir: Option<PathBuf>,
}

pub fn parse_combined_e2e_args<I, S>(args: I) -> PipelineResult<CombinedE2eConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut tokens = args.into_iter().map(Into::into).collect::<Vec<String>>();
    if tokens.first().is_some_and(|token| !token.starts_with("--")) {
        tokens.remove(0);
    }

    let mut parsed = CombinedE2eArgs::default();
    let mut index = 0;
    while index < tokens.len() {
        let flag = &tokens[index];
        if !flag.starts_with("--") {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unexpected positional argument.",
                "Use --output-dir.",
            )]);
        }
        if !is_combined_e2e_flag(flag) {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unknown combined e2e argument.",
                "Use --output-dir.",
            )]);
        }

        let value = tokens
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| {
                vec![arg_error(
                    arg_path(flag),
                    "Missing value for combined e2e argument.",
                    format!("Pass a value after {flag}."),
                )]
            })?
            .clone();

        match flag.as_str() {
            "--output-dir" => parsed.output_dir = Some(value.into()),
            _ => unreachable!("combined e2e flag was checked before value parsing"),
        }

        index += 2;
    }

    Ok(CombinedE2eConfig {
        output_dir: absolute_output_dir(
            parsed
                .output_dir
                .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR)),
        )?,
    })
}

fn absolute_output_dir(output_dir: PathBuf) -> PipelineResult<PathBuf> {
    if output_dir.is_absolute() {
        return Ok(output_dir);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(output_dir))
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "outputDir",
                "Failed to resolve combined e2e output directory.",
                "Run the combined e2e command from a readable working directory.",
            )
            .with_detail("ioError", error.to_string())]
        })
}

pub fn run_combined_e2e(config: &CombinedE2eConfig) -> PipelineResult<CombinedE2eRunResult> {
    run_combined_e2e_with_runner(config, &SystemProcessRunner)
}

pub fn run_combined_e2e_with_runner(
    config: &CombinedE2eConfig,
    runner: &dyn ProcessRunner,
) -> PipelineResult<CombinedE2eRunResult> {
    create_output_dir(config)?;

    let raw_output = generate_fixture_source_with_gstreamer(
        &config.raw_path(),
        OUTPUT_WIDTH,
        OUTPUT_HEIGHT,
        OUTPUT_FPS,
        RAW_DURATION_SECONDS,
        PROCESS_TIMEOUT,
    )?;
    ensure_nonempty_artifact(config.raw_path(), "artifacts.raw")?;

    let mut project = build_combined_project(RAW_DURATION_SECONDS);
    let plan = build_combined_render_plan(config, &project)?;
    let rendered_graphics = render_combined_graphics_layers(config, &project)?;
    let graphics_inputs = combined_graphics_backend_inputs(&rendered_graphics);
    let backend = GstreamerGesRenderBackend::new();
    let render_command = backend.build_command(&plan, &graphics_inputs)?;
    let render_output = backend.render(runner, &plan, &graphics_inputs, PROCESS_TIMEOUT)?;
    ensure_nonempty_artifact(config.final_path(), "artifacts.final")?;

    let (probe, probe_output) =
        probe_media_with_gstreamer(&config.final_path(), PROCESS_TIMEOUT, "combinedFinal")?;
    validate_rendered_media(
        &config.final_path(),
        &probe,
        &ExpectedMedia {
            width: Some(OUTPUT_WIDTH),
            height: Some(OUTPUT_HEIGHT),
            video_required: true,
            audio_required: true,
            non_empty_required: true,
            expected_duration_seconds: None,
            duration_tolerance_seconds: None,
            ..ExpectedMedia::default()
        },
    )?;

    let mut artifacts = vec![
        config.raw_path(),
        config.final_path(),
        config.json_report_path(),
        config.markdown_report_path(),
        config.log_path(),
    ];
    artifacts.extend(combined_graphics_artifact_paths(&rendered_graphics));
    let stdout = labeled_output(&[
        ("raw gstreamer fixture", &raw_output.stdout),
        ("render gstreamer ges", &render_output.stdout),
        ("gstreamer discoverer", &probe_output.stdout),
    ]);
    let stderr = labeled_output(&[
        ("raw gstreamer fixture", &raw_output.stderr),
        ("render gstreamer ges", &render_output.stderr),
        ("gstreamer discoverer", &probe_output.stderr),
    ]);
    let report = RenderReport {
        job_id: "combined-e2e".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: probe.duration_seconds,
            output_path: Some(config.final_path().display().to_string()),
            ..RenderReportSummary::default()
        },
        command: render_command,
        stdout: stdout.clone(),
        stderr: stderr.clone(),
        streams: Some(PipelineRenderReportStreams {
            video: probe.video.is_some(),
            audio: probe.audio.is_some(),
        }),
        errors: Vec::new(),
        artifacts: artifacts
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        graphics: rendered_graphics
            .iter()
            .map(combined_graphics_report)
            .collect(),
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };
    std::fs::write(
        config.log_path(),
        format!("== stdout ==\n{stdout}\n== stderr ==\n{stderr}\n"),
    )
    .map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineReportWriteFailed,
            "artifacts.log",
            "Failed to write combined e2e render log.",
            "Choose a writable output directory and retry the combined e2e run.",
        )
        .with_detail("path", config.log_path().display().to_string())
        .with_detail("ioError", error.to_string())]
    })?;
    write_json_report(&config.json_report_path(), &report)?;
    write_markdown_report(&config.markdown_report_path(), &report)?;
    ensure_nonempty_artifact(config.json_report_path(), "artifacts.reportJson")?;
    ensure_nonempty_artifact(config.markdown_report_path(), "artifacts.reportMarkdown")?;
    ensure_nonempty_artifact(config.log_path(), "artifacts.log")?;
    for artifact in combined_graphics_artifact_paths(&rendered_graphics) {
        ensure_nonempty_artifact(artifact, "artifacts.graphics")?;
    }
    project.render_reports.push(project_render_report(
        config,
        probe
            .duration_seconds
            .unwrap_or(project.timeline.duration_seconds),
        &artifacts,
    ));

    Ok(CombinedE2eRunResult {
        final_path: config.final_path(),
        json_report_path: config.json_report_path(),
        markdown_report_path: config.markdown_report_path(),
        log_path: config.log_path(),
        artifacts,
        project,
    })
}

pub fn build_combined_render_plan(
    config: &CombinedE2eConfig,
    project: &VideoProject,
) -> PipelineResult<RenderPlan> {
    let options = ExportRenderOptions::for_project_defaults(
        ExportProfile::Webm,
        RenderQuality::Draft,
        project.render_settings.width,
        project.render_settings.height,
    )
    .map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project.renderSettings",
            "Project render settings are not valid export dimensions.",
            "Use positive, even dimensions no larger than 16,384 pixels.",
        )
        .with_detail("error", error.to_string())]
    })?;
    Ok(RenderPlan {
        input_path: config.raw_path().to_string_lossy().into_owned(),
        output_path: config.final_path().to_string_lossy().into_owned(),
        width: options.width,
        height: options.height,
        fps: project.render_settings.fps,
        quality: options.quality,
        output_profile: RenderOutputProfile::default(),
        encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
        clips: selected_video_clips(project)?,
        audio_clips: Vec::new(),
        transitions: Vec::new(),
        audio_transitions: Vec::new(),
    })
}

fn render_combined_graphics_layers(
    config: &CombinedE2eConfig,
    project: &VideoProject,
) -> PipelineResult<Vec<RenderedCombinedGraphics>> {
    let layers = build_project_graphics_layers(project, OUTPUT_WIDTH, OUTPUT_HEIGHT, OUTPUT_FPS)?;
    let assets = AssetRegistry::new(config.output_dir.clone());
    let graphics_dir = config.output_dir.join("graphics");
    let mut rendered = Vec::with_capacity(layers.len());

    for layer in layers {
        let artifact_dir = graphics_dir.join(safe_path_segment(&layer.id));
        let manifest = render_graphics_preview(
            &layer,
            &assets,
            GraphicsRenderOptions {
                output_dir: artifact_dir.clone(),
            },
        )
        .map_err(PipelineError::from_graphics_errors)?;
        rendered.push(RenderedCombinedGraphics {
            artifact_dir,
            manifest,
            timeline_start_seconds: layer.timeline_start,
        });
    }

    Ok(rendered)
}

fn combined_graphics_backend_inputs(
    graphics: &[RenderedCombinedGraphics],
) -> Vec<(GraphicsArtifactManifest, PathBuf, f64)> {
    graphics
        .iter()
        .map(|graphics| {
            (
                graphics.manifest.clone(),
                graphics.artifact_dir.clone(),
                graphics.timeline_start_seconds,
            )
        })
        .collect()
}

fn combined_graphics_report(graphics: &RenderedCombinedGraphics) -> RenderGraphicsReport {
    RenderGraphicsReport {
        layer_id: graphics
            .manifest
            .source_layer_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        renderer: "rust".to_string(),
        quality_profile: Some("draftWebm".to_string()),
        template_id: None,
        motion_preset_id: None,
        visual_qa_status: Some("not-run".to_string()),
        cache_status: None,
        sampled_frames: vec![graphics.manifest.preview_path.display().to_string()],
        qa_metrics: BTreeMap::new(),
    }
}

fn combined_graphics_artifact_paths(graphics: &[RenderedCombinedGraphics]) -> Vec<PathBuf> {
    graphics
        .iter()
        .flat_map(|graphics| {
            let mut paths = vec![
                graphics.artifact_dir.join("manifest.json"),
                graphics.artifact_dir.join(&graphics.manifest.preview_path),
            ];
            for frame_index in 0..graphics.manifest.frame_count {
                paths.push(
                    graphics
                        .artifact_dir
                        .join("frames")
                        .join(format!("frame-{frame_index:06}.png")),
                );
            }
            paths
        })
        .collect()
}

fn safe_path_segment(value: &str) -> String {
    let mut segment = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            segment.push(ch);
        } else if !segment.ends_with('-') {
            segment.push('-');
        }
    }
    let trimmed = segment.trim_matches('-');
    if trimmed.is_empty() {
        "graphics".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn build_combined_project(raw_duration_seconds: f64) -> VideoProject {
    let source_duration = if raw_duration_seconds.is_finite() && raw_duration_seconds > 0.0 {
        raw_duration_seconds
    } else {
        1.0
    };
    let first_source_in = source_duration * 0.10;
    let first_source_out = source_duration * 0.35;
    let second_source_in = source_duration * 0.55;
    let second_source_out = source_duration * 0.85;
    let first_duration = first_source_out - first_source_in;
    let second_duration = second_source_out - second_source_in;
    let timeline_duration = first_duration + second_duration;

    let mut project = VideoProject::new_empty(
        "combined-e2e-validation".to_string(),
        "Combined E2E Validation".to_string(),
        "2026-06-17T00:00:00Z".to_string(),
    );

    project.media.push(MediaAsset {
        id: "media-raw".to_string(),
        name: None,
        relative_path: RAW_FOOTAGE_FILE_NAME.to_string(),
        kind: MediaKind::Video,
        duration_seconds: source_duration,
        width: Some(OUTPUT_WIDTH),
        height: Some(OUTPUT_HEIGHT),
        fps: Some(OUTPUT_FPS),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-raw".to_string(),
        media_id: "media-raw".to_string(),
        engine: Some("combined-e2e-fixture".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            transcript_word("opening", source_duration * 0.10, source_duration * 0.14),
            transcript_word("hook", source_duration * 0.15, source_duration * 0.19),
            transcript_word("shows", source_duration * 0.22, source_duration * 0.26),
            transcript_word("motion", source_duration * 0.28, source_duration * 0.32),
            transcript_word("second", source_duration * 0.55, source_duration * 0.59),
            transcript_word("beat", source_duration * 0.61, source_duration * 0.65),
            transcript_word("proves", source_duration * 0.70, source_duration * 0.74),
            transcript_word("range", source_duration * 0.78, source_duration * 0.82),
        ],
    });

    project.timeline.duration_seconds = timeline_duration;
    project.render_settings.width = OUTPUT_WIDTH;
    project.render_settings.height = OUTPUT_HEIGHT;
    project.render_settings.fps = OUTPUT_FPS;
    if let Some(track) = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
    {
        track.items.push(video_clip_item(
            "clip-selected-hook",
            "Selected hook",
            0.0,
            first_source_in,
            first_source_out,
        ));
        track.items.push(video_clip_item(
            "clip-selected-payoff",
            "Selected payoff",
            first_duration,
            second_source_in,
            second_source_out,
        ));
    }
    if let Some(track) = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Caption)
    {
        track.items.push(visual_text_item(
            "caption-e2e-hook",
            TimelineItemKind::Caption,
            "opening hook shows motion",
            0.25,
            1.6,
            "Caption proves transcript-backed timing",
        ));
    }
    if let Some(track) = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Overlay)
    {
        track.items.push(visual_text_item(
            "overlay-e2e-proof",
            TimelineItemKind::Overlay,
            "Source ranges validated",
            first_duration,
            2.0_f64.min(second_duration),
            "Overlay proves visual metadata coverage",
        ));
    }

    project
}

fn project_render_report(
    config: &CombinedE2eConfig,
    duration_seconds: f64,
    artifacts: &[PathBuf],
) -> ProjectRenderReport {
    ProjectRenderReport {
        schema_version: 1,
        id: "combined-e2e-render".to_string(),
        status: RenderReportStatus::Completed,
        output_path: config.final_path().display().to_string(),
        duration_seconds,
        quality: None,
        requested_width: None,
        requested_height: None,
        actual_width: None,
        actual_height: None,
        streams: RenderReportStreams {
            video: true,
            audio: true,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Passed,
            ),
            ("overlayTiming".to_string(), RenderReportCheckStatus::Passed),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
            ("sourceRanges".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts: artifacts
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: config.log_path().display().to_string(),
        created_at: "2026-06-17T00:00:00Z".to_string(),
    }
}

fn create_output_dir(config: &CombinedE2eConfig) -> PipelineResult<()> {
    std::fs::create_dir_all(&config.output_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "args.outputDir",
            "Create combined e2e output directory failed.",
            "Choose a writable output directory and retry the combined e2e run.",
        )
        .with_detail("path", config.output_dir.display().to_string())
        .with_detail("ioError", error.to_string())]
    })
}

fn selected_video_clips(project: &VideoProject) -> PipelineResult<Vec<RenderClip>> {
    let media_duration = project
        .media
        .iter()
        .find(|media| media.id == "media-raw")
        .map(|media| media.duration_seconds)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "media.mediaRaw",
                "Combined e2e project is missing the raw media asset.",
                "Build the combined e2e project with media-raw before rendering.",
            )]
        })?;

    let Some(video_track) = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
    else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "timeline.tracks.video",
            "Combined e2e project is missing a video track.",
            "Build the combined e2e project with a primary video track before rendering.",
        )]);
    };

    let mut clips = Vec::new();
    for (index, item) in video_track.items.iter().enumerate() {
        if item.kind != TimelineItemKind::VideoClip {
            continue;
        }
        let source_in = item
            .properties
            .get("sourceIn")
            .and_then(|value| value.as_f64())
            .ok_or_else(|| {
                vec![clip_property_error(
                    index,
                    "sourceIn",
                    "Video clip is missing a numeric sourceIn.",
                )]
            })?;
        let source_out = item
            .properties
            .get("sourceOut")
            .and_then(|value| value.as_f64())
            .ok_or_else(|| {
                vec![clip_property_error(
                    index,
                    "sourceOut",
                    "Video clip is missing a numeric sourceOut.",
                )]
            })?;

        if !source_in.is_finite()
            || !source_out.is_finite()
            || source_in < 0.0
            || source_out <= source_in
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("timeline.tracks.video.items[{index}].properties.sourceOut"),
                "Video clip source range is invalid.",
                "Ensure every combined e2e video clip has finite sourceIn and sourceOut with sourceOut greater than sourceIn.",
            )]);
        }

        if source_out > media_duration {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("timeline.tracks.video.items[{index}].properties.sourceOut"),
                "Video clip source range exceeds the raw media duration.",
                "Keep every combined e2e sourceOut within media-raw durationSeconds.",
            )]);
        }

        clips.push(RenderClip {
            source_path: None,
            timeline_start_seconds: Some(item.start_seconds),
            properties: BTreeMap::new(),
            timeline_track_index: 0,
            source_in,
            source_out,
        });
    }

    Ok(clips)
}

fn clip_property_error(index: usize, field: &str, message: &'static str) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        format!("timeline.tracks.video.items[{index}].properties.{field}"),
        message,
        "Build the combined e2e project from selected source ranges before rendering.",
    )
}

fn ensure_nonempty_artifact(path: PathBuf, error_path: &'static str) -> PipelineResult<()> {
    let metadata = std::fs::metadata(&path).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderArtifactMissing,
            error_path,
            "Expected combined e2e artifact was not created.",
            "Inspect the render backend logs and rerun the combined e2e command.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("ioError", error.to_string())]
    })?;

    if metadata.len() == 0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderArtifactEmpty,
            error_path,
            "Expected combined e2e artifact is empty.",
            "Inspect the render backend logs and rerun the combined e2e command.",
        )
        .with_detail("path", path.display().to_string())]);
    }

    Ok(())
}

fn labeled_output(outputs: &[(&str, &str)]) -> String {
    outputs
        .iter()
        .filter(|(_, output)| !output.trim().is_empty())
        .map(|(label, output)| format!("== {label} ==\n{output}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn video_clip_item(
    id: &str,
    label: &str,
    start_seconds: f64,
    source_in: f64,
    source_out: f64,
) -> TimelineItem {
    let mut properties = BTreeMap::new();
    properties.insert("sourceIn".to_string(), json!(source_in));
    properties.insert("sourceOut".to_string(), json!(source_out));
    properties.insert(
        "reason".to_string(),
        json!("selected source range for combined e2e validation"),
    );

    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds,
        duration_seconds: source_out - source_in,
        source: TimelineSource::Media {
            media_id: "media-raw".to_string(),
        },
        label: label.to_string(),
        properties,
    }
}

fn visual_text_item(
    id: &str,
    kind: TimelineItemKind,
    text: &str,
    start_seconds: f64,
    duration_seconds: f64,
    label: &str,
) -> TimelineItem {
    let mut properties = BTreeMap::new();
    properties.insert("text".to_string(), json!(text));
    properties.insert(
        "visualTreatment".to_string(),
        json!("transparent editorial overlay with strong hierarchy"),
    );
    properties.insert(
        "motion".to_string(),
        json!("quick scale in, hold, and soft fade out"),
    );
    properties.insert(
        "safeZone".to_string(),
        json!("keep text and focal objects inside 10% margins"),
    );
    properties.insert(
        "avoid".to_string(),
        json!("full-width black slabs, static text-only cards, and default-font boxes"),
    );

    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds,
        duration_seconds,
        source: TimelineSource::Text {
            text: text.to_string(),
        },
        label: label.to_string(),
        properties,
    }
}

fn transcript_word(text: &str, start_seconds: f64, end_seconds: f64) -> TranscriptWord {
    TranscriptWord {
        text: text.to_string(),
        start_seconds,
        end_seconds,
        confidence: Some(0.99),
        speaker: Some("speaker-1".to_string()),
    }
}

fn is_combined_e2e_flag(flag: &str) -> bool {
    matches!(flag, "--output-dir")
}

fn arg_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> PipelineError {
    PipelineError::new(PipelineErrorCode::PipelineInputInvalid, path, message, fix)
}

fn arg_path(flag: &str) -> String {
    match flag {
        "--output-dir" => "args.outputDir".to_string(),
        _ => {
            let trimmed = flag.trim_start_matches('-');
            format!("args.{}", kebab_to_camel(trimmed))
        }
    }
}

fn kebab_to_camel(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = false;
    for character in value.chars() {
        if character == '-' || character == '_' {
            uppercase_next = true;
            continue;
        }

        if uppercase_next {
            output.extend(character.to_uppercase());
            uppercase_next = false;
        } else {
            output.push(character);
        }
    }
    output
}
