use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::codex::proposal::{validate_codex_edit_proposal, CodexEditProposal};
use crate::edit::edl::{validate_one_click_edl, EdlClip, RoughCutEdl};
use crate::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use crate::edit::render_plan::{
    render_quality_from_legacy, RenderClip, RenderOutputProfile, RenderPlan, RenderQuality,
    RenderQualityProfile, TemplateRenderLayer,
};
use crate::gpu_graphics::error::{
    gpu_errors_to_pipeline_errors, GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult,
};
use crate::gpu_graphics::ir::GpuGraphicsLayer;
use crate::gpu_graphics::profile::{
    expand_profile, parse_profile_id, profile_id_name, supported_profile_ids,
    supports_cpu_fallback, GpuProfileExpansionInput, GpuVisualProfileId,
    HQ_NEON_WIREFRAME_SHADER_V1,
};
use crate::gpu_graphics::renderer::GpuRenderOptions;
use crate::gpu_graphics::software_renderer::{
    render_hq_profile_software, SoftwareGpuRenderOptions,
};
use crate::gpu_graphics::validation::validate_gpu_graphics_layer;
use crate::gpu_graphics::visual_qa::{
    run_hq_visual_qa, VisualQaOptions, VisualQaReport as GpuVisualQaReport,
};
use crate::graphics::assets::AssetRegistry;
use crate::graphics::ir::{
    Color, Dimensions, Easing, GraphicNode, GraphicRole, GraphicsLayer, LineNode, NodeAnimation,
    Point, Rect, RoundedRectNode, TextEmphasisRange, TextNode,
};
use crate::graphics::manifest::{graphics_playback_manifest, GraphicsArtifactManifest};
use crate::graphics::motion_presets::{
    motion_preset_animation, parse_motion_preset_id, MotionPresetId, SUPPORTED_MOTION_PRESETS,
};
use crate::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use crate::graphics::templates::template_layer_to_graphics_ir;
use crate::graphics::validation::validate_graphics_layer;
use crate::graphics::visual_qa::{run_cpu_visual_qa, CpuVisualQaOptions, CpuVisualQaReport};
use crate::project::export_options::ExportRenderOptions;
use crate::project::export_profiles::ExportProfile;
use crate::project::model::{MediaAsset, MediaKind, Transcript, TranscriptWord, VideoProject};
use crate::render_pipeline::backend::RenderBackend;
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::graphics_cache::{
    gpu_graphics_layer_fingerprint, graphics_layer_fingerprint, validate_gpu_graphics_cache_hit,
    validate_graphics_cache_hit, write_graphics_cache_metadata, GraphicsCacheLookup,
};
use crate::render_pipeline::gstreamer_backend::{
    probe_media_with_gstreamer, GstreamerGesRenderBackend,
};
use crate::render_pipeline::performance::RenderPerformanceRecorder;
use crate::render_pipeline::probe::{validate_rendered_media, ExpectedMedia, MediaProbe};
use crate::render_pipeline::process::{
    CommandSpec, ProcessOutput, ProcessRunner, SystemProcessRunner,
};
use crate::render_pipeline::quality::effective_quality_settings;
use crate::render_pipeline::report::{
    write_json_report, write_markdown_report, RenderGraphicsReport, RenderReport,
    RenderReportStreams, RenderReportSummary, RenderStageReport,
};
use crate::render_pipeline::source_probe_cache::{
    validate_source_probe_cache_hit, write_source_probe_cache, SourceProbeCacheLookup,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_FINAL_NAME: &str = "final.webm";
const RENDER_REPORT_FILE_NAME: &str = "render-report.json";
const RENDER_REPORT_MARKDOWN_FILE_NAME: &str = "render-report.md";
const GRAPHICS_DIR_NAME: &str = "graphics";
const GRAPHICS_MANIFEST_FILE_NAME: &str = "manifest.json";
const GRAPHICS_FRAMES_DIR_NAME: &str = "frames";
const GRAPHICS_FRAMES_PATTERN: &str = "frames/frame-%06d.png";
const SOURCE_PROBE_CACHE_FILE_NAME: &str = "source-probe-cache.json";
const VISUAL_TIMING_TOLERANCE_SECONDS: f64 = 1e-6;
const GRAPHICS_PREVIEW_FILE_NAME: &str = "preview.png";
const PROCESS_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalReport {
    pub generated_at: String,
    pub proposal: CodexEditProposal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProposalConfig {
    pub project_root: PathBuf,
    pub source_video_path: PathBuf,
    pub codex_report_path: PathBuf,
    pub output_dir: PathBuf,
    pub final_name: String,
    pub quality_profile: RenderQualityProfile,
}

impl RenderProposalConfig {
    pub fn final_path(&self) -> PathBuf {
        self.output_dir.join(&self.final_name)
    }

    pub fn render_report_path(&self) -> PathBuf {
        self.output_dir.join(RENDER_REPORT_FILE_NAME)
    }

    pub fn markdown_report_path(&self) -> PathBuf {
        self.output_dir.join(RENDER_REPORT_MARKDOWN_FILE_NAME)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderProposalPreview {
    pub graphics_layer_count: usize,
    pub command: CommandSpec,
    pub quality_profile: String,
    pub effective_width: u32,
    pub effective_height: u32,
    pub effective_fps: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProposalRunResult {
    pub final_path: PathBuf,
    pub render_report_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedProposalGraphics {
    pub artifact_dir: PathBuf,
    pub manifest: GraphicsArtifactManifest,
    pub timeline_start_seconds: f64,
    pub renderer: String,
    pub quality_profile: Option<String>,
    pub template_id: Option<String>,
    pub motion_preset_id: Option<String>,
    pub visual_qa_status: Option<String>,
    pub cache_status: Option<String>,
    pub sampled_frames: Vec<String>,
    pub qa_metrics: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
struct RenderedGraphicsArtifact {
    manifest: GraphicsArtifactManifest,
    artifact_dir: PathBuf,
    timeline_start_seconds: f64,
    renderer: String,
    quality_profile: Option<String>,
    template_id: Option<String>,
    motion_preset_id: Option<String>,
    visual_qa_status: Option<String>,
    cache_status: Option<String>,
    sampled_frames: Vec<String>,
    qa_metrics: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
struct RenderOutputSettings {
    width: u32,
    height: u32,
    fps: f64,
}

#[derive(Debug, Default)]
struct RenderProposalArgs {
    project_root: Option<PathBuf>,
    source_video_path: Option<PathBuf>,
    codex_report_path: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    final_name: Option<String>,
    quality_profile: Option<RenderQualityProfile>,
}

type GpuLayerRenderer =
    dyn Fn(&GpuGraphicsLayer, GpuRenderOptions) -> GpuGraphicsResult<GraphicsArtifactManifest>;

pub fn parse_render_proposal_args<I, S>(args: I) -> PipelineResult<RenderProposalConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut tokens = args.into_iter().map(Into::into).collect::<Vec<String>>();
    if tokens.first().is_some_and(|token| !token.starts_with("--")) {
        tokens.remove(0);
    }

    let mut parsed = RenderProposalArgs::default();
    let mut index = 0;
    while index < tokens.len() {
        let flag = &tokens[index];
        if !flag.starts_with("--") {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unexpected positional argument.",
                "Use --project-root, --source-video, --report, --output-dir, --final-name, or --quality.",
            )]);
        }
        if !is_render_proposal_flag(flag) {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unknown render proposal argument.",
                "Use --project-root, --source-video, --report, --output-dir, --final-name, or --quality.",
            )]);
        }

        let value = tokens
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| {
                vec![arg_error(
                    arg_path(flag),
                    "Missing value for render proposal argument.",
                    format!("Pass a value after {flag}."),
                )]
            })?
            .clone();

        match flag.as_str() {
            "--project-root" => parsed.project_root = Some(value.into()),
            "--source-video" => parsed.source_video_path = Some(value.into()),
            "--report" => parsed.codex_report_path = Some(value.into()),
            "--output-dir" => parsed.output_dir = Some(value.into()),
            "--final-name" => parsed.final_name = Some(value),
            "--quality" => {
                parsed.quality_profile =
                    Some(parse_render_quality_profile(&value).ok_or_else(|| {
                        vec![arg_error(
                            arg_path(flag),
                            "Unknown render quality profile.",
                            "Use draftWebm or finalWebm.",
                        )
                        .with_detail("quality", value.clone())]
                    })?)
            }
            _ => unreachable!("render proposal flag was checked before value parsing"),
        }

        index += 2;
    }

    Ok(RenderProposalConfig {
        codex_report_path: required_path(parsed.codex_report_path, "args.report", "--report")?,
        project_root: required_path(parsed.project_root, "args.projectRoot", "--project-root")?,
        source_video_path: required_path(
            parsed.source_video_path,
            "args.sourceVideo",
            "--source-video",
        )?,
        output_dir: required_path(parsed.output_dir, "args.outputDir", "--output-dir")?,
        final_name: parsed
            .final_name
            .unwrap_or_else(|| DEFAULT_FINAL_NAME.to_string()),
        quality_profile: parsed.quality_profile.unwrap_or_default(),
    })
}

pub fn build_render_proposal_preview(
    project: &VideoProject,
    request: &EditJobRequest,
    report: &CodexProposalReport,
    config: &RenderProposalConfig,
) -> PipelineResult<RenderProposalPreview> {
    let final_path = config.final_path();
    let plan = proposal_to_render_plan(
        project,
        request,
        &report.proposal,
        &config.source_video_path,
        &final_path,
        render_quality_from_legacy(config.quality_profile.clone()),
    )?;
    let render_duration_seconds = render_plan_duration_seconds(&plan);
    let layers = proposal_visuals_to_graphics_layers_for_duration(
        &report.proposal,
        plan.width,
        plan.height,
        plan.fps,
        render_duration_seconds,
    )?;
    let gpu_layers = proposal_gpu_visuals_to_layers_for_duration(
        &report.proposal,
        plan.width,
        plan.height,
        plan.fps,
        render_duration_seconds,
    )?;
    let mut graphics = layers
        .iter()
        .map(|layer| {
            (
                manifest_for_graphics_layer(layer),
                config
                    .output_dir
                    .join(GRAPHICS_DIR_NAME)
                    .join(layer.id.as_str()),
                layer.timeline_start,
            )
        })
        .collect::<Vec<_>>();
    graphics.extend(gpu_layers.iter().map(|layer| {
        (
            manifest_for_gpu_graphics_layer(layer),
            config
                .output_dir
                .join(GRAPHICS_DIR_NAME)
                .join(layer.id.as_str()),
            layer.timeline_start,
        )
    }));
    let command = GstreamerGesRenderBackend::new().build_command(&plan, &graphics)?;
    let quality_settings =
        effective_quality_settings(plan.quality, plan.width, plan.height, plan.fps);

    Ok(RenderProposalPreview {
        graphics_layer_count: layers.len() + gpu_layers.len(),
        command,
        quality_profile: render_quality_profile_value(plan.quality).to_string(),
        effective_width: quality_settings.output_width,
        effective_height: quality_settings.output_height,
        effective_fps: quality_settings.output_fps,
    })
}

pub fn run_render_proposal(
    config: &RenderProposalConfig,
) -> PipelineResult<RenderProposalRunResult> {
    run_render_proposal_with_runner(config, &SystemProcessRunner)
}

