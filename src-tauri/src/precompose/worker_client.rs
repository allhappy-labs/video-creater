use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use video_creater_precompose_protocol::{PrecomposeRequest, PrecomposeResult, WorkerEvent};

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_STDOUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;

struct BoundedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
pub struct WorkerRun {
    pub result: PrecomposeResult,
    pub events: Vec<WorkerEvent>,
    pub stderr: String,
}

pub fn run_precompose_worker(request: &PrecomposeRequest) -> PipelineResult<WorkerRun> {
    run_precompose_worker_cancellable(request, || false)
}

pub fn run_precompose_worker_cancellable(
    request: &PrecomposeRequest,
    is_cancelled: impl Fn() -> bool,
) -> PipelineResult<WorkerRun> {
    let worker_path = precompose_worker_path()?;
    let mut command = Command::new(&worker_path);
    command
        .env_clear()
        .env("RUST_BACKTRACE", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    let mut child = command.spawn().map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker",
            "Expression-enabled precompose worker could not be started.",
            "Bundle video-creater-precompose-worker beside the app binary or set VIDEO_CREATER_PRECOMPOSE_WORKER.",
        )
        .with_detail("workerPath", worker_path.display().to_string())
        .with_detail("error", error.to_string())]
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| worker_pipe_error("stdin"))?;
    serde_json::to_writer(&mut stdin, request).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.request",
            "Precompose request could not be serialized.",
            "Validate the precompose protocol request and retry.",
        )
        .with_detail("error", error.to_string())]
    })?;
    stdin.write_all(b"\n").map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.stdin",
            "Precompose request could not be written to the worker.",
            "Retry with a healthy bundled precompose worker.",
        )
        .with_detail("error", error.to_string())]
    })?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| worker_pipe_error("stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| worker_pipe_error("stderr"))?;
    let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
    let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
    let started = Instant::now();
    let timeout = Duration::from_millis(request.budgets.max_wall_time_ms.saturating_add(2_000));
    let status = loop {
        if is_cancelled() {
            terminate_worker(&mut child);
            let _ = child.wait();
            return Err(cancelled_error());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                terminate_worker(&mut child);
                let _ = child.wait();
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    "precompose.worker.timeout",
                    "Expression-enabled precompose worker exceeded its wall-time budget.",
                    "Reduce animation dimensions or duration, or increase the reviewed worker budget.",
                )
                .with_detail("timeoutMs", timeout.as_millis().to_string())]);
            }
            Err(error) => {
                terminate_worker(&mut child);
                return Err(vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    "precompose.worker.status",
                    "Precompose worker status could not be read.",
                    "Retry with a healthy bundled precompose worker.",
                )
                .with_detail("error", error.to_string())]);
            }
        }
    };
    let stdout = stdout_reader.join().unwrap_or(BoundedOutput {
        bytes: Vec::new(),
        truncated: true,
    });
    let stderr = stderr_reader.join().unwrap_or(BoundedOutput {
        bytes: Vec::new(),
        truncated: true,
    });
    if stdout.truncated {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.stdoutBudget",
            "Precompose worker exceeded the bounded stdout protocol budget.",
            "Reduce the animation frame count or inspect the worker for unexpected output.",
        )
        .with_detail("maxStdoutBytes", MAX_STDOUT_BYTES.to_string())]);
    }
    let stdout = String::from_utf8_lossy(&stdout.bytes);
    let mut stderr_text = String::from_utf8_lossy(&stderr.bytes).into_owned();
    if stderr.truncated {
        stderr_text.push_str("\n[stderr truncated at 65536 bytes]");
    }
    let mut events = Vec::new();
    for (line_index, line) in stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        events.push(serde_json::from_str::<WorkerEvent>(line).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "precompose.worker.stdout",
                "Precompose worker emitted an invalid protocol event.",
                "Keep worker stdout restricted to versioned NDJSON protocol records.",
            )
            .with_detail("line", (line_index + 1).to_string())
            .with_detail("error", error.to_string())]
        })?);
    }
    let final_events = events
        .iter()
        .filter(|event| event.is_final())
        .collect::<Vec<_>>();
    if final_events.len() != 1 {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.events",
            "Precompose worker did not emit exactly one final protocol event.",
            "Inspect the worker crash log and retry with the pinned worker binary.",
        )
        .with_detail("finalEventCount", final_events.len().to_string())
        .with_detail("stderr", stderr_text)]);
    }
    match final_events[0] {
        WorkerEvent::Completed { result, .. } if status.success() => Ok(WorkerRun {
            result: result.clone(),
            events,
            stderr: stderr_text,
        }),
        WorkerEvent::Failed { error, .. } => Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.render",
            error.message.clone(),
            "Inspect the Lottie expression, archive safety, and resource budgets before retrying.",
        )
        .with_detail("workerErrorCode", format!("{:?}", error.code))
        .with_detail("retryable", error.retryable.to_string())
        .with_detail("stderr", stderr_text)]),
        _ => Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.statusCode",
            "Precompose worker completion disagreed with its process status.",
            "Inspect the pinned worker binary and protocol output.",
        )
        .with_detail("statusCode", status.code().unwrap_or(-1).to_string())]),
    }
}

fn cancelled_error() -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.worker.cancelled",
        "Precompose worker was cancelled.",
        "Restart the render when preparation should continue.",
    )]
}

fn precompose_worker_path() -> PipelineResult<PathBuf> {
    if let Some(path) = std::env::var_os("VIDEO_CREATER_PRECOMPOSE_WORKER") {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe().map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.worker.path",
            "Current executable path could not be resolved.",
            "Set VIDEO_CREATER_PRECOMPOSE_WORKER to the bundled worker path.",
        )
        .with_detail("error", error.to_string())]
    })?;
    let mut binary_dir = current.parent().unwrap_or_else(|| Path::new("."));
    if binary_dir.file_name().and_then(|value| value.to_str()) == Some("deps") {
        binary_dir = binary_dir.parent().unwrap_or(binary_dir);
    }
    let sibling = binary_dir.join("video-creater-precompose-worker");
    Ok(sibling)
}

fn read_bounded(mut reader: impl Read, limit: usize) -> BoundedOutput {
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(read);
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    BoundedOutput { bytes, truncated }
}

fn terminate_worker(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        if let Ok(pid) = i32::try_from(child.id()) {
            let _ = unsafe { kill(-pid, 9) };
        }
    }
    let _ = child.kill();
}

fn worker_pipe_error(pipe: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.worker.{pipe}"),
        "Precompose worker pipe could not be opened.",
        "Retry with a healthy bundled precompose worker.",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reader_drains_input_and_marks_truncation() {
        let output = read_bounded(std::io::Cursor::new(vec![7_u8; 32]), 8);
        assert_eq!(output.bytes, vec![7_u8; 8]);
        assert!(output.truncated);
    }
}
