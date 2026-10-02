use super::*;
use std::time::{Duration, Instant};

fn fixture() -> (tempfile::TempDir, MediaRenderInput) {
    let root = tempfile::tempdir().unwrap();
    let project = VideoProject::new_empty(
        "retention-project".into(),
        "Retention".into(),
        "2026-10-01".into(),
    );
    crate::project::split::save_split_project(root.path(), &project).unwrap();
    let input = MediaRenderInput {
        project_id: project.id,
        profile: ExportProfile::Webm,
        quality: RenderQuality::Draft,
        width: 320,
        height: 180,
        job_id: "retention-job".into(),
        attempt_id: "render-attempt/retention".into(),
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

fn wait_for_exit(root: &Path, input: &MediaRenderInput) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while attempt_is_pinned(root, &input.job_id, &input.attempt_id) {
        assert!(Instant::now() < deadline, "worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn terminal_receipt_drops_snapshot_but_preserves_replay_and_input_conflict() {
    let (root, input) = fixture();
    let (release, wait) = std::sync::mpsc::channel();
    admit_with_executor(root.path(), 1, input.clone(), move |_, _, _| {
        let _ = wait.recv();
        Err(render_cancelled_error("retention.test"))
    })
    .unwrap();
    let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    let before_bytes = std::fs::metadata(dir.join("input.json")).unwrap().len();
    release.send(()).unwrap();
    wait_for_exit(root.path(), &input);
    let after_bytes = std::fs::metadata(dir.join("input.json")).unwrap().len();
    println!("retention fixture serialized input bytes: {before_bytes} -> {after_bytes}; terminal result bytes: {}", std::fs::metadata(dir.join("result.json")).unwrap().len());
    assert!(after_bytes < before_bytes);
    let receipt: serde_json::Value = read_record(root.path(), &dir.join("input.json")).unwrap();
    assert_eq!(receipt["protocol"], 2);
    assert!(
        receipt.get("project").is_none(),
        "terminal receipts retain no immutable project duplicate"
    );
    assert!(receipt["inputFingerprint"].as_str().is_some());
    let replay = admit_with_executor(root.path(), 1, input.clone(), |_, _, _| {
        panic!("replay must not execute")
    });
    assert!(replay.is_ok());
    let mut changed = input.clone();
    changed.width = 640;
    assert!(
        admit_with_executor(root.path(), 1, changed, |_, _, _| panic!(
            "conflict must not execute"
        ))
        .is_err()
    );
    assert!(matches!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed { .. }
    ));
    assert!(
        !pin_path(root.path(), &input.job_id, &input.attempt_id)
            .unwrap()
            .exists(),
        "durable terminal attempts release owned pin files"
    );
}

#[test]
fn attempt_directory_ceiling_rejects_before_durable_acceptance() {
    let (root, input) = fixture();
    let dir = root.path().join("logs/render-admissions");
    std::fs::create_dir_all(&dir).unwrap();
    for index in 0..1_024 {
        std::fs::create_dir(dir.join(format!("old-{index}"))).unwrap();
    }
    let result = admit_with_executor(root.path(), 1, input.clone(), |_, _, _| {
        Err(render_cancelled_error("retention.unaccepted"))
    });
    if result.is_ok() {
        wait_for_exit(root.path(), &input);
    }
    assert_eq!(
        result.unwrap_err().code(),
        crate::app_service::error::ServiceErrorCode::Busy
    );
    assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
    assert!(!attempt_directory(root.path(), &input.job_id, &input.attempt_id).exists());
}

#[test]
fn pending_inputs_and_live_pins_are_preserved() {
    let (root, input) = fixture();
    let (release, wait) = std::sync::mpsc::channel();
    admit_with_executor(root.path(), 1, input.clone(), move |_, _, _| {
        let _ = wait.recv();
        Err(render_cancelled_error("retention.pending"))
    })
    .unwrap();
    let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    let receipt: serde_json::Value = read_record(root.path(), &dir.join("input.json")).unwrap();
    let status = read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap();
    let pinned = pin_path(root.path(), &input.job_id, &input.attempt_id)
        .unwrap()
        .exists();
    release.send(()).unwrap();
    wait_for_exit(root.path(), &input);
    assert_eq!(receipt["protocol"], 1);
    assert!(receipt.get("project").is_some());
    assert_eq!(status, MediaRenderAttempt::Pending);
    assert!(pinned);
}

#[test]
fn oversized_immutable_input_is_rejected_before_a_job_or_pin_exists() {
    let (root, mut input) = fixture();
    input.updated_at = "x".repeat(64 * 1024 * 1024);
    let result = admit_with_executor_and_spawner(
        root.path(),
        1,
        input.clone(),
        |_, _, _| panic!("oversized input must not execute"),
        |worker| {
            drop(worker);
            Err(std::io::Error::other("oversized baseline spawn refusal"))
        },
    );
    assert!(
        matches!(result, Err(ref error) if error.code() == crate::app_service::error::ServiceErrorCode::Busy),
        "oversized input was accepted"
    );
    assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
    assert!(!pin_path(root.path(), &input.job_id, &input.attempt_id)
        .unwrap()
        .exists());
}

#[test]
fn package_metadata_byte_ceiling_preserves_history_and_rejects_new_work() {
    let (root, input) = fixture();
    let dir = root.path().join("logs/render-admissions/foreign-history");
    std::fs::create_dir_all(&dir).unwrap();
    let history = dir.join("preserved.bin");
    std::fs::File::create(&history)
        .unwrap()
        .set_len(512 * 1024 * 1024)
        .unwrap();
    let result = admit_with_executor(root.path(), 1, input.clone(), |_, _, _| {
        Err(render_cancelled_error("retention.package-full"))
    });
    if result.is_ok() {
        wait_for_exit(root.path(), &input);
    }
    assert_eq!(
        result.unwrap_err().code(),
        crate::app_service::error::ServiceErrorCode::Busy
    );
    assert_eq!(std::fs::metadata(history).unwrap().len(), 512 * 1024 * 1024);
    assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
}

#[test]
fn worker_byte_reservations_release_on_error_and_drop() {
    let capacity = std::sync::Arc::new(WorkerCapacity::new());
    let large = capacity.reserve_bytes(256 * 1024 * 1024).unwrap();
    assert!(capacity.reserve_bytes(1).is_err());
    drop(large);
    let first = capacity.reserve_bytes(128 * 1024 * 1024).unwrap();
    let second = capacity.reserve_bytes(128 * 1024 * 1024).unwrap();
    assert!(capacity.reserve_bytes(1).is_err());
    drop(first);
    drop(second);
    assert!(capacity.reserve_bytes(256 * 1024 * 1024).is_ok());
}

#[cfg(unix)]
#[test]
fn terminal_cleanup_preserves_a_replaced_pin_file() {
    let (root, input) = fixture();
    let pin = pin_path(root.path(), &input.job_id, &input.attempt_id).unwrap();
    let replacement_pin = pin.clone();
    let parked = pin.with_extension("parked");
    let worker_parked = parked.clone();
    // Execute synchronously, retaining real admission/bookkeeping and owned pin behavior.
    admit_with_executor_and_spawner(
        root.path(),
        1,
        input.clone(),
        move |_, _, _| {
            std::fs::rename(&replacement_pin, &worker_parked).unwrap();
            std::fs::write(&replacement_pin, b"foreign replacement").unwrap();
            Err(render_cancelled_error("retention.replaced-pin"))
        },
        |worker| {
            worker();
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(std::fs::read(&pin).unwrap(), b"foreign replacement");
    assert!(parked.exists());
    assert!(matches!(
        read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed { .. }
    ));
    std::fs::remove_file(pin).unwrap();
    std::fs::remove_file(parked).unwrap();
}

#[test]
fn compact_receipt_rejects_corruption_and_remains_available_when_history_is_full() {
    let (root, input) = fixture();
    admit_with_executor_and_spawner(
        root.path(),
        1,
        input.clone(),
        |_, _, _| Err(render_cancelled_error("retention.receipt")),
        |worker| {
            worker();
            Ok(())
        },
    )
    .unwrap();
    let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    for index in 0..1_023 {
        std::fs::create_dir(
            root.path()
                .join(format!("logs/render-admissions/old-{index}")),
        )
        .unwrap();
    }
    assert!(
        admit_with_executor(root.path(), 1, input.clone(), |_, _, _| panic!(
            "full history replay must not execute"
        ))
        .is_ok()
    );
    let mut receipt: serde_json::Value = read_record(root.path(), &dir.join("input.json")).unwrap();
    receipt["sourceRevision"] = serde_json::json!(999);
    write_record(root.path(), &dir.join("input.json"), &receipt).unwrap();
    assert!(
        admit_with_executor(root.path(), 999, input, |_, _, _| panic!(
            "corrupt receipt must not execute"
        ))
        .is_err()
    );
}

#[test]
fn missing_input_receipt_does_not_make_an_accepted_attempt_new_again() {
    let (root, input) = fixture();
    admit_with_executor_and_spawner(
        root.path(),
        1,
        input.clone(),
        |_, _, _| Err(render_cancelled_error("retention.missing")),
        |worker| {
            worker();
            Ok(())
        },
    )
    .unwrap();
    std::fs::remove_file(
        attempt_directory(root.path(), &input.job_id, &input.attempt_id).join("input.json"),
    )
    .unwrap();
    let before = load_split_project(root.path()).unwrap();
    let result = admit_with_executor_and_spawner(
        root.path(),
        before.content_revision,
        input,
        |_, _, _| Err(render_cancelled_error("retention.must-not-repeat")),
        |worker| {
            worker();
            Ok(())
        },
    );
    assert!(
        result.is_err(),
        "missing accepted history must fail closed instead of executing another worker"
    );
    assert_eq!(load_split_project(root.path()).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn new_admission_preserves_an_existing_foreign_pin() {
    let (root, input) = fixture();
    let pin = pin_path(root.path(), &input.job_id, &input.attempt_id).unwrap();
    std::fs::write(&pin, b"foreign existing pin").unwrap();
    let result = admit_with_executor_and_spawner(
        root.path(),
        1,
        input,
        |_, _, _| Err(render_cancelled_error("retention.foreign-pin")),
        |worker| {
            worker();
            Ok(())
        },
    );
    assert!(
        result.is_err(),
        "a new admission must not adopt a foreign pin"
    );
    assert_eq!(std::fs::read(&pin).unwrap(), b"foreign existing pin");
    assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
    std::fs::remove_file(pin).unwrap();
}

#[cfg(unix)]
#[test]
fn failed_unaccepted_record_creation_removes_only_its_new_pin() {
    use std::os::unix::fs::PermissionsExt;
    let (root, input) = fixture();
    let logs = root.path().join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::set_permissions(&logs, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = admit_with_executor(root.path(), 1, input.clone(), |_, _, _| {
        panic!("unaccepted write must not execute")
    });
    std::fs::set_permissions(logs, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(!attempt_directory(root.path(), &input.job_id, &input.attempt_id).exists());
    assert!(
        !pin_path(root.path(), &input.job_id, &input.attempt_id)
            .unwrap()
            .exists(),
        "unaccepted metadata failure must not leak a pin inode"
    );
    assert!(load_split_project(root.path()).unwrap().jobs.is_empty());
}

#[cfg(unix)]
#[test]
fn terminal_compaction_refuses_a_replacement_package_even_with_matching_run_ids() {
    let (root, input) = fixture();
    let captured = PackageIdentity::capture(root.path(), &input.project_id).unwrap();
    let pin = AdmissionPin::acquire(root.path(), &input.job_id, &input.attempt_id).unwrap();
    let parked = tempfile::tempdir().unwrap();
    std::fs::rename(root.path(), parked.path().join("original")).unwrap();
    std::fs::create_dir(root.path()).unwrap();
    let mut project = VideoProject::new_empty(
        input.project_id.clone(),
        "Replacement".into(),
        input.updated_at.clone(),
    );
    let mut job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &input.project_id,
        &input.job_id,
        JobStatus::Failed,
        &input.updated_at,
    );
    job.workflow.as_mut().unwrap().run_id = Some(input.attempt_id.clone());
    project.jobs.push(job);
    let project = crate::project::split::save_split_project(root.path(), &project)
        .unwrap()
        .project;
    let replacement_identity = PackageIdentity::capture(root.path(), &input.project_id).unwrap();
    assert_ne!(captured, replacement_identity);
    let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    write_input(
        root.path(),
        &dir,
        &DurableInput {
            identity: replacement_identity.clone(),
            protocol: 1,
            source_revision: project.content_revision,
            input: input.clone(),
            project,
        },
    )
    .unwrap();
    retention::write_result(
        root.path(),
        &dir.join("result.json"),
        &failed("Replacement outcome", false),
        &replacement_identity,
    )
    .unwrap();
    let original_bytes = std::fs::read(dir.join("input.json")).unwrap();
    let mutation =
        crate::project::mutation::acquire_split_project_mutation_lease(root.path()).unwrap();
    let result = retention::compact_terminal(
        root.path(),
        &captured,
        &input.job_id,
        &input.attempt_id,
        Some(&pin),
        &mutation,
    );
    drop(mutation);
    drop(pin);
    let pin_path = pin_path(root.path(), &input.job_id, &input.attempt_id).unwrap();
    std::fs::remove_file(pin_path).unwrap();
    assert!(
        result.is_err(),
        "a finished worker must not compact replacement history"
    );
    assert_eq!(
        std::fs::read(dir.join("input.json")).unwrap(),
        original_bytes
    );
}

thread_local! {
    static AFTER_DIRECTORY_CHARGE: std::cell::Cell<Option<fn(&Path)>> = const { std::cell::Cell::new(None) };
}
pub(super) fn after_directory_charge(path: &Path) {
    AFTER_DIRECTORY_CHARGE.with(|hook| {
        if let Some(callback) = hook.take() {
            callback(path);
        }
    });
}

fn nearly_full_pending_history(root: &Path) -> PathBuf {
    let dir = root.join("logs/render-admissions/old-attempt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("input.json"), b"{}").unwrap();
    std::fs::File::create(dir.join("retained.bin"))
        .unwrap()
        .set_len(447 * 1024 * 1024)
        .unwrap();
    dir
}
fn assert_new_attempt_budget_rejected(root: &Path, input: MediaRenderInput) {
    let result = admit_with_executor_and_spawner(
        root,
        1,
        input,
        |_, _, _| panic!("unaccepted work"),
        |worker| {
            drop(worker);
            Err(std::io::Error::other("budget baseline refused spawn"))
        },
    );
    assert!(
        matches!(result, Err(ref error) if error.code() == crate::app_service::error::ServiceErrorCode::Busy),
        "future result reservation was omitted"
    );
    assert!(load_split_project(root).unwrap().jobs.is_empty());
}
#[test]
fn result_publication_after_directory_charge_keeps_future_result_reservation() {
    let (root, input) = fixture();
    nearly_full_pending_history(root.path());
    AFTER_DIRECTORY_CHARGE.with(|hook| {
        hook.set(Some(|dir| {
            std::fs::File::create(dir.join("result.json"))
                .unwrap()
                .set_len(64 * 1024 * 1024)
                .unwrap();
        }))
    });
    assert_new_attempt_budget_rejected(root.path(), input);
}
#[test]
fn legacy_pending_result_keeps_future_result_reservation() {
    let (root, input) = fixture();
    let dir = nearly_full_pending_history(root.path());
    std::fs::write(dir.join("result.json"), br#"{"status":"pending"}"#).unwrap();
    assert_new_attempt_budget_rejected(root.path(), input);
}
#[cfg(unix)]
#[test]
fn explicit_terminal_recovery_compacts_crash_left_snapshot_and_owned_pin() {
    let (root, input) = fixture();
    let original = DurableInput {
        identity: PackageIdentity::capture(root.path(), &input.project_id).unwrap(),
        protocol: 1,
        source_revision: 1,
        input: input.clone(),
        project: load_split_project(root.path()).unwrap(),
    };
    admit_with_executor_and_spawner(
        root.path(),
        1,
        input.clone(),
        |_, _, _| Err(render_cancelled_error("retention.crash")),
        |worker| {
            worker();
            Ok(())
        },
    )
    .unwrap();
    let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
    write_input(root.path(), &dir, &original).unwrap();
    let pin = AdmissionPin::acquire(root.path(), &input.job_id, &input.attempt_id).unwrap();
    let mut identity: DurableAttemptIdentity =
        read_record(root.path(), &dir.join("identity.json")).unwrap();
    identity.pin_identity = Some(pin.identity().unwrap());
    write_record_with_identity(
        root.path(),
        &dir.join("identity.json"),
        &identity,
        Some(&identity.identity),
    )
    .unwrap();
    let snapshot = std::fs::read(dir.join("input.json")).unwrap();
    assert_eq!(
        recover_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Pending
    );
    assert_eq!(std::fs::read(dir.join("input.json")).unwrap(), snapshot);
    drop(pin);
    let terminal = std::fs::read(dir.join("result.json")).unwrap();
    std::fs::write(dir.join("result.json"), b"malformed crash result").unwrap();
    assert!(matches!(
        recover_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed { .. }
    ));
    assert_eq!(std::fs::read(dir.join("input.json")).unwrap(), snapshot);
    assert!(pin_path(root.path(), &input.job_id, &input.attempt_id)
        .unwrap()
        .exists());
    std::fs::write(dir.join("result.json"), terminal).unwrap();
    assert!(matches!(
        recover_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
        MediaRenderAttempt::Failed { .. }
    ));
    let receipt: serde_json::Value = read_record(root.path(), &dir.join("input.json")).unwrap();
    assert_eq!(
        receipt["protocol"], 2,
        "terminal recovery retained the crash-left snapshot"
    );
    assert!(receipt.get("project").is_none());
    assert!(!pin_path(root.path(), &input.job_id, &input.attempt_id)
        .unwrap()
        .exists());
}
