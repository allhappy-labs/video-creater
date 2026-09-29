use std::path::Path;

use crate::project::model::{ProjectExportArtifact, VideoProject};

pub fn export_artifact<'a>(
    project: &'a VideoProject,
    artifact_id: &str,
) -> Option<&'a ProjectExportArtifact> {
    project
        .export_artifacts
        .iter()
        .find(|artifact| artifact.id == artifact_id)
}

pub fn safe_download_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download")
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '"' | '\\' => '_',
            other => other,
        })
        .take(200)
        .collect()
}

pub fn attachment_header(display_name: &str) -> String {
    format!(
        "attachment; filename=\"{}\"",
        safe_download_name(display_name)
    )
}
