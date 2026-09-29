//! How long a render owns the canonical project.
//!
//! A render takes the project mutation lease for its start phase, releases it while it encodes,
//! and takes it again for its result writes. These tests pin both halves of that: an editor write
//! issued while a render encodes never waits for the encode, and the render keeps rendering the
//! snapshot it read at its start, so an edit that lands meanwhile cannot change its output.

use super::export_job_summary;
use super::named_exports::fixture_project_of_length;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::command_queue::project_command_queue;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::model::{JobStatus, TimelineTrack, TrackKind};
use video_creater_lib::project::mutation::{
    acquire_split_project_artifact_lease, acquire_split_project_mutation_lease,
};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project,
};
use video_creater_lib::render_pipeline::gstreamer_backend::probe_media_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::{
    render_media_export_to_split_project_folder, MediaExportRequest, ProjectMediaRenderPaths,
    ProjectMediaRenderResult,
};

const UPDATED_AT: &str = "2026-09-18T10:00:00Z";
const PROFILE: ExportProfile = ExportProfile::Mp4H264;
/// Enough timeline for the encode to outlast one queued project write by a wide margin.
const LONG_TIMELINE_SECONDS: f64 = 150.0;
const SHORT_TIMELINE_SECONDS: f64 = 6.0;
const WAIT_LIMIT: Duration = Duration::from_secs(600);
const POLL: Duration = Duration::from_millis(20);

/// A render running on its own thread, plus the channel that reports it finished.
struct BackgroundRender {
    handle: std::thread::JoinHandle<Result<ProjectMediaRenderResult, Vec<String>>>,
    finished: mpsc::Receiver<()>,
    log_path: PathBuf,
}

impl BackgroundRender {
    fn is_running(&self) -> bool {
        matches!(self.finished.try_recv(), Err(mpsc::TryRecvError::Empty))
    }

    /// Whether the encode, the probe and the media validation are done. The render writes this
    /// log right after them and before any of its result writes.
    fn encoded(&self) -> bool {
        self.log_path.is_file()
    }

    fn await_encoded(&self) -> Duration {
        let started = Instant::now();
        let deadline = started + WAIT_LIMIT;
        while !self.encoded() && Instant::now() < deadline {
            std::thread::sleep(POLL);
        }
        started.elapsed()
    }

    fn finish(self) -> Result<ProjectMediaRenderResult, Vec<String>> {
        self.handle.join().expect("render joins")
    }
}

fn start_render(project_dir: &Path, project_id: &str, job_id: &'static str) -> BackgroundRender {
    let options =
        ExportRenderOptions::new(PROFILE, RenderQuality::Draft, 320, 180).expect("export options");
    let (finished_tx, finished) = mpsc::channel();
    let render_dir = project_dir.to_path_buf();
    let render_project_id = project_id.to_string();
    let handle = std::thread::spawn(move || {
        let result = render_media_export_to_split_project_folder(MediaExportRequest {
            project_dir: &render_dir,
            project_id: &render_project_id,
            options,
            job: export_job_summary(job_id, UPDATED_AT),
            updated_at: UPDATED_AT,
            run_id: None,
            range_seconds: None,
            timeline_id: None,
            output: None,
        })
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|error| format!("{}: {}", error.path, error.message))
                .collect::<Vec<_>>()
        });
        let _ = finished_tx.send(());
        result
    });
    BackgroundRender {
        handle,
        finished,
        log_path: ProjectMediaRenderPaths::new(job_id, PROFILE)
            .expect("render paths")
            .absolute_log_path(project_dir),
    }
}

/// Blocks until the render recorded `job_id` as running, which is the end of its start phase.
fn await_encoding_start(project_dir: &Path, job_id: &str) {
    let deadline = Instant::now() + WAIT_LIMIT;
    while Instant::now() < deadline {
        let status = load_split_project(project_dir)
            .expect("read the project while the render runs")
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .map(|job| job.status.clone());
        match status {
            Some(JobStatus::Running | JobStatus::Progress) => return,
            _ => std::thread::sleep(POLL),
        }
    }
    panic!("the render never recorded {job_id} as running");
}

/// The editor's own write path: one canonical project action on the per-project write queue,
/// awaited on a helper thread so the test can watch it stay pending.
fn queue_project_write(
    project_dir: &Path,
    label: &'static str,
    actions: Vec<ProjectAction>,
) -> mpsc::Receiver<Result<usize, String>> {
    let (done_tx, done) = mpsc::channel();
    let queue_dir = project_dir.to_path_buf();
    let queued = project_command_queue()
        .submit(project_dir, label, move || {
            apply_project_actions_to_split_project(&queue_dir, actions)
                .map(|write| write.project.timeline.tracks.len())
                .map_err(|error| error.to_string())
        })
        .expect("submit the write");
    std::thread::spawn(move || done_tx.send(queued.wait()));
    done
}

fn audio_track(id: &str) -> TimelineTrack {
    TimelineTrack {
        id: id.to_string(),
        name: "Added during a render".to_string(),
        kind: TrackKind::Audio,
        items: Vec::new(),
        locked: false,
        sync_locked: false,
        enabled: true,
        transitions: Vec::new(),
    }
}

