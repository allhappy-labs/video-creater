use super::*;

fn fixture() -> (tempfile::TempDir, SessionStore) {
    let root = tempfile::tempdir().unwrap();
    let store =
        SessionStore::load(root.path().join("sessions.json"), "https://host.test", 100).unwrap();
    (root, store)
}

fn block_persistence(store: &SessionStore) {
    std::fs::create_dir(store.path.with_extension("tmp")).unwrap();
}

#[test]
fn failed_issue_does_not_add_a_credential_to_memory() {
    let (_root, store) = fixture();
    block_persistence(&store);
    assert_eq!(
        store.issue("phone", None, 100).unwrap_err(),
        SessionError::Io
    );
    assert!(store.list().is_empty());
}

#[test]
fn failed_revocation_rotation_and_csrf_refresh_preserve_authorization() {
    let (_root, store) = fixture();
    let issued = store.issue("phone", Some("owner@test"), 100).unwrap();
    let changes = store.authorization_changes();
    let original = std::fs::read(&store.path).unwrap();
    block_persistence(&store);
    assert_eq!(store.revoke(&issued.session_id), Err(SessionError::Io));
    assert!(store.authenticate(&issued.credential, 101).is_ok());
    assert_eq!(
        store.rotate(&issued.credential, 101).unwrap_err(),
        SessionError::Io
    );
    assert!(store.authenticate(&issued.credential, 101).is_ok());
    assert_eq!(
        store.refresh_csrf(&issued.session_id),
        Err(SessionError::Io)
    );
    let state = store.state.lock().unwrap();
    assert!(state.sessions[0].previous_csrf_hashes.is_empty());
    assert_eq!(state.sessions[0].csrf_hash, hash(&issued.csrf_token));
    drop(state);
    assert_eq!(store.revoke_all(), Err(SessionError::Io));
    assert!(store.authenticate(&issued.credential, 101).is_ok());
    assert_eq!(std::fs::read(&store.path).unwrap(), original);
    assert!(!changes.has_changed().unwrap());
    assert!(store.is_active(&issued.session_id, 101));
}

#[test]
fn issuance_prunes_only_expired_or_revoked_credentials_and_preserves_exact_expiry() {
    let (_root, store) = fixture();
    let boundary = store.issue("boundary", None, 0).unwrap();
    let revoked = store.issue("revoked", None, 1).unwrap();
    store.revoke(&revoked.session_id).unwrap();
    let valid = store.issue("valid", None, 100).unwrap();
    assert!(store.authenticate(&boundary.credential, 100).is_ok());
    assert!(store.authenticate(&valid.credential, 100).is_ok());
    assert_eq!(store.list().len(), 2);
    store.issue("next", None, 101).unwrap();
    assert_eq!(store.list().len(), 2);
    assert!(store.authenticate(&valid.credential, 101).is_ok());
}

#[test]
fn live_session_ceiling_rejects_new_issuance_without_revoking_existing_devices() {
    let (_root, store) = fixture();
    let issued = store.issue("live", None, 100).unwrap();
    let mut state = store.state.lock().unwrap();
    let prototype = state.sessions[0].clone();
    for index in 1..1_024 {
        let mut session = prototype.clone();
        session.id = format!("device-{index}");
        session.credential_hash = hash(&session.id);
        state.sessions.push(session);
    }
    store.persist(&state).unwrap();
    drop(state);
    let original = std::fs::read(&store.path).unwrap();
    assert!(store.issue("excess", None, 101).is_err());
    assert!(store.authenticate(&issued.credential, 101).is_ok());
    assert_eq!(store.list().len(), 1_024);
    assert_eq!(std::fs::read(&store.path).unwrap(), original);
    // Rotation replaces a live credential atomically and therefore needs no spare slot.
    let rotated = store.rotate(&issued.credential, 101).unwrap();
    assert!(store.authenticate(&rotated.credential, 101).is_ok());
}

#[test]
fn oversized_session_file_fails_closed_without_rewriting_it() {
    let (root, store) = fixture();
    let bytes = serde_json::to_vec(
        &serde_json::json!({"sessions": [], "padding": " ".repeat(2 * 1024 * 1024)}),
    )
    .unwrap();
    std::fs::write(&store.path, &bytes).unwrap();
    assert!(SessionStore::load(store.path.clone(), "https://host.test", 100).is_err());
    assert_eq!(
        std::fs::read(root.path().join("sessions.json")).unwrap(),
        bytes
    );
}

#[test]
fn startup_prunes_expired_legacy_population_before_live_capacity_check() {
    let (_root, store) = fixture();
    let valid = store.issue("valid", None, 100).unwrap();
    let mut state = store.state.lock().unwrap().clone();
    state.sessions[0].expires_at = u64::MAX;
    let prototype = state.sessions[0].clone();
    for index in 0..1_025 {
        let mut expired = prototype.clone();
        expired.id = format!("expired-{index}");
        expired.credential_hash = hash(&expired.id);
        expired.expires_at = 100;
        state.sessions.push(expired);
    }
    let original = serde_json::to_vec(&state).unwrap();
    assert!(original.len() < MAX_SESSION_FILE_BYTES);
    std::fs::write(&store.path, &original).unwrap();
    let loaded = SessionStore::load_at(store.path.clone(), "https://host.test", 100, 101)
        .expect("expired legacy population must not prevent startup");
    assert!(loaded.authenticate(&valid.credential, 101).is_ok());
    assert_eq!(loaded.list().len(), 1);
    assert_eq!(std::fs::read(&store.path).unwrap(), original);
}

#[test]
fn startup_preserves_exact_expiry_and_fails_closed_on_live_overflow() {
    let (_root, store) = fixture();
    let boundary = store.issue("boundary", None, 1).unwrap();
    let bytes = std::fs::read(&store.path).unwrap();
    let loaded = SessionStore::load_at(store.path.clone(), "https://host.test", 100, 101).unwrap();
    assert!(loaded.authenticate(&boundary.credential, 101).is_ok());
    assert_eq!(std::fs::read(&store.path).unwrap(), bytes);
    let mut state = store.state.lock().unwrap().clone();
    let prototype = state.sessions[0].clone();
    for index in 0..1_024 {
        let mut valid = prototype.clone();
        valid.id = format!("live-{index}");
        valid.credential_hash = hash(&valid.id);
        state.sessions.push(valid);
    }
    let original = serde_json::to_vec(&state).unwrap();
    std::fs::write(&store.path, &original).unwrap();
    assert!(SessionStore::load_at(store.path.clone(), "https://host.test", 100, 101).is_err());
    assert_eq!(std::fs::read(&store.path).unwrap(), original);
}
