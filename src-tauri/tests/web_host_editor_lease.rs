use video_creater_lib::web_host::editor_lease::{LeaseError, LeaseManager};

#[test]
fn one_writer_lease_renews_releases_and_takeover_invalidates_the_old_writer() {
    let leases = LeaseManager::new(30, 5);
    let first = leases.acquire("project-1", "session-a", 100).unwrap();
    assert_eq!(
        leases.acquire("project-1", "session-b", 101),
        Err(LeaseError::Held)
    );
    leases
        .renew("project-1", "session-a", &first.token, 110)
        .unwrap();

    let second = leases.takeover("project-1", "session-b", 111).unwrap();
    assert_eq!(
        leases.validate("project-1", "session-a", &first.token, 112),
        Err(LeaseError::Invalid)
    );
    assert!(leases
        .validate("project-1", "session-b", &second.token, 112)
        .is_ok());
    assert_eq!(leases.audit_records().last().unwrap().action, "takeover");

    leases
        .release("project-1", "session-b", &second.token)
        .unwrap();
    assert!(leases.acquire("project-1", "session-a", 113).is_ok());
}

#[test]
fn disconnect_grace_allows_reconnect_then_expires_without_affecting_background_jobs() {
    let leases = LeaseManager::new(30, 5);
    let lease = leases.acquire("project-1", "session-a", 100).unwrap();
    leases.disconnected("session-a", 101);
    assert!(leases
        .validate("project-1", "session-a", &lease.token, 105)
        .is_ok());
    assert_eq!(
        leases.validate("project-1", "session-a", &lease.token, 107),
        Err(LeaseError::Expired)
    );
    assert!(leases.background_job_allowed("project-1"));
}

#[test]
fn authenticated_reacquisition_and_takeover_recover_after_disconnect_grace() {
    let leases = LeaseManager::new(30, 5);
    leases.acquire("project-1", "session-a", 100).unwrap();
    leases.disconnected("session-a", 101);
    let renewed = leases
        .acquire_or_renew_for_session("project-1", "session-a", 107)
        .unwrap();
    assert_eq!(
        leases.validate("project-1", "session-a", &renewed.token, 107),
        Ok(())
    );

    leases.disconnected("session-a", 108);
    let reacquired = leases
        .acquire_or_renew_for_session("project-1", "session-a", 150)
        .unwrap();
    assert_eq!(
        leases.validate("project-1", "session-a", &reacquired.token, 150),
        Ok(())
    );

    leases.disconnected("session-a", 151);
    let takeover = leases.takeover("project-1", "session-a", 157).unwrap();
    assert_eq!(
        leases.validate("project-1", "session-a", &takeover.token, 157),
        Ok(())
    );
}
