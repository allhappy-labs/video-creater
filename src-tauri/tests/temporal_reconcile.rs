//! Temporal job reconciliation over a real split project.
//!
//! The fake describer tests always run. The `dev_server_*` tests are ignored and need a Temporal
//! dev server at `TEMPORAL_ADDRESS` (see docs/development/runtime-and-verification.md).

use serde_json::json;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, JobStatus, JobSummary, MediaKind, VideoProject,
};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project, save_split_project,
};
use video_creater_lib::workflows::temporal_reconcile::{
    apply_temporal_job_reconciliation, reconcile_temporal_jobs_with_describer, DescribeOutcome,
    TemporalJobReconciliationResult, TemporalReconcileOptions, TemporalWorkflowDescriber,
    WorkflowObservation,
};
use video_creater_lib::workflows::{
    temporal_job_summary, temporal_workflow_start_request, TemporalWorkflowKind,
};

#[cfg(feature = "temporal-worker")]
#[path = "temporal_reconcile/dev_server.rs"]
mod dev_server;

pub(crate) const NOW: &str = "2026-09-16T12:00:00Z";

struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}

type DescribeHook = Box<dyn Fn(&str) + Send + Sync>;

/// Answers from a fixed table and optionally runs a hook while "describing".
struct FakeDescriber {
    outcomes: BTreeMap<String, DescribeOutcome>,
    hook: Option<DescribeHook>,
    described: Mutex<Vec<String>>,
}

impl FakeDescriber {
    fn new(outcomes: impl IntoIterator<Item = (String, DescribeOutcome)>) -> Self {
        Self {
            outcomes: outcomes.into_iter().collect(),
            hook: None,
            described: Mutex::new(Vec::new()),
        }
    }
}

impl TemporalWorkflowDescriber for FakeDescriber {
    fn describe<'a>(
        &'a self,
        workflow_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = DescribeOutcome> + Send + 'a>> {
        Box::pin(async move {
            self.described
                .lock()
                .expect("described")
                .push(workflow_id.to_string());
            if let Some(hook) = &self.hook {
                hook(workflow_id);
            }
            self.outcomes
                .get(workflow_id)
                .cloned()
                .unwrap_or(DescribeOutcome::NotFound)
        })
    }
}

pub(crate) fn export_job(
    id: &str,
    status: JobStatus,
    run_id: Option<&str>,
    updated_at: &str,
) -> JobSummary {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        "project-1",
        id,
        status,
        updated_at,
    );
    job.start_request = Some(temporal_workflow_start_request(
        TemporalWorkflowKind::ExportMedia,
        "project-1",
        id,
        json!({}),
    ));
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.run_id = run_id.map(str::to_string);
    }
    job
}

pub(crate) fn workflow_id(job: &JobSummary) -> String {
    job.start_request
        .as_ref()
        .expect("start request")
        .workflow_id
        .clone()
}

pub(crate) fn project_with_jobs(jobs: Vec<JobSummary>) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temp project parent");
    let project_dir = temp.path().join("project");
    let mut project = sample_project();
    project.jobs = jobs;
    save_split_project(&project_dir, &project).expect("save project");
    (temp, project_dir)
}

pub(crate) fn job_in(project: &VideoProject, job_id: &str) -> JobSummary {
    project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .cloned()
        .expect("job")
}

fn reconcile(project_dir: &Path, describer: &FakeDescriber) -> TemporalJobReconciliationResult {
    let options = TemporalReconcileOptions::at(NOW, false);
    block_on(reconcile_temporal_jobs_with_describer(
        describer,
        false,
        || async { load_split_project(project_dir).map_err(|error| error.to_string()) },
        |observations| async move {
            apply_temporal_job_reconciliation(project_dir, &observations, &options)
        },
    ))
    .expect("reconcile")
}

#[test]
fn a_terminated_workflow_fails_its_running_export_job_with_a_plain_reason() {
    let job = export_job("export-1", JobStatus::Running, Some("run-1"), NOW);
    let describer = FakeDescriber::new([(
        workflow_id(&job),
        DescribeOutcome::Observed(WorkflowObservation::Terminated),
    )]);
    let (_temp, project_dir) = project_with_jobs(vec![job]);

    let result = reconcile(&project_dir, &describer);

    assert!(result.service_reachable);
    assert_eq!(result.detail, None);
    assert_eq!(result.failed_job_ids, vec!["export-1".to_string()]);
    let persisted = job_in(&load_split_project(&project_dir).expect("load"), "export-1");
    assert_eq!(persisted.status, JobStatus::Failed);
    assert_eq!(
        persisted.failure_reason.as_deref(),
        Some("The workflow was stopped.")
    );
    assert_eq!(persisted.updated_at, NOW);
    assert_eq!(
        job_in(result.project.as_ref().expect("project"), "export-1").status,
        JobStatus::Failed
    );
}

