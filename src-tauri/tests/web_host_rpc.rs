use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::json;
use video_creater_lib::app_service::operation::AuthorizationScope;
use video_creater_lib::web_host::idempotency::{IdempotencyDecision, IdempotencyStore};
use video_creater_lib::web_host::registry::{RegistryError, RpcRegistry};
use video_creater_lib::web_host::rpc::{
    RpcDispatcher, RpcEngine, RpcEnvelope, RpcErrorCode, RpcResponse, SessionLimitError,
    SessionLimiter,
};

struct RecordingDispatcher(AtomicUsize);

impl RpcDispatcher for RecordingDispatcher {
    fn dispatch(&self, request: &RpcEnvelope) -> Result<serde_json::Value, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"operation": request.operation}))
    }
}

fn project_writer() -> BTreeSet<AuthorizationScope> {
    BTreeSet::from([
        AuthorizationScope::Session,
        AuthorizationScope::ProjectRead,
        AuthorizationScope::ProjectWrite,
    ])
}

#[test]
fn registry_allows_only_remote_inventory_operations() {
    let registry = RpcRegistry::from_inventory();
    assert!(registry
        .operation("load_split_project_from_folder")
        .is_some());
    assert!(registry.operation("sync_native_menu_state").is_none());
    assert!(registry.operation("call_codex_local_tool").is_none());
    assert!(registry.operation("totally_unknown").is_none());
}

#[test]
fn envelope_validation_enforces_size_scope_project_revision_and_lease() {
    let registry = RpcRegistry::from_inventory();
    let request = RpcEnvelope {
        request_id: "request-1".into(),
        operation: "save_split_project_to_folder".into(),
        project_id: Some("project-1".into()),
        expected_revision: Some(7),
        editor_lease_token: Some("lease-1".into()),
        payload: json!({"project": {"id": "project-1"}}),
    };
    registry.validate(&request, &project_writer(), 512).unwrap();

    let mut missing_revision = request.clone();
    missing_revision.expected_revision = None;
    assert_eq!(
        registry.validate(&missing_revision, &project_writer(), 512),
        Err(RegistryError::ExpectedRevisionRequired)
    );
    let mut missing_lease = request.clone();
    missing_lease.editor_lease_token = None;
    assert_eq!(
        registry.validate(&missing_lease, &project_writer(), 512),
        Err(RegistryError::EditorLeaseRequired)
    );
    assert_eq!(
        registry.validate(
            &request,
            &BTreeSet::from([AuthorizationScope::Session]),
            512
        ),
        Err(RegistryError::Forbidden)
    );
    assert_eq!(
        registry.validate(&request, &project_writer(), 40_000_000),
        Err(RegistryError::BodyTooLarge)
    );
}

#[test]
fn idempotency_replays_same_payload_and_rejects_changed_payload() {
    let store = IdempotencyStore::new(8, 60);
    let response = RpcResponse::success("request-1", json!({"revision": 8}));
    assert_eq!(
        store.check("session-1", "request-1", b"payload-a", 100),
        IdempotencyDecision::New
    );
    store
        .record(
            "session-1",
            "request-1",
            b"payload-a",
            response.clone(),
            100,
        )
        .unwrap();
    assert_eq!(
        store.check("session-1", "request-1", b"payload-a", 101),
        IdempotencyDecision::Replay(response)
    );
    assert_eq!(
        store.check("session-1", "request-1", b"payload-b", 101),
        IdempotencyDecision::PayloadConflict
    );
    assert_eq!(
        store.check("session-1", "request-1", b"payload-a", 161),
        IdempotencyDecision::New
    );
}

#[test]
fn rpc_errors_have_stable_safe_envelopes() {
    let response = RpcResponse::error(
        "request-9",
        RpcErrorCode::Forbidden,
        "operation is not permitted",
        Some(250),
    );
    let serialized = serde_json::to_string(&response).unwrap();
    assert!(serialized.contains("forbidden"));
    assert!(serialized.contains("retryAfterMs"));
    assert!(!serialized.contains("/home/"));
}

