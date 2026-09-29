use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GenerationCancellationPhase {
    Active = 0,
    CancelRequested = 1,
    Completing = 2,
    Terminal = 3,
}

impl GenerationCancellationPhase {
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Active,
            1 => Self::CancelRequested,
            2 => Self::Completing,
            3 => Self::Terminal,
            _ => unreachable!("generation cancellation phase is written only by this module"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationCancellationRequestOutcome {
    Requested,
    AlreadyRequested,
    TooLate,
    NotFound,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GenerationCancellationRegistrationError {
    #[error("generation cancellation {field} cannot be blank")]
    BlankKey { field: &'static str },
    #[error("generation job is already registered for cancellation: {project_id}/{job_id}")]
    DuplicateRegistration { project_id: String, job_id: String },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GenerationJobKey {
    project_id: String,
    job_id: String,
}

impl GenerationJobKey {
    fn new(
        project_id: &str,
        job_id: &str,
    ) -> Result<Self, GenerationCancellationRegistrationError> {
        let project_id = project_id.trim();
        if project_id.is_empty() {
            return Err(GenerationCancellationRegistrationError::BlankKey { field: "projectId" });
        }
        let job_id = job_id.trim();
        if job_id.is_empty() {
            return Err(GenerationCancellationRegistrationError::BlankKey { field: "jobId" });
        }
        Ok(Self {
            project_id: project_id.to_string(),
            job_id: job_id.to_string(),
        })
    }

    fn normalized(project_id: &str, job_id: &str) -> Option<Self> {
        Self::new(project_id, job_id).ok()
    }
}

#[derive(Debug)]
struct GenerationCancellationState {
    phase: AtomicU8,
    cancellation_was_requested: AtomicBool,
    wait_lock: Mutex<()>,
    wait_condition: Condvar,
}

impl GenerationCancellationState {
    fn new() -> Self {
        Self {
            phase: AtomicU8::new(GenerationCancellationPhase::Active as u8),
            cancellation_was_requested: AtomicBool::new(false),
            wait_lock: Mutex::new(()),
            wait_condition: Condvar::new(),
        }
    }

    fn phase(&self) -> GenerationCancellationPhase {
        GenerationCancellationPhase::from_u8(self.phase.load(Ordering::Acquire))
    }

    fn request_cancellation(&self) -> GenerationCancellationRequestOutcome {
        loop {
            match self.phase() {
                GenerationCancellationPhase::Active => {
                    if self
                        .phase
                        .compare_exchange(
                            GenerationCancellationPhase::Active as u8,
                            GenerationCancellationPhase::CancelRequested as u8,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        self.cancellation_was_requested
                            .store(true, Ordering::Release);
                        self.wait_condition.notify_all();
                        return GenerationCancellationRequestOutcome::Requested;
                    }
                }
                GenerationCancellationPhase::CancelRequested => {
                    self.cancellation_was_requested
                        .store(true, Ordering::Release);
                    self.wait_condition.notify_all();
                    return GenerationCancellationRequestOutcome::AlreadyRequested;
                }
                GenerationCancellationPhase::Completing | GenerationCancellationPhase::Terminal => {
                    return GenerationCancellationRequestOutcome::TooLate;
                }
            }
        }
    }

    fn begin_completion(&self) -> bool {
        self.phase
            .compare_exchange(
                GenerationCancellationPhase::Active as u8,
                GenerationCancellationPhase::Completing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn mark_terminal(&self) {
        self.phase.store(
            GenerationCancellationPhase::Terminal as u8,
            Ordering::Release,
        );
        self.wait_condition.notify_all();
    }

    fn is_cancellation_requested(&self) -> bool {
        self.cancellation_was_requested.load(Ordering::Acquire)
            || self.phase() == GenerationCancellationPhase::CancelRequested
    }

    fn wait_for_cancellation(&self, timeout: Duration) -> bool {
        if self.is_cancellation_requested() {
            return true;
        }
        if timeout.is_zero() {
            return false;
        }
        let guard = self
            .wait_lock
            .lock()
            .expect("generation cancellation wait mutex");
        if self.is_cancellation_requested() {
            return true;
        }
        let _ = self
            .wait_condition
            .wait_timeout_while(guard, timeout, |_| !self.is_cancellation_requested())
            .expect("generation cancellation wait mutex");
        self.is_cancellation_requested()
    }
}

#[derive(Debug, Clone)]
pub struct GenerationCancellationToken {
    state: Arc<GenerationCancellationState>,
}

impl GenerationCancellationToken {
    pub fn phase(&self) -> GenerationCancellationPhase {
        self.state.phase()
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.is_cancellation_requested()
    }

    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        self.state.wait_for_cancellation(timeout)
    }

    /// Claims the completion transition. Exactly one of cancellation and
    /// completion can win while a job is active.
    pub fn begin_completion(&self) -> bool {
        self.state.begin_completion()
    }

    pub fn mark_terminal(&self) {
        self.state.mark_terminal();
    }
}

#[derive(Debug)]
pub struct GenerationCancellationGuard {
    key: GenerationJobKey,
    state: Arc<GenerationCancellationState>,
}

impl GenerationCancellationGuard {
    pub fn token(&self) -> GenerationCancellationToken {
        GenerationCancellationToken {
            state: Arc::clone(&self.state),
        }
    }

    pub fn phase(&self) -> GenerationCancellationPhase {
        self.state.phase()
    }
}

impl Drop for GenerationCancellationGuard {
    fn drop(&mut self) {
        unregister_generation_job_cancellation(&self.key, &self.state);
    }
}

pub fn register_generation_cancellation(
    project_id: &str,
    job_id: &str,
) -> Result<GenerationCancellationGuard, GenerationCancellationRegistrationError> {
    let key = GenerationJobKey::new(project_id, job_id)?;
    let state = Arc::new(GenerationCancellationState::new());
    let mut registrations = generation_cancellations()
        .lock()
        .expect("generation cancellation registry mutex");
    if registrations.contains_key(&key) {
        return Err(
            GenerationCancellationRegistrationError::DuplicateRegistration {
                project_id: key.project_id,
                job_id: key.job_id,
            },
        );
    }
    registrations.insert(key.clone(), Arc::clone(&state));
    drop(registrations);
    Ok(GenerationCancellationGuard { key, state })
}

pub fn request_generation_cancellation(
    project_id: &str,
    job_id: &str,
) -> GenerationCancellationRequestOutcome {
    let Some(key) = GenerationJobKey::normalized(project_id, job_id) else {
        return GenerationCancellationRequestOutcome::NotFound;
    };
    let state = generation_cancellations()
        .lock()
        .expect("generation cancellation registry mutex")
        .get(&key)
        .cloned();
    state
        .map(|state| state.request_cancellation())
        .unwrap_or(GenerationCancellationRequestOutcome::NotFound)
}

pub fn is_generation_job_active(project_id: &str, job_id: &str) -> bool {
    let Some(key) = GenerationJobKey::normalized(project_id, job_id) else {
        return false;
    };
    generation_cancellations()
        .lock()
        .expect("generation cancellation registry mutex")
        .contains_key(&key)
}

fn unregister_generation_job_cancellation(
    key: &GenerationJobKey,
    state: &Arc<GenerationCancellationState>,
) {
    let mut registrations = generation_cancellations()
        .lock()
        .expect("generation cancellation registry mutex");
    if registrations
        .get(key)
        .is_some_and(|registered| Arc::ptr_eq(registered, state))
    {
        registrations.remove(key);
    }
}

fn generation_cancellations(
) -> &'static Mutex<HashMap<GenerationJobKey, Arc<GenerationCancellationState>>> {
    static GENERATION_CANCELLATIONS: OnceLock<
        Mutex<HashMap<GenerationJobKey, Arc<GenerationCancellationState>>>,
    > = OnceLock::new();
    GENERATION_CANCELLATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Instant;

    #[test]
    fn registration_is_scoped_by_project_and_job() {
        let first = register_generation_cancellation("project-a", "job-1").expect("register");
        let second = register_generation_cancellation("project-b", "job-1").expect("register");
        assert!(is_generation_job_active("project-a", "job-1"));
        assert!(is_generation_job_active("project-b", "job-1"));

        assert_eq!(
            request_generation_cancellation("project-a", "job-1"),
            GenerationCancellationRequestOutcome::Requested
        );
        assert!(first.token().is_cancelled());
        assert!(!second.token().is_cancelled());
    }

    #[test]
    fn duplicate_registration_is_rejected_without_replacing_the_active_token() {
        let guard = register_generation_cancellation("cancel-duplicate", "job").expect("register");
        let error = register_generation_cancellation("cancel-duplicate", "job")
            .expect_err("duplicate must fail");
        assert!(matches!(
            error,
            GenerationCancellationRegistrationError::DuplicateRegistration { .. }
        ));
        assert_eq!(
            request_generation_cancellation("cancel-duplicate", "job"),
            GenerationCancellationRequestOutcome::Requested
        );
        assert!(guard.token().is_cancelled());
    }

    #[test]
    fn cancellation_is_idempotent_and_persists_after_terminal_transition() {
        let guard = register_generation_cancellation("cancel-idempotent", "job").expect("register");
        let token = guard.token();
        assert_eq!(
            request_generation_cancellation("cancel-idempotent", "job"),
            GenerationCancellationRequestOutcome::Requested
        );
        assert_eq!(
            request_generation_cancellation("cancel-idempotent", "job"),
            GenerationCancellationRequestOutcome::AlreadyRequested
        );
        token.mark_terminal();
        assert_eq!(token.phase(), GenerationCancellationPhase::Terminal);
        assert!(token.is_cancelled());
        assert_eq!(
            request_generation_cancellation("cancel-idempotent", "job"),
            GenerationCancellationRequestOutcome::TooLate
        );
    }

    #[test]
    fn completion_claim_blocks_late_cancellation() {
        let guard = register_generation_cancellation("cancel-completing", "job").expect("register");
        let token = guard.token();
        assert!(token.begin_completion());
        assert!(!token.begin_completion());
        assert_eq!(token.phase(), GenerationCancellationPhase::Completing);
        assert_eq!(
            request_generation_cancellation("cancel-completing", "job"),
            GenerationCancellationRequestOutcome::TooLate
        );
        token.mark_terminal();
        assert_eq!(token.phase(), GenerationCancellationPhase::Terminal);
    }

    #[test]
    fn cancellation_wakes_an_interruptible_wait() {
        let guard = register_generation_cancellation("cancel-wait", "job").expect("register");
        let token = guard.token();
        let started = Arc::new(Barrier::new(2));
        let waiter_started = Arc::clone(&started);
        let waiter = thread::spawn(move || {
            waiter_started.wait();
            let start = Instant::now();
            let cancelled = token.wait_timeout(Duration::from_secs(5));
            (cancelled, start.elapsed())
        });
        started.wait();
        thread::sleep(Duration::from_millis(20));
        assert_eq!(
            request_generation_cancellation("cancel-wait", "job"),
            GenerationCancellationRequestOutcome::Requested
        );
        let (cancelled, elapsed) = waiter.join().expect("waiter joins");
        assert!(cancelled);
        assert!(elapsed < Duration::from_secs(1), "wait took {elapsed:?}");
    }

    #[test]
    fn timeout_without_cancellation_returns_false() {
        let guard = register_generation_cancellation("cancel-timeout", "job").expect("register");
        assert!(!guard.token().wait_timeout(Duration::from_millis(5)));
        assert_eq!(guard.phase(), GenerationCancellationPhase::Active);
    }

    #[test]
    fn guard_drop_unregisters_but_existing_token_remains_observable() {
        let guard = register_generation_cancellation("cancel-drop", "job").expect("register");
        let token = guard.token();
        drop(guard);
        assert!(!is_generation_job_active("cancel-drop", "job"));
        assert_eq!(
            request_generation_cancellation("cancel-drop", "job"),
            GenerationCancellationRequestOutcome::NotFound
        );
        assert_eq!(token.phase(), GenerationCancellationPhase::Active);
    }

    #[test]
    fn stale_guard_cannot_remove_a_newer_pointer_for_the_same_key() {
        let guard = register_generation_cancellation("cancel-pointer", "job").expect("register");
        let key = guard.key.clone();
        let replacement = Arc::new(GenerationCancellationState::new());
        generation_cancellations()
            .lock()
            .expect("registry mutex")
            .insert(key.clone(), Arc::clone(&replacement));

        drop(guard);
        let registered = generation_cancellations()
            .lock()
            .expect("registry mutex")
            .get(&key)
            .cloned()
            .expect("replacement remains registered");
        assert!(Arc::ptr_eq(&registered, &replacement));
        generation_cancellations()
            .lock()
            .expect("registry mutex")
            .remove(&key);
    }

    #[test]
    fn blank_keys_are_rejected() {
        for (project_id, job_id, field) in [("", "job", "projectId"), ("project", "  ", "jobId")] {
            assert_eq!(
                register_generation_cancellation(project_id, job_id)
                    .expect_err("blank key must fail"),
                GenerationCancellationRegistrationError::BlankKey { field }
            );
        }
        assert_eq!(
            request_generation_cancellation("", "job"),
            GenerationCancellationRequestOutcome::NotFound
        );
    }

    #[test]
    fn cancel_and_completion_race_has_exactly_one_winner() {
        for index in 0..128 {
            let project_id = format!("cancel-race-{index}");
            let guard = register_generation_cancellation(&project_id, "job").expect("register");
            let completion_token = guard.token();
            let barrier = Arc::new(Barrier::new(3));
            let completion_barrier = Arc::clone(&barrier);
            let completion = thread::spawn(move || {
                completion_barrier.wait();
                completion_token.begin_completion()
            });
            let cancellation_barrier = Arc::clone(&barrier);
            let cancellation_project_id = project_id.clone();
            let cancellation = thread::spawn(move || {
                cancellation_barrier.wait();
                request_generation_cancellation(&cancellation_project_id, "job")
            });
            barrier.wait();
            let completion_won = completion.join().expect("completion joins");
            let cancellation_outcome = cancellation.join().expect("cancellation joins");
            assert_eq!(
                completion_won,
                cancellation_outcome == GenerationCancellationRequestOutcome::TooLate,
                "race {index}: completion={completion_won}, cancellation={cancellation_outcome:?}"
            );
            if !completion_won {
                assert_eq!(
                    cancellation_outcome,
                    GenerationCancellationRequestOutcome::Requested
                );
                assert!(guard.token().is_cancelled());
            }
        }
    }
}
