//! Reconciles unfinished Temporal-backed jobs with the state of their workflows.
//!
//! A job whose workflow closed without reporting its result, or which the workflow service does
//! not know, would otherwise stay "running" in Background tasks forever. Reconciliation describes
//! each candidate job's workflow and fails the job with a plain reason when the workflow is closed
//! or missing. An unreachable workflow service proves nothing about the workflow, so it leaves
//! every job untouched and reports "Workflow service unreachable". A namespace the service does
//! not have proves nothing either (the configuration is wrong, not the workflow), so it also
//! leaves every job untouched and reports the missing namespace.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::project::action::ProjectAction;
use crate::project::model::{GeneratedAssetStatus, JobStatus, JobSummary, VideoProject};
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::split::{apply_project_actions_to_split_project, load_split_project};
use crate::workflows::TemporalWorkflowKind;

/// How long a queued job without a run id may wait for its start to record one.
pub const TEMPORAL_START_GRACE: Duration = Duration::from_secs(120);
/// The detail shown on Temporal-backed tasks when the service cannot be reached.
pub const WORKFLOW_SERVICE_UNREACHABLE: &str = "Workflow service unreachable";
/// The failure reason for a job whose workflow never started.
pub const WORKFLOW_NEVER_STARTED: &str = "The workflow never started.";

const LOCAL_RUN_ID_PREFIXES: &[&str] = &["in-process/", "render-attempt/", "mock-run-"];
const CAPTURE_CANONICAL_PREVIEW_FRAME_KIND: &str = "captureCanonicalPreviewFrame";

/// A described workflow execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowObservation {
    Unspecified,
    Running,
    Completed,
    Failed,
    Canceled,
    Terminated,
    ContinuedAsNew,
    TimedOut,
    Paused,
}

/// The result of describing one workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescribeOutcome {
    Observed(WorkflowObservation),
    /// The service has no such workflow in its namespace.
    NotFound,
    /// The configured namespace does not exist on the service; holds the namespace name.
    NamespaceNotFound(String),
    Unreachable(String),
}

/// Describes workflows by id. The real implementation talks to Temporal; tests use fakes.
pub trait TemporalWorkflowDescriber: Send + Sync {
    fn describe<'a>(
        &'a self,
        workflow_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = DescribeOutcome> + Send + 'a>>;
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalJobReconciliationResult {
    pub project: Option<VideoProject>,
    pub failed_job_ids: Vec<String>,
    pub service_reachable: bool,
    pub detail: Option<String>,
}

/// What describing the workflows saw: each outcome by workflow id, and the run id each candidate
/// job had when its workflow was described.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemporalReconcileObservations {
    pub outcomes: BTreeMap<String, DescribeOutcome>,
    pub described_run_ids: BTreeMap<String, Option<String>>,
}

impl TemporalReconcileObservations {
    /// Whether `job` still has the run id it had when its workflow was described. A job that
    /// started (or restarted) since then was not what the outcome describes.
    fn still_describes(&self, job: &JobSummary) -> bool {
        self.described_run_ids
            .get(&job.id)
            .is_some_and(|run_id| run_id.as_deref() == job_run_id(job))
    }
}

/// Inputs shared by planning and applying a reconciliation.
#[derive(Debug, Clone)]
pub struct TemporalReconcileOptions {
    pub now: DateTime<Utc>,
    pub updated_at: String,
    pub grace: Duration,
    pub generation_backend_in_process: bool,
}

impl TemporalReconcileOptions {
    /// Options for `updated_at`, falling back to the current time when it does not parse.
    pub fn at(updated_at: &str, generation_backend_in_process: bool) -> Self {
        let now = DateTime::parse_from_rfc3339(updated_at)
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        Self {
            now,
            updated_at: updated_at.to_string(),
            grace: TEMPORAL_START_GRACE,
            generation_backend_in_process,
        }
    }
}

