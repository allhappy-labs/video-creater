use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use tempfile::tempdir;
use video_creater_lib::web_host::identity::{trusted_proxy_identity, ProxyMode};
use video_creater_lib::web_host::pairing::{generate_pairing_code, PairingError, PairingManager};
use video_creater_lib::web_host::session::{SessionError, SessionStore};

#[test]
fn proxy_identity_is_accepted_only_from_loopback_tailscale_serve() {
    let headers = BTreeMap::from([
        ("tailscale-user-login".into(), "person@example.test".into()),
        ("tailscale-user-name".into(), "Editing phone".into()),
    ]);
    assert!(trusted_proxy_identity(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        ProxyMode::TailscaleServe,
        &headers
    )
    .is_some());
    assert!(trusted_proxy_identity(
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 2)),
        ProxyMode::TailscaleServe,
        &headers
    )
    .is_none());
    assert!(
        trusted_proxy_identity(IpAddr::V4(Ipv4Addr::LOCALHOST), ProxyMode::Direct, &headers)
            .is_none()
    );
}

#[test]
fn pairing_code_is_single_use_expires_and_rate_limits_attempts() {
    let generated = generate_pairing_code();
    assert_eq!(generated.len(), 6);
    assert!(generated.bytes().all(|byte| byte.is_ascii_digit()));
    let manager = PairingManager::new("123456", 1_000);
    assert_eq!(manager.verify("000000", 1_001), Err(PairingError::Invalid));
    assert_eq!(manager.verify("000000", 1_002), Err(PairingError::Invalid));
    assert_eq!(
        manager.verify("000000", 1_003),
        Err(PairingError::TooManyAttempts)
    );

    let valid = PairingManager::new("654321", 2_000);
    assert!(valid.verify("654321", 2_001).is_ok());
    assert_eq!(
        valid.verify("654321", 2_002),
        Err(PairingError::AlreadyUsed)
    );
    assert_eq!(
        PairingManager::new("111111", 3_000).verify("111111", 3_301),
        Err(PairingError::Expired)
    );
}

#[test]
fn explicit_test_bootstrap_can_pair_multiple_isolated_browsers() {
    let manager = PairingManager::reusable_for_tests("246810", 1_000);
    assert!(manager.verify("246810", 1_001).is_ok());
    assert!(manager.verify("246810", 1_002).is_ok());
}

#[test]
fn sessions_persist_only_hashes_and_enforce_cookie_origin_csrf_expiry_and_revoke() {
    let root = tempdir().unwrap();
    let path = root.path().join("remote-devices.json");
    let store = SessionStore::load(path.clone(), "https://host.tail.test", 10_000).unwrap();
    let issued = store
        .issue("phone", Some("person@example.test"), 100)
        .unwrap();

    let persisted = std::fs::read_to_string(&path).unwrap();
    assert!(!persisted.contains(&issued.credential));
    assert!(!persisted.contains(&issued.csrf_token));
    assert!(issued.cookie_header.contains("Secure"));
    assert!(issued.cookie_header.contains("HttpOnly"));
    assert!(issued.cookie_header.contains("SameSite=Strict"));

    let session = store.authenticate(&issued.credential, 101).unwrap();
    assert_eq!(session.display_name, "phone");
    assert!(store
        .authenticate_for_identity(&issued.credential, "person@example.test", 101)
        .is_ok());
    assert_eq!(
        store.authenticate_for_identity(&issued.credential, "attacker@example.test", 101),
        Err(SessionError::Identity)
    );
    assert!(store
        .authorize_mutation(
            &issued.credential,
            &issued.csrf_token,
            "https://host.tail.test",
            101
        )
        .is_ok());
    assert_eq!(
        store.authorize_mutation(&issued.credential, "wrong", "https://host.tail.test", 101),
        Err(SessionError::Csrf)
    );
    assert_eq!(
        store.authorize_mutation(
            &issued.credential,
            &issued.csrf_token,
            "https://evil.test",
            101
        ),
        Err(SessionError::Origin)
    );
    let refreshed_csrf = store.refresh_csrf(&issued.session_id).unwrap();
    assert!(store
        .authorize_mutation(
            &issued.credential,
            &issued.csrf_token,
            "https://host.tail.test",
            101
        )
        .is_ok());
    assert!(store
        .authorize_mutation(
            &issued.credential,
            &refreshed_csrf,
            "https://host.tail.test",
            101
        )
        .is_ok());
    assert_eq!(
        store.authenticate(&issued.credential, 10_101),
        Err(SessionError::Expired)
    );

    let rotating = store.issue("rotating laptop", None, 150).unwrap();
    let rotated = store.rotate(&rotating.credential, 151).unwrap();
    assert_eq!(
        store.authenticate(&rotating.credential, 152),
        Err(SessionError::Revoked)
    );
    assert!(store.authenticate(&rotated.credential, 152).is_ok());

    let second = store.issue("laptop", None, 200).unwrap();
    store.revoke(&second.session_id).unwrap();
    assert_eq!(
        store.authenticate(&second.credential, 201),
        Err(SessionError::Revoked)
    );
    let third = store.issue("tablet", None, 300).unwrap();
    store.revoke_all().unwrap();
    assert_eq!(
        store.authenticate(&third.credential, 301),
        Err(SessionError::Revoked)
    );
}
