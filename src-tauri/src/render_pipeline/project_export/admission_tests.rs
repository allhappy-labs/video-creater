//! Regression cases found by independent admission review.
use super::*;
use crate::render_pipeline::report::{RenderReportStreams as PipelineStreams, RenderReportSummary};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn fixture() -> (tempfile::TempDir, MediaRenderInput) {
    let root = tempfile::tempdir().unwrap();
    let project = VideoProject::new_empty(
        "review-project".into(),
        "Review".into(),
        "2026-10-01".into(),
    );
    crate::project::split::save_split_project(root.path(), &project).unwrap();
    let input = MediaRenderInput {
        project_id: project.id,
        profile: ExportProfile::Webm,
        quality: RenderQuality::Draft,
        width: 1280,
        height: 720,
        job_id: "review-job".into(),
        attempt_id: "render-attempt/review-a".into(),
        updated_at: "2026-10-01T12:00:00Z".into(),
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

fn wait_for_worker_exit(root: &Path, input: &MediaRenderInput) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while attempt_is_pinned(root, &input.job_id, &input.attempt_id) {
        assert!(
            Instant::now() < deadline,
            "worker must finish the test outcome"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn old_worker_failure_preserves_a_newer_queued_attempt() {
    let (root, input) = fixture();
    let revision = load_split_project(root.path()).unwrap().content_revision;
    let next_attempt = "render-attempt/review-b";
    let (changed_tx, changed_rx) = mpsc::channel();
    admit_with_executor(
        root.path(),
        revision,
        input.clone(),
        move |path, record, _attempt| {
            apply_project_actions_to_split_project(
                &path,
                vec![ProjectAction::UpdateJobStatus {
                    job_id: record.input.job_id.clone(),
                    status: JobStatus::Cancelled,
                    updated_at: record.input.updated_at.clone(),
                    run_id: Some(record.input.attempt_id.clone()),
                }],
            )
            .unwrap();
            apply_project_actions_to_split_project(
                &path,
                vec![ProjectAction::UpdateJobStatus {
                    job_id: record.input.job_id,
                    status: JobStatus::Queued,
                    updated_at: record.input.updated_at,
                    run_id: Some(next_attempt.into()),
                }],
            )
            .unwrap();
            changed_tx.send(()).unwrap();
            Err(render_cancelled_error("review.old-worker"))
        },
    )
    .unwrap();
    changed_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    wait_for_worker_exit(root.path(), &input);
    let after = load_split_project(root.path()).unwrap();
    assert_eq!(
        after.jobs[0].status,
        JobStatus::Queued,
        "an old failure cannot fail the retry"
    );
    assert_eq!(
        after.jobs[0].workflow.as_ref().unwrap().run_id.as_deref(),
        Some(next_attempt)
    );
}

fn complete_fake_render(path: &Path, input: &MediaRenderInput) -> ProjectMediaRenderResult {
    let paths = ProjectMediaRenderPaths::new(&input.job_id, input.profile).unwrap();
    std::fs::create_dir_all(paths.absolute_render_dir(path)).unwrap();
    std::fs::write(paths.absolute_output_path(path), b"validated test fixture").unwrap();
    let render_report = RenderReport {
        job_id: input.job_id.clone(),
        summary: RenderReportSummary {
            // Match the real renderer's pipeline report; the canonical project report
            // separately derives its "ready" preview status from validated evidence.
            status: "succeeded".into(),
            duration_seconds: Some(1.0),
            output_path: Some(paths.output_path.clone()),
            ..Default::default()
        },
        command: crate::render_pipeline::process::CommandSpec::new("review-test"),
        stdout: String::new(),
        stderr: String::new(),
        streams: Some(PipelineStreams {
            video: true,
            audio: true,
        }),
        errors: Vec::new(),
        artifacts: vec![paths.output_path.clone()],
        graphics: Vec::new(),
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };
    write_json_report(&paths.absolute_json_report_path(path), &render_report).unwrap();
    let project_report = project_media_render_report_from_pipeline_report(
        &render_report,
        &paths,
        &input.updated_at,
        &ProjectRenderReportEvidence {
            video_stream: true,
            audio_stream: true,
            audio_required: true,
            expected_duration_seconds: Some(1.0),
            duration_tolerance_seconds: Some(0.1),
        },
    )
    .unwrap();
    let write = apply_project_actions_to_split_project(
        path,
        vec![
            ProjectAction::UpdateJobStatus {
                job_id: input.job_id.clone(),
                status: JobStatus::Completed,
                updated_at: input.updated_at.clone(),
                run_id: Some(input.attempt_id.clone()),
            },
            ProjectAction::AttachRenderReport {
                report: project_report.clone(),
            },
        ],
    )
    .unwrap();
    ProjectMediaRenderResult {
        project: write.project,
        render_report,
        project_render_report: project_report,
        output_path: paths.output_path,
        export_artifact: None,
    }
}

#[cfg(unix)]
#[test]
fn result_record_write_failure_keeps_the_committed_completed_outcome() {
    use std::os::unix::fs::PermissionsExt;
    let (root, input) = fixture();
    let revision = load_split_project(root.path()).unwrap().content_revision;
    let record_dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    admit_with_executor(
        root.path(),
        revision,
        input.clone(),
        |path, record, _attempt| {
            let result = complete_fake_render(&path, &record.input);
            let dir = attempt_directory(&path, &record.input.job_id, &record.input.attempt_id);
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o500)).unwrap();
            Ok(result)
        },
    )
    .unwrap();
    wait_for_worker_exit(root.path(), &input);
    std::fs::set_permissions(&record_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        !record_dir.join("result.json").exists(),
        "force the result-record failure"
    );
    assert_eq!(
        load_split_project(root.path()).unwrap().jobs[0].status,
        JobStatus::Completed
    );
    assert!(
        matches!(
            read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
            MediaRenderAttempt::Completed { .. }
        ),
        "a committed render cannot become a false failed outcome"
    );
}

#[test]
fn host_restart_terminalizes_an_unpinned_committed_queue_before_retry() {
    let (root, input) = fixture();
    let before = load_split_project(root.path()).unwrap();
    let mut job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &input.project_id,
        &input.job_id,
        JobStatus::Queued,
        &input.updated_at,
    );
    job.workflow.as_mut().unwrap().run_id = Some(input.attempt_id.clone());
    write_record(
        root.path(),
        &attempt_directory(root.path(), &input.job_id, &input.attempt_id).join("input.json"),
        &DurableInput {
            identity: PackageIdentity::capture(root.path(), &before.id).unwrap(),
            protocol: 1,
            source_revision: before.content_revision,
            input: input.clone(),
            project: before,
        },
    )
    .unwrap();
    apply_project_actions_to_split_project(
        root.path(),
        vec![ProjectAction::RecordJob { job: Box::new(job) }],
    )
    .unwrap();
    assert!(matches!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed { .. }
    ));
    assert_eq!(
        load_split_project(root.path()).unwrap().jobs[0].status,
        JobStatus::Queued,
        "status polling is read-only"
    );
    assert!(matches!(
        recover_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: true,
            ..
        }
    ));
    let after = load_split_project(root.path()).unwrap();
    assert_eq!(
        after.jobs[0].status,
        JobStatus::Failed,
        "a stopped host must not leave a permanently busy queued job"
    );
    let mut retry = input.clone();
    retry.attempt_id = "render-attempt/review-retry".into();
    admit_with_executor(
        root.path(),
        after.content_revision,
        retry.clone(),
        |_, _, _| Err(render_cancelled_error("review.retry")),
    )
    .expect("fresh retry must be admitted after terminal recovery");
    wait_for_worker_exit(root.path(), &retry);
}

