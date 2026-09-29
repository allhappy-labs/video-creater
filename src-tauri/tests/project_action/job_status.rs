//! `updateJobStatus` ordering for Temporal runs: the editor records a run's "started" status after
//! `startTemporalWorkflow` returns, and that write can land after the run already finished.

use video_creater_lib::project::action::{apply_project_action, ProjectAction};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;
use video_creater_lib::workflows::{temporal_job_summary, TemporalWorkflowKind};

const FINISHED_AT: &str = "2026-09-17T10:05:00Z";

fn project_with_job(status: JobStatus, run_id: &str) -> VideoProject {
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        "export-1",
        status,
        FINISHED_AT,
    );
    job.workflow.as_mut().expect("workflow").run_id = Some(run_id.to_string());
    project.jobs.push(job);
    project
}

fn update(status: JobStatus, run_id: &str) -> ProjectAction {
    ProjectAction::UpdateJobStatus {
        job_id: "export-1".to_string(),
        status,
        updated_at: "2026-09-17T10:06:00Z".to_string(),
        run_id: Some(run_id.to_string()),
    }
}

#[test]
fn a_late_started_write_leaves_the_finished_run_untouched() {
    for status in [
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ] {
        let mut project = project_with_job(status.clone(), "run-1");
        if status == JobStatus::Failed {
            project.jobs.last_mut().expect("job").failure_reason =
                Some("The workflow failed.".to_string());
        }
        let before = project.jobs.clone();

        apply_project_action(&mut project, update(JobStatus::Running, "run-1"))
            .unwrap_or_else(|error| panic!("{status:?}: late start is not an error: {error}"));

        assert_eq!(project.jobs, before, "{status:?}");
    }
}

#[test]
fn a_new_run_restarts_a_finished_job_and_a_run_still_records_its_result() {
    let mut project = project_with_job(JobStatus::Failed, "run-1");
    apply_project_action(&mut project, update(JobStatus::Running, "run-2"))
        .expect("a new run restarts the job");
    let job = project.jobs.last().expect("job");
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(
        job.workflow.as_ref().and_then(|w| w.run_id.as_deref()),
        Some("run-2")
    );

    let mut project = project_with_job(JobStatus::Running, "run-1");
    apply_project_action(&mut project, update(JobStatus::Completed, "run-1"))
        .expect("the run completes");
    assert_eq!(
        project.jobs.last().expect("job").status,
        JobStatus::Completed
    );
}