/// The encode holds no project lease, so a write issued while a render encodes waits only for
/// the render's own short result writes. Held here by the test instead of by the render, which
/// makes the assertion an ordering one rather than a wall-clock one.
#[cfg(feature = "ges-render")]
#[test]
fn an_encoding_render_holds_no_project_lease() {
    let test = "an_encoding_render_holds_no_project_lease";
    let Some((dir, project)) = fixture_project_of_length(test, PROFILE, SHORT_TIMELINE_SECONDS)
    else {
        return;
    };
    let job_id = "export-mp4H264-write-during-encode";
    let render = start_render(dir.path(), &project.id, job_id);
    await_encoding_start(dir.path(), job_id);

    // Stand in for the render's result writes: hold the lease the render needs to finish.
    let held = acquire_split_project_mutation_lease(dir.path()).expect("hold the project lease");
    let write = queue_project_write(
        dir.path(),
        "editor write during a render",
        vec![ProjectAction::CreateTrack {
            track: audio_track("track-added-during-render"),
            after_track_id: None,
        }],
    );

    let encoding = render.await_encoded();
    let encoded = render.encoded();
    let still_running = render.is_running();
    let write_is_pending = matches!(write.try_recv(), Err(mpsc::TryRecvError::Empty));
    drop(held);
    if !encoded {
        let outcome = render.finish();
        panic!(
            "the encode must run to completion while another owner holds the project lease; \
             render: {outcome:?}"
        );
    }
    assert!(
        still_running,
        "the render must still be waiting for the lease its result writes need"
    );
    assert!(
        write_is_pending,
        "the queued write waits for the lease, not for the encode"
    );

    // What the render does own for its whole run: the project's derived files, which is what
    // keeps two renders, or a render and a preparation, from publishing the same cache entry.
    let (artifacts_tx, artifacts_rx) = mpsc::channel();
    let artifact_dir = dir.path().to_path_buf();
    let artifact_contender = std::thread::spawn(move || {
        let lease = acquire_split_project_artifact_lease(&artifact_dir);
        artifacts_tx.send(()).expect("signal acquired");
        lease.map(drop)
    });
    assert!(
        artifacts_rx
            .recv_timeout(Duration::from_millis(300))
            .is_err(),
        "the render owns the project's artifacts until it ends"
    );

    let tracks = write
        .recv_timeout(WAIT_LIMIT)
        .expect("the write completes once the lease is free")
        .expect("the write succeeds");
    assert_eq!(tracks, project.timeline.tracks.len() + 1);
    let result = render
        .finish()
        .unwrap_or_else(|errors| panic!("render succeeds: {errors:?}"));
    assert_eq!(result.render_report.summary.status, "succeeded");
    assert_eq!(
        artifact_contender.join().expect("contender joins"),
        Ok(()),
        "the artifact lease is free once the render ends"
    );
    let reloaded = load_split_project(dir.path()).expect("reload");
    assert!(reloaded
        .timeline
        .tracks
        .iter()
        .any(|track| track.id == "track-added-during-render"));
    assert_eq!(
        reloaded
            .jobs
            .iter()
            .find(|job| job.id == job_id)
            .expect("render job")
            .status,
        JobStatus::Completed
    );
    eprintln!("{test}: the encode took {encoding:?} with the project lease held elsewhere");
}

/// A clip deleted while the render encodes lands in the canonical project without waiting for
/// the encode, and never reaches the render's output.
#[cfg(feature = "ges-render")]
#[test]
fn an_edit_during_a_render_does_not_change_its_output() {
    let test = "an_edit_during_a_render_does_not_change_its_output";
    let Some((dir, project)) = fixture_project_of_length(test, PROFILE, LONG_TIMELINE_SECONDS)
    else {
        return;
    };
    let job_id = "export-mp4H264-edit-during-encode";
    let removed_item_id = project.timeline.tracks[0].items[0].id.clone();
    let render = start_render(dir.path(), &project.id, job_id);
    await_encoding_start(dir.path(), job_id);
    assert!(
        !render.encoded(),
        "the encode must still be running when the delete is issued"
    );

    let delete = queue_project_write(
        dir.path(),
        "clip delete during a render",
        vec![ProjectAction::RemoveItems {
            item_ids: vec![removed_item_id],
        }],
    );
    let deleted_at = Instant::now();
    delete
        .recv_timeout(WAIT_LIMIT)
        .expect("the delete completes")
        .expect("the delete succeeds");
    let delete_seconds = deleted_at.elapsed();
    assert!(
        !render.encoded(),
        "the delete landed after {delete_seconds:?}, before the encode finished"
    );
    assert_eq!(
        load_split_project(dir.path())
            .expect("reload during the render")
            .timeline
            .tracks[0]
            .items
            .len(),
        0
    );

    let result = render
        .finish()
        .unwrap_or_else(|errors| panic!("render succeeds: {errors:?}"));
    assert_eq!(result.render_report.summary.status, "succeeded");
    let (probe, _) = probe_media_with_gstreamer(
        &dir.path().join(&result.output_path),
        Duration::from_secs(300),
        "renderLease",
    )
    .expect("probe the rendered output");
    let duration = probe.duration_seconds.expect("rendered duration");
    assert!(
        (duration - LONG_TIMELINE_SECONDS).abs() < 0.5,
        "the render kept the snapshot it started from: {duration} s"
    );

    let reloaded = load_split_project(dir.path()).expect("reload");
    assert!(
        reloaded.timeline.tracks[0].items.is_empty(),
        "the delete survived the render's result writes"
    );
    assert!(reloaded
        .render_reports
        .iter()
        .any(|report| report.output_path == result.output_path));
    eprintln!("{test}: the delete landed in {delete_seconds:?} and the render wrote {duration} s");
}
