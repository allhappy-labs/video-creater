use super::*;
use serde_json::json;
use tempfile::tempdir;

fn request(id: &str) -> RpcEnvelope {
    RpcEnvelope {
        request_id: id.into(),
        operation: "remote_create_project".into(),
        project_id: None,
        expected_revision: None,
        editor_lease_token: None,
        payload: json!({}),
    }
}
fn begin(
    store: &RequestOutcomeStore,
    id: &str,
    now: u64,
) -> Result<OutcomeAdmission, OutcomeError> {
    store.begin(
        "owner@example.test",
        &request(id),
        b"private-body-not-retained",
        now,
    )
}

#[test]
fn pending_survives_real_reopen_as_interrupted_and_never_new() {
    let root = tempdir().unwrap();
    let path = root.path().join("outcomes");
    let store = RequestOutcomeStore::open(&path, 100).unwrap();
    assert!(matches!(
        begin(&store, "legacy", 100),
        Ok(OutcomeAdmission::New { .. })
    ));
    assert_eq!(
        store.acknowledge("owner@example.test", "legacy"),
        Err(OutcomeError::Pending)
    );
    drop(store);
    let reopened = RequestOutcomeStore::open(&path, 101).unwrap();
    assert!(
        matches!(begin(&reopened,"legacy",101),Ok(OutcomeAdmission::Existing(outcome)) if outcome.status==OutcomeStatus::Interrupted)
    );
    assert_eq!(
        reopened
            .lookup("owner@example.test", "legacy")
            .unwrap()
            .status,
        OutcomeStatus::Interrupted
    );
}

#[test]
fn trusted_principal_and_exact_fingerprint_are_independent_boundaries() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    begin(&store, "original", 100).unwrap();
    assert!(matches!(
        store.begin("owner@example.test", &request("original"), b"changed", 100),
        Err(OutcomeError::Conflict)
    ));
    assert_eq!(
        store.lookup("other@example.test", "original").unwrap_err(),
        OutcomeError::NotFound
    );
    assert!(store
        .outstanding("other@example.test", None)
        .unwrap()
        .is_empty());
    assert!(matches!(
        store.begin("other@example.test", &request("original"), b"changed", 100),
        Ok(OutcomeAdmission::New { .. })
    ));
}

#[test]
fn new_format_expiry_never_converts_retained_or_pruned_requests_to_new() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    let id = "browser-v2-100-receipt";
    begin(&store, id, 100).unwrap();
    store
        .finish(
            "owner@example.test",
            id,
            &RpcResponse::success(id, json!({})),
        )
        .unwrap();
    assert!(matches!(
        begin(&store, id, 100 + RETENTION_SECONDS + 1),
        Ok(OutcomeAdmission::Existing(_))
    ));
    store.acknowledge("owner@example.test", id).unwrap();
    assert!(matches!(
        begin(&store, id, 100 + RETENTION_SECONDS + 62),
        Err(OutcomeError::Expired)
    ));
    assert_eq!(
        store.lookup("owner@example.test", id).unwrap_err(),
        OutcomeError::NotFound
    );
    for invalid in [
        "browser-v2-bad-receipt",
        "browser-v2-100-",
        "browser-v2-100999999-future",
    ] {
        assert!(
            matches!(begin(&store, invalid, 100), Err(OutcomeError::Expired)),
            "{invalid}"
        );
    }
}

#[test]
fn legacy_and_acknowledged_interruptions_are_never_silently_evicted() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    begin(&store, "legacy", 100).unwrap();
    store
        .finish(
            "owner@example.test",
            "legacy",
            &RpcResponse::success("legacy", json!({})),
        )
        .unwrap();
    store.acknowledge("owner@example.test", "legacy").unwrap();
    begin(&store, "browser-v2-100-unknown", 100).unwrap();
    drop(store);
    let store = RequestOutcomeStore::open(root.path(), 101).unwrap();
    store
        .acknowledge("owner@example.test", "browser-v2-100-unknown")
        .unwrap();
    store.maintain(100 + RETENTION_SECONDS + 100).unwrap();
    assert_eq!(
        store.lookup("owner@example.test", "legacy").unwrap().status,
        OutcomeStatus::Succeeded
    );
    assert_eq!(
        store
            .lookup("owner@example.test", "browser-v2-100-unknown")
            .unwrap()
            .status,
        OutcomeStatus::Interrupted
    );
}

