//! Durable local render admission. Editor ownership covers validation and the queued write;
//! workers own immutable inputs, cancellation and artifact leases after admission returns.

use serde::de::DeserializeOwned;
#[path = "admission_retention.rs"]
mod retention;
use std::fs::{File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::{
    fd::{AsRawFd, FromRawFd},
    unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::*;
use crate::app_service::error::ServiceError;
use crate::project::split::{
    apply_project_actions_to_split_project_if_revision, validate_split_project_write_path,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaRenderInput {
    pub project_id: String,
    pub profile: ExportProfile,
    pub quality: RenderQuality,
    pub width: u32,
    pub height: u32,
    pub job_id: String,
    pub attempt_id: String,
    pub updated_at: String,
    #[serde(default)]
    pub range_start_seconds: Option<f64>,
    #[serde(default)]
    pub range_end_seconds: Option<f64>,
    #[serde(default)]
    pub timeline_id: Option<String>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default)]
    pub encode_tier: Option<crate::edit::render_plan::ExportEncodeTier>,
    #[serde(default)]
    pub output: Option<ExportOutputRequest>,
    #[serde(default)]
    pub export_settings: Option<crate::project::export_options::JobExportSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaRenderAdmission {
    pub admission_protocol: u32,
    pub project: VideoProject,
    pub job_id: String,
    pub attempt_id: String,
    pub source_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum MediaRenderAttempt {
    Pending,
    Completed {
        result: Box<ProjectMediaRenderResult>,
    },
    Failed {
        message: String,
        #[serde(default)]
        interrupted: bool,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DurableInput {
    identity: PackageIdentity,
    protocol: u32,
    source_revision: u64,
    input: MediaRenderInput,
    project: VideoProject,
}

fn attempt_directory(project_dir: &Path, job_id: &str, attempt_id: &str) -> PathBuf {
    // Hash identities instead of lossy path sanitization, which can alias distinct attempts.
    let digest = Sha256::digest(format!("{}:{job_id}{attempt_id}", job_id.len()).as_bytes());
    project_dir
        .join("logs/render-admissions")
        .join(format!("{digest:x}"))
}

/// Root directory identity survives metadata transactions but rejects package replacement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct PackageIdentity {
    project_id: String,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl PackageIdentity {
    fn capture(project_dir: &Path, project_id: &str) -> Result<Self, ServiceError> {
        let actual = crate::app_service::projects::read_project_identity(project_dir)?;
        if actual.id != project_id {
            return Err(ServiceError::forbidden());
        }
        let root = open_directory(project_dir)?;
        let metadata = root.metadata().map_err(io_error)?;
        let identity = Self {
            project_id: project_id.into(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        };
        identity.validate(project_dir)?;
        Ok(identity)
    }

    pub(super) fn validate(&self, project_dir: &Path) -> Result<(), ServiceError> {
        let root = open_directory(project_dir)?;
        let metadata = root.metadata().map_err(io_error)?;
        #[cfg(unix)]
        if metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(ServiceError::forbidden());
        }
        if crate::app_service::projects::read_project_identity(project_dir)?.id != self.project_id {
            return Err(ServiceError::forbidden());
        }
        Ok(())
    }

    /// Call while holding the project mutation lease. Terminal/superseded jobs are immutable.
    pub(super) fn active_job(
        &self,
        project_dir: &Path,
        job_id: &str,
        attempt_id: &str,
    ) -> Result<JobSummary, ServiceError> {
        self.validate(project_dir)?;
        let project = load_split_project(project_dir)
            .map_err(|error| ServiceError::internal(error.to_string()))?;
        self.validate(project_dir)?;
        project
            .jobs
            .into_iter()
            .find(|job| matching_attempt(job, job_id, attempt_id) && active(&job.status))
            .ok_or_else(|| ServiceError::invalid_input("render attempt is terminal or superseded"))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DurableAttemptIdentity {
    protocol: u32,
    identity: PackageIdentity,
    job_id: String,
    attempt_id: String,
    profile: ExportProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pin_identity: Option<PinIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PinIdentity {
    device: u64,
    inode: u64,
}

fn io_error(error: std::io::Error) -> ServiceError {
    ServiceError::internal(error.to_string())
}

fn open_directory(path: &Path) -> Result<File, ServiceError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NONBLOCK);
    let file = options.open(path).map_err(io_error)?;
    if !file.metadata().map_err(io_error)?.is_dir() {
        return Err(ServiceError::forbidden());
    }
    Ok(file)
}

/// Every ancestor is opened without following links. Operations use the opened parent fd,
/// and re-open the pathname to reject an ancestor swap before publication/after reading.
struct RecordParent {
    root: PathBuf,
    path: PathBuf,
    root_file: File,
    file: File,
}
impl RecordParent {
    fn open(project_dir: &Path, path: &Path, create: bool) -> Result<Self, ServiceError> {
        Self::open_with_identity(project_dir, path, create, None)
    }
    fn open_with_identity(
        project_dir: &Path,
        path: &Path,
        create: bool,
        identity: Option<&PackageIdentity>,
    ) -> Result<Self, ServiceError> {
        let relative = path
            .parent()
            .and_then(|parent| parent.strip_prefix(project_dir).ok())
            .ok_or_else(ServiceError::forbidden)?;
        let root_file = open_directory(project_dir)?;
        #[cfg(unix)]
        if let Some(identity) = identity {
            let metadata = root_file.metadata().map_err(io_error)?;
            if (metadata.dev(), metadata.ino()) != (identity.device, identity.inode) {
                return Err(ServiceError::forbidden());
            }
        }
        let mut file = root_file.try_clone().map_err(io_error)?;
        for component in relative.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(ServiceError::forbidden());
            };
            #[cfg(unix)]
            {
                let name = std::ffi::CString::new(name.as_bytes())
                    .map_err(|_| ServiceError::forbidden())?;
                if create {
                    let result = unsafe { libc::mkdirat(file.as_raw_fd(), name.as_ptr(), 0o700) };
                    if result != 0
                        && std::io::Error::last_os_error().kind()
                            != std::io::ErrorKind::AlreadyExists
                    {
                        return Err(io_error(std::io::Error::last_os_error()));
                    }
                }
                let fd = unsafe {
                    libc::openat(
                        file.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY
                            | libc::O_DIRECTORY
                            | libc::O_NOFOLLOW
                            | libc::O_CLOEXEC
                            | libc::O_NONBLOCK,
                    )
                };
                if fd < 0 {
                    return Err(io_error(std::io::Error::last_os_error()));
                }
                file = unsafe { File::from_raw_fd(fd) };
            }
            #[cfg(not(unix))]
            {
                let _ = (name, create);
                return Err(ServiceError::unavailable("safe render records"));
            }
        }
        let result = Self {
            root: project_dir.into(),
            path: path.into(),
            root_file,
            file,
        };
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), ServiceError> {
        #[cfg(unix)]
        {
            let current = open_directory(&self.root)?;
            let expected = self.root_file.metadata().map_err(io_error)?;
            let actual = current.metadata().map_err(io_error)?;
            if (expected.dev(), expected.ino()) != (actual.dev(), actual.ino()) {
                return Err(ServiceError::forbidden());
            }
            // Open each component again, avoiding a recursive validate call.
            let relative = self
                .path
                .parent()
                .unwrap()
                .strip_prefix(&self.root)
                .map_err(|_| ServiceError::forbidden())?;
            let mut current = current;
            for component in relative.components() {
                let std::path::Component::Normal(name) = component else {
                    return Err(ServiceError::forbidden());
                };
                let name = std::ffi::CString::new(name.as_bytes())
                    .map_err(|_| ServiceError::forbidden())?;
                let fd = unsafe {
                    libc::openat(
                        current.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY
                            | libc::O_DIRECTORY
                            | libc::O_NOFOLLOW
                            | libc::O_CLOEXEC
                            | libc::O_NONBLOCK,
                    )
                };
                if fd < 0 {
                    return Err(io_error(std::io::Error::last_os_error()));
                }
                current = unsafe { File::from_raw_fd(fd) };
            }
            let expected = self.file.metadata().map_err(io_error)?;
            let actual = current.metadata().map_err(io_error)?;
            if (expected.dev(), expected.ino()) != (actual.dev(), actual.ino()) {
                return Err(ServiceError::forbidden());
            }
        }
        Ok(())
    }
    #[cfg(unix)]
    fn name(&self) -> Result<std::ffi::CString, ServiceError> {
        std::ffi::CString::new(
            self.path
                .file_name()
                .ok_or_else(ServiceError::forbidden)?
                .as_bytes(),
        )
        .map_err(|_| ServiceError::forbidden())
    }
}

fn read_record<T: DeserializeOwned>(project_dir: &Path, path: &Path) -> Result<T, ServiceError> {
    let parent = RecordParent::open(project_dir, path, false)?;
    #[cfg(unix)]
    {
        let name = parent.name()?;
        let fd = unsafe {
            libc::openat(
                parent.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata().map_err(io_error)?.is_file() {
            return Err(ServiceError::forbidden());
        }
        // The immutable project snapshot can be large; stream it without an arbitrary size cap.
        let value = serde_json::from_reader(file)
            .map_err(|error| ServiceError::internal(error.to_string()))?;
        parent.validate()?;
        Ok(value)
    }
    #[cfg(not(unix))]
    {
        Err(ServiceError::unavailable("safe render records"))
    }
}

#[cfg(test)]
fn write_record<T: Serialize>(
    project_dir: &Path,
    path: &Path,
    record: &T,
) -> Result<(), ServiceError> {
    write_record_with_identity(project_dir, path, record, None)
}

fn write_record_with_identity<T: Serialize>(
    project_dir: &Path,
    path: &Path,
    record: &T,
    identity: Option<&PackageIdentity>,
) -> Result<(), ServiceError> {
    let parent = RecordParent::open_with_identity(project_dir, path, true, identity)?;
    #[cfg(unix)]
    {
        let temp_name =
            std::ffi::CString::new(format!(".record-{}.tmp", uuid::Uuid::new_v4())).unwrap();
        let fd = unsafe {
            libc::openat(
                parent.file.as_raw_fd(),
                temp_name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            serde_json::to_writer(&mut file, record)
                .map_err(|error| ServiceError::internal(error.to_string()))?;
            file.flush()
                .and_then(|()| file.sync_all())
                .map_err(io_error)?;
            parent.validate()?;
            let name = parent.name()?;
            if unsafe {
                libc::renameat(
                    parent.file.as_raw_fd(),
                    temp_name.as_ptr(),
                    parent.file.as_raw_fd(),
                    name.as_ptr(),
                )
            } != 0
            {
                return Err(io_error(std::io::Error::last_os_error()));
            }
            parent.file.sync_all().map_err(io_error)?;
            parent.validate()
        })();
        if result.is_err() {
            unsafe {
                libc::unlinkat(parent.file.as_raw_fd(), temp_name.as_ptr(), 0);
            }
        }
        result
    }
    #[cfg(not(unix))]
    {
        Err(ServiceError::unavailable("safe render records"))
    }
}

#[cfg(test)]
fn write_input(project_dir: &Path, dir: &Path, record: &DurableInput) -> Result<(), ServiceError> {
    write_input_with_pin(project_dir, dir, record, None)
}

fn write_input_with_pin(
    project_dir: &Path,
    dir: &Path,
    record: &DurableInput,
    pin_identity: Option<PinIdentity>,
) -> Result<(), ServiceError> {
    write_record_with_identity(
        project_dir,
        &dir.join("input.json"),
        record,
        Some(&record.identity),
    )?;
    write_record_with_identity(
        project_dir,
        &dir.join("identity.json"),
        &DurableAttemptIdentity {
            protocol: record.protocol,
            identity: record.identity.clone(),
            job_id: record.input.job_id.clone(),
            attempt_id: record.input.attempt_id.clone(),
            profile: record.input.profile,
            pin_identity,
        },
        Some(&record.identity),
    )
}

fn matching_attempt(job: &JobSummary, job_id: &str, attempt_id: &str) -> bool {
    job.id == job_id
        && job
            .workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref())
            == Some(attempt_id)
}
fn active(status: &JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Queued | JobStatus::Running | JobStatus::Progress
    )
}

fn fail_active_attempt(
    project_dir: &Path,
    identity: &PackageIdentity,
    job_id: &str,
    attempt_id: &str,
) {
    if let Ok(mutation) =
        crate::project::mutation::acquire_split_project_mutation_lease(project_dir)
    {
        if identity.active_job(project_dir, job_id, attempt_id).is_ok() {
            let _ = crate::project::split::apply_project_bookkeeping_actions_to_split_project_with_lease(
                project_dir,
                vec![ProjectAction::UpdateJobStatus {
                    job_id: job_id.into(),
                    status: JobStatus::Failed,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    run_id: Some(attempt_id.into()),
                }],
                &mutation,
            );
        }
    }
}

/// A transferable OS lock protects admitted attempts before a worker obtains its artifact lease.
/// Recovery probes it without waiting, after taking artifact-before-mutation ownership.
struct AdmissionPin {
    file: File,
    path: PathBuf,
    project_dir: PathBuf,
    unaccepted_identity: Option<PackageIdentity>,
    locked: bool,
}

// Storage ownership serializes execution, but waiting workers still retain a project
// snapshot and an OS thread. Bound accepted workers across all packages in this process.
const ADMITTED_WORKER_LIMIT: usize = 4;

#[derive(Default)]
struct WorkerUsage {
    active: usize,
    bytes: u64,
}

struct WorkerCapacity {
    usage: std::sync::Mutex<WorkerUsage>,
}

impl WorkerCapacity {
    fn new() -> Self {
        Self {
            usage: std::sync::Mutex::new(WorkerUsage::default()),
        }
    }
    #[cfg(test)]
    fn reserve(self: &std::sync::Arc<Self>) -> Result<WorkerPermit, ServiceError> {
        self.reserve_bytes(0)
    }
    fn reserve_bytes(
        self: &std::sync::Arc<Self>,
        bytes: u64,
    ) -> Result<WorkerPermit, ServiceError> {
        let mut usage = self
            .usage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if usage.active >= ADMITTED_WORKER_LIMIT
            || usage.bytes.saturating_add(bytes) > retention::MAX_IN_FLIGHT_BYTES
        {
            return Err(ServiceError::busy("render worker capacity"));
        }
        usage.active += 1;
        usage.bytes += bytes;
        Ok(WorkerPermit {
            capacity: std::sync::Arc::clone(self),
            bytes,
        })
    }
}
struct WorkerPermit {
    capacity: std::sync::Arc<WorkerCapacity>,
    bytes: u64,
}
impl Drop for WorkerPermit {
    fn drop(&mut self) {
        let mut usage = self
            .capacity
            .usage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        usage.active = usage.active.saturating_sub(1);
        usage.bytes = usage.bytes.saturating_sub(self.bytes);
    }
}

fn worker_capacity() -> std::sync::Arc<WorkerCapacity> {
    #[cfg(not(test))]
    {
        static CAPACITY: std::sync::OnceLock<std::sync::Arc<WorkerCapacity>> =
            std::sync::OnceLock::new();
        std::sync::Arc::clone(CAPACITY.get_or_init(|| std::sync::Arc::new(WorkerCapacity::new())))
    }
    #[cfg(test)]
    {
        // Existing executor-injected unit tests remain independent. Capacity regressions
        // explicitly share a limiter between competing callers and packages.
        capacity_tests::isolated_worker_capacity()
    }
}

fn pin_path(project_dir: &Path, job_id: &str, attempt_id: &str) -> Result<PathBuf, ServiceError> {
    let root = std::fs::canonicalize(project_dir)
        .map_err(|error| ServiceError::internal(error.to_string()))?;
    let digest = Sha256::digest(
        format!("{}:{}:{job_id}{attempt_id}", root.display(), job_id.len()).as_bytes(),
    );
    Ok(root
        .parent()
        .ok_or_else(|| ServiceError::invalid_input("project has no parent"))?
        .join(format!(".render-admission-{digest:x}.lock")))
}

impl AdmissionPin {
    fn acquire(project_dir: &Path, job_id: &str, attempt_id: &str) -> Result<Self, ServiceError> {
        Self::open(project_dir, job_id, attempt_id, None)
    }

    fn acquire_new(
        project_dir: &Path,
        job_id: &str,
        attempt_id: &str,
        identity: &PackageIdentity,
    ) -> Result<Self, ServiceError> {
        Self::open(project_dir, job_id, attempt_id, Some(identity))
    }

    fn open(
        project_dir: &Path,
        job_id: &str,
        attempt_id: &str,
        new_identity: Option<&PackageIdentity>,
    ) -> Result<Self, ServiceError> {
        #[cfg(unix)]
        {
            let path = pin_path(project_dir, job_id, attempt_id)?;
            let mut options = OpenOptions::new();
            options
                .read(true)
                .write(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
            if new_identity.is_some() {
                options.create_new(true);
            } else {
                options.create(true);
            }
            let file = options.open(&path).map_err(|error| {
                if new_identity.is_some() && error.kind() == std::io::ErrorKind::AlreadyExists {
                    ServiceError::busy("render attempt pin")
                } else {
                    io_error(error)
                }
            })?;
            if !file.metadata().map_err(io_error)?.is_file() {
                return Err(ServiceError::forbidden());
            }
            let mut pin = Self {
                file,
                path,
                project_dir: project_dir.into(),
                unaccepted_identity: new_identity.cloned(),
                locked: false,
            };
            if unsafe { libc::flock(pin.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(ServiceError::busy("render attempt"));
            }
            pin.locked = true;
            Ok(pin)
        }
        #[cfg(not(unix))]
        {
            let _ = (project_dir, job_id, attempt_id, new_identity);
            Err(ServiceError::unavailable("render admission locking"))
        }
    }
}

impl AdmissionPin {
    fn identity(&self) -> Result<PinIdentity, ServiceError> {
        let metadata = self.file.metadata().map_err(io_error)?;
        #[cfg(unix)]
        {
            Ok(PinIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            Err(ServiceError::unavailable("safe render pins"))
        }
    }
    fn remove_if_owned(
        &self,
        project_dir: &Path,
        identity: &PackageIdentity,
        expected: &PinIdentity,
    ) -> Result<(), ServiceError> {
        identity.validate(project_dir)?;
        if &self.identity()? != expected {
            return Err(ServiceError::forbidden());
        }
        #[cfg(unix)]
        {
            let parent_path = self.path.parent().ok_or_else(ServiceError::forbidden)?;
            let parent = open_directory(parent_path)?;
            let name = std::ffi::CString::new(
                self.path
                    .file_name()
                    .ok_or_else(ServiceError::forbidden)?
                    .as_bytes(),
            )
            .map_err(|_| ServiceError::forbidden())?;
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io_error(std::io::Error::last_os_error()));
            }
            let current = unsafe { File::from_raw_fd(fd) };
            let metadata = current.metadata().map_err(io_error)?;
            if !metadata.is_file()
                || (metadata.dev(), metadata.ino()) != (expected.device, expected.inode)
            {
                return Err(ServiceError::forbidden());
            }
            let current_parent = open_directory(parent_path)?.metadata().map_err(io_error)?;
            let expected_parent = parent.metadata().map_err(io_error)?;
            if (current_parent.dev(), current_parent.ino())
                != (expected_parent.dev(), expected_parent.ino())
            {
                return Err(ServiceError::forbidden());
            }
            identity.validate(project_dir)?;
            if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                return Err(io_error(std::io::Error::last_os_error()));
            }
            parent.sync_all().map_err(io_error)
        }
        #[cfg(not(unix))]
        {
            Err(ServiceError::unavailable("safe render pins"))
        }
    }
}

impl Drop for AdmissionPin {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            if self.locked {
                if let Some(identity) = self.unaccepted_identity.as_ref() {
                    // New admission callers still hold their mutation lease on every
                    // pre-accept return. Only exclusive-created, still-owned files qualify.
                    if let Ok(expected) = self.identity() {
                        let _ = self.remove_if_owned(&self.project_dir, identity, &expected);
                    }
                }
                unsafe {
                    libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
                }
            }
        }
    }
}

pub(super) fn attempt_is_pinned(project_dir: &Path, job_id: &str, attempt_id: &str) -> bool {
    let Ok(path) = pin_path(project_dir, job_id, attempt_id) else {
        return true;
    };
    #[cfg(unix)]
    {
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
            Err(_) => return true,
        };
        if !file.metadata().map(|m| m.is_file()).unwrap_or(false) {
            return true;
        }
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            unsafe {
                libc::flock(file.as_raw_fd(), libc::LOCK_UN);
            }
            false
        } else {
            true
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn admit_media_render(
    project_dir: &Path,
    expected_revision: u64,
    input: MediaRenderInput,
) -> Result<MediaRenderAdmission, ServiceError> {
    admit_with_executor(project_dir, expected_revision, input, execute_admitted)
}

fn admit_with_executor(
    project_dir: &Path,
    expected_revision: u64,
    input: MediaRenderInput,
    execute: impl FnOnce(
            PathBuf,
            DurableInput,
            RegisteredProjectRenderAttempt,
        ) -> PipelineResult<ProjectMediaRenderResult>
        + Send
        + 'static,
) -> Result<MediaRenderAdmission, ServiceError> {
    admit_with_executor_and_spawner(project_dir, expected_revision, input, execute, |worker| {
        std::thread::Builder::new()
            .name("admitted-render".into())
            .spawn(worker)
            .map(|_| ())
    })
}

fn admit_with_executor_and_spawner(
    project_dir: &Path,
    expected_revision: u64,
    input: MediaRenderInput,
    execute: impl FnOnce(
            PathBuf,
            DurableInput,
            RegisteredProjectRenderAttempt,
        ) -> PipelineResult<ProjectMediaRenderResult>
        + Send
        + 'static,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
) -> Result<MediaRenderAdmission, ServiceError> {
    admit_with_capacity_and_spawner(
        project_dir,
        expected_revision,
        input,
        worker_capacity(),
        execute,
        spawn,
    )
}

fn admit_with_capacity_and_spawner(
    project_dir: &Path,
    expected_revision: u64,
    input: MediaRenderInput,
    capacity: std::sync::Arc<WorkerCapacity>,
    execute: impl FnOnce(
            PathBuf,
            DurableInput,
            RegisteredProjectRenderAttempt,
        ) -> PipelineResult<ProjectMediaRenderResult>
        + Send
        + 'static,
    spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<()>,
) -> Result<MediaRenderAdmission, ServiceError> {
    if input.job_id.trim().is_empty()
        || !input.attempt_id.starts_with("render-attempt/")
        || input.attempt_id.len() > 256
        || input.job_id.len() > 256
    {
        return Err(ServiceError::invalid_input(
            "render job and attempt identities are invalid",
        ));
    }
    let options = ExportRenderOptions::new(input.profile, input.quality, input.width, input.height)
        .and_then(|options| options.with_fps(input.fps))
        .and_then(|options| options.with_encode_tier(input.encode_tier.unwrap_or_default()))
        .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
    let range = match (input.range_start_seconds, input.range_end_seconds) {
        (Some(start), Some(end)) => Some((start, end)),
        (None, None) => None,
        _ => {
            return Err(ServiceError::invalid_input(
                "render range bounds must be provided together",
            ))
        }
    };
    validate_render_range(range)
        .map_err(|_| ServiceError::invalid_input("render range is invalid"))?;
    let project_dir = std::fs::canonicalize(project_dir)
        .map_err(|error| ServiceError::internal(error.to_string()))?;
    let _mutation = crate::project::mutation::acquire_split_project_mutation_lease(&project_dir)
        .map_err(ServiceError::internal)?;
    let project = load_split_project(&project_dir)
        .map_err(|error| ServiceError::internal(error.to_string()))?;
    if project.id != input.project_id {
        return Err(ServiceError::forbidden());
    }
    let dir = attempt_directory(&project_dir, &input.job_id, &input.attempt_id);
    let record_path = dir.join("input.json");
    validate_split_project_write_path(&project_dir, &record_path)
        .map_err(|error| ServiceError::internal(error.to_string()))?;
    if record_path.exists() {
        let record = retention::read_input(&project_dir, &record_path)?;
        record.identity.validate(&project_dir)?;
        if record.input != input || record.source_revision != expected_revision {
            return Err(ServiceError::invalid_input(
                "render attempt identity was reused with different inputs",
            ));
        }
        if !project.jobs.iter().any(|job| {
            job.id == input.job_id
                && job
                    .workflow
                    .as_ref()
                    .and_then(|workflow| workflow.run_id.as_deref())
                    == Some(input.attempt_id.as_str())
        }) {
            return Err(ServiceError::invalid_input(
                "render admission was not committed or was superseded",
            ));
        }
        return Ok(MediaRenderAdmission {
            admission_protocol: 1,
            project,
            job_id: input.job_id,
            attempt_id: input.attempt_id,
            source_revision: expected_revision,
        });
    }
    if project.content_revision != expected_revision {
        return Err(ServiceError::revision_conflict(
            expected_revision,
            project.content_revision,
        ));
    }
    if project.jobs.iter().any(|job| {
        job.id == input.job_id
            && matches!(
                job.status,
                JobStatus::Queued | JobStatus::Running | JobStatus::Progress
            )
    }) {
        return Err(ServiceError::busy("render job"));
    }
    if let Some(timeline_id) = input.timeline_id.as_deref() {
        if project.projected_for_timeline(timeline_id).is_none() {
            return Err(ServiceError::invalid_input(
                "requested timeline was not found",
            ));
        }
    }
    if let Some(output) = input.output.as_ref() {
        named_export::resolve_output_destination(&project_dir, options, output)
            .map_err(|_| ServiceError::invalid_input("export destination is invalid"))?;
    }
    // Replay above does not consume another slot. Reject new work before any durable
    // input/job write, and release the reservation on every unsuccessful admission.
    let record = DurableInput {
        identity: PackageIdentity::capture(&project_dir, &project.id)?,
        protocol: 1,
        source_revision: expected_revision,
        input: input.clone(),
        project: project.clone(),
    };
    let input_bytes = retention::serialized_bytes(&record, retention::MAX_INPUT_BYTES)?;
    retention::check_package_capacity(&project_dir, &record, input_bytes)?;
    let permit = capacity.reserve_bytes(input_bytes)?;
    let mut pin = AdmissionPin::acquire_new(
        &project_dir,
        &input.job_id,
        &input.attempt_id,
        &record.identity,
    )?;
    let attempt = register_project_render_attempt(
        &project_dir,
        &project.id,
        &input.job_id,
        Some(input.attempt_id.clone()),
    )
    .map_err(|_| ServiceError::busy("render attempt"))?;
    let mut job = crate::workflows::temporal_job_summary(
        crate::workflows::TemporalWorkflowKind::RenderDraft,
        &input.project_id,
        &input.job_id,
        JobStatus::Queued,
        &input.updated_at,
    );
    if let Some(workflow) = job.workflow.as_mut() {
        workflow.run_id = Some(input.attempt_id.clone());
    }
    if let Some(output) = input.output.clone() {
        job.export_settings = Some(
            crate::project::export_options::JobExportSettings::for_export(
                options,
                input.export_settings.clone(),
                output,
            ),
        );
    }
    let actions = if let Some(existing) = project.jobs.iter().find(|existing| existing.id == job.id)
    {
        if existing.export_settings != job.export_settings {
            return Err(ServiceError::invalid_input(
                "retry must preserve the recorded export settings",
            ));
        }
        vec![ProjectAction::UpdateJobStatus {
            job_id: job.id.clone(),
            status: JobStatus::Queued,
            updated_at: input.updated_at.clone(),
            run_id: Some(input.attempt_id.clone()),
        }]
    } else {
        vec![ProjectAction::RecordJob { job: Box::new(job) }]
    };
    let mut validated = project.clone();
    for action in actions.iter().cloned() {
        crate::project::action::apply_project_action(&mut validated, action)
            .map_err(|error| ServiceError::invalid_input(error.to_string()))?;
    }
    write_input_with_pin(&project_dir, &dir, &record, Some(pin.identity()?))?;
    let write = apply_project_actions_to_split_project_if_revision(
        &project_dir,
        actions,
        &input.project_id,
        expected_revision,
    )
    .map_err(|error| ServiceError::internal(error.to_string()))?;
    // Canonical admission has committed. Keep the pin through terminal publication;
    // only pre-accept failures qualify for the Drop cleanup above.
    pin.unaccepted_identity = None;
    let admission = MediaRenderAdmission {
        admission_protocol: 1,
        project: write.project,
        job_id: input.job_id.clone(),
        attempt_id: input.attempt_id.clone(),
        source_revision: expected_revision,
    };
    // The worker never takes the editor gate. A pin bridges the cross-process recovery gap
    // between this durable write and acquisition of the worker's artifact lease.
    let worker_dir = project_dir.clone();
    let identity = record.identity.clone();
    let failure_identity = identity.clone();
    let result_path = dir.join("result.json");
    drop(_mutation);
    let spawned = spawn(Box::new(move || {
        let pin = pin;
        let mut worker_permit = Some(permit);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute(worker_dir.clone(), record, attempt)
        }));
        let result = match outcome {
            Ok(Ok(result)) => MediaRenderAttempt::Completed {
                result: Box::new(result),
            },
            _ => {
                fail_active_attempt(&worker_dir, &identity, &input.job_id, &input.attempt_id);
                MediaRenderAttempt::Failed {
                    message: "Render stopped before completion. Inspect the render log and retry."
                        .into(),
                    interrupted: false,
                }
            }
        };
        if identity.validate(&worker_dir).is_ok()
            && retention::write_result(&worker_dir, &result_path, &result, &identity).is_ok()
        {
            // The completed result's project duplicate is no longer needed after receipt IO.
            drop(result);
            if let Ok(mutation) =
                crate::project::mutation::acquire_split_project_mutation_lease(&worker_dir)
            {
                if retention::compact_terminal(
                    &worker_dir,
                    &identity,
                    &input.job_id,
                    &input.attempt_id,
                    Some(&pin),
                    &mutation,
                )
                .unwrap_or(false)
                {
                    // Pin readers must not observe "worker exited" while its capacity is held.
                    drop(worker_permit.take());
                    if let Ok(expected) = pin.identity() {
                        let _ = pin.remove_if_owned(&worker_dir, &identity, &expected);
                    }
                }
            }
        }
    }));
    if spawned.is_err() {
        // Admission has already committed. A failed spawn is a terminal job outcome,
        // never a failed mutation acknowledgement.
        fail_active_attempt(
            &project_dir,
            &failure_identity,
            &admission.job_id,
            &admission.attempt_id,
        );
        if retention::write_result(
            &project_dir,
            &dir.join("result.json"),
            &MediaRenderAttempt::Failed {
                message: "Render worker could not start. Retry the render.".into(),
                interrupted: false,
            },
            &failure_identity,
        )
        .is_ok()
        {
            if let Ok(mutation) =
                crate::project::mutation::acquire_split_project_mutation_lease(&project_dir)
            {
                let _ = retention::compact_terminal(
                    &project_dir,
                    &failure_identity,
                    &admission.job_id,
                    &admission.attempt_id,
                    None,
                    &mutation,
                );
            }
        }
    }
    Ok(admission)
}