/// The workflow id to describe when `job` is a reconciliation candidate.
fn candidate_workflow_id(job: &JobSummary, generation_backend_in_process: bool) -> Option<&str> {
    if !matches!(
        job.status,
        JobStatus::Queued | JobStatus::Running | JobStatus::Progress | JobStatus::Blocked
    ) || job.kind == CAPTURE_CANONICAL_PREVIEW_FRAME_KIND
    {
        return None;
    }
    let start_request = job.start_request.as_ref()?;
    let run_id = job_run_id(job);
    if run_id.is_some_and(|run_id| {
        LOCAL_RUN_ID_PREFIXES
            .iter()
            .any(|prefix| run_id.starts_with(prefix))
    }) {
        return None;
    }
    // `reconcile_interrupted_generation_jobs_on_project_open` owns these.
    if run_id.is_none()
        && generation_backend_in_process
        && job.kind == TemporalWorkflowKind::GenerateMedia.job_kind()
    {
        return None;
    }
    Some(start_request.workflow_id.as_str())
}

fn job_run_id(job: &JobSummary) -> Option<&str> {
    job.workflow
        .as_ref()
        .and_then(|workflow| workflow.run_id.as_deref())
}

/// Workflow ids of every candidate job, without duplicates.
pub fn temporal_reconciliation_workflow_ids(
    project: &VideoProject,
    generation_backend_in_process: bool,
) -> Vec<String> {
    let mut ids: Vec<String> = project
        .jobs
        .iter()
        .filter_map(|job| candidate_workflow_id(job, generation_backend_in_process))
        .map(str::to_string)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn closed_workflow_reason(observation: WorkflowObservation) -> Option<&'static str> {
    match observation {
        WorkflowObservation::Running
        | WorkflowObservation::Paused
        | WorkflowObservation::ContinuedAsNew
        | WorkflowObservation::Unspecified => None,
        WorkflowObservation::Completed => {
            Some("The workflow finished without reporting this task's result.")
        }
        WorkflowObservation::Failed => Some("The workflow failed."),
        WorkflowObservation::Canceled => Some("The workflow was cancelled."),
        WorkflowObservation::Terminated => Some("The workflow was stopped."),
        WorkflowObservation::TimedOut => Some("The workflow timed out."),
    }
}

/// Maps each candidate job and its workflow's observation to a failure reason.
pub fn plan_temporal_job_reconciliation(
    project: &VideoProject,
    observations: &BTreeMap<String, DescribeOutcome>,
    now: DateTime<Utc>,
    grace: Duration,
    generation_backend_in_process: bool,
) -> Vec<(String, String)> {
    project
        .jobs
        .iter()
        .filter_map(|job| {
            let workflow_id = candidate_workflow_id(job, generation_backend_in_process)?;
            let outcome = observations.get(workflow_id)?;
            let reason = if job_run_id(job).is_some() {
                match outcome {
                    DescribeOutcome::Observed(observation) => closed_workflow_reason(*observation)?,
                    DescribeOutcome::NotFound => "The workflow service has no record of this task.",
                    DescribeOutcome::NamespaceNotFound(_) | DescribeOutcome::Unreachable(_) => {
                        return None
                    }
                }
            } else {
                if *outcome != DescribeOutcome::NotFound || !older_than(job, now, grace) {
                    return None;
                }
                WORKFLOW_NEVER_STARTED
            };
            Some((job.id.clone(), reason.to_string()))
        })
        .collect()
}

fn older_than(job: &JobSummary, now: DateTime<Utc>, grace: Duration) -> bool {
    let Ok(updated_at) = DateTime::parse_from_rfc3339(&job.updated_at) else {
        return false;
    };
    let Ok(grace) = chrono::Duration::from_std(grace) else {
        return false;
    };
    now.signed_duration_since(updated_at.with_timezone(&Utc)) > grace
}