#[test]
fn session_limits_bound_request_rate_and_concurrency() {
    let limiter = SessionLimiter::new(2, 1, 10);
    let first = limiter.begin("session-1", 100).unwrap();
    assert!(matches!(
        limiter.begin("session-1", 100),
        Err(SessionLimitError::Concurrent)
    ));
    drop(first);
    let second = limiter.begin("session-1", 101).unwrap();
    drop(second);
    assert!(matches!(
        limiter.begin("session-1", 102),
        Err(SessionLimitError::Rate {
            retry_after_ms: 8_000
        })
    ));
    assert!(limiter.begin("session-1", 111).is_ok());
}

#[test]
fn rpc_engine_dispatches_allowlisted_requests_once_and_replays_the_result() {
    let dispatcher = Arc::new(RecordingDispatcher(AtomicUsize::new(0)));
    let engine = RpcEngine::new(dispatcher.clone());
    let body = serde_json::to_vec(&RpcEnvelope {
        request_id: "request-read-1".into(),
        operation: "load_split_project_from_folder".into(),
        project_id: Some("project-1".into()),
        expected_revision: None,
        editor_lease_token: None,
        payload: json!({}),
    })
    .unwrap();

    let first = engine.execute("session-1", &project_writer(), &body, 100);
    let replay = engine.execute("session-1", &project_writer(), &body, 101);
    assert!(first.ok);
    assert_eq!(replay, first);
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 1);

    let denied = serde_json::to_vec(&RpcEnvelope {
        request_id: "request-native".into(),
        operation: "sync_native_menu_state".into(),
        project_id: None,
        expected_revision: None,
        editor_lease_token: None,
        payload: json!({}),
    })
    .unwrap();
    let denied = engine.execute("session-1", &project_writer(), &denied, 102);
    assert_eq!(denied.error.unwrap().code, RpcErrorCode::NotFound);
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 1);
}

#[test]
fn concurrent_duplicate_requests_dispatch_exactly_once() {
    let dispatcher = Arc::new(RecordingDispatcher(AtomicUsize::new(0)));
    let engine = Arc::new(RpcEngine::new(dispatcher.clone()));
    let body = Arc::new(
        serde_json::to_vec(&RpcEnvelope {
            request_id: "request-concurrent".into(),
            operation: "load_split_project_from_folder".into(),
            project_id: Some("project-1".into()),
            expected_revision: None,
            editor_lease_token: None,
            payload: json!({}),
        })
        .unwrap(),
    );
    let scopes = Arc::new(project_writer());
    let mut threads = Vec::new();
    for _ in 0..4 {
        let engine = Arc::clone(&engine);
        let body = Arc::clone(&body);
        let scopes = Arc::clone(&scopes);
        threads.push(std::thread::spawn(move || {
            engine.execute("session-1", &scopes, &body, 100)
        }));
    }
    let responses = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();

    assert!(responses.iter().all(|response| response == &responses[0]));
    assert_eq!(dispatcher.0.load(Ordering::SeqCst), 1);
}

#[test]
fn bounded_rpc_envelope_corpus_always_returns_a_safe_response() {
    let engine = RpcEngine::new(Arc::new(RecordingDispatcher(AtomicUsize::new(0))));
    let scopes = project_writer();
    let mut corpus = vec![
        Vec::new(),
        b"null".to_vec(),
        b"[]".to_vec(),
        b"{}".to_vec(),
        b"{\"requestId\":\"x\"}".to_vec(),
        vec![b'['; 4_096],
        vec![0xff; 4_096],
    ];
    for length in 0..=512 {
        corpus.push(
            (0..length)
                .map(|index| ((index * 31 + length * 17) % 256) as u8)
                .collect(),
        );
    }
    for (index, bytes) in corpus.iter().enumerate() {
        let response = engine.execute(&format!("session-{index}"), &scopes, bytes, 100);
        assert!(!response.request_id.contains('/'));
        if !response.ok {
            let message = response.error.expect("failed response has error").message;
            assert!(!message.contains("/home/"));
            assert!(!message.contains("\\Users\\"));
        }
    }
}