pub fn run_render_proposal_with_runner(
    config: &RenderProposalConfig,
    runner: &dyn ProcessRunner,
) -> PipelineResult<RenderProposalRunResult> {
    let mut performance = RenderPerformanceRecorder::start();
    create_output_dir(config)?;

    let report_json = performance.measure_stage(
        "readReport",
        BTreeMap::from([(
            "path".to_string(),
            config.codex_report_path.display().to_string(),
        )]),
        || read_codex_report(config),
    )?;
    let report = codex_report_from_json(&report_json)?;
    ensure_source_video_exists(config)?;

    let source_probe_started_at = Instant::now();
    let (source_probe, source_probe_output, _source_probe_cache_status) =
        match probe_source_video_with_cache(
            &config.source_video_path,
            &config.output_dir.join(SOURCE_PROBE_CACHE_FILE_NAME),
            || {
                probe_media_with_gstreamer(
                    &config.source_video_path,
                    PROCESS_TIMEOUT,
                    "sourceVideo",
                )
            },
        ) {
            Ok(result) => {
                performance.push_stage(RenderStageReport {
                    name: "sourceProbe".to_string(),
                    status: "succeeded".to_string(),
                    duration_ms: source_probe_started_at.elapsed().as_millis(),
                    details: BTreeMap::from([
                        (
                            "path".to_string(),
                            config.source_video_path.display().to_string(),
                        ),
                        ("cacheStatus".to_string(), result.2.clone()),
                    ]),
                });
                result
            }
            Err(errors) => {
                performance.push_stage(RenderStageReport {
                    name: "sourceProbe".to_string(),
                    status: "failed".to_string(),
                    duration_ms: source_probe_started_at.elapsed().as_millis(),
                    details: BTreeMap::from([(
                        "path".to_string(),
                        config.source_video_path.display().to_string(),
                    )]),
                });
                return Err(errors);
            }
        };
    validate_rendered_media(
        &config.source_video_path,
        &source_probe,
        &ExpectedMedia {
            width: None,
            height: None,
            video_required: true,
            audio_required: true,
            non_empty_required: true,
            expected_duration_seconds: None,
            duration_tolerance_seconds: None,
            ..ExpectedMedia::default()
        },
    )?;

    let project = project_from_source_probe(config, &report, &source_probe)?;
    let request = request_for_report(&report);
    let final_path = config.final_path();
    let plan = performance.measure_stage(
        "buildRenderPlan",
        BTreeMap::from([
            ("mediaId".to_string(), report.proposal.media_id.clone()),
            (
                "clipCount".to_string(),
                report.proposal.clips.len().to_string(),
            ),
        ]),
        || {
            proposal_to_render_plan(
                &project,
                &request,
                &report.proposal,
                &config.source_video_path,
                &final_path,
                render_quality_from_legacy(config.quality_profile.clone()),
            )
        },
    )?;
    let graphics_started_at = Instant::now();
    let graphics = match render_proposal_graphics_artifacts(
        &report,
        &config.output_dir.join(GRAPHICS_DIR_NAME),
        render_output_settings_from_plan(&plan),
    ) {
        Ok(graphics) => {
            performance.push_stage(RenderStageReport {
                name: "graphics".to_string(),
                status: "succeeded".to_string(),
                duration_ms: graphics_started_at.elapsed().as_millis(),
                details: graphics_performance_details(&graphics),
            });
            graphics
        }
        Err(errors) => {
            performance.push_stage(RenderStageReport {
                name: "graphics".to_string(),
                status: "failed".to_string(),
                duration_ms: graphics_started_at.elapsed().as_millis(),
                details: BTreeMap::from([
                    (
                        "captionCount".to_string(),
                        report.proposal.captions.len().to_string(),
                    ),
                    (
                        "overlayCount".to_string(),
                        report.proposal.overlays.len().to_string(),
                    ),
                    (
                        "gpuVisualCount".to_string(),
                        report.proposal.gpu_visuals.len().to_string(),
                    ),
                ]),
            });
            return Err(errors);
        }
    };
    let backend_graphics = graphics_backend_inputs(&graphics);

    let backend = GstreamerGesRenderBackend::new();
    let render_command = backend.build_command(&plan, &backend_graphics)?;
    let render_output = performance.measure_stage(
        "gesRender",
        BTreeMap::from([
            ("clipCount".to_string(), plan.clips.len().to_string()),
            ("graphicsCount".to_string(), graphics.len().to_string()),
            (
                "qualityProfile".to_string(),
                render_quality_profile_value(plan.quality).to_string(),
            ),
        ]),
        || backend.render(runner, &plan, &backend_graphics, PROCESS_TIMEOUT),
    )?;

    let (final_probe, final_probe_output) = performance.measure_stage(
        "finalProbe",
        BTreeMap::from([("path".to_string(), final_path.display().to_string())]),
        || probe_media_with_gstreamer(&final_path, PROCESS_TIMEOUT, "final"),
    )?;
    validate_rendered_media(
        &final_path,
        &final_probe,
        &ExpectedMedia {
            width: Some(plan.width),
            height: Some(plan.height),
            video_required: true,
            audio_required: true,
            non_empty_required: true,
            expected_duration_seconds: Some(render_plan_duration_seconds(&plan)),
            duration_tolerance_seconds: Some(render_plan_duration_tolerance_seconds(plan.fps)),
            ..ExpectedMedia::default()
        },
    )?;

    let mut artifacts = vec![
        final_path.display().to_string(),
        config.render_report_path().display().to_string(),
        config.markdown_report_path().display().to_string(),
    ];
    artifacts.extend(graphics_artifact_paths(&graphics));
    performance.push_stage(RenderStageReport {
        name: "writeReport".to_string(),
        status: "succeeded".to_string(),
        duration_ms: 0,
        details: BTreeMap::from([
            (
                "json".to_string(),
                config.render_report_path().display().to_string(),
            ),
            (
                "markdown".to_string(),
                config.markdown_report_path().display().to_string(),
            ),
        ]),
    });
    let performance = Some(performance.finish());
    let render_report = RenderReport {
        job_id: "render-proposal".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: final_probe.duration_seconds,
            output_path: Some(final_path.display().to_string()),
            ..RenderReportSummary::default()
        },
        command: render_command,
        stdout: labeled_output(&[
            ("source gstreamer discoverer", &source_probe_output.stdout),
            ("render gstreamer ges", &render_output.stdout),
            ("final gstreamer discoverer", &final_probe_output.stdout),
        ]),
        stderr: labeled_output(&[
            ("source gstreamer discoverer", &source_probe_output.stderr),
            ("render gstreamer ges", &render_output.stderr),
            ("final gstreamer discoverer", &final_probe_output.stderr),
        ]),
        errors: Vec::new(),
        streams: Some(RenderReportStreams {
            video: final_probe.video.is_some(),
            audio: final_probe.audio.is_some(),
        }),
        artifacts,
        graphics: graphics.iter().map(render_graphics_report).collect(),
        performance,
        preview_comparison_request: None,
        preview_comparison: None,
    };
    write_json_report(&config.render_report_path(), &render_report)?;
    write_markdown_report(&config.markdown_report_path(), &render_report)?;

    Ok(RenderProposalRunResult {
        final_path,
        render_report_path: config.render_report_path(),
    })
}

#[doc(hidden)]
pub fn probe_source_video_with_cache<F>(
    source_path: &Path,
    cache_path: &Path,
    probe_source: F,
) -> PipelineResult<(MediaProbe, ProcessOutput, String)>
where
    F: FnOnce() -> PipelineResult<(MediaProbe, ProcessOutput)>,
{
    match validate_source_probe_cache_hit(cache_path, source_path)
        .map_err(source_probe_cache_io_error)?
    {
        SourceProbeCacheLookup::Hit(probe) => {
            let stdout = serde_json::to_string_pretty(&probe).unwrap_or_else(|_| "{}".to_string());
            Ok((
                probe,
                ProcessOutput {
                    status_code: Some(0),
                    stdout,
                    stderr: String::new(),
                },
                "hit".to_string(),
            ))
        }
        SourceProbeCacheLookup::Miss(reason) => {
            let (probe, output) = probe_source()?;
            let cache_status = match write_source_probe_cache(cache_path, source_path, &probe) {
                Ok(()) => format!("miss:{reason}"),
                Err(error) => format!("miss:{reason};writeFailed={error}"),
            };
            Ok((probe, output, cache_status))
        }
    }
}

fn render_plan_duration_tolerance_seconds(fps: f64) -> f64 {
    if fps.is_finite() && fps > 0.0 {
        1.0 / fps
    } else {
        0.05
    }
}

fn render_quality_profile_value(quality: RenderQuality) -> &'static str {
    match quality {
        RenderQuality::Draft => "draftWebm",
        RenderQuality::Final => "finalWebm",
    }
}

fn parse_render_quality_profile(value: &str) -> Option<RenderQualityProfile> {
    match value {
        "draftWebm" => Some(RenderQualityProfile::DraftWebm),
        "finalWebm" => Some(RenderQualityProfile::FinalWebm),
        _ => None,
    }
}

fn render_output_settings_from_plan(plan: &RenderPlan) -> RenderOutputSettings {
    RenderOutputSettings {
        width: plan.width,
        height: plan.height,
        fps: plan.fps,
    }
}

fn draft_render_output_settings(project: &VideoProject) -> RenderOutputSettings {
    let options = ExportRenderOptions::for_project_defaults(
        ExportProfile::Webm,
        RenderQuality::Draft,
        project.render_settings.width,
        project.render_settings.height,
    )
    .expect("project render settings must yield valid draft dimensions");
    let settings = effective_quality_settings(
        options.quality,
        options.width,
        options.height,
        project.render_settings.fps,
    );
    RenderOutputSettings {
        width: settings.output_width,
        height: settings.output_height,
        fps: settings.output_fps,
    }
}

pub fn render_proposal_graphics(
    project: &VideoProject,
    report: &CodexProposalReport,
    output_dir: &Path,
) -> PipelineResult<Vec<RenderedProposalGraphics>> {
    render_proposal_graphics_with_gpu_renderer(
        project,
        report,
        output_dir,
        &crate::gpu_graphics::renderer::render_gpu_graphics_layer,
    )
}

#[doc(hidden)]
pub fn render_proposal_graphics_with_gpu_renderer(
    project: &VideoProject,
    report: &CodexProposalReport,
    output_dir: &Path,
    gpu_renderer: &GpuLayerRenderer,
) -> PipelineResult<Vec<RenderedProposalGraphics>> {
    let settings = draft_render_output_settings(project);
    render_proposal_graphics_artifacts_with_gpu_renderer(report, output_dir, settings, gpu_renderer)
        .map(|graphics| {
            graphics
                .into_iter()
                .map(|graphics| RenderedProposalGraphics {
                    artifact_dir: graphics.artifact_dir,
                    manifest: graphics.manifest,
                    timeline_start_seconds: graphics.timeline_start_seconds,
                    renderer: graphics.renderer,
                    quality_profile: graphics.quality_profile,
                    template_id: graphics.template_id,
                    motion_preset_id: graphics.motion_preset_id,
                    visual_qa_status: graphics.visual_qa_status,
                    cache_status: graphics.cache_status,
                    sampled_frames: graphics.sampled_frames,
                    qa_metrics: graphics.qa_metrics,
                })
                .collect()
        })
}

fn render_proposal_graphics_artifacts(
    report: &CodexProposalReport,
    output_dir: &Path,
    settings: RenderOutputSettings,
) -> PipelineResult<Vec<RenderedGraphicsArtifact>> {
    render_proposal_graphics_artifacts_with_gpu_renderer(
        report,
        output_dir,
        settings,
        &crate::gpu_graphics::renderer::render_gpu_graphics_layer,
    )
}

