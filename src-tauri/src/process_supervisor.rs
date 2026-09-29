use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub trait CancellationSignal: Send + Sync {
    fn is_cancelled(&self) -> bool;
    fn wait_timeout(&self, duration: Duration) -> bool;
}

#[derive(Debug, Clone)]
pub struct SupervisedCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub clear_env: bool,
    pub env: BTreeMap<String, String>,
    /// Variables withheld from the child even though the parent has them. Applied after
    /// `env`, so a name in both is removed: a caller that means to unset something must be
    /// able to say so without knowing whether the parent happens to export it.
    pub env_remove: Vec<String>,
    pub stdin: Vec<u8>,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct SupervisionPolicy {
    pub deadline: Duration,
    pub poll_interval: Duration,
    pub term_grace: Duration,
    pub kill_grace: Duration,
}

impl Default for SupervisionPolicy {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(300),
            poll_interval: Duration::from_millis(20),
            term_grace: Duration::from_millis(200),
            kill_grace: Duration::from_secs(1),
        }
    }
}

#[derive(Debug)]
pub struct SupervisedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr: Vec<u8>,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CleanupReport {
    pub reaped: bool,
    pub stderr: Vec<u8>,
    pub stderr_truncated: bool,
}

pub struct SupervisedSession {
    child: Option<Child>,
    process_group: u32,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stdout_buffer: Vec<u8>,
    stderr_capture: Capture,
    policy: SupervisionPolicy,
    max_line_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum SupervisionError {
    #[error("supervised command could not start: {0}")]
    Launch(String),
    #[error("supervised command stdin failed: {0}")]
    Stdin(String),
    #[error("supervised command status failed: {0}")]
    Status(String),
    #[error("supervised command exceeded its deadline: {stderr}")]
    Deadline {
        stderr: String,
        stderr_truncated: bool,
    },
    #[error("supervised command was cancelled: {stderr}")]
    Cancelled {
        stderr: String,
        stderr_truncated: bool,
    },
    #[error("supervised command output capture did not finish")]
    CaptureTimeout,
    #[error("supervised framed session message exceeded {limit} bytes")]
    MessageTooLarge { limit: usize },
    #[error("supervised framed session reached EOF")]
    Eof,
    #[error("supervised framed session JSON failed: {0}")]
    Json(String),
}

pub fn spawn_framed_session(
    spec: SupervisedCommand,
    policy: SupervisionPolicy,
    max_line_bytes: usize,
) -> Result<SupervisedSession, SupervisionError> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    if spec.clear_env {
        command.env_clear();
    }
    command.envs(&spec.env);
    for name in &spec.env_remove {
        command.env_remove(name);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| SupervisionError::Launch(error.to_string()))?;
    let process_group = child.id();
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    if let Err(error) = set_nonblocking(&stdin) {
        terminate_and_reap(&mut child, process_group, policy);
        return Err(SupervisionError::Stdin(error));
    }
    if let Err(error) = set_nonblocking(&stdout).and_then(|_| set_nonblocking(&stderr)) {
        terminate_and_reap(&mut child, process_group, policy);
        return Err(SupervisionError::Status(error));
    }
    Ok(SupervisedSession {
        child: Some(child),
        process_group,
        stdin: Some(stdin),
        stdout,
        stderr,
        stdout_buffer: Vec::new(),
        stderr_capture: Capture::new(spec.stderr_limit, true),
        policy,
        max_line_bytes,
    })
}

impl SupervisedSession {
    pub fn send_json(&mut self, value: &serde_json::Value) -> Result<(), SupervisionError> {
        self.send_json_until(value, Instant::now() + self.policy.deadline, None)
    }

