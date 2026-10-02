use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Condvar};
use std::time::Duration;

const READ: &str = "load_job_progress_from_split_project_folder";
const WRITE: &str = "apply_project_actions_to_split_project_folder";
const STOP: &str = "cancel_render_job_in_split_project_folder";

struct SuccessfulDispatcher;
impl RpcDispatcher for SuccessfulDispatcher {
    fn dispatch(&self, request: &RpcEnvelope) -> Result<Value, String> {
        Ok(json!({"operation": request.operation}))
    }
}

fn scopes() -> BTreeSet<AuthorizationScope> {
    BTreeSet::from([
        AuthorizationScope::Session,
        AuthorizationScope::ProjectWrite,
    ])
}

fn envelope(id: &str, operation: &str) -> RpcEnvelope {
    RpcEnvelope {
        request_id: id.into(),
        operation: operation.into(),
        project_id: Some("project-test".into()),
        expected_revision: Some(7),
        editor_lease_token: Some("editor-test".into()),
        payload: json!({}),
    }
}

fn execute(engine: &RpcEngine, id: &str, operation: &str, now: u64) -> RpcResponse {
    engine.execute(
        "session-test",
        &scopes(),
        &serde_json::to_vec(&envelope(id, operation)).unwrap(),
        now,
    )
}

#[test]
fn progress_polling_does_not_starve_write_stop_or_project_reopen() {
    let engine = RpcEngine::new(Arc::new(SuccessfulDispatcher));
    for index in 0..61 {
        let response = execute(&engine, &format!("poll-{index}"), READ, 100);
        assert!(response.ok, "poll {index}: {:?}", response.error);
    }
    for (id, operation) in [
        ("write-after-polls", WRITE),
        ("stop-after-polls", STOP),
        ("reopen-after-polls", "load_split_project_from_folder"),
    ] {
        let response = execute(&engine, id, operation, 110);
        assert!(response.ok, "{operation}: {:?}", response.error);
    }
}

#[test]
fn read_mutation_and_cancellation_budgets_are_finite_with_accurate_reset_hint() {
    let engine = RpcEngine::new(Arc::new(SuccessfulDispatcher));
    for (operation, budget) in [(READ, 300), (WRITE, 60), (STOP, 30)] {
        for index in 0..budget {
            let response = execute(&engine, &format!("{operation}-{index}"), operation, 100);
            assert!(response.ok, "{operation} {index}: {:?}", response.error);
        }
        let response = execute(&engine, &format!("{operation}-limited"), operation, 103);
        let error = response.error.unwrap();
        assert_eq!(error.code, RpcErrorCode::RateLimited);
        assert_eq!(error.retry_after_ms, Some(57_000));
    }
    // All finite budgets reset at the shared window boundary, with no retry of a mutation.
    for operation in [READ, WRITE, STOP] {
        assert!(execute(&engine, &format!("{operation}-new-window"), operation, 160).ok);
    }
    let body = serde_json::to_vec(&envelope("new-session", READ)).unwrap();
    assert!(engine.execute("other-session", &scopes(), &body, 103).ok);
}

struct BlockingDispatcher {
    entered: mpsc::Sender<String>,
    released: Mutex<bool>,
    ready: Condvar,
    calls: AtomicUsize,
}
impl RpcDispatcher for BlockingDispatcher {
    fn dispatch(&self, request: &RpcEnvelope) -> Result<Value, String> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if request.request_id.starts_with("blocked-") {
            self.entered.send(request.request_id.clone()).unwrap();
            let released = self.released.lock().unwrap();
            drop(
                self.ready
                    .wait_while(released, |released| !*released)
                    .unwrap(),
            );
        }
        Ok(json!({"operation": request.operation, "call": call}))
    }
}
impl BlockingDispatcher {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.ready.notify_all();
    }
}