fn render_proposal_graphics_artifacts_with_gpu_renderer(
    report: &CodexProposalReport,
    output_dir: &Path,
    settings: RenderOutputSettings,
    gpu_renderer: &GpuLayerRenderer,
) -> PipelineResult<Vec<RenderedGraphicsArtifact>> {
    let render_duration_seconds = proposal_duration_seconds(&report.proposal);
    let cpu_layers = proposal_visuals_to_graphics_layers_for_duration(
        &report.proposal,
        settings.width,
        settings.height,
        settings.fps,
        render_duration_seconds,
    )?;
    let gpu_layers = proposal_gpu_visuals_to_layers_for_duration(
        &report.proposal,
        settings.width,
        settings.height,
        settings.fps,
        render_duration_seconds,
    )?;
    let assets = AssetRegistry::new(PathBuf::new());
    let mut graphics = Vec::with_capacity(cpu_layers.len() + gpu_layers.len());

    for layer in &cpu_layers {
        let layer_dir = output_dir.join(&layer.id);
        let (template_id, motion_preset_id) = proposal_layer_metadata(&report.proposal, &layer.id);
        let fingerprint =
            graphics_layer_fingerprint(layer, "rust", None).map_err(graphics_cache_serde_error)?;
        let (manifest, cache_status) =
            match validate_graphics_cache_hit(&layer_dir, layer, "rust", None, &fingerprint)
                .map_err(graphics_cache_io_error)?
            {
                GraphicsCacheLookup::Hit(manifest) => (manifest, "hit".to_string()),
                GraphicsCacheLookup::Miss(reason) => {
                    let manifest = render_graphics_preview(
                        layer,
                        &assets,
                        GraphicsRenderOptions {
                            output_dir: layer_dir.clone(),
                        },
                    )
                    .map_err(PipelineError::from_graphics_errors)?;
                    write_graphics_cache_metadata(&layer_dir, &fingerprint, "rust", None)
                        .map_err(graphics_cache_io_error)?;
                    (manifest, format!("miss:{reason}"))
                }
            };
        let qa_report =
            run_cpu_visual_qa(layer, &layer_dir, &manifest, CpuVisualQaOptions::default())
                .map_err(PipelineError::from_graphics_errors)?;
        let sampled_frames = qa_report.sampled_frames.clone();
        let qa_metrics = cpu_visual_qa_metrics(&qa_report);
        graphics.push(RenderedGraphicsArtifact {
            manifest,
            artifact_dir: layer_dir,
            timeline_start_seconds: layer.timeline_start,
            renderer: "rust".to_string(),
            quality_profile: None,
            template_id,
            motion_preset_id,
            visual_qa_status: Some("passed".to_string()),
            cache_status: Some(cache_status),
            sampled_frames,
            qa_metrics,
        });
    }

    for layer in &gpu_layers {
        let layer_dir = output_dir.join(&layer.id);
        let (manifest, renderer, visual_qa_status, cache_status, qa_report) =
            render_gpu_layer_with_cache(layer, layer_dir.clone(), gpu_renderer)?;
        let sampled_frames = qa_report.sampled_frames.clone();
        let qa_metrics = gpu_visual_qa_metrics(&qa_report);
        graphics.push(RenderedGraphicsArtifact {
            manifest,
            artifact_dir: layer_dir,
            timeline_start_seconds: layer.timeline_start,
            renderer,
            quality_profile: layer
                .quality_profile
                .clone()
                .or_else(|| Some(HQ_NEON_WIREFRAME_SHADER_V1.to_string())),
            template_id: None,
            motion_preset_id: None,
            visual_qa_status: Some(visual_qa_status),
            cache_status: Some(cache_status),
            sampled_frames,
            qa_metrics,
        });
    }

    Ok(graphics)
}

fn render_gpu_layer_with_software_fallback(
    layer: &GpuGraphicsLayer,
    layer_dir: PathBuf,
    gpu_renderer: &GpuLayerRenderer,
) -> PipelineResult<(GraphicsArtifactManifest, String, String, GpuVisualQaReport)> {
    let (manifest, renderer) = match gpu_renderer(
        layer,
        GpuRenderOptions {
            output_dir: layer_dir.clone(),
        },
    ) {
        Ok(manifest) => Ok((manifest, "gpu".to_string())),
        Err(errors)
            if only_gpu_device_unavailable_errors(&errors)
                && layer
                    .quality_profile
                    .as_deref()
                    .and_then(parse_profile_id)
                    .is_some_and(supports_cpu_fallback) =>
        {
            let manifest = render_hq_profile_software(
                layer,
                SoftwareGpuRenderOptions {
                    output_dir: layer_dir.clone(),
                },
            )
            .map_err(gpu_errors_to_pipeline_errors)?;
            Ok((manifest, "software".to_string()))
        }
        Err(errors) => Err(gpu_errors_to_pipeline_errors(errors)),
    }?;

    let qa_report = run_hq_visual_qa(
        &layer_dir,
        manifest.dimensions.width,
        manifest.dimensions.height,
        manifest.frame_count,
        VisualQaOptions::default(),
    )
    .map_err(|errors| enrich_gpu_visual_qa_errors(errors, layer, &layer_dir))
    .map_err(gpu_errors_to_pipeline_errors)?;

    Ok((manifest, renderer, "passed".to_string(), qa_report))
}

fn render_gpu_layer_with_cache(
    layer: &GpuGraphicsLayer,
    layer_dir: PathBuf,
    gpu_renderer: &GpuLayerRenderer,
) -> PipelineResult<(
    GraphicsArtifactManifest,
    String,
    String,
    String,
    GpuVisualQaReport,
)> {
    let quality_profile = layer.quality_profile.as_deref();
    let gpu_fingerprint = gpu_graphics_layer_fingerprint(layer, "gpu", quality_profile)
        .map_err(graphics_cache_serde_error)?;
    match validate_gpu_graphics_cache_hit(
        &layer_dir,
        layer,
        "gpu",
        quality_profile,
        &gpu_fingerprint,
    )
    .map_err(graphics_cache_io_error)?
    {
        GraphicsCacheLookup::Hit(manifest) => {
            let qa_report = run_hq_visual_qa(
                &layer_dir,
                manifest.dimensions.width,
                manifest.dimensions.height,
                manifest.frame_count,
                VisualQaOptions::default(),
            )
            .map_err(|errors| enrich_gpu_visual_qa_errors(errors, layer, &layer_dir))
            .map_err(gpu_errors_to_pipeline_errors)?;
            return Ok((
                manifest,
                "gpu".to_string(),
                "passed".to_string(),
                "hit".to_string(),
                qa_report,
            ));
        }
        GraphicsCacheLookup::Miss(_) => {}
    }

    let software_fingerprint = gpu_graphics_layer_fingerprint(layer, "software", quality_profile)
        .map_err(graphics_cache_serde_error)?;
    if let GraphicsCacheLookup::Hit(manifest) = validate_gpu_graphics_cache_hit(
        &layer_dir,
        layer,
        "software",
        quality_profile,
        &software_fingerprint,
    )
    .map_err(graphics_cache_io_error)?
    {
        let qa_report = run_hq_visual_qa(
            &layer_dir,
            manifest.dimensions.width,
            manifest.dimensions.height,
            manifest.frame_count,
            VisualQaOptions::default(),
        )
        .map_err(|errors| enrich_gpu_visual_qa_errors(errors, layer, &layer_dir))
        .map_err(gpu_errors_to_pipeline_errors)?;
        return Ok((
            manifest,
            "software".to_string(),
            "passed".to_string(),
            "hit".to_string(),
            qa_report,
        ));
    }

    let (manifest, renderer, visual_qa_status, qa_report) =
        render_gpu_layer_with_software_fallback(layer, layer_dir.clone(), gpu_renderer)?;
    let fingerprint = if renderer == "software" {
        software_fingerprint
    } else {
        gpu_fingerprint
    };
    write_graphics_cache_metadata(&layer_dir, &fingerprint, &renderer, quality_profile)
        .map_err(graphics_cache_io_error)?;

    Ok((
        manifest,
        renderer,
        visual_qa_status,
        "miss".to_string(),
        qa_report,
    ))
}

fn enrich_gpu_visual_qa_errors(
    mut errors: Vec<GpuGraphicsError>,
    layer: &GpuGraphicsLayer,
    layer_dir: &Path,
) -> Vec<GpuGraphicsError> {
    for error in &mut errors {
        if error.code == GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed {
            error
                .details
                .insert("layerId".to_string(), layer.id.clone());
            error
                .details
                .insert("artifactDir".to_string(), layer_dir.display().to_string());
        }
    }
    errors
}

fn only_gpu_device_unavailable_errors(errors: &[GpuGraphicsError]) -> bool {
    !errors.is_empty()
        && errors
            .iter()
            .all(|error| error.code == GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable)
}

fn graphics_backend_inputs(
    graphics: &[RenderedGraphicsArtifact],
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

fn proposal_layer_metadata(
    proposal: &CodexEditProposal,
    layer_id: &str,
) -> (Option<String>, Option<String>) {
    for (index, _) in proposal.captions.iter().enumerate() {
        let caption_id = format!("proposal-caption-{}", index + 1);
        if caption_id == layer_id {
            return (None, Some("snap-pop-v1".to_string()));
        }
    }

    for (index, overlay) in proposal.overlays.iter().enumerate() {
        let overlay_id = optional_non_empty_string(overlay, "id")
            .unwrap_or_else(|| format!("proposal-overlay-{}", index + 1));
        if overlay_id == layer_id {
            let motion_preset_id = optional_non_empty_string(overlay, "motionPresetId")
                .or_else(|| {
                    optional_non_empty_string(overlay, "templateId")
                        .and_then(|template_id| motion_preset_for_template_id(&template_id))
                })
                .or_else(|| Some("slide-fade-up-v1".to_string()));
            return (
                optional_non_empty_string(overlay, "templateId"),
                motion_preset_id,
            );
        }
    }

    for (index, hyperframe) in proposal.hyperframes.iter().enumerate() {
        let hyperframe_id = optional_non_empty_string(hyperframe, "id")
            .unwrap_or_else(|| format!("proposal-hyperframe-{}", index + 1));
        if hyperframe_id == layer_id {
            let template_id = optional_non_empty_string(hyperframe, "templateId").or_else(|| {
                optional_non_empty_string(hyperframe, "kind")
                    .and_then(|kind| default_hyperframe_template_id(hyperframe, &kind))
            });
            let motion_preset_id =
                optional_non_empty_string(hyperframe, "motionPresetId").or_else(|| {
                    template_id
                        .as_deref()
                        .and_then(motion_preset_for_template_id)
                });
            return (template_id, motion_preset_id);
        }
    }
    (None, None)
}

fn default_hyperframe_template_id(hyperframe: &Value, kind: &str) -> Option<String> {
    match kind {
        "title_card" => Some("chapter-card-v1".to_string()),
        "lower_third" => Some("kinetic-lower-third-v1".to_string()),
        "diagram" => Some("metric-callout-v1".to_string()),
        "transition" => Some("punchy-caption-v1".to_string()),
        "immersive_scene" => Some(proposal_immersive_scene_template_id(hyperframe)),
        _ => None,
    }
}

fn motion_preset_for_template_id(template_id: &str) -> Option<String> {
    match template_id {
        "kinetic-lower-third-v1" => Some("slide-fade-up-v1".to_string()),
        "punchy-caption-v1" => Some("snap-pop-v1".to_string()),
        "metric-callout-v1" => Some("metric-count-pop-v1".to_string()),
        "chapter-card-v1" => Some("vertical-reveal-v1".to_string()),
        "tracking-highlight-v1" => Some("tracking-draw-v1".to_string()),
        "holographic-logo-cutout-v1" => Some("pulse-emphasis-v2".to_string()),
        _ => None,
    }
}

fn render_graphics_report(graphics: &RenderedGraphicsArtifact) -> RenderGraphicsReport {
    RenderGraphicsReport {
        layer_id: graphics
            .manifest
            .source_layer_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        renderer: graphics.renderer.clone(),
        quality_profile: graphics.quality_profile.clone(),
        template_id: graphics.template_id.clone(),
        motion_preset_id: graphics.motion_preset_id.clone(),
        visual_qa_status: graphics.visual_qa_status.clone(),
        cache_status: graphics.cache_status.clone(),
        sampled_frames: graphics.sampled_frames.clone(),
        qa_metrics: graphics.qa_metrics.clone(),
    }
}

fn cpu_visual_qa_metrics(report: &CpuVisualQaReport) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "visibleAlphaRatio".to_string(),
            format_metric(report.visible_alpha_ratio),
        ),
        (
            "temporalDelta".to_string(),
            format_metric(report.temporal_delta),
        ),
    ])
}

fn gpu_visual_qa_metrics(report: &GpuVisualQaReport) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "opaqueRatio".to_string(),
            format_metric(report.opaque_ratio),
        ),
        (
            "saturatedRatio".to_string(),
            format_metric(report.saturated_ratio),
        ),
        ("edgeRatio".to_string(), format_metric(report.edge_ratio)),
        (
            "temporalDelta".to_string(),
            format_metric(report.temporal_delta),
        ),
    ])
}

fn format_metric(value: f64) -> String {
    format!("{value:.6}")
}

