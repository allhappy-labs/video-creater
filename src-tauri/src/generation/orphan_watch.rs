//! Stops an in-process generation whose job an out-of-process agent Undo removed.
//!
//! Agent Undo cancels the removed generations' runs through this process's cancellation
//! registry. The Codex MCP sidecar is a separate process, so its Undo can't reach the app's
//! registry: the app watches each running generation's job and cancels the run once the job is
//! gone from the project.

use super::cancel::request_generation_cancellation;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// A running watch. Dropping it stops the watch and waits for its thread.
pub struct GenerationJobWatch {
    stop: Arc<(Mutex<bool>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

/// Watches a running in-process generation's job. When an agent Undo from another process (the
/// MCP sidecar) removes the job, this process's cancellation registry is the only way to stop it.
///
/// Every `interval`, `job_recorded` reports whether the job is still in the project. `Ok(false)`
/// requests cancellation of the run once and ends the watch; read errors are ignored, since they
/// aren't proof that the job is gone.
pub fn watch_generation_job(
    project_id: &str,
    job_id: &str,
    interval: Duration,
    job_recorded: impl Fn() -> Result<bool, String> + Send + 'static,
) -> GenerationJobWatch {
    let stop = Arc::new((Mutex::new(false), Condvar::new()));
    let signal = Arc::clone(&stop);
    let project_id = project_id.to_string();
    let job_id = job_id.to_string();
    let thread = std::thread::spawn(move || {
        let (stopped, wake) = &*signal;
        loop {
            let guard = stopped
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let (guard, _) = wake
                .wait_timeout_while(guard, interval, |stopped| !*stopped)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if *guard {
                return;
            }
            drop(guard);
            if matches!(job_recorded(), Ok(false)) {
                let stopped = stopped
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !*stopped {
                    let _ = request_generation_cancellation(&project_id, &job_id);
                }
                return;
            }
        }
    });
    GenerationJobWatch {
        stop,
        thread: Some(thread),
    }
}

impl Drop for GenerationJobWatch {
    fn drop(&mut self) {
        let (stopped, wake) = &*self.stop;
        *stopped
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        wake.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::watch_generation_job;
    use crate::generation::cancel::register_generation_cancellation;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const INTERVAL: Duration = Duration::from_millis(2);

    #[test]
    fn a_removed_job_requests_cancellation_once() {
        let run = register_generation_cancellation("orphan-p", "orphan-j").expect("register run");
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        let _watch = watch_generation_job("orphan-p", "orphan-j", INTERVAL, move || {
            Ok(counted.fetch_add(1, Ordering::SeqCst) < 2)
        });

        let deadline = Instant::now() + Duration::from_secs(1);
        while !run.token().is_cancelled() && Instant::now() < deadline {
            std::thread::sleep(INTERVAL);
        }

        assert!(run.token().is_cancelled());
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "the watch ends after cancelling"
        );
    }

    #[test]
    fn a_read_error_never_cancels() {
        let run =
            register_generation_cancellation("orphan-p", "orphan-read-error").expect("register");
        let _watch = watch_generation_job("orphan-p", "orphan-read-error", INTERVAL, || {
            Err("project is being written".to_string())
        });

        std::thread::sleep(Duration::from_millis(50));

        assert!(!run.token().is_cancelled());
    }

    #[test]
    fn dropping_the_guard_stops_watching() {
        let run = register_generation_cancellation("orphan-p", "orphan-dropped").expect("register");
        let removed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&removed);
        let watch = watch_generation_job("orphan-p", "orphan-dropped", INTERVAL, move || {
            Ok(!flag.load(Ordering::SeqCst))
        });

        drop(watch);
        removed.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(50));

        assert!(!run.token().is_cancelled());
    }
}
