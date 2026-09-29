use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use video_creater_lib::gpu_graphics::ir::{
    GpuGraphicRole, GpuGraphicsLayer, ShaderLanguage, ShaderPass,
};
use video_creater_lib::gpu_graphics::renderer::{render_gpu_graphics_layer, GpuRenderOptions};
use video_creater_lib::gpu_graphics::templates::{
    project_shader_background_templates, shared_shader_utils, ShaderBackgroundTemplate,
};
use video_creater_lib::gpu_graphics::visual_qa::{run_hq_visual_qa, VisualQaOptions};
use video_creater_lib::graphics::ir::Dimensions;

const DEFAULT_OUTPUT_DIR: &str = "output/shader-template-videos";
const DEFAULT_DURATION_SECONDS: f64 = 10.0;
const DEFAULT_FPS: f64 = 30.0;
const DEFAULT_WIDTH: u32 = 640;
const DEFAULT_HEIGHT: u32 = 360;
const DURATION_TOLERANCE_SECONDS: f64 = 0.15;

#[derive(Debug, Clone)]
struct Config {
    project_root: PathBuf,
    output_dir: PathBuf,
    duration_seconds: f64,
    fps: f64,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchReport {
    schema_version: u32,
    generated_at: String,
    project_root: String,
    output_dir: String,
    duration_seconds: f64,
    fps: f64,
    dimensions: Dimensions,
    template_count: usize,
    videos: Vec<TemplateVideoReport>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateVideoReport {
    template_id: String,
    title: String,
    source_kind: String,
    video_path: String,
    artifact_dir: String,
    frame_count: u32,
    visual_qa: serde_json::Value,
    validation: VideoValidationReport,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoValidationReport {
    path: String,
    duration_seconds: f64,
    width: u32,
    height: u32,
    video_stream_count: usize,
    codec_name: String,
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: FfprobeFormat,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = parse_args(std::env::args().skip(1))?;
    validate_config(&config)?;
    std::fs::create_dir_all(&config.output_dir)
        .with_context(|| format!("create output dir {}", config.output_dir.display()))?;

    let templates = project_shader_background_templates(&config.project_root)
        .map_err(|error| anyhow!(error.to_string()))?;
    if templates.is_empty() {
        bail!("no shader background templates found");
    }

    let mut videos = Vec::with_capacity(templates.len());
    for template in templates {
        println!(
            "rendering {} ({})",
            template.config.id, template.config.title
        );
        videos.push(render_template_video(&config, &template)?);
    }

    let report = BatchReport {
        schema_version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        project_root: config.project_root.display().to_string(),
        output_dir: config.output_dir.display().to_string(),
        duration_seconds: config.duration_seconds,
        fps: config.fps,
        dimensions: Dimensions {
            width: config.width,
            height: config.height,
        },
        template_count: videos.len(),
        videos,
    };
    write_batch_report(&config.output_dir, &report)?;

    println!(
        "{}",
        serde_json::json!({
            "ok": true,
            "templateCount": report.template_count,
            "outputDir": report.output_dir,
            "reportPath": config.output_dir.join("manifest.json"),
        })
    );
    Ok(())
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Config> {
    let current_dir = std::env::current_dir().context("resolve current dir")?;
    let mut config = Config {
        project_root: current_dir.clone(),
        output_dir: current_dir.join(DEFAULT_OUTPUT_DIR),
        duration_seconds: DEFAULT_DURATION_SECONDS,
        fps: DEFAULT_FPS,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    };

    for arg in args {
        let Some((key, value)) = arg.split_once('=') else {
            bail!("unsupported argument {arg}; use --key=value");
        };
        match key {
            "--project-root" => config.project_root = PathBuf::from(value),
            "--output-dir" => config.output_dir = PathBuf::from(value),
            "--duration" => {
                config.duration_seconds = value
                    .parse()
                    .with_context(|| format!("parse --duration={value}"))?
            }
            "--fps" => {
                config.fps = value
                    .parse()
                    .with_context(|| format!("parse --fps={value}"))?
            }
            "--width" => {
                config.width = value
                    .parse()
                    .with_context(|| format!("parse --width={value}"))?
            }
            "--height" => {
                config.height = value
                    .parse()
                    .with_context(|| format!("parse --height={value}"))?
            }
            _ => bail!("unsupported argument {key}"),
        }
    }

    if !config.project_root.is_absolute() {
        config.project_root = current_dir.join(config.project_root);
    }
    if !config.output_dir.is_absolute() {
        config.output_dir = current_dir.join(config.output_dir);
    }

    Ok(config)
}

fn validate_config(config: &Config) -> Result<()> {
    if !config.duration_seconds.is_finite() || config.duration_seconds <= 0.0 {
        bail!("duration must be finite and positive");
    }
    if !config.fps.is_finite() || config.fps <= 0.0 {
        bail!("fps must be finite and positive");
    }
    if (config.duration_seconds * config.fps).ceil() > 600.0 {
        bail!("duration * fps must fit the GPU frame budget of 600 frames");
    }
    if config.width == 0 || config.height == 0 {
        bail!("width and height must be positive");
    }

    Ok(())
}

fn render_template_video(
    config: &Config,
    template: &ShaderBackgroundTemplate,
) -> Result<TemplateVideoReport> {
    let template_dir = config.output_dir.join(&template.config.id);
    std::fs::create_dir_all(&template_dir)
        .with_context(|| format!("create template dir {}", template_dir.display()))?;

    let layer = layer_for_template(config, template)?;
    let manifest = render_gpu_graphics_layer(
        &layer,
        GpuRenderOptions {
            output_dir: template_dir.clone(),
        },
    )
    .map_err(|errors| anyhow!("GPU render failed for {}: {errors:?}", template.config.id))?;

    let visual_qa = run_hq_visual_qa(
        &template_dir,
        config.width,
        config.height,
        manifest.frame_count,
        shader_template_visual_qa_options(),
    )
    .map_err(|errors| anyhow!("visual QA failed for {}: {errors:?}", template.config.id))?;
    let visual_qa = serde_json::to_value(visual_qa).context("serialize visual QA report")?;

    let video_path = template_dir.join(format!("{}.webm", template.config.id));
    encode_webm(&template_dir, config.fps, &video_path)?;
    let validation = validate_video(&video_path, config)?;

    let report = TemplateVideoReport {
        template_id: template.config.id.clone(),
        title: template.config.title.clone(),
        source_kind: serde_json::to_value(&template.config.source_kind)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string()),
        video_path: video_path.display().to_string(),
        artifact_dir: template_dir.display().to_string(),
        frame_count: manifest.frame_count,
        visual_qa,
        validation,
    };
    let report_json = serde_json::to_string_pretty(&report).context("serialize template report")?;
    std::fs::write(template_dir.join("validation.json"), report_json)
        .with_context(|| format!("write validation report for {}", template.config.id))?;

    Ok(report)
}

fn layer_for_template(
    config: &Config,
    template: &ShaderBackgroundTemplate,
) -> Result<GpuGraphicsLayer> {
    Ok(GpuGraphicsLayer {
        schema_version: 1,
        id: format!("template-video-{}", template.config.id),
        role: GpuGraphicRole::ShaderBackground,
        timeline_start: 0.0,
        duration_seconds: config.duration_seconds,
        dimensions: Dimensions {
            width: config.width,
            height: config.height,
        },
        fps: config.fps,
        alpha: template.config.render_contract.alpha,
        source_beat: format!("Ten-second template preview for {}", template.config.title),
        visual_treatment: template.config.visual_treatment.clone(),
        motion: template.config.motion.clone(),
        safe_zone: template.config.safe_zone.clone(),
        avoid: template.config.avoid.clone(),
        quality_profile: Some(template.config.shader_profile_id.clone()),
        background: Some(ShaderPass {
            shader_language: ShaderLanguage::Glsl,
            fragment_source: fragment_source_for_template(template)?,
            uniforms: Default::default(),
        }),
        scene: None,
    })
}

fn shader_template_visual_qa_options() -> VisualQaOptions {
    VisualQaOptions {
        min_opaque_ratio: 0.05,
        min_visible_ratio: 0.05,
        max_saturated_ratio: 0.95,
        min_edge_ratio: 0.0,
        min_temporal_delta: 0.0001,
    }
}

fn fragment_source_for_template(template: &ShaderBackgroundTemplate) -> Result<String> {
    let mut chunks = Vec::new();
    for utility_ref in &template.config.utility_refs {
        if utility_ref == "shared/utils.glsl" {
            chunks.push(shared_shader_utils().trim().to_string());
        } else {
            let shader_dir = Path::new(&template.shader_path)
                .parent()
                .ok_or_else(|| anyhow!("template shader path has no parent"))?;
            let utility_path = shader_dir.join(utility_ref);
            chunks.push(
                std::fs::read_to_string(&utility_path)
                    .with_context(|| format!("read utility {}", utility_path.display()))?
                    .trim()
                    .to_string(),
            );
        }
    }
    chunks.push(template.fragment_body.trim().to_string());
    Ok(chunks.join("\n\n"))
}

fn encode_webm(artifact_dir: &Path, fps: f64, video_path: &Path) -> Result<()> {
    let input_pattern = artifact_dir.join("frames/frame-%06d.png");
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-framerate")
        .arg(format!("{fps:.6}"))
        .arg("-i")
        .arg(&input_pattern)
        .arg("-an")
        .arg("-c:v")
        .arg("libvpx-vp9")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-deadline")
        .arg("good")
        .arg("-cpu-used")
        .arg("4")
        .arg("-crf")
        .arg("32")
        .arg("-b:v")
        .arg("0")
        .arg(video_path)
        .output()
        .with_context(|| format!("spawn ffmpeg for {}", video_path.display()))?;

    if !output.status.success() {
        bail!(
            "ffmpeg failed for {}: {}",
            video_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn validate_video(video_path: &Path, config: &Config) -> Result<VideoValidationReport> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_streams")
        .arg("-show_format")
        .arg("-of")
        .arg("json")
        .arg(video_path)
        .output()
        .with_context(|| format!("spawn ffprobe for {}", video_path.display()))?;

    if !output.status.success() {
        bail!(
            "ffprobe failed for {}: {}",
            video_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let probe: FfprobeOutput =
        serde_json::from_slice(&output.stdout).context("parse ffprobe JSON")?;
    let duration_seconds = probe
        .format
        .duration
        .as_deref()
        .ok_or_else(|| anyhow!("ffprobe format duration is missing"))?
        .parse::<f64>()
        .context("parse ffprobe format duration")?;
    if (duration_seconds - config.duration_seconds).abs() > DURATION_TOLERANCE_SECONDS {
        bail!(
            "video duration mismatch for {}: expected {:.3}s, got {:.3}s",
            video_path.display(),
            config.duration_seconds,
            duration_seconds
        );
    }

    let video_streams = probe
        .streams
        .iter()
        .filter(|stream| stream.codec_type.as_deref() == Some("video"))
        .collect::<Vec<_>>();
    if video_streams.len() != 1 {
        bail!(
            "expected exactly one video stream for {}, got {}",
            video_path.display(),
            video_streams.len()
        );
    }
    let stream = video_streams[0];
    let width = stream
        .width
        .ok_or_else(|| anyhow!("ffprobe video stream width is missing"))?;
    let height = stream
        .height
        .ok_or_else(|| anyhow!("ffprobe video stream height is missing"))?;
    if width != config.width || height != config.height {
        bail!(
            "video dimensions mismatch for {}: expected {}x{}, got {}x{}",
            video_path.display(),
            config.width,
            config.height,
            width,
            height
        );
    }

    Ok(VideoValidationReport {
        path: video_path.display().to_string(),
        duration_seconds,
        width,
        height,
        video_stream_count: video_streams.len(),
        codec_name: stream
            .codec_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn write_batch_report(output_dir: &Path, report: &BatchReport) -> Result<()> {
    let manifest_json = serde_json::to_string_pretty(report).context("serialize batch manifest")?;
    std::fs::write(output_dir.join("manifest.json"), manifest_json)
        .with_context(|| format!("write {}", output_dir.join("manifest.json").display()))?;

    let mut markdown = String::new();
    markdown.push_str("# Shader Template Video Validation\n\n");
    markdown.push_str(&format!(
        "- Templates: {}\n- Duration: {:.3}s\n- Dimensions: {}x{}\n- FPS: {:.3}\n\n",
        report.template_count,
        report.duration_seconds,
        report.dimensions.width,
        report.dimensions.height,
        report.fps
    ));
    markdown.push_str("| Template | Video | Duration | Codec | QA temporal delta |\n");
    markdown.push_str("| --- | --- | ---: | --- | ---: |\n");
    for video in &report.videos {
        let temporal_delta = video
            .visual_qa
            .get("temporalDelta")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        markdown.push_str(&format!(
            "| {} | {} | {:.3}s | {} | {:.6} |\n",
            video.template_id,
            video.video_path,
            video.validation.duration_seconds,
            video.validation.codec_name,
            temporal_delta
        ));
    }
    std::fs::write(output_dir.join("README.md"), markdown)
        .with_context(|| format!("write {}", output_dir.join("README.md").display()))?;

    Ok(())
}
