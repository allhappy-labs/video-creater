//! Project-scoped template rendering.
//!
//! This renders a single motion template from a self-contained project folder:
//! `video-creater.project.json` plus `render-template.json`, media/assets, generated artifacts,
//! renders, and logs.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::edit::render_plan::TemplateRenderLayer;
use crate::graphics::assets::AssetRegistry;
use crate::graphics::ir::Dimensions;
use crate::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};
use crate::graphics::templates::template_layer_to_graphics_ir;
use crate::graphics::webm_export::{export_frame_sequence_to_webm, WebmEncoder, WebmExportOptions};
use crate::project::storage::{load_project, ProjectStorageError};
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};

pub const TEMPLATE_RENDER_FILE_NAME: &str = "render-template.json";
const GENERATED_GRAPHICS_DIR_NAME: &str = "generated/graphics";
const RENDERS_DIR_NAME: &str = "renders";
const TEMPLATE_RENDER_REPORT_FILE_NAME: &str = "render-report.json";
const DEFAULT_SCHEMA_VERSION: u32 = 1;
const DEFAULT_TARGET_BITRATE_BPS: u32 = 8_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTemplateProjectConfig {
    pub project_root: PathBuf,
    pub template_config_path: PathBuf,
}

impl RenderTemplateProjectConfig {
    pub fn from_project_root(project_root: PathBuf) -> Self {
        let template_config_path = project_root.join(TEMPLATE_RENDER_FILE_NAME);
        Self {
            project_root,
            template_config_path,
        }
    }

    pub fn renders_dir(&self) -> PathBuf {
        self.project_root.join(RENDERS_DIR_NAME)
    }

    pub fn render_report_path(&self) -> PathBuf {
        self.renders_dir().join(TEMPLATE_RENDER_REPORT_FILE_NAME)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRenderConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub template_id: String,
    pub output_name: String,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    pub render_settings: TemplateRenderSettings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_treatment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avoid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_seconds: f64,
    pub format: TemplateRenderFormat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemplateRenderFormat {
    Webm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRenderReport {
    pub status: String,
    pub template_id: String,
    pub output_name: String,
    pub video_path: String,
    pub graphics_dir: String,
    pub preview_path: String,
    pub manifest_path: String,
    pub frame_count: u32,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_seconds: f64,
    pub format: TemplateRenderFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTemplateProjectResult {
    pub video_path: PathBuf,
    pub report_path: PathBuf,
    pub graphics_dir: PathBuf,
    pub preview_path: PathBuf,
    pub frame_count: u32,
}

#[derive(Debug, Default)]
struct RenderTemplateArgs {
    project_root: Option<PathBuf>,
    template_config_path: Option<PathBuf>,
}

pub fn parse_render_template_args<I, S>(args: I) -> PipelineResult<RenderTemplateProjectConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut tokens = args.into_iter().map(Into::into).collect::<Vec<String>>();
    if tokens.first().is_some_and(|token| !token.starts_with("--")) {
        tokens.remove(0);
    }
    if tokens.first().is_some_and(|token| token == "--") {
        tokens.remove(0);
    }

    let mut parsed = RenderTemplateArgs::default();
    let mut index = 0;
    while index < tokens.len() {
        let flag = &tokens[index];
        if !flag.starts_with("--") {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unexpected positional argument.",
                "Use --project-root and optionally --config.",
            )]);
        }
        if !is_render_template_flag(flag) {
            return Err(vec![arg_error(
                arg_path(flag),
                "Unknown render template argument.",
                "Use --project-root and optionally --config.",
            )]);
        }

        let value = tokens
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| {
                vec![arg_error(
                    arg_path(flag),
                    "Missing value for render template argument.",
                    format!("Pass a value after {flag}."),
                )]
            })?
            .clone();

        match flag.as_str() {
            "--project-root" => parsed.project_root = Some(value.into()),
            "--config" => parsed.template_config_path = Some(value.into()),
            _ => unreachable!("render template flag was checked before value parsing"),
        }

        index += 2;
    }

    let project_root = required_path(parsed.project_root, "args.projectRoot", "--project-root")?;
    let template_config_path = parsed
        .template_config_path
        .unwrap_or_else(|| project_root.join(TEMPLATE_RENDER_FILE_NAME));

    Ok(RenderTemplateProjectConfig {
        project_root,
        template_config_path,
    })
}

