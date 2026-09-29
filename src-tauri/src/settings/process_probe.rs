use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROCESS_CLEANUP_GRACE: Duration = Duration::from_millis(250);
const PROCESS_IO_BYTES_PER_POLL: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct ProcessProbeRequest {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub stdin_lines: Vec<String>,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub terminate_after_response_id: Option<u64>,
    /// How the probe reads the child's stdout. JSON-RPC stdio servers write one value per
    /// line; a CLI that reports a status pretty-prints one value across many lines.
    pub stdout_format: ProcessProbeStdoutFormat,
}

/// The shape of a probe's stdout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProcessProbeStdoutFormat {
    /// One JSON value per non-empty line.
    #[default]
    JsonLines,
    /// The whole stdout is a single JSON value.
    JsonDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessProbeExitState {
    pub code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProcessProbeTerminationState {
    NotRequired,
    Confirmed,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessProbeCleanup {
    pub termination: ProcessProbeTerminationState,
    pub child_reaped: bool,
    /// True on Unix, where the probe starts a dedicated process group and can
    /// address descendants. False on platforms where cleanup covers only the
    /// direct child.
    pub descendant_termination_supported: bool,
    /// True only when the operating-system mechanism prevents descendants from
    /// escaping cleanup. Unix process groups are addressable but not
    /// containment, because a descendant can create a new session/process
    /// group.
    pub descendant_containment_guaranteed: bool,
    pub steps: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessProbeResult {
    pub exit_state: ProcessProbeExitState,
    pub response_lines: Vec<Value>,
    pub stderr: String,
    pub stderr_truncated: bool,
    pub elapsed_millis: u64,
    /// True only when required cleanup was confirmed within the platform scope
    /// described by `cleanup.descendant_termination_supported`.
    pub terminated: bool,
    pub cleanup: ProcessProbeCleanup,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ProcessProbeError {
    #[error("process probe request field {field} is invalid: {detail}")]
    InvalidRequest { field: &'static str, detail: String },
    #[error("process probes are unsupported on this platform")]
    UnsupportedPlatform,
    #[error("process probe could not launch {}: {detail}", program.display())]
    LaunchFailed { program: PathBuf, detail: String },
    #[error("process probe stdin write failed: {detail}")]
    StdinWriteFailed {
        detail: String,
        cleanup: Box<ProcessProbeCleanup>,
    },
    #[error("process probe status check failed: {detail}")]
    StatusFailed {
        detail: String,
        cleanup: Box<ProcessProbeCleanup>,
    },
    #[error("process probe {stream} capture failed: {detail}")]
    OutputReadFailed {
        stream: &'static str,
        detail: String,
    },
    #[error("process probe timed out after {timeout_millis} ms")]
    Timeout {
        timeout_millis: u64,
        elapsed_millis: u64,
        stderr: String,
        stderr_truncated: bool,
        terminated: bool,
        cleanup: Box<ProcessProbeCleanup>,
        capture_errors: Vec<String>,
    },
    #[error("process probe exited, but stdio capture did not finish before cleanup deadline")]
    CaptureTimeout {
        exit_state: ProcessProbeExitState,
        elapsed_millis: u64,
        pending_streams: Vec<String>,
        stderr: String,
        stderr_truncated: bool,
        terminated: bool,
        cleanup: Box<ProcessProbeCleanup>,
        capture_errors: Vec<String>,
    },
    #[error("process probe returned malformed JSON on line {line_number}: {detail}")]
    MalformedJson {
        line_number: usize,
        line: String,
        detail: String,
        exit_state: ProcessProbeExitState,
        stderr: String,
        stderr_truncated: bool,
        elapsed_millis: u64,
        terminated: bool,
        cleanup: Box<ProcessProbeCleanup>,
    },
    #[error("process probe stdout exceeded its {max_bytes}-byte limit")]
    StdoutLimitExceeded {
        max_bytes: usize,
        elapsed_millis: u64,
        stderr: String,
        stderr_truncated: bool,
        terminated: bool,
        cleanup: Box<ProcessProbeCleanup>,
    },
}

pub fn run_process_probe(
    request: ProcessProbeRequest,
) -> Result<ProcessProbeResult, ProcessProbeError> {
    #[cfg(not(unix))]
    {
        let _ = request;
        return Err(ProcessProbeError::UnsupportedPlatform);
    }
    #[cfg(unix)]
    run_process_probe_unix(request)
}

#[cfg(unix)]
fn run_process_probe_unix(
    request: ProcessProbeRequest,
) -> Result<ProcessProbeResult, ProcessProbeError> {
    validate_request(&request)?;

    let mut command = Command::new(&request.program);
    command
        .args(&request.args)
        .stdin(if request.stdin_lines.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    use std::os::unix::process::CommandExt;
    command.process_group(0);

    let child = command
        .spawn()
        .map_err(|error| ProcessProbeError::LaunchFailed {
            program: request.program.clone(),
            detail: error.to_string(),
        })?;
    let mut child = ChildProcessGroupGuard::new(child);
    let stdout =
        child
            .child_mut()
            .stdout
            .take()
            .ok_or_else(|| ProcessProbeError::OutputReadFailed {
                stream: "stdout",
                detail: "stdout pipe was unavailable".to_string(),
            })?;
    let stderr =
        child
            .child_mut()
            .stderr
            .take()
            .ok_or_else(|| ProcessProbeError::OutputReadFailed {
                stream: "stderr",
                detail: "stderr pipe was unavailable".to_string(),
            })?;
    let mut io = ProbeIo::new(
        stdout,
        stderr,
        child.child_mut().stdin.take(),
        request.stdin_lines,
        request.max_stdout_bytes,
        request.max_stderr_bytes,
        request.terminate_after_response_id.is_some(),
    )
    .map_err(|detail| ProcessProbeError::StdinWriteFailed {
        detail,
        cleanup: Box::new(ProcessProbeCleanup::partial_without_child(
            "stdio setup failed before cleanup",
        )),
    })?;
    let started = Instant::now();
    let probe_deadline = started + request.timeout;
    let cleanup_deadline = probe_deadline + PROCESS_CLEANUP_GRACE;

    loop {
        io.poll();
        if let Some(Err(detail)) = io.stdin_result() {
            let mut cleanup = child.cleanup(false, cleanup_deadline);
            poll_io_until(&mut io, cleanup_deadline);
            let io_state = io.into_state();
            downgrade_cleanup_for_incomplete_output(&mut cleanup, &io_state);
            return Err(ProcessProbeError::StdinWriteFailed {
                detail,
                cleanup: Box::new(cleanup),
            });
        }

        if request
            .terminate_after_response_id
            .is_some_and(|response_id| io.has_response_id(response_id))
        {
            let mut cleanup = child.cleanup(false, cleanup_deadline);
            poll_io_until(&mut io, cleanup_deadline);
            let io_state = io.into_state();
            downgrade_cleanup_for_incomplete_output(&mut cleanup, &io_state);
            return finalize_probe(
                ProcessProbeExitState {
                    code: None,
                    success: true,
                },
                io_state,
                started.elapsed(),
                cleanup,
                request.stdout_format,
            );
        }

        match child.child_mut().try_wait() {
            Ok(Some(status)) => {
                let mut cleanup = child.cleanup(true, cleanup_deadline);
                poll_io_until(&mut io, cleanup_deadline);
                let io_state = io.into_state();
                downgrade_cleanup_for_incomplete_output(&mut cleanup, &io_state);
                return finalize_completed_probe(
                    status,
                    io_state,
                    started.elapsed(),
                    cleanup,
                    request.stdout_format,
                );
            }
            Ok(None) => {}
            Err(error) => {
                let mut cleanup = child.cleanup(false, cleanup_deadline);
                poll_io_until(&mut io, cleanup_deadline);
                let io_state = io.into_state();
                downgrade_cleanup_for_incomplete_output(&mut cleanup, &io_state);
                return Err(ProcessProbeError::StatusFailed {
                    detail: error.to_string(),
                    cleanup: Box::new(cleanup),
                });
            }
        }

        if Instant::now() >= probe_deadline {
            let mut cleanup = child.cleanup(false, cleanup_deadline);
            poll_io_until(&mut io, cleanup_deadline);
            let io_state = io.into_state();
            downgrade_cleanup_for_incomplete_output(&mut cleanup, &io_state);
            return Err(ProcessProbeError::Timeout {
                timeout_millis: duration_millis(request.timeout),
                elapsed_millis: duration_millis(started.elapsed()),
                stderr: io_state.stderr.text(),
                stderr_truncated: io_state.stderr.truncated,
                terminated: cleanup.termination == ProcessProbeTerminationState::Confirmed,
                capture_errors: io_state.capture_errors(),
                cleanup: Box::new(cleanup),
            });
        }
        thread::sleep(
            PROCESS_POLL_INTERVAL.min(probe_deadline.saturating_duration_since(Instant::now())),
        );
    }
}

impl ProcessProbeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest { .. } => "processProbe.invalidRequest",
            Self::UnsupportedPlatform => "processProbe.unsupportedPlatform",
            Self::LaunchFailed { .. } => "processProbe.launchFailed",
            Self::StdinWriteFailed { .. } => "processProbe.stdinWriteFailed",
            Self::StatusFailed { .. } => "processProbe.statusFailed",
            Self::OutputReadFailed { .. } => "processProbe.outputReadFailed",
            Self::Timeout { .. } => "processProbe.timeout",
            Self::CaptureTimeout { .. } => "processProbe.captureTimeout",
            Self::MalformedJson { .. } => "processProbe.malformedJson",
            Self::StdoutLimitExceeded { .. } => "processProbe.stdoutLimitExceeded",
        }
    }
}

fn validate_request(request: &ProcessProbeRequest) -> Result<(), ProcessProbeError> {
    if request.timeout.is_zero() {
        return Err(ProcessProbeError::InvalidRequest {
            field: "timeout",
            detail: "timeout must be greater than zero".to_string(),
        });
    }
    if request.max_stdout_bytes == 0 {
        return Err(ProcessProbeError::InvalidRequest {
            field: "maxStdoutBytes",
            detail: "stdout limit must be greater than zero".to_string(),
        });
    }
    Ok(())
}

/// Why a probe's stdout could not be read as JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MalformedProbeStdout {
    line_number: usize,
    line: String,
    detail: String,
}

/// Reads a probe's stdout in the requested shape. Pure, so both shapes are unit tested
/// without spawning a process.
fn parse_probe_stdout(
    stdout: &str,
    format: ProcessProbeStdoutFormat,
) -> Result<Vec<Value>, MalformedProbeStdout> {
    match format {
        ProcessProbeStdoutFormat::JsonLines => {
            let mut responses = Vec::new();
            for (index, line) in stdout.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                responses.push(serde_json::from_str(line).map_err(|error| {
                    MalformedProbeStdout {
                        line_number: index + 1,
                        line: line.to_string(),
                        detail: error.to_string(),
                    }
                })?);
            }
            Ok(responses)
        }
        ProcessProbeStdoutFormat::JsonDocument => {
            let document = stdout.trim();
            if document.is_empty() {
                return Ok(Vec::new());
            }
            // A whole document has no offending line, so the first line names it.
            let response =
                serde_json::from_str(document).map_err(|error| MalformedProbeStdout {
                    line_number: 1,
                    line: document.to_string(),
                    detail: error.to_string(),
                })?;
            Ok(vec![response])
        }
    }
}

fn finalize_completed_probe(
    status: ExitStatus,
    io_state: IoState,
    elapsed: Duration,
    cleanup: ProcessProbeCleanup,
    stdout_format: ProcessProbeStdoutFormat,
) -> Result<ProcessProbeResult, ProcessProbeError> {
    let exit_state = ProcessProbeExitState {
        code: status.code(),
        success: status.success(),
    };
    finalize_probe(exit_state, io_state, elapsed, cleanup, stdout_format)
}

fn finalize_probe(
    exit_state: ProcessProbeExitState,
    io_state: IoState,
    elapsed: Duration,
    cleanup: ProcessProbeCleanup,
    stdout_format: ProcessProbeStdoutFormat,
) -> Result<ProcessProbeResult, ProcessProbeError> {
    let elapsed_millis = duration_millis(elapsed);
    let stderr_text = io_state.stderr.text();
    let terminated = cleanup.termination == ProcessProbeTerminationState::Confirmed;

    if !io_state.is_complete() {
        return Err(ProcessProbeError::CaptureTimeout {
            exit_state,
            elapsed_millis,
            pending_streams: io_state.pending_streams(),
            stderr: stderr_text,
            stderr_truncated: io_state.stderr.truncated,
            terminated,
            capture_errors: io_state.capture_errors(),
            cleanup: Box::new(cleanup),
        });
    }
    if let Some(detail) = io_state.stdout.read_error.clone() {
        return Err(ProcessProbeError::OutputReadFailed {
            stream: "stdout",
            detail,
        });
    }
    if let Some(detail) = io_state.stderr.read_error.clone() {
        return Err(ProcessProbeError::OutputReadFailed {
            stream: "stderr",
            detail,
        });
    }
    if io_state.stdout.truncated {
        return Err(ProcessProbeError::StdoutLimitExceeded {
            max_bytes: io_state.stdout.limit,
            elapsed_millis,
            stderr: stderr_text,
            stderr_truncated: io_state.stderr.truncated,
            terminated,
            cleanup: Box::new(cleanup),
        });
    }

    let stdout_text = io_state.stdout.text();
    let response_lines = match parse_probe_stdout(&stdout_text, stdout_format) {
        Ok(responses) => responses,
        Err(MalformedProbeStdout {
            line_number,
            line,
            detail,
        }) => {
            return Err(ProcessProbeError::MalformedJson {
                line_number,
                line,
                detail,
                exit_state,
                stderr: stderr_text,
                stderr_truncated: io_state.stderr.truncated,
                elapsed_millis,
                terminated,
                cleanup: Box::new(cleanup),
            });
        }
    };

    Ok(ProcessProbeResult {
        exit_state,
        response_lines,
        stderr: stderr_text,
        stderr_truncated: io_state.stderr.truncated,
        elapsed_millis,
        terminated,
        cleanup,
    })
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

#[derive(Debug, Clone)]
struct BoundedCapture {
    bytes: Vec<u8>,
    limit: usize,
    truncated: bool,
    read_error: Option<String>,
    complete: bool,
}

impl BoundedCapture {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

#[derive(Debug, Clone, Copy)]
enum CaptureEnd {
    Head,
    Tail,
}

#[derive(Debug)]
struct IoState {
    stdout: BoundedCapture,
    stderr: BoundedCapture,
    stdin: Option<Result<(), String>>,
}

impl IoState {
    #[cfg(test)]
    fn new(stdout_limit: usize, stderr_limit: usize) -> Self {
        Self {
            stdout: BoundedCapture::empty(stdout_limit),
            stderr: BoundedCapture::empty(stderr_limit),
            stdin: None,
        }
    }

    fn is_complete(&self) -> bool {
        self.stdout.complete && self.stderr.complete && self.stdin.is_some()
    }

    fn pending_streams(&self) -> Vec<String> {
        let mut pending = Vec::new();
        if !self.stdout.complete {
            pending.push("stdout".to_string());
        }
        if !self.stderr.complete {
            pending.push("stderr".to_string());
        }
        if self.stdin.is_none() {
            pending.push("stdin".to_string());
        }
        pending
    }

    fn capture_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(error) = &self.stdout.read_error {
            errors.push(format!("stdout: {error}"));
        }
        if let Some(error) = &self.stderr.read_error {
            errors.push(format!("stderr: {error}"));
        }
        errors
    }
}

impl BoundedCapture {
    fn empty(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8 * 1024)),
            limit,
            truncated: false,
            read_error: None,
            complete: false,
        }
    }
}

