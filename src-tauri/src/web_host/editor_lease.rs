use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorLease {
    pub project_id: String,
    pub session_id: String,
    pub token: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseAuditRecord {
    pub project_id: String,
    pub session_id: String,
    pub action: &'static str,
    pub at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    Held,
    Invalid,
    Expired,
}

#[derive(Default)]
struct State {
    leases: HashMap<String, EditorLease>,
    disconnected_until: HashMap<String, u64>,
    audit: Vec<LeaseAuditRecord>,
}

pub struct LeaseManager {
    duration_seconds: u64,
    disconnect_grace_seconds: u64,
    state: Mutex<State>,
    mutation_locks: Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>>,
}

impl LeaseManager {
    pub fn new(duration_seconds: u64, disconnect_grace_seconds: u64) -> Self {
        Self {
            duration_seconds: duration_seconds.max(1),
            disconnect_grace_seconds,
            state: Mutex::new(State::default()),
            mutation_locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn acquire(
        &self,
        project_id: &str,
        session_id: &str,
        now: u64,
    ) -> Result<EditorLease, LeaseError> {
        let project_lock = self.project_lock(project_id);
        let _mutation = project_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = self.lock();
        expire_project(&mut state, project_id, now);
        if state.leases.contains_key(project_id) {
            return Err(LeaseError::Held);
        }
        Ok(insert_lease(
            &mut state,
            project_id,
            session_id,
            now,
            self.duration_seconds,
            "acquire",
        ))
    }

    pub fn acquire_or_renew_for_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: u64,
    ) -> Result<EditorLease, LeaseError> {
        let project_lock = self.project_lock(project_id);
        let _mutation = project_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = self.lock();
        expire_project(&mut state, project_id, now);
        if let Some(lease) = state.leases.get_mut(project_id) {
            if lease.session_id != session_id {
                return Err(LeaseError::Held);
            }
            lease.expires_at = now.saturating_add(self.duration_seconds);
            return Ok(lease.clone());
        }
        Ok(insert_lease(
            &mut state,
            project_id,
            session_id,
            now,
            self.duration_seconds,
            "acquire",
        ))
    }

    pub fn renew(
        &self,
        project_id: &str,
        session_id: &str,
        token: &str,
        now: u64,
    ) -> Result<EditorLease, LeaseError> {
        let project_lock = self.project_lock(project_id);
        let _mutation = project_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = self.lock();
        validate_locked(&mut state, project_id, session_id, token, now)?;
        let lease = state
            .leases
            .get_mut(project_id)
            .ok_or(LeaseError::Invalid)?;
        lease.expires_at = now.saturating_add(self.duration_seconds);
        let result = lease.clone();
        state.disconnected_until.remove(session_id);
        Ok(result)
    }

    pub fn takeover(
        &self,
        project_id: &str,
        session_id: &str,
        now: u64,
    ) -> Result<EditorLease, LeaseError> {
        let project_lock = self.project_lock(project_id);
        let _mutation = project_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = self.lock();
        state.leases.remove(project_id);
        Ok(insert_lease(
            &mut state,
            project_id,
            session_id,
            now,
            self.duration_seconds,
            "takeover",
        ))
    }

    pub fn validate(
        &self,
        project_id: &str,
        session_id: &str,
        token: &str,
        now: u64,
    ) -> Result<(), LeaseError> {
        let mut state = self.lock();
        validate_locked(&mut state, project_id, session_id, token, now)
    }

    pub fn validate_owner(
        &self,
        project_id: &str,
        session_id: &str,
        token: &str,
    ) -> Result<(), LeaseError> {
        let state = self.lock();
        let lease = state.leases.get(project_id).ok_or(LeaseError::Invalid)?;
        if lease.session_id != session_id || !constant_eq(&lease.token, token) {
            return Err(LeaseError::Invalid);
        }
        Ok(())
    }

    /// Validates a lease and keeps the per-project mutation gate locked until
    /// the mutation completes, so takeover cannot invalidate a writer mid-commit.
    pub fn with_valid_lease<T>(
        &self,
        project_id: &str,
        session_id: &str,
        token: &str,
        now: u64,
        action: impl FnOnce() -> T,
    ) -> Result<T, LeaseError> {
        let project_lock = self.project_lock(project_id);
        let _mutation = project_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        {
            let mut state = self.lock();
            validate_locked(&mut state, project_id, session_id, token, now)?;
        }
        Ok(action())
    }

    pub fn release(
        &self,
        project_id: &str,
        session_id: &str,
        token: &str,
    ) -> Result<(), LeaseError> {
        let project_lock = self.project_lock(project_id);
        let _mutation = project_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = self.lock();
        let lease = state.leases.get(project_id).ok_or(LeaseError::Invalid)?;
        if lease.session_id != session_id || !constant_eq(&lease.token, token) {
            return Err(LeaseError::Invalid);
        }
        state.leases.remove(project_id);
        Ok(())
    }

    pub fn disconnected(&self, session_id: &str, now: u64) {
        self.lock().disconnected_until.insert(
            session_id.to_owned(),
            now.saturating_add(self.disconnect_grace_seconds),
        );
    }

    pub fn audit_records(&self) -> Vec<LeaseAuditRecord> {
        self.lock().audit.clone()
    }

    pub fn current(&self, project_id: &str, now: u64) -> Option<EditorLease> {
        let mut state = self.lock();
        expire_project(&mut state, project_id, now);
        state.leases.get(project_id).cloned()
    }

    pub fn background_job_allowed(&self, _project_id: &str) -> bool {
        true
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn project_lock(&self, project_id: &str) -> std::sync::Arc<Mutex<()>> {
        self.mutation_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(project_id.to_owned())
            .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
            .clone()
    }
}

fn insert_lease(
    state: &mut State,
    project_id: &str,
    session_id: &str,
    now: u64,
    duration: u64,
    action: &'static str,
) -> EditorLease {
    let lease = EditorLease {
        project_id: project_id.to_owned(),
        session_id: session_id.to_owned(),
        token: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
        expires_at: now.saturating_add(duration),
    };
    state.leases.insert(project_id.to_owned(), lease.clone());
    state.audit.push(LeaseAuditRecord {
        project_id: project_id.to_owned(),
        session_id: session_id.to_owned(),
        action,
        at: now,
    });
    lease
}

fn expire_project(state: &mut State, project_id: &str, now: u64) {
    if state
        .leases
        .get(project_id)
        .is_some_and(|lease| now > lease.expires_at)
    {
        state.leases.remove(project_id);
    }
}

fn validate_locked(
    state: &mut State,
    project_id: &str,
    session_id: &str,
    token: &str,
    now: u64,
) -> Result<(), LeaseError> {
    let Some(lease) = state.leases.get(project_id) else {
        return Err(LeaseError::Invalid);
    };
    if lease.session_id != session_id || !constant_eq(&lease.token, token) {
        return Err(LeaseError::Invalid);
    }
    let expired = now > lease.expires_at
        || state
            .disconnected_until
            .get(session_id)
            .is_some_and(|expiry| now > *expiry);
    if expired {
        state.leases.remove(project_id);
        return Err(LeaseError::Expired);
    }
    Ok(())
}

fn constant_eq(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn takeover_waits_for_an_authorized_mutation_to_finish() {
        let leases = Arc::new(LeaseManager::new(30, 5));
        let first = leases.acquire("project", "first", 100).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let mutation_leases = Arc::clone(&leases);
        let mutation = std::thread::spawn(move || {
            mutation_leases
                .with_valid_lease("project", "first", &first.token, 101, || {
                    started_tx.send(()).unwrap();
                    finish_rx.recv().unwrap();
                })
                .unwrap();
        });
        started_rx.recv().unwrap();
        let (takeover_tx, takeover_rx) = mpsc::channel();
        let takeover_leases = Arc::clone(&leases);
        let takeover = std::thread::spawn(move || {
            let lease = takeover_leases.takeover("project", "second", 102).unwrap();
            takeover_tx.send(lease).unwrap();
        });
        assert!(takeover_rx.recv_timeout(Duration::from_millis(20)).is_err());
        finish_tx.send(()).unwrap();
        mutation.join().unwrap();
        assert_eq!(
            takeover_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .session_id,
            "second"
        );
        takeover.join().unwrap();
    }
}