#[test]
fn ordinary_reads_and_writes_share_four_slots_and_stop_has_only_one_reserved_slot() {
    let (entered, receiver) = mpsc::channel();
    let dispatcher = Arc::new(BlockingDispatcher {
        entered,
        released: Mutex::new(false),
        ready: Condvar::new(),
        calls: AtomicUsize::new(0),
    });
    let engine = Arc::new(RpcEngine::new(dispatcher.clone()));
    let workers = (0..4)
        .map(|index| {
            let engine = engine.clone();
            std::thread::spawn(move || {
                execute(
                    &engine,
                    &format!("blocked-ordinary-{index}"),
                    if index % 2 == 0 { READ } else { WRITE },
                    100,
                )
            })
        })
        .collect::<Vec<_>>();
    let ordinary_entered = (0..4).all(|_| receiver.recv_timeout(Duration::from_secs(2)).is_ok());
    // Advancing the rate window must not release occupied concurrency slots.
    let fifth = execute(&engine, "ordinary-fifth", WRITE, 160);
    let stop_engine = engine.clone();
    let stop = std::thread::spawn(move || execute(&stop_engine, "blocked-stop", STOP, 160));
    let stop_entered = receiver.recv_timeout(Duration::from_secs(2)).is_ok();
    let second_stop = execute(&engine, "second-stop", STOP, 220);
    let read_at_capacity = execute(&engine, "read-at-capacity", READ, 220);
    // Release every worker before asserting, so a failed regression cannot leak stalled threads.
    dispatcher.release();
    let ordinary = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    let stop = stop.join().unwrap();
    assert!(ordinary_entered);
    assert!(ordinary.iter().all(|response| response.ok));
    assert_eq!(fifth.error.unwrap().code, RpcErrorCode::Busy);
    assert!(stop_entered, "Stop never entered its reserved slot");
    assert!(stop.ok, "{:?}", stop.error);
    assert_eq!(second_stop.error.unwrap().code, RpcErrorCode::Busy);
    assert_eq!(read_at_capacity.error.unwrap().code, RpcErrorCode::Busy);
    assert!(execute(&engine, "ordinary-after-drop", READ, 220).ok);
    assert!(execute(&engine, "stop-after-drop", STOP, 220).ok);
}

#[test]
fn cancellation_reserve_requires_validated_authorized_job_mutation() {
    let engine = RpcEngine::new(Arc::new(SuccessfulDispatcher));
    for index in 0..60 {
        let response = execute(
            &engine,
            &format!("recover-{index}"),
            "recover_render_attempt_in_split_project_folder",
            100,
        );
        assert!(response.ok, "{:?}", response.error);
    }
    let ordinary_job = execute(
        &engine,
        "recover-limited",
        "recover_render_attempt_in_split_project_folder",
        101,
    );
    assert_eq!(ordinary_job.error.unwrap().code, RpcErrorCode::RateLimited);
    let unknown = execute(&engine, "spoof-stop", "cancel_unknown_operation", 101);
    assert_eq!(unknown.error.unwrap().code, RpcErrorCode::NotFound);
    let body = serde_json::to_vec(&envelope("denied-stop", STOP)).unwrap();
    let denied = engine.execute(
        "session-test",
        &BTreeSet::from([AuthorizationScope::Session]),
        &body,
        101,
    );
    assert_eq!(denied.error.unwrap().code, RpcErrorCode::Forbidden);
    let mut request = envelope("unleased-stop", STOP);
    request.editor_lease_token = None;
    let unleased = engine.execute(
        "session-test",
        &scopes(),
        &serde_json::to_vec(&request).unwrap(),
        101,
    );
    assert_eq!(unleased.error.unwrap().code, RpcErrorCode::InvalidRequest);
    for index in 0..30 {
        assert!(execute(&engine, &format!("valid-stop-{index}"), STOP, 101).ok);
    }
    assert_eq!(
        execute(&engine, "stop-reserve-finite", STOP, 101)
            .error
            .unwrap()
            .code,
        RpcErrorCode::RateLimited
    );
}

#[test]
fn public_unclassified_limiter_retains_custom_rate_and_concurrency_contract() {
    let limiter = SessionLimiter::new(2, 1, 10);
    let first = limiter.begin("session", 100).unwrap();
    assert!(matches!(
        limiter.begin("session", 100),
        Err(SessionLimitError::Concurrent)
    ));
    drop(first);
    drop(limiter.begin("session", 101).unwrap());
    assert!(matches!(
        limiter.begin("session", 102),
        Err(SessionLimitError::Rate {
            retry_after_ms: 8_000
        })
    ));
    assert!(limiter.begin("session", 110).is_ok());
}

#[test]
fn session_population_is_bounded_without_evicting_live_permits() {
    let limiter = SessionLimiter::new(60, 4, 60);
    let retained = limiter.begin("retained", 100).unwrap();
    for index in 0..2047 {
        drop(limiter.begin(&format!("ephemeral-{index}"), 100).unwrap());
    }
    assert!(
        limiter.begin("overflow", 100).is_err(),
        "unbounded session population accepted"
    );
    assert_eq!(limiter.sessions.lock().unwrap().len(), 2048);
    drop(limiter.begin("after-expiry", 160).unwrap());
    assert!(limiter.sessions.lock().unwrap().contains_key("retained"));
    drop(retained);
    drop(limiter.begin("after-retained-expiry", 220).unwrap());
    assert!(!limiter.sessions.lock().unwrap().contains_key("retained"));
}

