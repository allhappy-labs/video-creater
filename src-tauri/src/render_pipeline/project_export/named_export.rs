//! In-process video exports saved under the chosen file name and folder.
//!
//! The render still writes `renders/<jobId>/output.<ext>`; on completion the
//! file is materialized at its export name without overwriting anything, and
//! the export artifact is recorded in the same batch as the render report.

use std::path::Path;

use crate::project::export_destination::{
    export_artifact_contract, export_artifact_for, prepare_export_output_cancellable,
    resolve_export_destination, ExportDestination, ExportDestinationError, ExportMaterialization,
    ExportOutputRequest, ExportPublicationOwner, LinkPolicy, MaterializedExport,
    PreparedExportOutput,
};
use crate::project::export_options::{ExportRenderOptions, JobExportSettings};
use crate::project::model::{JobSummary, ProjectExportArtifact};
use crate::render_pipeline::cancel::RenderCancellationToken;
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

pub(super) fn resolve_output_destination(
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
    owner: ExportPublicationOwner,
}

impl SavedExport {
    pub(super) fn rollback(&self) {
        let _ = self.owner.rollback(&self.materialized.absolute_path);
    }
}

pub(super) struct PreparedNamedExport {
    staged: PreparedExportOutput,
    profile: crate::project::export_profiles::ExportProfile,
    job_id: String,
    created_at: String,
}

pub(super) fn prepare_rendered_export(
    project_dir: &Path,
    options: ExportRenderOptions,
    output: &ExportOutputRequest,
    rendered_output: &Path,
    job_id: &str,
    created_at: &str,
    cancellation: &RenderCancellationToken,
) -> PipelineResult<PreparedNamedExport> {
    let destination = resolve_output_destination(project_dir, options, output)?;
    let staged = prepare_export_output_cancellable(
        ExportMaterialization {
            destination: &destination,
            source: rendered_output,
            job_id,
            link_policy: LinkPolicy::Auto,
        },
        &mut || !cancellation.is_cancelled(),
    )
    .map_err(|error| {
        if error == ExportDestinationError::Cancelled {
            super::render_cancelled_error("export.preparation.cancelled")
        } else {
            export_output_error(error)
        }
    })?;
    Ok(PreparedNamedExport {
        staged,
        profile: options.profile,
        job_id: job_id.into(),
        created_at: created_at.into(),
    })
}

impl PreparedNamedExport {
    pub(super) fn publish(self) -> PipelineResult<SavedExport> {
        let owner = self
            .staged
            .publication_owner()
            .map_err(export_output_error)?;
        let materialized = self.staged.publish().map_err(export_output_error)?;
        match export_artifact_for(
            &self.job_id,
            self.profile,
            &materialized.recorded_path,
            &self.created_at,
        ) {
            Ok(artifact) => Ok(SavedExport {
                materialized,
                artifact,
                owner,
            }),
            Err(error) => {
                let _ = owner.rollback(&materialized.absolute_path);
                Err(export_output_error(error))
            }
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
