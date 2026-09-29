use serde::{Deserialize, Serialize};
use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use super::error::{PipelineError, PipelineErrorCode, PipelineResult};

const STDERR_DETAIL_LIMIT: usize = 500;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PROCESS_TERM_GRACE: Duration = Duration::from_millis(100);

#[cfg(unix)]
const SIGTERM: i32 = 15;
#[cfg(unix)]
const SIGKILL: i32 = 9;

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait ProcessRunner {
    fn run(&self, spec: &CommandSpec, timeout: Duration) -> PipelineResult<ProcessOutput>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn run(&self, spec: &CommandSpec, timeout: Duration) -> PipelineResult<ProcessOutput> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|error| spawn_error(spec, error))?;
        let process_group_id = child.id();
        let stdout_reader = child.stdout.take().map(read_pipe_in_background);
        let stderr_reader = child.stderr.take().map(read_pipe_in_background);

        let started_at = Instant::now();
        let mut exit_status = None;
        let output = loop {
            if exit_status.is_none() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        exit_status = Some(status);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        return Err(process_wait_error(
                            spec,
                            "process.statusCode",
                            "poll process status",
                            error,
                        ));
                    }
                }
            }

            if let Some(status) = exit_status {
                if pipe_reader_finished(&stdout_reader) && pipe_reader_finished(&stderr_reader) {
                    break ProcessOutput {
                        status_code: status.code(),
                        stdout: join_pipe_reader(stdout_reader),
                        stderr: join_pipe_reader(stderr_reader),
                    };
                }
            }

            if started_at.elapsed() >= timeout {
                terminate_process_group(process_group_id);
                let _ = child.kill();

                let deadline = Instant::now() + PROCESS_TERM_GRACE;
                while Instant::now() < deadline {
                    if exit_status.is_none() {
                        exit_status = child.try_wait().map_err(|error| {
                            process_wait_error(
                                spec,
                                "process.timeout",
                                "poll timed out process status",
                                error,
                            )
                        })?;
                    }

                    if exit_status.is_some()
                        && pipe_reader_finished(&stdout_reader)
                        && pipe_reader_finished(&stderr_reader)
                    {
                        break;
                    }

                    thread::sleep(PROCESS_POLL_INTERVAL);
                }

                terminate_process_group_force(process_group_id);
                let _ = child.kill();
                let _ = child.wait();
                let stdout = join_pipe_reader_if_finished(stdout_reader);
                let stderr = join_pipe_reader_if_finished(stderr_reader);

                return Err(timeout_error(spec, timeout, exit_status, stdout, stderr));
            }

            thread::sleep(PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(started_at.elapsed())));
        };

        let status_success = output.status_code == Some(0);
        let process_output = output;
        if status_success {
            Ok(process_output)
        } else {
            Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "process.statusCode",
                "Render backend command exited with a nonzero status.",
                "Inspect stderr and render logs, then fix the backend command or input media.",
            )
            .with_detail("program", spec.program.clone())
            .with_detail("args", spec.args.join(" "))
            .with_detail(
                "statusCode",
                process_output
                    .status_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
            .with_detail(
                "stderr",
                bounded_stderr_tail(&process_output.stderr),
            )])
        }
    }
}

fn read_pipe_in_background<R>(mut pipe: R) -> JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        let _ = pipe.read_to_end(&mut output);
        output
    })
}

fn pipe_reader_finished(reader: &Option<JoinHandle<Vec<u8>>>) -> bool {
    reader.as_ref().is_none_or(JoinHandle::is_finished)
}

fn join_pipe_reader(reader: Option<JoinHandle<Vec<u8>>>) -> String {
    reader
        .and_then(|reader| reader.join().ok())
        .map(|output| String::from_utf8_lossy(&output).into_owned())
        .unwrap_or_default()
}

fn join_pipe_reader_if_finished(reader: Option<JoinHandle<Vec<u8>>>) -> String {
    if pipe_reader_finished(&reader) {
        join_pipe_reader(reader)
    } else {
        String::new()
    }
}

fn timeout_error(
    spec: &CommandSpec,
    timeout: Duration,
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
) -> Vec<PipelineError> {
    let process_output = ProcessOutput {
        status_code: status.and_then(|status| status.code()),
        stdout,
        stderr,
    };

    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendTimeout,
        "process.timeout",
        "Render backend command exceeded its timeout.",
        "Reduce input complexity, increase the render timeout, or inspect backend logs for a stuck process.",
    )
    .with_detail("program", spec.program.clone())
    .with_detail("args", spec.args.join(" "))
    .with_detail("timeoutMs", timeout.as_millis().to_string())
    .with_detail(
        "statusCode",
        process_output
            .status_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    )
    .with_detail("stderr", bounded_stderr_tail(&process_output.stderr))]
}

#[cfg(unix)]
fn terminate_process_group(process_group_id: u32) {
    signal_process_group(process_group_id, SIGTERM);
}

#[cfg(not(unix))]
fn terminate_process_group(_process_group_id: u32) {}

#[cfg(unix)]
fn terminate_process_group_force(process_group_id: u32) {
    signal_process_group(process_group_id, SIGKILL);
}

#[cfg(not(unix))]
fn terminate_process_group_force(_process_group_id: u32) {}

#[cfg(unix)]
fn signal_process_group(process_group_id: u32, signal: i32) {
    let process_group_id = match i32::try_from(process_group_id) {
        Ok(process_group_id) => process_group_id,
        Err(_) => return,
    };

    unsafe {
        kill(-process_group_id, signal);
    }
}

fn process_wait_error(
    spec: &CommandSpec,
    path: &'static str,
    action: &'static str,
    error: std::io::Error,
) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        path,
        format!("Render backend command failed to {action}."),
        "Retry the render and inspect backend logs if the process cannot be managed.",
    )
    .with_detail("program", spec.program.clone())
    .with_detail("command", spec.display())
    .with_detail("ioError", error.to_string())]
}

fn bounded_stderr_tail(stderr: &str) -> String {
    if stderr.chars().count() <= STDERR_DETAIL_LIMIT {
        return stderr.to_string();
    }

    let prefix = "[stderr truncated] ";
    let tail_limit = STDERR_DETAIL_LIMIT.saturating_sub(prefix.chars().count());
    let tail = stderr
        .chars()
        .rev()
        .take(tail_limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{prefix}{tail}")
}

fn spawn_error(spec: &CommandSpec, error: std::io::Error) -> Vec<PipelineError> {
    let code = if error.kind() == std::io::ErrorKind::NotFound {
        PipelineErrorCode::RenderBackendUnavailable
    } else {
        PipelineErrorCode::RenderBackendFailed
    };

    vec![PipelineError::new(
        code,
        "process.program",
        format!("Render backend command could not start: {}.", spec.program),
        "Install the render backend binary or configure the path to an executable command.",
    )
    .with_detail("program", spec.program.clone())
    .with_detail("command", spec.display())
    .with_detail("ioError", error.to_string())]
}