#[test]
fn stop_dispatches_while_the_ordinary_replay_cache_is_full_of_pending_work() {
    let (entered, receiver) = mpsc::channel();
    let dispatcher = Arc::new(BlockingDispatcher {
        entered,
        released: Mutex::new(false),
        ready: Condvar::new(),
        calls: AtomicUsize::new(0),
    });
    let mut engine = RpcEngine::new(dispatcher.clone());
    engine.idempotency = IdempotencyStore::new(1, 300);
    let engine = Arc::new(engine);
    let ordinary_engine = engine.clone();
    let ordinary =
        std::thread::spawn(move || execute(&ordinary_engine, "blocked-ordinary-cache", WRITE, 100));
    let ordinary_entered = receiver.recv_timeout(Duration::from_secs(2)).is_ok();
    let stop_engine = engine.clone();
    let (finished, finish) = mpsc::channel();
    let stop = std::thread::spawn(move || {
        let response = execute(&stop_engine, "stop-at-cache-capacity", STOP, 100);
        finished.send(response.clone()).unwrap();
        response
    });
    let stop_before_release = finish.recv_timeout(Duration::from_secs(2)).ok();
    // Unblock and join both calls before asserting, including on the RED path.
    dispatcher.release();
    let ordinary = ordinary.join().unwrap();
    let stop = stop.join().unwrap();
    assert!(ordinary_entered && ordinary.ok);
    assert!(
        stop_before_release.is_some_and(|response| response.ok),
        "validated Stop waited behind a full ordinary replay cache"
    );
    assert!(stop.ok);
    let replay = execute(&engine, "stop-at-cache-capacity", STOP, 101);
    assert_eq!(replay, stop);
    let mut changed = envelope("stop-at-cache-capacity", STOP);
    changed.payload = json!({"jobId":"different-job"});
    let conflict = engine.execute(
        "session-test",
        &scopes(),
        &serde_json::to_vec(&changed).unwrap(),
        101,
    );
    assert_eq!(conflict.error.unwrap().code, RpcErrorCode::Conflict);
}

#[test]
fn occupied_stop_cache_reserve_rejects_another_stop_but_does_not_use_an_ordinary_slot() {
    let (entered, receiver) = mpsc::channel();
    let dispatcher = Arc::new(BlockingDispatcher {
        entered,
        released: Mutex::new(false),
        ready: Condvar::new(),
        calls: AtomicUsize::new(0),
    });
    let mut engine = RpcEngine::new(dispatcher.clone());
    engine.idempotency = IdempotencyStore::new(1, 300);
    let engine = Arc::new(engine);
    let first_engine = engine.clone();
    let first = std::thread::spawn(move || execute(&first_engine, "blocked-stop-cache", STOP, 100));
    let first_entered = receiver.recv_timeout(Duration::from_secs(2)).is_ok();
    let overflow_engine = engine.clone();
    let (overflow_sent, overflow_received) = mpsc::channel();
    let overflow = std::thread::spawn(move || {
        // A different session proves that the shared cache, rather than the per-session
        // Stop concurrency permit, bounds this second cancellation.
        let body = serde_json::to_vec(&envelope("stop-cache-overflow", STOP)).unwrap();
        let response = overflow_engine.execute("other-session", &scopes(), &body, 100);
        overflow_sent.send(response.clone()).unwrap();
        response
    });
    let ordinary_engine = engine.clone();
    let (ordinary_sent, ordinary_received) = mpsc::channel();
    let ordinary = std::thread::spawn(move || {
        let response = execute(
            &ordinary_engine,
            "ordinary-with-stop-cache-full",
            WRITE,
            100,
        );
        ordinary_sent.send(response.clone()).unwrap();
        response
    });
    let overflow_before_release = overflow_received.recv_timeout(Duration::from_secs(2)).ok();
    let ordinary_before_release = ordinary_received.recv_timeout(Duration::from_secs(2)).ok();
    dispatcher.release();
    let first = first.join().unwrap();
    let overflow = overflow.join().unwrap();
    let ordinary = ordinary.join().unwrap();
    assert!(first_entered && first.ok);
    assert_eq!(
        overflow_before_release.and_then(|response| response.error.map(|error| error.code)),
        Some(RpcErrorCode::Busy),
        "a full Stop cache reserve must reject before dispatch instead of waiting"
    );
    assert_eq!(overflow.error.unwrap().code, RpcErrorCode::Busy);
    assert!(
        ordinary_before_release.is_some_and(|response| response.ok),
        "a Stop reservation consumed ordinary replay capacity"
    );
    assert!(ordinary.ok);
}
