use super::model::VideoProject;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use thiserror::Error;

pub const PROJECT_FILE_NAME: &str = "video-creater.project.json";

#[derive(Debug, Error)]
pub enum ProjectStorageError {
    #[error("failed to create project directory {path}: {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to serialize project: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write project file {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to sync project temp file {path}: {source}")]
    Sync {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to rename project temp file {from} to {to}: {source}")]
    Rename {
        from: String,
        to: String,
        source: std::io::Error,
    },
    #[error("failed to read project file {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse project file {path}: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
}

pub fn project_file_path(project_dir: &Path) -> PathBuf {
    project_dir.join(PROJECT_FILE_NAME)
}

pub fn save_project(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<PathBuf, ProjectStorageError> {
    create_project_dir(project_dir)?;

    let path = project_file_path(project_dir);
    let json = serde_json::to_string_pretty(project).map_err(ProjectStorageError::Serialize)?;
    let mut temp_file =
        NamedTempFile::with_prefix_in(format!(".{PROJECT_FILE_NAME}."), project_dir).map_err(
            |source| ProjectStorageError::Write {
                path: project_dir.display().to_string(),
                source,
            },
        )?;
    let temp_path = temp_file.path().to_path_buf();

    temp_file
        .write_all(json.as_bytes())
        .map_err(|source| ProjectStorageError::Write {
            path: temp_path.display().to_string(),
            source,
        })?;

    temp_file
        .as_file()
        .sync_all()
        .map_err(|source| ProjectStorageError::Sync {
            path: temp_path.display().to_string(),
            source,
        })?;

    temp_file
        .persist(&path)
        .map_err(|error| ProjectStorageError::Rename {
            from: temp_path.display().to_string(),
            to: path.display().to_string(),
            source: error.error,
        })?;

    Ok(path)
}

pub fn load_project(project_dir: &Path) -> Result<VideoProject, ProjectStorageError> {
    let path = project_file_path(project_dir);
    let json = fs::read_to_string(&path).map_err(|source| ProjectStorageError::Read {
        path: path.display().to_string(),
        source,
    })?;

    serde_json::from_str(&json).map_err(|source| ProjectStorageError::Parse {
        path: path.display().to_string(),
        source,
    })
}

fn create_project_dir(project_dir: &Path) -> Result<(), ProjectStorageError> {
    create_dir(project_dir)?;

    for child in [
        "media",
        "transcripts",
        "generated/hyperframes",
        "generated/previews",
        "renders",
        "logs",
    ] {
        create_dir(&project_dir.join(child))?;
    }

    Ok(())
}

fn create_dir(path: &Path) -> Result<(), ProjectStorageError> {
    fs::create_dir_all(path).map_err(|source| ProjectStorageError::CreateDir {
        path: path.display().to_string(),
        source,
    })
}