fn execute_admitted(
    project_dir: PathBuf,
    record: DurableInput,
    attempt: RegisteredProjectRenderAttempt,
) -> PipelineResult<ProjectMediaRenderResult> {
    let input = record.input;
    let options = ExportRenderOptions::new(input.profile, input.quality, input.width, input.height)
        .and_then(|options| options.with_fps(input.fps))
        .and_then(|options| options.with_encode_tier(input.encode_tier.unwrap_or_default()))
        .map_err(|_| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "render.options",
                "Invalid render options",
                "Start a new render",
            )]
        })?;
    let lease = acquire_render_storage_mutation_lease()?;
    let _artifacts = acquire_render_project_artifact_lease(&project_dir)?;
    let job = {
        let _mutation = acquire_render_project_mutation_lease(&project_dir)?;
        record
            .identity
            .active_job(&project_dir, &input.job_id, &input.attempt_id)
            .map_err(|_| render_cancelled_error("render.admission.identity"))?
    };
    let project = match input.timeline_id.as_deref() {
        Some(id) => record
            .project
            .projected_for_timeline(id)
            .ok_or_else(|| render_cancelled_error("render.admission.timeline"))?,
        None => record.project,
    };
    render_project_media_to_split_project_folder(ProjectMediaRenderRequest {
        admitted_project: Some(project),
        admitted_identity: Some(record.identity),
        project_dir: &project_dir,
        quality_profile: match input.quality {
            RenderQuality::Draft => RenderQualityProfile::DraftWebm,
            RenderQuality::Final => RenderQualityProfile::FinalWebm,
        },
        options: Some(options),
        output_profile: render_output_profile_for_export(input.profile)?,
        paths: ProjectMediaRenderPaths::new(&input.job_id, input.profile)?,
        job,
        updated_at: &input.updated_at,
        attempt: Some(attempt),
        range_seconds: input.range_start_seconds.zip(input.range_end_seconds),
        timeline_id: None,
        render_label: "admitted media",
        lease: &lease,
        output: input.output,
        completes_job: true,
    })
}