/// Reloads the project under its lease, re-plans against it, and records each failure.
///
/// Re-planning against a fresh load keeps a job that finished while its workflow was described
/// from being failed, and a job whose run id changed since then keeps its new run. Each failure is
/// stamped no earlier than the job's own last update, so the editor's merge takes it.
pub fn apply_temporal_job_reconciliation(
    project_dir: &Path,
    observations: &TemporalReconcileObservations,
    options: &TemporalReconcileOptions,
) -> Result<(VideoProject, Vec<String>), String> {
    let _lease = acquire_split_project_mutation_lease(project_dir)?;
    let project = load_split_project(project_dir).map_err(|error| error.to_string())?;
    let failures: Vec<(String, String)> = plan_temporal_job_reconciliation(
        &project,
        &observations.outcomes,
        options.now,
        options.grace,
        options.generation_backend_in_process,
    )
    .into_iter()
    .filter(|(job_id, _)| {
        project
            .jobs
            .iter()
            .find(|job| &job.id == job_id)
            .is_some_and(|job| observations.still_describes(job))
    })
    .collect();
    if failures.is_empty() {
        return Ok((project, Vec::new()));
    }
    let failed_job_ids = failures.iter().map(|(job_id, _)| job_id.clone()).collect();
    let mut actions = Vec::new();
    for (job_id, reason) in failures {
        // A generation's tile follows its asset, so a failed generation job fails its unfinished
        // asset too, as every other generation failure path does.
        let unfinished_asset_id = unfinished_generation_asset_id(&project, &job_id);
        let updated_at = failure_updated_at(&project, &job_id, options);
        actions.push(ProjectAction::RecordJobFailure {
            job_id,
            reason,
            updated_at,
            run_id: None,
        });
        if let Some(asset_id) = unfinished_asset_id {
            actions.push(ProjectAction::UpdateGeneratedAssetStatus {
                asset_id,
                status: GeneratedAssetStatus::Failed,
            });
        }
    }
    let write = apply_project_actions_to_split_project(project_dir, actions)
        .map_err(|error| error.to_string())?;
    Ok((write.project, failed_job_ids))
}

/// The reconciliation time, or the job's own last update when that is later.
fn failure_updated_at(
    project: &VideoProject,
    job_id: &str,
    options: &TemporalReconcileOptions,
) -> String {
    let job_updated_at = project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .and_then(|job| {
            DateTime::parse_from_rfc3339(&job.updated_at)
                .ok()
                .map(|parsed| (parsed.with_timezone(&Utc), job.updated_at.clone()))
        });
    match job_updated_at {
        Some((parsed, text)) if parsed > options.now => text,
        _ => options.updated_at.clone(),
    }
}

/// The queued or running generated asset of a `generate_media` job, if any.
fn unfinished_generation_asset_id(project: &VideoProject, job_id: &str) -> Option<String> {
    let job = project.jobs.iter().find(|job| job.id == job_id)?;
    if job.kind != TemporalWorkflowKind::GenerateMedia.job_kind() {
        return None;
    }
    let asset_id = job
        .start_request
        .as_ref()
        .and_then(|request| request.input.get("assetId"))
        .and_then(|value| value.as_str())
        .unwrap_or(job.id.as_str());
    project
        .generated_assets
        .iter()
        .find(|asset| {
            asset.id == asset_id
                && matches!(
                    asset.status,
                    GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
                )
        })
        .map(|asset| asset.id.clone())
}