#[test]
fn worker_does_not_mutate_a_replacement_package_with_the_same_job_and_attempt() {
    let (root, input) = fixture();
    let revision = load_split_project(root.path()).unwrap().content_revision;
    let (replacement_tx, replacement_rx) = mpsc::channel();
    admit_with_executor(
        root.path(),
        revision,
        input.clone(),
        move |path, record, attempt| {
            let replacement_dir = tempfile::tempdir_in(path.parent().unwrap()).unwrap();
            let parked_dir = tempfile::tempdir_in(path.parent().unwrap()).unwrap();
            let mut replacement = load_split_project(&path).unwrap();
            replacement.name = "Replacement".into();
            let expected =
                crate::project::split::save_split_project(replacement_dir.path(), &replacement)
                    .unwrap()
                    .project;
            std::fs::rename(&path, parked_dir.path()).unwrap();
            std::fs::rename(replacement_dir.path(), &path).unwrap();
            replacement_tx.send(expected).unwrap();
            execute_admitted(path, record, attempt)
        },
    )
    .unwrap();
    let expected = replacement_rx
        .recv_timeout(Duration::from_secs(30))
        .unwrap();
    wait_for_worker_exit(root.path(), &input);
    assert_eq!(
        load_split_project(root.path()).unwrap(),
        expected,
        "the old attempt cannot change jobs or revisions in a replacement package"
    );
}

