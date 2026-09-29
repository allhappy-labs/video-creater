use video_creater_lib::web_host::resource_ticket::{
    ResourceAccess, ResourceTicketError, ResourceTicketStore, TicketStatus,
};

#[test]
fn tickets_are_opaque_short_lived_and_scoped_to_session_project_and_kind() {
    let store = ResourceTicketStore::new(60, 300);
    let issued = store.mint(
        "session-a",
        "project-a",
        ResourceAccess::Media,
        "media/clip.mp4",
        "clip.mp4",
        100,
    );

    assert_eq!(issued.token.len(), 32);
    assert!(!issued.token.contains("clip"));
    assert_eq!(issued.expires_at, 160);
    let valid = store
        .validate(&issued.token, "session-a", ResourceAccess::Media, 159)
        .unwrap();
    assert_eq!(valid.status, TicketStatus::Valid);
    assert_eq!(valid.grant.project_id, "project-a");
    assert_eq!(valid.grant.relative_path, "media/clip.mp4");
    assert_eq!(
        store.validate(&issued.token, "session-b", ResourceAccess::Media, 120),
        Err(ResourceTicketError::Scope)
    );
    assert_eq!(
        store.validate(&issued.token, "session-a", ResourceAccess::Artifact, 120),
        Err(ResourceTicketError::Scope)
    );

    let expired = store
        .validate(&issued.token, "session-a", ResourceAccess::Media, 161)
        .unwrap();
    assert_eq!(expired.status, TicketStatus::Expired);
    let refreshed = store
        .refresh(&issued.token, "session-a", ResourceAccess::Media, 161)
        .unwrap();
    assert_ne!(refreshed.token, issued.token);
    assert_eq!(
        store.validate(&issued.token, "session-a", ResourceAccess::Media, 161),
        Err(ResourceTicketError::Unknown)
    );
}

#[test]
fn expired_tickets_cannot_refresh_after_the_bounded_grace_period() {
    let store = ResourceTicketStore::new(10, 20);
    let issued = store.mint(
        "session-a",
        "project-a",
        ResourceAccess::Media,
        "media/clip.mp4",
        "clip.mp4",
        100,
    );

    assert_eq!(
        store.refresh(&issued.token, "session-a", ResourceAccess::Media, 131),
        Err(ResourceTicketError::Expired)
    );
}