fn attempt_identity(
    project_dir: &Path,
    job_id: &str,
    attempt_id: &str,
) -> Result<DurableAttemptIdentity, ServiceError> {
    let dir = attempt_directory(project_dir, job_id, attempt_id);
    // Small identity record keeps live polling independent of the immutable project snapshot.
    let record: DurableAttemptIdentity = if dir.join("identity.json").exists() {
        read_record(project_dir, &dir.join("identity.json"))?
    } else if dir.join("input.json").exists() {
        let input = retention::read_input(project_dir, &dir.join("input.json"))?;
        DurableAttemptIdentity {
            protocol: input.protocol,
            identity: input.identity,
            job_id: input.input.job_id,
            attempt_id: input.input.attempt_id,
            profile: input.input.profile,
            pin_identity: input.pin_identity,
        }
    } else {
        return Err(ServiceError::not_found("render attempt"));
    };
    if !matches!(record.protocol, 1 | 2)
        || record.job_id != job_id
        || record.attempt_id != attempt_id
    {
        return Err(ServiceError::invalid_input(
            "render attempt identity does not match its record",
        ));
    }
    record.identity.validate(project_dir)?;
    Ok(record)
}

fn failed(message: &str, interrupted: bool) -> MediaRenderAttempt {
    MediaRenderAttempt::Failed {
        message: message.into(),
        interrupted,
    }
}

