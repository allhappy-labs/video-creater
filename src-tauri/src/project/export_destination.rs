//! Where a finished video export is saved, and how it gets there.
//!
//! Shared by the in-process renderer and the Temporal `WriteExportArtifact`
//! activity. An existing file is never overwritten: the export takes the next
//! free name (`Name.mp4`, `Name (2).mp4`, ...).

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

use crate::project::export_profiles::ExportProfile;
use crate::project::model::{ProjectExportArtifact, ProjectExportArtifactKind};

const MAX_EXPORT_NAME_BYTES: usize = 180;
const MAX_COLLISION_SUFFIX: u32 = 999;
const PROJECT_EXPORTS_DIR: &str = "exports";

/// The file name and folder the user chose for an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOutputRequest {
    pub file_name: String,
    /// An absolute folder. `None` saves into the project's `exports/` folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

/// The messages match `exportFileNameProblem` in `src/lib/export/export-naming.ts`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExportDestinationError {
    #[error("Give the export a name.")]
    BlankName,
    #[error("Export names can't contain slashes.")]
    NameHasSlash,
    #[error("Export names can't contain control characters.")]
    NameHasControlCharacter,
    #[error("Export names can't start with a dot.")]
    NameStartsWithDot,
    #[error("Export names can't be longer than 180 bytes.")]
    NameTooLong,
    #[error("The export folder must be a full path.")]
    DirectoryNotAbsolute,
    #[error("The export folder no longer exists.")]
    MissingDirectory,
    #[error("Choose a folder outside the project, or the project's exports folder.")]
    DirectoryInsideProject,
    #[error("This folder already has 999 exports with this name. Choose another name.")]
    NoFreeName,
    #[error("The rendered file is empty, so there is nothing to save.")]
    EmptySource,
    #[error("This format doesn't save a video file.")]
    UnsupportedProfile,
    #[error("The export couldn't be saved: {0}")]
    Io(String),
}

impl From<std::io::Error> for ExportDestinationError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// Validates an export file name and returns its stem, without a duplicate
/// `.<extension>` suffix.
pub fn validate_export_file_name(
    name: &str,
    extension: &str,
) -> Result<String, ExportDestinationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ExportDestinationError::BlankName);
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(ExportDestinationError::NameHasSlash);
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ExportDestinationError::NameHasControlCharacter);
    }
    if trimmed.starts_with('.') {
        return Err(ExportDestinationError::NameStartsWithDot);
    }
    if trimmed.len() > MAX_EXPORT_NAME_BYTES {
        return Err(ExportDestinationError::NameTooLong);
    }
    let suffix = format!(".{extension}");
    let stem = if trimmed.len() > suffix.len()
        && trimmed.is_char_boundary(trimmed.len() - suffix.len())
        && trimmed[trimmed.len() - suffix.len()..].eq_ignore_ascii_case(&suffix)
    {
        trimmed[..trimmed.len() - suffix.len()].trim_end()
    } else {
        trimmed
    };
    if stem.is_empty() {
        return Err(ExportDestinationError::BlankName);
    }
    Ok(stem.to_string())
}

/// Lexical checks for a stored or requested output: a valid name and, when a
/// folder is given, a full path. Existence is checked when the export is saved.
pub fn validate_export_output_request(
    output: &ExportOutputRequest,
    extension: &str,
) -> Result<(), ExportDestinationError> {
    validate_export_file_name(&output.file_name, extension)?;
    if let Some(directory) = output.directory.as_deref() {
        validate_absolute_directory(Path::new(directory))?;
    }
    Ok(())
}

