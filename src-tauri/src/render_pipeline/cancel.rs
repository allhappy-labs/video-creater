use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;
use thiserror::Error;

use crate::project::job_progress::JobProgressReporter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RenderAttemptPhase {
    Active = 0,
    CancelRequested = 1,
    Completing = 2,
    Terminal = 3,
}

impl RenderAttemptPhase {
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Active,
            1 => Self::CancelRequested,
            2 => Self::Completing,
            3 => Self::Terminal,
            _ => unreachable!("render attempt phase is written only by this module"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCancellationOutcome {
    Requested,
    AlreadyRequested,
    TooLate,
    NotFound,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderAttemptRegistrationError {
    #[error("render attempt {field} cannot be blank")]
    BlankKey { field: &'static str },
    #[error("render attempt is already active: {project_id}/{job_id}/{attempt_id}")]
    DuplicateRegistration {
        project_id: String,
        job_id: String,
        attempt_id: String,
    },
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RenderAttemptKey {
    project_root: PathBuf,
    project_id: String,
    job_id: String,
    attempt_id: String,
}

impl RenderAttemptKey {
    pub fn new(
        project_root: PathBuf,
        project_id: &str,
        job_id: &str,
        attempt_id: &str,
    ) -> Result<Self, RenderAttemptRegistrationError> {
        if project_root.as_os_str().is_empty() {
            return Err(RenderAttemptRegistrationError::BlankKey {
                field: "projectRoot",
            });
        }
        let project_id = nonblank(project_id, "projectId")?;
        let job_id = nonblank(job_id, "jobId")?;
        let attempt_id = nonblank(attempt_id, "attemptId")?;
        Ok(Self {
            project_root,
            project_id,
            job_id,
            attempt_id,
        })
    }

    pub fn project_root(&self) -> &std::path::Path {
        &self.project_root
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }
}

fn nonblank(value: &str, field: &'static str) -> Result<String, RenderAttemptRegistrationError> {
    let value = value.trim();
    if value.is_empty() {
        Err(RenderAttemptRegistrationError::BlankKey { field })
    } else {
        Ok(value.to_string())
    }
}

#[derive(Debug)]
struct RenderCancellationState {
    phase: AtomicU8,
    cancellation_requested: AtomicBool,
    wait_lock: Mutex<()>,
    wait_condition: Condvar,
    /// Lease-free progress snapshot for the attempt's job; a no-op outside a split project.
    progress: JobProgressReporter,
}

impl RenderCancellationState {
    fn new(key: &RenderAttemptKey) -> Self {
        Self {
            phase: AtomicU8::new(RenderAttemptPhase::Active as u8),
            cancellation_requested: AtomicBool::new(false),
            wait_lock: Mutex::new(()),
            wait_condition: Condvar::new(),
            progress: JobProgressReporter::new(key.project_root(), key.job_id()),
        }
    }

    fn phase(&self) -> RenderAttemptPhase {
        RenderAttemptPhase::from_u8(self.phase.load(Ordering::Acquire))
    }

    fn request_cancellation(&self) -> RenderCancellationOutcome {
        loop {
            match self.phase() {
                RenderAttemptPhase::Active => {
                    if self
                        .phase
                        .compare_exchange(
                            RenderAttemptPhase::Active as u8,
                            RenderAttemptPhase::CancelRequested as u8,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        self.cancellation_requested.store(true, Ordering::Release);
                        self.wait_condition.notify_all();
                        return RenderCancellationOutcome::Requested;
                    }
                }
                RenderAttemptPhase::CancelRequested => {
                    return RenderCancellationOutcome::AlreadyRequested;
                }
                RenderAttemptPhase::Completing | RenderAttemptPhase::Terminal => {
                    return RenderCancellationOutcome::TooLate;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderCancellationToken {
    state: Arc<RenderCancellationState>,
}

impl RenderCancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.state.cancellation_requested.load(Ordering::Acquire)
            || self.phase() == RenderAttemptPhase::CancelRequested
    }

    pub fn phase(&self) -> RenderAttemptPhase {
        self.state.phase()
    }

    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        if self.is_cancelled() || timeout.is_zero() {
            return self.is_cancelled();
        }
        let guard = self.state.wait_lock.lock().expect("render wait mutex");
        let _ = self
            .state
            .wait_condition
            .wait_timeout_while(guard, timeout, |_| !self.is_cancelled())
            .expect("render wait mutex");
        self.is_cancelled()
    }

    pub fn begin_completion(&self) -> bool {
        self.state
            .phase
            .compare_exchange(
                RenderAttemptPhase::Active as u8,
                RenderAttemptPhase::Completing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub fn begin_cancelled_completion(&self) -> bool {
        self.state
            .phase
            .compare_exchange(
                RenderAttemptPhase::CancelRequested as u8,
                RenderAttemptPhase::Completing as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// Reports render progress (0..=1), throttled and never decreasing.
    pub fn report_progress(&self, fraction: f64) {
        self.state.progress.report(fraction);
    }

    pub fn mark_terminal(&self) {
        self.state
            .phase
            .store(RenderAttemptPhase::Terminal as u8, Ordering::Release);
        self.state.wait_condition.notify_all();
    }
}

impl crate::process_supervisor::CancellationSignal for RenderCancellationToken {
    fn is_cancelled(&self) -> bool {
        Self::is_cancelled(self)
    }

    fn wait_timeout(&self, duration: Duration) -> bool {
        Self::wait_timeout(self, duration)
    }
}

#[derive(Debug)]
pub struct RenderAttemptGuard {
    key: RenderAttemptKey,
    state: Arc<RenderCancellationState>,
}

impl RenderAttemptGuard {
    pub fn token(&self) -> RenderCancellationToken {
        RenderCancellationToken {
            state: Arc::clone(&self.state),
        }
    }
}

impl Drop for RenderAttemptGuard {
    fn drop(&mut self) {
        unregister_render_attempt(&self.key, &self.state);
    }
}

pub fn register_render_attempt(
    key: RenderAttemptKey,
) -> Result<RenderAttemptGuard, RenderAttemptRegistrationError> {
    let state = Arc::new(RenderCancellationState::new(&key));
    let mut attempts = render_attempts().lock().expect("render registry mutex");
    if attempts.contains_key(&key) {
        return Err(RenderAttemptRegistrationError::DuplicateRegistration {
            project_id: key.project_id.clone(),
            job_id: key.job_id.clone(),
            attempt_id: key.attempt_id.clone(),
        });
    }
    attempts.insert(key.clone(), Arc::clone(&state));
    Ok(RenderAttemptGuard { key, state })
}

pub fn request_render_cancellation(key: &RenderAttemptKey) -> RenderCancellationOutcome {
    let state = render_attempts()
        .lock()
        .expect("render registry mutex")
        .get(key)
        .cloned();
    state
        .map(|state| state.request_cancellation())
        .unwrap_or(RenderCancellationOutcome::NotFound)
}

pub fn request_render_cancellation_by_locator(
    project_root: &std::path::Path,
    job_id: &str,
    attempt_id: &str,
) -> RenderCancellationOutcome {
    let state = render_attempts()
        .lock()
        .expect("render registry mutex")
        .iter()
        .find(|(key, _)| {
            key.project_root() == project_root
                && key.job_id() == job_id
                && key.attempt_id() == attempt_id
        })
        .map(|(_, state)| Arc::clone(state));
    state
        .map(|state| state.request_cancellation())
        .unwrap_or(RenderCancellationOutcome::NotFound)
}

pub fn is_render_attempt_active(key: &RenderAttemptKey) -> bool {
    render_attempts()
        .lock()
        .expect("render registry mutex")
        .contains_key(key)
}

fn unregister_render_attempt(key: &RenderAttemptKey, state: &Arc<RenderCancellationState>) {
    state.progress.clear();
    let mut attempts = render_attempts().lock().expect("render registry mutex");
    if attempts
        .get(key)
        .is_some_and(|registered| Arc::ptr_eq(registered, state))
    {
        attempts.remove(key);
    }
}

fn render_attempts() -> &'static Mutex<HashMap<RenderAttemptKey, Arc<RenderCancellationState>>> {
    static RENDER_ATTEMPTS: OnceLock<
        Mutex<HashMap<RenderAttemptKey, Arc<RenderCancellationState>>>,
    > = OnceLock::new();
    RENDER_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant};

    fn key(root: &str, project: &str, job: &str, attempt: &str) -> RenderAttemptKey {
        RenderAttemptKey::new(PathBuf::from(root), project, job, attempt).expect("valid key")
    }

    #[test]
    fn render_cancellation_is_scoped_by_package_project_job_and_attempt() {
        let first_key = key("/tmp/render-a", "project", "job", "attempt-a");
        let second_key = key("/tmp/render-b", "project", "job", "attempt-b");
        let first = register_render_attempt(first_key.clone()).expect("register first");
        let second = register_render_attempt(second_key.clone()).expect("register second");

        assert_eq!(
            request_render_cancellation(&first_key),
            RenderCancellationOutcome::Requested
        );
        assert!(first.token().is_cancelled());
        assert!(!second.token().is_cancelled());
        assert!(is_render_attempt_active(&second_key));
    }

    #[test]
    fn locator_cancellation_finds_only_the_exact_root_job_and_attempt() {
        let first_key = key("/tmp/render-locator-a", "project-a", "job", "attempt");
        let second_key = key("/tmp/render-locator-b", "project-b", "job", "attempt");
        let first = register_render_attempt(first_key).expect("register first");
        let second = register_render_attempt(second_key).expect("register second");

        assert_eq!(
            request_render_cancellation_by_locator(
                std::path::Path::new("/tmp/render-locator-a"),
                "job",
                "attempt",
            ),
            RenderCancellationOutcome::Requested
        );
        assert!(first.token().is_cancelled());
        assert!(!second.token().is_cancelled());
    }

    #[test]
    fn duplicate_exact_registration_does_not_replace_active_attempt() {
        let attempt_key = key("/tmp/render-duplicate", "project", "job", "attempt");
        let guard = register_render_attempt(attempt_key.clone()).expect("register");
        assert!(matches!(
            register_render_attempt(attempt_key.clone()),
            Err(RenderAttemptRegistrationError::DuplicateRegistration { .. })
        ));
        assert_eq!(
            request_render_cancellation(&attempt_key),
            RenderCancellationOutcome::Requested
        );
        assert!(guard.token().is_cancelled());
    }

    #[test]
    fn completion_claim_blocks_late_cancellation() {
        let attempt_key = key("/tmp/render-complete", "project", "job", "attempt");
        let guard = register_render_attempt(attempt_key.clone()).expect("register");
        let token = guard.token();
        assert!(token.begin_completion());
        assert!(!token.begin_completion());
        assert_eq!(
            request_render_cancellation(&attempt_key),
            RenderCancellationOutcome::TooLate
        );
        token.mark_terminal();
        assert_eq!(token.phase(), RenderAttemptPhase::Terminal);
    }

    #[test]
    fn failure_and_cancellation_claims_choose_one_terminal_owner() {
        let failure_key = key("/tmp/render-failure", "project", "job", "failure");
        let failure = register_render_attempt(failure_key.clone()).expect("register failure");
        assert!(failure.token().begin_completion());
        assert_eq!(
            request_render_cancellation(&failure_key),
            RenderCancellationOutcome::TooLate
        );

        let cancelled_key = key("/tmp/render-cancelled", "project", "job", "cancelled");
        let cancelled = register_render_attempt(cancelled_key.clone()).expect("register cancelled");
        assert_eq!(
            request_render_cancellation(&cancelled_key),
            RenderCancellationOutcome::Requested
        );
        assert!(cancelled.token().begin_cancelled_completion());
        assert!(!cancelled.token().begin_completion());
    }

    #[test]
    fn cancellation_wakes_interruptible_wait() {
        let attempt_key = key("/tmp/render-wait", "project", "job", "attempt");
        let guard = register_render_attempt(attempt_key.clone()).expect("register");
        let token = guard.token();
        let barrier = Arc::new(Barrier::new(2));
        let waiter_barrier = Arc::clone(&barrier);
        let waiter = thread::spawn(move || {
            waiter_barrier.wait();
            let started = Instant::now();
            (
                token.wait_timeout(Duration::from_secs(5)),
                started.elapsed(),
            )
        });
        barrier.wait();
        assert_eq!(
            request_render_cancellation(&attempt_key),
            RenderCancellationOutcome::Requested
        );
        let (cancelled, elapsed) = waiter.join().expect("join waiter");
        assert!(cancelled);
        assert!(elapsed < Duration::from_secs(1), "wait took {elapsed:?}");
    }

    #[test]
    fn cancellation_and_completion_race_has_one_winner() {
        for index in 0..64 {
            let attempt_key = key(
                &format!("/tmp/render-race-{index}"),
                "project",
                "job",
                "attempt",
            );
            let guard = register_render_attempt(attempt_key.clone()).expect("register");
            let token = guard.token();
            let barrier = Arc::new(Barrier::new(3));
            let completion_barrier = Arc::clone(&barrier);
            let completion = thread::spawn(move || {
                completion_barrier.wait();
                token.begin_completion()
            });
            let cancellation_barrier = Arc::clone(&barrier);
            let cancellation_key = attempt_key.clone();
            let cancellation = thread::spawn(move || {
                cancellation_barrier.wait();
                request_render_cancellation(&cancellation_key)
            });
            barrier.wait();
            let completed = completion.join().expect("completion joins");
            let cancelled = cancellation.join().expect("cancellation joins");
            assert_eq!(completed, cancelled == RenderCancellationOutcome::TooLate);
        }
    }

    #[test]
    fn blank_identity_fields_are_rejected() {
        for (project, job, attempt, field) in [
            ("", "job", "attempt", "projectId"),
            ("project", " ", "attempt", "jobId"),
            ("project", "job", "", "attemptId"),
        ] {
            assert_eq!(
                RenderAttemptKey::new(PathBuf::from("/tmp/render-blank"), project, job, attempt)
                    .expect_err("blank field"),
                RenderAttemptRegistrationError::BlankKey { field }
            );
        }
    }

    #[test]
    fn render_attempt_progress_is_written_for_a_split_project_and_cleared_on_drop() {
        let temp = tempfile::tempdir().expect("temp project");
        std::fs::write(
            temp.path().join(crate::project::storage::PROJECT_FILE_NAME),
            b"{}",
        )
        .expect("manifest");
        let attempt_key = RenderAttemptKey::new(
            temp.path().to_path_buf(),
            "project",
            "job-progress",
            "attempt",
        )
        .expect("key");
        let guard = register_render_attempt(attempt_key).expect("register");

        guard.token().report_progress(0.5);
        let snapshots = crate::project::job_progress::read_job_progress_snapshots(temp.path())
            .expect("read progress");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].job_id, "job-progress");
        assert_eq!(snapshots[0].progress, 0.5);

        drop(guard);
        assert!(
            crate::project::job_progress::read_job_progress_snapshots(temp.path())
                .expect("read progress")
                .is_empty()
        );
    }

    #[test]
    fn render_attempt_progress_is_a_no_op_for_a_non_project_root() {
        let temp = tempfile::tempdir().expect("temp root");
        let attempt_key =
            RenderAttemptKey::new(temp.path().to_path_buf(), "project", "job", "attempt")
                .expect("key");
        let guard = register_render_attempt(attempt_key).expect("register");

        guard.token().report_progress(0.5);

        assert!(!temp.path().join("logs").exists());
    }

    #[test]
    fn guard_drop_unregisters_exact_attempt() {
        let attempt_key = key("/tmp/render-drop", "project", "job", "attempt");
        let guard = register_render_attempt(attempt_key.clone()).expect("register");
        assert!(is_render_attempt_active(&attempt_key));
        drop(guard);
        assert!(!is_render_attempt_active(&attempt_key));
        assert_eq!(
            request_render_cancellation(&attempt_key),
            RenderCancellationOutcome::NotFound
        );
    }
}
