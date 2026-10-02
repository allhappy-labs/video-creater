//! Admission limits count accepted workers, including workers waiting for storage.

use super::*;
use crate::app_service::error::ServiceErrorCode;
use std::sync::{mpsc, Arc, Barrier};
use std::time::{Duration, Instant};

thread_local! {
    static TEST_CAPACITY: Arc<WorkerCapacity> = Arc::new(WorkerCapacity::new());
}

pub(super) fn isolated_worker_capacity() -> Arc<WorkerCapacity> {
    TEST_CAPACITY.with(Arc::clone)
}

fn admit(
    root: &Path,
    revision: u64,
    input: MediaRenderInput,
    capacity: &Arc<WorkerCapacity>,
    execute: impl FnOnce(
            PathBuf,
            DurableInput,
            RegisteredProjectRenderAttempt,
        ) -> PipelineResult<ProjectMediaRenderResult>
        + Send
        + 'static,
) -> Result<MediaRenderAdmission, ServiceError> {
    admit_with_capacity_and_spawner(
        root,
        revision,
        input,
        Arc::clone(capacity),
        execute,
        |worker| std::thread::Builder::new().spawn(worker).map(|_| ()),
    )
}

fn fixture(index: usize) -> (tempfile::TempDir, MediaRenderInput) {
    let root = tempfile::tempdir().unwrap();
    let project = VideoProject::new_empty(
        format!("capacity-project-{index}"),
        format!("Retained snapshot {index}"),
        "2026-10-01".into(),
    );
    crate::project::split::save_split_project(root.path(), &project).unwrap();
    let input = MediaRenderInput {
        project_id: project.id,
        profile: ExportProfile::Webm,
        quality: RenderQuality::Draft,
        width: 320,
        height: 180,
        job_id: format!("capacity-job-{index}"),
        attempt_id: format!("render-attempt/capacity-{index}"),
        updated_at: "2026-10-01".into(),
        range_start_seconds: None,
        range_end_seconds: None,
        timeline_id: None,
        fps: None,
        encode_tier: None,
        output: None,
        export_settings: None,
    };
    (root, input)
}

struct BlockedWorkers {
    roots: Vec<(tempfile::TempDir, MediaRenderInput)>,
    releases: Vec<mpsc::Sender<()>>,
}