fn canonical_outcome(
    project_dir: &Path,
    record: &DurableAttemptIdentity,
) -> Result<MediaRenderAttempt, ServiceError> {
    record.identity.validate(project_dir)?;
    let project = load_split_project(project_dir)
        .map_err(|error| ServiceError::internal(error.to_string()))?;
    record.identity.validate(project_dir)?;
    let Some(job) = project
        .jobs
        .iter()
        .find(|job| matching_attempt(job, &record.job_id, &record.attempt_id))
    else {
        return Ok(failed(
            "Render attempt was superseded. Start a new render.",
            false,
        ));
    };
    match job.status {
        JobStatus::Completed => {
            let paths = ProjectMediaRenderPaths::new(&record.job_id, record.profile)
                .map_err(|_| ServiceError::invalid_input("invalid render artifact paths"))?;
            let project_render_report = project
                .render_reports
                .iter()
                .find(|report| report.id == record.job_id)
                .cloned()
                .ok_or_else(|| {
                    ServiceError::internal("completed render has no canonical report")
                })?;
            let render_report: RenderReport =
                read_record(project_dir, &paths.absolute_json_report_path(project_dir))?;
            if render_report.job_id != record.job_id || render_report.summary.status != "succeeded"
            {
                return Err(ServiceError::internal(
                    "completed render report is inconsistent",
                ));
            }
            let export_artifact = project
                .export_artifacts
                .iter()
                .rev()
                .find(|artifact| artifact.job_id.as_deref() == Some(&record.job_id))
                .cloned();
            record.identity.validate(project_dir)?;
            Ok(MediaRenderAttempt::Completed {
                result: Box::new(ProjectMediaRenderResult {
                    project,
                    render_report,
                    project_render_report,
                    output_path: paths.output_path,
                    export_artifact,
                }),
            })
        }
        JobStatus::Cancelled => Ok(failed("Render was cancelled. Start a new render.", false)),
        JobStatus::Failed => Ok(failed(
            "Render stopped before completion. Inspect the render log and retry.",
            false,
        )),
        _ => Ok(failed(
            "Render stopped when the host closed. Recover the attempt before retrying.",
            true,
        )),
    }
}