/// Describes every candidate job's workflow, then applies the failures through `apply`.
///
/// `load` and `apply` are injected so the caller decides where blocking project I/O runs (the
/// desktop command uses the blocking pool and the project command queue). When any workflow
/// service call is unreachable, or the namespace is missing, nothing is written.
pub async fn reconcile_temporal_jobs_with_describer<Load, LoadFuture, Apply, ApplyFuture>(
    describer: &dyn TemporalWorkflowDescriber,
    generation_backend_in_process: bool,
    load: Load,
    apply: Apply,
) -> Result<TemporalJobReconciliationResult, String>
where
    Load: FnOnce() -> LoadFuture,
    LoadFuture: Future<Output = Result<VideoProject, String>>,
    Apply: FnOnce(TemporalReconcileObservations) -> ApplyFuture,
    ApplyFuture: Future<Output = Result<(VideoProject, Vec<String>), String>>,
{
    let project = load().await?;
    let mut observations = TemporalReconcileObservations {
        described_run_ids: project
            .jobs
            .iter()
            .filter(|job| candidate_workflow_id(job, generation_backend_in_process).is_some())
            .map(|job| (job.id.clone(), job_run_id(job).map(str::to_string)))
            .collect(),
        ..TemporalReconcileObservations::default()
    };
    for workflow_id in temporal_reconciliation_workflow_ids(&project, generation_backend_in_process)
    {
        let outcome = describer.describe(&workflow_id).await;
        match outcome {
            DescribeOutcome::Unreachable(_) => return Ok(unreachable_result()),
            DescribeOutcome::NamespaceNotFound(namespace) => {
                return Ok(namespace_not_found_result(&namespace))
            }
            _ => {}
        }
        observations.outcomes.insert(workflow_id, outcome);
    }
    if observations.outcomes.is_empty() {
        return Ok(TemporalJobReconciliationResult {
            project: Some(project),
            failed_job_ids: Vec::new(),
            service_reachable: true,
            detail: None,
        });
    }
    let (project, failed_job_ids) = apply(observations).await?;
    Ok(TemporalJobReconciliationResult {
        project: Some(project),
        failed_job_ids,
        service_reachable: true,
        detail: None,
    })
}

/// The result reported when the workflow service cannot be reached.
pub fn unreachable_result() -> TemporalJobReconciliationResult {
    TemporalJobReconciliationResult {
        project: None,
        failed_job_ids: Vec::new(),
        service_reachable: false,
        detail: Some(WORKFLOW_SERVICE_UNREACHABLE.to_string()),
    }
}

/// The result reported when the workflow service has no namespace named `namespace`.
pub fn namespace_not_found_result(namespace: &str) -> TemporalJobReconciliationResult {
    TemporalJobReconciliationResult {
        project: None,
        failed_job_ids: Vec::new(),
        service_reachable: false,
        detail: Some(format!("Workflow namespace \"{namespace}\" not found")),
    }
}

#[cfg(feature = "temporal-worker")]
mod client {
    use super::{DescribeOutcome, TemporalWorkflowDescriber, WorkflowObservation};
    use prost::Message;
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;
    use temporalio_client::errors::WorkflowInteractionError;
    use temporalio_client::{Client, UntypedWorkflow, WorkflowDescribeOptions};
    use temporalio_common::protos::google::rpc::Status as RpcStatus;
    use temporalio_common::protos::temporal::api::enums::v1::WorkflowExecutionStatus;
    use temporalio_common::protos::temporal::api::errordetails::v1::NamespaceNotFoundFailure;

    const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
    const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Connects with the same environment configuration as starting a workflow, bounded so an
    /// unreachable server is reported quickly.
    pub async fn connect_temporal_client_from_environment() -> Result<Client, String> {
        let (connection_options, client_options) =
            temporalio_client::ClientOptions::load_from_config(
                temporalio_client::envconfig::LoadClientConfigProfileOptions::default(),
            )
            .map_err(|error| error.to_string())?;
        let connection = tokio::time::timeout(
            CONNECT_TIMEOUT,
            temporalio_client::Connection::connect(connection_options),
        )
        .await
        .map_err(|_| "timed out connecting to the workflow service".to_string())?
        .map_err(|error| error.to_string())?;
        Client::new(connection, client_options).map_err(|error| error.to_string())
    }