fn graphics_performance_details(graphics: &[RenderedGraphicsArtifact]) -> BTreeMap<String, String> {
    let frame_count = graphics
        .iter()
        .map(|graphics| graphics.manifest.frame_count as u64)
        .sum::<u64>();
    let cache_hits = graphics
        .iter()
        .filter(|graphics| graphics.cache_status.as_deref() == Some("hit"))
        .count();
    let cache_misses = graphics
        .iter()
        .filter(|graphics| {
            graphics
                .cache_status
                .as_deref()
                .is_some_and(|status| status.starts_with("miss"))
        })
        .count();
    let artifact_bytes = graphics
        .iter()
        .map(graphics_artifact_size_bytes)
        .sum::<u64>();

    BTreeMap::from([
        ("graphicsCount".to_string(), graphics.len().to_string()),
        ("frameCount".to_string(), frame_count.to_string()),
        ("cacheHits".to_string(), cache_hits.to_string()),
        ("cacheMisses".to_string(), cache_misses.to_string()),
        ("artifactBytes".to_string(), artifact_bytes.to_string()),
    ])
}

fn graphics_artifact_size_bytes(graphics: &RenderedGraphicsArtifact) -> u64 {
    graphics_artifact_paths_for_one(graphics)
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum()
}

fn graphics_artifact_paths_for_one(graphics: &RenderedGraphicsArtifact) -> Vec<PathBuf> {
    let mut paths = vec![
        graphics.artifact_dir.join(GRAPHICS_MANIFEST_FILE_NAME),
        graphics.artifact_dir.join(&graphics.manifest.preview_path),
    ];
    for frame_index in 0..graphics.manifest.frame_count {
        paths.push(
            graphics
                .artifact_dir
                .join(GRAPHICS_FRAMES_DIR_NAME)
                .join(format!("frame-{frame_index:06}.png")),
        );
    }
    paths
}

fn graphics_cache_serde_error(error: serde_json::Error) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "renderPlan.graphics.cache",
        "Graphics cache metadata could not be serialized.",
        "Inspect graphics layer fields and retry rendering without stale cache metadata.",
    )
    .with_detail("serdeError", error.to_string())]
}

fn graphics_cache_io_error(error: std::io::Error) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "renderPlan.graphics.cache",
        "Graphics cache metadata could not be read or written.",
        "Remove stale graphics cache artifacts or choose a writable output directory.",
    )
    .with_detail("ioError", error.to_string())]
}

fn source_probe_cache_io_error(error: std::io::Error) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "sourceVideo.cache",
        "Source probe cache metadata could not be read.",
        "Remove stale source probe cache metadata or choose a readable source video path.",
    )
    .with_detail("ioError", error.to_string())]
}

fn graphics_artifact_paths(graphics: &[RenderedGraphicsArtifact]) -> Vec<String> {
    let mut paths = Vec::new();
    for graphics in graphics {
        paths.extend(
            graphics_artifact_paths_for_one(graphics)
                .into_iter()
                .map(|path| path.display().to_string()),
        );
    }
    paths
}

pub fn codex_report_from_json(input: &str) -> PipelineResult<CodexProposalReport> {
    let value: Value = serde_json::from_str(input).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "report",
            "Codex proposal report is not valid JSON.",
            "Return a JSON object with generatedAt and proposal.",
        )
        .with_detail("error", error.to_string())]
    })?;

    let Some(object) = value.as_object() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "report",
            "Codex proposal report must be a JSON object.",
            "Return an object with generatedAt and proposal.",
        )
        .with_detail("error", "top-level value is not an object")]);
    };

    if !object.contains_key("proposal") {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "proposal",
            "Codex proposal report is missing the top-level proposal field.",
            "Add proposal with mediaId, clips, captions, overlays, hyperframes, and renderReview.",
        )]);
    }

    serde_json::from_value::<CodexProposalReport>(value).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "report",
            "Codex proposal report shape does not match the expected schema.",
            "Return generatedAt as a string and proposal as a valid Codex edit proposal.",
        )
        .with_detail("error", error.to_string())]
    })
}

fn create_output_dir(config: &RenderProposalConfig) -> PipelineResult<()> {
    std::fs::create_dir_all(&config.output_dir).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "args.outputDir",
            "Create render proposal output directory failed.",
            "Choose a writable output directory and retry the proposal render.",
        )
        .with_detail("path", config.output_dir.display().to_string())
        .with_detail("ioError", error.to_string())]
    })
}

fn read_codex_report(config: &RenderProposalConfig) -> PipelineResult<String> {
    std::fs::read_to_string(&config.codex_report_path).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "args.report",
            "Read Codex proposal report failed.",
            "Pass a readable JSON report generated by the Codex proposal step.",
        )
        .with_detail("path", config.codex_report_path.display().to_string())
        .with_detail("ioError", error.to_string())]
    })
}

fn ensure_source_video_exists(config: &RenderProposalConfig) -> PipelineResult<()> {
    if config.source_video_path.is_file() {
        return Ok(());
    }

    Err(vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "args.sourceVideo",
        "Source video does not exist or is not a file.",
        "Pass an existing source video file before rendering a Codex proposal.",
    )
    .with_detail(
        "path",
        config.source_video_path.display().to_string(),
    )])
}

fn project_from_source_probe(
    config: &RenderProposalConfig,
    report: &CodexProposalReport,
    probe: &MediaProbe,
) -> PipelineResult<VideoProject> {
    let duration_seconds = required_source_duration(probe)?;
    let video = probe.video.as_ref().ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "streams.video",
            "Source media does not contain a video stream.",
            "Choose a source video with a readable video stream.",
        )
        .with_detail("path", config.source_video_path.display().to_string())]
    })?;

    let mut project = VideoProject::new_empty(
        "render-proposal-project".to_string(),
        "Render Proposal Project".to_string(),
        report.generated_at.clone(),
    );
    project.media.push(MediaAsset {
        id: report.proposal.media_id.clone(),
        name: None,
        relative_path: project_relative_source_path(config),
        kind: MediaKind::Video,
        duration_seconds,
        width: video.width,
        height: video.height,
        fps: video.fps,
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "render-proposal-transcript".to_string(),
        media_id: report.proposal.media_id.clone(),
        engine: Some("render-proposal-minimal".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "proposal".to_string(),
            start_seconds: 0.0,
            end_seconds: duration_seconds.min(1.0),
            confidence: Some(1.0),
            speaker: None,
        }],
    });
    project.render_settings.width = video.width.unwrap_or(1920);
    project.render_settings.height = video.height.unwrap_or(1080);
    project.render_settings.fps = video.fps.unwrap_or(24.0);

    Ok(project)
}

fn request_for_report(report: &CodexProposalReport) -> EditJobRequest {
    EditJobRequest {
        media_id: report.proposal.media_id.clone(),
        preset: EditPreset::TrailerCut,
        prompt: "Render the accepted Codex edit proposal.".to_string(),
        target_duration_seconds: None,
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: report.generated_at.clone(),
    }
}

fn required_source_duration(probe: &MediaProbe) -> PipelineResult<f64> {
    let Some(duration_seconds) = probe.duration_seconds else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "format.duration",
            "Source media duration is missing from GStreamer discovery output.",
            "Use a source video with a readable finite duration.",
        )]);
    };

    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "format.duration",
            "Source media duration is invalid.",
            "Use a source video with a readable finite positive duration.",
        )
        .with_detail("durationSeconds", duration_seconds.to_string())]);
    }

    Ok(duration_seconds)
}

fn project_relative_source_path(config: &RenderProposalConfig) -> String {
    config
        .source_video_path
        .strip_prefix(&config.project_root)
        .unwrap_or(&config.source_video_path)
        .to_string_lossy()
        .into_owned()
}

fn labeled_output(sections: &[(&str, &str)]) -> String {
    sections
        .iter()
        .filter(|(_, output)| !output.trim().is_empty())
        .map(|(label, output)| format!("## {label}\n{output}"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn required_path(value: Option<PathBuf>, path: &str, flag: &str) -> PipelineResult<PathBuf> {
    value.ok_or_else(|| {
        vec![arg_error(
            path,
            "Missing required render proposal argument.",
            format!("Pass {flag} before rendering a Codex proposal."),
        )]
    })
}

fn arg_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> PipelineError {
    PipelineError::new(PipelineErrorCode::PipelineInputInvalid, path, message, fix)
}

fn is_render_proposal_flag(flag: &str) -> bool {
    matches!(
        flag,
        "--project-root"
            | "--source-video"
            | "--report"
            | "--output-dir"
            | "--final-name"
            | "--quality"
    )
}

fn arg_path(flag: &str) -> String {
    match flag {
        "--project-root" => "args.projectRoot".to_string(),
        "--source-video" => "args.sourceVideo".to_string(),
        "--report" => "args.report".to_string(),
        "--output-dir" => "args.outputDir".to_string(),
        "--final-name" => "args.finalName".to_string(),
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

fn proposal_duration_seconds(proposal: &CodexEditProposal) -> f64 {
    proposal
        .clips
        .iter()
        .map(|clip| clip.source_out - clip.source_in)
        .sum()
}

fn render_plan_duration_seconds(plan: &RenderPlan) -> f64 {
    let mut cursor_seconds = 0.0;
    let mut duration_seconds: f64 = 0.0;
    for clip in &plan.clips {
        let clip_duration_seconds = clip.source_out - clip.source_in;
        let start_seconds = clip.timeline_start_seconds.unwrap_or(cursor_seconds);
        let end_seconds = start_seconds + clip_duration_seconds;
        cursor_seconds = end_seconds;
        duration_seconds = duration_seconds.max(end_seconds);
    }
    duration_seconds
}

fn manifest_for_graphics_layer(layer: &GraphicsLayer) -> GraphicsArtifactManifest {
    GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("{}.preview", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count: 1,
        frames_pattern: GRAPHICS_FRAMES_PATTERN.to_string(),
        playback: graphics_playback_manifest(
            1,
            layer.fps,
            layer.duration_seconds,
            false,
            GRAPHICS_FRAMES_PATTERN,
        ),
        preview_path: PathBuf::from(GRAPHICS_PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    }
}

fn manifest_for_gpu_graphics_layer(layer: &GpuGraphicsLayer) -> GraphicsArtifactManifest {
    GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("{}.preview", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count: (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32,
        frames_pattern: GRAPHICS_FRAMES_PATTERN.to_string(),
        playback: graphics_playback_manifest(
            (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32,
            layer.fps,
            layer.duration_seconds,
            true,
            GRAPHICS_FRAMES_PATTERN,
        ),
        preview_path: PathBuf::from(GRAPHICS_PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    }
}

pub fn proposal_to_render_plan(
    project: &VideoProject,
    request: &EditJobRequest,
    proposal: &CodexEditProposal,
    input_path: &Path,
    output_path: &Path,
    quality: RenderQuality,
) -> PipelineResult<RenderPlan> {
    let selected_duration_seconds = proposal_selected_edl_duration(project, request, proposal)?;
    validate_proposal_visual_timing(proposal, selected_duration_seconds)?;
    validate_gpu_visual_timing(proposal, selected_duration_seconds)?;
    validate_codex_edit_proposal(project, request, proposal).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::CodexProposalInvalid,
            "proposal",
            "Codex edit proposal failed validation.",
            "Revise the proposal so its EDL, templates, and render-review checks pass validation.",
        )
        .with_detail("error", error.to_string())]
    })?;

    let options = ExportRenderOptions::for_project_defaults(
        ExportProfile::Webm,
        quality,
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
    let quality_settings = effective_quality_settings(
        options.quality,
        options.width,
        options.height,
        project.render_settings.fps,
    );

    Ok(RenderPlan {
        input_path: input_path.to_string_lossy().into_owned(),
        output_path: output_path.to_string_lossy().into_owned(),
        width: quality_settings.output_width,
        height: quality_settings.output_height,
        fps: quality_settings.output_fps,
        quality: options.quality,
        output_profile: RenderOutputProfile::default(),
        encode_tier: crate::edit::render_plan::ExportEncodeTier::Standard,
        clips: proposal
            .clips
            .iter()
            .map(|clip| RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: BTreeMap::new(),
                timeline_track_index: 0,
                source_in: clip.source_in,
                source_out: clip.source_out,
            })
            .collect(),
        audio_clips: Vec::new(),
        transitions: Vec::new(),
        audio_transitions: Vec::new(),
    })
}

fn proposal_selected_edl_duration(
    project: &VideoProject,
    request: &EditJobRequest,
    proposal: &CodexEditProposal,
) -> PipelineResult<f64> {
    let media = project
        .media
        .iter()
        .find(|media| media.id == request.media_id)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::CodexProposalInvalid,
                "proposal.mediaId",
                "Proposal media was not found.",
                "Use a mediaId that exists in the project before adding visuals.",
            )
            .with_detail("mediaId", request.media_id.clone())]
        })?;
    let mut source_durations_seconds =
        BTreeMap::from([(proposal.media_id.clone(), media.duration_seconds)]);
    for clip in &proposal.clips {
        let media_id = if clip.media_id.is_empty() {
            &proposal.media_id
        } else {
            &clip.media_id
        };
        let clip_media = project
            .media
            .iter()
            .find(|media| media.id == *media_id)
            .ok_or_else(|| {
                vec![PipelineError::new(
                    PipelineErrorCode::CodexProposalInvalid,
                    "proposal.clips.mediaId",
                    "Proposal clip media was not found.",
                    "Use a clip mediaId that exists in the project.",
                )
                .with_detail("mediaId", media_id.clone())]
            })?;
        source_durations_seconds.insert(media_id.clone(), clip_media.duration_seconds);
    }
    let edl = RoughCutEdl {
        media_id: proposal.media_id.clone(),
        source_duration_seconds: source_durations_seconds.values().sum(),
        source_durations_seconds,
        clips: proposal
            .clips
            .iter()
            .map(|clip| EdlClip {
                media_id: if clip.media_id.is_empty() {
                    proposal.media_id.clone()
                } else {
                    clip.media_id.clone()
                },
                source_in: clip.source_in,
                source_out: clip.source_out,
                reason: clip.reason.clone(),
                selection_reasons: vec![clip.reason.clone()],
                score: None,
            })
            .collect(),
    };
    validate_one_click_edl(&request.preset, &edl).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::CodexProposalInvalid,
            "proposal.clips",
            "Proposal EDL failed validation.",
            "Revise the proposal clips so the selected edit is a valid rough cut before adding visuals.",
        )
        .with_detail("error", error.to_string())]
    })?;

    Ok(edl.duration_seconds())
}