#[cfg(unix)]
struct NonblockingCapture<R> {
    reader: Option<R>,
    capture: BoundedCapture,
    capture_end: CaptureEnd,
}

#[cfg(unix)]
impl<R> NonblockingCapture<R>
where
    R: Read + std::os::fd::AsRawFd,
{
    fn new(reader: R, limit: usize, capture_end: CaptureEnd) -> Result<Self, String> {
        set_nonblocking(&reader)?;
        Ok(Self {
            reader: Some(reader),
            capture: BoundedCapture::empty(limit),
            capture_end,
        })
    }

    fn poll(&mut self) {
        let Some(reader) = self.reader.as_mut() else {
            return;
        };
        let mut buffer = [0_u8; 8 * 1024];
        let mut remaining_budget = PROCESS_IO_BYTES_PER_POLL;
        while remaining_budget > 0 {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    self.capture.complete = true;
                    self.reader.take();
                    return;
                }
                Ok(count) => {
                    append_bounded(&mut self.capture, &buffer[..count], self.capture_end);
                    remaining_budget = remaining_budget.saturating_sub(count);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
                Err(error) => {
                    self.capture.read_error = Some(error.to_string());
                    self.capture.complete = true;
                    self.reader.take();
                    return;
                }
            }
        }
    }
}