#[cfg(unix)]
#[test]
fn committed_admission_ack_survives_spawn_and_result_write_failure() {
    use std::os::unix::fs::PermissionsExt;
    let (root, input) = fixture();
    let before = load_split_project(root.path()).unwrap();
    let record_dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    let admission = admit_with_executor_and_spawner(
        root.path(),
        before.content_revision,
        input.clone(),
        |_, _, _| panic!("refused worker must not execute"),
        |worker| {
            drop(worker);
            std::fs::set_permissions(&record_dir, std::fs::Permissions::from_mode(0o500)).unwrap();
            Err(std::io::Error::other("deterministic spawn refusal"))
        },
    );
    std::fs::set_permissions(&record_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let acknowledged = admission.expect("durable queued commit must be acknowledged");
    assert_eq!(acknowledged.project.jobs[0].status, JobStatus::Queued);
    assert_eq!(
        load_split_project(root.path()).unwrap().jobs[0].status,
        JobStatus::Failed
    );
    assert!(!record_dir.join("result.json").exists());
    assert!(matches!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: false,
            ..
        }
    ));
}

#[cfg(unix)]
#[test]
fn refused_spawn_never_writes_into_a_replaced_package() {
    let (root, input) = fixture();
    let before = load_split_project(root.path()).unwrap();
    let parked = tempfile::tempdir().unwrap();
    let admission = admit_with_executor_and_spawner(
        root.path(),
        before.content_revision,
        input,
        |_, _, _| panic!("refused worker must not execute"),
        |worker| {
            drop(worker);
            std::fs::rename(root.path(), parked.path().join("original")).unwrap();
            std::fs::create_dir(root.path()).unwrap();
            crate::project::split::save_split_project(root.path(), &before).unwrap();
            Err(std::io::Error::other("deterministic spawn refusal"))
        },
    )
    .unwrap();
    assert_eq!(admission.project.jobs[0].status, JobStatus::Queued);
    assert!(
        !root.path().join("logs/render-admissions").exists(),
        "old spawn failure must not write into a new package"
    );
    assert_eq!(load_split_project(root.path()).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn opened_record_parent_rejects_nested_directory_replacement() {
    let (root, input) = fixture();
    let record_path =
        attempt_directory(root.path(), &input.job_id, &input.attempt_id).join("identity.json");
    write_record(root.path(), &record_path, &true).unwrap();
    let parent = RecordParent::open(root.path(), &record_path, false).unwrap();
    let directory = record_path.parent().unwrap();
    let parked = directory.with_extension("parked");
    std::fs::rename(directory, &parked).unwrap();
    std::fs::create_dir(directory).unwrap();
    assert!(
        parent.validate().is_err(),
        "a held parent fd cannot authorize a new pathname parent"
    );
}

#[cfg(unix)]
#[test]
fn record_reads_reject_leaf_and_ancestor_symlinks() {
    use std::os::unix::fs::symlink;
    let (root, input) = fixture();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("record.json");
    std::fs::write(&secret, b"true").unwrap();
    let record_path =
        attempt_directory(root.path(), &input.job_id, &input.attempt_id).join("identity.json");
    std::fs::create_dir_all(record_path.parent().unwrap()).unwrap();
    symlink(&secret, &record_path).unwrap();
    assert!(read_record::<bool>(root.path(), &record_path).is_err());
    std::fs::remove_dir_all(root.path().join("logs/render-admissions")).unwrap();
    symlink(outside.path(), root.path().join("logs/render-admissions")).unwrap();
    assert!(read_record::<bool>(root.path(), &record_path).is_err());
    assert!(write_record(root.path(), &record_path, &false).is_err());
    assert_eq!(std::fs::read(&secret).unwrap(), b"true");
}

#[test]
fn pending_status_does_not_load_the_snapshot_or_wait_for_mutation_ownership() {
    let (root, input) = fixture();
    let before = load_split_project(root.path()).unwrap();
    let (finish_tx, finish_rx) = mpsc::channel();
    admit_with_executor(
        root.path(),
        before.content_revision,
        input.clone(),
        move |_, _, _| {
            finish_rx.recv().unwrap();
            Err(render_cancelled_error("review.pending"))
        },
    )
    .unwrap();
    let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    std::fs::write(dir.join("input.json"), b"broken full immutable snapshot").unwrap();
    let held_path = root.path().to_path_buf();
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let holder = std::thread::spawn(move || {
        let _lease =
            crate::project::mutation::acquire_split_project_mutation_lease(&held_path).unwrap();
        held_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    held_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    let read_path = root.path().to_path_buf();
    let query = input.clone();
    let (status_tx, status_rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        status_tx
            .send(read_media_render_attempt(
                &read_path,
                &query.job_id,
                &query.attempt_id,
            ))
            .unwrap();
    });
    let status = status_rx.recv_timeout(Duration::from_millis(500));
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    reader.join().unwrap();
    finish_tx.send(()).unwrap();
    wait_for_worker_exit(root.path(), &input);
    assert_eq!(
        status
            .expect("pending reads cannot wait for mutation ownership")
            .unwrap(),
        MediaRenderAttempt::Pending
    );
}

#[cfg(unix)]
#[test]
#[ignore = "child-process half of recovery_observes_a_cross_process_admission_pin"]
fn cross_process_pin_holder() {
    use std::io::Read;
    let root =
        PathBuf::from(std::env::var_os("VIDEO_CREATER_ADMISSION_PIN_HELPER_PROJECT").unwrap());
    let _pin = AdmissionPin::acquire(&root, "review-job", "render-attempt/review-a").unwrap();
    std::fs::write(root.join("child-pinned"), b"ready").unwrap();
    std::io::stdin().read_exact(&mut [0u8]).unwrap();
}

#[cfg(unix)]
#[test]
fn recovery_observes_a_cross_process_admission_pin() {
    use std::process::{Command, Stdio};
    let (root, input) = fixture();
    let before = load_split_project(root.path()).unwrap();
    let mut job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &input.project_id,
        &input.job_id,
        JobStatus::Queued,
        &input.updated_at,
    );
    job.workflow.as_mut().unwrap().run_id = Some(input.attempt_id.clone());
    write_input(
        root.path(),
        &attempt_directory(root.path(), &input.job_id, &input.attempt_id),
        &DurableInput {
            identity: PackageIdentity::capture(root.path(), &before.id).unwrap(),
            protocol: 1,
            source_revision: before.content_revision,
            input: input.clone(),
            project: before,
        },
    )
    .unwrap();
    apply_project_actions_to_split_project(
        root.path(),
        vec![ProjectAction::RecordJob { job: Box::new(job) }],
    )
    .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "render_pipeline::project_export::admission::review_tests::cross_process_pin_holder",
        ])
        .env("VIDEO_CREATER_ADMISSION_PIN_HELPER_PROJECT", root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !root.path().join("child-pinned").exists() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("child must acquire its OS pin");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        recover_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Pending
    );
    assert_eq!(
        load_split_project(root.path()).unwrap().jobs[0].status,
        JobStatus::Queued
    );
    child.stdin.take().unwrap().write_all(b"x").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(matches!(
        recover_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: true,
            ..
        }
    ));
    assert!(matches!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed {
            interrupted: true,
            ..
        }
    ));
}
