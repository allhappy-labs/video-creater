#![cfg(feature = "web-host")]

use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;
use video_creater_lib::project::model::VideoProject;
use video_creater_lib::web_host::dispatcher::HostDispatcher;
use video_creater_lib::web_host::http::{host_router, HostHttpConfig, PairResponse};
use video_creater_lib::web_host::project_catalog::ProjectCatalog;
use video_creater_lib::web_host::rpc::{RpcEnvelope, RpcResponse};

const ORIGIN: &str = "https://outcomes.tail.test";

fn host(root: &Path, now: u64) -> Router {
    let projects = ProjectCatalog::new(vec![root.join("projects")]).unwrap();
    host_with_dispatcher(
        root,
        now,
        Arc::new(HostDispatcher::with_project_catalog(projects)),
    )
}

fn host_with_dispatcher(
    root: &Path,
    now: u64,
    dispatcher: Arc<dyn video_creater_lib::web_host::rpc::RpcDispatcher>,
) -> Router {
    let projects = ProjectCatalog::new(vec![root.join("projects")]).unwrap();
    host_router(
        HostHttpConfig {
            assets_dir: root.to_path_buf(),
            session_file: root.join("sessions.json"),
            public_origin: ORIGIN.into(),
            host_label: "Outcomes".into(),
            pairing_code: "123456".into(),
            reusable_pairing_code: true,
            issued_at: now,
            now_override: Some(now),
            uploads_dir: root.join("uploads"),
            upload_max_bytes: 1024,
            upload_reserve_bytes: 0,
            project_catalog: projects.clone(),
            allow_direct_test_identity: false,
        },
        dispatcher,
    )
    .unwrap()
}

