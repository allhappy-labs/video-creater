//! The Temporal export workflow's step order: RenderMedia encodes into `renders/<jobId>/`, and
//! only WriteExportArtifact, once the export file exists, completes the job. The editor treats a
//! completed export job as "the file is ready", so the render step must not complete it.

#![cfg(feature = "ges-render")]

use super::named_exports::fixture_project;
use serde_json::json;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_destination::ExportOutputRequest;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::model::{JobStatus, JobSummary};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::workflows::{
    temporal_attach_render_report_activity_value, temporal_export_media_start_request_with_output,
    temporal_export_media_workflow_activity_plan_value,
    temporal_export_media_write_artifact_activity_value, temporal_job_summary,
    temporal_render_build_plan_activity_value, temporal_render_media_activity_input_value,
    temporal_render_media_activity_value, TemporalWorkflowKind,
};

const JOB_ID: &str = "export-mp4H264-workflow";
const RUN_ID: &str = "run-export-workflow";
const STARTED_AT: &str = "2026-09-17T10:00:00Z";

fn job(project_dir: &std::path::Path) -> JobSummary {
    let project = load_split_project(project_dir).expect("load project");
    project
        .jobs
        .into_iter()
        .find(|job| job.id == JOB_ID)
        .expect("export job")
}

fn has_artifact(project_dir: &std::path::Path) -> bool {
    load_split_project(project_dir)
        .expect("load project")
        .export_artifacts
        .iter()
        .any(|artifact| artifact.job_id.as_deref() == Some(JOB_ID))
}

#[test]
fn a_temporal_export_job_completes_only_after_its_export_file_is_written() {
    let test = "a_temporal_export_job_completes_only_after_its_export_file_is_written";
    let Some((dir, mut project)) = fixture_project(test, ExportProfile::Mp4H264) else {
        return;
    };
    let project_dir = dir.path();
    let project_dir_text = project_dir.to_str().expect("utf-8 path");
    let options = ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 320, 180)
        .expect("export options");
    let output = ExportOutputRequest {
        file_name: "Workflow export".to_string(),
        directory: None,
    };
    let start = temporal_export_media_start_request_with_output(
        &project.id,
        project_dir_text,
        JOB_ID,
        options,
        &format!("exports/{JOB_ID}.mp4"),
        Some(&output),
    );
    // The editor records the job with its start request and the started run.
    let mut recorded = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        JOB_ID,
        JobStatus::Running,
        STARTED_AT,
    );
    recorded.start_request = Some(start.clone());
    recorded.workflow.as_mut().expect("workflow").run_id = Some(RUN_ID.to_string());
    project.jobs.push(recorded);
    save_split_project(project_dir, &project).expect("record export job");

    let plan = temporal_export_media_workflow_activity_plan_value(
        start.input.clone(),
        STARTED_AT,
        Some(RUN_ID),
    )
    .expect("export plan");
    let build = temporal_render_build_plan_activity_value(plan["buildRenderPlanInput"].clone())
        .expect("BuildRenderPlan");
    let render_input = temporal_render_media_activity_input_value(build, STARTED_AT, Some(RUN_ID))
        .expect("RenderMedia input");
    let render_output = temporal_render_media_activity_value(render_input).expect("RenderMedia");

    // Step 1: the render is encoded, but no export file is recorded yet.
    assert_eq!(job(project_dir).status, JobStatus::Running);
    assert!(!has_artifact(project_dir));

    // What ValidateRenderedMedia passes on (its own profile rules are covered by
    // tests/temporal_workflows.rs); AttachRenderReport runs next and must not finish the job.
    let validation_output = json!({
        "status": "validated",
        "renderReport": render_output["renderReport"].clone(),
        "projectRenderReport": render_output["projectRenderReport"].clone(),
    });
    temporal_attach_render_report_activity_value(validation_output.clone())
        .expect("AttachRenderReport");
    assert_eq!(job(project_dir).status, JobStatus::Running);
    assert!(!has_artifact(project_dir));

    // Step 2: writing the export file completes the job with its artifact.
    let mut write_input = plan["writeExportArtifactBaseInput"].clone();
    write_input
        .as_object_mut()
        .expect("write input object")
        .insert("validationOutput".to_string(), validation_output);
    let write_output = temporal_export_media_write_artifact_activity_value(write_input)
        .expect("WriteExportArtifact");

    let completed = job(project_dir);
    assert_eq!(completed.status, JobStatus::Completed);
    assert_eq!(
        completed
            .workflow
            .and_then(|workflow| workflow.run_id)
            .as_deref(),
        Some(RUN_ID)
    );
    assert!(has_artifact(project_dir));
    let written = write_output["writtenPath"]
        .as_str()
        .map(std::path::PathBuf::from);
    assert!(
        written.as_deref().is_some_and(std::path::Path::is_file),
        "{write_output}"
    );
}
