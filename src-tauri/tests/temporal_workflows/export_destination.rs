//! Temporal exports with a chosen folder, file name, frame rate and Master.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use video_creater_lib::edit::render_plan::{ExportEncodeTier, RenderQuality};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::export_destination::ExportOutputRequest;
use video_creater_lib::project::export_options::{ExportRenderOptions, JobExportSettings};
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{JobStatus, ProjectExportArtifactKind, VideoProject};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project, save_split_project,
};
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::workflows::{
    temporal_export_media_start_request_with_options,
    temporal_export_media_start_request_with_output,
    temporal_export_media_workflow_activity_plan_value,
    temporal_export_media_write_artifact_activity_value, temporal_job_summary,
    temporal_render_options_from_input, TemporalWorkflowInputError, TemporalWorkflowKind,
};

fn master_options(profile: ExportProfile, fps: Option<f64>) -> ExportRenderOptions {
    ExportRenderOptions::new(profile, RenderQuality::Final, 1280, 720)
        .and_then(|options| options.with_fps(fps))
        .and_then(|options| options.with_encode_tier(ExportEncodeTier::Master))
        .expect("master options")
}

fn chosen_output(directory: Option<&str>) -> ExportOutputRequest {
    ExportOutputRequest {
        file_name: "Edison intro".to_string(),
        directory: directory.map(str::to_string),
    }
}

#[test]
fn temporal_export_media_start_request_carries_fps_encode_tier_and_destination() {
    let output = chosen_output(Some("/tmp/vc-exports"));
    let request = temporal_export_media_start_request_with_output(
        "project-1",
        "/tmp/video-creater-project",
        "export-mp4H264-1",
        master_options(ExportProfile::Mp4H264, Some(25.0)),
        "exports/Edison intro.mp4",
        Some(&output),
    );

    assert_eq!(request.input["fps"], json!(25.0));
    assert_eq!(request.input["encodeTier"], json!("master"));
    assert_eq!(
        request.input["destination"],
        json!({ "fileName": "Edison intro", "directory": "/tmp/vc-exports" })
    );

    let standard = temporal_export_media_start_request_with_options(
        "project-1",
        "/tmp/video-creater-project",
        "export-mp4H264-2",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 1280, 720)
            .expect("standard options"),
        "exports/Edison intro.mp4",
    );
    for key in ["fps", "encodeTier", "destination"] {
        assert!(standard.input.get(key).is_none(), "{key}");
    }
}

#[test]
fn temporal_render_options_from_input_reads_fps_and_encode_tier() {
    let options = temporal_render_options_from_input(&json!({
        "profile": "webm",
        "quality": "final",
        "width": 1280,
        "height": 720,
        "fps": 29.97,
        "encodeTier": "master"
    }))
    .expect("options with fps and Master");
    assert_eq!(options.fps, Some(29.97));
    assert_eq!(options.encode_tier, ExportEncodeTier::Master);

    let error = temporal_render_options_from_input(&json!({
        "profile": "mp4H264",
        "quality": "draft",
        "width": 1280,
        "height": 720,
        "encodeTier": "master"
    }))
    .expect_err("Master with Draft is refused");
    assert!(
        matches!(
            &error,
            TemporalWorkflowInputError::Render(message)
                if message == "Master quality needs Final quality."
        ),
        "{error:?}"
    );
}

#[test]
fn temporal_export_media_workflow_activity_plan_passes_export_settings_through() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let output = chosen_output(Some("/tmp/vc-exports"));
    let request = temporal_export_media_start_request_with_output(
        "project-1",
        "/tmp/video-creater-project",
        "export-webm-1",
        master_options(ExportProfile::Webm, Some(25.0)),
        "exports/Edison intro.webm",
        Some(&output),
    );

    let plan = temporal_export_media_workflow_activity_plan_value(
        request.input,
        "2026-09-17T10:00:00Z",
        Some("run-1"),
    )
    .expect("export plan");

    for key in [
        "buildRenderPlanInput",
        "renderMediaInput",
        "validateRenderedMediaInput",
        "writeExportArtifactBaseInput",
    ] {
        assert_eq!(plan[key]["fps"], json!(25.0), "{key}");
        assert_eq!(plan[key]["encodeTier"], json!("master"), "{key}");
    }
    assert_eq!(
        plan["writeExportArtifactBaseInput"]["destination"],
        json!({ "fileName": "Edison intro", "directory": "/tmp/vc-exports" })
    );
    assert!(plan["buildRenderPlanInput"].get("destination").is_none());

    let mut escaping = temporal_export_media_start_request_with_output(
        "project-1",
        "/tmp/video-creater-project",
        "export-webm-2",
        master_options(ExportProfile::Webm, None),
        "exports/Edison intro.webm",
        Some(&output),
    )
    .input;
    escaping["outputPath"] = json!("renders/escape.webm");
    assert!(matches!(
        temporal_export_media_workflow_activity_plan_value(escaping, "2026-09-17T10:00:00Z", None),
        Err(TemporalWorkflowInputError::MismatchedInputField(field)) if field == "outputPath"
    ));
}

