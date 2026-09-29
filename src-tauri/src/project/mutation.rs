use std::collections::HashMap;
use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex};
use std::thread::ThreadId;

/// The two boundaries one canonical project package enforces.
///
/// They are separate locks because they protect work of very different lengths. Taking
/// [`ProjectLeaseKind::Artifacts`] before [`ProjectLeaseKind::Mutation`] is the only allowed
/// order: a render holds the artifact lease for its whole run and takes the mutation lease for
/// each of its short writes, so a holder of the mutation lease must never wait for the artifact
/// lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ProjectLeaseKind {
    /// Writing the canonical project: its manifest and sidecars. Short by construction.
    Mutation,
    /// Producing the project's derived files: renders, precompose intermediates and caches.
    /// Held for a whole render, preparation or filmstrip pass.
    Artifacts,
}

impl ProjectLeaseKind {
    fn lock_suffix(self) -> &'static str {
        match self {
            Self::Mutation => "mutation",
            Self::Artifacts => "artifacts",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Mutation => "split-project mutation",
            Self::Artifacts => "split-project artifact",
        }
    }
}

#[derive(Default)]
struct ProjectMutationState {
    owners: HashMap<ProjectLeaseSlot, ProjectMutationOwner>,
}

struct ProjectMutationOwner {
    thread: ThreadId,
    depth: usize,
    lock_file: Option<File>,
}

static PROJECT_MUTATIONS: LazyLock<(Mutex<ProjectMutationState>, Condvar)> =
    LazyLock::new(|| (Mutex::new(ProjectMutationState::default()), Condvar::new()));

/// Cross-process, reentrant ownership of one boundary of one canonical project package.
struct ProjectLease {
    slot: ProjectLeaseSlot,
    owner: ThreadId,
}

type ProjectLeaseSlot = (ProjectLeaseKind, PathBuf);

/// Cross-process, reentrant ownership of one canonical project package's stored project.
///
/// Reentrancy lets a caller that already owns the project call nested canonical action helpers
/// that independently enforce the same boundary. A sibling OS lock remains stable while package
/// transactions rename the project directory. Different projects remain concurrent.
///
/// Holds are short. A render takes this lease for its start phase (a consistent read and the
/// job-running write), releases it while it encodes, and takes it again for each result write, so
/// an editor write never waits for an encode. What a render owns for its whole run is
/// [`SplitProjectArtifactLease`] instead.
pub struct SplitProjectMutationLease {
    _lease: ProjectLease,
}

/// Cross-process, reentrant ownership of one canonical project package's derived files.
///
/// Renders, preview preparation, filmstrip caching and interrupted-render recovery hold this for
/// their whole run: they share the project's `renders/`, precompose and cache trees, whose entries
/// are published by directory rename and cannot be produced twice at once. Holding it does not
/// delay a canonical project write, which needs [`SplitProjectMutationLease`] only.
///
/// Take this lease before the mutation lease, never after.
pub struct SplitProjectArtifactLease {
    _lease: ProjectLease,
}

pub fn acquire_split_project_mutation_lease(
    project_dir: &Path,
) -> Result<SplitProjectMutationLease, String> {
    acquire_project_lease(ProjectLeaseKind::Mutation, project_dir)
        .map(|_lease| SplitProjectMutationLease { _lease })
}

pub fn acquire_split_project_artifact_lease(
    project_dir: &Path,
) -> Result<SplitProjectArtifactLease, String> {
    acquire_project_lease(ProjectLeaseKind::Artifacts, project_dir)
        .map(|_lease| SplitProjectArtifactLease { _lease })
}

fn acquire_project_lease(
    kind: ProjectLeaseKind,
    project_dir: &Path,
) -> Result<ProjectLease, String> {
    let slot: ProjectLeaseSlot = (kind, project_mutation_key(project_dir)?);
    let owner = std::thread::current().id();
    let poisoned = || format!("{} coordinator lock is poisoned", kind.label());
    let (state, changed) = &*PROJECT_MUTATIONS;
    let mut state = state.lock().map_err(|_| poisoned())?;
    loop {
        match state.owners.get_mut(&slot) {
            Some(current) if current.thread == owner => {
                current.depth = current
                    .depth
                    .checked_add(1)
                    .ok_or_else(|| format!("{} lease depth is exhausted", kind.label()))?;
                return Ok(ProjectLease { slot, owner });
            }
            Some(_) => {
                state = changed.wait(state).map_err(|_| poisoned())?;
            }
            None => {
                state.owners.insert(
                    slot.clone(),
                    ProjectMutationOwner {
                        thread: owner,
                        depth: 1,
                        lock_file: None,
                    },
                );
                break;
            }
        }
    }
    drop(state);

    let lock_result = (|| {
        let lock_path = project_lease_lock_path(kind, &slot.1)?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        options
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .mode(0o600);
        let lock_file = options.open(&lock_path).map_err(|error| {
            format!(
                "could not open project {} lock at {}: {error}",
                kind.lock_suffix(),
                lock_path.display()
            )
        })?;
        lock_exclusive(&lock_file).map_err(|error| {
            format!(
                "could not lock project {} file at {}: {error}",
                kind.lock_suffix(),
                lock_path.display()
            )
        })?;
        Ok::<_, String>(lock_file)
    })();
    let lock_file = match lock_result {
        Ok(file) => file,
        Err(error) => {
            let (state, changed) = &*PROJECT_MUTATIONS;
            if let Ok(mut state) = state.lock() {
                state.owners.remove(&slot);
                changed.notify_all();
            }
            return Err(error);
        }
    };

    let mut state = PROJECT_MUTATIONS.0.lock().map_err(|_| poisoned())?;
    let current = state
        .owners
        .get_mut(&slot)
        .ok_or_else(|| format!("{} reservation disappeared", kind.label()))?;
    current.lock_file = Some(lock_file);
    drop(state);
    Ok(ProjectLease { slot, owner })
}