fn append_bounded(capture: &mut BoundedCapture, bytes: &[u8], capture_end: CaptureEnd) {
    match capture_end {
        CaptureEnd::Head => {
            let remaining = capture.limit.saturating_sub(capture.bytes.len());
            capture
                .bytes
                .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
            capture.truncated |= bytes.len() > remaining;
        }
        CaptureEnd::Tail => {
            if capture.limit == 0 {
                capture.truncated = true;
                return;
            }
            if bytes.len() >= capture.limit {
                capture.bytes.clear();
                capture
                    .bytes
                    .extend_from_slice(&bytes[bytes.len() - capture.limit..]);
                capture.truncated = true;
            } else {
                let overflow = capture
                    .bytes
                    .len()
                    .saturating_add(bytes.len())
                    .saturating_sub(capture.limit);
                if overflow > 0 {
                    capture.bytes.drain(..overflow);
                    capture.truncated = true;
                }
                capture.bytes.extend_from_slice(bytes);
            }
        }
    }
}

#[cfg(unix)]
struct NonblockingStdin<W = ChildStdin> {
    writer: Option<W>,
    bytes: Vec<u8>,
    offset: usize,
    result: Option<Result<(), String>>,
    keep_open_after_write: bool,
}

#[cfg(unix)]
impl NonblockingStdin<ChildStdin> {
    fn new(
        writer: Option<ChildStdin>,
        lines: Vec<String>,
        keep_open_after_write: bool,
    ) -> Result<Self, String> {
        if lines.is_empty() {
            return Ok(Self {
                writer: None,
                bytes: Vec::new(),
                offset: 0,
                result: Some(Ok(())),
                keep_open_after_write: false,
            });
        }
        let writer = writer.ok_or_else(|| "stdin pipe was unavailable".to_string())?;
        Self::from_writer_with_policy(writer, lines, keep_open_after_write)
    }
}

#[cfg(unix)]
impl<W> NonblockingStdin<W>
where
    W: Write + std::os::fd::AsRawFd,
{
    #[cfg(test)]
    fn from_writer(writer: W, lines: Vec<String>) -> Result<Self, String> {
        Self::from_writer_with_policy(writer, lines, false)
    }

    fn from_writer_with_policy(
        writer: W,
        lines: Vec<String>,
        keep_open_after_write: bool,
    ) -> Result<Self, String> {
        set_nonblocking(&writer)?;
        let mut bytes = Vec::new();
        for line in lines {
            bytes.extend_from_slice(line.as_bytes());
            bytes.push(b'\n');
        }
        Ok(Self {
            writer: Some(writer),
            bytes,
            offset: 0,
            result: None,
            keep_open_after_write,
        })
    }

    fn poll(&mut self) {
        let Some(writer) = self.writer.as_mut() else {
            return;
        };
        let mut remaining_budget = PROCESS_IO_BYTES_PER_POLL;
        while self.offset < self.bytes.len() && remaining_budget > 0 {
            let write_end = self
                .offset
                .saturating_add(remaining_budget)
                .min(self.bytes.len());
            match writer.write(&self.bytes[self.offset..write_end]) {
                Ok(0) => {
                    self.result = Some(Err("stdin pipe closed before input completed".to_string()));
                    self.writer.take();
                    return;
                }
                Ok(count) => {
                    self.offset += count;
                    remaining_budget = remaining_budget.saturating_sub(count);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
                Err(error) => {
                    self.result = Some(Err(error.to_string()));
                    self.writer.take();
                    return;
                }
            }
        }
        if self.offset < self.bytes.len() {
            return;
        }
        self.result = Some(Ok(()));
        if !self.keep_open_after_write {
            self.writer.take();
        }
    }
}

#[cfg(unix)]
struct ProbeIo {
    stdout: NonblockingCapture<ChildStdout>,
    stderr: NonblockingCapture<ChildStderr>,
    stdin: NonblockingStdin,
}

#[cfg(unix)]
impl ProbeIo {
    fn new(
        stdout: ChildStdout,
        stderr: ChildStderr,
        stdin: Option<ChildStdin>,
        stdin_lines: Vec<String>,
        stdout_limit: usize,
        stderr_limit: usize,
        keep_stdin_open_after_write: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            stdout: NonblockingCapture::new(stdout, stdout_limit, CaptureEnd::Head)?,
            stderr: NonblockingCapture::new(stderr, stderr_limit, CaptureEnd::Tail)?,
            stdin: NonblockingStdin::new(stdin, stdin_lines, keep_stdin_open_after_write)?,
        })
    }

    fn poll(&mut self) {
        self.stdout.poll();
        self.stderr.poll();
        self.stdin.poll();
    }

    fn stdin_result(&self) -> Option<Result<(), String>> {
        self.stdin.result.clone()
    }

    fn has_response_id(&self, response_id: u64) -> bool {
        self.stdout.capture.text().lines().any(|line| {
            serde_json::from_str::<Value>(line)
                .is_ok_and(|value| value.get("id").and_then(Value::as_u64) == Some(response_id))
        })
    }

    fn is_complete(&self) -> bool {
        self.stdout.capture.complete && self.stderr.capture.complete && self.stdin.result.is_some()
    }

    fn into_state(self) -> IoState {
        IoState {
            stdout: self.stdout.capture,
            stderr: self.stderr.capture,
            stdin: self.stdin.result,
        }
    }
}

