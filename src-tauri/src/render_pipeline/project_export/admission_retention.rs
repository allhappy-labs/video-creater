//! Serialized-byte admission budgets and terminal receipts. These limits describe
//! retained JSON payloads, not allocator overhead, RSS, render media or export bytes.
use super::*;
use std::io;

pub(super) const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_IN_FLIGHT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ATTEMPTS: usize = 1_024;
const MAX_RESULT_BYTES: u64 = 64 * 1024 * 1024;

struct ByteCounter {
    bytes: u64,
    maximum: u64,
    oversized: bool,
}

impl io::Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self.bytes.saturating_add(bytes.len() as u64);
        if next > self.maximum {
            self.oversized = true;
            return Err(io::Error::other(
                "serialized render metadata budget exceeded",
            ));
        }
        self.bytes = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn serialized_bytes(value: &impl Serialize, maximum: u64) -> Result<u64, ServiceError> {
    let mut counter = ByteCounter {
        bytes: 0,
        maximum,
        oversized: false,
    };
    let result = serde_json::to_writer(&mut counter, value);
    if counter.oversized {
        return Err(ServiceError::busy("render metadata byte capacity"));
    }
    result.map_err(|error| ServiceError::internal(error.to_string()))?;
    Ok(counter.bytes)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InputReceipt {
    pub(super) identity: PackageIdentity,
    pub(super) protocol: u32,
    pub(super) source_revision: u64,
    pub(super) input: MediaRenderInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) pin_identity: Option<PinIdentity>,
}