pub fn run_render_template_project(
    config: &RenderTemplateProjectConfig,
) -> PipelineResult<RenderTemplateProjectResult> {
    let _project = load_project(&config.project_root).map_err(project_storage_error)?;
    let template_config = read_template_config(config)?;
    validate_template_config(&template_config)?;

    let layer = template_render_layer_from_config(&template_config);
    let dimensions = Dimensions {
        width: template_config.render_settings.width,
        height: template_config.render_settings.height,
    };
    let graphics_layer = template_layer_to_graphics_ir(
        &layer,
        dimensions.clone(),
        template_config.render_settings.fps,
    )
    .map_err(PipelineError::from_graphics_errors)?;

    let graphics_dir = config
        .project_root
        .join(GENERATED_GRAPHICS_DIR_NAME)
        .join(&template_config.output_name);
    let renders_dir = config.renders_dir();
    create_dir(&graphics_dir, "project.generatedGraphics")?;
    create_dir(&renders_dir, "project.renders")?;

    let assets = AssetRegistry::new(config.project_root.clone());
    let manifest = render_graphics_preview(
        &graphics_layer,
        &assets,
        GraphicsRenderOptions {
            output_dir: graphics_dir.clone(),
        },
    )
    .map_err(PipelineError::from_graphics_errors)?;

    let video_path = match template_config.render_settings.format {
        TemplateRenderFormat::Webm => {
            renders_dir.join(format!("{}.webm", template_config.output_name))
        }
    };
    export_frame_sequence_to_webm(&WebmExportOptions {
        frames_pattern: graphics_dir.join(&manifest.frames_pattern),
        output_path: video_path.clone(),
        dimensions,
        fps: template_config.render_settings.fps,
        frame_count: manifest.frame_count,
        encoder: WebmEncoder::Vp8,
        target_bitrate_bps: DEFAULT_TARGET_BITRATE_BPS,
    })
    .map_err(PipelineError::from_graphics_errors)?;

    let preview_path = graphics_dir.join(&manifest.preview_path);
    let manifest_path = graphics_dir.join("manifest.json");
    let report_path = config.render_report_path();
    let report = TemplateRenderReport {
        status: "succeeded".to_string(),
        template_id: template_config.template_id,
        output_name: template_config.output_name,
        video_path: project_relative_report_path(&config.project_root, &video_path)?,
        graphics_dir: project_relative_report_path(&config.project_root, &graphics_dir)?,
        preview_path: project_relative_report_path(&config.project_root, &preview_path)?,
        manifest_path: project_relative_report_path(&config.project_root, &manifest_path)?,
        frame_count: manifest.frame_count,
        width: template_config.render_settings.width,
        height: template_config.render_settings.height,
        fps: template_config.render_settings.fps,
        duration_seconds: template_config.render_settings.duration_seconds,
        format: template_config.render_settings.format,
    };
    write_template_report(&report_path, &report)?;

    Ok(RenderTemplateProjectResult {
        video_path,
        report_path,
        graphics_dir,
        preview_path,
        frame_count: manifest.frame_count,
    })
}

pub fn project_relative_report_path(
    project_root: &Path,
    artifact_path: &Path,
) -> PipelineResult<String> {
    let relative_path = artifact_path.strip_prefix(project_root).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineReportWriteFailed,
            "renderReport.artifactPath",
            "Template render report artifact path is outside the project folder.",
            "Write render artifacts under the project folder before recording the report.",
        )
        .with_detail("projectRoot", project_root.display().to_string())
        .with_detail("artifactPath", artifact_path.display().to_string())
        .with_detail("pathError", error.to_string())]
    })?;

    let parts = relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(parts.join("/"))
}

fn read_template_config(
    config: &RenderTemplateProjectConfig,
) -> PipelineResult<TemplateRenderConfig> {
    let input = std::fs::read_to_string(&config.template_config_path).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "args.config",
            "Read template render config failed.",
            "Pass a readable render-template.json inside the project folder.",
        )
        .with_detail("path", config.template_config_path.display().to_string())
        .with_detail("ioError", error.to_string())]
    })?;

    serde_json::from_str(&input).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate",
            "Template render config is not valid JSON for the project renderer.",
            "Use render-template.json with templateId, outputName, fields, and renderSettings.",
        )
        .with_detail("path", config.template_config_path.display().to_string())
        .with_detail("serdeError", error.to_string())]
    })
}

