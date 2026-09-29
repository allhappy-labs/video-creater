use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};

use sha2::{Digest, Sha256};

use super::rpc::RpcResponse;

#[derive(Debug, Clone, PartialEq)]
pub enum IdempotencyDecision {
    New,
    Replay(RpcResponse),
    PayloadConflict,
}

#[derive(Clone)]
struct Entry {
    payload_hash: [u8; 32],
    response: Option<RpcResponse>,
    expires_at: u64,
}

#[derive(Default)]
struct State {
    entries: HashMap<(String, String), Entry>,
    order: VecDeque<(String, String)>,
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
                .map_or(IdempotencyDecision::New, IdempotencyDecision::Replay),
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
                    return Err(IdempotencyDecision::Replay(response));
                }
                drop(
                    self.changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                );
                continue;
            }
            if state.entries.len() >= self.capacity {
                if let Some(index) = state.order.iter().position(|candidate| {
                    state
                        .entries
                        .get(candidate)
                        .is_some_and(|entry| entry.response.is_some())
                }) {
                    if let Some(completed) = state.order.remove(index) {
                        state.entries.remove(&completed);
                    }
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
                    expires_at: now.saturating_add(self.ttl_seconds),
                },
            );
            drop(state);

            let response = action.take().expect("idempotent action runs at most once")();
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(entry) = state.entries.get_mut(&key) {
                entry.response = Some(response.clone());
                entry.expires_at = now.saturating_add(self.ttl_seconds);
            }
            self.changed.notify_all();
            return Ok(response);
        }
    }
}

fn digest(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}

fn prune(state: &mut State, now: u64) {
    state
        .entries
        .retain(|_, entry| entry.response.is_none() || now <= entry.expires_at);
    state.order.retain(|key| state.entries.contains_key(key));
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
                    Ok(response) | Err(IdempotencyDecision::Replay(response)) => response,
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
}
