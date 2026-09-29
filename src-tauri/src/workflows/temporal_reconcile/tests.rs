use super::*;
use crate::project::fixtures::sample_project;
use crate::workflows::{temporal_job_summary, temporal_workflow_start_request};
use serde_json::json;

const NOW: &str = "2026-09-16T12:00:00Z";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(NOW)
        .expect("now")
        .with_timezone(&Utc)
}

fn job(
    kind: TemporalWorkflowKind,
    id: &str,
    status: JobStatus,
    run_id: Option<&str>,
    updated_at: &str,
) -> JobSummary {
    let mut job = temporal_job_summary(kind, "project-1", id, status, updated_at);
    job.start_request = Some(temporal_workflow_start_request(
        kind,
        "project-1",
        id,
        json!({}),
    ));
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.run_id = run_id.map(str::to_string);
    }
    job
}

fn export_job(run_id: Option<&str>, updated_at: &str) -> JobSummary {
    job(
        TemporalWorkflowKind::ExportMedia,
        "export-1",
        JobStatus::Running,
        run_id,
        updated_at,
    )
}

fn plan_one(job: JobSummary, outcome: DescribeOutcome, in_process: bool) -> Option<String> {
    let workflow_id = job
        .start_request
        .as_ref()
        .expect("start request")
        .workflow_id
        .clone();
    let mut project = sample_project();
    project.jobs = vec![job];
    let observations = BTreeMap::from([(workflow_id, outcome)]);
    let mut plan = plan_temporal_job_reconciliation(
        &project,
        &observations,
        now(),
        TEMPORAL_START_GRACE,
        in_process,
    );
    assert!(plan.len() <= 1);
    plan.pop().map(|(_, reason)| reason)
}

fn observed(observation: WorkflowObservation) -> DescribeOutcome {
    DescribeOutcome::Observed(observation)
}

#[test]
fn open_workflows_leave_the_job_alone() {
    for observation in [
        WorkflowObservation::Running,
        WorkflowObservation::Paused,
        WorkflowObservation::ContinuedAsNew,
        WorkflowObservation::Unspecified,
    ] {
        assert_eq!(
            plan_one(export_job(Some("run-1"), NOW), observed(observation), false),
            None,
            "{observation:?}"
        );
    }
}

#[test]
fn closed_workflows_fail_the_job_with_a_plain_reason() {
    for (observation, reason) in [
        (
            WorkflowObservation::Completed,
            "The workflow finished without reporting this task's result.",
        ),
        (WorkflowObservation::Failed, "The workflow failed."),
        (WorkflowObservation::Canceled, "The workflow was cancelled."),
        (WorkflowObservation::Terminated, "The workflow was stopped."),
        (WorkflowObservation::TimedOut, "The workflow timed out."),
    ] {
        assert_eq!(
            plan_one(export_job(Some("run-1"), NOW), observed(observation), false).as_deref(),
            Some(reason),
            "{observation:?}"
        );
    }
}

#[test]
fn a_missing_workflow_with_a_run_id_fails_the_job() {
    assert_eq!(
        plan_one(
            export_job(Some("run-1"), NOW),
            DescribeOutcome::NotFound,
            false
        )
        .as_deref(),
        Some("The workflow service has no record of this task.")
    );
}

#[test]
fn a_job_without_a_run_id_fails_only_after_the_grace_period() {
    assert_eq!(
        plan_one(
            export_job(None, "2026-09-16T11:50:00Z"),
            DescribeOutcome::NotFound,
            false
        )
        .as_deref(),
        Some(WORKFLOW_NEVER_STARTED)
    );
    assert_eq!(
        plan_one(
            export_job(None, "2026-09-16T11:59:00Z"),
            DescribeOutcome::NotFound,
            false
        ),
        None
    );
    // Exactly at the grace boundary the start may still record its run id.
    assert_eq!(
        plan_one(
            export_job(None, "2026-09-16T11:58:00Z"),
            DescribeOutcome::NotFound,
            false
        ),
        None
    );
    assert_eq!(
        plan_one(
            export_job(None, "2026-09-16T11:57:59Z"),
            DescribeOutcome::NotFound,
            false
        )
        .as_deref(),
        Some(WORKFLOW_NEVER_STARTED)
    );
}

#[test]
fn a_job_without_a_run_id_and_an_existing_workflow_is_left_alone() {
    for outcome in [
        observed(WorkflowObservation::Running),
        observed(WorkflowObservation::Terminated),
    ] {
        assert_eq!(
            plan_one(export_job(None, "2026-09-16T11:00:00Z"), outcome, false),
            None
        );
    }
}

#[test]
fn an_unreachable_service_changes_nothing() {
    for job in [
        export_job(Some("run-1"), NOW),
        export_job(None, "2026-09-16T11:00:00Z"),
    ] {
        assert_eq!(
            plan_one(
                job,
                DescribeOutcome::Unreachable("connection refused".to_string()),
                false
            ),
            None
        );
    }
}

#[test]
fn a_missing_namespace_changes_nothing() {
    for job in [
        export_job(Some("run-1"), NOW),
        export_job(None, "2026-09-16T11:00:00Z"),
    ] {
        assert_eq!(
            plan_one(
                job,
                DescribeOutcome::NamespaceNotFound("video-creater".to_string()),
                false
            ),
            None
        );
    }
}

