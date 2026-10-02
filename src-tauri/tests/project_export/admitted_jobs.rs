//! Real Linux admission/worker integration, including a worker blocked beyond editor-lease TTL.
#![cfg(all(feature = "ges-render", feature = "web-host", target_os = "linux"))]

use super::named_exports::fixture_project_of_length;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use video_creater_lib::app_service::context::{ClientKind, RequestContext};
use video_creater_lib::app_service::events::NoopEventSink;
use video_creater_lib::app_service::operation::AuthorizationScope;
use video_creater_lib::app_service::projects::ProjectService;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{JobStatus, VideoProject};
use video_creater_lib::project::mutation::acquire_split_project_artifact_lease;
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::render_pipeline::cancel::{
    request_render_cancellation_by_locator, RenderCancellationOutcome,
};
use video_creater_lib::render_pipeline::gstreamer_backend::probe_media_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::{
    admit_media_render, read_media_render_attempt, MediaRenderAdmission, MediaRenderAttempt,
    MediaRenderInput, ProjectMediaRenderPaths,
};
use video_creater_lib::settings::storage::acquire_storage_mutation_lease;
use video_creater_lib::web_host::editor_lease::{EditorLease, LeaseError, LeaseManager};

const WAIT: Duration = Duration::from_secs(180);
const UPDATED_AT: &str = "2026-10-01T12:00:00Z";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn context(project: &VideoProject, revision: Option<u64>) -> RequestContext {
    RequestContext::new(
        ClientKind::Browser,
        "admitted-integration",
        Some(project.id.clone()),
        revision,
        BTreeSet::from([
            AuthorizationScope::ProjectRead,
            AuthorizationScope::ProjectWrite,
        ]),
        None,
    )
    .unwrap()
}

fn input(project: &VideoProject, job: &str) -> MediaRenderInput {
    MediaRenderInput {
        project_id: project.id.clone(),
        profile: ExportProfile::Mp4H264,
        quality: RenderQuality::Draft,
        width: 320,
        height: 180,
        job_id: job.into(),
        attempt_id: format!("render-attempt/{job}"),
        updated_at: UPDATED_AT.into(),
        range_start_seconds: None,
        range_end_seconds: None,
        timeline_id: None,
        fps: None,
        encode_tier: None,
        output: None,
        export_settings: None,
    }
}

fn bounded<T: Send + 'static>(label: &str, action: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(action());
    });
    rx.recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|error| panic!("{label} blocked: {error}"))
}

