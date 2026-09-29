//! Per-project FIFO executor for Tauri commands that write a canonical project.
//!
//! A render holds the process-wide storage lease and the project's artifact lease for its whole
//! duration, and takes the project mutation lease for its start phase and again for each of its
//! result writes. A synchronous command that writes the project would therefore block the main
//! thread until the lease it needs is free. Commands instead submit their work here and await the
//! result on the async runtime, so the waiting happens on a worker thread.
//!
//! # Ordering
//!
//! Work for one canonical project directory runs on at most one worker thread at a time, in the
//! order `submit` was called. `submit` enqueues synchronously before it returns the future, so the
//! enqueue order is fixed even though the caller awaits later. The editor's own write queue
//! (`enqueue` in `src/editor/store/project-slice.ts`) keeps its submissions sequential, which makes
//! the order `submit` is called the order the user made the edits.
//!
//! Work keeps taking the storage and project leases inside its closure. A write queued while a
//! render holds a lease waits for it on the worker thread, and later writes wait behind it in
//! order. A canonical project action needs only the project mutation lease, which a render holds
//! for its short writes alone, so such a write lands while the render encodes.
//! Different projects use different workers and never block each other here.
//!
//! A panic in one closure fails that command only. A nested submit for the same project from its
//! own worker runs inline, so it cannot deadlock waiting for itself.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};

use crate::project::mutation::project_mutation_key;

type QueuedWork = Box<dyn FnOnce() + Send + 'static>;
type Queues = Arc<Mutex<HashMap<PathBuf, VecDeque<QueuedWork>>>>;

thread_local! {
    /// The project key whose queue the current thread is draining, if any.
    static CURRENT_PROJECT_KEY: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// FIFO queues keyed by canonical project directory. A key is present while its worker runs.
pub struct ProjectCommandQueue {
    queues: Queues,
}

static PROJECT_COMMAND_QUEUE: OnceLock<ProjectCommandQueue> = OnceLock::new();

/// The process-wide project command queue.
pub fn project_command_queue() -> &'static ProjectCommandQueue {
    PROJECT_COMMAND_QUEUE.get_or_init(ProjectCommandQueue::new)
}

impl ProjectCommandQueue {
    fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Enqueues `work` behind earlier commands for the same project and returns its result slot.
    pub fn submit<T: Send + 'static>(
        &self,
        project_dir: &Path,
        label: &'static str,
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<QueuedProjectCommand<T>, String> {
        let key = project_mutation_key(project_dir)?;
        let slot = Arc::new(ResultSlot::default());
        let completer = SlotCompleter {
            slot: Some(Arc::clone(&slot)),
            label,
        };

        let nested = CURRENT_PROJECT_KEY.with(|current| current.borrow().as_ref() == Some(&key));
        if nested {
            completer.complete(run_guarded(label, work));
            return Ok(QueuedProjectCommand { slot });
        }

        let job: QueuedWork = Box::new(move || completer.complete(run_guarded(label, work)));
        let mut queues = self
            .queues
            .lock()
            .map_err(|_| "project command queue lock is poisoned".to_string())?;
        if let Some(pending) = queues.get_mut(&key) {
            pending.push_back(job);
            return Ok(QueuedProjectCommand { slot });
        }
        queues.insert(key.clone(), VecDeque::from([job]));
        drop(queues);

        let worker_queues = Arc::clone(&self.queues);
        let worker_key = key.clone();
        let spawned = std::thread::Builder::new()
            .name("project-commands".to_string())
            .spawn(move || drain_project_queue(worker_queues, worker_key));
        if let Err(error) = spawned {
            // Dropping the queued work completes every waiter with an error.
            if let Ok(mut queues) = self.queues.lock() {
                queues.remove(&key);
            }
            return Err(format!(
                "could not start the project command worker: {error}"
            ));
        }
        Ok(QueuedProjectCommand { slot })
    }

    #[cfg(test)]
    fn active_project_keys_for_test(&self) -> Vec<PathBuf> {
        self.queues
            .lock()
            .expect("queue lock")
            .keys()
            .cloned()
            .collect()
    }
}

fn drain_project_queue(queues: Queues, key: PathBuf) {
    CURRENT_PROJECT_KEY.with(|current| *current.borrow_mut() = Some(key.clone()));
    loop {
        let next = {
            let Ok(mut queues) = queues.lock() else {
                return;
            };
            let next = queues.get_mut(&key).and_then(VecDeque::pop_front);
            if next.is_none() {
                queues.remove(&key);
            }
            next
        };
        match next {
            Some(job) => job(),
            None => break,
        }
    }
    CURRENT_PROJECT_KEY.with(|current| *current.borrow_mut() = None);
}