fn validate_template_config(config: &TemplateRenderConfig) -> PipelineResult<()> {
    let mut errors = Vec::new();

    if config.schema_version != DEFAULT_SCHEMA_VERSION {
        errors.push(PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate.schemaVersion",
            "Template render config schema version is unsupported.",
            "Set schemaVersion to 1.",
        ));
    }
    if config.template_id.trim().is_empty() {
        errors.push(PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate.templateId",
            "Template render config is missing templateId.",
            "Set templateId to a supported motion template id.",
        ));
    }
    if !is_safe_output_name(&config.output_name) {
        errors.push(PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate.outputName",
            "Template render outputName is not a safe file stem.",
            "Use only letters, numbers, hyphens, and underscores, without path separators.",
        ));
    }
    if config.render_settings.width == 0 || config.render_settings.height == 0 {
        errors.push(PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate.renderSettings",
            "Template render dimensions must be positive.",
            "Set width and height to positive pixel values.",
        ));
    }
    if !config.render_settings.fps.is_finite() || config.render_settings.fps <= 0.0 {
        errors.push(PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate.renderSettings.fps",
            "Template render fps must be finite and positive.",
            "Set fps to a positive value such as 24 or 30.",
        ));
    }
    if !config.render_settings.duration_seconds.is_finite()
        || config.render_settings.duration_seconds <= 0.0
    {
        errors.push(PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "renderTemplate.renderSettings.durationSeconds",
            "Template render durationSeconds must be finite and positive.",
            "Set durationSeconds to a positive value.",
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn template_render_layer_from_config(config: &TemplateRenderConfig) -> TemplateRenderLayer {
    TemplateRenderLayer {
        item_id: config.output_name.clone(),
        template_id: config.template_id.clone(),
        label: config.output_name.clone(),
        timeline_start_seconds: 0.0,
        duration_seconds: config.render_settings.duration_seconds,
        fields: config.fields.clone(),
        visual_treatment: config.visual_treatment.clone().unwrap_or_else(|| {
            "project-scoped full-frame template render with purposeful visual treatment".to_string()
        }),
        motion: config.motion.clone().unwrap_or_else(|| {
            "template-defined motion over the configured render duration".to_string()
        }),
        safe_zone: config
            .safe_zone
            .clone()
            .unwrap_or_else(|| "keep essential text inside 10% margins".to_string()),
        avoid: config.avoid.clone().unwrap_or_else(|| {
            "plain boxes, opaque caption slabs, and static default-font title cards".to_string()
        }),
        preview_variant: config.template_id.clone(),
    }
}

fn write_template_report(path: &Path, report: &TemplateRenderReport) -> PipelineResult<()> {
    if let Some(parent) = path.parent() {
        create_dir(parent, "project.renders")?;
    }
    let json = serde_json::to_string_pretty(report).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineReportWriteFailed,
            "renderReport",
            "Template render report could not be serialized.",
            "Inspect the render report fields and retry.",
        )
        .with_detail("serdeError", error.to_string())]
    })?;
    std::fs::write(path, json).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineReportWriteFailed,
            "renderReport",
            "Template render report could not be written.",
            "Choose a writable project renders directory.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("ioError", error.to_string())]
    })
}

fn create_dir(path: &Path, error_path: &str) -> PipelineResult<()> {
    std::fs::create_dir_all(path).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            error_path,
            "Create project render directory failed.",
            "Choose a writable project folder and retry the template render.",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("ioError", error.to_string())]
    })
}

fn project_storage_error(error: ProjectStorageError) -> Vec<PipelineError> {
    match error {
        ProjectStorageError::Read { path, source } => vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project.file",
            "Project folder is missing a readable video-creater.project.json file.",
            "Create or open a project folder before rendering the template.",
        )
        .with_detail("path", path)
        .with_detail("ioError", source.to_string())],
        ProjectStorageError::Parse { path, source } => vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project.file",
            "Project file is not valid JSON for this renderer.",
            "Repair video-creater.project.json before rendering the template.",
        )
        .with_detail("path", path)
        .with_detail("serdeError", source.to_string())],
        ProjectStorageError::CreateDir { path, source }
        | ProjectStorageError::Write { path, source }
        | ProjectStorageError::Sync { path, source } => vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project.file",
            "Project storage operation failed.",
            "Choose a writable project folder and retry.",
        )
        .with_detail("path", path)
        .with_detail("ioError", source.to_string())],
        ProjectStorageError::Rename { from, to, source } => vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project.file",
            "Project storage rename failed.",
            "Choose a writable project folder and retry.",
        )
        .with_detail("from", from)
        .with_detail("to", to)
        .with_detail("ioError", source.to_string())],
        ProjectStorageError::Serialize(source) => vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "project.file",
            "Project file could not be serialized.",
            "Inspect the project model and retry.",
        )
        .with_detail("serdeError", source.to_string())],
    }
}

fn required_path(value: Option<PathBuf>, path: &str, flag: &str) -> PipelineResult<PathBuf> {
    value.ok_or_else(|| {
        vec![arg_error(
            path,
            "Missing required render template argument.",
            format!("Pass {flag} before rendering a project template."),
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

fn is_render_template_flag(flag: &str) -> bool {
    matches!(flag, "--project-root" | "--config")
}

fn arg_path(flag: &str) -> String {
    match flag {
        "--project-root" => "args.projectRoot".to_string(),
        "--config" => "args.config".to_string(),
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

fn is_safe_output_name(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed == value
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn default_schema_version() -> u32 {
    DEFAULT_SCHEMA_VERSION
}