    pub fn send_json_until(
        &mut self,
        value: &serde_json::Value,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<(), SupervisionError> {
        let mut bytes =
            serde_json::to_vec(value).map_err(|error| SupervisionError::Json(error.to_string()))?;
        bytes.push(b'\n');
        let mut offset = 0;
        while offset < bytes.len() {
            if cancellation.is_some_and(CancellationSignal::is_cancelled) {
                return Err(SupervisionError::Cancelled {
                    stderr: self.stderr_text(),
                    stderr_truncated: self.stderr_capture.truncated,
                });
            }
            if Instant::now() >= deadline {
                return Err(SupervisionError::Deadline {
                    stderr: self.stderr_text(),
                    stderr_truncated: self.stderr_capture.truncated,
                });
            }
            match self
                .stdin
                .as_mut()
                .ok_or_else(|| SupervisionError::Stdin("session stdin is closed".into()))?
                .write(&bytes[offset..])
            {
                Ok(0) => return Err(SupervisionError::Stdin("session closed stdin".into())),
                Ok(count) => offset += count,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(SupervisionError::Stdin(error.to_string())),
            }
            self.pump_pipes()?;
            if offset < bytes.len() {
                thread::sleep(self.policy.poll_interval);
            }
        }
        Ok(())
    }

    pub fn recv_json_until(
        &mut self,
        deadline: Instant,
    ) -> Result<serde_json::Value, SupervisionError> {
        loop {
            if let Some(newline) = self.stdout_buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.stdout_buffer.drain(..=newline).collect();
                let line = &line[..line.len().saturating_sub(1)];
                return serde_json::from_slice(line)
                    .map_err(|error| SupervisionError::Json(error.to_string()));
            }
            if Instant::now() >= deadline {
                return Err(SupervisionError::Deadline {
                    stderr: self.stderr_text(),
                    stderr_truncated: self.stderr_capture.truncated,
                });
            }
            self.pump_pipes()?;
            if self
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok())
                .flatten()
                .is_some()
            {
                self.pump_pipes()?;
                return Err(SupervisionError::Eof);
            }
            thread::sleep(
                self.policy
                    .poll_interval
                    .min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    }

    pub fn diagnostic_stderr_tail(&self) -> (&[u8], bool) {
        (&self.stderr_capture.bytes, self.stderr_capture.truncated)
    }

    pub fn terminate_and_reap(&mut self) -> CleanupReport {
        self.stdin.take();
        let reaped = if let Some(mut child) = self.child.take() {
            terminate_and_reap(&mut child, self.process_group, self.policy);
            true
        } else {
            true
        };
        let mut ignored_stdout = Capture::new(0, false);
        drain_after_shutdown(
            &mut self.stdout,
            &mut self.stderr,
            &mut ignored_stdout,
            &mut self.stderr_capture,
            self.policy.kill_grace,
        );
        CleanupReport {
            reaped,
            stderr: self.stderr_capture.bytes.clone(),
            stderr_truncated: self.stderr_capture.truncated,
        }
    }

    fn pump_pipes(&mut self) -> Result<(), SupervisionError> {
        let mut buffer = [0_u8; 8192];
        for _ in 0..16 {
            match self.stdout.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    self.stdout_buffer.extend_from_slice(&buffer[..count]);
                    if self.stdout_buffer.len() > self.max_line_bytes {
                        return Err(SupervisionError::MessageTooLarge {
                            limit: self.max_line_bytes,
                        });
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(SupervisionError::Status(error.to_string())),
            }
        }
        pump_output(&mut self.stderr, &mut self.stderr_capture);
        Ok(())
    }

    fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr_capture.bytes).into_owned()
    }
}

impl Drop for SupervisedSession {
    fn drop(&mut self) {
        let _ = self.terminate_and_reap();
    }
}

#[derive(Debug)]
struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
    limit: usize,
    tail: bool,
}

