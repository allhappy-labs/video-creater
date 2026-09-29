use std::path::{Path, PathBuf};

use super::project_catalog::ProjectCatalog;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    pub fn length(self) -> u64 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeError {
    Invalid,
    Multiple,
    Unsatisfiable,
}

pub fn parse_single_range(
    value: Option<&str>,
    total: u64,
) -> Result<Option<ByteRange>, RangeError> {
    let Some(value) = value else { return Ok(None) };
    if total == 0 {
        return Err(RangeError::Unsatisfiable);
    }
    let value = value.strip_prefix("bytes=").ok_or(RangeError::Invalid)?;
    if value.contains(',') {
        return Err(RangeError::Multiple);
    }
    let (start, end) = value.split_once('-').ok_or(RangeError::Invalid)?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| RangeError::Invalid)?;
        if suffix == 0 {
            return Err(RangeError::Unsatisfiable);
        }
        let length = suffix.min(total);
        return Ok(Some(ByteRange {
            start: total - length,
            end: total - 1,
        }));
    }
    let start = start.parse::<u64>().map_err(|_| RangeError::Invalid)?;
    if start >= total {
        return Err(RangeError::Unsatisfiable);
    }
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>()
            .map_err(|_| RangeError::Invalid)?
            .min(total - 1)
    };
    if end < start {
        return Err(RangeError::Unsatisfiable);
    }
    Ok(Some(ByteRange { start, end }))
}

pub fn resolve_project_resource(
    catalog: &ProjectCatalog,
    project_id: &str,
    relative_path: &str,
) -> Result<PathBuf, String> {
    validate_relative_path(relative_path)?;
    let project = catalog.resolve(project_id)?;
    let path = std::fs::canonicalize(project.join(relative_path))
        .map_err(|_| "resource is not available".to_string())?;
    if !path.is_file() || !beneath(&path, &project) {
        return Err("resource is not available".into());
    }
    Ok(path)
}

pub fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "json" => "application/json",
        "xml" | "fcpxml" => "application/xml",
        "zip" | "palmier" => "application/zip",
        _ => "application/octet-stream",
    }
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("resource path is invalid".into());
    }
    for component in path.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("resource path is invalid".into());
        }
    }
    Ok(())
}

fn beneath(path: &Path, project: &Path) -> bool {
    path != project && path.starts_with(project)
}