fn run_guarded<T>(
    label: &'static str,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| Err(format!("{label} task panicked")))
}

struct SlotState<T> {
    result: Option<Result<T, String>>,
    waker: Option<Waker>,
}

struct ResultSlot<T> {
    state: Mutex<SlotState<T>>,
    ready: Condvar,
}

impl<T> Default for ResultSlot<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(SlotState {
                result: None,
                waker: None,
            }),
            ready: Condvar::new(),
        }
    }
}

/// Delivers a command's result once. Dropped unrun work completes with an error.
struct SlotCompleter<T> {
    slot: Option<Arc<ResultSlot<T>>>,
    label: &'static str,
}

impl<T> SlotCompleter<T> {
    fn complete(mut self, result: Result<T, String>) {
        if let Some(slot) = self.slot.take() {
            deliver(&slot, result);
        }
    }
}

impl<T> Drop for SlotCompleter<T> {
    fn drop(&mut self) {
        if let Some(slot) = self.slot.take() {
            deliver(
                &slot,
                Err(format!("{} was dropped before it ran", self.label)),
            );
        }
    }
}

fn deliver<T>(slot: &ResultSlot<T>, result: Result<T, String>) {
    let waker = {
        let mut state = match slot.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.result = Some(result);
        state.waker.take()
    };
    slot.ready.notify_all();
    if let Some(waker) = waker {
        waker.wake();
    }
}

/// The pending result of a queued project command. Await it, or call [`Self::wait`].
pub struct QueuedProjectCommand<T> {
    slot: Arc<ResultSlot<T>>,
}