pub fn read_media_render_attempt(
    project_dir: &Path,
    job_id: &str,
    attempt_id: &str,
) -> Result<MediaRenderAttempt, ServiceError> {
    let record = attempt_identity(project_dir, job_id, attempt_id)?;
    let result_path = attempt_directory(project_dir, job_id, attempt_id).join("result.json");
    if result_path.exists() {
        let stored: retention::StoredResult = read_record(project_dir, &result_path)?;
        stored.validate()?;
        // Canonical terminal state wins over an old or cancelled worker's result record.
        let _mutation = crate::project::mutation::acquire_split_project_mutation_lease(project_dir)
            .map_err(ServiceError::internal)?;
        record.identity.validate(project_dir)?;
        let project = load_split_project(project_dir)
            .map_err(|error| ServiceError::internal(error.to_string()))?;
        record.identity.validate(project_dir)?;
        let Some(job) = project
            .jobs
            .iter()
            .find(|job| matching_attempt(job, job_id, attempt_id))
        else {
            return Ok(failed(
                "Render attempt was superseded. Start a new render.",
                false,
            ));
        };
        if job.status == JobStatus::Completed {
            if matches!(stored.outcome, retention::StoredOutcome::Completed { .. }) {
                return Ok(stored.into_attempt(project));
            }
            return canonical_outcome(project_dir, &record);
        }
        if job.status == JobStatus::Cancelled {
            return Ok(failed("Render was cancelled. Start a new render.", false));
        }
        if job.status == JobStatus::Failed
            && matches!(stored.outcome, retention::StoredOutcome::Failed { .. })
        {
            return Ok(stored.into_attempt(project));
        }
        if active(&job.status) && attempt_is_pinned(project_dir, job_id, attempt_id) {
            return Ok(MediaRenderAttempt::Pending);
        }
        return canonical_outcome(project_dir, &record);
    }
    // An active pin means a live worker. No full project load, no mutation lease, no status writes.
    if attempt_is_pinned(project_dir, job_id, attempt_id) {
        return Ok(MediaRenderAttempt::Pending);
    }
    let _mutation = crate::project::mutation::acquire_split_project_mutation_lease(project_dir)
        .map_err(ServiceError::internal)?;
    canonical_outcome(project_dir, &record)
}

