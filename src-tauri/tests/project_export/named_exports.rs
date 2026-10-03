//! In-process exports saved under a chosen name and folder, with their export
//! artifact and recorded export settings.

use super::export_job_summary;
#[cfg(feature = "ges-render")]
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
#[cfg(feature = "ges-render")]
use video_creater_lib::edit::render_plan::ExportEncodeTier;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_destination::ExportOutputRequest;
use video_creater_lib::project::export_options::ExportRenderOptions;
#[cfg(feature = "ges-render")]
use video_creater_lib::project::export_profiles::mp4_export_profile_availability_report;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{JobStatus, VideoProject};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::render_pipeline::error::PipelineError;
#[cfg(feature = "ges-render")]
use video_creater_lib::render_pipeline::gstreamer_backend::{
    generate_fixture_source_with_gstreamer, probe_media_with_gstreamer,
};
use video_creater_lib::render_pipeline::project_export::{
    render_media_export_to_split_project_folder, MediaExportRequest, ProjectMediaRenderResult,
};
#[cfg(feature = "ges-render")]
use video_creater_lib::render_runtime::start_render_process_runtime;

const UPDATED_AT: &str = "2026-09-17T10:00:00Z";

fn output(file_name: &str, directory: Option<&Path>) -> ExportOutputRequest {
    ExportOutputRequest {
        file_name: file_name.to_string(),
        directory: directory.map(|path| path.display().to_string()),
    }
}

fn final_options(profile: ExportProfile) -> ExportRenderOptions {
    ExportRenderOptions::new(profile, RenderQuality::Final, 320, 180).expect("export options")
}

fn export(
    project_dir: &Path,
    project: &VideoProject,
    options: ExportRenderOptions,
    job_id: &str,
    output: Option<&ExportOutputRequest>,
) -> Result<ProjectMediaRenderResult, Vec<PipelineError>> {
    render_media_export_to_split_project_folder(MediaExportRequest {
        project_dir,
        project_id: &project.id,
        options,
        job: export_job_summary(job_id, UPDATED_AT),
        updated_at: UPDATED_AT,
        run_id: None,
        range_seconds: None,
        timeline_id: None,
        output,
    })
}

/// The 320x180, 24 fps, 3 s fixture project, or `None` when `profile` can't
/// render here (the caller reports the test as not run).
#[cfg(feature = "ges-render")]
pub(super) fn fixture_project(
    test: &str,
    profile: ExportProfile,
) -> Option<(tempfile::TempDir, VideoProject)> {
    fixture_project_of_length(test, profile, 3.0)
}

/// [`fixture_project`] with a chosen timeline length, for tests that need a render long enough
/// to observe something happening while it encodes.
#[cfg(feature = "ges-render")]
pub(super) fn fixture_project_of_length(
    test: &str,
    profile: ExportProfile,
    timeline_seconds: f64,
) -> Option<(tempfile::TempDir, VideoProject)> {
    start_render_process_runtime().expect("initialize curated render runtime");
    let availability = mp4_export_profile_availability_report()
        .into_iter()
        .find(|candidate| candidate.profile == profile)
        .expect("profile is reported");
    if !availability.available {
        eprintln!(
            "{test} not run: {profile:?} unavailable: {}",
            availability.unavailable_reason.unwrap_or_default()
        );
        return None;
    }
    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    let source_seconds = timeline_seconds + 1.0;
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.mp4"),
        320,
        180,
        24.0,
        source_seconds,
        Duration::from_secs(120),
    )
    .expect("generate fixture source");
    let mut project = sample_project();
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = timeline_seconds;
    project.media[0].duration_seconds = source_seconds;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = timeline_seconds;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties
        .insert("sourceOut".to_string(), json!(timeline_seconds));
    save_split_project(dir.path(), &project).expect("save split project");
    Some((dir, project))
}

