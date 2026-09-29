//! In-process video exports saved under the chosen file name and folder.
//!
//! The render still writes `renders/<jobId>/output.<ext>`; on completion the
//! file is materialized at its export name without overwriting anything, and
//! the export artifact is recorded in the same batch as the render report.

use std::path::Path;

use crate::project::export_destination::{
    export_artifact_contract, export_artifact_for, materialize_export_output,
    resolve_export_destination, ExportDestination, ExportDestinationError, ExportMaterialization,
    ExportOutputRequest, LinkPolicy, MaterializedExport,
};
use crate::project::export_options::{ExportRenderOptions, JobExportSettings};
use crate::project::model::{JobSummary, ProjectExportArtifact};
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};

use super::{
    register_project_render_attempt, render_media_to_split_project_folder_for_timeline_with_lease,
    MediaRenderRequest, ProjectMediaRenderResult,
};

/// An in-process export request. `output` names the saved file and folder;
/// without it the render only produces `renders/<jobId>/output.<ext>`.
pub struct MediaExportRequest<'a> {
    pub project_dir: &'a Path,
    pub project_id: &'a str,
    pub options: ExportRenderOptions,
    pub job: JobSummary,
    pub updated_at: &'a str,
    pub run_id: Option<String>,
    pub range_seconds: Option<(f64, f64)>,
    pub timeline_id: Option<&'a str>,
    pub output: Option<&'a ExportOutputRequest>,
}

pub fn render_media_export_to_split_project_folder(
    request: MediaExportRequest<'_>,
) -> PipelineResult<ProjectMediaRenderResult> {
    let MediaExportRequest {
        project_dir,
        project_id,
        options,
        mut job,
        updated_at,
        run_id,
        range_seconds,
        timeline_id,
        output,
    } = request;
    if let Some(output) = output {
        // Validate only; the destination is resolved again when the file is saved.
        resolve_output_destination(project_dir, options, output)?;
        if job.export_settings.is_none() {
            job.export_settings = Some(JobExportSettings {
                options,
                output: Some(output.clone()),
            });
        }
    }
    let attempt = register_project_render_attempt(project_dir, project_id, &job.id, run_id)?;
    let lease = super::acquire_render_storage_mutation_lease()?;
    let _artifacts = super::acquire_render_project_artifact_lease(project_dir)?;
    render_media_to_split_project_folder_for_timeline_with_lease(MediaRenderRequest {
        project_dir,
        options,
        job,
        updated_at,
        attempt,
        range_seconds,
        timeline_id,
        lease: &lease,
        output: output.cloned(),
        completes_job: true,
    })
}

fn resolve_output_destination(
    project_dir: &Path,
    options: ExportRenderOptions,
    output: &ExportOutputRequest,
) -> PipelineResult<ExportDestination> {
    let (_, extension, _) = export_artifact_contract(options.profile)
        .ok_or_else(|| export_output_error(ExportDestinationError::UnsupportedProfile))?;
    resolve_export_destination(project_dir, output, extension).map_err(export_output_error)
}

/// A rendered export saved at its export name, and the artifact to record.
pub(super) struct SavedExport {
    pub(super) materialized: MaterializedExport,
    pub(super) artifact: ProjectExportArtifact,
}

pub(super) fn save_rendered_export(
    project_dir: &Path,
    options: ExportRenderOptions,
    output: &ExportOutputRequest,
    rendered_output: &Path,
    job_id: &str,
    created_at: &str,
) -> PipelineResult<SavedExport> {
    let destination = resolve_output_destination(project_dir, options, output)?;
    let materialized = materialize_export_output(ExportMaterialization {
        destination: &destination,
        source: rendered_output,
        job_id,
        link_policy: LinkPolicy::Auto,
    })
    .map_err(export_output_error)?;
    match export_artifact_for(
        job_id,
        options.profile,
        &materialized.recorded_path,
        created_at,
    ) {
        Ok(artifact) => Ok(SavedExport {
            materialized,
            artifact,
        }),
        Err(error) => {
            let _ = std::fs::remove_file(&materialized.absolute_path);
            Err(export_output_error(error))
        }
    }
}

fn export_output_error(error: ExportDestinationError) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineInputInvalid,
        "export.output",
        error.to_string(),
        "Choose another export folder or name, then export again.",
    )]
}