#[cfg(unix)]
fn set_nonblocking(handle: &impl std::os::fd::AsRawFd) -> Result<(), String> {
    let fd = handle.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(format!(
            "read descriptor flags failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        return Err(format!(
            "set descriptor nonblocking failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn poll_io_until(io: &mut ProbeIo, deadline: Instant) {
    io.poll();
    while !io.is_complete() && Instant::now() < deadline {
        thread::sleep(
            PROCESS_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
        );
        io.poll();
    }
}

fn downgrade_cleanup_for_incomplete_output(cleanup: &mut ProcessProbeCleanup, io_state: &IoState) {
    let mut pending = Vec::new();
    if !io_state.stdout.complete {
        pending.push("stdout");
    }
    if !io_state.stderr.complete {
        pending.push("stderr");
    }
    if pending.is_empty() {
        return;
    }
    cleanup.termination = ProcessProbeTerminationState::Partial;
    cleanup.errors.push(format!(
        "stdio streams remained open ({}); escaped descendant termination was not confirmed",
        pending.join(", ")
    ));
}

trait CleanupDriver {
    fn descendant_termination_supported(&self) -> bool;
    fn descendant_containment_guaranteed(&self) -> bool;
    fn process_group_exists(&mut self) -> Result<bool, String>;
    fn kill_process_group(&mut self) -> Result<(), String>;
    fn kill_child(&mut self) -> Result<(), String>;
    fn try_reap_child(&mut self) -> Result<bool, String>;
}

fn run_cleanup(
    driver: &mut impl CleanupDriver,
    child_already_reaped: bool,
    deadline: Instant,
) -> ProcessProbeCleanup {
    let descendant_termination_supported = driver.descendant_termination_supported();
    let descendant_containment_guaranteed = driver.descendant_containment_guaranteed();
    let mut steps = Vec::new();
    let mut errors = Vec::new();
    let mut child_reaped = child_already_reaped;
    let mut termination_required = false;
    let mut group_absent = !descendant_termination_supported;

    if descendant_termination_supported {
        match driver.process_group_exists() {
            Ok(true) => {
                termination_required = true;
                match driver.kill_process_group() {
                    Ok(()) => steps.push("process-group SIGKILL sent".to_string()),
                    Err(error) => errors.push(format!("process-group kill failed: {error}")),
                }
            }
            Ok(false) => {
                group_absent = true;
                steps.push("process group already absent".to_string());
            }
            Err(error) => errors.push(format!("process-group status failed: {error}")),
        }
    } else {
        steps.push("descendant termination unsupported on this platform".to_string());
    }

    if !child_reaped {
        termination_required = true;
        match driver.kill_child() {
            Ok(()) => steps.push("direct child kill sent".to_string()),
            Err(error) => errors.push(format!("direct child kill failed: {error}")),
        }
    }

    while Instant::now() < deadline && (!child_reaped || !group_absent) {
        if !child_reaped {
            match driver.try_reap_child() {
                Ok(true) => {
                    child_reaped = true;
                    steps.push("child reaped".to_string());
                }
                Ok(false) => {}
                Err(error) => {
                    errors.push(format!("child reap status failed: {error}"));
                    break;
                }
            }
        }
        if descendant_termination_supported && !group_absent {
            match driver.process_group_exists() {
                Ok(false) => {
                    group_absent = true;
                    steps.push("process group confirmed absent".to_string());
                }
                Ok(true) => {}
                Err(error) => {
                    errors.push(format!("process-group confirmation failed: {error}"));
                    break;
                }
            }
        }
        if !child_reaped || !group_absent {
            thread::sleep(
                PROCESS_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }

    if !child_reaped {
        errors.push("child reap was not confirmed before cleanup deadline".to_string());
    }
    if descendant_termination_supported && !group_absent {
        errors.push(
            "process-group termination was not confirmed before cleanup deadline".to_string(),
        );
    }
    if termination_required
        && descendant_termination_supported
        && !descendant_containment_guaranteed
    {
        errors.push(
            "process-group cleanup cannot prove termination of escaped descendants".to_string(),
        );
    }

    let termination = if !termination_required {
        ProcessProbeTerminationState::NotRequired
    } else if child_reaped && group_absent && descendant_containment_guaranteed && errors.is_empty()
    {
        ProcessProbeTerminationState::Confirmed
    } else {
        ProcessProbeTerminationState::Partial
    };
    ProcessProbeCleanup {
        termination,
        child_reaped,
        descendant_termination_supported,
        descendant_containment_guaranteed,
        steps,
        errors,
    }
}

struct ChildProcessGroupGuard {
    child: Option<Child>,
    #[cfg(unix)]
    process_group_id: Option<i32>,
}

impl ChildProcessGroupGuard {
    fn new(child: Child) -> Self {
        #[cfg(unix)]
        let process_group_id = i32::try_from(child.id()).ok();
        Self {
            child: Some(child),
            #[cfg(unix)]
            process_group_id,
        }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("managed child is present")
    }

    fn cleanup(&mut self, child_already_reaped: bool, deadline: Instant) -> ProcessProbeCleanup {
        let cleanup = run_cleanup(self, child_already_reaped, deadline);
        self.child.take();
        cleanup
    }
}

impl CleanupDriver for ChildProcessGroupGuard {
    fn descendant_termination_supported(&self) -> bool {
        cfg!(unix)
    }

    fn descendant_containment_guaranteed(&self) -> bool {
        false
    }

    fn process_group_exists(&mut self) -> Result<bool, String> {
        #[cfg(unix)]
        {
            let Some(process_group_id) = self.process_group_id else {
                return Ok(false);
            };
            let result = unsafe { libc::kill(-process_group_id, 0) };
            if result == 0 {
                return Ok(true);
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(false)
            } else if error.raw_os_error() == Some(libc::EPERM) {
                Ok(true)
            } else {
                Err(error.to_string())
            }
        }
        #[cfg(not(unix))]
        Ok(false)
    }

    fn kill_process_group(&mut self) -> Result<(), String> {
        #[cfg(unix)]
        {
            let Some(process_group_id) = self.process_group_id else {
                return Ok(());
            };
            let result = unsafe { libc::kill(-process_group_id, libc::SIGKILL) };
            if result == 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error.to_string())
            }
        }
        #[cfg(not(unix))]
        Err("process groups are unsupported".to_string())
    }

    fn kill_child(&mut self) -> Result<(), String> {
        self.child_mut().kill().map_err(|error| error.to_string())
    }

    fn try_reap_child(&mut self) -> Result<bool, String> {
        self.child_mut()
            .try_wait()
            .map(|status| status.is_some())
            .map_err(|error| error.to_string())
    }
}

impl Drop for ChildProcessGroupGuard {
    fn drop(&mut self) {
        if self.child.is_some() {
            let deadline = Instant::now() + PROCESS_CLEANUP_GRACE;
            let _ = self.cleanup(false, deadline);
        }
    }
}

impl ProcessProbeCleanup {
    fn partial_without_child(error: impl Into<String>) -> Self {
        Self {
            termination: ProcessProbeTerminationState::Partial,
            child_reaped: false,
            descendant_termination_supported: cfg!(unix),
            descendant_containment_guaranteed: false,
            steps: Vec::new(),
            errors: vec![error.into()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[cfg(unix)]
    fn shell_request(script: &str) -> ProcessProbeRequest {
        ProcessProbeRequest {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_string(), script.to_string()],
            stdin_lines: Vec::new(),
            timeout: Duration::from_secs(1),
            max_stdout_bytes: 4 * 1024,
            max_stderr_bytes: 4 * 1024,
            terminate_after_response_id: None,
            stdout_format: ProcessProbeStdoutFormat::JsonLines,
        }
    }

    #[test]
    fn json_document_format_reads_a_pretty_printed_object_as_one_response() {
        let responses = parse_probe_stdout(
            "{\n  \"loggedIn\": true,\n  \"authMethod\": \"claude.ai\"\n}\n",
            ProcessProbeStdoutFormat::JsonDocument,
        )
        .expect("a pretty-printed document");

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["loggedIn"], serde_json::json!(true));
    }

    #[test]
    fn json_document_format_reports_output_that_is_not_json() {
        let error =
            parse_probe_stdout("not json\nat all\n", ProcessProbeStdoutFormat::JsonDocument)
                .expect_err("a malformed document");

        assert_eq!(error.line_number, 1);
        assert!(error.line.contains("not json"), "{error:?}");
    }

    #[test]
    fn json_document_format_accepts_empty_output() {
        assert_eq!(
            parse_probe_stdout("  \n", ProcessProbeStdoutFormat::JsonDocument),
            Ok(Vec::new())
        );
    }

    #[test]
    fn json_lines_format_still_reads_one_value_per_line_and_names_a_bad_line() {
        let responses = parse_probe_stdout(
            "{\"id\":1}\n\n{\"id\":2}\n",
            ProcessProbeStdoutFormat::JsonLines,
        )
        .expect("two responses");
        assert_eq!(responses.len(), 2);

        let error = parse_probe_stdout("{\"id\":1}\nbroken\n", ProcessProbeStdoutFormat::JsonLines)
            .expect_err("a malformed line");
        assert_eq!(error.line_number, 2);
        assert_eq!(error.line, "broken");
    }

    #[cfg(unix)]
    #[test]
    fn parses_json_lines_and_records_bounded_diagnostics() {
        let mut request = shell_request(
            "IFS= read -r request; printf '%s\\n' '{\"id\":1,\"result\":\"ready\"}' '{\"event\":\"done\"}'; printf 'probe detail' >&2",
        );
        request.stdin_lines = vec!["{\"id\":1,\"method\":\"initialize\"}".to_string()];

        let result = run_process_probe(request).expect("successful probe");

        assert_eq!(
            result.response_lines,
            vec![
                serde_json::json!({"id": 1, "result": "ready"}),
                serde_json::json!({"event": "done"}),
            ]
        );
        assert_eq!(
            result.exit_state,
            ProcessProbeExitState {
                code: Some(0),
                success: true,
            }
        );
        assert_eq!(result.stderr, "probe detail");
        assert!(!result.stderr_truncated);
        assert!(!result.terminated);
        assert!(result.elapsed_millis <= 1_000);
    }

    #[cfg(unix)]
    #[test]
    fn terminates_a_long_running_server_after_the_expected_response() {
        let mut request = shell_request(
            "IFS= read -r request; printf '%s\\n' '{\"id\":1,\"result\":{\"ready\":true}}'; sleep 30",
        );
        request.stdin_lines = vec!["{\"id\":1,\"method\":\"initialize\"}".to_string()];
        request.terminate_after_response_id = Some(1);

        let result = run_process_probe(request).expect("bounded server probe");

        assert_eq!(
            result.response_lines,
            vec![serde_json::json!({"id": 1, "result": {"ready": true}})],
        );
        assert!(result.exit_state.success);
        assert!(!result.terminated);
        assert!(result.cleanup.child_reaped);
        assert_eq!(
            result.cleanup.termination,
            ProcessProbeTerminationState::Partial
        );
        assert!(result.elapsed_millis < 1_000);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_is_structured_without_overclaiming_descendant_termination() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("probe.pid");
        let mut request = shell_request("echo $$ > \"$1\"; exec sleep 30");
        request
            .args
            .extend(["process-probe".to_string(), pid_path.display().to_string()]);
        request.timeout = Duration::from_millis(50);

        let error = run_process_probe(request).expect_err("probe must time out");

        match error {
            ProcessProbeError::Timeout {
                timeout_millis,
                terminated,
                cleanup,
                ..
            } => {
                assert_eq!(timeout_millis, 50);
                assert!(!terminated);
                assert_eq!(cleanup.termination, ProcessProbeTerminationState::Partial);
                assert!(cleanup.steps.iter().any(|step| step == "child reaped"));
                assert!(cleanup.errors.iter().any(|error| {
                    error.contains("cannot prove termination of escaped descendants")
                }));
            }
            other => panic!("expected timeout, got {other:?}"),
        }
        assert_processes_gone(&pid_path, 1);
    }

    #[cfg(unix)]
    #[test]
    fn malformed_json_reports_the_bounded_line_and_exit_context() {
        let request = shell_request(
            "printf '%s\\n' '{\"id\":1,\"result\":\"ready\"}' 'not-json'; printf 'bad response' >&2",
        );

        let error = run_process_probe(request).expect_err("malformed response");

        match error {
            ProcessProbeError::MalformedJson {
                line_number,
                line,
                exit_state,
                stderr,
                terminated,
                ..
            } => {
                assert_eq!(line_number, 2);
                assert_eq!(line, "not-json");
                assert_eq!(exit_state.code, Some(0));
                assert!(exit_state.success);
                assert_eq!(stderr, "bad response");
                assert!(!terminated);
            }
            other => panic!("expected malformed JSON, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn early_nonzero_exit_is_a_probe_result() {
        let request = shell_request(
            "printf '%s\\n' '{\"error\":{\"code\":\"incompatible\"}}'; printf 'unsupported protocol' >&2; exit 7",
        );

        let result = run_process_probe(request).expect("completed probe");

        assert_eq!(
            result.exit_state,
            ProcessProbeExitState {
                code: Some(7),
                success: false,
            }
        );
        assert_eq!(
            result.response_lines,
            vec![serde_json::json!({"error": {"code": "incompatible"}})]
        );
        assert_eq!(result.stderr, "unsupported protocol");
        assert!(!result.terminated);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_descendants_and_does_not_wait_for_inherited_pipes() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("probe-pids");
        let mut request = shell_request("sleep 30 & child=$!; echo \"$$ $child\" > \"$1\"; wait");
        request
            .args
            .extend(["process-probe".to_string(), pid_path.display().to_string()]);
        request.timeout = Duration::from_millis(75);
        let started = Instant::now();

        run_process_probe(request).expect_err("probe must time out");

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "probe waited for inherited pipes: {:?}",
            started.elapsed()
        );
        assert_processes_gone(&pid_path, 2);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_remains_bounded_when_the_child_does_not_read_stdin() {
        let mut request = shell_request("sleep 2");
        request.stdin_lines = vec!["x".repeat(1024 * 1024)];
        request.timeout = Duration::from_millis(50);
        let started = Instant::now();

        let error = run_process_probe(request).expect_err("probe must time out");

        assert!(matches!(error, ProcessProbeError::Timeout { .. }));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "stdin write bypassed the probe deadline: {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[test]
    fn completed_parent_cleans_group_without_overclaiming_containment() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_path = temp.path().join("descendant.pid");
        let mut request = shell_request(
            "sleep 30 & child=$!; echo \"$child\" > \"$1\"; printf '%s\\n' '{\"ok\":true}'; exit 0",
        );
        request
            .args
            .extend(["process-probe".to_string(), pid_path.display().to_string()]);

        let result = run_process_probe(request).expect("completed probe");

        assert!(!result.terminated);
        assert_eq!(
            result.cleanup.termination,
            ProcessProbeTerminationState::Partial
        );
        assert_eq!(result.response_lines, vec![serde_json::json!({"ok": true})]);
        assert_processes_gone(&pid_path, 1);
    }

    #[cfg(unix)]
    #[test]
    fn completed_parent_capture_timeout_preserves_partial_stderr_and_is_bounded() {
        let temp = tempfile::tempdir().expect("temp");
        let ready_path = temp.path().join("parent-ready");
        let release_path = temp.path().join("release-parent");
        let escaped_pid_path = temp.path().join("escaped.pid");
        let mut escaped_fixture =
            EscapedFixtureGuard::new(escaped_pid_path.clone(), release_path.clone());
        let code = r#"
import os
import sys
import time

sys.stdout.write('{"ok":true}\n')
sys.stdout.flush()
sys.stderr.write("captured-before-escape")
sys.stderr.flush()
with open(sys.argv[1], "w", encoding="utf-8") as marker:
    marker.write("ready")
while not os.path.exists(sys.argv[2]):
    time.sleep(0.005)
pid = os.fork()
if pid == 0:
    os.setsid()
    with open(sys.argv[3], "w", encoding="utf-8") as pid_file:
        pid_file.write(str(os.getpid()))
        pid_file.flush()
    time.sleep(10)
    os._exit(0)
os._exit(0)
"#;
        let request = ProcessProbeRequest {
            program: PathBuf::from("/usr/bin/python3"),
            args: vec![
                "-c".to_string(),
                code.to_string(),
                ready_path.display().to_string(),
                release_path.display().to_string(),
                escaped_pid_path.display().to_string(),
            ],
            stdin_lines: Vec::new(),
            timeout: Duration::from_secs(2),
            max_stdout_bytes: 4 * 1024,
            max_stderr_bytes: 4 * 1024,
            terminate_after_response_id: None,
            stdout_format: ProcessProbeStdoutFormat::JsonLines,
        };
        let started = Instant::now();
        let probe = std::thread::spawn(move || run_process_probe(request));
        let ready_deadline = Instant::now() + Duration::from_millis(1_500);
        while !ready_path.exists() && Instant::now() < ready_deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !ready_path.exists() {
            std::fs::write(&release_path, b"release").expect("release stalled fixture");
            let outcome = probe.join().expect("probe thread");
            panic!("fixture parent did not reach the capture barrier: {outcome:?}");
        }
        escaped_fixture.release_parent();

        let outcome = probe.join().expect("probe thread");
        escaped_fixture.terminate_and_verify_absent();
        let error = outcome.expect_err("escaped pipe must time out");

        match error {
            ProcessProbeError::CaptureTimeout {
                stderr,
                pending_streams,
                terminated,
                cleanup,
                ..
            } => {
                assert_eq!(stderr, "captured-before-escape");
                assert!(pending_streams.iter().any(|stream| stream == "stderr"));
                assert!(!terminated);
                assert_eq!(cleanup.termination, ProcessProbeTerminationState::Partial);
                assert!(cleanup.child_reaped);
                assert!(cleanup.descendant_termination_supported);
                assert!(!cleanup.descendant_containment_guaranteed);
                assert!(cleanup.errors.iter().any(|error| {
                    error.contains("escaped descendant termination was not confirmed")
                }));
            }
            other => panic!("expected capture timeout, got {other:?}"),
        }
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "capture cleanup exceeded timeout plus grace: {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[test]
    fn timeout_with_escaped_pipe_holder_never_claims_confirmed_termination() {
        let temp = tempfile::tempdir().expect("temp");
        let escaped_pid_path = temp.path().join("escaped.pid");
        let request = escaped_pipe_holder_request(&escaped_pid_path);

        let error = run_process_probe(request).expect_err("probe must time out");
        let escaped_pid = read_single_pid(&escaped_pid_path);
        terminate_fixture_process(escaped_pid);

        match error {
            ProcessProbeError::Timeout {
                terminated,
                cleanup,
                stderr,
                ..
            } => {
                assert!(!terminated);
                assert_eq!(cleanup.termination, ProcessProbeTerminationState::Partial);
                assert!(cleanup.errors.iter().any(|error| {
                    error.contains("stdio")
                        && error.contains("descendant termination was not confirmed")
                }));
                assert_eq!(stderr, "escaped diagnostic");
            }
            other => panic!("expected timeout, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn escaped_pipe_holder_does_not_retain_probe_file_descriptors_after_return() {
        let temp = tempfile::tempdir().expect("temp");
        let escaped_pid_path = temp.path().join("escaped.pid");
        let before = settled_file_descriptor_count();

        let error = run_process_probe(escaped_pipe_holder_request(&escaped_pid_path))
            .expect_err("probe must time out");
        let after = settled_file_descriptor_count();
        let escaped_pid = read_single_pid(&escaped_pid_path);
        terminate_fixture_process(escaped_pid);

        assert!(matches!(error, ProcessProbeError::Timeout { .. }));
        assert!(
            after <= before + 1,
            "probe retained blocking pipe descriptors after return: before={before}, after={after}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn continuous_stdout_cannot_bypass_the_absolute_deadline() {
        let code = r#"
import os
import time

deadline = time.monotonic() + 1.0
chunk = b"x" * 8192
while time.monotonic() < deadline:
    os.write(1, chunk)
"#;
        let request = ProcessProbeRequest {
            program: PathBuf::from("/usr/bin/python3"),
            args: vec!["-c".to_string(), code.to_string()],
            stdin_lines: Vec::new(),
            timeout: Duration::from_millis(50),
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            terminate_after_response_id: None,
            stdout_format: ProcessProbeStdoutFormat::JsonLines,
        };
        let started = Instant::now();

        let error = run_process_probe(request).expect_err("continuous writer must time out");

        assert!(matches!(error, ProcessProbeError::Timeout { .. }));
        assert!(
            started.elapsed() < Duration::from_millis(400),
            "continuous capture bypassed timeout plus cleanup grace: {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[test]
    fn one_capture_poll_is_bounded_for_a_continuously_readable_source() {
        use std::os::fd::{AsRawFd, RawFd};

        struct ContinuouslyReadable {
            descriptor: std::fs::File,
            readable_until: Instant,
        }

        impl Read for ContinuouslyReadable {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if Instant::now() >= self.readable_until {
                    return Ok(0);
                }
                buffer.fill(b'x');
                Ok(buffer.len())
            }
        }

        impl AsRawFd for ContinuouslyReadable {
            fn as_raw_fd(&self) -> RawFd {
                self.descriptor.as_raw_fd()
            }
        }

        let reader = ContinuouslyReadable {
            descriptor: std::fs::File::open("/dev/null").expect("descriptor"),
            readable_until: Instant::now() + Duration::from_secs(1),
        };
        let mut capture = NonblockingCapture::new(reader, 64, CaptureEnd::Head).expect("capture");
        let started = Instant::now();

        capture.poll();

        assert!(
            started.elapsed() < Duration::from_millis(100),
            "one capture poll monopolized the deadline loop: {:?}",
            started.elapsed()
        );
        assert!(!capture.capture.complete);
        assert!(capture.capture.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn one_stdin_poll_is_bounded_for_an_always_writable_sink() {
        use std::os::fd::{AsRawFd, RawFd};

        struct AlwaysWritable {
            descriptor: std::fs::File,
            writable_until: Instant,
        }

        impl Write for AlwaysWritable {
            fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
                if Instant::now() >= self.writable_until {
                    return Err(std::io::ErrorKind::WouldBlock.into());
                }
                Ok(buffer.len().min(8 * 1024))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl AsRawFd for AlwaysWritable {
            fn as_raw_fd(&self) -> RawFd {
                self.descriptor.as_raw_fd()
            }
        }

        let writer = AlwaysWritable {
            descriptor: std::fs::File::open("/dev/null").expect("descriptor"),
            writable_until: Instant::now() + Duration::from_secs(1),
        };
        let mut stdin = NonblockingStdin::from_writer(writer, vec!["x".repeat(128 * 1024 * 1024)])
            .expect("stdin");
        let started = Instant::now();

        stdin.poll();

        assert!(
            started.elapsed() < Duration::from_millis(100),
            "one stdin poll monopolized the deadline loop: {:?}",
            started.elapsed()
        );
        assert!(stdin.result.is_none());
        assert!(stdin.offset <= PROCESS_IO_BYTES_PER_POLL);
    }

    #[cfg(unix)]
    #[test]
    fn silent_escaped_descendant_prevents_confirmed_termination() {
        let temp = tempfile::tempdir().expect("temp");
        let escaped_pid_path = temp.path().join("silent-escaped.pid");
        let code = r#"
import os
import sys
import time

pid = os.fork()
if pid == 0:
    os.setsid()
    os.close(0)
    os.close(1)
    os.close(2)
    with open(sys.argv[1], "w") as pid_file:
        pid_file.write(str(os.getpid()))
        pid_file.flush()
    time.sleep(30)
    os._exit(0)
time.sleep(30)
"#;
        let request = ProcessProbeRequest {
            program: PathBuf::from("/usr/bin/python3"),
            args: vec![
                "-c".to_string(),
                code.to_string(),
                escaped_pid_path.display().to_string(),
            ],
            stdin_lines: Vec::new(),
            timeout: Duration::from_millis(75),
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            terminate_after_response_id: None,
            stdout_format: ProcessProbeStdoutFormat::JsonLines,
        };

        let error = run_process_probe(request).expect_err("probe must time out");
        let escaped_pid = read_single_pid(&escaped_pid_path);
        terminate_fixture_process(escaped_pid);

        match error {
            ProcessProbeError::Timeout {
                terminated,
                cleanup,
                ..
            } => {
                assert!(!terminated);
                assert_eq!(cleanup.termination, ProcessProbeTerminationState::Partial);
                assert!(cleanup.errors.iter().any(|error| {
                    error.contains("process-group cleanup cannot prove")
                        && error.contains("escaped descendants")
                }));
            }
            other => panic!("expected timeout, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn stdout_limit_is_enforced_without_unbounded_capture() {
        let mut request = shell_request("yes x | head -c 8192");
        request.max_stdout_bytes = 64;

        let error = run_process_probe(request).expect_err("stdout must be capped");

        assert!(matches!(
            error,
            ProcessProbeError::StdoutLimitExceeded { max_bytes: 64, .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn stderr_is_truncated_to_the_requested_byte_limit() {
        let mut request =
            shell_request("printf '0123456789abcdef' >&2; printf '%s\\n' '{\"ok\":true}'");
        request.max_stderr_bytes = 8;

        let result = run_process_probe(request).expect("bounded stderr");

        assert!(result.stderr.len() <= 8);
        assert!(result.stderr_truncated);
    }

    #[test]
    fn cleanup_deadline_is_bounded_when_reap_never_completes() {
        let mut driver = FakeCleanupDriver {
            descendant_supported: true,
            group_exists: true,
            group_kill: Ok(()),
            child_kill: Ok(()),
            reap: ReapBehavior::Never,
        };
        let started = Instant::now();

        let cleanup = run_cleanup(
            &mut driver,
            false,
            Instant::now() + Duration::from_millis(30),
        );

        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(cleanup.termination, ProcessProbeTerminationState::Partial);
        assert!(!cleanup.child_reaped);
        assert!(cleanup
            .errors
            .iter()
            .any(|error| error.contains("reap was not confirmed")));
    }

    #[test]
    fn cleanup_failures_never_claim_confirmed_termination() {
        let mut driver = FakeCleanupDriver {
            descendant_supported: true,
            group_exists: true,
            group_kill: Err("injected group kill".to_string()),
            child_kill: Err("injected child kill".to_string()),
            reap: ReapBehavior::Error("injected reap failure".to_string()),
        };

        let cleanup = run_cleanup(
            &mut driver,
            false,
            Instant::now() + Duration::from_millis(30),
        );

        assert_eq!(cleanup.termination, ProcessProbeTerminationState::Partial);
        assert!(!cleanup.child_reaped);
        assert_eq!(
            cleanup.errors,
            vec![
                "process-group kill failed: injected group kill",
                "direct child kill failed: injected child kill",
                "child reap status failed: injected reap failure",
                "child reap was not confirmed before cleanup deadline",
                "process-group termination was not confirmed before cleanup deadline",
                "process-group cleanup cannot prove termination of escaped descendants",
            ]
        );
    }

    #[test]
    fn portable_cleanup_contract_marks_descendant_termination_unsupported() {
        let mut driver = FakeCleanupDriver {
            descendant_supported: false,
            group_exists: false,
            group_kill: Ok(()),
            child_kill: Ok(()),
            reap: ReapBehavior::Complete,
        };

        let cleanup = run_cleanup(
            &mut driver,
            false,
            Instant::now() + Duration::from_millis(30),
        );

        assert!(!cleanup.descendant_termination_supported);
        assert_eq!(cleanup.termination, ProcessProbeTerminationState::Confirmed);
        assert!(cleanup
            .steps
            .iter()
            .any(|step| step == "descendant termination unsupported on this platform"));
    }

    #[cfg(unix)]
    #[test]
    fn capture_timeout_preserves_partial_stderr_and_capture_errors() {
        use std::os::unix::process::ExitStatusExt;

        let mut io_state = IoState::new(128, 128);
        io_state.stdout.complete = true;
        io_state.stderr.bytes = b"partial diagnostic".to_vec();
        io_state.stderr.read_error = Some("injected stderr read failure".to_string());
        io_state.stdin = Some(Ok(()));
        let cleanup = ProcessProbeCleanup {
            termination: ProcessProbeTerminationState::NotRequired,
            child_reaped: true,
            descendant_termination_supported: true,
            descendant_containment_guaranteed: false,
            steps: Vec::new(),
            errors: Vec::new(),
        };

        let error = finalize_completed_probe(
            ExitStatus::from_raw(0),
            io_state,
            Duration::from_millis(5),
            cleanup,
            ProcessProbeStdoutFormat::JsonLines,
        )
        .expect_err("incomplete stderr capture");

        match error {
            ProcessProbeError::CaptureTimeout {
                stderr,
                capture_errors,
                ..
            } => {
                assert_eq!(stderr, "partial diagnostic");
                assert_eq!(capture_errors, vec!["stderr: injected stderr read failure"]);
            }
            other => panic!("expected capture timeout, got {other:?}"),
        }
    }

    enum ReapBehavior {
        Complete,
        Never,
        Error(String),
    }

    #[cfg(unix)]
    fn escaped_pipe_holder_request(escaped_pid_path: &Path) -> ProcessProbeRequest {
        let code = r#"
import os
import sys
import time

pid = os.fork()
if pid == 0:
    os.setsid()
    with open(sys.argv[1], "w") as pid_file:
        pid_file.write(str(os.getpid()))
        pid_file.flush()
    sys.stderr.write("escaped diagnostic")
    sys.stderr.flush()
    time.sleep(30)
    os._exit(0)
time.sleep(30)
"#;
        ProcessProbeRequest {
            program: PathBuf::from("/usr/bin/python3"),
            args: vec![
                "-c".to_string(),
                code.to_string(),
                escaped_pid_path.display().to_string(),
            ],
            stdin_lines: Vec::new(),
            timeout: Duration::from_millis(75),
            max_stdout_bytes: 4 * 1024,
            max_stderr_bytes: 4 * 1024,
            terminate_after_response_id: None,
            stdout_format: ProcessProbeStdoutFormat::JsonLines,
        }
    }

    #[cfg(unix)]
    fn open_file_descriptor_count() -> usize {
        std::fs::read_dir("/dev/fd").expect("read /dev/fd").count()
    }

    /// The process-wide descriptor count is shared with every other test thread, so a
    /// leak check reads the settled low-water mark rather than one racy sample.
    #[cfg(unix)]
    fn settled_file_descriptor_count() -> usize {
        (0..5)
            .map(|attempt| {
                if attempt > 0 {
                    std::thread::sleep(Duration::from_millis(50));
                }
                open_file_descriptor_count()
            })
            .min()
            .expect("at least one sample")
    }

    #[cfg(unix)]
    fn read_single_pid(pid_path: &Path) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if let Ok(pid) = std::fs::read_to_string(pid_path) {
                return pid.trim().parse::<i32>().expect("numeric pid");
            }
            assert!(
                Instant::now() < deadline,
                "escaped fixture did not publish its pid"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn terminate_fixture_process(pid: i32) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    struct EscapedFixtureGuard {
        pid_path: PathBuf,
        release_path: PathBuf,
        pid: Option<i32>,
        released: bool,
        armed: bool,
    }

    #[cfg(unix)]
    impl EscapedFixtureGuard {
        fn new(pid_path: PathBuf, release_path: PathBuf) -> Self {
            Self {
                pid_path,
                release_path,
                pid: None,
                released: false,
                armed: true,
            }
        }

        fn release_parent(&mut self) {
            std::fs::write(&self.release_path, b"release").expect("release fixture parent");
            self.released = true;
        }

        fn wait_for_pid_until(&mut self, deadline: Instant) -> Option<i32> {
            if self.pid.is_some() {
                return self.pid;
            }
            while Instant::now() < deadline {
                if let Ok(value) = std::fs::read_to_string(&self.pid_path) {
                    if let Ok(pid) = value.trim().parse::<i32>() {
                        self.pid = Some(pid);
                        return Some(pid);
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            None
        }

        fn terminate_and_verify_absent(&mut self) {
            let pid = self
                .wait_for_pid_until(Instant::now() + Duration::from_secs(2))
                .expect("escaped fixture did not publish its pid");
            let absent = terminate_and_reap_fixture_process(pid, Duration::from_secs(2));
            assert!(
                absent,
                "escaped fixture process {pid} was not reaped and verified absent"
            );
            self.armed = false;
            self.pid = None;
        }
    }

    #[cfg(unix)]
    impl Drop for EscapedFixtureGuard {
        fn drop(&mut self) {
            if !self.armed {
                return;
            }
            if !self.released {
                let _ = std::fs::write(&self.release_path, b"release");
                self.released = true;
            }
            if let Some(pid) = self.wait_for_pid_until(Instant::now() + Duration::from_secs(2)) {
                let _ = terminate_and_reap_fixture_process(pid, Duration::from_secs(2));
            }
            self.armed = false;
        }
    }

    #[cfg(unix)]
    fn terminate_and_reap_fixture_process(pid: i32, timeout: Duration) -> bool {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let mut status = 0;
            let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
            if waited == pid || fixture_process_is_absent(pid) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    fn fixture_process_is_absent(pid: i32) -> bool {
        let result = unsafe { libc::kill(pid, 0) };
        result != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }

    struct FakeCleanupDriver {
        descendant_supported: bool,
        group_exists: bool,
        group_kill: Result<(), String>,
        child_kill: Result<(), String>,
        reap: ReapBehavior,
    }

    impl CleanupDriver for FakeCleanupDriver {
        fn descendant_termination_supported(&self) -> bool {
            self.descendant_supported
        }

        fn descendant_containment_guaranteed(&self) -> bool {
            !self.descendant_supported
        }

        fn process_group_exists(&mut self) -> Result<bool, String> {
            Ok(self.group_exists)
        }

        fn kill_process_group(&mut self) -> Result<(), String> {
            self.group_kill.clone()
        }

        fn kill_child(&mut self) -> Result<(), String> {
            self.child_kill.clone()
        }

        fn try_reap_child(&mut self) -> Result<bool, String> {
            match &self.reap {
                ReapBehavior::Complete => Ok(true),
                ReapBehavior::Never => Ok(false),
                ReapBehavior::Error(error) => Err(error.clone()),
            }
        }
    }

    #[cfg(unix)]
    fn assert_processes_gone(pid_path: &Path, expected: usize) {
        let pids = std::fs::read_to_string(pid_path)
            .expect("pid fixture")
            .split_whitespace()
            .map(|value| value.parse::<i32>().expect("numeric pid"))
            .collect::<Vec<_>>();
        assert_eq!(pids.len(), expected);

        for pid in pids {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                let exists = unsafe { libc::kill(pid, 0) } == 0;
                if !exists {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "process {pid} remained alive after probe cleanup"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