fn validate_absolute_directory(directory: &Path) -> Result<(), ExportDestinationError> {
    if !directory.is_absolute()
        || directory
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(ExportDestinationError::DirectoryNotAbsolute);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDestination {
    pub directory: PathBuf,
    pub stem: String,
    pub extension: String,
    pub inside_project_exports: bool,
    /// `exports` or `exports/<sub>` for project folders, else the absolute folder.
    recorded_directory: String,
}

/// Validates the requested folder and name. The default folder is the
/// project's `exports/`; a chosen folder must exist and must be outside the
/// project or inside its `exports/`.
pub fn resolve_export_destination(
    project_dir: &Path,
    output: &ExportOutputRequest,
    extension: &str,
) -> Result<ExportDestination, ExportDestinationError> {
    let stem = validate_export_file_name(&output.file_name, extension)?;
    let Some(directory) = output.directory.as_deref() else {
        return Ok(ExportDestination {
            directory: project_dir.join(PROJECT_EXPORTS_DIR),
            stem,
            extension: extension.to_string(),
            inside_project_exports: true,
            recorded_directory: PROJECT_EXPORTS_DIR.to_string(),
        });
    };
    let directory_path = Path::new(directory);
    validate_absolute_directory(directory_path)?;
    if !directory_path.is_dir() {
        return Err(ExportDestinationError::MissingDirectory);
    }
    let canonical_directory = fs::canonicalize(directory_path)?;
    let canonical_project = fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.into());
    if !canonical_directory.starts_with(&canonical_project) {
        // A folder named inside the project that resolves outside it (a symlink) is recorded by
        // where it resolves, so "Show in folder" finds the file outside the project.
        let named_inside_project = directory_path.starts_with(project_dir)
            || directory_path.starts_with(&canonical_project);
        let directory = if named_inside_project {
            canonical_directory
        } else {
            directory_path.to_path_buf()
        };
        return Ok(ExportDestination {
            recorded_directory: directory.display().to_string(),
            directory,
            stem,
            extension: extension.to_string(),
            inside_project_exports: false,
        });
    }
    let relative = canonical_directory
        .strip_prefix(&canonical_project)
        .map_err(|_| ExportDestinationError::DirectoryInsideProject)?;
    let mut components = relative.components();
    let under_exports = matches!(
        components.next(),
        Some(Component::Normal(first)) if first == PROJECT_EXPORTS_DIR
    );
    if !under_exports {
        return Err(ExportDestinationError::DirectoryInsideProject);
    }
    let recorded_directory = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    Ok(ExportDestination {
        directory: canonical_directory,
        stem,
        extension: extension.to_string(),
        inside_project_exports: true,
        recorded_directory,
    })
}

impl ExportDestination {
    fn candidate_file_name(&self, index: u32) -> String {
        if index <= 1 {
            format!("{}.{}", self.stem, self.extension)
        } else {
            format!("{} ({index}).{}", self.stem, self.extension)
        }
    }

    fn free_candidates(&self) -> impl Iterator<Item = (String, PathBuf)> + '_ {
        (1..=MAX_COLLISION_SUFFIX)
            .map(|index| {
                let name = self.candidate_file_name(index);
                let path = self.directory.join(&name);
                (name, path)
            })
            .filter(|(_, path)| fs::symlink_metadata(path).is_err())
    }

    /// The first name that doesn't exist yet: `Name.ext`, `Name (2).ext`, ...
    pub fn free_path(&self) -> Result<PathBuf, ExportDestinationError> {
        self.free_candidates()
            .next()
            .map(|(_, path)| path)
            .ok_or(ExportDestinationError::NoFreeName)
    }

    fn recorded_path(&self, file_name: &str) -> String {
        if self.inside_project_exports {
            format!("{}/{file_name}", self.recorded_directory)
        } else {
            self.directory.join(file_name).display().to_string()
        }
    }
}

/// How the rendered file reaches its export name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LinkPolicy {
    /// Hard-link the render when possible, otherwise copy.
    #[default]
    Auto,
    /// Always copy. Tests use it to exercise the fallback.
    CopyOnly,
}

pub struct ExportMaterialization<'a> {
    pub destination: &'a ExportDestination,
    pub source: &'a Path,
    pub job_id: &'a str,
    pub link_policy: LinkPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedExport {
    /// Project-relative for the project's `exports/`, otherwise absolute.
    pub recorded_path: String,
    pub absolute_path: PathBuf,
}

pub fn materialize_export_output(
    request: ExportMaterialization<'_>,
) -> Result<MaterializedExport, ExportDestinationError> {
    materialize_export_output_with_hook(request, &mut |_| {})
}

