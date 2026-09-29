//! Export settings recorded on jobs, and export artifacts saved outside the
//! project folder.

use video_creater_lib::edit::render_plan::{ExportEncodeTier, RenderQuality};
use video_creater_lib::project::action::{apply_project_action, ProjectAction, ProjectActionError};
use video_creater_lib::project::export_destination::ExportOutputRequest;
use video_creater_lib::project::export_options::{ExportRenderOptions, JobExportSettings};
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;

fn options(
    profile: ExportProfile,
    quality: RenderQuality,
    fps: Option<f64>,
    encode_tier: ExportEncodeTier,
) -> ExportRenderOptions {
    let mut options = ExportRenderOptions::new(profile, quality, 1920, 1080).expect("options");
    // Bypass the builders: stored settings arrive through serde, unchecked.
    options.fps = fps;
    options.encode_tier = encode_tier;
    options
}

fn settings(
    options: ExportRenderOptions,
    file_name: &str,
    directory: Option<&str>,
) -> JobExportSettings {
    JobExportSettings {
        options,
        output: Some(ExportOutputRequest {
            file_name: file_name.to_string(),
            directory: directory.map(str::to_string),
        }),
    }
}

pub(crate) fn export_job(id: &str, export_settings: Option<JobExportSettings>) -> JobSummary {
    JobSummary {
        id: id.to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Running,
        updated_at: "2026-09-17T10:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings,
    }
}

fn master_settings() -> JobExportSettings {
    settings(
        options(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            Some(25.0),
            ExportEncodeTier::Master,
        ),
        "Edison intro",
        None,
    )
}

#[test]
fn record_job_accepts_valid_export_settings() {
    let mut project = sample_project();
    let job = export_job("export-mp4H264-1", Some(master_settings()));

    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(job.clone()),
        },
    )
    .expect("valid export settings are recorded");

    assert_eq!(project.jobs.last(), Some(&job));
    let value = serde_json::to_value(&job).expect("serialize job");
    let settings = &value["exportSettings"];
    assert_eq!(settings["profile"], "mp4H264");
    assert_eq!(settings["quality"], "final");
    assert_eq!(settings["encodeTier"], "master");
    assert_eq!(settings["fps"], 25.0);
    assert_eq!(settings["output"]["fileName"], "Edison intro");
    let round_trip: JobSummary = serde_json::from_value(value).expect("deserialize job");
    assert_eq!(round_trip, job);
}

#[test]
fn record_job_rejects_invalid_export_settings() {
    let final_mp4 = |fps, tier| options(ExportProfile::Mp4H264, RenderQuality::Final, fps, tier);
    let cases = [
        (
            settings(final_mp4(None, ExportEncodeTier::Standard), "a/b", None),
            "Export names can't contain slashes.",
        ),
        (
            settings(final_mp4(Some(0.0), ExportEncodeTier::Standard), "a", None),
            "Choose a frame rate between 1 and 120 fps.",
        ),
        (
            settings(
                options(
                    ExportProfile::Mp4H264,
                    RenderQuality::Draft,
                    None,
                    ExportEncodeTier::Master,
                ),
                "a",
                None,
            ),
            "Master quality needs Final quality.",
        ),
        (
            settings(
                options(
                    ExportProfile::ProResMov,
                    RenderQuality::Final,
                    None,
                    ExportEncodeTier::Master,
                ),
                "a",
                None,
            ),
            "Master quality is for MP4 and WebM exports",
        ),
        (
            settings(
                final_mp4(None, ExportEncodeTier::Standard),
                "a",
                Some("exports"),
            ),
            "The export folder must be a full path.",
        ),
    ];
    for (export_settings, rule) in cases {
        let mut project = sample_project();
        let before = project.clone();
        let error = apply_project_action(
            &mut project,
            ProjectAction::RecordJob {
                job: Box::new(export_job("export-mp4H264-1", Some(export_settings))),
            },
        )
        .expect_err("invalid export settings are refused");

        assert!(
            matches!(error, ProjectActionError::InvalidJobExportSettings(_)),
            "{error:?}"
        );
        assert!(
            error.to_string().contains(rule),
            "{error} should name {rule}"
        );
        assert_eq!(project, before);
    }
}

