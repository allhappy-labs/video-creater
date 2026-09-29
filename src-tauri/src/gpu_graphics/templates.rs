use super::shader::validate_fragment_source;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy)]
struct BuiltinShaderTemplateFile {
    config_path: &'static str,
    shader_path: &'static str,
    config: &'static str,
    fragment_body: &'static str,
}

macro_rules! builtin_template {
    ($id:literal) => {
        BuiltinShaderTemplateFile {
            config_path: concat!(
                "src-tauri/assets/shader-background-templates/builtin/",
                $id,
                "/template.json"
            ),
            shader_path: concat!(
                "src-tauri/assets/shader-background-templates/builtin/",
                $id,
                "/shader.frag"
            ),
            config: include_str!(concat!(
                "../../assets/shader-background-templates/builtin/",
                $id,
                "/template.json"
            )),
            fragment_body: include_str!(concat!(
                "../../assets/shader-background-templates/builtin/",
                $id,
                "/shader.frag"
            )),
        }
    };
}

const BUILTIN_SHADER_TEMPLATE_FILES: &[BuiltinShaderTemplateFile] = &[
    builtin_template!("shadertoy-octagrams-v1"),
    builtin_template!("shadertoy-base-warp-fbm-v1"),
    builtin_template!("shadertoy-phantom-star-v1"),
    builtin_template!("shadertoy-volumetric-clouds-v1"),
    builtin_template!("shadertoy-cube-lines-v1"),
    builtin_template!("shadertoy-crumpled-wave-v1"),
    builtin_template!("shadertoy-glowing-marbling-black-v1"),
    builtin_template!("shadertoy-geodesic-tiling-v1"),
    builtin_template!("shadertoy-kaleidoscope-tunnel-v1"),
    builtin_template!("shadertoy-digital-brain-v1"),
    builtin_template!("shadertoy-starry-planes-v1"),
    builtin_template!("shadertoy-mandelbulb-interior-v1"),
    builtin_template!("shadertoy-tiny-clouds-v1"),
    builtin_template!("shadertoy-neon-lit-hexagons-v1"),
    builtin_template!("shadertoy-another-cube-v1"),
    builtin_template!("shadertoy-object-mesh-v1"),
    builtin_template!("shadertoy-looping-clouds-v1"),
];