fn materialize_export_output_with_hook(
    request: ExportMaterialization<'_>,
    before_link: &mut dyn FnMut(&Path),
) -> Result<MaterializedExport, ExportDestinationError> {
    let ExportMaterialization {
        destination,
        source,
        job_id,
        link_policy,
    } = request;
    let source_len = fs::metadata(source)?.len();
    if source_len == 0 {
        return Err(ExportDestinationError::EmptySource);
    }
    if destination.inside_project_exports {
        fs::create_dir_all(&destination.directory)?;
    } else if !destination.directory.is_dir() {
        return Err(ExportDestinationError::MissingDirectory);
    }

    if link_policy == LinkPolicy::Auto {
        match link_to_free_name(destination, source, before_link)? {
            LinkOutcome::Linked(file_name, path) => {
                return Ok(materialized(destination, &file_name, path))
            }
            LinkOutcome::Unsupported => {}
        }
    }

    let staging = StagingFile(destination.directory.join(format!(
        ".{}.{}.partial",
        destination.stem,
        staging_job_segment(job_id)
    )));
    if fs::symlink_metadata(&staging.0).is_ok() {
        fs::remove_file(&staging.0)?;
    }
    let copied = fs::copy(source, &staging.0)?;
    if copied != source_len || fs::metadata(&staging.0)?.len() != source_len {
        return Err(ExportDestinationError::Io(
            "the copied export doesn't match the rendered file".to_string(),
        ));
    }
    match link_to_free_name(destination, &staging.0, before_link)? {
        LinkOutcome::Linked(file_name, path) => Ok(materialized(destination, &file_name, path)),
        LinkOutcome::Unsupported => {
            // This filesystem has no hard links, so there is no atomic
            // no-clobber rename. A file created between the existence check
            // and the rename could be replaced; the window is a few syscalls.
            for (file_name, path) in destination.free_candidates() {
                before_link(&path);
                if fs::symlink_metadata(&path).is_ok() {
                    continue;
                }
                fs::rename(&staging.0, &path)?;
                return Ok(materialized(destination, &file_name, path));
            }
            Err(ExportDestinationError::NoFreeName)
        }
    }
}

enum LinkOutcome {
    Linked(String, PathBuf),
    Unsupported,
}

fn link_to_free_name(
    destination: &ExportDestination,
    source: &Path,
    before_link: &mut dyn FnMut(&Path),
) -> Result<LinkOutcome, ExportDestinationError> {
    for (file_name, path) in destination.free_candidates() {
        before_link(&path);
        match fs::hard_link(source, &path) {
            Ok(()) => return Ok(LinkOutcome::Linked(file_name, path)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return Ok(LinkOutcome::Unsupported),
        }
    }
    Err(ExportDestinationError::NoFreeName)
}

fn materialized(
    destination: &ExportDestination,
    file_name: &str,
    path: PathBuf,
) -> MaterializedExport {
    MaterializedExport {
        recorded_path: destination.recorded_path(file_name),
        absolute_path: path,
    }
}

fn staging_job_segment(job_id: &str) -> String {
    job_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .take(48)
        .collect()
}

/// Removes the staging copy on every exit path; a completed rename leaves
/// nothing to remove.
struct StagingFile(PathBuf);

impl Drop for StagingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Artifact kind, extension and MIME type of a video export profile.
pub fn export_artifact_contract(
    profile: ExportProfile,
) -> Option<(ProjectExportArtifactKind, &'static str, &'static str)> {
    match profile {
        ExportProfile::Webm => Some((ProjectExportArtifactKind::Webm, "webm", "video/webm")),
        ExportProfile::Mp4H264 | ExportProfile::Mp4H265 => {
            Some((ProjectExportArtifactKind::Mp4, "mp4", "video/mp4"))
        }
        ExportProfile::ProResMov => {
            Some((ProjectExportArtifactKind::Mov, "mov", "video/quicktime"))
        }
        ExportProfile::PalmierProject => None,
    }
}

/// The export artifact record, with the same values the Temporal writer records.
pub fn export_artifact_for(
    job_id: &str,
    profile: ExportProfile,
    recorded_path: &str,
    created_at: &str,
) -> Result<ProjectExportArtifact, ExportDestinationError> {
    let (kind, _, mime_type) =
        export_artifact_contract(profile).ok_or(ExportDestinationError::UnsupportedProfile)?;
    let format = serde_json::to_value(profile)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .ok_or(ExportDestinationError::UnsupportedProfile)?;
    Ok(ProjectExportArtifact {
        schema_version: 1,
        id: job_id.to_string(),
        kind,
        format,
        path: recorded_path.to_string(),
        mime_type: mime_type.to_string(),
        job_id: Some(job_id.to_string()),
        created_at: created_at.to_string(),
    })
}

#[cfg(test)]
#[path = "export_destination_tests.rs"]
mod tests;
