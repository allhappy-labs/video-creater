//! Lease-free progress snapshots for running background jobs.
//!
//! A render holds the storage lease for its whole duration, so progress cannot travel through
//! canonical project actions: the editor's project reload takes that lease and returns only after
//! the render ends, and every action advances `contentRevision`. Progress is instead written as a
//! small JSON snapshot per job under `logs/job-progress/`, atomically and at most every
//! [`JOB_PROGRESS_MIN_INTERVAL`]. Reads take no lease. Snapshots are bookkeeping: they never enter
//! `job.json` or undo history.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::project::model::{JobStatus, VideoProject};
use crate::project::split::validate_split_project_write_path;
use crate::project::storage::PROJECT_FILE_NAME;

/// Project-relative directory that holds one `<jobId>.json` snapshot per running job.
pub const JOB_PROGRESS_DIR: &str = "logs/job-progress";
/// Minimum time between two written snapshots of one job, except the final 1.0.
pub const JOB_PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(500);

const MAX_SNAPSHOT_BYTES: u64 = 4 * 1024;
const MAX_SNAPSHOTS: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressSnapshot {
    pub job_id: String,
    pub progress: f64,
    pub updated_at: String,
}

/// Writes throttled, never-decreasing progress for one job of one split project.
#[derive(Debug)]
pub struct JobProgressReporter {
    project_dir: PathBuf,
    job_id: String,
    last: Mutex<Option<(Instant, f64)>>,
}

impl JobProgressReporter {
    pub fn new(project_dir: impl Into<PathBuf>, job_id: &str) -> Self {
        Self {
            project_dir: project_dir.into(),
            job_id: job_id.to_string(),
            last: Mutex::new(None),
        }
    }

    /// Reports `fraction` (0..=1) now. Returns whether a snapshot was written.
    pub fn report(&self, fraction: f64) -> bool {
        self.report_at(fraction, Instant::now())
    }

    /// Reports `fraction` as of `now`. NaN is ignored, values are clamped to 0..=1, a value
    /// below the last written one is dropped, and writes within the throttle interval are
    /// skipped unless the job reached 1.0.
    pub fn report_at(&self, fraction: f64, now: Instant) -> bool {
        if fraction.is_nan() {
            return false;
        }
        let fraction = fraction.clamp(0.0, 1.0);
        let Ok(mut last) = self.last.lock() else {
            return false;
        };
        if let Some((written_at, written)) = *last {
            if fraction <= written {
                return false;
            }
            let throttled = now.saturating_duration_since(written_at) < JOB_PROGRESS_MIN_INTERVAL;
            if throttled && fraction < 1.0 {
                return false;
            }
        }
        if self.write_snapshot(fraction).is_err() {
            return false;
        }
        *last = Some((now, fraction));
        true
    }

    /// Removes this job's snapshot, if any.
    pub fn clear(&self) {
        if let Some(path) = snapshot_path(&self.project_dir, &self.job_id) {
            let _ = std::fs::remove_file(path);
        }
    }