/// Explicitly terminalize an interrupted admitted attempt. Status polling never mutates jobs.
/// The nonblocking pin probe runs under a short mutation lease; no artifact lease is awaited.
pub fn recover_media_render_attempt(
    project_dir: &Path,
    job_id: &str,
    attempt_id: &str,
) -> Result<MediaRenderAttempt, ServiceError> {
    let record = attempt_identity(project_dir, job_id, attempt_id)?;
    let mutation = crate::project::mutation::acquire_split_project_mutation_lease(project_dir)
        .map_err(ServiceError::internal)?;
    record.identity.validate(project_dir)?;
    if attempt_is_pinned(project_dir, job_id, attempt_id) {
        return Ok(MediaRenderAttempt::Pending);
    }
    if record
        .identity
        .active_job(project_dir, job_id, attempt_id)
        .is_ok()
    {
        crate::project::split::apply_project_bookkeeping_actions_to_split_project_with_lease(
            project_dir,
            vec![ProjectAction::UpdateJobStatus {
                job_id: job_id.into(),
                status: JobStatus::Failed,
                updated_at: chrono::Utc::now().to_rfc3339(),
                run_id: Some(attempt_id.into()),
            }],
            &mutation,
        )
        .map_err(|error| ServiceError::internal(error.to_string()))?;
        let outcome = failed(
            "Render stopped when the host closed. Retry the render.",
            true,
        );
        if retention::write_result(
            project_dir,
            &attempt_directory(project_dir, job_id, attempt_id).join("result.json"),
            &outcome,
            &record.identity,
        )
        .is_ok()
        {
            let _ = retention::compact_terminal(
                project_dir,
                &record.identity,
                job_id,
                attempt_id,
                None,
                &mutation,
            );
        }
        return Ok(outcome);
    }
    let outcome = canonical_outcome(project_dir, &record)?;
    // Recovery also closes the crash window between durable terminal publication and
    // receipt compaction. Cleanup failure cannot change an already committed outcome.
    let _ = retention::compact_terminal(
        project_dir,
        &record.identity,
        job_id,
        attempt_id,
        None,
        &mutation,
    );
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    };
    use std::time::Duration;

    fn fixture() -> (tempfile::TempDir, MediaRenderInput) {
        let root = tempfile::tempdir().unwrap();
        let project = VideoProject::new_empty(
            "render-project".into(),
            "Original".into(),
            "2026-10-01".into(),
        );
        crate::project::split::save_split_project(root.path(), &project).unwrap();
        let input = MediaRenderInput {
            project_id: project.id,
            profile: ExportProfile::Webm,
            quality: RenderQuality::Draft,
            width: 1280,
            height: 720,
            job_id: "render-job".into(),
            attempt_id: "render-attempt/test".into(),
            updated_at: "2026-10-01".into(),
            range_start_seconds: None,
            range_end_seconds: None,
            timeline_id: None,
            fps: None,
            encode_tier: None,
            output: None,
            export_settings: None,
        };
        (root, input)
    }

    #[test]
    fn durable_admission_releases_gate_and_preserves_snapshot_and_exact_attempt() {
        let (root, input) = fixture();
        let revision = load_split_project(root.path()).unwrap().content_revision;
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let executions = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&executions);
        #[cfg(feature = "web-host")]
        let leases = crate::web_host::editor_lease::LeaseManager::new(30, 5);
        #[cfg(feature = "web-host")]
        let lease = leases.acquire("catalog-project", "first", 100).unwrap();
        let admission = {
            let admit = || {
                admit_with_executor(
                    root.path(),
                    revision,
                    input.clone(),
                    move |_, record, attempt| {
                        count.fetch_add(1, Ordering::SeqCst);
                        started_tx.send(record.project.name).unwrap();
                        finish_rx.recv().unwrap();
                        assert!(attempt.guard.token().is_cancelled());
                        Err(render_cancelled_error("test.cancelled"))
                    },
                )
            };
            #[cfg(feature = "web-host")]
            {
                leases
                    .with_valid_lease("catalog-project", "first", &lease.token, 101, admit)
                    .unwrap()
                    .unwrap()
            }
            #[cfg(not(feature = "web-host"))]
            {
                admit().unwrap()
            }
        };
        assert_eq!(
            started_rx.recv_timeout(Duration::from_secs(30)).unwrap(),
            "Original"
        );
        assert!(root.path().join("logs/render-admissions").is_dir());
        assert_eq!(admission.source_revision, revision);
        assert_eq!(admission.project.jobs[0].status, JobStatus::Queued);
        assert!(attempt_is_pinned(
            root.path(),
            &input.job_id,
            &input.attempt_id
        ));
        assert!(!render_job_needs_recovery(
            root.path(),
            &input.project_id,
            &admission.project.jobs[0]
        ));
        #[cfg(feature = "web-host")]
        {
            // Simulate a job lasting more than the 30-second lease duration. Renewal,
            // editing and takeover each finish while execution remains blocked.
            leases
                .renew("catalog-project", "first", &lease.token, 120)
                .unwrap();
            leases
                .renew("catalog-project", "first", &lease.token, 140)
                .unwrap();
            leases
                .with_valid_lease("catalog-project", "first", &lease.token, 141, || {
                    apply_project_actions_to_split_project(
                        root.path(),
                        vec![ProjectAction::UpdateProjectSettings {
                            name: "Edited during render".into(),
                            render_settings: admission.project.render_settings.clone(),
                        }],
                    )
                    .unwrap();
                })
                .unwrap();
            leases.takeover("catalog-project", "second", 142).unwrap();
        }
        assert_eq!(
            read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
            MediaRenderAttempt::Pending
        );
        let replay = admit_with_executor(root.path(), revision, input.clone(), |_, _, _| {
            panic!("completed admission must not run twice")
        })
        .unwrap();
        assert_eq!(replay.attempt_id, input.attempt_id);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            crate::render_pipeline::cancel::request_render_cancellation_by_locator(
                root.path(),
                &input.job_id,
                &input.attempt_id
            ),
            crate::render_pipeline::cancel::RenderCancellationOutcome::Requested
        );
        finish_tx.send(()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap()
            == MediaRenderAttempt::Pending
        {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        #[cfg(feature = "web-host")]
        assert_eq!(
            load_split_project(root.path()).unwrap().name,
            "Edited during render"
        );
    }

    #[test]
    fn admission_rejects_stale_revision_without_job_or_worker() {
        let (root, input) = fixture();
        let project = load_split_project(root.path()).unwrap();
        let error = admit_with_executor(
            root.path(),
            project.content_revision + 1,
            input,
            |_, _, _| panic!("stale job cannot execute"),
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            crate::app_service::error::ServiceErrorCode::RevisionConflict
        );
        let after = load_split_project(root.path()).unwrap();
        assert_eq!(after.content_revision, project.content_revision);
        assert!(after.jobs.is_empty());
    }

    #[test]
    fn abandoned_durable_input_is_terminal_and_never_automatically_restarted() {
        let (root, input) = fixture();
        let project = load_split_project(root.path()).unwrap();
        let dir = attempt_directory(root.path(), &input.job_id, &input.attempt_id);
        write_record(
            root.path(),
            &dir.join("input.json"),
            &DurableInput {
                identity: PackageIdentity::capture(root.path(), &project.id).unwrap(),
                protocol: 1,
                source_revision: project.content_revision,
                input: input.clone(),
                project,
            },
        )
        .unwrap();
        assert!(matches!(
            read_media_render_attempt(root.path(), &input.job_id, &input.attempt_id).unwrap(),
            MediaRenderAttempt::Failed { .. }
        ));
        assert!(admit_with_executor(root.path(), 0, input, |_, _, _| panic!(
            "orphan input must not execute"
        ))
        .is_err());
    }
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod review_tests;

#[cfg(test)]
#[path = "admission_capacity_tests.rs"]
mod capacity_tests;

#[cfg(test)]
#[path = "admission_retention_tests.rs"]
mod retention_tests;