pub fn proposal_visuals_to_graphics_layers(
    proposal: &CodexEditProposal,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<Vec<GraphicsLayer>> {
    let mut layers = Vec::with_capacity(
        proposal.captions.len() + proposal.overlays.len() + proposal.hyperframes.len(),
    );

    for (index, caption) in proposal.captions.iter().enumerate() {
        layers.push(caption_to_graphics_layer(
            caption, index, width, height, fps,
        )?);
    }

    for (index, overlay) in proposal.overlays.iter().enumerate() {
        layers.push(overlay_to_graphics_layer(
            overlay, index, width, height, fps,
        )?);
    }

    for (index, hyperframe) in proposal.hyperframes.iter().enumerate() {
        layers.push(hyperframe_to_graphics_layer(
            hyperframe, index, width, height, fps,
        )?);
    }

    for layer in &layers {
        if let Err(errors) = validate_graphics_layer(layer) {
            return Err(PipelineError::from_graphics_errors(errors));
        }
    }

    Ok(layers)
}

pub fn proposal_visuals_to_graphics_layers_for_duration(
    proposal: &CodexEditProposal,
    width: u32,
    height: u32,
    fps: f64,
    render_duration_seconds: f64,
) -> PipelineResult<Vec<GraphicsLayer>> {
    validate_proposal_visual_timing(proposal, render_duration_seconds)?;
    proposal_visuals_to_graphics_layers(proposal, width, height, fps)
}

pub fn proposal_gpu_visuals_to_layers_for_duration(
    proposal: &CodexEditProposal,
    width: u32,
    height: u32,
    fps: f64,
    render_duration_seconds: f64,
) -> PipelineResult<Vec<GpuGraphicsLayer>> {
    validate_gpu_visual_timing(proposal, render_duration_seconds)?;
    let mut layers = Vec::with_capacity(proposal.gpu_visuals.len());
    for (index, visual) in proposal.gpu_visuals.iter().enumerate() {
        let layer = gpu_visual_to_layer(visual, index, width, height, fps)?;
        validate_gpu_graphics_layer(&layer).map_err(gpu_errors_to_pipeline_errors)?;
        layers.push(layer);
    }
    Ok(layers)
}

pub fn validate_proposal_visual_timing(
    proposal: &CodexEditProposal,
    render_duration_seconds: f64,
) -> PipelineResult<()> {
    if !render_duration_seconds.is_finite() || render_duration_seconds <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "proposal.durationSeconds",
            "Selected edit duration is invalid.",
            "Build a proposal with at least one positive selected clip range before adding visuals.",
        )]);
    }

    for (index, caption) in proposal.captions.iter().enumerate() {
        validate_visual_timing(
            caption,
            &format!("captions[{index}]"),
            render_duration_seconds,
        )?;
    }

    for (index, overlay) in proposal.overlays.iter().enumerate() {
        validate_visual_timing(
            overlay,
            &format!("overlays[{index}]"),
            render_duration_seconds,
        )?;
    }

    for (index, hyperframe) in proposal.hyperframes.iter().enumerate() {
        validate_visual_timing(
            hyperframe,
            &format!("hyperframes[{index}]"),
            render_duration_seconds,
        )?;
    }

    Ok(())
}

fn validate_gpu_visual_timing(
    proposal: &CodexEditProposal,
    render_duration_seconds: f64,
) -> PipelineResult<()> {
    if !render_duration_seconds.is_finite() || render_duration_seconds <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "proposal.durationSeconds",
            "Selected edit duration is invalid.",
            "Build a proposal with at least one positive selected clip range before adding GPU visuals.",
        )]);
    }

    for (index, visual) in proposal.gpu_visuals.iter().enumerate() {
        validate_visual_timing(
            visual,
            &format!("gpuVisuals[{index}]"),
            render_duration_seconds,
        )?;
    }
    Ok(())
}

fn gpu_visual_to_layer(
    visual: &Value,
    index: usize,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GpuGraphicsLayer> {
    let base_path = format!("gpuVisuals[{index}]");
    let id = required_string(visual, &base_path, "id")?;
    let kind = required_string(visual, &base_path, "kind")?;
    let timeline_start = required_non_negative_number(visual, &base_path, "startSeconds")?;
    let duration_seconds = required_positive_number(visual, &base_path, "durationSeconds")?;
    let quality_profile = required_quality_profile(visual, &base_path)?;
    let profile_id = parse_profile_id(&quality_profile).ok_or_else(|| {
        let supported = supported_profile_ids().join(", ");
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.qualityProfile"),
            "GPU visual quality profile is unsupported.",
            format!("Use one of these GPU visual quality profiles: {supported}."),
        )
        .with_detail("qualityProfile", quality_profile)]
    })?;
    validate_profile_kind(profile_id, &kind, &base_path)?;
    required_visual_metadata(visual, &base_path, "visualTreatment")?;
    required_visual_metadata(visual, &base_path, "motion")?;
    required_visual_metadata(visual, &base_path, "safeZone")?;
    required_visual_metadata(visual, &base_path, "avoid")?;
    let source_beat = required_gpu_source_beat(visual, &base_path)?;

    Ok(expand_profile(
        profile_id,
        GpuProfileExpansionInput {
            id,
            timeline_start,
            duration_seconds,
            dimensions: Dimensions { width, height },
            fps,
            source_beat,
        },
    ))
}

fn validate_profile_kind(
    profile_id: GpuVisualProfileId,
    kind: &str,
    base_path: &str,
) -> PipelineResult<()> {
    match profile_id {
        GpuVisualProfileId::HqNeonWireframeShaderV1 if kind == "hybrid_scene" => Ok(()),
        GpuVisualProfileId::HqNeonWireframeShaderV1 => Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.kind"),
            "GPU visual kind is incompatible with the selected quality profile.",
            format!("Use kind hybrid_scene for qualityProfile {HQ_NEON_WIREFRAME_SHADER_V1}."),
        )
        .with_detail("qualityProfile", HQ_NEON_WIREFRAME_SHADER_V1)]),
        profile_id if kind == "shader_background" => {
            let _ = profile_id;
            Ok(())
        }
        profile_id => {
            let quality_profile = profile_id_name(profile_id);
            Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.kind"),
                "GPU visual kind is incompatible with the selected quality profile.",
                format!("Use kind shader_background for qualityProfile {quality_profile}."),
            )
            .with_detail("qualityProfile", quality_profile)])
        }
    }
}

pub(crate) fn caption_to_graphics_layer(
    caption: &Value,
    index: usize,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GraphicsLayer> {
    let base_path = format!("captions[{index}]");
    let text = required_string(caption, &base_path, "text")?;
    let timeline_start = required_non_negative_number(caption, &base_path, "startSeconds")?;
    let duration_seconds = required_positive_number(caption, &base_path, "durationSeconds")?;
    let visual_treatment = required_visual_metadata(caption, &base_path, "visualTreatment")?;
    let motion = required_visual_metadata(caption, &base_path, "motion")?;
    let safe_zone = required_visual_metadata(caption, &base_path, "safeZone")?;
    let avoid = required_visual_metadata(caption, &base_path, "avoid")?;
    let source_beat = optional_non_empty_string(caption, "sourceBeat")
        .unwrap_or_else(|| format!("Caption emphasizes: {}", text));
    let placement = optional_non_empty_string(caption, "captionPlacement")
        .unwrap_or_else(|| "lower".to_string());
    if !matches!(placement.as_str(), "lower" | "center" | "upper") {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.captionPlacement"),
            "Caption placement must be lower, center, or upper.",
            "Use lower for subtitles, center for emphasis, or upper to avoid lower-frame action.",
        )]);
    }
    let style_preset = optional_non_empty_string(caption, "stylePreset")
        .unwrap_or_else(|| "boldReadableLower".to_string());
    let (backplate_fill, accent_fill, text_fill, font_weight, motion_preset) =
        match style_preset.as_str() {
            "boldReadableLower" => (
                "#101820CC",
                "#39D98AFF",
                "#FFFFFFFF",
                800,
                MotionPresetId::SnapPopV1,
            ),
            "kineticFocus" => (
                "#1018208C",
                "#FFD166FF",
                "#FFF7E6FF",
                900,
                MotionPresetId::PulseEmphasisV2,
            ),
            "centeredMinimal" => (
                "#111827A8",
                "#C4B5FDFF",
                "#F8FAFCFF",
                700,
                MotionPresetId::SoftDepthCardV2,
            ),
            _ => {
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    format!("{base_path}.stylePreset"),
                    "Caption style preset is unsupported.",
                    "Use boldReadableLower, kineticFocus, or centeredMinimal.",
                )]);
            }
        };
    let emphasis = caption_emphasis_ranges(
        caption,
        &text,
        &base_path,
        accent_fill,
        font_weight,
        duration_seconds,
    )?;

    let frame_width = width as f64;
    let frame_height = height as f64;
    let scale = proposal_layout_scale(width, height);
    let backing_width = frame_width * 0.76;
    let backing_height = (frame_height * 0.16)
        .max(96.0 * scale)
        .max(40.0)
        .min(frame_height * 0.5);
    let x = (frame_width - backing_width) / 2.0;
    let y = match placement.as_str() {
        "upper" => (frame_height * 0.1).max(0.0),
        "center" => ((frame_height - backing_height) / 2.0).max(0.0),
        _ => (frame_height - backing_height - frame_height * 0.08).max(0.0),
    };
    let text_pad_x = scaled_layout_value(scale, 42.0, 6.0, 42.0);
    let text_pad_top = scaled_layout_value(scale, 32.0, 5.0, 32.0);
    let text_pad_bottom = scaled_layout_value(scale, 12.0, 4.0, 12.0);
    let accent_inset = scaled_layout_value(scale, 28.0, 4.0, 28.0);
    let accent_y = y + scaled_layout_value(scale, 18.0, 3.0, 18.0);
    let accent_stroke = scaled_layout_value(scale, 4.0, 1.5, 4.0);
    let radius = scaled_layout_value(scale, 18.0, 6.0, 18.0);

    Ok(GraphicsLayer {
        schema_version: 1,
        id: optional_non_empty_string(caption, "id")
            .unwrap_or_else(|| format!("proposal-caption-{}", index + 1)),
        role: GraphicRole::Caption,
        timeline_start,
        duration_seconds,
        dimensions: Dimensions { width, height },
        fps,
        alpha: true,
        source_beat,
        visual_treatment,
        motion,
        safe_zone,
        avoid,
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "caption-backplate".to_string(),
                box_rect: Rect {
                    x,
                    y,
                    width: backing_width,
                    height: backing_height,
                },
                radius,
                fill: Color::Hex(backplate_fill.to_string()),
                animate: proposal_motion(motion_preset, "caption-backplate"),
            }),
            GraphicNode::Line(LineNode {
                id: "caption-accent".to_string(),
                points: vec![
                    Point {
                        x: x + accent_inset,
                        y: accent_y,
                    },
                    Point {
                        x: x + backing_width - accent_inset,
                        y: accent_y,
                    },
                ],
                stroke: Color::Hex(accent_fill.to_string()),
                stroke_width: accent_stroke,
                animate: proposal_motion(motion_preset, "caption-accent"),
            }),
            GraphicNode::Text(TextNode {
                id: "caption-text".to_string(),
                text,
                box_rect: Rect {
                    x: x + text_pad_x,
                    y: y + text_pad_top,
                    width: backing_width - text_pad_x * 2.0,
                    height: backing_height - text_pad_top - text_pad_bottom,
                },
                font_size: responsive_font_size(width, 34.0, 52.0),
                font_weight,
                align: "center".to_string(),
                fill: Color::Hex(text_fill.to_string()),
                max_lines: Some(2),
                text_reveal: None,
                emphasis,
                animate: proposal_motion(motion_preset, "caption-text"),
            }),
        ],
    })
}