fn project_lease_lock_path(kind: ProjectLeaseKind, project_key: &Path) -> Result<PathBuf, String> {
    let parent = project_key
        .parent()
        .ok_or_else(|| "split-project directory has no lock parent".to_string())?;
    let name = project_key
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "split-project directory has no UTF-8 lock name".to_string())?;
    Ok(parent.join(format!(".{name}.{}.lock", kind.lock_suffix())))
}

#[cfg(unix)]
fn lock_exclusive(file: &File) -> std::io::Result<()> {
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn unlock(file: &File) {
    let _ = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &File) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "cross-process project locks require Unix flock",
    ))
}

#[cfg(not(unix))]
fn unlock(_file: &File) {}

/// The canonical key that identifies one project package for mutation ordering.
pub(crate) fn project_mutation_key(project_dir: &Path) -> Result<PathBuf, String> {
    if let Ok(canonical) = project_dir.canonicalize() {
        return Ok(canonical);
    }
    let parent = project_dir
        .parent()
        .ok_or_else(|| "split-project directory has no parent".to_string())?;
    let canonical_parent = parent.canonicalize().map_err(|error| {
        format!(
            "split-project mutation parent could not be canonicalized at {}: {error}",
            parent.display()
        )
    })?;
    let name = project_dir
        .file_name()
        .ok_or_else(|| "split-project directory has no final path component".to_string())?;
    Ok(canonical_parent.join(name))
}

impl Drop for ProjectLease {
    fn drop(&mut self) {
        let (state, changed) = &*PROJECT_MUTATIONS;
        let Ok(mut state) = state.lock() else {
            return;
        };
        let Some(current) = state.owners.get_mut(&self.slot) else {
            return;
        };
        if current.thread != self.owner {
            return;
        }
        if current.depth > 1 {
            current.depth -= 1;
        } else {
            if let Some(current) = state.owners.remove(&self.slot) {
                if let Some(file) = current.lock_file {
                    unlock(&file);
                }
            }
            changed.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    #[ignore = "child-process half of project_mutation_lease_blocks_a_separate_process, which spawns it by name with --ignored"]
    fn cross_process_lock_helper() {
        let Ok(project_dir) = std::env::var("VIDEO_CREATER_TEST_LOCK_PROJECT") else {
            return;
        };
        let signal = std::env::var("VIDEO_CREATER_TEST_LOCK_SIGNAL").expect("signal path");
        let _lease =
            acquire_split_project_mutation_lease(Path::new(&project_dir)).expect("child lease");
        std::fs::write(signal, b"acquired").expect("write acquired signal");
    }

    #[test]
    fn project_mutation_lease_blocks_a_separate_process() {
        let temp = tempfile::tempdir().expect("temp project parent");
        let project_dir = temp.path().join("project");
        std::fs::create_dir(&project_dir).expect("project directory");
        let signal = temp.path().join("child-acquired");
        let lease = acquire_split_project_mutation_lease(&project_dir).expect("parent lease");
        let nested = acquire_split_project_mutation_lease(&project_dir).expect("reentrant lease");
        let parked = temp.path().join("project-backup");
        std::fs::rename(&project_dir, &parked).expect("simulate package transaction rename");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "project::mutation::tests::cross_process_lock_helper",
                "--ignored",
            ])
            .env("VIDEO_CREATER_TEST_LOCK_PROJECT", &project_dir)
            .env("VIDEO_CREATER_TEST_LOCK_SIGNAL", &signal)
            .spawn()
            .expect("spawn lock contender");
        std::thread::sleep(Duration::from_millis(100));
        assert!(!signal.exists(), "child must wait on the OS-backed lock");
        drop(nested);
        std::fs::rename(&parked, &project_dir).expect("restore canonical project path");
        drop(lease);
        assert!(child.wait().expect("wait child").success());
        assert_eq!(std::fs::read(signal).unwrap(), b"acquired");
    }

    /// The two boundaries are separate locks: a render can own the artifact lease for its whole
    /// run while an editor write takes and releases the mutation lease.
    #[test]
    fn the_artifact_lease_does_not_block_a_project_write() {
        let temp = tempfile::tempdir().expect("temp project parent");
        let project_dir = temp.path().join("project");
        std::fs::create_dir(&project_dir).expect("project directory");
        let artifacts = acquire_split_project_artifact_lease(&project_dir).expect("artifact lease");

        let write_dir = project_dir.clone();
        let writer =
            std::thread::spawn(move || acquire_split_project_mutation_lease(&write_dir).map(drop));
        assert_eq!(writer.join().expect("writer joins"), Ok(()));

        // And it does exclude a second artifact owner, which is what serializes two renders.
        let second_dir = project_dir.clone();
        let (blocked_tx, blocked_rx) = std::sync::mpsc::channel();
        let second = std::thread::spawn(move || {
            let lease = acquire_split_project_artifact_lease(&second_dir);
            blocked_tx.send(()).expect("signal acquired");
            lease.map(drop)
        });
        assert!(blocked_rx.recv_timeout(Duration::from_millis(200)).is_err());
        drop(artifacts);
        assert_eq!(second.join().expect("second joins"), Ok(()));
    }
}
