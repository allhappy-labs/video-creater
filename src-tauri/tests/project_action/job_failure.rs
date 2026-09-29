//! `recordJobFailure`: a bookkeeping action that fails an unfinished job with a
//! plain reason, and the reason's lifecycle across later status updates.

use serde_json::json;
use video_creater_lib::project::action::{apply_project_action, ProjectAction, ProjectActionError};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;

const REASON: &str = "The workflow never started.";

fn job(id: &str, status: JobStatus) -> JobSummary {
    JobSummary {
        id: id.to_string(),
        kind: "export_media".to_string(),
        status,
        updated_at: "2026-09-16T10:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    }
}

fn project_with(job: JobSummary) -> VideoProject {
    let mut project = sample_project();
    project.jobs.push(job);
    project
}

fn record_failure(job_id: &str, reason: &str) -> ProjectAction {
    ProjectAction::RecordJobFailure {
        job_id: job_id.to_string(),
        reason: reason.to_string(),
        updated_at: "2026-09-16T10:05:00Z".to_string(),
        run_id: None,
    }
}

fn find_job<'a>(project: &'a VideoProject, job_id: &str) -> &'a JobSummary {
    project
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .expect("job exists")
}

#[test]
fn record_job_failure_fails_a_queued_job_with_its_reason() {
    for status in [
        JobStatus::Queued,
        JobStatus::Running,
        JobStatus::Progress,
        JobStatus::Blocked,
    ] {
        let mut project = project_with(job("job-stale", status));

        apply_project_action(&mut project, record_failure("job-stale", REASON))
            .expect("record job failure");

        let job = find_job(&project, "job-stale");
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.failure_reason.as_deref(), Some(REASON));
        assert_eq!(job.updated_at, "2026-09-16T10:05:00Z");
    }
}

#[test]
fn record_job_failure_leaves_a_finished_job_untouched() {
    for status in [
        JobStatus::Completed,
        JobStatus::Cancelled,
        JobStatus::Failed,
    ] {
        let mut project = project_with(job("job-done", status));
        let before = project.clone();

        apply_project_action(&mut project, record_failure("job-done", REASON))
            .expect("a finished job is not an error");

        assert_eq!(project.jobs, before.jobs);
    }
}

#[test]
fn record_job_failure_rejects_a_blank_reason_and_unknown_job() {
    let mut project = project_with(job("job-stale", JobStatus::Queued));

    let blank = apply_project_action(&mut project, record_failure("job-stale", "  "))
        .expect_err("blank reason is rejected");
    assert_eq!(blank, ProjectActionError::EmptyJobFailureReason);

    let unknown = apply_project_action(&mut project, record_failure("job-missing", REASON))
        .expect_err("unknown job is rejected");
    assert_eq!(
        unknown,
        ProjectActionError::JobNotFound("job-missing".to_string())
    );
    assert_eq!(find_job(&project, "job-stale").status, JobStatus::Queued);
}

#[test]
fn update_job_status_to_running_clears_the_failure_reason() {
    let mut project = project_with(job("job-retry", JobStatus::Queued));
    apply_project_action(&mut project, record_failure("job-retry", REASON))
        .expect("record job failure");

    apply_project_action(
        &mut project,
        ProjectAction::UpdateJobStatus {
            job_id: "job-retry".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-09-16T10:06:00Z".to_string(),
            run_id: None,
        },
    )
    .expect("retry the job");

    let job = find_job(&project, "job-retry");
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(job.failure_reason, None);
}

#[test]
fn record_job_failure_round_trips_as_camel_case_json() {
    let wire = json!({
        "type": "recordJobFailure",
        "jobId": "job-stale",
        "reason": REASON,
        "updatedAt": "2026-09-16T10:05:00Z",
        "runId": null
    });
    let action: ProjectAction = serde_json::from_value(wire).expect("decode action");
    assert_eq!(action, record_failure("job-stale", REASON));
    assert_eq!(
        serde_json::to_value(&action).expect("encode action"),
        json!({
            "type": "recordJobFailure",
            "jobId": "job-stale",
            "reason": REASON,
            "updatedAt": "2026-09-16T10:05:00Z"
        })
    );

    let mut failed = job("job-stale", JobStatus::Failed);
    failed.failure_reason = Some(REASON.to_string());
    let encoded = serde_json::to_value(&failed).expect("encode job");
    assert_eq!(encoded["failureReason"], json!(REASON));
    let decoded: JobSummary = serde_json::from_value(encoded).expect("decode job");
    assert_eq!(decoded, failed);

    let plain = serde_json::to_value(job("job-plain", JobStatus::Queued)).expect("encode job");
    assert!(plain.get("failureReason").is_none());
}