fn fingerprint(input: &MediaRenderInput, source_revision: u64) -> Result<String, ServiceError> {
    // The receipt retains the original input as well as its digest. Equality checks
    // preserve the existing replay contract; the digest also validates receipt integrity.
    let mut digest = Sha256::new();
    digest.update(source_revision.to_le_bytes());
    digest.update(
        serde_json::to_vec(input).map_err(|error| ServiceError::internal(error.to_string()))?,
    );
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) fn read_input(project_dir: &Path, path: &Path) -> Result<InputReceipt, ServiceError> {
    // Serde skips the legacy project's fields instead of allocating a second full snapshot.
    let receipt: InputReceipt = read_record(project_dir, path)?;
    if receipt.protocol != 1 && receipt.protocol != 2 {
        return Err(ServiceError::invalid_input(
            "unsupported render admission record",
        ));
    }
    if receipt.protocol == 2
        && receipt.input_fingerprint.as_deref()
            != Some(fingerprint(&receipt.input, receipt.source_revision)?.as_str())
    {
        return Err(ServiceError::invalid_input(
            "render admission receipt fingerprint is invalid",
        ));
    }
    Ok(receipt)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StoredResult {
    #[serde(default = "legacy_protocol")]
    protocol: u32,
    #[serde(flatten)]
    pub(super) outcome: StoredOutcome,
}
fn legacy_protocol() -> u32 {
    1
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(super) enum StoredOutcome {
    Pending,
    Completed {
        result: Box<CompactResult>,
    },
    Failed {
        message: String,
        #[serde(default)]
        interrupted: bool,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompactResult {
    render_report: RenderReport,
    project_render_report: ProjectRenderReport,
    output_path: String,
    export_artifact: Option<ProjectExportArtifact>,
}

impl StoredResult {
    pub(super) fn validate(&self) -> Result<(), ServiceError> {
        if self.protocol != 1 && self.protocol != 2 {
            return Err(ServiceError::invalid_input(
                "unsupported render result record",
            ));
        }
        Ok(())
    }
    pub(super) fn into_attempt(self, project: VideoProject) -> MediaRenderAttempt {
        match self.outcome {
            StoredOutcome::Pending => MediaRenderAttempt::Pending,
            StoredOutcome::Failed {
                message,
                interrupted,
            } => MediaRenderAttempt::Failed {
                message,
                interrupted,
            },
            StoredOutcome::Completed { result } => MediaRenderAttempt::Completed {
                result: Box::new(ProjectMediaRenderResult {
                    project,
                    render_report: result.render_report,
                    project_render_report: result.project_render_report,
                    output_path: result.output_path,
                    export_artifact: result.export_artifact,
                }),
            },
        }
    }
}

// Serialize through borrowed result fields; no extra clone of the project, log or report.
#[derive(Serialize)]
struct ResultReference<'a> {
    protocol: u32,
    #[serde(flatten)]
    outcome: OutcomeReference<'a>,
}
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum OutcomeReference<'a> {
    Pending,
    Completed { result: CompactResultReference<'a> },
    Failed { message: &'a str, interrupted: bool },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompactResultReference<'a> {
    render_report: &'a RenderReport,
    project_render_report: &'a ProjectRenderReport,
    output_path: &'a str,
    export_artifact: &'a Option<ProjectExportArtifact>,
}

pub(super) fn write_result(
    project_dir: &Path,
    path: &Path,
    result: &MediaRenderAttempt,
    identity: &PackageIdentity,
) -> Result<(), ServiceError> {
    let outcome = match result {
        MediaRenderAttempt::Pending => OutcomeReference::Pending,
        MediaRenderAttempt::Failed {
            message,
            interrupted,
        } => OutcomeReference::Failed {
            message,
            interrupted: *interrupted,
        },
        MediaRenderAttempt::Completed { result } => OutcomeReference::Completed {
            result: CompactResultReference {
                render_report: &result.render_report,
                project_render_report: &result.project_render_report,
                output_path: &result.output_path,
                export_artifact: &result.export_artifact,
            },
        },
    };
    let value = ResultReference {
        protocol: 2,
        outcome,
    };
    serialized_bytes(&value, MAX_RESULT_BYTES)?;
    write_record_with_identity(project_dir, path, &value, Some(identity))
}

#[derive(Default)]
struct PackageUsage {
    bytes: u64,
    attempts: usize,
}

fn add_bytes(usage: &mut PackageUsage, bytes: u64) -> Result<(), ServiceError> {
    usage.bytes = usage.bytes.saturating_add(bytes);
    if usage.bytes > MAX_PACKAGE_BYTES {
        return Err(ServiceError::busy("render history byte capacity"));
    }
    Ok(())
}

#[derive(Default)]
struct ChargedDirectory {
    input: bool,
    terminal_result: bool,
}

fn directory_bytes(
    path: &Path,
    usage: &mut PackageUsage,
) -> Result<ChargedDirectory, ServiceError> {
    let opened = open_directory(path)?;
    let mut charged = ChargedDirectory::default();
    for item in std::fs::read_dir(path).map_err(io_error)? {
        let item = item.map_err(io_error)?;
        #[cfg(unix)]
        let file = {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::CString::new(item.file_name().as_bytes())
                .map_err(|_| ServiceError::forbidden())?;
            let fd = unsafe {
                libc::openat(
                    opened.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                )
            };
            if fd < 0 {
                return Err(io_error(io::Error::last_os_error()));
            }
            unsafe { File::from_raw_fd(fd) }
        };
        #[cfg(not(unix))]
        let file = OpenOptions::new()
            .read(true)
            .open(item.path())
            .map_err(io_error)?;
        let metadata = file.metadata().map_err(io_error)?;
        if !metadata.is_file() {
            return Err(ServiceError::forbidden());
        }
        add_bytes(usage, metadata.len())?;
        if item.file_name() == "input.json" {
            charged.input = true;
        }
        if item.file_name() == "result.json" && metadata.len() <= MAX_RESULT_BYTES {
            // Publication atomically replaces this leaf. Parse the same opened inode whose
            // size was charged, never a later pathname that might hold a larger result.
            use std::io::Read;
            if let Ok(result) =
                serde_json::from_reader::<_, StoredResult>(file.take(MAX_RESULT_BYTES + 1))
            {
                charged.terminal_result =
                    result.validate().is_ok() && !matches!(result.outcome, StoredOutcome::Pending);
            }
        }
    }
    #[cfg(unix)]
    {
        let actual = open_directory(path)?.metadata().map_err(io_error)?;
        let expected = opened.metadata().map_err(io_error)?;
        if (actual.dev(), actual.ino()) != (expected.dev(), expected.ino()) {
            return Err(ServiceError::forbidden());
        }
    }
    Ok(charged)
}

fn package_usage(project_dir: &Path) -> Result<PackageUsage, ServiceError> {
    let mut usage = PackageUsage::default();
    let root = project_dir.join("logs/render-admissions");
    match std::fs::symlink_metadata(&root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(usage),
        Err(error) => return Err(io_error(error)),
        Ok(metadata) if !metadata.is_dir() => return Err(ServiceError::forbidden()),
        Ok(_) => {}
    }
    // Safe ancestor opening protects the accounting directory just like record IO.
    let parent = RecordParent::open(project_dir, &root.join(".budget"), false)?;
    for item in std::fs::read_dir(&root).map_err(io_error)? {
        let item = item.map_err(io_error)?;
        let metadata = std::fs::symlink_metadata(item.path()).map_err(io_error)?;
        if metadata.is_dir() {
            usage.attempts += 1;
            if usage.attempts > MAX_ATTEMPTS {
                return Err(ServiceError::busy("render history entry capacity"));
            }
            let charged = directory_bytes(&item.path(), &mut usage)?;
            #[cfg(test)]
            super::retention_tests::after_directory_charge(&item.path());
            // Only a terminal result actually charged by this scan releases the future
            // allowance. Absent, Pending and unreadable results retain it after restart.
            if charged.input && !charged.terminal_result {
                add_bytes(&mut usage, MAX_RESULT_BYTES)?;
            }
        } else if metadata.is_file() {
            add_bytes(&mut usage, metadata.len())?;
        } else {
            return Err(ServiceError::forbidden());
        }
    }
    parent.validate()?;
    Ok(usage)
}

pub(super) fn check_package_capacity(
    project_dir: &Path,
    record: &DurableInput,
    bytes: u64,
) -> Result<(), ServiceError> {
    let mut usage = package_usage(project_dir)?;
    if usage.attempts >= MAX_ATTEMPTS {
        return Err(ServiceError::busy("render history entry capacity"));
    }
    let identity = DurableAttemptIdentity {
        protocol: 1,
        identity: record.identity.clone(),
        job_id: record.input.job_id.clone(),
        attempt_id: record.input.attempt_id.clone(),
        profile: record.input.profile,
        pin_identity: Some(PinIdentity {
            device: u64::MAX,
            inode: u64::MAX,
        }),
    };
    add_bytes(&mut usage, bytes)?;
    add_bytes(&mut usage, serialized_bytes(&identity, MAX_INPUT_BYTES)?)?;
    add_bytes(&mut usage, MAX_RESULT_BYTES)
}

/// Caller holds mutation ownership. An executor's still-held pin proves exclusive
/// ownership of its finished work; maintenance/recovery must acquire the pin without waiting.
pub(super) fn compact_terminal(
    project_dir: &Path,
    expected_identity: &PackageIdentity,
    job_id: &str,
    attempt_id: &str,
    own_pin: Option<&AdmissionPin>,
    _mutation: &SplitProjectMutationLease,
) -> Result<bool, ServiceError> {
    expected_identity.validate(project_dir)?;
    let dir = attempt_directory(project_dir, job_id, attempt_id);
    let result_path = dir.join("result.json");
    if !result_path.exists() {
        return Ok(false);
    }
    let result: StoredResult = read_record(project_dir, &result_path)?;
    result.validate()?;
    if matches!(result.outcome, StoredOutcome::Pending) {
        return Ok(false);
    }
    let mut input = read_input(project_dir, &dir.join("input.json"))?;
    if &input.identity != expected_identity {
        return Err(ServiceError::forbidden());
    }
    input.identity.validate(project_dir)?;
    let project = load_split_project(project_dir)
        .map_err(|error| ServiceError::internal(error.to_string()))?;
    input.identity.validate(project_dir)?;
    if !project
        .jobs
        .iter()
        .any(|job| matching_attempt(job, job_id, attempt_id) && !active(&job.status))
    {
        return Ok(false);
    }
    let pin_identity = attempt_identity(project_dir, job_id, attempt_id)?.pin_identity;
    let acquired_pin;
    let pin = if let Some(pin) = own_pin {
        pin
    } else {
        // This method is called under mutation ownership, so another admission cannot
        // open/recreate this lock file between the probe and owned unlink.
        acquired_pin = AdmissionPin::acquire(project_dir, job_id, attempt_id)?;
        &acquired_pin
    };
    if input.protocol == 1 {
        input.protocol = 2;
        input.pin_identity = pin_identity.clone();
        input.input_fingerprint = Some(fingerprint(&input.input, input.source_revision)?);
        serialized_bytes(&input, MAX_INPUT_BYTES)?;
        // The writer atomically replaces input.json. A crash leaves the legacy snapshot
        // or the complete receipt, and result.json was durably published first.
        write_record_with_identity(
            project_dir,
            &dir.join("input.json"),
            &input,
            Some(&input.identity),
        )?;
    }
    if let Some(expected) = pin_identity {
        if own_pin.is_some() {
            if pin.identity()? != expected {
                return Err(ServiceError::forbidden());
            }
            // The worker releases its byte/thread permit before unlinking its own pin.
            // A missing/unlocked pin has always promised that this reservation is gone.
            return Ok(true);
        }
        pin.remove_if_owned(project_dir, &input.identity, &expected)?;
    }
    Ok(false)
}
