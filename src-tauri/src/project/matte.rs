use super::model::{MediaAsset, MediaKind, VideoProject};
use super::mutation::acquire_split_project_mutation_lease;
use super::split::{load_split_project, save_split_project};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

const SUPPORTED_ASPECTS: &str = "Project, 16:9, 9:16, 1:1, 4:3, 9:14, 2.4:1";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatteRequest {
    pub hex: String,
    pub aspect_ratio: String,
    pub name: Option<String>,
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatteResult {
    pub project: VideoProject,
    pub media: MediaAsset,
}

#[derive(Debug, Error)]
pub enum MatteError {
    #[error("{0}")]
    InvalidArgument(String),
    #[error("{0}")]
    Persistence(String),
}

pub fn create_matte(project_dir: &Path, request: MatteRequest) -> Result<MatteResult, MatteError> {
    let _project_lease =
        acquire_split_project_mutation_lease(project_dir).map_err(MatteError::Persistence)?;
    let project = load_split_project(project_dir)
        .map_err(|error| MatteError::Persistence(error.to_string()))?;
    create_matte_for_project(project_dir, &project, request)
}

pub fn create_matte_for_project(
    project_dir: &Path,
    project: &VideoProject,
    request: MatteRequest,
) -> Result<MatteResult, MatteError> {
    let _project_lease =
        acquire_split_project_mutation_lease(project_dir).map_err(MatteError::Persistence)?;
    let canonical_project;
    let project = if super::storage::project_file_path(project_dir).exists() {
        canonical_project = load_split_project(project_dir)
            .map_err(|error| MatteError::Persistence(error.to_string()))?;
        &canonical_project
    } else {
        project
    };
    let (red, green, blue) = parse_matte_hex(&request.hex)?;
    let aspect = request.aspect_ratio.trim();
    if aspect.is_empty() {
        return Err(unknown_aspect_error(&request.aspect_ratio));
    }
    let (width, height) = matte_pixel_size(
        aspect,
        project.render_settings.width,
        project.render_settings.height,
    )?;
    let folder_id = validate_folder_id(project, request.folder_id.as_deref())?;
    let name = request
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_matte_name(aspect, width, height));
    let file_stem = sanitize_file_stem(&name).unwrap_or_else(|| "matte".to_string());
    let media_id = format!("media-{}", uuid::Uuid::new_v4());
    let relative_path = format!("media/{media_id}-{file_stem}.png");
    let output_path = project_dir.join(&relative_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            MatteError::Persistence(format!("failed to create matte media directory: {error}"))
        })?;
    }

    image::RgbaImage::from_pixel(width, height, image::Rgba([red, green, blue, 255]))
        .save(&output_path)
        .map_err(|error| MatteError::Persistence(format!("failed to write matte PNG: {error}")))?;

    let media = MediaAsset {
        id: media_id,
        name: Some(name),
        relative_path,
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(width),
        height: Some(height),
        fps: None,
        folder_id,
    };
    let mut next_project = project.clone();
    next_project.media.push(media.clone());
    next_project = save_split_project(project_dir, &next_project)
        .map_err(|error| MatteError::Persistence(error.to_string()))?
        .project;

    Ok(MatteResult {
        project: next_project,
        media,
    })
}

pub fn matte_pixel_size(
    aspect: &str,
    timeline_width: u32,
    timeline_height: u32,
) -> Result<(u32, u32), MatteError> {
    let timeline_width = timeline_width.max(2);
    let timeline_height = timeline_height.max(2);
    match aspect {
        "Project" | "project" => Ok(even_matte_size(timeline_width, timeline_height)),
        "16:9" => Ok(fit_matte_short_edge(timeline_width, timeline_height, 16, 9)),
        "9:16" => Ok(fit_matte_short_edge(timeline_width, timeline_height, 9, 16)),
        "1:1" => Ok(fit_matte_short_edge(timeline_width, timeline_height, 1, 1)),
        "4:3" => Ok(fit_matte_short_edge(timeline_width, timeline_height, 4, 3)),
        "9:14" => Ok(fit_matte_short_edge(timeline_width, timeline_height, 9, 14)),
        "2.4:1" => Ok(fit_matte_short_edge(
            timeline_width,
            timeline_height,
            24,
            10,
        )),
        other => Err(unknown_aspect_error(other)),
    }
}

fn parse_matte_hex(hex: &str) -> Result<(u8, u8, u8), MatteError> {
    let trimmed = hex.trim();
    if trimmed.is_empty() {
        return Err(invalid_hex_error());
    }
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if !matches!(hex.len(), 3 | 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_hex_error());
    }
    match hex.len() {
        3 => Ok((
            parse_hex_component(hex, 0, 1)?,
            parse_hex_component(hex, 1, 1)?,
            parse_hex_component(hex, 2, 1)?,
        )),
        6 | 8 => Ok((
            parse_hex_component(hex, 0, 2)?,
            parse_hex_component(hex, 2, 2)?,
            parse_hex_component(hex, 4, 2)?,
        )),
        _ => Err(invalid_hex_error()),
    }
}

fn parse_hex_component(hex: &str, start: usize, len: usize) -> Result<u8, MatteError> {
    let component = &hex[start..start + len];
    let expanded;
    let component = if len == 1 {
        expanded = format!("{component}{component}");
        expanded.as_str()
    } else {
        component
    };
    u8::from_str_radix(component, 16).map_err(|_| invalid_hex_error())
}