pub fn run_to_completion(
    spec: SupervisedCommand,
    policy: SupervisionPolicy,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<SupervisedOutput, SupervisionError> {
    let deadline = Instant::now() + policy.deadline;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }
    if spec.clear_env {
        command.env_clear();
    }
    command.envs(&spec.env);
    for name in &spec.env_remove {
        command.env_remove(name);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| SupervisionError::Launch(error.to_string()))?;
    let process_group = child.id();
    let mut stdin = Some(child.stdin.take().expect("piped stdin"));
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    set_nonblocking(stdin.as_ref().expect("stdin present")).map_err(|error| {
        terminate_and_reap(&mut child, process_group, policy);
        SupervisionError::Stdin(error)
    })?;
    set_nonblocking(&stdout).map_err(|error| {
        terminate_and_reap(&mut child, process_group, policy);
        SupervisionError::Status(error)
    })?;
    set_nonblocking(&stderr).map_err(|error| {
        terminate_and_reap(&mut child, process_group, policy);
        SupervisionError::Status(error)
    })?;
    let mut stdin_offset = 0;
    let mut stdin_open = true;
    let mut stdout_capture = Capture::new(spec.stdout_limit, false);
    let mut stderr_capture = Capture::new(spec.stderr_limit, true);
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut exited: Option<(ExitStatus, Instant)> = None;

    loop {
        if cancellation.is_some_and(CancellationSignal::is_cancelled) {
            stdin.take();
            terminate_and_reap(&mut child, process_group, policy);
            drain_after_shutdown(
                &mut stdout,
                &mut stderr,
                &mut stdout_capture,
                &mut stderr_capture,
                policy.kill_grace,
            );
            return Err(SupervisionError::Cancelled {
                stderr: String::from_utf8_lossy(&stderr_capture.bytes).into_owned(),
                stderr_truncated: stderr_capture.truncated,
            });
        }
        if Instant::now() >= deadline {
            stdin.take();
            terminate_and_reap(&mut child, process_group, policy);
            drain_after_shutdown(
                &mut stdout,
                &mut stderr,
                &mut stdout_capture,
                &mut stderr_capture,
                policy.kill_grace,
            );
            return Err(SupervisionError::Deadline {
                stderr: String::from_utf8_lossy(&stderr_capture.bytes).into_owned(),
                stderr_truncated: stderr_capture.truncated,
            });
        }

        if stdin_open {
            match pump_stdin(
                stdin.as_mut().expect("stdin open"),
                &spec.stdin,
                &mut stdin_offset,
            ) {
                Ok(done) => {
                    if done {
                        stdin_open = false;
                        stdin.take();
                    }
                }
                Err(error) => {
                    stdin.take();
                    terminate_and_reap(&mut child, process_group, policy);
                    drain_after_shutdown(
                        &mut stdout,
                        &mut stderr,
                        &mut stdout_capture,
                        &mut stderr_capture,
                        policy.kill_grace,
                    );
                    return Err(SupervisionError::Stdin(error));
                }
            }
        }
        stdout_open &= pump_output(&mut stdout, &mut stdout_capture);
        stderr_open &= pump_output(&mut stderr, &mut stderr_capture);

        if exited.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => exited = Some((status, Instant::now())),
                Ok(None) => {}
                Err(error) => {
                    stdin.take();
                    terminate_and_reap(&mut child, process_group, policy);
                    return Err(SupervisionError::Status(error.to_string()));
                }
            }
        }
        if let Some((status, exited_at)) = exited {
            if !stdout_open && !stderr_open {
                return Ok(SupervisedOutput {
                    status,
                    stdout: stdout_capture.bytes,
                    stdout_truncated: stdout_capture.truncated,
                    stderr: stderr_capture.bytes,
                    stderr_truncated: stderr_capture.truncated,
                });
            }
            if exited_at.elapsed() >= policy.kill_grace {
                kill_group(process_group, libc::SIGKILL);
            }
        }

        let wait = policy
            .poll_interval
            .min(deadline.saturating_duration_since(Instant::now()));
        if let Some(signal) = cancellation {
            if signal.wait_timeout(wait) {
                stdin.take();
                terminate_and_reap(&mut child, process_group, policy);
                drain_after_shutdown(
                    &mut stdout,
                    &mut stderr,
                    &mut stdout_capture,
                    &mut stderr_capture,
                    policy.kill_grace,
                );
                return Err(SupervisionError::Cancelled {
                    stderr: String::from_utf8_lossy(&stderr_capture.bytes).into_owned(),
                    stderr_truncated: stderr_capture.truncated,
                });
            }
        } else {
            thread::sleep(wait);
        }
    }
}

impl Capture {
    fn new(limit: usize, tail: bool) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8192)),
            truncated: false,
            limit,
            tail,
        }
    }
}