#[test]
fn a_queued_job_never_started_fails_after_the_grace_period() {
    let stale = export_job(
        "export-stale",
        JobStatus::Queued,
        None,
        "2026-09-16T11:50:00Z",
    );
    let fresh = export_job(
        "export-fresh",
        JobStatus::Queued,
        None,
        "2026-09-16T11:59:30Z",
    );
    let describer = FakeDescriber::new([
        (workflow_id(&stale), DescribeOutcome::NotFound),
        (workflow_id(&fresh), DescribeOutcome::NotFound),
    ]);
    let (_temp, project_dir) = project_with_jobs(vec![stale, fresh]);

    let result = reconcile(&project_dir, &describer);

    assert_eq!(result.failed_job_ids, vec!["export-stale".to_string()]);
    let project = load_split_project(&project_dir).expect("load");
    assert_eq!(
        job_in(&project, "export-stale").failure_reason.as_deref(),
        Some("The workflow never started.")
    );
    assert_eq!(job_in(&project, "export-fresh").status, JobStatus::Queued);
}

#[test]
fn a_running_workflow_leaves_the_job_untouched() {
    let job = export_job("export-1", JobStatus::Running, Some("run-1"), NOW);
    let describer = FakeDescriber::new([(
        workflow_id(&job),
        DescribeOutcome::Observed(WorkflowObservation::Running),
    )]);
    let (_temp, project_dir) = project_with_jobs(vec![job]);
    let before = load_split_project(&project_dir).expect("load");

    let result = reconcile(&project_dir, &describer);

    assert!(result.service_reachable);
    assert!(result.failed_job_ids.is_empty());
    let after = load_split_project(&project_dir).expect("load");
    assert_eq!(after.content_revision, before.content_revision);
    assert_eq!(job_in(&after, "export-1").status, JobStatus::Running);
}

#[test]
fn an_unreachable_service_writes_nothing_and_reports_unreachable() {
    let terminated = export_job("export-1", JobStatus::Running, Some("run-1"), NOW);
    let unreachable = export_job("export-2", JobStatus::Running, Some("run-2"), NOW);
    let describer = FakeDescriber::new([
        (
            workflow_id(&terminated),
            DescribeOutcome::Observed(WorkflowObservation::Terminated),
        ),
        (
            workflow_id(&unreachable),
            DescribeOutcome::Unreachable("connection refused".to_string()),
        ),
    ]);
    let (_temp, project_dir) = project_with_jobs(vec![terminated, unreachable]);
    let before = load_split_project(&project_dir).expect("load");

    let result = reconcile(&project_dir, &describer);

    assert_eq!(
        result,
        TemporalJobReconciliationResult {
            project: None,
            failed_job_ids: Vec::new(),
            service_reachable: false,
            detail: Some("Workflow service unreachable".to_string()),
        }
    );
    let after = load_split_project(&project_dir).expect("load");
    assert_eq!(after.content_revision, before.content_revision);
    assert_eq!(after.jobs, before.jobs);
}

#[test]
fn a_missing_namespace_writes_nothing_and_reports_it() {
    let terminated = export_job("export-1", JobStatus::Running, Some("run-1"), NOW);
    let queued = export_job("export-2", JobStatus::Queued, None, "2026-09-16T11:00:00Z");
    let describer = FakeDescriber::new([
        (
            workflow_id(&terminated),
            DescribeOutcome::Observed(WorkflowObservation::Terminated),
        ),
        (
            workflow_id(&queued),
            DescribeOutcome::NamespaceNotFound("video-creater".to_string()),
        ),
    ]);
    let (_temp, project_dir) = project_with_jobs(vec![terminated, queued]);
    let before = load_split_project(&project_dir).expect("load");

    let result = reconcile(&project_dir, &describer);

    assert_eq!(
        result,
        TemporalJobReconciliationResult {
            project: None,
            failed_job_ids: Vec::new(),
            service_reachable: false,
            detail: Some("Workflow namespace \"video-creater\" not found".to_string()),
        }
    );
    let after = load_split_project(&project_dir).expect("load");
    assert_eq!(after.content_revision, before.content_revision);
    assert_eq!(after.jobs, before.jobs);
}