pub fn shared_shader_utils() -> &'static str {
    include_str!("../../assets/shader-background-templates/builtin/shared/utils.glsl")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShaderTemplateSourceKind {
    BuiltIn,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderBackgroundTemplateConfig {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub source_kind: ShaderTemplateSourceKind,
    pub category: String,
    pub shader_profile_id: String,
    pub default_duration_seconds: f64,
    pub source_url: Option<String>,
    pub license: String,
    pub shader_ref: String,
    #[serde(default)]
    pub utility_refs: Vec<String>,
    pub placement: ShaderTemplatePlacement,
    pub render_contract: ShaderTemplateRenderContract,
    pub preview: ShaderTemplatePreview,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
    pub agent_summary: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderTemplatePlacement {
    pub track_kind: String,
    pub default_start_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderTemplateRenderContract {
    pub dimensions: String,
    pub fps: String,
    pub alpha: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderTemplatePreview {
    pub thumbnail_kind: String,
    pub accent_color: String,
    pub secondary_color: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderBackgroundTemplate {
    pub config: ShaderBackgroundTemplateConfig,
    pub config_path: String,
    pub shader_path: String,
    pub fragment_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderTemplateCatalogError {
    InvalidConfig { path: String, message: String },
    InvalidShader { path: String, message: String },
    Io { path: PathBuf, message: String },
}

impl fmt::Display for ShaderTemplateCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderTemplateCatalogError::InvalidConfig { path, message } => {
                write!(
                    formatter,
                    "invalid shader template config {path}: {message}"
                )
            }
            ShaderTemplateCatalogError::InvalidShader { path, message } => {
                write!(
                    formatter,
                    "invalid shader template shader {path}: {message}"
                )
            }
            ShaderTemplateCatalogError::Io { path, message } => {
                write!(
                    formatter,
                    "failed to read shader template {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ShaderTemplateCatalogError {}

pub fn builtin_shader_background_templates(
) -> Result<Vec<ShaderBackgroundTemplate>, ShaderTemplateCatalogError> {
    BUILTIN_SHADER_TEMPLATE_FILES
        .iter()
        .map(load_builtin_template)
        .collect()
}

pub fn builtin_shader_background_template(
    template_id: &str,
) -> Result<Option<ShaderBackgroundTemplate>, ShaderTemplateCatalogError> {
    for template in builtin_shader_background_templates()? {
        if template.config.id == template_id {
            return Ok(Some(template));
        }
    }
    Ok(None)
}

pub fn agent_shader_background_catalog() -> Result<String, ShaderTemplateCatalogError> {
    let templates = builtin_shader_background_templates()?;
    Ok(format_agent_shader_background_catalog(&templates))
}

pub fn project_shader_background_templates(
    project_root: impl AsRef<Path>,
) -> Result<Vec<ShaderBackgroundTemplate>, ShaderTemplateCatalogError> {
    let project_root = project_root.as_ref();
    let mut templates = builtin_shader_background_templates()?;
    let user_root = project_root.join("shader-background-templates");

    if user_root.is_dir() {
        templates.extend(load_user_shader_background_templates(&user_root)?);
    }

    validate_unique_template_ids(&templates)?;
    Ok(templates)
}

pub fn project_agent_shader_background_catalog(
    project_root: impl AsRef<Path>,
) -> Result<String, ShaderTemplateCatalogError> {
    let templates = project_shader_background_templates(project_root)?;
    Ok(format_agent_shader_background_catalog(&templates))
}

pub fn load_user_shader_background_templates(
    root: impl AsRef<Path>,
) -> Result<Vec<ShaderBackgroundTemplate>, ShaderTemplateCatalogError> {
    let root = root.as_ref();
    let mut directories = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|error| ShaderTemplateCatalogError::Io {
        path: root.to_path_buf(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| ShaderTemplateCatalogError::Io {
            path: root.to_path_buf(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            directories.push(path);
        }
    }
    directories.sort();

    directories
        .into_iter()
        .map(load_user_template_dir)
        .collect::<Result<Vec<_>, _>>()
}

fn load_builtin_template(
    file: &BuiltinShaderTemplateFile,
) -> Result<ShaderBackgroundTemplate, ShaderTemplateCatalogError> {
    let config = parse_config(file.config_path, file.config)?;
    validate_config(&config, file.config_path, ShaderTemplateSourceKind::BuiltIn)?;
    validate_shader(file.shader_path, file.fragment_body)?;
    Ok(ShaderBackgroundTemplate {
        config,
        config_path: file.config_path.to_string(),
        shader_path: file.shader_path.to_string(),
        fragment_body: file.fragment_body.to_string(),
    })
}

fn load_user_template_dir(
    template_dir: PathBuf,
) -> Result<ShaderBackgroundTemplate, ShaderTemplateCatalogError> {
    let config_path = template_dir.join("template.json");
    let config_string =
        std::fs::read_to_string(&config_path).map_err(|error| ShaderTemplateCatalogError::Io {
            path: config_path.clone(),
            message: error.to_string(),
        })?;
    let config_path_string = config_path.display().to_string();
    let config = parse_config(&config_path_string, &config_string)?;
    validate_config(&config, &config_path_string, ShaderTemplateSourceKind::User)?;
    validate_relative_shader_ref(&config, &config_path_string)?;

    let shader_path = template_dir.join(&config.shader_ref);
    let fragment_body =
        std::fs::read_to_string(&shader_path).map_err(|error| ShaderTemplateCatalogError::Io {
            path: shader_path.clone(),
            message: error.to_string(),
        })?;
    let shader_path_string = shader_path.display().to_string();
    validate_shader(&shader_path_string, &fragment_body)?;

    Ok(ShaderBackgroundTemplate {
        config,
        config_path: config_path_string,
        shader_path: shader_path_string,
        fragment_body,
    })
}

fn format_agent_shader_background_catalog(templates: &[ShaderBackgroundTemplate]) -> String {
    templates
        .iter()
        .map(|template| {
            let source_kind = match template.config.source_kind {
                ShaderTemplateSourceKind::BuiltIn => "built_in",
                ShaderTemplateSourceKind::User => "user",
            };
            format!(
                "- {} sourceKind {} config {}",
                template.config.agent_summary, source_kind, template.config_path
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_unique_template_ids(
    templates: &[ShaderBackgroundTemplate],
) -> Result<(), ShaderTemplateCatalogError> {
    let mut seen = BTreeSet::new();
    for template in templates {
        if !seen.insert(template.config.id.clone()) {
            return Err(ShaderTemplateCatalogError::InvalidConfig {
                path: template.config_path.clone(),
                message: format!(
                    "duplicate shader background template id {}",
                    template.config.id
                ),
            });
        }
    }

    Ok(())
}

fn validate_relative_shader_ref(
    config: &ShaderBackgroundTemplateConfig,
    path: &str,
) -> Result<(), ShaderTemplateCatalogError> {
    let shader_ref = Path::new(&config.shader_ref);
    if shader_ref.is_absolute()
        || shader_ref
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(ShaderTemplateCatalogError::InvalidConfig {
            path: path.to_string(),
            message: "shaderRef must stay inside its template folder".to_string(),
        });
    }

    Ok(())
}

fn parse_config(
    path: &str,
    config: &str,
) -> Result<ShaderBackgroundTemplateConfig, ShaderTemplateCatalogError> {
    serde_json::from_str(config).map_err(|error| ShaderTemplateCatalogError::InvalidConfig {
        path: path.to_string(),
        message: error.to_string(),
    })
}

fn validate_config(
    config: &ShaderBackgroundTemplateConfig,
    path: &str,
    expected_source_kind: ShaderTemplateSourceKind,
) -> Result<(), ShaderTemplateCatalogError> {
    let mut errors = Vec::new();

    if config.id.trim().is_empty() {
        errors.push("id must be non-empty".to_string());
    }
    if config.version == 0 {
        errors.push("version must be positive".to_string());
    }
    if config.source_kind != expected_source_kind {
        errors.push(format!(
            "sourceKind must be {expected_source_kind:?} for this catalog"
        ));
    }
    if config.shader_profile_id.trim().is_empty() {
        errors.push("shaderProfileId must be non-empty".to_string());
    }
    if !config.default_duration_seconds.is_finite() || config.default_duration_seconds <= 0.0 {
        errors.push("defaultDurationSeconds must be finite and positive".to_string());
    }
    if config.shader_ref.trim().is_empty() {
        errors.push("shaderRef must be non-empty".to_string());
    }
    if config.placement.track_kind != "hyperframe_scene" {
        errors.push("placement.trackKind must be hyperframe_scene".to_string());
    }
    if config.render_contract.dimensions != "project" || config.render_contract.fps != "project" {
        errors.push("renderContract dimensions and fps must be project".to_string());
    }
    for (field, value) in [
        ("title", &config.title),
        ("category", &config.category),
        ("license", &config.license),
        ("visualTreatment", &config.visual_treatment),
        ("motion", &config.motion),
        ("safeZone", &config.safe_zone),
        ("avoid", &config.avoid),
        ("agentSummary", &config.agent_summary),
    ] {
        if value.trim().is_empty() {
            errors.push(format!("{field} must be non-empty"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ShaderTemplateCatalogError::InvalidConfig {
            path: path.to_string(),
            message: errors.join("; "),
        })
    }
}

fn validate_shader(path: &str, fragment_body: &str) -> Result<(), ShaderTemplateCatalogError> {
    validate_fragment_source(fragment_body).map_err(|errors| {
        ShaderTemplateCatalogError::InvalidShader {
            path: path.to_string(),
            message: errors
                .into_iter()
                .map(|error| format!("{}: {}", error.path, error.message))
                .collect::<Vec<_>>()
                .join("; "),
        }
    })
}