    /// Describes workflows through a connected Temporal client.
    pub struct TemporalClientDescriber {
        client: Client,
    }

    impl TemporalClientDescriber {
        pub fn new(client: Client) -> Self {
            Self { client }
        }
    }

    impl TemporalWorkflowDescriber for TemporalClientDescriber {
        fn describe<'a>(
            &'a self,
            workflow_id: &'a str,
        ) -> Pin<Box<dyn Future<Output = DescribeOutcome> + Send + 'a>> {
            Box::pin(async move {
                let handle = self
                    .client
                    .get_workflow_handle::<UntypedWorkflow>(workflow_id);
                let described = tokio::time::timeout(
                    DESCRIBE_TIMEOUT,
                    handle.describe(WorkflowDescribeOptions::default()),
                )
                .await;
                match described {
                    Ok(Ok(description)) => {
                        DescribeOutcome::Observed(observation(description.status()))
                    }
                    Ok(Err(WorkflowInteractionError::NotFound(status))) => {
                        not_found_outcome(status.details(), status.message())
                    }
                    // An RPC or conversion failure does not prove the workflow is gone.
                    Ok(Err(error)) => DescribeOutcome::Unreachable(error.to_string()),
                    Err(_) => DescribeOutcome::Unreachable(
                        "timed out describing the workflow".to_string(),
                    ),
                }
            })
        }
    }

    const NAMESPACE_NOT_FOUND_TYPE: &str = "temporal.api.errordetails.v1.NamespaceNotFoundFailure";

    /// Tells a missing namespace apart from a missing workflow in a gRPC `NotFound` reply.
    ///
    /// Both arrive as `NotFound`; the server attaches a `NamespaceNotFoundFailure` detail for a
    /// missing namespace and a `NotFoundFailure` for a missing workflow. The detail's type URL
    /// decides, because the two messages decode into each other. Without details, the server's
    /// "Namespace <name> is not found." message decides.
    pub(super) fn not_found_outcome(details: &[u8], message: &str) -> DescribeOutcome {
        if let Ok(status) = RpcStatus::decode(details) {
            if let Some(detail) = status.details.iter().find(|detail| {
                detail
                    .type_url
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name == NAMESPACE_NOT_FOUND_TYPE)
            }) {
                let namespace = NamespaceNotFoundFailure::decode(detail.value.as_slice())
                    .map(|failure| failure.namespace)
                    .unwrap_or_default();
                return DescribeOutcome::NamespaceNotFound(namespace);
            }
            if !status.details.is_empty() {
                return DescribeOutcome::NotFound;
            }
        }
        match message
            .strip_prefix("Namespace ")
            .and_then(|rest| rest.strip_suffix(" is not found."))
        {
            Some(namespace) => DescribeOutcome::NamespaceNotFound(namespace.to_string()),
            None => DescribeOutcome::NotFound,
        }
    }

    fn observation(status: WorkflowExecutionStatus) -> WorkflowObservation {
        match status {
            WorkflowExecutionStatus::Unspecified => WorkflowObservation::Unspecified,
            WorkflowExecutionStatus::Running => WorkflowObservation::Running,
            WorkflowExecutionStatus::Completed => WorkflowObservation::Completed,
            WorkflowExecutionStatus::Failed => WorkflowObservation::Failed,
            WorkflowExecutionStatus::Canceled => WorkflowObservation::Canceled,
            WorkflowExecutionStatus::Terminated => WorkflowObservation::Terminated,
            WorkflowExecutionStatus::ContinuedAsNew => WorkflowObservation::ContinuedAsNew,
            WorkflowExecutionStatus::TimedOut => WorkflowObservation::TimedOut,
            WorkflowExecutionStatus::Paused => WorkflowObservation::Paused,
        }
    }
}

#[cfg(feature = "temporal-worker")]
pub use client::{connect_temporal_client_from_environment, TemporalClientDescriber};

#[cfg(test)]
mod tests;
