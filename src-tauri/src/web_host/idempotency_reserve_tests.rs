use super::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

#[test]
fn duplicate_stop_coalesces_around_the_existing_pending_reserved_entry() {
    let store = Arc::new(IdempotencyStore::new(1, 60));
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, entering) = mpsc::channel();
    let first_store = store.clone();
    let first_calls = calls.clone();
    let first_release = release.clone();
    let first = std::thread::spawn(move || {
        first_store.execute_once_cancellation("session", "stop", b"body", 100, || {
            first_calls.fetch_add(1, Ordering::SeqCst);
            entered.send(()).unwrap();
            let (lock, changed) = &*first_release;
            drop(
                changed
                    .wait_while(lock.lock().unwrap(), |released| !*released)
                    .unwrap(),
            );
            RpcResponse::success("stop", json!({"cancelled": true}))
        })
    });
    let first_entered = entering.recv_timeout(Duration::from_secs(2)).is_ok();
    let duplicate_store = store.clone();
    let duplicate_calls = calls.clone();
    let (started, starting) = mpsc::channel();
    let (finished, finishing) = mpsc::channel();
    let duplicate = std::thread::spawn(move || {
        started.send(()).unwrap();
        let response =
            duplicate_store.execute_once_cancellation("session", "stop", b"body", 100, || {
                duplicate_calls.fetch_add(1, Ordering::SeqCst);
                RpcResponse::success("stop", json!({"cancelled": false}))
            });
        finished.send(response.clone()).unwrap();
        response
    });
    let duplicate_started = starting.recv_timeout(Duration::from_secs(2)).is_ok();
    let duplicate_waited = finishing.recv_timeout(Duration::from_millis(100)).is_err();
    let (lock, changed) = &*release;
    *lock.lock().unwrap() = true;
    changed.notify_all();
    let first = first.join().unwrap();
    let duplicate = duplicate.join().unwrap();
    assert!(first_entered && duplicate_started && duplicate_waited);
    let first = first.unwrap();
    assert_eq!(duplicate, Err(IdempotencyDecision::Replay(Box::new(first))));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn panicked_stop_retains_uncertainty_and_releases_its_reserved_slot() {
    let store = IdempotencyStore::new(1, 60);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.execute_once_cancellation("session", "panicked-stop", b"body", 100, || {
            panic!("injected Stop dispatcher unwind")
        })
    }));
    assert!(panic.is_err());
    let replay = store.execute_once_cancellation("session", "panicked-stop", b"body", 101, || {
        panic!("uncertain Stop reexecuted")
    });
    assert!(matches!(replay, Err(IdempotencyDecision::Replay(response))
        if response.error.as_ref().is_some_and(|error| error.code == super::super::rpc::RpcErrorCode::OutcomeUnknown)));
    assert!(
        store
            .execute_once("session", "ordinary", b"ordinary", 101, || {
                RpcResponse::success("ordinary", json!({"edited": true}))
            })
            .unwrap()
            .ok
    );
    assert!(
        store
            .execute_once_cancellation("session", "next-stop", b"next", 101, || {
                RpcResponse::success("next-stop", json!({"cancelled": true}))
            })
            .unwrap()
            .ok
    );
}

#[test]
fn ordinary_and_stop_responses_share_one_total_byte_ceiling_and_keep_replay_receipts() {
    let store = IdempotencyStore::new(32, 60);
    for index in 0..16 {
        let id = format!("large-{index}");
        let action = || RpcResponse::success(&id, json!({"project": "x".repeat(7 * 1024 * 1024)}));
        let response = if index < 10 {
            store.execute_once("session", &id, b"body", 100, action)
        } else {
            store.execute_once_cancellation("session", &id, b"body", 100, action)
        };
        assert!(response.unwrap().ok);
    }
    let state = store.state.lock().unwrap();
    let retained_bytes: usize = state
        .entries
        .values()
        .map(|entry| {
            serde_json::to_vec(entry.response.as_ref().unwrap())
                .unwrap()
                .len()
        })
        .sum();
    assert!(
        retained_bytes <= 64 * 1024 * 1024,
        "combined cache retained {retained_bytes} bytes"
    );
    assert_eq!(state.response_bytes, retained_bytes);
    assert_eq!(
        state.entries.len(),
        16,
        "response pressure dropped a replay receipt"
    );
    drop(state);
    for index in 0..16 {
        let id = format!("large-{index}");
        let action = || panic!("oversized cached operation reexecuted");
        let replay = if index < 10 {
            store.execute_once("session", &id, b"body", 101, action)
        } else {
            store.execute_once_cancellation("session", &id, b"body", 101, action)
        };
        assert!(matches!(replay, Err(IdempotencyDecision::Replay(_))));
    }
}
