use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};

use sha2::{Digest, Sha256};

use super::rpc::RpcResponse;

#[derive(Debug, Clone, PartialEq)]
pub enum IdempotencyDecision {
    New,
    // Box the Rust decision payload; RPC serialization still uses the same response value.
    Replay(Box<RpcResponse>),
    PayloadConflict,
    Capacity,
}

#[derive(Clone)]
struct Entry {
    payload_hash: [u8; 32],
    response: Option<RpcResponse>,
    expires_at: u64,
    response_bytes: usize,
    cancellation: bool,
}

#[derive(Default)]
struct State {
    entries: HashMap<(String, String), Entry>,
    order: VecDeque<(String, String)>,
    response_bytes: usize,
}

pub struct IdempotencyStore {
    capacity: usize,
    ttl_seconds: u64,
    state: Mutex<State>,
    changed: Condvar,
}

impl IdempotencyStore {
    pub fn new(capacity: usize, ttl_seconds: u64) -> Self {
        Self {
            capacity: capacity.max(1),
            ttl_seconds,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    pub fn check(
        &self,
        session_id: &str,
        request_id: &str,
        payload: &[u8],
        now: u64,
    ) -> IdempotencyDecision {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prune(&mut state, now);
        let key = (session_id.to_owned(), request_id.to_owned());
        match state.entries.get(&key) {
            None => IdempotencyDecision::New,
            Some(entry) if entry.payload_hash == digest(payload) => entry
                .response
                .clone()
                .map_or(IdempotencyDecision::New, |response| {
                    IdempotencyDecision::Replay(Box::new(response))
                }),
            Some(_) => IdempotencyDecision::PayloadConflict,
        }
    }

    pub fn record(
        &self,
        session_id: &str,
        request_id: &str,
        payload: &[u8],
        response: RpcResponse,
        now: u64,
    ) -> Result<(), IdempotencyDecision> {
        self.execute_once(session_id, request_id, payload, now, || response)
            .map(|_| ())
    }

    pub fn execute_once(
        &self,
        session_id: &str,
        request_id: &str,
        payload: &[u8],
        now: u64,
        action: impl FnOnce() -> RpcResponse,
    ) -> Result<RpcResponse, IdempotencyDecision> {
        self.execute_classified(session_id, request_id, payload, now, false, action)
    }

    /// Only RpcEngine's validated cancellation class can use this finite reserve.
    /// Both classes share replay identity and the same retained-response byte budget.
    pub(super) fn execute_once_cancellation(
        &self,
        session_id: &str,
        request_id: &str,
        payload: &[u8],
        now: u64,
        action: impl FnOnce() -> RpcResponse,
    ) -> Result<RpcResponse, IdempotencyDecision> {
        self.execute_classified(session_id, request_id, payload, now, true, action)
    }

    fn execute_classified(
        &self,
        session_id: &str,
        request_id: &str,
        payload: &[u8],
        now: u64,
        cancellation: bool,
        action: impl FnOnce() -> RpcResponse,
    ) -> Result<RpcResponse, IdempotencyDecision> {
        let key = (session_id.to_owned(), request_id.to_owned());
        let payload_hash = digest(payload);
        let mut action = Some(action);
        loop {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            prune(&mut state, now);
            if let Some(entry) = state.entries.get(&key) {
                if entry.payload_hash != payload_hash {
                    return Err(IdempotencyDecision::PayloadConflict);
                }
                if let Some(response) = entry.response.clone() {
                    return Err(IdempotencyDecision::Replay(Box::new(response)));
                }
                drop(
                    self.changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
                continue;
            }
            let capacity = if cancellation {
                self.capacity.min(MAX_CANCELLATION_ENTRIES)
            } else {
                self.capacity
            };
            let entries = state
                .entries
                .values()
                .filter(|entry| entry.cancellation == cancellation)
                .count();
            if entries >= capacity {
                if let Some(index) = state.order.iter().position(|candidate| {
                    state.entries.get(candidate).is_some_and(|entry| {
                        entry.cancellation == cancellation && entry.response.is_some()
                    })
                }) {
                    if let Some(completed) = state.order.remove(index) {
                        if let Some(entry) = state.entries.remove(&completed) {
                            state.response_bytes =
                                state.response_bytes.saturating_sub(entry.response_bytes);
                        }
                    }
                } else if cancellation {
                    // Stop never waits behind ordinary work or other stalled cancellations.
                    // This rejection precedes dispatch and durable journal acceptance.
                    return Err(IdempotencyDecision::Capacity);
                } else {
                    // Every slot is an in-flight reservation. Wait for a completion rather than
                    // exceeding the configured bound or evicting another request's reservation.
                    drop(
                        self.changed
                            .wait(state)
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                    continue;
                }
            }
            state.order.push_back(key.clone());
            state.entries.insert(
                key.clone(),
                Entry {
                    payload_hash,
                    response: None,
                    response_bytes: 0,
                    expires_at: now.saturating_add(self.ttl_seconds),
                    cancellation,
                },
            );
            drop(state);

            let response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                action.take().expect("idempotent action runs at most once"),
            )) {
                Ok(response) => response,
                Err(panic) => {
                    let receipt = RpcResponse::error(request_id, super::rpc::RpcErrorCode::OutcomeUnknown,
                        "The original dispatcher was interrupted. Resolve its durable outcome and refresh canonical state; do not redispatch.", None);
                    let bytes = response_size(&receipt, MAX_RESPONSE_BYTES).unwrap_or(0);
                    let mut state = self
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(entry) = state.entries.get_mut(&key) {
                        entry.response = Some(receipt);
                        entry.response_bytes = bytes;
                        entry.expires_at = now.saturating_add(self.ttl_seconds);
                        state.response_bytes += bytes;
                    }
                    self.changed.notify_all();
                    drop(state);
                    std::panic::resume_unwind(panic);
                }
            };
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let retained = bounded_response(
                &response,
                MAX_CACHE_BYTES
                    .saturating_sub(RECEIPT_RESERVE)
                    .saturating_sub(state.response_bytes),
            );
            let bytes = response_size(&retained, MAX_RESPONSE_BYTES).unwrap_or(MAX_RESPONSE_BYTES);
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.response = Some(retained);
                entry.response_bytes = bytes;
                entry.expires_at = now.saturating_add(self.ttl_seconds);
                state.response_bytes += bytes;
            }
            self.changed.notify_all();
            return Ok(response);
        }
    }
}

const MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const RECEIPT_RESERVE: usize = 1024 * 1024;
const MAX_CANCELLATION_ENTRIES: usize = 64;

#[cfg(test)]
#[path = "idempotency_reserve_tests.rs"]
mod reserve_tests;

fn bounded_response(response: &RpcResponse, available: usize) -> RpcResponse {
    if response_size(response, available.min(MAX_RESPONSE_BYTES)).is_some() {
        return response.clone();
    }
    RpcResponse::error(&response.request_id, super::rpc::RpcErrorCode::OutcomeUnknown,
        "The accepted request response exceeds replay cache capacity. Resolve its durable outcome and read canonical state; do not redispatch.", None)
}

// Count serialized bytes without allocating an additional response-sized buffer.
fn response_size(response: &RpcResponse, maximum: usize) -> Option<usize> {
    struct Counter {
        bytes: usize,
        maximum: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self.bytes.saturating_add(bytes.len());
            if next > self.maximum {
                return Err(std::io::Error::other("response byte limit"));
            }
            self.bytes = next;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter { bytes: 0, maximum };
    serde_json::to_writer(&mut counter, response).ok()?;
    Some(counter.bytes)
}

fn digest(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}

fn prune(state: &mut State, now: u64) {
    state
        .entries
        .retain(|_, entry| entry.response.is_none() || now <= entry.expires_at);
    state.order.retain(|key| state.entries.contains_key(key));
    state.response_bytes = state
        .entries
        .values()
        .map(|entry| entry.response_bytes)
        .sum();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn concurrent_retries_coalesce_around_one_execution() {
        let store = Arc::new(IdempotencyStore::new(8, 60));
        let executions = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for _ in 0..4 {
            let store = Arc::clone(&store);
            let executions = Arc::clone(&executions);
            threads.push(std::thread::spawn(move || {
                let result = store.execute_once("session", "request", b"payload", 100, || {
                    executions.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    RpcResponse::success("request", json!({"revision":2}))
                });
                match result {
                    Ok(response) => response,
                    Err(IdempotencyDecision::Replay(response)) => *response,
                    other => panic!("unexpected idempotency result: {other:?}"),
                }
            }));
        }
        let responses = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert!(responses.iter().all(|response| response == &responses[0]));
        assert_eq!(executions.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn capacity_waits_for_an_in_flight_reservation_instead_of_growing() {
        use std::sync::atomic::AtomicBool;

        let store = Arc::new(IdempotencyStore::new(1, 60));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let first_store = Arc::clone(&store);
        let first_release = Arc::clone(&release);
        let first = std::thread::spawn(move || {
            first_store
                .execute_once("session", "first", b"one", 100, || {
                    started_tx.send(()).unwrap();
                    let (lock, changed) = &*first_release;
                    let mut ready = lock.lock().unwrap();
                    while !*ready {
                        ready = changed.wait(ready).unwrap();
                    }
                    RpcResponse::success("first", json!({"done":1}))
                })
                .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        let second_executed = Arc::new(AtomicBool::new(false));
        let second_store = Arc::clone(&store);
        let second_flag = Arc::clone(&second_executed);
        let second = std::thread::spawn(move || {
            second_store
                .execute_once("session", "second", b"two", 100, || {
                    second_flag.store(true, Ordering::SeqCst);
                    RpcResponse::success("second", json!({"done":2}))
                })
                .unwrap()
        });
        std::thread::sleep(Duration::from_millis(20));
        assert!(!second_executed.load(Ordering::SeqCst));
        assert_eq!(store.state.lock().unwrap().entries.len(), 1);

        let (lock, changed) = &*release;
        *lock.lock().unwrap() = true;
        changed.notify_all();
        assert_eq!(first.join().unwrap().request_id, "first");
        assert_eq!(second.join().unwrap().request_id, "second");
        assert!(second_executed.load(Ordering::SeqCst));
        assert_eq!(store.state.lock().unwrap().entries.len(), 1);
    }
    #[test]
    fn oversized_response_retains_a_small_receipt_and_never_reexecutes() {
        let store = IdempotencyStore::new(8, 60);
        let reply = RpcResponse::success("large", json!({"project": "x".repeat(9 * 1024 * 1024)}));
        assert!(
            store
                .execute_once("session", "large", b"body", 100, || reply)
                .unwrap()
                .ok
        );
        let state = store.state.lock().unwrap();
        let retained = state
            .entries
            .get(&("session".into(), "large".into()))
            .unwrap()
            .response
            .as_ref()
            .unwrap();
        eprintln!(
            "retained_oversized_response_bytes={}",
            serde_json::to_vec(retained).unwrap().len()
        );
        assert!(
            serde_json::to_vec(retained).unwrap().len() < 4096,
            "oversized RPC response retained in memory"
        );
        drop(state);
        let replay = store.execute_once("session", "large", b"body", 101, || {
            panic!("accepted operation redispatched")
        });
        assert!(matches!(replay, Err(IdempotencyDecision::Replay(_))));
    }

    #[test]
    fn total_response_cache_is_bounded_without_losing_replay_receipts() {
        let store = IdempotencyStore::new(32, 60);
        for index in 0..10 {
            let id = format!("big-{index}");
            store
                .execute_once("session", &id, b"body", 100, || {
                    RpcResponse::success(&id, json!({"payload":"x".repeat(7 * 1024 * 1024)}))
                })
                .unwrap();
        }
        let state = store.state.lock().unwrap();
        let bytes: usize = state
            .entries
            .values()
            .filter_map(|entry| entry.response.as_ref())
            .map(|response| serde_json::to_vec(response).unwrap().len())
            .sum();
        eprintln!("retained_ten_7mib_response_bytes={bytes}");
        assert!(
            bytes <= 64 * 1024 * 1024,
            "response cache retained {bytes} bytes"
        );
        assert_eq!(
            state.entries.len(),
            10,
            "accepted replay receipts were dropped"
        );
    }
    #[test]
    fn unwinding_dispatch_releases_replay_waiters_with_an_uncertain_receipt() {
        let store = IdempotencyStore::new(1, 60);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.execute_once("session", "panicked", b"body", 100, || {
                panic!("injected dispatcher panic")
            })
        }));
        assert!(unwind.is_err());
        let state = store.state.lock().unwrap();
        let response = state
            .entries
            .get(&("session".into(), "panicked".into()))
            .unwrap()
            .response
            .as_ref();
        assert!(
            response.is_some(),
            "dispatcher panic left a permanently pending cache reservation"
        );
        assert!(!response.unwrap().ok);
        drop(state);
        let replay = store.execute_once("session", "panicked", b"body", 101, || {
            panic!("panicked operation reexecuted")
        });
        assert!(matches!(replay, Err(IdempotencyDecision::Replay(_))));
        assert!(
            store
                .execute_once("session", "new", b"new", 101, || RpcResponse::success(
                    "new",
                    json!({})
                ))
                .unwrap()
                .ok
        );
    }
}