fn request(
    method: &str,
    uri: &str,
    login: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-video-creater-outcome-protocol", "1")
        .header("tailscale-user-login", login)
        .header("tailscale-user-name", "Outcome test")
        .extension(ConnectInfo(SocketAddr::from((Ipv4Addr::LOCALHOST, 4321))));
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    builder
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

async fn pair(app: &Router, login: &str) -> (String, String) {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/pair",
            login,
            None,
            None,
            json!({"code":"123456", "displayName":"Outcome browser"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let pair: PairResponse =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    (cookie, pair.csrf_token)
}

fn creation(id: &str) -> Value {
    json!(RpcEnvelope {
        request_id: id.into(),
        operation: "remote_create_project".into(),
        project_id: None,
        expected_revision: None,
        editor_lease_token: None,
        payload: json!({"project":VideoProject::new_empty("submitted".into(), "Durable creation".into(), "2026-10-01T00:00:00Z".into())}),
    })
}

async fn rpc(
    app: &Router,
    login: &str,
    credentials: &(String, String),
    body: Value,
) -> RpcResponse {
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/rpc",
            login,
            Some(&credentials.0),
            Some(&credentials.1),
            body,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(
        &to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn creation_request_is_not_dispatched_again_after_host_restart_and_session_replacement() {
    let root = tempdir().unwrap();
    let app = host(root.path(), 100);
    let credentials = pair(&app, "owner@example.test").await;
    let body = creation("browser-durable-create");
    let accepted = rpc(&app, "owner@example.test", &credentials, body.clone()).await;
    assert!(accepted.ok, "{accepted:?}");
    let catalog_id = accepted.result.as_ref().unwrap()["catalogProjectId"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(app);
    let restarted = host(root.path(), 101);
    let new_session = pair(&restarted, "owner@example.test").await;
    let replay = rpc(&restarted, "owner@example.test", &new_session, body).await;
    let projects = ProjectCatalog::new(vec![root.path().join("projects")])
        .unwrap()
        .list()
        .unwrap();
    assert_eq!(
        projects.len(),
        1,
        "restart must not execute an accepted creation again"
    );
    assert!(
        replay.ok
            || replay
                .error
                .as_ref()
                .is_some_and(|error| error.message.contains("outcome"))
    );
    assert_eq!(projects[0].project_id, catalog_id);
}

#[tokio::test]
async fn authenticated_outcome_lookup_survives_restart_and_is_scoped_to_trusted_identity() {
    let root = tempdir().unwrap();
    let app = host(root.path(), 100);
    let credentials = pair(&app, "owner@example.test").await;
    let id = "browser-durable-lookup";
    let accepted = rpc(&app, "owner@example.test", &credentials, creation(id)).await;
    assert!(accepted.ok);
    drop(app);
    let restarted = host(root.path(), 101);
    let owner = pair(&restarted, "owner@example.test").await;
    let response = restarted
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{id}"),
            "owner@example.test",
            Some(&owner.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let outcome: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(outcome["requestId"], id);
    assert_eq!(outcome["status"], "succeeded");
    assert_eq!(
        outcome["catalogProjectId"],
        accepted.result.unwrap()["catalogProjectId"]
    );
    let other = pair(&restarted, "other@example.test").await;
    let rejected = restarted
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{id}"),
            "other@example.test",
            Some(&other.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::NOT_FOUND);
}

struct BlockingRecovery;
impl video_creater_lib::web_host::rpc::RpcDispatcher for BlockingRecovery {
    fn dispatch(&self, _: &RpcEnvelope) -> Result<Value, String> {
        Err("unused".into())
    }
    fn recover_durable_creation(&self, _: &str) -> Option<Value> {
        std::thread::sleep(std::time::Duration::from_millis(200));
        None
    }
}

#[tokio::test(flavor = "current_thread")]
async fn outcome_recovery_io_leaves_the_async_control_worker_available() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use video_creater_lib::web_host::request_outcome::RequestOutcomeStore;
    let root = tempdir().unwrap();
    let id = "browser-stalled-recovery";
    let store =
        RequestOutcomeStore::open(&root.path().join("sessions.request-outcomes"), 100).unwrap();
    let body = creation(id);
    let envelope: RpcEnvelope = serde_json::from_value(body.clone()).unwrap();
    store
        .begin(
            "owner@example.test",
            &envelope,
            &serde_json::to_vec(&body).unwrap(),
            100,
        )
        .unwrap();
    drop(store);
    let app = host_with_dispatcher(root.path(), 101, Arc::new(BlockingRecovery));
    let credentials = pair(&app, "owner@example.test").await;
    let advanced = Arc::new(AtomicBool::new(false));
    let heartbeat = advanced.clone();
    let worker = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        heartbeat.store(true, Ordering::SeqCst);
    });
    let response = app
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{id}"),
            "owner@example.test",
            Some(&credentials.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    let control_advanced = advanced.load(Ordering::SeqCst);
    worker.await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        control_advanced,
        "blocking outcome recovery monopolized the async control worker"
    );
}

#[tokio::test]
async fn committed_creation_is_recovered_when_host_dies_before_receipt_publication() {
    use video_creater_lib::web_host::request_outcome::{OutcomeAdmission, RequestOutcomeStore};
    use video_creater_lib::web_host::rpc::RpcDispatcher;
    let root = tempdir().unwrap();
    let id = "browser-creation-commit-gap";
    let body = creation(id);
    let envelope: RpcEnvelope = serde_json::from_value(body.clone()).unwrap();
    let store =
        RequestOutcomeStore::open(&root.path().join("sessions.request-outcomes"), 100).unwrap();
    let nonce = match store
        .begin(
            "owner@example.test",
            &envelope,
            &serde_json::to_vec(&body).unwrap(),
            100,
        )
        .unwrap()
    {
        OutcomeAdmission::New {
            creation_nonce: Some(nonce),
        } => nonce,
        _ => panic!("new creation witness expected"),
    };
    let projects = ProjectCatalog::new(vec![root.path().join("projects")]).unwrap();
    let saved = HostDispatcher::with_project_catalog(projects.clone())
        .dispatch_with_creation_nonce(&envelope, Some(&nonce))
        .unwrap();
    // Deliberately omit store.finish: the canonical commit happened, its ACK/receipt did not.
    drop(store);
    let app = host(root.path(), 101);
    let credentials = pair(&app, "owner@example.test").await;
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{id}"),
            "owner@example.test",
            Some(&credentials.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let outcome: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
    assert_eq!(outcome["status"], "succeeded");
    assert_eq!(outcome["catalogProjectId"], saved["catalogProjectId"]);
    let replay = rpc(&app, "owner@example.test", &credentials, body).await;
    assert!(!replay.ok);
    assert_eq!(projects.list().unwrap().len(), 1);
}

#[tokio::test]
async fn interrupted_creation_without_commit_witness_stays_uncertain_after_acknowledgement() {
    use video_creater_lib::web_host::request_outcome::RequestOutcomeStore;
    let root = tempdir().unwrap();
    let id = "browser-creation-interrupted";
    let body = creation(id);
    let envelope: RpcEnvelope = serde_json::from_value(body.clone()).unwrap();
    let store =
        RequestOutcomeStore::open(&root.path().join("sessions.request-outcomes"), 100).unwrap();
    store
        .begin(
            "owner@example.test",
            &envelope,
            &serde_json::to_vec(&body).unwrap(),
            100,
        )
        .unwrap();
    drop(store);
    let app = host(root.path(), 101);
    let credentials = pair(&app, "owner@example.test").await;
    let lookup = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/request-outcomes/{id}"),
            "owner@example.test",
            Some(&credentials.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    let outcome: Value =
        serde_json::from_slice(&to_bytes(lookup.into_body(), 16384).await.unwrap()).unwrap();
    assert_eq!(outcome["status"], "interrupted");
    let denied = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{id}/ack"),
            "owner@example.test",
            Some(&credentials.0),
            None,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let ack = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{id}/ack"),
            "owner@example.test",
            Some(&credentials.0),
            Some(&credentials.1),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(ack.status(), StatusCode::OK);
    let replay = rpc(&app, "owner@example.test", &credentials, body).await;
    assert_eq!(
        replay.error.unwrap().code,
        video_creater_lib::web_host::rpc::RpcErrorCode::OutcomeUnknown
    );
    assert!(ProjectCatalog::new(vec![root.path().join("projects")])
        .unwrap()
        .list()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn terminal_acknowledgement_hides_from_list_but_retains_the_replay_receipt() {
    let root = tempdir().unwrap();
    let app = host(root.path(), 100);
    let credentials = pair(&app, "owner@example.test").await;
    let id = "browser-acked-create";
    let body = creation(id);
    let accepted = rpc(&app, "owner@example.test", &credentials, body.clone()).await;
    assert!(accepted.ok);
    let ack = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/request-outcomes/{id}/ack"),
            "owner@example.test",
            Some(&credentials.0),
            Some(&credentials.1),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(ack.status(), StatusCode::OK);
    let list = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/request-outcomes",
            "owner@example.test",
            Some(&credentials.0),
            None,
            Value::Null,
        ))
        .await
        .unwrap();
    let list: Value =
        serde_json::from_slice(&to_bytes(list.into_body(), 16384).await.unwrap()).unwrap();
    assert_eq!(list["outcomes"], json!([]));
    drop(app);
    let restarted = host(root.path(), 101);
    let credentials = pair(&restarted, "owner@example.test").await;
    let replay = rpc(&restarted, "owner@example.test", &credentials, body).await;
    assert!(!replay.ok);
    assert_eq!(
        ProjectCatalog::new(vec![root.path().join("projects")])
            .unwrap()
            .list()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn legacy_clients_keep_uncertainty_when_their_old_error_decoder_cannot_read_new_outcomes() {
    let root = tempdir().unwrap();
    let app = host(root.path(), 100);
    let credentials = pair(&app, "owner@example.test").await;
    let body = creation("legacy-outcome-decoder");
    assert!(
        rpc(&app, "owner@example.test", &credentials, body.clone())
            .await
            .ok
    );
    drop(app);
    let app = host(root.path(), 101);
    let credentials = pair(&app, "owner@example.test").await;
    let mut replay = request(
        "POST",
        "/api/v1/rpc",
        "owner@example.test",
        Some(&credentials.0),
        Some(&credentials.1),
        body,
    );
    replay
        .headers_mut()
        .remove("x-video-creater-outcome-protocol");
    let response = app.oneshot(replay).await.unwrap();
    assert_eq!(response.status(),StatusCode::BAD_GATEWAY,"legacy error decoders must receive an uncertain transport outcome, not a misleading rejected mutation");
    let body = to_bytes(response.into_body(), 16384).await.unwrap();
    assert!(serde_json::from_slice::<RpcResponse>(&body).is_err());
    assert_eq!(
        ProjectCatalog::new(vec![root.path().join("projects")])
            .unwrap()
            .list()
            .unwrap()
            .len(),
        1
    );
}

#[path = "web_host_request_outcomes/expiry.rs"]
mod expiry;