/// A saved project whose export job carries `output` in its export settings,
/// and a fabricated validated WebM render for it.
fn project_with_validated_render(
    project_dir: &Path,
    job_id: &str,
    output: &ExportOutputRequest,
) -> (VideoProject, String) {
    let project = sample_project();
    save_split_project(project_dir, &project).expect("save split project");
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::ExportMedia,
        &project.id,
        job_id,
        JobStatus::Running,
        "2026-09-17T10:00:00Z",
    );
    job.export_settings = Some(JobExportSettings {
        options: ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Final, 1280, 720)
            .expect("WebM options"),
        output: Some(output.clone()),
    });
    apply_project_actions_to_split_project(
        project_dir,
        vec![ProjectAction::RecordJob { job: Box::new(job) }],
    )
    .expect("record export job");
    let source_relative = format!("renders/{job_id}/output.webm");
    let source = project_dir.join(&source_relative);
    fs::create_dir_all(source.parent().expect("render dir")).expect("render dir");
    fs::write(&source, b"validated webm bytes").expect("validated render");
    (project, source_relative)
}

fn write_input(
    project: &VideoProject,
    project_dir: &Path,
    job_id: &str,
    source_relative: &str,
    output: &ExportOutputRequest,
) -> Value {
    let request = temporal_export_media_start_request_with_output(
        &project.id,
        project_dir.to_str().expect("project path"),
        job_id,
        ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Final, 1280, 720)
            .expect("WebM options"),
        "exports/Edison intro.webm",
        Some(output),
    );
    json!({
        "projectId": project.id,
        "projectDir": project_dir,
        "jobId": job_id,
        "profile": "webm",
        "quality": "final",
        "width": 1280,
        "height": 720,
        "outputPath": "exports/Edison intro.webm",
        "overwrite": true,
        "createdAt": "2026-09-17T10:01:00Z",
        "runId": format!("{job_id}-run"),
        "validation": request.input["validation"].clone(),
        "destination": request.input["destination"].clone(),
        "validationOutput": {
            "status": "validated",
            "renderReport": { "summary": { "outputPath": source_relative } },
            "projectRenderReport": { "id": job_id }
        }
    })
}

#[test]
fn temporal_export_media_writer_saves_to_the_chosen_folder_without_overwriting() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let chosen = tempfile::tempdir().expect("chosen folder");
    fs::write(chosen.path().join("Edison intro.webm"), b"keep").expect("existing export");
    let output = chosen_output(Some(chosen.path().to_str().expect("chosen path")));
    let (project, source_relative) =
        project_with_validated_render(project_dir.path(), "export-webm-chosen", &output);

    let write_output = temporal_export_media_write_artifact_activity_value(write_input(
        &project,
        project_dir.path(),
        "export-webm-chosen",
        &source_relative,
        &output,
    ))
    .expect("write to the chosen folder");

    let expected = chosen.path().join("Edison intro (2).webm");
    let expected_text = expected.display().to_string();
    assert_eq!(write_output["artifactPath"], json!(expected_text));
    assert_eq!(write_output["writtenPath"], json!(expected_text));
    assert_eq!(write_output["materialization"]["byteIdentical"], true);
    assert_eq!(
        fs::read(&expected).expect("export"),
        b"validated webm bytes"
    );
    assert_eq!(
        fs::read(chosen.path().join("Edison intro.webm")).expect("kept"),
        b"keep"
    );
    let reloaded = load_split_project(project_dir.path()).expect("reload");
    let artifact = reloaded
        .export_artifacts
        .iter()
        .find(|artifact| artifact.id == "export-webm-chosen")
        .expect("recorded artifact");
    assert_eq!(artifact.kind, ProjectExportArtifactKind::Webm);
    assert_eq!(artifact.path, expected_text);
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "export-webm-chosen")
        .expect("job");
    assert_eq!(job.status, JobStatus::Completed);
    assert_eq!(
        job.export_settings
            .as_ref()
            .and_then(|settings| settings.output.as_ref()),
        Some(&output)
    );
}

#[test]
fn temporal_export_media_writer_defaults_destination_to_project_exports() {
    let project_dir = tempfile::tempdir().expect("project dir");
    let output = chosen_output(None);
    let (project, source_relative) =
        project_with_validated_render(project_dir.path(), "export-webm-default", &output);

    let write_output = temporal_export_media_write_artifact_activity_value(write_input(
        &project,
        project_dir.path(),
        "export-webm-default",
        &source_relative,
        &output,
    ))
    .expect("write to the project exports folder");

    assert_eq!(write_output["artifactPath"], "exports/Edison intro.webm");
    assert_eq!(
        fs::read(project_dir.path().join("exports/Edison intro.webm")).expect("export"),
        b"validated webm bytes"
    );
    let reloaded = load_split_project(project_dir.path()).expect("reload");
    assert_eq!(
        reloaded.export_artifacts[0].path,
        "exports/Edison intro.webm"
    );
}