#[test]
fn job_summary_without_export_settings_serializes_unchanged() {
    let value = serde_json::to_value(export_job("job-1", None)).expect("serialize job");
    assert!(value.get("exportSettings").is_none());
}

fn mp4_artifact(path: &str) -> ProjectExportArtifact {
    ProjectExportArtifact {
        schema_version: 1,
        id: "export-mp4H264-1".to_string(),
        kind: ProjectExportArtifactKind::Mp4,
        format: "mp4H264".to_string(),
        path: path.to_string(),
        mime_type: "video/mp4".to_string(),
        job_id: Some("export-mp4H264-1".to_string()),
        created_at: "2026-09-17T10:05:00Z".to_string(),
    }
}

fn chosen_folder_settings(directory: &str) -> JobExportSettings {
    settings(
        options(
            ExportProfile::Mp4H264,
            RenderQuality::Final,
            None,
            ExportEncodeTier::Standard,
        ),
        "Edison intro",
        Some(directory),
    )
}

fn project_with_job(job: Option<JobSummary>) -> VideoProject {
    let mut project = sample_project();
    project.jobs.extend(job);
    project
}

#[test]
fn record_export_artifact_accepts_an_absolute_path_in_the_jobs_chosen_folder() {
    let mut project = project_with_job(Some(export_job(
        "export-mp4H264-1",
        Some(chosen_folder_settings("/tmp/vc-exports")),
    )));
    let artifact = mp4_artifact("/tmp/vc-exports/Edison intro (2).mp4");

    apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact {
            artifact: artifact.clone(),
        },
    )
    .expect("an artifact in the job's chosen folder is recorded");

    assert_eq!(project.export_artifacts, vec![artifact]);
}

#[test]
fn record_export_artifact_rejects_an_absolute_path_the_job_did_not_choose() {
    let cases = [
        ("no job", None, "/tmp/vc-exports/Edison intro.mp4"),
        (
            "job without settings",
            Some(export_job("export-mp4H264-1", None)),
            "/tmp/vc-exports/Edison intro.mp4",
        ),
        (
            "different folder",
            Some(export_job(
                "export-mp4H264-1",
                Some(chosen_folder_settings("/tmp/vc-exports")),
            )),
            "/home/me/Edison intro.mp4",
        ),
        (
            "nested folder",
            Some(export_job(
                "export-mp4H264-1",
                Some(chosen_folder_settings("/tmp/vc-exports")),
            )),
            "/tmp/vc-exports/nested/Edison intro.mp4",
        ),
    ];
    for (case, job, path) in cases {
        let mut project = project_with_job(job);
        let before = project.clone();

        let error = apply_project_action(
            &mut project,
            ProjectAction::RecordExportArtifact {
                artifact: mp4_artifact(path),
            },
        )
        .expect_err("an unchosen absolute path is refused");

        assert!(
            error.to_string().contains(
                "export artifact path must stay under exports/ or in the folder its export chose"
            ),
            "{case}: {error}"
        );
        assert_eq!(project, before, "{case}");
    }
}

#[test]
fn record_export_artifact_rejects_absolute_paths_with_parent_components() {
    let mut project = project_with_job(Some(export_job(
        "export-mp4H264-1",
        Some(chosen_folder_settings("/tmp/vc-exports")),
    )));
    let before = project.clone();

    let error = apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact {
            artifact: mp4_artifact("/tmp/vc-exports/../etc/x.mp4"),
        },
    )
    .expect_err("parent components are refused");

    assert!(matches!(
        error,
        ProjectActionError::UnsafeExportArtifactPath(_)
    ));
    assert_eq!(project, before);
}