#[test]
fn a_job_that_finished_between_describe_and_apply_is_not_failed() {
    let job = export_job("export-1", JobStatus::Running, Some("run-1"), NOW);
    let (_temp, project_dir) = project_with_jobs(vec![job.clone()]);
    let mut describer = FakeDescriber::new([(
        workflow_id(&job),
        DescribeOutcome::Observed(WorkflowObservation::Completed),
    )]);
    let hook_dir = project_dir.clone();
    describer.hook = Some(Box::new(move |_| {
        apply_project_actions_to_split_project(
            &hook_dir,
            vec![ProjectAction::UpdateJobStatus {
                job_id: "export-1".to_string(),
                status: JobStatus::Completed,
                updated_at: NOW.to_string(),
                run_id: None,
            }],
        )
        .expect("the workflow reports its result while being described");
    }));

    let result = reconcile(&project_dir, &describer);

    assert!(result.failed_job_ids.is_empty());
    let persisted = job_in(&load_split_project(&project_dir).expect("load"), "export-1");
    assert_eq!(persisted.status, JobStatus::Completed);
    assert_eq!(persisted.failure_reason, None);
    assert_eq!(describer.described.lock().expect("described").len(), 1);
}

#[test]
fn a_job_that_started_between_describe_and_apply_is_not_failed() {
    let job = export_job("export-1", JobStatus::Queued, None, "2026-09-16T11:59:30Z");
    let (_temp, project_dir) = project_with_jobs(vec![job.clone()]);
    let mut describer = FakeDescriber::new([(workflow_id(&job), DescribeOutcome::NotFound)]);
    let hook_dir = project_dir.clone();
    describer.hook = Some(Box::new(move |_| {
        apply_project_actions_to_split_project(
            &hook_dir,
            vec![ProjectAction::UpdateJobStatus {
                job_id: "export-1".to_string(),
                status: JobStatus::Running,
                updated_at: NOW.to_string(),
                run_id: Some("run-1".to_string()),
            }],
        )
        .expect("the editor records the started run while the workflow is described");
    }));

    let result = reconcile(&project_dir, &describer);

    assert!(result.failed_job_ids.is_empty());
    let persisted = job_in(&load_split_project(&project_dir).expect("load"), "export-1");
    assert_eq!(persisted.status, JobStatus::Running);
    assert_eq!(persisted.failure_reason, None);
}

#[test]
fn a_failure_is_stamped_no_earlier_than_the_jobs_last_update() {
    let later = "2026-09-16T12:00:05Z";
    let job = export_job("export-1", JobStatus::Running, Some("run-1"), later);
    let describer = FakeDescriber::new([(
        workflow_id(&job),
        DescribeOutcome::Observed(WorkflowObservation::Terminated),
    )]);
    let (_temp, project_dir) = project_with_jobs(vec![job]);

    let result = reconcile(&project_dir, &describer);

    assert_eq!(result.failed_job_ids, vec!["export-1".to_string()]);
    let persisted = job_in(&load_split_project(&project_dir).expect("load"), "export-1");
    assert_eq!(persisted.status, JobStatus::Failed);
    assert_eq!(persisted.updated_at, later);
}

fn generation_job_with_asset(asset_status: GeneratedAssetStatus) -> (JobSummary, GeneratedAsset) {
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generate-1",
        JobStatus::Running,
        NOW,
    );
    job.start_request = Some(temporal_workflow_start_request(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generate-1",
        json!({ "assetId": "generated-1" }),
    ));
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.run_id = Some("run-1".to_string());
    }
    let asset = GeneratedAsset {
        schema_version: 1,
        id: "generated-1".to_string(),
        kind: MediaKind::Generated,
        status: asset_status,
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: "a lighthouse at dusk".to_string(),
        model: GenerationModel {
            provider: "fal.ai".to_string(),
            id: "fal-ai/flux/schnell".to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings: GeneratedAssetSettings::default(),
        outputs: Vec::new(),
        created_at: NOW.to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    };
    (job, asset)
}

#[test]
fn a_closed_generation_workflow_fails_its_generated_asset_too() {
    let (job, asset) = generation_job_with_asset(GeneratedAssetStatus::Running);
    let describer = FakeDescriber::new([(
        workflow_id(&job),
        DescribeOutcome::Observed(WorkflowObservation::Terminated),
    )]);
    let temp = tempfile::tempdir().expect("temp project parent");
    let project_dir = temp.path().join("project");
    let mut project = sample_project();
    project.jobs = vec![job];
    project.generated_assets = vec![asset];
    save_split_project(&project_dir, &project).expect("save project");

    let result = reconcile(&project_dir, &describer);

    assert_eq!(result.failed_job_ids, vec!["generate-1".to_string()]);
    let persisted = load_split_project(&project_dir).expect("load");
    assert_eq!(job_in(&persisted, "generate-1").status, JobStatus::Failed);
    let asset = persisted
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-1")
        .expect("generated asset");
    assert_eq!(
        asset.status,
        GeneratedAssetStatus::Failed,
        "a failed generation job must not leave its tile generating"
    );
}