#[test]
fn reservations_reject_new_work_but_allow_accepted_outcome_and_replay() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    store.state.lock().unwrap().bytes = MAX_BYTES - MAX_RECORD_BYTES;
    begin(&store, "reserved", 100).unwrap();
    assert_eq!(store.state.lock().unwrap().bytes, MAX_BYTES);
    assert!(matches!(
        begin(&store, "unaccepted", 100),
        Err(OutcomeError::Capacity)
    ));
    assert!(matches!(
        begin(&store, "reserved", 100),
        Ok(OutcomeAdmission::Existing(_))
    ));
    store.finish("owner@example.test","reserved",&RpcResponse::success("reserved",json!({"catalogProjectId":"catalog","project":{"id":"canonical","contentRevision":4},"secret":"discard"}))).unwrap();
    let outcome = store.lookup("owner@example.test", "reserved").unwrap();
    assert_eq!(outcome.content_revision, Some(4));
    assert_eq!(outcome.canonical_project_id.as_deref(), Some("canonical"));
    assert!(store.state.lock().unwrap().bytes < MAX_BYTES);
}

#[test]
fn journal_contains_only_compact_credential_free_metadata() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    begin(&store, "opaque", 100).unwrap();
    store.finish("owner@example.test","opaque",&RpcResponse::success("opaque",json!({"privatePath":"/private/media.mov","project":{"id":"canonical","contentRevision":3,"media":["secret"]}}))).unwrap();
    let file = std::fs::read_dir(root.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(file).unwrap();
    for private in [
        "private-body-not-retained",
        "owner@example.test",
        "privatePath",
        "/private",
        "secret",
        "editorLeaseToken",
        "csrfToken",
    ] {
        assert!(!text.contains(private), "persisted {private}");
    }
}

#[test]
fn a_second_live_owner_cannot_turn_pending_into_interrupted() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    begin(&store, "active", 100).unwrap();
    assert!(matches!(
        RequestOutcomeStore::open(root.path(), 101),
        Err(OutcomeError::Unavailable)
    ));
    assert_eq!(
        store.lookup("owner@example.test", "active").unwrap().status,
        OutcomeStatus::Pending
    );
}

#[test]
fn corrupt_and_oversized_records_fail_closed_and_are_preserved() {
    for contents in [b"bad".to_vec(), vec![b'x'; MAX_RECORD_BYTES + 1]] {
        let root = tempdir().unwrap();
        let file = root.path().join(format!("{}.json", "a".repeat(64)));
        std::fs::write(&file, &contents).unwrap();
        assert!(matches!(
            RequestOutcomeStore::open(root.path(), 100),
            Err(OutcomeError::Unavailable)
        ));
        assert_eq!(std::fs::read(file).unwrap(), contents);
    }
}

#[cfg(unix)]
#[test]
fn journal_and_receipt_symlinks_are_refused_without_following() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let linked = root.path().join("linked");
    symlink(&outside, &linked).unwrap();
    assert!(matches!(
        RequestOutcomeStore::open(&linked, 100),
        Err(OutcomeError::Unavailable)
    ));
    let file = outside.join(format!("{}.json", "a".repeat(64)));
    let user_file = root.path().join("user");
    std::fs::write(&user_file, b"preserve").unwrap();
    symlink(&user_file, &file).unwrap();
    assert!(matches!(
        RequestOutcomeStore::open(&outside, 100),
        Err(OutcomeError::Unavailable)
    ));
    assert_eq!(std::fs::read(&user_file).unwrap(), b"preserve");
}

#[test]
fn concurrent_cross_session_admission_has_one_durable_winner() {
    let root = tempdir().unwrap();
    let store = std::sync::Arc::new(RequestOutcomeStore::open(root.path(), 100).unwrap());
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            std::thread::spawn(move || {
                matches!(
                    begin(&store, "one", 100).unwrap(),
                    OutcomeAdmission::New { .. }
                )
            })
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|won| *won)
            .count(),
        1
    );
}