fn caption_emphasis_ranges(
    caption: &Value,
    text: &str,
    base_path: &str,
    accent_fill: &str,
    font_weight: u16,
    duration_seconds: f64,
) -> PipelineResult<Vec<TextEmphasisRange>> {
    let Some(indices) = caption.get("emphasizedWordIndices") else {
        return Ok(Vec::new());
    };
    let Some(indices) = indices.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.emphasizedWordIndices"),
            "Caption emphasizedWordIndices must be an array of word indexes.",
            "Use zero-based indexes for words in the caption text.",
        )]);
    };
    let word_ranges = text_word_byte_ranges(text);
    let timings = caption_word_timing_ranges(caption, &word_ranges, base_path, duration_seconds)?;
    let animations = caption_word_animation_ranges(caption, &timings, base_path)?;
    let mut selected = std::collections::BTreeSet::new();
    for index in indices {
        let Some(index) = index.as_u64().and_then(|value| usize::try_from(value).ok()) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.emphasizedWordIndices"),
                "Caption word emphasis indexes must be non-negative integers.",
                "Use zero-based indexes for words in the caption text.",
            )]);
        };
        if index >= word_ranges.len() {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.emphasizedWordIndices"),
                "Caption word emphasis index is outside the caption text.",
                "Choose an index for an existing caption word.",
            )]);
        }
        selected.insert(index);
    }
    Ok(selected
        .into_iter()
        .map(|index| {
            let (start_byte, end_byte) = word_ranges[index];
            let animation = animations.get(&index);
            TextEmphasisRange {
                start_byte,
                end_byte,
                fill: Color::Hex(
                    animation
                        .map(|value| value.color.as_str())
                        .unwrap_or(accent_fill)
                        .to_string(),
                ),
                font_weight: font_weight.max(850),
                active_start_seconds: timings.get(&index).map(|(start, _)| *start),
                active_end_seconds: timings.get(&index).map(|(_, end)| *end),
                enter_start_seconds: animation.map(|value| value.enter_start),
                enter_end_seconds: animation.map(|value| value.enter_end),
                hold_end_seconds: animation.map(|value| value.hold_end),
                exit_end_seconds: animation.map(|value| value.exit_end),
                emphasis_scale: animation.map(|value| value.scale),
                emphasis_opacity: animation.map(|value| value.opacity),
                easing: animation.map(|value| value.easing),
            }
        })
        .collect())
}

struct CaptionWordAnimationValue {
    enter_start: f64,
    enter_end: f64,
    hold_end: f64,
    exit_end: f64,
    scale: f64,
    color: String,
    opacity: f64,
    easing: Easing,
}

fn caption_word_animation_ranges(
    caption: &Value,
    timings: &BTreeMap<usize, (f64, f64)>,
    base_path: &str,
) -> PipelineResult<BTreeMap<usize, CaptionWordAnimationValue>> {
    let Some(values) = caption.get("captionWordAnimations") else {
        return Ok(BTreeMap::new());
    };
    let Some(values) = values.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.captionWordAnimations"),
            "Caption word animations must be an array.",
            "Use one bounded animation object per emphasized word.",
        )]);
    };
    let mut result = BTreeMap::new();
    for value in values {
        let Some(value) = value.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordAnimations"),
                "Caption word animation must be an object.",
                "Use typed enter, hold, exit, scale, color, opacity, and easing fields.",
            )]);
        };
        let index = value
            .get("wordIndex")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        let Some(index) = index else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordAnimations.wordIndex"),
                "Caption word animation index is invalid.",
                "Use a zero-based timed word index.",
            )]);
        };
        let Some((word_start, word_end)) = timings.get(&index).copied() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordAnimations"),
                "Caption word animation has no matching transcript timing.",
                "Keep animation indexes aligned with captionWordTimings.",
            )]);
        };
        let numbers = [
            "enterStartSeconds",
            "enterEndSeconds",
            "holdEndSeconds",
            "exitEndSeconds",
            "emphasisScale",
            "emphasisOpacity",
        ]
        .map(|key| value.get(key).and_then(Value::as_f64));
        let [Some(enter_start), Some(enter_end), Some(hold_end), Some(exit_end), Some(scale), Some(opacity)] =
            numbers
        else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordAnimations"),
                "Caption word animation is missing numeric fields.",
                "Set complete enter, hold, exit, scale, and opacity values.",
            )]);
        };
        let color = value
            .get("emphasisColor")
            .and_then(Value::as_str)
            .unwrap_or("");
        let easing = match value.get("easing").and_then(Value::as_str) {
            Some("linear") => Easing::Linear,
            Some("outQuad") => Easing::OutQuad,
            Some("outBack") => Easing::OutBack,
            _ => {
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::PipelineInputInvalid,
                    format!("{base_path}.captionWordAnimations.easing"),
                    "Caption word animation easing is unsupported.",
                    "Use linear, outQuad, or outBack.",
                )])
            }
        };
        let animation_window_is_valid =
            [enter_start, enter_end, hold_end, exit_end, scale, opacity]
                .iter()
                .all(|value| value.is_finite())
                && word_start <= enter_start
                && enter_start <= enter_end
                && enter_end <= hold_end
                && hold_end <= exit_end
                && exit_end <= word_end
                && (0.5..=2.0).contains(&scale)
                && (0.0..=1.0).contains(&opacity);
        if !animation_window_is_valid || color.is_empty() || result.contains_key(&index) {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordAnimations"),
                "Caption word animation window is invalid or outside transcript word bounds.",
                "Keep enter, hold, and exit ordered inside the matching captionWordTimings range.",
            )]);
        }
        result.insert(
            index,
            CaptionWordAnimationValue {
                enter_start,
                enter_end,
                hold_end,
                exit_end,
                scale,
                color: color.to_string(),
                opacity,
                easing,
            },
        );
    }
    Ok(result)
}

fn caption_word_timing_ranges(
    caption: &Value,
    word_ranges: &[(usize, usize)],
    base_path: &str,
    duration_seconds: f64,
) -> PipelineResult<BTreeMap<usize, (f64, f64)>> {
    let Some(timings) = caption.get("captionWordTimings") else {
        return Ok(BTreeMap::new());
    };
    let Some(timings) = timings.as_array() else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.captionWordTimings"),
            "Caption word timings must be an array.",
            "Use wordIndex, startSeconds, and endSeconds for each timed caption word.",
        )]);
    };
    let mut ranges = BTreeMap::new();
    for timing in timings {
        let Some(timing) = timing.as_object() else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordTimings"),
                "Caption word timing must be an object.",
                "Use wordIndex, startSeconds, and endSeconds for each timed caption word.",
            )]);
        };
        let Some(word_index) = timing
            .get("wordIndex")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
        else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordTimings.wordIndex"),
                "Caption word timing index must be a non-negative integer.",
                "Use a zero-based index for a word in the caption text.",
            )]);
        };
        let start = timing.get("startSeconds").and_then(Value::as_f64);
        let end = timing.get("endSeconds").and_then(Value::as_f64);
        let Some((start, end)) = start.zip(end) else {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordTimings"),
                "Caption word timing requires startSeconds and endSeconds.",
                "Set a finite positive timing range within the caption duration.",
            )]);
        };
        if word_index >= word_ranges.len()
            || !start.is_finite()
            || !end.is_finite()
            || start < 0.0
            || end <= start
            || end > duration_seconds
            || ranges.insert(word_index, (start, end)).is_some()
        {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.captionWordTimings"),
                "Caption word timing is invalid or duplicated.",
                "Use unique word indexes with finite ranges inside the caption duration.",
            )]);
        }
    }
    Ok(ranges)
}

fn text_word_byte_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = start.take() {
                ranges.push((start, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(start) = start {
        ranges.push((start, text.len()));
    }
    ranges
}

fn hyperframe_to_graphics_layer(
    hyperframe: &Value,
    index: usize,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GraphicsLayer> {
    let base_path = format!("hyperframes[{index}]");
    let kind = required_string(hyperframe, &base_path, "kind")?;
    if !is_renderable_hyperframe_kind(&kind) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.kind"),
            "HyperFrame kind is not renderable yet.",
            "Use kind title_card, lower_third, diagram, transition, immersive_scene, or template_overlay, or ask Codex to express the layer as a caption, overlay, or template layer.",
        )
        .with_detail("kind", kind)]);
    }

    let timeline_start = required_non_negative_number(hyperframe, &base_path, "startSeconds")?;
    let duration_seconds = required_positive_number(hyperframe, &base_path, "durationSeconds")?;
    let visual_treatment = required_visual_metadata(hyperframe, &base_path, "visualTreatment")?;
    let motion = required_visual_metadata(hyperframe, &base_path, "motion")?;
    let safe_zone = required_visual_metadata(hyperframe, &base_path, "safeZone")?;
    let avoid = required_visual_metadata(hyperframe, &base_path, "avoid")?;
    validate_optional_motion_preset_id(hyperframe, &base_path)?;
    if overlay_visible_text(hyperframe).is_none() {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.visibleText"),
            "HyperFrame template overlay is missing visible text.",
            "Provide one of brief, fields.headline, headline, title, label, or text.",
        )]);
    }

    let template_id = hyperframe_template_id(hyperframe, &kind, &base_path)?;
    let template_layer = TemplateRenderLayer {
        item_id: optional_non_empty_string(hyperframe, "id")
            .unwrap_or_else(|| format!("proposal-hyperframe-{}", index + 1)),
        template_id: template_id.clone(),
        label: optional_non_empty_string(hyperframe, "brief")
            .unwrap_or_else(|| "Proposal HyperFrame".to_string()),
        timeline_start_seconds: timeline_start,
        duration_seconds,
        fields: hyperframe_template_fields(hyperframe, &kind, &template_id),
        visual_treatment,
        motion,
        safe_zone,
        avoid,
        preview_variant: optional_non_empty_string(hyperframe, "previewVariant")
            .unwrap_or_else(|| "hyperframe".to_string()),
    };

    let mut layer =
        template_layer_to_graphics_ir(&template_layer, Dimensions { width, height }, fps).map_err(
            |errors| {
                PipelineError::from_graphics_errors(errors)
                    .into_iter()
                    .map(|mut error| {
                        error.path = format!("{base_path}.{}", error.path);
                        error
                    })
                    .collect::<Vec<_>>()
            },
        )?;
    if kind == "lower_third" {
        layer.source_beat = optional_non_empty_string(hyperframe, "sourceBeat")
            .or_else(|| optional_non_empty_string(hyperframe, "brief"))
            .unwrap_or_else(|| "Lower-third HyperFrame scene".to_string());
    } else if kind == "diagram" {
        layer.role = GraphicRole::Diagram;
        layer.source_beat = optional_non_empty_string(hyperframe, "sourceBeat")
            .or_else(|| optional_non_empty_string(hyperframe, "brief"))
            .unwrap_or_else(|| "Diagram HyperFrame scene".to_string());
    } else if kind == "transition" {
        layer.role = GraphicRole::Transition;
        layer.source_beat = optional_non_empty_string(hyperframe, "sourceBeat")
            .or_else(|| optional_non_empty_string(hyperframe, "brief"))
            .unwrap_or_else(|| "Transition HyperFrame scene".to_string());
    } else if kind == "immersive_scene" {
        layer.source_beat = optional_non_empty_string(hyperframe, "sourceBeat")
            .or_else(|| optional_non_empty_string(hyperframe, "brief"))
            .unwrap_or_else(|| "Immersive HyperFrame scene".to_string());
    }
    Ok(layer)
}