impl<T> QueuedProjectCommand<T> {
    /// Blocks the current thread until the command finishes.
    pub fn wait(self) -> Result<T, String> {
        let mut state = match self.slot.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        loop {
            if let Some(result) = state.result.take() {
                return result;
            }
            state = match self.slot.ready.wait(state) {
                Ok(state) => state,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

impl<T> Future for QueuedProjectCommand<T> {
    type Output = Result<T, String>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = match self.slot.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        match state.result.take() {
            Some(result) => Poll::Ready(result),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::mutation::acquire_split_project_mutation_lease;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::Barrier;
    use std::task::Wake;
    use std::thread::Thread;
    use std::time::{Duration, Instant};

    struct ThreadWaker {
        thread: Thread,
        wakes: AtomicUsize,
    }

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.wakes.fetch_add(1, Ordering::SeqCst);
            self.thread.unpark();
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(ThreadWaker {
            thread: std::thread::current(),
            wakes: AtomicUsize::new(0),
        }));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
            std::thread::park_timeout(Duration::from_millis(50));
        }
    }

    fn poll_once<T>(command: &mut QueuedProjectCommand<T>) -> Poll<Result<T, String>> {
        let waker = Waker::from(Arc::new(ThreadWaker {
            thread: std::thread::current(),
            wakes: AtomicUsize::new(0),
        }));
        Pin::new(command).poll(&mut Context::from_waker(&waker))
    }

    fn project_dir(temp: &tempfile::TempDir, name: &str) -> PathBuf {
        let dir = temp.path().join(name);
        std::fs::create_dir_all(&dir).expect("project dir");
        dir
    }

    /// Holds the project lease on a helper thread until the returned sender is used.
    fn hold_project_lease(dir: &Path) -> (mpsc::Sender<()>, std::thread::JoinHandle<()>) {
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let dir = dir.to_path_buf();
        let holder = std::thread::spawn(move || {
            let lease = acquire_split_project_mutation_lease(&dir).expect("render lease");
            acquired_tx.send(()).expect("signal acquired");
            let _ = release_rx.recv();
            drop(lease);
        });
        acquired_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("lease acquired");
        (release_tx, holder)
    }

    #[test]
    fn commands_for_one_project_complete_in_submission_order() {
        let temp = tempfile::tempdir().expect("temp");
        let dir = project_dir(&temp, "project");
        let queue = Arc::new(ProjectCommandQueue::new());
        let barrier = Arc::new(Barrier::new(5));
        let submit_order = Arc::new(Mutex::new(Vec::new()));
        let run_order = Arc::new(Mutex::new(Vec::new()));

        let submitters: Vec<_> = (0..5)
            .map(|index| {
                let (queue, barrier, dir) = (Arc::clone(&queue), Arc::clone(&barrier), dir.clone());
                let (submit_order, run_order) = (Arc::clone(&submit_order), Arc::clone(&run_order));
                std::thread::spawn(move || {
                    barrier.wait();
                    let command = {
                        let mut submitted = submit_order.lock().expect("submit order");
                        let command = queue
                            .submit(&dir, "ordered write", move || {
                                std::thread::sleep(Duration::from_millis(10));
                                run_order.lock().expect("run order").push(index);
                                Ok(index)
                            })
                            .expect("submit");
                        submitted.push(index);
                        command
                    };
                    command.wait().expect("command result")
                })
            })
            .collect();
        for submitter in submitters {
            submitter.join().expect("submitter");
        }

        assert_eq!(
            *run_order.lock().expect("run order"),
            *submit_order.lock().expect("submit order")
        );
    }

    #[test]
    fn submit_returns_while_a_render_holds_the_project_lease() {
        let temp = tempfile::tempdir().expect("temp");
        let dir = project_dir(&temp, "project");
        let queue = ProjectCommandQueue::new();
        let (release, holder) = hold_project_lease(&dir);
        let writes = Arc::new(AtomicUsize::new(0));

        let started = Instant::now();
        let write_dir = dir.clone();
        let recorded = Arc::clone(&writes);
        let mut command = queue
            .submit(&dir, "write behind render", move || {
                let _lease = acquire_split_project_mutation_lease(&write_dir)?;
                recorded.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .expect("submit");
        assert!(started.elapsed() < Duration::from_millis(50));

        std::thread::sleep(Duration::from_millis(200));
        assert!(poll_once(&mut command).is_pending());
        assert_eq!(writes.load(Ordering::SeqCst), 0);

        release.send(()).expect("release lease");
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || done_tx.send(command.wait()));
        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("write completes after the render")
            .expect("write succeeds");
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        holder.join().expect("lease holder");
    }

    #[test]
    fn different_projects_run_concurrently() {
        let temp = tempfile::tempdir().expect("temp");
        let blocked_dir = project_dir(&temp, "project-a");
        let free_dir = project_dir(&temp, "project-b");
        let queue = ProjectCommandQueue::new();
        let (release, holder) = hold_project_lease(&blocked_dir);

        let lease_dir = blocked_dir.clone();
        let blocked = queue
            .submit(&blocked_dir, "blocked write", move || {
                acquire_split_project_mutation_lease(&lease_dir).map(drop)
            })
            .expect("submit A");
        let free = queue
            .submit(&free_dir, "free write", || Ok("B"))
            .expect("submit B");

        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || done_tx.send(free.wait()));
        assert_eq!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("project B is not blocked by project A"),
            Ok("B")
        );

        release.send(()).expect("release lease");
        blocked.wait().expect("project A completes");
        holder.join().expect("lease holder");
    }

    #[test]
    fn a_panicking_command_fails_alone() {
        let temp = tempfile::tempdir().expect("temp");
        let dir = project_dir(&temp, "project");
        let queue = ProjectCommandQueue::new();

        let panicked = queue
            .submit(&dir, "exploding write", || -> Result<(), String> {
                panic!("boom")
            })
            .expect("submit");
        let next = queue.submit(&dir, "next write", || Ok(7)).expect("submit");

        assert_eq!(
            panicked.wait(),
            Err("exploding write task panicked".to_string())
        );
        assert_eq!(next.wait(), Ok(7));
    }

    #[test]
    fn nested_submit_for_the_same_project_runs_inline() {
        let temp = tempfile::tempdir().expect("temp");
        let dir = project_dir(&temp, "project");
        let queue = Arc::new(ProjectCommandQueue::new());

        let inner_queue = Arc::clone(&queue);
        let inner_dir = dir.clone();
        let command = queue
            .submit(&dir, "outer write", move || {
                inner_queue
                    .submit(&inner_dir, "inner write", || Ok(41))?
                    .wait()
                    .map(|value| value + 1)
            })
            .expect("submit");

        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || done_tx.send(command.wait()));
        assert_eq!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("nested submit does not deadlock"),
            Ok(42)
        );
    }

    #[test]
    fn awaiting_a_command_on_an_executor_completes() {
        let temp = tempfile::tempdir().expect("temp");
        let dir = project_dir(&temp, "project");
        let queue = ProjectCommandQueue::new();

        let command = queue
            .submit(&dir, "awaited write", || {
                std::thread::sleep(Duration::from_millis(20));
                Ok("done")
            })
            .expect("submit");

        assert_eq!(block_on(command), Ok("done"));
    }

    #[test]
    fn worker_exits_when_its_queue_drains() {
        let temp = tempfile::tempdir().expect("temp");
        let dir = project_dir(&temp, "project");
        let queue = ProjectCommandQueue::new();

        queue
            .submit(&dir, "single write", || Ok(()))
            .expect("submit")
            .wait()
            .expect("result");

        let deadline = Instant::now() + Duration::from_secs(1);
        while !queue.active_project_keys_for_test().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(queue.active_project_keys_for_test().is_empty());
    }
}