struct SuccessfulDispatcher;
impl crate::web_host::rpc::RpcDispatcher for SuccessfulDispatcher {
    fn dispatch(&self, _request: &RpcEnvelope) -> Result<serde_json::Value, String> {
        Ok(json!({"cancelled":true}))
    }
}
fn cancellation(engine: &crate::web_host::rpc::RpcEngine, id: &str) -> RpcResponse {
    let mut request = request(id);
    request.operation = "cancel_render_job_in_split_project_folder".into();
    request.project_id = Some("opaque-project".into());
    request.editor_lease_token = Some("lease".into());
    request.expected_revision = Some(1);
    let scopes = std::collections::BTreeSet::from([
        crate::app_service::operation::AuthorizationScope::Session,
        crate::app_service::operation::AuthorizationScope::ProjectWrite,
    ]);
    engine.execute_for_principal(
        "session",
        "owner@example.test",
        &scopes,
        &serde_json::to_vec(&request).unwrap(),
        100,
    )
}
#[test]
fn cancellation_has_durable_recovery_without_losing_its_reserved_lane() {
    let root = tempdir().unwrap();
    let store = std::sync::Arc::new(RequestOutcomeStore::open(root.path(), 100).unwrap());
    let engine = crate::web_host::rpc::RpcEngine::with_outcome_store(
        std::sync::Arc::new(SuccessfulDispatcher),
        store.clone(),
    );
    assert!(cancellation(&engine, "stop").ok);
    assert_eq!(
        store
            .lookup("owner@example.test", "stop")
            .expect("accepted Stop must have a durable outcome")
            .status,
        OutcomeStatus::Succeeded
    );
}
#[test]
fn full_ordinary_journal_does_not_starve_durable_stop_receipts() {
    let root = tempdir().unwrap();
    let store = std::sync::Arc::new(RequestOutcomeStore::open(root.path(), 100).unwrap());
    store.state.lock().unwrap().bytes = MAX_BYTES;
    let engine = crate::web_host::rpc::RpcEngine::with_outcome_store(
        std::sync::Arc::new(SuccessfulDispatcher),
        store.clone(),
    );
    assert!(cancellation(&engine, "reserved-stop").ok);
    assert!(store.lookup("owner@example.test", "reserved-stop").is_ok());
    assert!(matches!(
        begin(&store, "ordinary-overflow", 100),
        Err(OutcomeError::Capacity)
    ));
}

#[test]
fn hashed_legacy_request_id_domain_remains_wire_compatible() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    let id = "legacy:request/ä with space";
    assert!(matches!(
        begin(&store, id, 100),
        Ok(OutcomeAdmission::New { .. })
    ));
    drop(store);
    let store = RequestOutcomeStore::open(root.path(), 101).unwrap();
    assert_eq!(
        store.lookup("owner@example.test", id).unwrap().status,
        OutcomeStatus::Interrupted
    );
}

#[test]
fn expired_absence_attestation_and_ack_require_strict_age_and_supported_identity() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    let id = "browser-v2-100-never-accepted";
    let aged = 100 + RETENTION_SECONDS + 1;
    assert!(!store.replay_fenced_absent(id, aged - 1).unwrap());
    assert_eq!(
        store.acknowledge_at("owner@example.test", id, aged - 1),
        Err(OutcomeError::NotFound)
    );
    assert!(store.replay_fenced_absent(id, aged).unwrap());
    assert!(store.acknowledge_at("owner@example.test", id, aged).is_ok());
    assert_eq!(
        store.acknowledge("owner@example.test", id),
        Err(OutcomeError::NotFound)
    );
    assert!(store
        .outstanding("owner@example.test", None)
        .unwrap()
        .is_empty());
    for missing in [
        "legacy",
        "browser-v2-bad-id",
        "browser-v2-100-",
        "browser-v2-9999999999-future",
    ] {
        assert!(
            !store.replay_fenced_absent(missing, aged).unwrap(),
            "{missing}"
        );
        assert_eq!(
            store.acknowledge_at("owner@example.test", missing, aged),
            Err(OutcomeError::NotFound),
            "{missing}"
        );
    }
}

#[test]
fn retained_and_foreign_request_id_records_prevent_expiry_attestation() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    let id = "browser-v2-100-retained";
    begin(&store, id, 100).unwrap();
    let aged = 100 + RETENTION_SECONDS + 1;
    assert!(!store.replay_fenced_absent(id, aged).unwrap());
    assert_eq!(
        store.acknowledge_at("owner@example.test", id, aged),
        Err(OutcomeError::Pending)
    );
    assert_eq!(
        store.acknowledge_at("other@example.test", id, aged),
        Err(OutcomeError::NotFound)
    );
    store.interrupt("owner@example.test", id).unwrap();
    assert!(!store.replay_fenced_absent(id, aged).unwrap());
    // A retained interrupted receipt still uses the established explicit ACK contract.
    store
        .acknowledge_at("owner@example.test", id, aged)
        .unwrap();
    assert!(!store.replay_fenced_absent(id, aged).unwrap());
    let outcome = store.lookup("owner@example.test", id).unwrap();
    assert_eq!(outcome.status, OutcomeStatus::Interrupted);
    assert!(outcome.acknowledged);
}

#[test]
fn unavailable_store_never_attests_or_acknowledges_missing_expiry() {
    let root = tempdir().unwrap();
    let store = RequestOutcomeStore::open(root.path(), 100).unwrap();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _state = store.state.lock().unwrap();
        panic!("injected unavailable journal mutex");
    }));
    let id = "browser-v2-100-absent";
    let aged = 100 + RETENTION_SECONDS + 1;
    assert_eq!(
        store.replay_fenced_absent(id, aged),
        Err(OutcomeError::Unavailable)
    );
    assert_eq!(
        store.acknowledge_at("owner@example.test", id, aged),
        Err(OutcomeError::Unavailable)
    );
}