#[cfg(unix)]
fn set_nonblocking(stream: &impl AsRawFd) -> Result<(), String> {
    let fd = stream.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_nonblocking(_stream: &impl std::fmt::Debug) -> Result<(), String> {
    Err("bounded process supervision requires nonblocking child pipes".to_string())
}

fn pump_stdin(stdin: &mut ChildStdin, input: &[u8], offset: &mut usize) -> Result<bool, String> {
    if *offset >= input.len() {
        return Ok(true);
    }
    let end = (*offset + 8192).min(input.len());
    match stdin.write(&input[*offset..end]) {
        Ok(0) => Err("worker closed stdin before accepting the request".to_string()),
        Ok(count) => {
            *offset += count;
            Ok(*offset >= input.len())
        }
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn pump_output(reader: &mut impl Read, capture: &mut Capture) -> bool {
    let mut buffer = [0_u8; 8192];
    for _ in 0..16 {
        match reader.read(&mut buffer) {
            Ok(0) => return false,
            Ok(count) => capture.push(&buffer[..count]),
            Err(error) if error.kind() == ErrorKind::WouldBlock => return true,
            Err(_) => return false,
        }
    }
    true
}

impl Capture {
    fn push(&mut self, chunk: &[u8]) {
        let limit = self.limit;
        if !self.tail {
            let remaining = limit.saturating_sub(self.bytes.len());
            self.bytes
                .extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            self.truncated |= chunk.len() > remaining;
            return;
        }
        if chunk.len() >= limit {
            self.bytes.clear();
            if limit > 0 {
                self.bytes.extend_from_slice(&chunk[chunk.len() - limit..]);
            }
            self.truncated = true;
        } else {
            let overflow = self
                .bytes
                .len()
                .saturating_add(chunk.len())
                .saturating_sub(limit);
            if overflow > 0 {
                self.bytes.drain(..overflow);
                self.truncated = true;
            }
            self.bytes.extend_from_slice(chunk);
        }
    }
}

fn drain_after_shutdown(
    stdout: &mut ChildStdout,
    stderr: &mut ChildStderr,
    stdout_capture: &mut Capture,
    stderr_capture: &mut Capture,
    grace: Duration,
) {
    let deadline = Instant::now() + grace;
    let mut stdout_open = true;
    let mut stderr_open = true;
    while (stdout_open || stderr_open) && Instant::now() < deadline {
        stdout_open &= pump_output(stdout, stdout_capture);
        stderr_open &= pump_output(stderr, stderr_capture);
        if stdout_open || stderr_open {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

fn terminate_and_reap(
    child: &mut std::process::Child,
    process_group: u32,
    policy: SupervisionPolicy,
) {
    kill_group(process_group, libc::SIGTERM);
    let term_deadline = Instant::now() + policy.term_grace;
    while Instant::now() < term_deadline {
        if child.try_wait().ok().flatten().is_some() {
            kill_group(process_group, libc::SIGKILL);
            return;
        }
        thread::sleep(policy.poll_interval.min(Duration::from_millis(20)));
    }
    kill_group(process_group, libc::SIGKILL);
    let kill_deadline = Instant::now() + policy.kill_grace;
    while Instant::now() < kill_deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(policy.poll_interval.min(Duration::from_millis(20)));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn kill_group(process_group: u32, signal: i32) {
    #[cfg(unix)]
    if let Ok(process_group) = i32::try_from(process_group) {
        let _ = unsafe { libc::kill(-process_group, signal) };
    }
    #[cfg(not(unix))]
    let _ = (process_group, signal);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[cfg(unix)]
    fn shell(script: &str) -> SupervisedCommand {
        SupervisedCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), script.into()],
            cwd: None,
            clear_env: false,
            env: BTreeMap::new(),
            env_remove: Vec::new(),
            stdin: Vec::new(),
            stdout_limit: 1024,
            stderr_limit: 1024,
        }
    }

    #[cfg(unix)]
    #[test]
    fn noisy_stdout_and_stderr_are_drained_and_capped() {
        let output = run_to_completion(
            shell("yes out | head -c 131072; yes err | head -c 131072 >&2"),
            SupervisionPolicy::default(),
            None,
        )
        .expect("worker completes");
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 1024);
        assert_eq!(output.stderr.len(), 1024);
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
    }

    #[cfg(unix)]
    #[test]
    fn framed_session_drains_stderr_and_returns_valid_json() {
        let mut command = shell("read request; yes diagnostic | head -c 131072 >&2; printf '%s\\n' '{\"id\":1,\"result\":{}}'; sleep 30");
        command.stderr_limit = 1024;
        let mut session = spawn_framed_session(
            command,
            SupervisionPolicy {
                deadline: Duration::from_secs(1),
                term_grace: Duration::from_millis(20),
                kill_grace: Duration::from_millis(200),
                ..SupervisionPolicy::default()
            },
            4096,
        )
        .unwrap();
        session.send_json(&serde_json::json!({"id":1})).unwrap();
        assert_eq!(
            session
                .recv_json_until(Instant::now() + Duration::from_secs(1))
                .unwrap()["id"],
            1
        );
        let cleanup = session.terminate_and_reap();
        assert!(cleanup.reaped);
        assert_eq!(cleanup.stderr.len(), 1024);
        assert!(cleanup.stderr_truncated);
    }

    #[cfg(unix)]
    #[test]
    fn framed_session_rejects_oversized_unterminated_message() {
        let mut session = spawn_framed_session(
            shell("head -c 8192 /dev/zero | tr '\\0' x; sleep 30"),
            SupervisionPolicy {
                deadline: Duration::from_secs(1),
                term_grace: Duration::from_millis(20),
                kill_grace: Duration::from_millis(200),
                ..SupervisionPolicy::default()
            },
            1024,
        )
        .unwrap();
        let error = session
            .recv_json_until(Instant::now() + Duration::from_secs(1))
            .unwrap_err();
        assert!(matches!(
            error,
            SupervisionError::MessageTooLarge { limit: 1024 }
        ));
        assert!(session.terminate_and_reap().reaped);
    }

    #[cfg(unix)]
    #[test]
    fn hung_worker_returns_within_parent_deadline() {
        let started = Instant::now();
        let error = run_to_completion(
            shell("printf 'deadline diagnostic' >&2; exec sleep 30"),
            SupervisionPolicy {
                deadline: Duration::from_millis(50),
                term_grace: Duration::from_millis(30),
                kill_grace: Duration::from_millis(300),
                ..SupervisionPolicy::default()
            },
            None,
        )
        .expect_err("worker times out");
        match error {
            SupervisionError::Deadline { stderr, .. } => {
                assert_eq!(stderr, "deadline diagnostic")
            }
            other => panic!("expected deadline, got {other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn deadline_bounds_oversized_stdin_when_worker_never_reads() {
        let mut command = shell("exec sleep 30");
        command.stdin = vec![b'x'; 8 * 1024 * 1024];
        let started = Instant::now();
        let error = run_to_completion(
            command,
            SupervisionPolicy {
                deadline: Duration::from_millis(75),
                term_grace: Duration::from_millis(30),
                kill_grace: Duration::from_millis(300),
                ..SupervisionPolicy::default()
            },
            None,
        )
        .expect_err("non-reading worker must time out");
        assert!(matches!(error, SupervisionError::Deadline { .. }));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_term_ignoring_child_and_descendant_process_group() {
        let temp = tempfile::tempdir().expect("temp");
        let pid_file = temp.path().join("pids");
        let mut command = shell("trap '' TERM; (trap '' TERM; exec sleep 30) & child=$!; echo \"$$ $child\" > \"$1\"; wait");
        command.args.extend([
            "supervisor-fixture".to_string(),
            pid_file.to_string_lossy().into_owned(),
        ]);
        let error = run_to_completion(
            command,
            SupervisionPolicy {
                deadline: Duration::from_millis(100),
                term_grace: Duration::from_millis(30),
                kill_grace: Duration::from_millis(500),
                ..SupervisionPolicy::default()
            },
            None,
        )
        .expect_err("worker times out");
        assert!(matches!(error, SupervisionError::Deadline { .. }));
        let pids = std::fs::read_to_string(pid_file).expect("read readiness marker");
        for pid in pids
            .split_whitespace()
            .map(|pid| pid.parse::<i32>().expect("pid"))
        {
            let deadline = Instant::now() + Duration::from_secs(1);
            while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "process {pid} survived");
        }
    }

    struct TestCancellation(AtomicBool);

    impl CancellationSignal for TestCancellation {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::Acquire)
        }

        fn wait_timeout(&self, duration: Duration) -> bool {
            let deadline = Instant::now() + duration;
            while Instant::now() < deadline && !self.is_cancelled() {
                thread::sleep(Duration::from_millis(2));
            }
            self.is_cancelled()
        }
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_interrupts_a_hung_worker() {
        let cancellation = Arc::new(TestCancellation(AtomicBool::new(false)));
        let worker_cancellation = Arc::clone(&cancellation);
        let started = Instant::now();
        let worker = thread::spawn(move || {
            let mut command = shell("exec sleep 30");
            command.stdin = vec![b'x'; 8 * 1024 * 1024];
            run_to_completion(
                command,
                SupervisionPolicy::default(),
                Some(worker_cancellation.as_ref()),
            )
        });
        thread::sleep(Duration::from_millis(30));
        cancellation.0.store(true, Ordering::Release);
        let error = worker.join().expect("join").expect_err("cancelled");
        assert!(matches!(error, SupervisionError::Cancelled { .. }));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