fn is_renderable_hyperframe_kind(kind: &str) -> bool {
    matches!(
        kind,
        "template_overlay"
            | "title_card"
            | "lower_third"
            | "diagram"
            | "transition"
            | "immersive_scene"
    )
}

fn hyperframe_template_id(
    hyperframe: &Value,
    kind: &str,
    base_path: &str,
) -> PipelineResult<String> {
    if let Some(template_id) = optional_non_empty_string(hyperframe, "templateId") {
        return Ok(template_id);
    }

    match kind {
        "title_card" => Ok("chapter-card-v1".to_string()),
        "lower_third" => Ok("kinetic-lower-third-v1".to_string()),
        "diagram" => Ok("metric-callout-v1".to_string()),
        "transition" => Ok("gradient-background-loop-v1".to_string()),
        "immersive_scene" => Ok(proposal_immersive_scene_template_id(hyperframe)),
        _ => required_string(hyperframe, base_path, "templateId"),
    }
}

fn proposal_immersive_scene_template_id(hyperframe: &Value) -> String {
    let mut cues = Vec::new();
    for key in [
        "brief",
        "sourceBeat",
        "role",
        "visualTreatment",
        "motion",
        "safeZone",
    ] {
        if let Some(value) = optional_non_empty_string(hyperframe, key) {
            cues.push(value);
        }
    }
    if let Some(fields) = hyperframe.get("fields").and_then(Value::as_object) {
        cues.extend(
            fields
                .values()
                .filter_map(Value::as_str)
                .map(str::to_string),
        );
    }
    let cue_refs = cues.iter().map(String::as_str).collect::<Vec<_>>();

    if scene_cues_contain_any(&cue_refs, &["logo", "brand", "wordmark", "identity"]) {
        "holographic-logo-cutout-v1".to_string()
    } else if scene_cues_contain_any(
        &cue_refs,
        &[
            "metric",
            "data",
            "kpi",
            "retention",
            "conversion",
            "revenue",
            "percent",
            "percentage",
            "growth",
            "pricing",
            "price",
            "cost",
            "savings",
            "roi",
            "margin",
            "spend",
            "budget",
            "comparison",
            "compare",
            "versus",
            "before vs after",
            "split comparison",
        ],
    ) {
        "metric-callout-v1".to_string()
    } else if scene_cues_contain_any(
        &cue_refs,
        &[
            "risk",
            "warning",
            "alert",
            "security",
            "compliance",
            "failure",
            "blocked",
            "urgent",
        ],
    ) {
        "punchy-caption-v1".to_string()
    } else if scene_cues_contain_any(
        &cue_refs,
        &[
            "process",
            "roadmap",
            "workflow",
            "steps",
            "step",
            "sequence",
            "milestone",
            "rollout",
            "journey",
            "timeline",
            "schedule",
            "deadline",
            "calendar",
            "due date",
        ],
    ) {
        "chapter-card-v1".to_string()
    } else if scene_cues_contain_any(
        &cue_refs,
        &[
            "callout",
            "detail",
            "tracking",
            "highlight",
            "pointer",
            "look here",
            "watch this",
            "product",
            "feature",
            "spec",
            "leader line",
            "lens",
            "map",
            "route",
            "location",
            "place marker",
            "map pin",
            "arrival",
            "venue",
            "city map",
        ],
    ) {
        "tracking-highlight-v1".to_string()
    } else if scene_cues_contain_any(
        &cue_refs,
        &[
            "quote",
            "quoted",
            "emphasis",
            "emphasize",
            "punch",
            "punchy",
            "caption",
            "key line",
            "hook line",
            "testimonial",
            "interview",
            "speaker",
            "reaction",
            "emotional",
            "emotion",
            "surprised",
            "surprise",
            "wow",
        ],
    ) {
        "punchy-caption-v1".to_string()
    } else if scene_cues_contain_any(
        &cue_refs,
        &[
            "chapter",
            "story",
            "story beat",
            "reset",
            "section",
            "act break",
            "setup",
            "payoff",
            "proof segment",
            "launch",
            "announcement",
            "announce",
            "release",
        ],
    ) {
        "chapter-card-v1".to_string()
    } else {
        "gradient-background-loop-v1".to_string()
    }
}

fn scene_cues_contain_any(cues: &[&str], keywords: &[&str]) -> bool {
    let text = cues.join(" ").to_ascii_lowercase();
    let normalized = normalize_scene_cue_text(&text);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    keywords.iter().any(|keyword| {
        let keyword = normalize_scene_cue_text(keyword);
        if keyword.contains(' ') {
            normalized.contains(&keyword)
        } else {
            tokens.contains(&keyword.as_str())
        }
    })
}

fn normalize_scene_cue_text(value: &str) -> String {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn hyperframe_template_fields(
    hyperframe: &Value,
    kind: &str,
    template_id: &str,
) -> BTreeMap<String, String> {
    let mut fields = overlay_template_fields(hyperframe);
    if (kind == "title_card" || template_requires_subline(template_id))
        && !fields.contains_key("subline")
    {
        let subline = optional_non_empty_string(hyperframe, "sourceBeat")
            .or_else(|| optional_non_empty_string(hyperframe, "brief"))
            .or_else(|| optional_non_empty_string(hyperframe, "role"))
            .unwrap_or_else(|| "Scene beat".to_string());
        fields.insert("subline".to_string(), subline);
    }
    if template_id == "holographic-logo-cutout-v1" && !fields.contains_key("logoAssetId") {
        fields.insert(
            "logoAssetId".to_string(),
            "builtin:v-photo-light".to_string(),
        );
    }
    fields
}

fn template_requires_subline(template_id: &str) -> bool {
    matches!(
        template_id,
        "kinetic-lower-third-v1" | "metric-callout-v1" | "chapter-card-v1"
    )
}

pub(crate) fn overlay_to_graphics_layer(
    overlay: &Value,
    index: usize,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GraphicsLayer> {
    let base_path = format!("overlays[{index}]");
    let timeline_start = required_non_negative_number(overlay, &base_path, "startSeconds")?;
    let duration_seconds = required_positive_number(overlay, &base_path, "durationSeconds")?;
    let visual_treatment = required_visual_metadata(overlay, &base_path, "visualTreatment")?;
    let motion = required_visual_metadata(overlay, &base_path, "motion")?;
    let safe_zone = required_visual_metadata(overlay, &base_path, "safeZone")?;
    let avoid = required_visual_metadata(overlay, &base_path, "avoid")?;
    validate_optional_motion_preset_id(overlay, &base_path)?;
    let explicit_source_beat = optional_non_empty_string(overlay, "sourceBeat")
        .or_else(|| optional_non_empty_string(overlay, "brief"));
    let role = overlay_role(overlay);
    if let Some(template_id) = optional_non_empty_string(overlay, "templateId") {
        if overlay_visible_text(overlay).is_none() {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.visibleText"),
                "Overlay is missing visible text.",
                "Provide one of brief, fields.headline, headline, title, label, or text.",
            )]);
        }
        let template_layer = TemplateRenderLayer {
            item_id: optional_non_empty_string(overlay, "id")
                .unwrap_or_else(|| format!("proposal-overlay-{}", index + 1)),
            template_id,
            label: overlay
                .get("brief")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Proposal overlay")
                .to_string(),
            timeline_start_seconds: timeline_start,
            duration_seconds,
            fields: overlay_template_fields(overlay),
            visual_treatment,
            motion,
            safe_zone,
            avoid,
            preview_variant: optional_non_empty_string(overlay, "previewVariant")
                .unwrap_or_else(|| "template".to_string()),
        };
        return template_layer_to_graphics_ir(&template_layer, Dimensions { width, height }, fps)
            .map_err(|errors| {
                PipelineError::from_graphics_errors(errors)
                    .into_iter()
                    .map(|mut error| {
                        error.path = format!("{base_path}.{}", error.path);
                        error
                    })
                    .collect()
            });
    }
    if let Some(nodes) = custom_overlay_nodes(overlay, &base_path)? {
        let source_beat =
            explicit_source_beat.unwrap_or_else(|| "Custom proposal overlay".to_string());
        return Ok(GraphicsLayer {
            schema_version: 1,
            id: optional_non_empty_string(overlay, "id")
                .unwrap_or_else(|| format!("proposal-overlay-{}", index + 1)),
            role,
            timeline_start,
            duration_seconds,
            dimensions: Dimensions { width, height },
            fps,
            alpha: true,
            source_beat,
            visual_treatment,
            motion,
            safe_zone,
            avoid,
            nodes,
        });
    }

    let text = overlay_visible_text(overlay).ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.visibleText"),
            "Overlay is missing visible text.",
            "Provide one of brief, fields.headline, headline, title, label, or text.",
        )]
    })?;
    let source_beat = explicit_source_beat.unwrap_or_else(|| format!("Overlay callout: {text}"));

    let frame_width = width as f64;
    let frame_height = height as f64;
    let scale = proposal_layout_scale(width, height);
    let subline = overlay_subline_text(overlay);
    let has_subline = subline.is_some();
    let backing_width = frame_width * 0.44;
    let backing_height = if has_subline {
        (frame_height * 0.19)
            .max(132.0 * scale)
            .max(64.0)
            .min(frame_height * 0.55)
    } else {
        (frame_height * 0.13)
            .max(92.0 * scale)
            .max(36.0)
            .min(frame_height * 0.5)
    };
    let x = frame_width * 0.07;
    let y = (frame_height * 0.66)
        .min(frame_height - backing_height - frame_height * 0.08)
        .max(0.0);
    let text_pad_x = scaled_layout_value(scale, 42.0, 6.0, 42.0);
    let text_pad_y = scaled_layout_value(scale, 18.0, 4.0, 18.0);
    let accent_x = scaled_layout_value(scale, 18.0, 3.0, 18.0);
    let accent_width = scaled_layout_value(scale, 8.0, 2.0, 8.0);
    let radius = scaled_layout_value(scale, 16.0, 5.0, 16.0);
    let text_box_x = x + text_pad_x;
    let text_box_y = y + text_pad_y;
    let text_box_width = backing_width - text_pad_x - scaled_layout_value(scale, 22.0, 6.0, 22.0);
    let text_box_height = backing_height - text_pad_y * 2.0;
    let mut nodes = vec![
        GraphicNode::RoundedRect(RoundedRectNode {
            id: "overlay-backplate".to_string(),
            box_rect: Rect {
                x,
                y,
                width: backing_width,
                height: backing_height,
            },
            radius,
            fill: Color::Hex("#0B1020C2".to_string()),
            animate: proposal_motion(MotionPresetId::SlideFadeUpV1, "overlay-backplate"),
        }),
        GraphicNode::RoundedRect(RoundedRectNode {
            id: "overlay-accent-pill".to_string(),
            box_rect: Rect {
                x: x + accent_x,
                y: y + text_pad_y,
                width: accent_width,
                height: backing_height - text_pad_y * 2.0,
            },
            radius: accent_width / 2.0,
            fill: Color::Hex("#FFB703FF".to_string()),
            animate: proposal_motion(MotionPresetId::SlideFadeUpV1, "overlay-accent-pill"),
        }),
    ];

    if let Some(subline) = subline {
        let headline_font_size = responsive_font_size(width, 24.0, 38.0);
        let subline_font_size = responsive_font_size(width, 18.0, 30.0);
        let headline_height = (headline_font_size * 1.28).min(text_box_height * 0.62);
        nodes.push(GraphicNode::Text(TextNode {
            id: "overlay-text".to_string(),
            text,
            box_rect: Rect {
                x: text_box_x,
                y: text_box_y,
                width: text_box_width,
                height: headline_height,
            },
            font_size: headline_font_size,
            font_weight: 760,
            align: "left".to_string(),
            fill: Color::Hex("#FFFFFFFF".to_string()),
            max_lines: Some(1),
            text_reveal: None,
            emphasis: Vec::new(),
            animate: proposal_motion(MotionPresetId::SlideFadeUpV1, "overlay-text"),
        }));
        nodes.push(GraphicNode::Text(TextNode {
            id: "overlay-subline".to_string(),
            text: subline,
            box_rect: Rect {
                x: text_box_x,
                y: text_box_y + headline_height,
                width: text_box_width,
                height: text_box_height - headline_height,
            },
            font_size: subline_font_size,
            font_weight: 520,
            align: "left".to_string(),
            fill: Color::Hex("#DDE6F3FF".to_string()),
            max_lines: Some(2),
            text_reveal: None,
            emphasis: Vec::new(),
            animate: proposal_motion(MotionPresetId::SlideFadeUpV1, "overlay-subline"),
        }));
    } else {
        nodes.push(GraphicNode::Text(TextNode {
            id: "overlay-text".to_string(),
            text,
            box_rect: Rect {
                x: text_box_x,
                y: text_box_y,
                width: text_box_width,
                height: text_box_height,
            },
            font_size: responsive_font_size(width, 28.0, 44.0),
            font_weight: 760,
            align: "left".to_string(),
            fill: Color::Hex("#FFFFFFFF".to_string()),
            max_lines: Some(2),
            text_reveal: None,
            emphasis: Vec::new(),
            animate: proposal_motion(MotionPresetId::SlideFadeUpV1, "overlay-text"),
        }));
    }

    Ok(GraphicsLayer {
        schema_version: 1,
        id: optional_non_empty_string(overlay, "id")
            .unwrap_or_else(|| format!("proposal-overlay-{}", index + 1)),
        role,
        timeline_start,
        duration_seconds,
        dimensions: Dimensions { width, height },
        fps,
        alpha: true,
        source_beat,
        visual_treatment,
        motion,
        safe_zone,
        avoid,
        nodes,
    })
}