#[test]
fn a_missing_namespace_is_reported_with_its_name() {
    assert_eq!(
        serde_json::to_value(namespace_not_found_result("video-creater")).expect("json"),
        json!({
            "project": null,
            "failedJobIds": [],
            "serviceReachable": false,
            "detail": "Workflow namespace \"video-creater\" not found"
        })
    );
}

#[test]
fn local_runs_are_not_reconciled() {
    for run_id in [
        "in-process/video-creater/project-1/export-media/export-1",
        "render-attempt/0b1c",
        "mock-run-export-1",
    ] {
        let job = export_job(Some(run_id), NOW);
        assert_eq!(
            plan_one(job, observed(WorkflowObservation::Terminated), false),
            None,
            "{run_id}"
        );
    }
}

#[test]
fn finished_jobs_capture_jobs_and_jobs_without_a_start_request_are_not_candidates() {
    let completed = job(
        TemporalWorkflowKind::ExportMedia,
        "export-1",
        JobStatus::Completed,
        Some("run-1"),
        NOW,
    );
    assert_eq!(
        plan_one(completed, observed(WorkflowObservation::Terminated), false),
        None
    );

    let mut capture = export_job(Some("run-1"), NOW);
    capture.kind = "captureCanonicalPreviewFrame".to_string();
    assert_eq!(
        plan_one(capture, observed(WorkflowObservation::Terminated), false),
        None
    );

    let mut project = sample_project();
    let mut no_start = export_job(Some("run-1"), NOW);
    no_start.start_request = None;
    project.jobs = vec![no_start];
    assert!(temporal_reconciliation_workflow_ids(&project, false).is_empty());
}

#[test]
fn in_process_generation_without_a_run_id_is_left_to_project_open_recovery() {
    let generation = || {
        job(
            TemporalWorkflowKind::GenerateMedia,
            "generate-1",
            JobStatus::Queued,
            None,
            "2026-09-16T11:00:00Z",
        )
    };
    assert_eq!(
        plan_one(generation(), DescribeOutcome::NotFound, true),
        None
    );
    assert_eq!(
        plan_one(generation(), DescribeOutcome::NotFound, false).as_deref(),
        Some(WORKFLOW_NEVER_STARTED)
    );
}

#[test]
fn candidate_workflow_ids_are_distinct() {
    let mut project = sample_project();
    let first = export_job(Some("run-1"), NOW);
    let mut duplicate = first.clone();
    duplicate.id = "export-1-copy".to_string();
    project.jobs = vec![first, duplicate];

    assert_eq!(
        temporal_reconciliation_workflow_ids(&project, false).len(),
        1
    );
}

#[test]
fn reconciliation_serializes_camel_case() {
    assert_eq!(
        serde_json::to_value(unreachable_result()).expect("json"),
        json!({
            "project": null,
            "failedJobIds": [],
            "serviceReachable": false,
            "detail": "Workflow service unreachable"
        })
    );
}

#[cfg(feature = "temporal-worker")]
mod not_found_status {
    use super::*;
    use crate::workflows::temporal_reconcile::client::not_found_outcome;
    use prost::Message;
    use temporalio_common::protos::google::rpc::Status as RpcStatus;
    use temporalio_common::protos::temporal::api::errordetails::v1::{
        NamespaceNotFoundFailure, NotFoundFailure,
    };
    use temporalio_common::protos::utilities::pack_any;

    fn details(type_url: &str, detail: &impl Message) -> Vec<u8> {
        RpcStatus {
            code: 5,
            message: String::new(),
            details: vec![pack_any(type_url.to_string(), detail).expect("pack detail")],
        }
        .encode_to_vec()
    }

    #[test]
    fn a_namespace_not_found_failure_is_a_missing_namespace() {
        let failure = NamespaceNotFoundFailure {
            namespace: "bogus".to_string(),
        };
        assert_eq!(
            not_found_outcome(
                &details(
                    "type.googleapis.com/temporal.api.errordetails.v1.NamespaceNotFoundFailure",
                    &failure
                ),
                "Namespace bogus is not found."
            ),
            DescribeOutcome::NamespaceNotFound("bogus".to_string())
        );
    }

    #[test]
    fn a_not_found_failure_is_a_missing_workflow() {
        // Encoded like a namespace failure, a workflow failure's first string field would read as
        // a namespace name, so the type URL decides.
        let failure = NotFoundFailure {
            current_cluster: "active".to_string(),
            active_cluster: "active".to_string(),
        };
        assert_eq!(
            not_found_outcome(
                &details(
                    "type.googleapis.com/temporal.api.errordetails.v1.NotFoundFailure",
                    &failure
                ),
                "workflow not found for ID: export-1"
            ),
            DescribeOutcome::NotFound
        );
    }

    #[test]
    fn without_details_the_message_decides() {
        assert_eq!(
            not_found_outcome(&[], "Namespace bogus is not found."),
            DescribeOutcome::NamespaceNotFound("bogus".to_string())
        );
        assert_eq!(
            not_found_outcome(&[], "workflow not found for ID: export-1"),
            DescribeOutcome::NotFound
        );
        assert_eq!(
            not_found_outcome(b"not a status", "sql: no rows in result set"),
            DescribeOutcome::NotFound
        );
    }
}