fn terminal(dir: &Path, request: &MediaRenderInput) -> MediaRenderAttempt {
    let deadline = Instant::now() + WAIT;
    loop {
        let outcome = read_media_render_attempt(dir, &request.job_id, &request.attempt_id).unwrap();
        if outcome != MediaRenderAttempt::Pending {
            return outcome;
        }
        assert!(Instant::now() < deadline, "admitted worker did not finish");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn admit_bounded(
    dir: &Path,
    revision: u64,
    request: &MediaRenderInput,
    leases: &Arc<LeaseManager>,
    lease: &EditorLease,
) -> MediaRenderAdmission {
    let dir = dir.to_path_buf();
    let request = request.clone();
    let leases = Arc::clone(leases);
    let lease = lease.clone();
    bounded("admission ACK with worker ownership held", move || {
        leases
            .with_valid_lease(
                &lease.project_id,
                &lease.session_id,
                &lease.token,
                now(),
                || admit_media_render(&dir, revision, request),
            )
            .unwrap()
            .unwrap()
    })
}

#[test]
fn real_admission_preserves_source_snapshot_and_replays_without_another_render() {
    let (dir, _) = fixture_project_of_length(
        "real_admission_preserves_source_snapshot",
        ExportProfile::Mp4H264,
        150.0,
    )
    .expect("BLOCKED: real admitted render requires the prepared Linux MP4 runtime");
    let project = load_split_project(dir.path()).unwrap();
    let request = input(&project, "admitted-snapshot");
    let leases = Arc::new(LeaseManager::new(30, 5));
    let lease = leases.acquire(&project.id, "editor", now()).unwrap();
    // Artifact ownership blocks the real worker, not admission or the editor gate.
    let held = acquire_split_project_artifact_lease(dir.path()).unwrap();
    let started = Instant::now();
    let admission = admit_bounded(
        dir.path(),
        project.content_revision,
        &request,
        &leases,
        &lease,
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "ACK waited for execution"
    );
    assert_eq!(admission.admission_protocol, 1);
    assert_eq!(admission.source_revision, project.content_revision);
    assert_eq!(admission.job_id, request.job_id);
    assert_eq!(admission.attempt_id, request.attempt_id);
    assert_eq!(
        admission
            .project
            .jobs
            .iter()
            .find(|job| job.id == request.job_id)
            .unwrap()
            .status,
        JobStatus::Queued
    );
    assert_eq!(
        read_media_render_attempt(dir.path(), &request.job_id, &request.attempt_id).unwrap(),
        MediaRenderAttempt::Pending
    );
    drop(held);

    let paths = ProjectMediaRenderPaths::new(&request.job_id, request.profile).unwrap();
    let deadline = Instant::now() + WAIT;
    loop {
        let current = load_split_project(dir.path()).unwrap();
        if current.jobs.iter().any(|job| {
            job.id == request.job_id
                && matches!(job.status, JobStatus::Running | JobStatus::Progress)
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "worker never reached encoding");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !paths.absolute_log_path(dir.path()).exists(),
        "encode already finished"
    );
    let current = load_split_project(dir.path()).unwrap();
    let service = ProjectService::new(Arc::new(NoopEventSink));
    let write = leases
        .with_valid_lease(&project.id, "editor", &lease.token, now(), || {
            service.apply_action(
                &context(&project, Some(current.content_revision)),
                dir.path(),
                ProjectAction::RemoveItems {
                    item_ids: vec![project.timeline.tracks[0].items[0].id.clone()],
                },
            )
        })
        .unwrap()
        .unwrap();
    assert!(write.project.timeline.tracks[0].items.is_empty());
    assert!(
        !paths.absolute_log_path(dir.path()).exists(),
        "edit must land during encoding"
    );
    let MediaRenderAttempt::Completed { result } = terminal(dir.path(), &request) else {
        panic!("real render failed");
    };
    assert!(
        result.project.timeline.tracks[0].items.is_empty(),
        "completion returned an old project"
    );
    let output = dir.path().join(&result.output_path);
    let (probe, _) =
        probe_media_with_gstreamer(&output, Duration::from_secs(30), "admittedSnapshot").unwrap();
    assert!(
        (probe.duration_seconds.unwrap() - 150.0).abs() < 0.5,
        "output did not retain admitted snapshot"
    );
    let before = std::fs::metadata(&output).unwrap().modified().unwrap();
    let replay = admit_media_render(dir.path(), project.content_revision, request.clone()).unwrap();
    assert_eq!(replay.source_revision, admission.source_revision);
    assert_eq!(replay.attempt_id, admission.attempt_id);
    assert_eq!(
        request_render_cancellation_by_locator(dir.path(), &request.job_id, &request.attempt_id),
        RenderCancellationOutcome::NotFound,
        "replaying a completed admission must not register another worker"
    );
    assert_eq!(
        std::fs::metadata(&output).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(
        replay
            .project
            .jobs
            .iter()
            .filter(|job| job.id == request.job_id)
            .count(),
        1
    );
    assert_eq!(
        replay
            .project
            .render_reports
            .iter()
            .filter(|report| report.id == request.job_id)
            .count(),
        1
    );
    assert!(matches!(
        read_media_render_attempt(dir.path(), &request.job_id, &request.attempt_id).unwrap(),
        MediaRenderAttempt::Completed { .. }
    ));
}

#[test]
fn real_pending_worker_outlives_editor_ttl_without_blocking_reads_edit_cancel_or_takeover() {
    let (dir, _) = fixture_project_of_length(
        "real_pending_worker_outlives_editor_ttl",
        ExportProfile::Mp4H264,
        6.0,
    )
    .expect("BLOCKED: real admitted worker requires the prepared Linux MP4 runtime");
    let project = load_split_project(dir.path()).unwrap();
    let unrelated = tempfile::tempdir().unwrap();
    let mut other = sample_project();
    other.id = "unrelated-admitted-project".into();
    save_split_project(unrelated.path(), &other).unwrap();
    let request = input(&project, "admitted-pending-ttl");
    let leases = Arc::new(LeaseManager::new(30, 5));
    let lease = leases.acquire(&project.id, "first", now()).unwrap();
    let held = acquire_storage_mutation_lease().unwrap();
    let admitted_at = Instant::now();
    let admission = admit_bounded(
        dir.path(),
        project.content_revision,
        &request,
        &leases,
        &lease,
    );
    let service = ProjectService::new(Arc::new(NoopEventSink));
    // Thirty-one actual seconds of a pinned production worker waiting for storage;
    // this deliberately does not claim thirty-one seconds spent encoding media.
    while admitted_at.elapsed() < Duration::from_secs(31) {
        assert_eq!(
            read_media_render_attempt(dir.path(), &request.job_id, &request.attempt_id).unwrap(),
            MediaRenderAttempt::Pending
        );
        let renewal_leases = Arc::clone(&leases);
        let renewal_lease = lease.clone();
        let renewed = bounded(
            "editor lease renewal while worker remains pending",
            move || {
                renewal_leases
                    .renew(
                        &renewal_lease.project_id,
                        &renewal_lease.session_id,
                        &renewal_lease.token,
                        now(),
                    )
                    .unwrap()
            },
        );
        assert!(renewed.expires_at >= now() + 29);
        let read_service = service.clone();
        let read_dir = unrelated.path().to_path_buf();
        let read_context = context(&other, None);
        let id = other.id.clone();
        bounded("unrelated snapshot and progress", move || {
            assert_eq!(read_service.load(&read_context, &read_dir).unwrap().id, id);
            assert!(read_service
                .job_progress(&read_context, &read_dir)
                .unwrap()
                .is_empty());
        });
        let read_service = service.clone();
        let read_dir = dir.path().to_path_buf();
        let read_context = context(&project, None);
        let job_id = request.job_id.clone();
        bounded("pending worker snapshot and progress", move || {
            let snapshot = read_service.load(&read_context, &read_dir).unwrap();
            assert_eq!(
                snapshot
                    .jobs
                    .iter()
                    .find(|job| job.id == job_id)
                    .unwrap()
                    .status,
                JobStatus::Queued
            );
            read_service.job_progress(&read_context, &read_dir).unwrap();
        });
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(
        now() > lease.expires_at,
        "initial thirty-second TTL must really elapse"
    );
    let edit_service = service.clone();
    let edit_dir = dir.path().to_path_buf();
    let edit_context = context(&project, Some(admission.project.content_revision));
    let settings = project.render_settings.clone();
    let edit_leases = Arc::clone(&leases);
    let edit_id = project.id.clone();
    let token = lease.token.clone();
    let edited = bounded("edit beyond initial editor TTL", move || {
        edit_leases
            .with_valid_lease(&edit_id, "first", &token, now(), || {
                edit_service.update_settings(
                    &edit_context,
                    &edit_dir,
                    "Edited after 31 seconds".into(),
                    settings,
                )
            })
            .unwrap()
            .unwrap()
    });
    assert_eq!(edited.project.name, "Edited after 31 seconds");
    assert_eq!(
        read_media_render_attempt(dir.path(), &request.job_id, &request.attempt_id).unwrap(),
        MediaRenderAttempt::Pending
    );
    assert_eq!(
        request_render_cancellation_by_locator(dir.path(), &request.job_id, &request.attempt_id),
        RenderCancellationOutcome::Requested
    );
    let take_leases = Arc::clone(&leases);
    let id = project.id.clone();
    let replacement = bounded("takeover while worker remains pending", move || {
        take_leases.takeover(&id, "second", now()).unwrap()
    });
    assert_eq!(
        leases.validate(&project.id, "first", &lease.token, now()),
        Err(LeaseError::Invalid)
    );
    leases
        .validate(&project.id, "second", &replacement.token, now())
        .unwrap();
    assert!(admitted_at.elapsed() >= Duration::from_secs(31));
    drop(held);
    assert!(matches!(
        terminal(dir.path(), &request),
        MediaRenderAttempt::Failed {
            interrupted: false,
            ..
        }
    ));
    let latest = load_split_project(dir.path()).unwrap();
    assert_eq!(latest.name, edited.project.name);
    assert_eq!(
        latest
            .jobs
            .iter()
            .find(|job| job.id == request.job_id)
            .unwrap()
            .status,
        JobStatus::Cancelled
    );
    assert!(latest.render_reports.is_empty());
    eprintln!(
        "real admitted worker remained pending {:?}; no 31-second encode claim",
        admitted_at.elapsed()
    );
}
