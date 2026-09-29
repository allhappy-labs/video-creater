//! Reconciliation through the real `temporalio_client` describer.
//!
//! The `dev_server_*` tests are ignored: they need a Temporal dev server at `TEMPORAL_ADDRESS`.
//! Run them with `-- --ignored` (see docs/development/runtime-and-verification.md).

use super::{export_job, job_in, project_with_jobs, workflow_id};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::{Duration, Instant};
use temporalio_client::{Client, UntypedWorkflow, WorkflowTerminateOptions};
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::model::{JobStatus, JobSummary, VideoProject};
use video_creater_lib::project::split::load_split_project;
use video_creater_lib::workflows::temporal_reconcile::{
    apply_temporal_job_reconciliation, connect_temporal_client_from_environment,
    reconcile_temporal_jobs_with_describer, DescribeOutcome, TemporalClientDescriber,
    TemporalJobReconciliationResult, TemporalReconcileOptions, TemporalWorkflowDescriber,
};
use video_creater_lib::workflows::{
    temporal_export_media_start_request, temporal_job_summary, temporal_start_workflow_with_client,
    TemporalWorkflowKind,
};

pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

/// Restores `TEMPORAL_ADDRESS` when a test that points it at a closed port ends.
struct TemporalAddressOverride(Option<String>);

impl TemporalAddressOverride {
    fn closed_port() -> Self {
        let previous = std::env::var("TEMPORAL_ADDRESS").ok();
        std::env::set_var("TEMPORAL_ADDRESS", "http://127.0.0.1:1");
        Self(previous)
    }
}

impl Drop for TemporalAddressOverride {
    fn drop(&mut self) {
        match self.0.take() {
            Some(address) => std::env::set_var("TEMPORAL_ADDRESS", address),
            None => std::env::remove_var("TEMPORAL_ADDRESS"),
        }
    }
}

/// Points `TEMPORAL_NAMESPACE` at a namespace the server does not have, restoring it on drop.
struct TemporalNamespaceOverride(Option<String>);

impl TemporalNamespaceOverride {
    fn set(namespace: &str) -> Self {
        let previous = std::env::var("TEMPORAL_NAMESPACE").ok();
        std::env::set_var("TEMPORAL_NAMESPACE", namespace);
        Self(previous)
    }
}

impl Drop for TemporalNamespaceOverride {
    fn drop(&mut self) {
        match self.0.take() {
            Some(namespace) => std::env::set_var("TEMPORAL_NAMESPACE", namespace),
            None => std::env::remove_var("TEMPORAL_NAMESPACE"),
        }
    }
}

/// Stands in for the service when the connection itself failed.
struct UnreachableDescriber(String);

impl TemporalWorkflowDescriber for UnreachableDescriber {
    fn describe<'a>(
        &'a self,
        _workflow_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = DescribeOutcome> + Send + 'a>> {
        Box::pin(async move { DescribeOutcome::Unreachable(self.0.clone()) })
    }
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    )
}

/// The same reconciliation the desktop command runs, with the project I/O inline.
async fn reconcile_with(
    describer: &dyn TemporalWorkflowDescriber,
    project_dir: &Path,
) -> TemporalJobReconciliationResult {
    let options = TemporalReconcileOptions::at(&chrono::Utc::now().to_rfc3339(), false);
    reconcile_temporal_jobs_with_describer(
        describer,
        false,
        || async { load_split_project(project_dir).map_err(|error| error.to_string()) },
        |observations| async move {
            apply_temporal_job_reconciliation(project_dir, &observations, &options)
        },
    )
    .await
    .expect("reconcile")
}

/// An export job whose workflow runs on a task queue no worker polls, so it stays Running.
fn workerless_export_job(project_dir: &Path, job_id: &str) -> JobSummary {
    let task_queue = unique("vc-reconcile-no-worker");
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        "project-1",
        job_id,
        JobStatus::Queued,
        &chrono::Utc::now().to_rfc3339(),
    );
    let mut start_request = temporal_export_media_start_request(
        "project-1",
        &project_dir.display().to_string(),
        job_id,
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Draft, 320, 180)
            .expect("export options"),
        &format!("exports/{job_id}.mp4"),
    );
    start_request.task_queue = task_queue.clone();
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.workflow_id = start_request.workflow_id.clone();
        workflow.workflow_type = start_request.workflow_type.clone();
        workflow.task_queue = task_queue;
        workflow.activity_types = start_request.activity_types.clone();
    }
    job.start_request = Some(start_request);
    job
}

/// Starts the job's workflow and records it running with its run id.
async fn start_and_record(
    client: &Client,
    job_id: &str,
) -> (tempfile::TempDir, std::path::PathBuf) {
    let (temp, project_dir) = project_with_jobs(Vec::new());
    let mut job = workerless_export_job(&project_dir, job_id);
    let started = temporal_start_workflow_with_client(client, &job)
        .await
        .expect("start workflow on the dev server");
    let run_id = started.run_id.expect("run id");
    job.status = JobStatus::Running;
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.run_id = Some(run_id);
    }
    let mut project: VideoProject = load_split_project(&project_dir).expect("load");
    project.jobs.push(job);
    video_creater_lib::project::split::save_split_project(&project_dir, &project)
        .expect("record the running job");
    (temp, project_dir)
}

#[test]
fn unreachable_server_is_reported_quickly() {
    let _address = TemporalAddressOverride::closed_port();
    let started = Instant::now();

    let result = runtime().block_on(connect_temporal_client_from_environment());

    let elapsed = started.elapsed();
    assert!(result.is_err(), "a closed port must not connect");
    assert!(
        elapsed < Duration::from_secs(5),
        "connecting to a closed port took {elapsed:?}"
    );
}