    fn write_snapshot(&self, fraction: f64) -> Result<(), String> {
        if !self.project_dir.join(PROJECT_FILE_NAME).is_file() {
            return Err("not a split project".to_string());
        }
        let path = snapshot_path(&self.project_dir, &self.job_id)
            .ok_or_else(|| format!("unsafe job id for progress: {}", self.job_id))?;
        let dir = self.project_dir.join(JOB_PROGRESS_DIR);
        validate_split_project_write_path(&self.project_dir, &path)
            .map_err(|error| error.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        validate_split_project_write_path(&self.project_dir, &dir)
            .map_err(|error| error.to_string())?;
        let snapshot = JobProgressSnapshot {
            job_id: self.job_id.clone(),
            progress: fraction,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        let bytes = serde_json::to_vec(&snapshot).map_err(|error| error.to_string())?;
        let mut temp = tempfile::Builder::new()
            .prefix(".progress-")
            .suffix(".tmp")
            .tempfile_in(&dir)
            .map_err(|error| error.to_string())?;
        temp.write_all(&bytes).map_err(|error| error.to_string())?;
        temp.persist(&path).map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn snapshot_path(project_dir: &Path, job_id: &str) -> Option<PathBuf> {
    let mut components = Path::new(job_id).components();
    let safe = matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !job_id.contains(['/', '\\'])
        && !job_id.starts_with('.');
    safe.then(|| {
        project_dir
            .join(JOB_PROGRESS_DIR)
            .join(format!("{job_id}.json"))
    })
}

/// Snapshot files in the progress directory: regular `.json` files only, never symlinks.
fn snapshot_files(project_dir: &Path) -> Result<Vec<(String, PathBuf, u64)>, String> {
    let dir = project_dir.join(JOB_PROGRESS_DIR);
    validate_split_project_write_path(project_dir, &dir).map_err(|error| error.to_string())?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read job progress: {error}")),
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(job_id) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".json"))
            .filter(|stem| !stem.is_empty() && !stem.starts_with('.'))
        else {
            continue;
        };
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_file() {
            files.push((job_id.to_string(), path, metadata.len()));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

/// Reads every valid progress snapshot of a split project without taking any lease.
pub fn read_job_progress_snapshots(project_dir: &Path) -> Result<Vec<JobProgressSnapshot>, String> {
    let mut snapshots = Vec::new();
    for (job_id, path, len) in snapshot_files(project_dir)? {
        if snapshots.len() >= MAX_SNAPSHOTS {
            break;
        }
        if len > MAX_SNAPSHOT_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(snapshot) = serde_json::from_slice::<JobProgressSnapshot>(&bytes) else {
            continue;
        };
        if snapshot.job_id == job_id && (0.0..=1.0).contains(&snapshot.progress) {
            snapshots.push(snapshot);
        }
    }
    Ok(snapshots)
}

/// Removes snapshots whose job is missing from `project` or already finished.
pub fn remove_settled_job_progress(
    project_dir: &Path,
    project: &VideoProject,
) -> Result<usize, String> {
    let mut removed = 0;
    for (job_id, path, _) in snapshot_files(project_dir)? {
        let unfinished = project.jobs.iter().any(|job| {
            job.id == job_id
                && matches!(
                    job.status,
                    JobStatus::Queued
                        | JobStatus::Running
                        | JobStatus::Progress
                        | JobStatus::Blocked
                )
        });
        if !unfinished && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;
    use crate::project::model::JobSummary;
    use crate::project::mutation::acquire_split_project_mutation_lease;
    use crate::project::split::save_split_project;
    use crate::settings::storage::acquire_storage_mutation_lease;
    use std::sync::mpsc;

    fn split_project() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("temp");
        let dir = temp.path().join("project");
        save_split_project(&dir, &sample_project()).expect("save fixture");
        (temp, dir)
    }

    fn snapshot_progress(dir: &Path, job_id: &str) -> Option<f64> {
        read_job_progress_snapshots(dir)
            .expect("read snapshots")
            .into_iter()
            .find(|snapshot| snapshot.job_id == job_id)
            .map(|snapshot| snapshot.progress)
    }

    fn job(id: &str, status: JobStatus) -> JobSummary {
        JobSummary {
            id: id.to_string(),
            kind: "export_media".to_string(),
            status,
            updated_at: "2026-09-16T10:00:00Z".to_string(),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        }
    }

    #[test]
    fn progress_snapshots_survive_a_metadata_save() {
        let (_temp, dir) = split_project();
        let reporter = JobProgressReporter::new(&dir, "job-render");
        assert!(reporter.report(0.4));

        let mut project = sample_project();
        project.name = "Renamed while rendering".to_string();
        save_split_project(&dir, &project).expect("metadata save");

        assert_eq!(snapshot_progress(&dir, "job-render"), Some(0.4));
    }

    #[test]
    fn reporter_clamps_ignores_nan_and_never_goes_backwards() {
        let (_temp, dir) = split_project();
        let reporter = JobProgressReporter::new(&dir, "job-clamp");
        let t0 = Instant::now();

        assert!(!reporter.report_at(f64::NAN, t0));
        assert_eq!(snapshot_progress(&dir, "job-clamp"), None);
        assert!(reporter.report_at(-3.0, t0));
        assert_eq!(snapshot_progress(&dir, "job-clamp"), Some(0.0));
        assert!(reporter.report_at(0.6, t0 + Duration::from_secs(1)));
        assert!(!reporter.report_at(0.3, t0 + Duration::from_secs(2)));
        assert_eq!(snapshot_progress(&dir, "job-clamp"), Some(0.6));
        assert!(reporter.report_at(7.0, t0 + Duration::from_secs(3)));
        assert_eq!(snapshot_progress(&dir, "job-clamp"), Some(1.0));
    }

    #[test]
    fn reporter_throttles_writes_except_completion() {
        let (_temp, dir) = split_project();
        let reporter = JobProgressReporter::new(&dir, "job-throttle");
        let t0 = Instant::now();

        assert!(reporter.report_at(0.1, t0));
        assert!(!reporter.report_at(0.2, t0 + Duration::from_millis(100)));
        assert_eq!(snapshot_progress(&dir, "job-throttle"), Some(0.1));
        assert!(reporter.report_at(0.3, t0 + Duration::from_millis(600)));
        assert!(reporter.report_at(1.0, t0 + Duration::from_millis(650)));
        assert_eq!(snapshot_progress(&dir, "job-throttle"), Some(1.0));
    }

    #[test]
    fn reporter_writes_atomically_inside_the_project() {
        let (_temp, dir) = split_project();
        let reporter = JobProgressReporter::new(&dir, "job-atomic");
        assert!(reporter.report(0.25));

        let progress_dir = dir.join(JOB_PROGRESS_DIR);
        let names: Vec<String> = std::fs::read_dir(&progress_dir)
            .expect("progress dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .into_string()
                    .expect("utf8")
            })
            .collect();
        assert_eq!(names, vec!["job-atomic.json".to_string()]);
        let snapshot: JobProgressSnapshot = serde_json::from_slice(
            &std::fs::read(progress_dir.join("job-atomic.json")).expect("snapshot"),
        )
        .expect("json");
        assert_eq!(snapshot.job_id, "job-atomic");
        assert!(!snapshot.updated_at.is_empty());

        assert!(!JobProgressReporter::new(&dir, "../escape").report(0.5));
        assert!(!dir.join("logs/escape.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn reporter_refuses_a_symlinked_progress_directory() {
        let (temp, dir) = split_project();
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).expect("outside");
        std::fs::create_dir_all(dir.join("logs")).expect("logs");
        std::os::unix::fs::symlink(&outside, dir.join(JOB_PROGRESS_DIR)).expect("symlink");

        assert!(!JobProgressReporter::new(&dir, "job-link").report(0.5));
        assert_eq!(std::fs::read_dir(&outside).expect("outside").count(), 0);
        assert!(read_job_progress_snapshots(&dir).is_err());
    }

    #[test]
    fn reporter_does_not_write_without_a_project_manifest() {
        let temp = tempfile::tempdir().expect("temp");
        let reporter = JobProgressReporter::new(temp.path(), "job-none");

        assert!(!reporter.report(0.5));
        assert!(!temp.path().join("logs").exists());
    }

    #[test]
    fn reader_skips_invalid_entries_and_caps_the_count() {
        let (_temp, dir) = split_project();
        let progress_dir = dir.join(JOB_PROGRESS_DIR);
        std::fs::create_dir_all(&progress_dir).expect("progress dir");
        let valid = |id: &str| {
            serde_json::to_vec(&JobProgressSnapshot {
                job_id: id.to_string(),
                progress: 0.5,
                updated_at: "2026-09-16T10:00:00Z".to_string(),
            })
            .expect("json")
        };
        std::fs::write(progress_dir.join("job-ok.json"), valid("job-ok")).expect("ok");
        std::fs::write(progress_dir.join("job-ok.txt"), valid("job-ok")).expect("txt");
        std::fs::write(progress_dir.join("job-bad.json"), b"{not json").expect("bad");
        std::fs::write(progress_dir.join("job-other.json"), valid("job-mismatch")).expect("id");
        let mut large = valid("job-large");
        large.resize(5 * 1024, b' ');
        std::fs::write(progress_dir.join("job-large.json"), large).expect("large");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            progress_dir.join("job-ok.json"),
            progress_dir.join("job-link.json"),
        )
        .expect("symlink");

        let ids: Vec<String> = read_job_progress_snapshots(&dir)
            .expect("read")
            .into_iter()
            .map(|snapshot| snapshot.job_id)
            .collect();
        assert_eq!(ids, vec!["job-ok".to_string()]);

        for index in 0..300 {
            let id = format!("job-many-{index:03}");
            std::fs::write(progress_dir.join(format!("{id}.json")), valid(&id)).expect("many");
        }
        assert_eq!(read_job_progress_snapshots(&dir).expect("read").len(), 256);
    }

    #[test]
    fn reader_takes_no_lease() {
        let (_temp, dir) = split_project();
        assert!(JobProgressReporter::new(&dir, "job-leased").report(0.7));
        let (held_tx, held_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let holder_dir = dir.clone();
        let holder = std::thread::spawn(move || {
            let storage = acquire_storage_mutation_lease().expect("storage lease");
            let project = acquire_split_project_mutation_lease(&holder_dir).expect("project lease");
            held_tx.send(()).expect("held");
            let _ = release_rx.recv();
            drop(project);
            drop(storage);
        });
        held_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("leases held");

        let (read_tx, read_rx) = mpsc::channel();
        let reader_dir = dir.clone();
        std::thread::spawn(move || read_tx.send(read_job_progress_snapshots(&reader_dir)));
        let snapshots = read_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("read returns while both leases are held")
            .expect("read");
        assert_eq!(snapshots.len(), 1);

        release_tx.send(()).expect("release");
        holder.join().expect("holder");
    }

    #[test]
    fn clear_removes_the_snapshot() {
        let (_temp, dir) = split_project();
        let reporter = JobProgressReporter::new(&dir, "job-clear");
        assert!(reporter.report(0.2));
        reporter.clear();
        assert_eq!(snapshot_progress(&dir, "job-clear"), None);
    }

    #[test]
    fn settled_snapshots_are_removed_and_unfinished_ones_kept() {
        let (_temp, dir) = split_project();
        for id in ["job-running", "job-done", "job-gone"] {
            assert!(JobProgressReporter::new(&dir, id).report(0.5));
        }
        let mut project = sample_project();
        project.jobs = vec![
            job("job-running", JobStatus::Running),
            job("job-done", JobStatus::Completed),
        ];

        assert_eq!(remove_settled_job_progress(&dir, &project), Ok(2));
        let ids: Vec<String> = read_job_progress_snapshots(&dir)
            .expect("read")
            .into_iter()
            .map(|snapshot| snapshot.job_id)
            .collect();
        assert_eq!(ids, vec!["job-running".to_string()]);
    }
}