impl Drop for BlockedWorkers {
    fn drop(&mut self) {
        // Always unblock accepted workers, including when a regression assertion fails.
        self.releases.clear();
        let deadline = Instant::now() + Duration::from_secs(10);
        for (root, input) in &self.roots {
            while attempt_is_pinned(root.path(), &input.job_id, &input.attempt_id) {
                assert!(Instant::now() < deadline, "blocked worker did not exit");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[test]
fn fifth_retained_worker_is_rejected_before_durable_admission() {
    let capacity = Arc::new(WorkerCapacity::new());
    let mut workers = BlockedWorkers {
        roots: Vec::new(),
        releases: Vec::new(),
    };
    for index in 0..4 {
        let (root, input) = fixture(index);
        let revision = load_split_project(root.path()).unwrap().content_revision;
        let (release, released) = mpsc::channel();
        let (started, entered) = mpsc::channel();
        admit(
            root.path(),
            revision,
            input.clone(),
            &capacity,
            move |_, record, _attempt| {
                started.send(record.project.name.clone()).unwrap();
                let _ = released.recv();
                // The production closure retains the admitted snapshot until execution exits.
                assert_eq!(record.project.name, format!("Retained snapshot {index}"));
                Err(render_cancelled_error("capacity.test"))
            },
        )
        .unwrap();
        assert_eq!(
            entered.recv_timeout(Duration::from_secs(5)).unwrap(),
            format!("Retained snapshot {index}")
        );
        workers.roots.push((root, input));
        workers.releases.push(release);
    }
    let (root, input) = fixture(4);
    let before = load_split_project(root.path()).unwrap();
    for (accepted_root, accepted_input) in &workers.roots {
        let replay = admit(
            accepted_root.path(),
            1,
            accepted_input.clone(),
            &capacity,
            |_, _, _| panic!("exact replay must not start another worker"),
        )
        .unwrap();
        assert_eq!(replay.source_revision, 1);
        assert_eq!(replay.project.jobs.len(), 1);
        assert_eq!(
            read_media_render_attempt(
                accepted_root.path(),
                &accepted_input.job_id,
                &accepted_input.attempt_id
            )
            .unwrap(),
            MediaRenderAttempt::Pending
        );
    }
    let result = admit(
        root.path(),
        before.content_revision,
        input.clone(),
        &capacity,
        |_, _, _| Err(render_cancelled_error("capacity.unaccepted")),
    );
    assert_eq!(result.unwrap_err().code(), ServiceErrorCode::Busy);
    assert_eq!(load_split_project(root.path()).unwrap(), before);
    assert!(!attempt_directory(root.path(), &input.job_id, &input.attempt_id).exists());
    drop(workers);
    assert_accepts_last_slot(&capacity);
}

fn assert_accepts_last_slot(capacity: &Arc<WorkerCapacity>) {
    // Three other reservations leave exactly one slot. Any leaked permit
    // from the preceding path makes the real admission below fail.
    let _held = [
        capacity.reserve().unwrap(),
        capacity.reserve().unwrap(),
        capacity.reserve().unwrap(),
    ];
    let (root, input) = fixture(99);
    let (release, released) = mpsc::channel();
    admit(root.path(), 1, input.clone(), capacity, move |_, _, _| {
        let _ = released.recv();
        Err(render_cancelled_error("capacity.release"))
    })
    .unwrap();
    drop(BlockedWorkers {
        roots: vec![(root, input)],
        releases: vec![release],
    });
}

#[test]
fn failed_durable_input_write_releases_reserved_worker_slot() {
    let capacity = Arc::new(WorkerCapacity::new());
    let (root, input) = fixture(10);
    let identity_path =
        attempt_directory(root.path(), &input.job_id, &input.attempt_id).join("identity.json");
    std::fs::create_dir_all(&identity_path).unwrap();
    assert!(admit(root.path(), 1, input, &capacity, |_, _, _| {
        panic!("failed durable write cannot start a worker")
    })
    .is_err());
    assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
    assert_accepts_last_slot(&capacity);
}

#[test]
fn refused_spawn_preserves_terminal_ack_and_releases_worker_slot() {
    let capacity = Arc::new(WorkerCapacity::new());
    let (root, input) = fixture(11);
    let admission = admit_with_capacity_and_spawner(
        root.path(),
        1,
        input.clone(),
        Arc::clone(&capacity),
        |_, _, _| panic!("refused worker cannot execute"),
        |worker| {
            drop(worker);
            Err(std::io::Error::other("capacity test spawn refusal"))
        },
    )
    .unwrap();
    assert_eq!(admission.project.jobs[0].status, JobStatus::Queued);
    assert_eq!(
        load_split_project(root.path()).unwrap().jobs[0].status,
        JobStatus::Failed
    );
    assert!(matches!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: false,
            ..
        }
    ));
    assert_accepts_last_slot(&capacity);
}

#[test]
fn panicking_worker_releases_reserved_slot_after_terminal_publication() {
    let capacity = Arc::new(WorkerCapacity::new());
    let (root, input) = fixture(12);
    admit(root.path(), 1, input.clone(), &capacity, |_, _, _| {
        panic!("injected worker panic")
    })
    .unwrap();
    // Pin ownership lasts through result publication; waiting for it to leave also
    // verifies the panic was contained by the production worker wrapper.
    drop(BlockedWorkers {
        roots: vec![(root, input)],
        releases: vec![],
    });
    assert_accepts_last_slot(&capacity);
}

#[test]
fn completed_worker_publishes_result_before_releasing_slot() {
    let capacity = Arc::new(WorkerCapacity::new());
    let (root, input) = fixture(13);
    let project_dir = root.path().to_path_buf();
    let result_input = input.clone();
    admit(
        root.path(),
        1,
        input.clone(),
        &capacity,
        move |path, record, _| {
            // This substitutes only encoding. Canonical bookkeeping and durable result
            // publication are real; the fixture is not evidence of native media validity.
            let paths =
                ProjectMediaRenderPaths::new(&record.input.job_id, record.input.profile).unwrap();
            let report = RenderReport {
                job_id: record.input.job_id.clone(),
                summary: super::super::RenderReportSummary {
                    status: "succeeded".into(),
                    duration_seconds: Some(1.0),
                    output_path: Some(paths.output_path.clone()),
                    ..Default::default()
                },
                command: crate::render_pipeline::process::CommandSpec::new("capacity-fixture"),
                stdout: String::new(),
                stderr: String::new(),
                streams: None,
                errors: vec![],
                artifacts: vec![paths.output_path.clone()],
                graphics: vec![],
                performance: None,
                preview_comparison_request: None,
                preview_comparison: None,
            };
            let project_report = project_media_render_report_from_pipeline_report(
                &report,
                &paths,
                &record.input.updated_at,
                &ProjectRenderReportEvidence {
                    video_stream: true,
                    audio_stream: false,
                    audio_required: false,
                    expected_duration_seconds: Some(1.0),
                    duration_tolerance_seconds: Some(0.1),
                },
            )
            .unwrap();
            let write = apply_project_actions_to_split_project(
                &path,
                vec![
                    ProjectAction::UpdateJobStatus {
                        job_id: record.input.job_id.clone(),
                        status: JobStatus::Completed,
                        updated_at: record.input.updated_at,
                        run_id: Some(record.input.attempt_id),
                    },
                    ProjectAction::AttachRenderReport {
                        report: project_report.clone(),
                    },
                ],
            )
            .unwrap();
            Ok(ProjectMediaRenderResult {
                project: write.project,
                render_report: report,
                project_render_report: project_report,
                output_path: paths.output_path,
                export_artifact: None,
            })
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while attempt_is_pinned(root.path(), &input.job_id, &input.attempt_id) {
        assert!(Instant::now() < deadline, "completed worker did not exit");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        attempt_directory(&project_dir, &result_input.job_id, &result_input.attempt_id)
            .join("result.json")
            .is_file()
    );
    let outcome = read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap();
    assert!(matches!(outcome, MediaRenderAttempt::Completed { .. }));
    let result_path =
        attempt_directory(&project_dir, &input.job_id, &input.attempt_id).join("result.json");
    let stored: serde_json::Value = read_record(root.path(), &result_path).unwrap();
    assert_eq!(stored["protocol"], 2);
    assert!(stored["result"].get("project").is_none());
    let legacy_bytes = serde_json::to_vec(&outcome).unwrap().len();
    let compact_bytes = std::fs::metadata(&result_path).unwrap().len();
    println!(
        "completed fixture serialized result bytes: legacy {legacy_bytes}, compact {compact_bytes}"
    );
    assert!(compact_bytes < legacy_bytes as u64);
    // Existing protocol-1 result records remain readable and rehydrate canonical state.
    write_record(root.path(), &result_path, &outcome).unwrap();
    assert_eq!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        outcome
    );
    assert_accepts_last_slot(&capacity);
}

#[test]
fn competing_packages_share_one_atomic_worker_capacity() {
    let capacity = Arc::new(WorkerCapacity::new());
    let barrier = Arc::new(Barrier::new(12));
    let callers = (0..12)
        .map(|index| {
            let capacity = Arc::clone(&capacity);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let (root, input) = fixture(20 + index);
                let (release, released) = mpsc::channel();
                barrier.wait();
                let admission = admit(root.path(), 1, input.clone(), &capacity, move |_, _, _| {
                    let _ = released.recv();
                    Err(render_cancelled_error("capacity.competing"))
                });
                (root, input, release, admission)
            })
        })
        .collect::<Vec<_>>();
    let mut workers = BlockedWorkers {
        roots: vec![],
        releases: vec![],
    };
    let mut accepted = 0;
    let mut rejected = 0;
    for caller in callers {
        let (root, input, release, admission) = caller.join().unwrap();
        match admission {
            Ok(_) => accepted += 1,
            Err(error) => {
                assert_eq!(error.code(), ServiceErrorCode::Busy);
                assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
                assert!(!attempt_directory(root.path(), &input.job_id, &input.attempt_id).exists());
                rejected += 1;
            }
        }
        workers.roots.push((root, input));
        workers.releases.push(release);
    }
    assert_eq!((accepted, rejected), (4, 8));
    drop(workers);
    assert_accepts_last_slot(&capacity);
}