fn invalid_hex_error() -> MatteError {
    MatteError::InvalidArgument(
        "create_matte.hex must be a hex color like #RGB, #RRGGBB, or #RRGGBBAA".to_string(),
    )
}

fn unknown_aspect_error(aspect: &str) -> MatteError {
    MatteError::InvalidArgument(format!(
        "create_matte: unknown aspectRatio '{aspect}'. Use one of {SUPPORTED_ASPECTS}."
    ))
}

fn validate_folder_id(
    project: &VideoProject,
    folder_id: Option<&str>,
) -> Result<Option<String>, MatteError> {
    let Some(folder_id) = folder_id else {
        return Ok(None);
    };
    let folder_id = folder_id.trim();
    if folder_id.is_empty() {
        return Err(MatteError::InvalidArgument(
            "folderId must not be blank".to_string(),
        ));
    }
    if !project
        .media_folders
        .iter()
        .any(|folder| folder.id == folder_id)
    {
        return Err(MatteError::InvalidArgument(format!(
            "folderId references missing media folder: {folder_id}"
        )));
    }
    Ok(Some(folder_id.to_string()))
}

fn fit_matte_short_edge(
    timeline_width: u32,
    timeline_height: u32,
    aspect_width: u32,
    aspect_height: u32,
) -> (u32, u32) {
    let short_edge = timeline_width.min(timeline_height).max(2);
    if aspect_width >= aspect_height {
        even_matte_size(
            ((f64::from(short_edge) * f64::from(aspect_width) / f64::from(aspect_height))
                .round()
                .max(2.0)) as u32,
            short_edge,
        )
    } else {
        even_matte_size(
            short_edge,
            ((f64::from(short_edge) * f64::from(aspect_height) / f64::from(aspect_width))
                .round()
                .max(2.0)) as u32,
        )
    }
}

fn even_matte_size(width: u32, height: u32) -> (u32, u32) {
    ((width.max(2) / 2) * 2, (height.max(2) / 2) * 2)
}

fn default_matte_name(aspect: &str, width: u32, height: u32) -> String {
    if aspect.eq_ignore_ascii_case("project") {
        format!("Matte - {width}x{height}")
    } else {
        format!("Matte - {aspect}")
    }
}

fn sanitize_file_stem(stem: &str) -> Option<String> {
    let sanitized = stem
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    (!sanitized.is_empty()).then_some(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::model::MediaFolder;
    use crate::project::split::save_split_project;

    #[test]
    fn matte_dimensions_match_palmier_aspects_and_stay_even() {
        assert_eq!(matte_pixel_size("Project", 736, 400).unwrap(), (736, 400));
        assert_eq!(matte_pixel_size("16:9", 736, 400).unwrap(), (710, 400));
        assert_eq!(matte_pixel_size("9:16", 736, 400).unwrap(), (400, 710));
        assert_eq!(matte_pixel_size("1:1", 736, 400).unwrap(), (400, 400));
        assert!(matte_pixel_size("3:2", 736, 400).is_err());
    }

    #[test]
    fn create_matte_writes_png_and_persists_media() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut project = sample_project();
        project.render_settings.width = 736;
        project.render_settings.height = 400;
        project.media_folders.push(MediaFolder {
            id: "folder-1".to_string(),
            name: "Mattes".to_string(),
            parent_id: None,
        });
        save_split_project(temp_dir.path(), &project).unwrap();

        let result = create_matte(
            temp_dir.path(),
            MatteRequest {
                hex: "#112233".into(),
                aspect_ratio: "Project".into(),
                name: None,
                folder_id: Some("folder-1".into()),
            },
        )
        .unwrap();

        assert!(temp_dir.path().join(&result.media.relative_path).is_file());
        assert_eq!(result.media.kind, MediaKind::Image);
        assert_eq!(result.media.folder_id.as_deref(), Some("folder-1"));
        let persisted = load_split_project(temp_dir.path()).unwrap();
        assert_eq!(result.project, persisted);
        assert!(persisted
            .media
            .iter()
            .any(|media| media.id == result.media.id));
        crate::project::split::replace_split_project_if_revision(
            temp_dir.path(),
            result.project.clone(),
            result.project.content_revision,
        )
        .expect("subsequent CAS save");
    }

    #[test]
    fn create_matte_for_project_reloads_canonical_project_before_mutating() {
        let temp_dir = tempfile::tempdir().unwrap();
        let stale_project = sample_project();
        save_split_project(temp_dir.path(), &stale_project).unwrap();
        let mut canonical_project = stale_project.clone();
        canonical_project.name = "Settings-promoted name".to_string();
        save_split_project(temp_dir.path(), &canonical_project).unwrap();

        create_matte_for_project(
            temp_dir.path(),
            &stale_project,
            MatteRequest {
                hex: "#112233".into(),
                aspect_ratio: "Project".into(),
                name: None,
                folder_id: None,
            },
        )
        .unwrap();

        let persisted = load_split_project(temp_dir.path()).unwrap();
        assert_eq!(persisted.name, "Settings-promoted name");
        assert_eq!(persisted.media.len(), canonical_project.media.len() + 1);
    }
}