fn custom_overlay_nodes(
    overlay: &Value,
    base_path: &str,
) -> PipelineResult<Option<Vec<GraphicNode>>> {
    let Some(nodes) = overlay.get("nodes") else {
        return Ok(None);
    };

    let mut nodes = serde_json::from_value::<Vec<GraphicNode>>(nodes.clone()).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.nodes"),
            "Custom overlay nodes do not match the Rust graphics IR schema.",
            "Use nodes with type text, rect, roundedRect, polygon, line, or imageRef and valid camelCase fields; built-in templates may expand to internal holographicLogo nodes.",
        )
        .with_detail("serdeError", error.to_string())]
    })?;
    let motion_preset = optional_non_empty_string(overlay, "motionPresetId")
        .and_then(|motion_preset_id| parse_motion_preset_id(&motion_preset_id))
        .unwrap_or(MotionPresetId::SlideFadeUpV1);
    apply_missing_motion_preset(&mut nodes, motion_preset);

    Ok(Some(nodes))
}

fn apply_missing_motion_preset(nodes: &mut [GraphicNode], preset: MotionPresetId) {
    for node in nodes {
        if node.animation().is_some() {
            continue;
        }

        let node_id = node.id().to_string();
        let animation = motion_preset_animation(preset, &node_id);
        match node {
            GraphicNode::Text(node) => node.animate = Some(animation),
            GraphicNode::RoundedRect(node) => node.animate = Some(animation),
            GraphicNode::Rect(node) => node.animate = Some(animation),
            GraphicNode::Polygon(node) => node.animate = Some(animation),
            GraphicNode::Line(node) => node.animate = Some(animation),
            GraphicNode::ImageRef(node) => node.animate = Some(animation),
            GraphicNode::HolographicLogo(node) => node.animate = Some(animation),
        }
    }
}

fn overlay_template_fields(overlay: &Value) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    if let Some(object) = overlay.get("fields").and_then(Value::as_object) {
        for (key, value) in object {
            if let Some(value) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                fields.insert(key.clone(), value.to_string());
            }
        }
    }
    for (field, key) in [
        ("headline", "headline"),
        ("title", "headline"),
        ("label", "headline"),
        ("text", "headline"),
        ("brief", "headline"),
        ("subline", "subline"),
    ] {
        if !fields.contains_key(key) {
            if let Some(value) = overlay
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                fields.insert(key.to_string(), value.to_string());
            }
        }
    }
    fields
}

fn overlay_role(overlay: &Value) -> GraphicRole {
    match optional_non_empty_string(overlay, "kind").as_deref() {
        Some("lower_third") => GraphicRole::LowerThird,
        Some("title_card") => GraphicRole::TitleCard,
        Some("diagram") => GraphicRole::Diagram,
        Some("transition") => GraphicRole::Transition,
        _ => GraphicRole::Overlay,
    }
}

fn proposal_motion(preset: MotionPresetId, node_id: &str) -> Option<NodeAnimation> {
    Some(motion_preset_animation(preset, node_id))
}

fn required_visual_metadata(value: &Value, base_path: &str, field: &str) -> PipelineResult<String> {
    required_string_with_message(
        value,
        base_path,
        field,
        "Required visual metadata is missing.",
        format!("Set {base_path}.{field} to a concise non-empty visual direction."),
    )
}

fn validate_optional_motion_preset_id(value: &Value, base_path: &str) -> PipelineResult<()> {
    let Some(motion_preset_id) = optional_non_empty_string(value, "motionPresetId") else {
        return Ok(());
    };

    if parse_motion_preset_id(&motion_preset_id).is_some() {
        return Ok(());
    }

    Err(vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        format!("{base_path}.motionPresetId"),
        "Motion preset is unsupported.",
        format!(
            "Use one of the built-in motion presets: {}.",
            SUPPORTED_MOTION_PRESETS.join(", ")
        ),
    )
    .with_detail("motionPresetId", motion_preset_id)])
}

fn required_gpu_source_beat(value: &Value, base_path: &str) -> PipelineResult<String> {
    required_string_with_message(
        value,
        base_path,
        "sourceBeat",
        "GPU visual source beat is missing.",
        format!("Set {base_path}.sourceBeat to the edit beat this generated visual supports."),
    )
}

fn required_string(value: &Value, base_path: &str, field: &str) -> PipelineResult<String> {
    required_string_with_message(
        value,
        base_path,
        field,
        "Required proposal text is missing.",
        format!("Set {base_path}.{field} to a concise non-empty string."),
    )
}

fn required_quality_profile(value: &Value, base_path: &str) -> PipelineResult<String> {
    value
        .get("qualityProfile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.qualityProfile"),
                "GPU visual quality profile is missing.",
                format!(
                    "Set {base_path}.qualityProfile to one of: {}.",
                    supported_profile_ids().join(", ")
                ),
            )]
        })
}

fn validate_visual_timing(
    visual: &Value,
    base_path: &str,
    render_duration_seconds: f64,
) -> PipelineResult<()> {
    let start_seconds = required_non_negative_number(visual, base_path, "startSeconds")?;
    let duration_seconds = required_positive_number(visual, base_path, "durationSeconds")?;
    let visual_end_seconds = start_seconds + duration_seconds;

    if start_seconds - render_duration_seconds > VISUAL_TIMING_TOLERANCE_SECONDS {
        return Err(vec![visual_timing_out_of_range_error(
            base_path,
            "startSeconds",
            render_duration_seconds,
            visual_end_seconds,
        )]);
    }

    if visual_end_seconds - render_duration_seconds > VISUAL_TIMING_TOLERANCE_SECONDS {
        return Err(vec![visual_timing_out_of_range_error(
            base_path,
            "durationSeconds",
            render_duration_seconds,
            visual_end_seconds,
        )]);
    }

    Ok(())
}

fn visual_timing_out_of_range_error(
    base_path: &str,
    field: &str,
    render_duration_seconds: f64,
    visual_end_seconds: f64,
) -> PipelineError {
    PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        format!("{base_path}.{field}"),
        "Visual timing exceeds the selected edit duration.",
        format!(
            "Keep {base_path} within the selected edit duration; set startSeconds + durationSeconds <= {:.3}.",
            render_duration_seconds
        ),
    )
    .with_detail(
        "renderDurationSeconds",
        format!("{render_duration_seconds:.3}"),
    )
    .with_detail("visualEndSeconds", format!("{visual_end_seconds:.3}"))
}

fn required_string_with_message(
    value: &Value,
    base_path: &str,
    field: &str,
    message: &str,
    fix: String,
) -> PipelineResult<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.{field}"),
                message,
                fix,
            )]
        })
}

fn required_non_negative_number(
    value: &Value,
    base_path: &str,
    field: &str,
) -> PipelineResult<f64> {
    let Some(number) = value.get(field).and_then(Value::as_f64) else {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.{field}"),
            "Required proposal timing is missing.",
            format!("Set {base_path}.{field} to a finite number greater than or equal to 0."),
        )]);
    };

    if !number.is_finite() || number < 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.{field}"),
            "Proposal timing is invalid.",
            format!("Set {base_path}.{field} to a finite number greater than or equal to 0."),
        )]);
    }

    Ok(number)
}

fn required_positive_number(value: &Value, base_path: &str, field: &str) -> PipelineResult<f64> {
    let number = required_non_negative_number(value, base_path, field)?;
    if number <= 0.0 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.{field}"),
            "Proposal duration is invalid.",
            format!("Set {base_path}.{field} to a finite number greater than 0."),
        )]);
    }

    Ok(number)
}

fn optional_non_empty_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn overlay_visible_text(overlay: &Value) -> Option<String> {
    [
        overlay
            .get("fields")
            .and_then(|fields| fields.get("headline"))
            .and_then(Value::as_str),
        overlay.get("headline").and_then(Value::as_str),
        overlay.get("title").and_then(Value::as_str),
        overlay.get("label").and_then(Value::as_str),
        overlay.get("text").and_then(Value::as_str),
        overlay.get("brief").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|text| !text.is_empty())
    .map(str::to_string)
}

fn overlay_subline_text(overlay: &Value) -> Option<String> {
    overlay
        .get("fields")
        .and_then(|fields| fields.get("subline"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn responsive_font_size(width: u32, _min: f64, max: f64) -> f64 {
    ((width as f64) * 0.032).clamp(14.0, max)
}

fn proposal_layout_scale(width: u32, height: u32) -> f64 {
    ((width as f64) / 1280.0)
        .min((height as f64) / 720.0)
        .max(0.1)
}

fn scaled_layout_value(scale: f64, base: f64, min: f64, max: f64) -> f64 {
    (base * scale).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_word_emphasis_becomes_a_graphics_text_span() {
        let caption = serde_json::json!({
            "id": "caption-emphasis",
            "text": "Keep Zürich clear",
            "startSeconds": 0.0,
            "durationSeconds": 1.0,
            "visualTreatment": "phone-readable caption with one accented word",
            "motion": "quick pop in and soft fade out",
            "safeZone": "inside 10 percent margins",
            "avoid": "opaque slabs",
            "stylePreset": "kineticFocus",
            "emphasizedWordIndices": [1],
            "captionWordTimings": [{
                "wordIndex": 1,
                "startSeconds": 0.2,
                "endSeconds": 0.7
            }]
        });

        let layer = caption_to_graphics_layer(&caption, 0, 1920, 1080, 30.0)
            .expect("caption graphics layer");
        let GraphicNode::Text(text) = &layer.nodes[2] else {
            panic!("caption text node should be third");
        };

        assert_eq!(text.emphasis.len(), 1);
        assert_eq!(text.emphasis[0].start_byte, 5);
        assert_eq!(text.emphasis[0].end_byte, 12);
        assert_eq!(text.emphasis[0].fill, Color::Hex("#FFD166FF".to_string()));
        assert!(text.emphasis[0].font_weight >= 900);
        assert_eq!(text.emphasis[0].active_start_seconds, Some(0.2));
        assert_eq!(text.emphasis[0].active_end_seconds, Some(0.7));
    }
}
