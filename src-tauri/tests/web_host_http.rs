use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;
use video_creater_lib::web_host::dispatcher::HostDispatcher;
use video_creater_lib::web_host::http::{
    host_router, HostHttpConfig, PairResponse, SessionResponse,
};
use video_creater_lib::web_host::project_catalog::ProjectCatalog;
use video_creater_lib::web_host::rpc::{RpcDispatcher, RpcEnvelope, RpcResponse};
use video_creater_lib::{project::model::VideoProject, project::split::save_split_project};

struct PlatformDispatcher(AtomicUsize);

impl RpcDispatcher for PlatformDispatcher {
    fn dispatch(&self, request: &RpcEnvelope) -> Result<Value, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        match request.operation.as_str() {
            "get_platform_info" => Ok(json!({"platform": "linux"})),
            _ => Err("unsupported in test".into()),
        }
    }
}

#[tokio::test]
async fn api_rejects_forged_proxy_headers_and_binds_sessions_to_tailscale_identity() {
    let root = tempdir().unwrap();
    let app = host_router(
        HostHttpConfig {
            assets_dir: root.path().to_path_buf(),
            session_file: root.path().join("sessions.json"),
            public_origin: "https://host.tail.test".into(),
            host_label: "Studio host".into(),
            pairing_code: "123456".into(),
            reusable_pairing_code: true,
            issued_at: 100,
            now_override: Some(101),
            uploads_dir: root.path().join("uploads"),
            upload_max_bytes: 1_024,
            upload_reserve_bytes: 0,
            project_catalog: ProjectCatalog::new(vec![root.path().join("projects")]).unwrap(),
            allow_direct_test_identity: false,
        },
        Arc::new(PlatformDispatcher(AtomicUsize::new(0))),
    )
    .unwrap();
    let pair_request = |peer: Ipv4Addr, login: &str| {
        Request::post("/api/v1/pair")
            .header(header::ORIGIN, "https://host.tail.test")
            .header(header::CONTENT_TYPE, "application/json")
            .header("tailscale-user-login", login)
            .header("tailscale-user-name", "Editing phone")
            .extension(ConnectInfo(SocketAddr::from((peer, 4242))))
            .body(Body::from(r#"{"code":"123456","displayName":"Phone"}"#))
            .unwrap()
    };

    let forged = app
        .clone()
        .oneshot(pair_request(
            Ipv4Addr::new(100, 64, 0, 2),
            "person@example.test",
        ))
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);

    let paired = app
        .clone()
        .oneshot(pair_request(Ipv4Addr::LOCALHOST, "person@example.test"))
        .await
        .unwrap();
    assert_eq!(paired.status(), StatusCode::OK);
    let cookie = paired.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let stolen = app
        .oneshot(
            Request::get("/api/v1/session")
                .header(header::COOKIE, cookie)
                .header("tailscale-user-login", "attacker@example.test")
                .header("tailscale-user-name", "Attacker")
                .extension(ConnectInfo(SocketAddr::from((Ipv4Addr::LOCALHOST, 4242))))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stolen.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn browser_pairs_checks_session_and_calls_authenticated_rpc() {
    let root = tempdir().unwrap();
    let dispatcher = Arc::new(PlatformDispatcher(AtomicUsize::new(0)));
    let app = host_router(
        HostHttpConfig {
            assets_dir: root.path().to_path_buf(),
            session_file: root.path().join("sessions.json"),
            public_origin: "http://host.test".into(),
            host_label: "Studio host".into(),
            pairing_code: "123456".into(),
            reusable_pairing_code: false,
            issued_at: 100,
            now_override: Some(101),
            uploads_dir: root.path().join("uploads"),
            upload_max_bytes: 1_024,
            upload_reserve_bytes: 0,
            project_catalog: ProjectCatalog::new(vec![root.path().join("projects")]).unwrap(),
            allow_direct_test_identity: true,
        },
        dispatcher.clone(),
    )
    .unwrap();

    let rejected = app
        .clone()
        .oneshot(
            Request::post("/api/v1/pair")
                .header(header::ORIGIN, "http://evil.test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"code":"123456","displayName":"Phone"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

    let paired = app
        .clone()
        .oneshot(
            Request::post("/api/v1/pair")
                .header(header::ORIGIN, "http://host.test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"code":"123456","displayName":"Phone"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paired.status(), StatusCode::OK);
    let cookie = paired.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let pair: PairResponse =
        serde_json::from_slice(&to_bytes(paired.into_body(), 16_384).await.unwrap()).unwrap();
    assert!(pair.authenticated);

    let session = app
        .clone()
        .oneshot(
            Request::get("/api/v1/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(session.status(), StatusCode::OK);
    let session: SessionResponse =
        serde_json::from_slice(&to_bytes(session.into_body(), 16_384).await.unwrap()).unwrap();
    assert!(session.authenticated);
    assert_eq!(session.host_label, "Studio host");
    assert!(session
        .csrf_token
        .as_deref()
        .is_some_and(|token| !token.is_empty()));
    let csrf = session.csrf_token.unwrap();

    let envelope = RpcEnvelope {
        request_id: "request-platform".into(),
        operation: "get_platform_info".into(),
        project_id: None,
        expected_revision: None,
        editor_lease_token: None,
        payload: json!({}),
    };
    let rpc = app
        .clone()
        .oneshot(
            Request::post("/api/v1/rpc")
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&envelope).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rpc.status(), StatusCode::OK);
    let rpc: RpcResponse =
        serde_json::from_slice(&to_bytes(rpc.into_body(), 16_384).await.unwrap()).unwrap();
    assert_eq!(rpc.result, Some(json!({"platform": "linux"})));
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 1);

    let upload_bytes = [
        0_u8, 0, 0, 16, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm', 0, 0, 0, 0,
    ];
    let upload = app
        .clone()
        .oneshot(
            Request::post("/api/v1/uploads?name=clip.mp4")
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_LENGTH, upload_bytes.len())
                .body(Body::from(upload_bytes.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::OK);
    let upload: Value =
        serde_json::from_slice(&to_bytes(upload.into_body(), 16_384).await.unwrap()).unwrap();
    assert_eq!(upload["mediaType"], "video/mp4");
    assert_eq!(upload["sizeBytes"], 16);

    let sessions = app
        .clone()
        .oneshot(
            Request::get("/api/v1/admin/sessions")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sessions.status(), StatusCode::OK);
    let sessions: Value =
        serde_json::from_slice(&to_bytes(sessions.into_body(), 16_384).await.unwrap()).unwrap();
    assert_eq!(sessions["sessions"].as_array().unwrap().len(), 1);
    assert!(sessions["sessions"][0].get("credential").is_none());

    let rotated = app
        .clone()
        .oneshot(
            Request::post("/api/v1/session/rotate")
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated_cookie = rotated.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let rotated_body: Value =
        serde_json::from_slice(&to_bytes(rotated.into_body(), 16_384).await.unwrap()).unwrap();
    let rotated_csrf = rotated_body["csrfToken"].as_str().unwrap();

    let old_session = app
        .clone()
        .oneshot(
            Request::get("/api/v1/session")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_session.status(), StatusCode::UNAUTHORIZED);

    let logout = app
        .oneshot(
            Request::post("/api/v1/session/logout")
                .header(header::COOKIE, rotated_cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", rotated_csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::OK);
    assert!(logout.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
}

#[tokio::test]
async fn self_revocation_clears_the_cookie_and_a_revoked_browser_can_pair_again() {
    let root = tempdir().unwrap();
    let app = host_router(
        HostHttpConfig {
            assets_dir: root.path().to_path_buf(),
            session_file: root.path().join("sessions.json"),
            public_origin: "http://host.test".into(),
            host_label: "Studio host".into(),
            pairing_code: "123456".into(),
            reusable_pairing_code: true,
            issued_at: 100,
            now_override: Some(101),
            uploads_dir: root.path().join("uploads"),
            upload_max_bytes: 1_024,
            upload_reserve_bytes: 0,
            project_catalog: ProjectCatalog::new(vec![root.path().join("projects")]).unwrap(),
            allow_direct_test_identity: true,
        },
        Arc::new(PlatformDispatcher(AtomicUsize::new(0))),
    )
    .unwrap();
    let pair_request = |cookie: Option<&str>| {
        let mut builder = Request::post("/api/v1/pair")
            .header(header::ORIGIN, "http://host.test")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        builder
            .body(Body::from(r#"{"code":"123456","displayName":"Phone"}"#))
            .unwrap()
    };

    let paired = app.clone().oneshot(pair_request(None)).await.unwrap();
    assert_eq!(paired.status(), StatusCode::OK);
    let cookie = paired.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let pair: PairResponse =
        serde_json::from_slice(&to_bytes(paired.into_body(), 16_384).await.unwrap()).unwrap();

    let revoked = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/admin/sessions/{}/revoke", pair.session_id))
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", &pair.csrf_token)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::OK);
    assert!(revoked.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));

    let repaired = app
        .clone()
        .oneshot(pair_request(Some(&cookie)))
        .await
        .unwrap();
    assert_eq!(repaired.status(), StatusCode::OK);
    assert_ne!(
        repaired.headers()[header::SET_COOKIE].to_str().unwrap(),
        cookie
    );
    let repaired_cookie = repaired.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let repaired_pair: PairResponse =
        serde_json::from_slice(&to_bytes(repaired.into_body(), 16_384).await.unwrap()).unwrap();
    let revoke_all = app
        .oneshot(
            Request::post("/api/v1/admin/sessions/revoke-all")
                .header(header::COOKIE, repaired_cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", repaired_pair.csrf_token)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_all.status(), StatusCode::OK);
    assert!(revoke_all.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
}

#[test]
fn production_dispatcher_supports_browser_bootstrap_operations() {
    let dispatcher = HostDispatcher::default();
    let platform = dispatcher
        .dispatch(&RpcEnvelope {
            request_id: "platform".into(),
            operation: "get_platform_info".into(),
            project_id: None,
            expected_revision: None,
            editor_lease_token: None,
            payload: json!({}),
        })
        .unwrap();
    assert!(matches!(
        platform["platform"].as_str(),
        Some("linux" | "macos" | "windows" | "other")
    ));

    let preferences = dispatcher
        .dispatch(&RpcEnvelope {
            request_id: "preferences".into(),
            operation: "get_app_preferences".into(),
            project_id: None,
            expected_revision: None,
            editor_lease_token: None,
            payload: json!({}),
        })
        .unwrap();
    assert_eq!(preferences["schemaVersion"], 2);
    assert_eq!(preferences["projectLocation"]["mode"], "ask");
}

#[tokio::test]
async fn project_mutations_require_the_sessions_current_editor_lease() {
    let sandbox = tempdir().unwrap();
    let projects_root = sandbox.path().join("projects");
    std::fs::create_dir_all(&projects_root).unwrap();
    let project_path = projects_root.join("lease-test.palmier");
    let project = VideoProject::new_empty(
        "lease-test".into(),
        "Lease test".into(),
        "2026-09-24T00:00:00Z".into(),
    );
    save_split_project(&project_path, &project).unwrap();
    let catalog = ProjectCatalog::new(vec![projects_root]).unwrap();
    let project_id = catalog.id_for_path(&project_path).unwrap();
    let dispatcher = Arc::new(PlatformDispatcher(AtomicUsize::new(0)));
    let app = host_router(
        HostHttpConfig {
            assets_dir: sandbox.path().to_path_buf(),
            session_file: sandbox.path().join("sessions.json"),
            public_origin: "http://host.test".into(),
            host_label: "Studio host".into(),
            pairing_code: "654321".into(),
            reusable_pairing_code: false,
            issued_at: 100,
            now_override: Some(101),
            uploads_dir: sandbox.path().join("uploads"),
            upload_max_bytes: 1_024,
            upload_reserve_bytes: 0,
            project_catalog: catalog,
            allow_direct_test_identity: true,
        },
        dispatcher.clone(),
    )
    .unwrap();

    let paired = app
        .clone()
        .oneshot(
            Request::post("/api/v1/pair")
                .header(header::ORIGIN, "http://host.test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"code":"654321","displayName":"Phone"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = paired.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let pair: PairResponse =
        serde_json::from_slice(&to_bytes(paired.into_body(), 16_384).await.unwrap()).unwrap();
    let csrf = pair.csrf_token;
    let mutation = |request_id: &str, token: Option<&str>| RpcEnvelope {
        request_id: request_id.into(),
        operation: "apply_project_action_to_split_project_folder".into(),
        project_id: Some(project_id.clone()),
        expected_revision: Some(1),
        editor_lease_token: token.map(str::to_owned),
        payload: json!({"action":{"type":"createMediaFolder","folder":{"id":"remote","name":"Remote","parentId":null}}}),
    };
    let rpc_request = |envelope: &RpcEnvelope| {
        Request::post("/api/v1/rpc")
            .header(header::COOKIE, &cookie)
            .header(header::ORIGIN, "http://host.test")
            .header("x-csrf-token", &csrf)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(envelope).unwrap()))
            .unwrap()
    };

    let missing = app
        .clone()
        .oneshot(rpc_request(&mutation("missing-lease", None)))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::CONFLICT);
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 0);

    let lease = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/projects/{project_id}/lease"))
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, "http://host.test")
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"takeover":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lease.status(), StatusCode::OK);
    let lease: Value =
        serde_json::from_slice(&to_bytes(lease.into_body(), 16_384).await.unwrap()).unwrap();
    let token = lease["editorLeaseToken"].as_str().unwrap();

    let wrong = app
        .clone()
        .oneshot(rpc_request(&mutation("wrong-lease", Some("not-the-token"))))
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::CONFLICT);
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 0);

    let valid = app
        .oneshot(rpc_request(&mutation("valid-lease", Some(token))))
        .await
        .unwrap();
    assert_eq!(valid.status(), StatusCode::OK);
    let valid: RpcResponse =
        serde_json::from_slice(&to_bytes(valid.into_body(), 16_384).await.unwrap()).unwrap();
    assert!(!valid.ok); // The test dispatcher rejects the operation after lease authorization.
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 1);
}