#[test]
fn closed_port_leaves_jobs_untouched() {
    let _address = TemporalAddressOverride::closed_port();
    let job = export_job("export-1", JobStatus::Running, Some("run-1"), super::NOW);
    let (_temp, project_dir) = project_with_jobs(vec![job]);
    let before = load_split_project(&project_dir).expect("load");

    let result = runtime().block_on(async {
        match connect_temporal_client_from_environment().await {
            Ok(client) => reconcile_with(&TemporalClientDescriber::new(client), &project_dir).await,
            Err(error) => reconcile_with(&UnreachableDescriber(error), &project_dir).await,
        }
    });

    assert!(!result.service_reachable);
    assert_eq!(
        result.detail.as_deref(),
        Some("Workflow service unreachable")
    );
    let after = load_split_project(&project_dir).expect("load");
    assert_eq!(after.content_revision, before.content_revision);
    assert_eq!(job_in(&after, "export-1").status, JobStatus::Running);
}

#[test]
#[ignore = "needs a Temporal dev server at TEMPORAL_ADDRESS"]
fn dev_server_running_workflow_without_a_worker_is_left_running() {
    runtime().block_on(async {
        let client = connect_temporal_client_from_environment()
            .await
            .expect("connect to the dev server");
        let job_id = unique("export-running");
        let (_temp, project_dir) = start_and_record(&client, &job_id).await;

        let result = reconcile_with(&TemporalClientDescriber::new(client), &project_dir).await;

        eprintln!(
            "running workflow reconciliation: {:?}",
            result.failed_job_ids
        );
        assert!(result.service_reachable);
        assert!(result.failed_job_ids.is_empty());
        let job = job_in(&load_split_project(&project_dir).expect("load"), &job_id);
        assert_eq!(job.status, JobStatus::Running);
    });
}

#[test]
#[ignore = "needs a Temporal dev server at TEMPORAL_ADDRESS"]
fn dev_server_terminated_workflow_fails_the_job() {
    runtime().block_on(async {
        let client = connect_temporal_client_from_environment()
            .await
            .expect("connect to the dev server");
        let job_id = unique("export-terminated");
        let (_temp, project_dir) = start_and_record(&client, &job_id).await;
        let project = load_split_project(&project_dir).expect("load");
        client
            .get_workflow_handle::<UntypedWorkflow>(workflow_id(&job_in(&project, &job_id)))
            .terminate(WorkflowTerminateOptions::default())
            .await
            .expect("terminate the workflow");

        let result = reconcile_with(&TemporalClientDescriber::new(client), &project_dir).await;

        eprintln!(
            "terminated workflow reconciliation: {:?}",
            result.failed_job_ids
        );
        assert_eq!(result.failed_job_ids, vec![job_id.clone()]);
        let job = job_in(&load_split_project(&project_dir).expect("load"), &job_id);
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(
            job.failure_reason.as_deref(),
            Some("The workflow was stopped.")
        );
    });
}

#[test]
#[ignore = "needs a Temporal dev server at TEMPORAL_ADDRESS"]
fn dev_server_unknown_workflow_fails_a_stale_queued_job() {
    runtime().block_on(async {
        let client = connect_temporal_client_from_environment()
            .await
            .expect("connect to the dev server");
        let job_id = unique("export-never-started");
        let ten_minutes_ago = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        let (_temp, project_dir) = project_with_jobs(Vec::new());
        let mut job = workerless_export_job(&project_dir, &job_id);
        job.updated_at = ten_minutes_ago;
        let mut project = load_split_project(&project_dir).expect("load");
        project.jobs.push(job);
        video_creater_lib::project::split::save_split_project(&project_dir, &project)
            .expect("record the queued job");

        let result = reconcile_with(&TemporalClientDescriber::new(client), &project_dir).await;

        eprintln!("never-started reconciliation: {:?}", result.failed_job_ids);
        assert_eq!(result.failed_job_ids, vec![job_id.clone()]);
        let job = job_in(&load_split_project(&project_dir).expect("load"), &job_id);
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(
            job.failure_reason.as_deref(),
            Some("The workflow never started.")
        );
    });
}

#[test]
#[ignore = "needs a Temporal dev server at TEMPORAL_ADDRESS"]
fn dev_server_missing_namespace_leaves_jobs_untouched() {
    let namespace = unique("vc-missing-namespace");
    let _namespace = TemporalNamespaceOverride::set(&namespace);
    let running = export_job("export-1", JobStatus::Running, Some("run-1"), super::NOW);
    let (_temp, project_dir) = project_with_jobs(vec![running]);
    let before = load_split_project(&project_dir).expect("load");

    let (outcome, result) = runtime().block_on(async {
        let client = connect_temporal_client_from_environment()
            .await
            .expect("connect to the dev server");
        let describer = TemporalClientDescriber::new(client);
        let project = load_split_project(&project_dir).expect("load");
        let outcome = describer
            .describe(&workflow_id(&job_in(&project, "export-1")))
            .await;
        let result = reconcile_with(&describer, &project_dir).await;
        (outcome, result)
    });

    eprintln!("missing namespace describe: {outcome:?}; reconciliation: {result:?}");
    assert_eq!(
        outcome,
        DescribeOutcome::NamespaceNotFound(namespace.clone())
    );
    assert!(!result.service_reachable);
    assert_eq!(
        result.detail,
        Some(format!("Workflow namespace \"{namespace}\" not found"))
    );
    let after = load_split_project(&project_dir).expect("load");
    assert_eq!(after.content_revision, before.content_revision);
    assert_eq!(job_in(&after, "export-1").status, JobStatus::Running);
}
