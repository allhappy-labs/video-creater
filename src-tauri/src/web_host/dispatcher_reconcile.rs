//! Remote reconciliation uses the same workflow observations and durable job updates as desktop.
use super::*;
use crate::workflows::temporal_reconcile::*;

impl HostDispatcher {
    pub(super) fn reconcile_workflows(&self, request: &RpcEnvelope) -> Result<Value, ServiceError> {
        let (path, project) = self.checked_project(request, true)?;
        let updated_at =
            required_string(&request.payload, "updatedAt").map_err(ServiceError::invalid_input)?;
        let in_process = self.settings.generation_backend_in_process()?;
        let result =
            reconcile(path, project, updated_at, in_process).map_err(ServiceError::internal)?;
        serde_json::to_value(result).map_err(|error| ServiceError::internal(error.to_string()))
    }
}

#[cfg(feature = "temporal-worker")]
fn reconcile(
    path: std::path::PathBuf,
    project: VideoProject,
    updated_at: &str,
    in_process: bool,
) -> Result<TemporalJobReconciliationResult, String> {
    if temporal_reconciliation_workflow_ids(&project, in_process).is_empty() {
        return Ok(TemporalJobReconciliationResult {
            project: Some(project),
            failed_job_ids: vec![],
            service_reachable: true,
            detail: None,
        });
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async {
        let Ok(client) = connect_temporal_client_from_environment().await else {
            return Ok(unreachable_result());
        };
        reconcile_with_describer(
            &TemporalClientDescriber::new(client),
            path,
            project,
            updated_at,
            in_process,
        )
        .await
    })
}

#[cfg(any(feature = "temporal-worker", test))]
async fn reconcile_with_describer(
    describer: &dyn TemporalWorkflowDescriber,
    path: std::path::PathBuf,
    project: VideoProject,
    updated_at: &str,
    in_process: bool,
) -> Result<TemporalJobReconciliationResult, String> {
    let options = TemporalReconcileOptions::at(updated_at, in_process);
    reconcile_temporal_jobs_with_describer(describer, in_process,
        || async move { Ok(project) },
        |observations| async move {
            apply_temporal_job_reconciliation(&path, &observations, &options)
        }).await
}

#[cfg(not(feature = "temporal-worker"))]
fn reconcile(
    _path: std::path::PathBuf,
    _project: VideoProject,
    _updated_at: &str,
    _in_process: bool,
) -> Result<TemporalJobReconciliationResult, String> {
    Ok(unreachable_result())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::workflows::{
        temporal_job_summary, temporal_transcribe_media_start_request, TemporalWorkflowKind,
    };
    struct MissingWorkflow;
    impl TemporalWorkflowDescriber for MissingWorkflow {
        fn describe<'a>(
            &'a self,
            _: &'a str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = DescribeOutcome> + Send + 'a>>
        {
            Box::pin(async { DescribeOutcome::NotFound })
        }
    }
    #[test]
    fn remote_observation_durably_fails_missing_workflow_without_touching_finished_jobs() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("reconcile.palmier");
        let mut project = sample_project();
        let mut job = temporal_job_summary(
            TemporalWorkflowKind::TranscribeMedia,
            &project.id,
            "speech-job",
            crate::project::model::JobStatus::Running,
            "2026-10-03T00:00:00Z",
        );
        job.start_request = Some(temporal_transcribe_media_start_request(
            &project.id,
            "opaque",
            &project.media[0].id,
            &job.id,
            "auto",
        ));
        job.workflow.as_mut().unwrap().run_id = Some("temporal-run".into());
        let mut finished = job.clone();
        finished.id = "finished-job".into();
        finished.status = crate::project::model::JobStatus::Completed;
        project.jobs.extend([job, finished]);
        let project = save_split_project(&path, &project).unwrap().project;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime
            .block_on(reconcile_with_describer(
                &MissingWorkflow,
                path.clone(),
                project,
                "2026-10-03T01:00:00Z",
                true,
            ))
            .unwrap();
        assert!(result.service_reachable);
        assert_eq!(result.failed_job_ids, ["speech-job"]);
        let saved = load_split_project(&path).unwrap();
        assert_eq!(
            saved
                .jobs
                .iter()
                .find(|job| job.id == "speech-job")
                .unwrap()
                .status,
            crate::project::model::JobStatus::Failed
        );
        assert_eq!(
            saved
                .jobs
                .iter()
                .find(|job| job.id == "finished-job")
                .unwrap()
                .status,
            crate::project::model::JobStatus::Completed
        );
    }
}
