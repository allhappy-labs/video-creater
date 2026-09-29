//! A GES render reports progress that the editor can read while the render holds the leases.
//!
//! Needs the `ges-render` feature and a resolvable render runtime. On Linux development machines
//! point `VIDEO_CREATER_RENDER_RUNTIME_ROOT` at a staged runtime root.
#![cfg(feature = "ges-render")]

use serde_json::json;
use std::fs;
use std::time::{Duration, Instant};
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::job_progress::{read_job_progress_snapshots, JOB_PROGRESS_DIR};
use video_creater_lib::project::model::{JobStatus, JobSummary};
use video_creater_lib::project::split::save_split_project;
use video_creater_lib::render_pipeline::gstreamer_backend::generate_fixture_source_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::render_media_to_split_project_folder;
use video_creater_lib::render_runtime::start_render_process_runtime;

const JOB_ID: &str = "render-progress-ges";
const SOURCE_SECONDS: f64 = 60.0;

fn export_job_summary(job_id: &str, updated_at: &str) -> JobSummary {
    video_creater_lib::workflows::temporal_job_summary(
        video_creater_lib::workflows::TemporalWorkflowKind::ExportMedia,
        "project-export-fixture",
        job_id,
        JobStatus::Queued,
        updated_at,
    )
}

#[test]
fn in_process_render_reports_progress_readable_without_the_lease() {
    if let Err(error) = start_render_process_runtime() {
        eprintln!("SKIPPED: render runtime could not resolve: {error:?}");
        return;
    }
    let dir = tempfile::tempdir().expect("temp project dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    generate_fixture_source_with_gstreamer(
        &dir.path().join("media/input.mp4"),
        640,
        360,
        30.0,
        SOURCE_SECONDS,
        Duration::from_secs(120),
    )
    .expect("generate fixture source");
    let mut project = sample_project();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 30.0;
    project.timeline.duration_seconds = SOURCE_SECONDS;
    project.media[0].duration_seconds = SOURCE_SECONDS;
    project.media[0].width = Some(640);
    project.media[0].height = Some(360);
    project.media[0].fps = Some(30.0);
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = SOURCE_SECONDS;
    item.properties.insert("sourceIn".to_string(), json!(0.0));
    item.properties
        .insert("sourceOut".to_string(), json!(SOURCE_SECONDS));
    save_split_project(dir.path(), &project).expect("save split project");

    let render_dir = dir.path().to_path_buf();
    let project_id = project.id.clone();
    let render = std::thread::spawn(move || {
        render_media_to_split_project_folder(
            &render_dir,
            &project_id,
            ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 640, 360)
                .expect("export options"),
            export_job_summary(JOB_ID, "2026-09-16T10:00:00Z"),
            "2026-09-16T10:00:00Z",
            None,
            None,
        )
    });

    let mut observed = Vec::new();
    while !render.is_finished() {
        let started = Instant::now();
        let snapshots = read_job_progress_snapshots(dir.path()).expect("read progress");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(100),
            "progress read took {elapsed:?} while the render held the leases"
        );
        if let Some(snapshot) = snapshots.iter().find(|snapshot| snapshot.job_id == JOB_ID) {
            if observed.last() != Some(&snapshot.progress) {
                observed.push(snapshot.progress);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let result = render.join().expect("render thread");
    eprintln!("observed progress values: {observed:?}");
    let result = result.expect("render succeeds");
    assert!(dir.path().join(&result.output_path).is_file());

    assert!(
        observed
            .iter()
            .any(|progress| *progress > 0.0 && *progress <= 0.95),
        "expected at least one progress value in (0, 0.95], observed {observed:?}"
    );
    assert!(
        observed.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress decreased: {observed:?}"
    );
    assert!(
        !dir.path()
            .join(JOB_PROGRESS_DIR)
            .join(format!("{JOB_ID}.json"))
            .exists(),
        "the snapshot is removed when the render ends"
    );
}
