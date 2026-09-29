//! Temporal `WriteExportArtifact` with a chosen export name and folder.
//!
//! The legacy `outputPath` writer overwrites into `exports/`; with a
//! `destination` the validated render is saved under the next free name in the
//! chosen folder instead, through the same materializer as in-process exports.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use super::TemporalWorkflowInputError;
use crate::project::export_destination::{
    export_artifact_contract, export_artifact_for, materialize_export_output,
    resolve_export_destination, ExportMaterialization, ExportOutputRequest, LinkPolicy,
};
use crate::project::export_profiles::ExportProfile;
use crate::project::model::ProjectExportArtifact;

/// Reads the optional `destination` object of an export input.
pub(super) fn export_destination_from_input(
    input: &Value,
) -> Result<Option<ExportOutputRequest>, TemporalWorkflowInputError> {
    match input.get("destination") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|_| TemporalWorkflowInputError::InvalidFieldType("destination".to_string())),
    }
}

pub(super) struct DestinationWrite {
    pub(super) artifact: ProjectExportArtifact,
    pub(super) written_path: String,
    pub(super) written_bytes: u64,
}

/// Saves `rendered_output` at the destination without overwriting and builds
/// the artifact record. The caller records the artifact.
pub(super) fn write_export_to_destination(
    project_dir: &Path,
    profile: ExportProfile,
    destination: &ExportOutputRequest,
    rendered_output: &Path,
    job_id: &str,
    created_at: &str,
    files_match: impl Fn(&Path, &Path) -> Result<bool, TemporalWorkflowInputError>,
) -> Result<DestinationWrite, TemporalWorkflowInputError> {
    let destination_error =
        |message: String| TemporalWorkflowInputError::Render(format!("destination: {message}"));
    let (_, extension, _) = export_artifact_contract(profile)
        .ok_or_else(|| TemporalWorkflowInputError::MismatchedInputField("profile".to_string()))?;
    let resolved = resolve_export_destination(project_dir, destination, extension)
        .map_err(|error| destination_error(error.to_string()))?;
    let materialized = materialize_export_output(ExportMaterialization {
        destination: &resolved,
        source: rendered_output,
        job_id,
        link_policy: LinkPolicy::Auto,
    })
    .map_err(|error| destination_error(error.to_string()))?;
    let remove_and_fail = |error: TemporalWorkflowInputError| {
        let _ = fs::remove_file(&materialized.absolute_path);
        error
    };
    let rendered_len = fs::metadata(rendered_output)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))
        .map_err(remove_and_fail)?
        .len();
    let written_bytes = fs::metadata(&materialized.absolute_path)
        .map_err(|error| TemporalWorkflowInputError::SaveSplitProject(error.to_string()))
        .map_err(remove_and_fail)?
        .len();
    let identical = rendered_len == written_bytes
        && files_match(rendered_output, &materialized.absolute_path).map_err(remove_and_fail)?;
    if written_bytes == 0 || !identical {
        return Err(remove_and_fail(TemporalWorkflowInputError::Render(
            format!(
                "{} export artifact did not match the validated render",
                json!(profile).as_str().unwrap_or_default()
            ),
        )));
    }
    let artifact = export_artifact_for(job_id, profile, &materialized.recorded_path, created_at)
        .map_err(|error| remove_and_fail(destination_error(error.to_string())))?;
    Ok(DestinationWrite {
        artifact,
        written_path: materialized.absolute_path.display().to_string(),
        written_bytes,
    })
}