#[test]
fn media_export_refuses_a_destination_inside_the_project_before_rendering() {
    let dir = tempfile::tempdir().expect("temp project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    let media = dir.path().join("media");
    fs::create_dir_all(&media).expect("media dir");

    let errors = export(
        dir.path(),
        &project,
        final_options(ExportProfile::Mp4H264),
        "export-mp4H264-inside",
        Some(&output("x", Some(&media))),
    )
    .expect_err("a destination inside the project is refused");

    assert_eq!(errors[0].path, "export.output");
    assert_eq!(
        errors[0].message,
        "Choose a folder outside the project, or the project's exports folder."
    );
    let reloaded = load_split_project(dir.path()).expect("reload");
    assert!(reloaded
        .jobs
        .iter()
        .all(|job| job.id != "export-mp4H264-inside"));
    assert!(!dir.path().join("renders/export-mp4H264-inside").exists());
}

#[cfg(feature = "ges-render")]
#[test]
fn in_process_export_records_a_named_artifact_in_the_project_exports_folder() {
    let test = "in_process_export_records_a_named_artifact_in_the_project_exports_folder";
    let Some((dir, project)) = fixture_project(test, ExportProfile::Mp4H264) else {
        return;
    };
    let request = output("Edison intro", None);

    let result = export(
        dir.path(),
        &project,
        final_options(ExportProfile::Mp4H264),
        "export-mp4H264-named",
        Some(&request),
    )
    .expect("named export renders");

    let artifact = result.export_artifact.clone().expect("export artifact");
    assert_eq!(artifact.path, "exports/Edison intro.mp4");
    assert_eq!(artifact.job_id.as_deref(), Some("export-mp4H264-named"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::metadata(dir.path().join(&result.output_path))
                .expect("render output")
                .ino(),
            fs::metadata(dir.path().join(&artifact.path))
                .expect("export")
                .ino()
        );
    }
    let reloaded = load_split_project(dir.path()).expect("reload");
    assert_eq!(reloaded.export_artifacts, vec![artifact.clone()]);
    let job = reloaded
        .jobs
        .iter()
        .find(|job| job.id == "export-mp4H264-named")
        .expect("export job");
    assert_eq!(job.status, JobStatus::Completed);
    let settings = job.export_settings.as_ref().expect("export settings");
    assert_eq!(settings.options.profile, ExportProfile::Mp4H264);
    assert_eq!(settings.options.quality, RenderQuality::Final);
    assert_eq!(settings.output.as_ref(), Some(&request));
    eprintln!("{test} rendered: {}", artifact.path);
}

#[cfg(feature = "ges-render")]
#[test]
fn in_process_export_to_a_chosen_folder_never_overwrites() {
    let test = "in_process_export_to_a_chosen_folder_never_overwrites";
    let Some((dir, project)) = fixture_project(test, ExportProfile::Mp4H264) else {
        return;
    };
    let chosen = tempfile::tempdir().expect("chosen folder");
    fs::write(chosen.path().join("Edison intro.mp4"), b"keep").expect("existing export");
    let request = output("Edison intro", Some(chosen.path()));

    let first = export(
        dir.path(),
        &project,
        final_options(ExportProfile::Mp4H264),
        "export-mp4H264-chosen-1",
        Some(&request),
    )
    .expect("first export renders");
    let second = export(
        dir.path(),
        &project,
        final_options(ExportProfile::Mp4H264),
        "export-mp4H264-chosen-2",
        Some(&request),
    )
    .expect("second export renders");

    let first_path = chosen.path().join("Edison intro (2).mp4");
    let second_path = chosen.path().join("Edison intro (3).mp4");
    assert_eq!(
        first.export_artifact.expect("first artifact").path,
        first_path.display().to_string()
    );
    assert_eq!(
        second.export_artifact.expect("second artifact").path,
        second_path.display().to_string()
    );
    assert_eq!(
        fs::read(chosen.path().join("Edison intro.mp4")).expect("kept"),
        b"keep"
    );
    assert!(fs::metadata(&first_path).expect("first").len() > 0);
    let reloaded = load_split_project(dir.path()).expect("reload");
    assert_eq!(reloaded.export_artifacts.len(), 2);
    eprintln!("{test} rendered: {}", second_path.display());
}

#[cfg(feature = "ges-render")]
#[test]
fn in_process_export_applies_the_frame_rate_override() {
    let test = "in_process_export_applies_the_frame_rate_override";
    let Some((dir, project)) = fixture_project(test, ExportProfile::Mp4H264) else {
        return;
    };
    let options = final_options(ExportProfile::Mp4H264)
        .with_fps(Some(30.0))
        .expect("30 fps");

    let result = export(
        dir.path(),
        &project,
        options,
        "export-mp4H264-30fps",
        Some(&output("Edison 30", None)),
    )
    .expect("30 fps export renders");

    let artifact = result.export_artifact.expect("artifact");
    let (probe, _) = probe_media_with_gstreamer(
        &dir.path().join(&artifact.path),
        Duration::from_secs(30),
        "fpsOverride",
    )
    .expect("probe export");
    let fps = probe.video.and_then(|video| video.fps).expect("video fps");
    assert!((fps - 30.0).abs() < 0.01, "exported at {fps} fps");
    eprintln!("{test} rendered: {} at {fps} fps", artifact.path);
}

#[cfg(feature = "ges-render")]
#[test]
fn in_process_master_export_renders_h264_and_webm() {
    let test = "in_process_master_export_renders_h264_and_webm";
    for (profile, extension) in [
        (ExportProfile::Mp4H264, "mp4"),
        (ExportProfile::Webm, "webm"),
    ] {
        let Some((dir, project)) = fixture_project(test, profile) else {
            continue;
        };
        let options = final_options(profile)
            .with_encode_tier(ExportEncodeTier::Master)
            .expect("master options");
        let job_id = format!("export-{extension}-master");

        let result = export(
            dir.path(),
            &project,
            options,
            &job_id,
            Some(&output("Edison master", None)),
        )
        .unwrap_or_else(|errors| panic!("{profile:?} Master export renders: {errors:?}"));

        if result.render_report.command.program == "gstreamer-ges" {
            assert!(
                result
                    .render_report
                    .command
                    .args
                    .contains(&"--encode-tier=master".to_string()),
                "{profile:?}"
            );
        }
        assert_eq!(
            result.export_artifact.expect("artifact").path,
            format!("exports/Edison master.{extension}")
        );
        let reloaded = load_split_project(dir.path()).expect("reload");
        let job = reloaded
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .expect("job");
        assert_eq!(job.status, JobStatus::Completed);
        let settings = serde_json::to_value(job.export_settings.as_ref().expect("settings"))
            .expect("settings json");
        assert_eq!(settings["encodeTier"], "master");
        eprintln!(
            "{test} rendered {profile:?} with {}",
            result.render_report.command.program
        );
    }
}

#[test]
fn in_process_export_leaves_no_artifact_when_cancelled() {
    use video_creater_lib::project::mutation::acquire_split_project_mutation_lease;
    use video_creater_lib::render_pipeline::cancel::{
        is_render_attempt_active, request_render_cancellation, RenderAttemptKey,
        RenderCancellationOutcome,
    };

    let dir = tempfile::tempdir().expect("temp project dir");
    let chosen = tempfile::tempdir().expect("chosen folder");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    let attempt_id = "render-attempt/cancelled-export";
    let key = RenderAttemptKey::new(
        fs::canonicalize(dir.path()).expect("canonical root"),
        &project.id,
        "export-mp4H264-cancelled",
        attempt_id,
    )
    .expect("attempt key");
    let lease = acquire_split_project_mutation_lease(dir.path()).expect("hold project lease");
    let project_dir = dir.path().to_path_buf();
    let chosen_dir = chosen.path().to_path_buf();
    let project_id = project.id.clone();
    let render = std::thread::spawn(move || {
        let request = output("Edison intro", Some(&chosen_dir));
        render_media_export_to_split_project_folder(MediaExportRequest {
            project_dir: &project_dir,
            project_id: &project_id,
            options: final_options(ExportProfile::Mp4H264),
            job: export_job_summary("export-mp4H264-cancelled", UPDATED_AT),
            updated_at: UPDATED_AT,
            run_id: Some(attempt_id.to_string()),
            range_seconds: None,
            timeline_id: None,
            output: Some(&request),
        })
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !is_render_attempt_active(&key) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        request_render_cancellation(&key),
        RenderCancellationOutcome::Requested
    );
    drop(lease);

    let errors = render
        .join()
        .expect("render joins")
        .expect_err("cancelled export");
    assert_eq!(errors[0].path, "render.start.cancelled");
    let reloaded = load_split_project(dir.path()).expect("reload");
    assert!(reloaded.export_artifacts.is_empty());
    assert_eq!(
        reloaded
            .jobs
            .iter()
            .find(|job| job.id == "export-mp4H264-cancelled")
            .expect("cancelled job")
            .status,
        JobStatus::Cancelled
    );
    assert_eq!(fs::read_dir(chosen.path()).expect("chosen").count(), 0);
}
