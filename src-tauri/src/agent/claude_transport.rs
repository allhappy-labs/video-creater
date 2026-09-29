//! Running one conversation turn as one `claude` process.
//!
//! A per-turn process is not a compromise: `--resume` recovers the full history in a fresh
//! process because conversation state is a file, not a live server. So there is no supervisor,
//! no reconnect, and cancel is a signal.
//!
//! Spawning, the deadline, the SIGTERM-then-SIGKILL cancel and the stderr cap all come from
//! `process_supervisor::run_to_completion`, the same machinery the Codex probe uses. This
//! module adds nothing of its own to that; a second cancellation mechanism would be a bug
//! waiting to happen.

use super::claude_cli::{
    build_claude_turn_argv, claude_child_env_removals, claude_mcp_config_json,
    claude_turn_stdin_line, resolve_claude_executable, ClaudeAuthMode, ClaudeSessionHandle,
    ClaudeTurnInvocation, CLAUDE_DEFAULT_MODEL, CLAUDE_FALLBACK_MODEL, CLAUDE_TURN_BUDGET_USD,
    CLAUDE_TURN_TIMEOUT,
};
use super::claude_frames::summarize_claude_stream;
use super::{
    AgentBackendKind, AgentTurnError, AgentTurnOutcome, AgentTurnRequest, AgentTurnTransport,
};
use crate::process_supervisor::{
    run_to_completion, CancellationSignal, SupervisedCommand, SupervisionError, SupervisionPolicy,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The stdout cap. A turn's stream is a few dozen frames; a megabyte is far past generous.
const CLAUDE_STDOUT_LIMIT: usize = 4 * 1024 * 1024;
/// The stderr tail kept for diagnostics, matching the Codex probe.
const CLAUDE_STDERR_LIMIT: usize = 64 * 1024;

pub struct ClaudeTurnTransport {
    executable: PathBuf,
    /// The app's own MCP sidecar, which the turn reads the project through.
    mcp_server: PathBuf,
    model: String,
    fallback_model: Option<String>,
    /// Which credential the readiness probe says this turn will spend. It decides whether an
    /// `ANTHROPIC_API_KEY` in the app's environment reaches the child.
    auth: ClaudeAuthMode,
    budget_usd: f64,
    policy: SupervisionPolicy,
}

impl ClaudeTurnTransport {
    /// Build a transport around the user's own installed `claude`.
    ///
    /// Nothing is ever bundled, so a missing binary is a normal outcome and is reported as
    /// "unavailable" rather than as a failure.
    pub fn new(
        configured_path: Option<&std::path::Path>,
        mcp_server: PathBuf,
    ) -> Result<Self, AgentTurnError> {
        let executable = resolve_claude_executable(configured_path).ok_or_else(|| {
            AgentTurnError::Unavailable(
                "Claude isn't installed. Install it, then open Agent settings to check it."
                    .to_string(),
            )
        })?;
        Ok(Self::with_executable(executable, mcp_server))
    }

    pub fn with_executable(executable: PathBuf, mcp_server: PathBuf) -> Self {
        Self {
            executable,
            mcp_server,
            model: CLAUDE_DEFAULT_MODEL.to_string(),
            fallback_model: Some(CLAUDE_FALLBACK_MODEL.to_string()),
            // The subscription is the supported path, and it is also the cautious default:
            // it withholds an environment API key rather than letting one bill silently.
            auth: ClaudeAuthMode::Subscription,
            budget_usd: CLAUDE_TURN_BUDGET_USD,
            policy: SupervisionPolicy {
                deadline: CLAUDE_TURN_TIMEOUT,
                // Cancel is SIGTERM, then SIGKILL after a short grace, as the Codex path does.
                term_grace: Duration::from_millis(200),
                kill_grace: Duration::from_millis(500),
                ..SupervisionPolicy::default()
            },
        }
    }

    pub fn with_model(mut self, model: String, fallback_model: Option<String>) -> Self {
        self.model = model;
        self.fallback_model = fallback_model;
        self
    }

    pub fn with_auth(mut self, auth: ClaudeAuthMode) -> Self {
        self.auth = auth;
        self
    }

    /// The invocation this transport would run for a request.
    ///
    /// Public so a test can pin the argv — the model alias above all — without spawning
    /// anything and without spending a token.
    pub fn invocation(
        &self,
        request: &AgentTurnRequest,
        session: ClaudeSessionHandle,
    ) -> ClaudeTurnInvocation {
        let project_dir = request
            .project_dir
            .clone()
            .unwrap_or_else(|| request.cwd.clone());
        ClaudeTurnInvocation {
            model: self.model.clone(),
            fallback_model: self.fallback_model.clone(),
            developer_instructions: request.developer_instructions.clone(),
            output_schema: request.output_schema.clone(),
            session,
            mcp_config: claude_mcp_config_json(&self.mcp_server, &project_dir),
            budget_usd: self.budget_usd,
        }
    }
}

impl AgentTurnTransport for ClaudeTurnTransport {
    fn backend(&self) -> AgentBackendKind {
        AgentBackendKind::Claude
    }

    fn run_conversation_turn(
        &mut self,
        request: &AgentTurnRequest,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<AgentTurnOutcome, AgentTurnError> {
        let session = match &request.session {
            Some(session) => ClaudeSessionHandle::Resume(session.clone()),
            None => ClaudeSessionHandle::New(uuid::Uuid::new_v4().to_string()),
        };
        let minted_session = match &session {
            ClaudeSessionHandle::New(id) | ClaudeSessionHandle::Resume(id) => id.clone(),
        };

        let spec = SupervisedCommand {
            program: self.executable.clone(),
            args: build_claude_turn_argv(&self.invocation(request, session)),
            cwd: Some(request.cwd.clone()),
            // The subscription credential is a file under ~/.claude (or the macOS keychain),
            // so the child needs the environment as it is and nothing forwarded. The one
            // exception is ANTHROPIC_API_KEY, which the CLI prefers over the login: a
            // subscription session withholds it so the login the user chose is the one that
            // pays, and a key-backed session passes it through because it is the credential.
            clear_env: false,
            env: BTreeMap::new(),
            env_remove: claude_child_env_removals(self.auth),
            stdin: claude_turn_stdin_line(&request.prompt).into_bytes(),
            stdout_limit: CLAUDE_STDOUT_LIMIT,
            stderr_limit: CLAUDE_STDERR_LIMIT,
        };
        let mut policy = self.policy;
        policy.deadline = deadline
            .checked_duration_since(Instant::now())
            .unwrap_or_default()
            .min(self.policy.deadline);

        let output = run_to_completion(spec, policy, cancellation).map_err(map_supervision)?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        // A run that failed without writing a single frame never got as far as a turn: a
        // missing binary (exit 127), a rejected flag, a broken install. That is "unavailable",
        // not "the agent failed", and the stderr tail is the only useful thing to say.
        if !output.status.success() && !stdout.lines().any(is_frame) {
            return Err(AgentTurnError::Unavailable(unavailable_message(
                &String::from_utf8_lossy(&output.stderr),
                output.status.code(),
            )));
        }

        let summary = summarize_claude_stream(stdout.lines())?;
        let session = if summary.session_id.is_empty() {
            minted_session
        } else {
            summary.session_id.clone()
        };

        Ok(AgentTurnOutcome {
            session,
            proposal: summary.proposal,
            model: summary.model,
            notice: summary.notice,
            transcript: json!({
                "frames": summary.transcript.get("frames").cloned().unwrap_or(json!([])),
                "mcpServers": summary
                    .mcp_servers
                    .iter()
                    .map(|(name, status)| json!({ "name": name, "status": status }))
                    .collect::<Vec<_>>(),
                "toolCalls": summary.tool_calls,
                "permissionDenials": summary.permission_denials,
                "costUsd": summary.cost_usd,
            }),
        })
    }
}

fn is_frame(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line).is_ok_and(|value| value.is_object())
}

fn unavailable_message(stderr: &str, code: Option<i32>) -> String {
    let tail = stderr.trim();
    let tail = tail
        .lines()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    match (tail.is_empty(), code) {
        (true, Some(code)) => format!("Claude didn't start (exit {code})."),
        (true, None) => "Claude didn't start.".to_string(),
        (false, _) => format!("Claude didn't start: {tail}"),
    }
}

fn map_supervision(error: SupervisionError) -> AgentTurnError {
    match error {
        SupervisionError::Launch(detail) => {
            AgentTurnError::Unavailable(format!("Claude didn't start: {detail}"))
        }
        SupervisionError::Cancelled { .. } => AgentTurnError::Interrupted,
        SupervisionError::Deadline { .. } => AgentTurnError::Deadline,
        other => AgentTurnError::TurnFailed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct FlaggedCancellation(Arc<AtomicBool>);

    impl CancellationSignal for FlaggedCancellation {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }

        fn wait_timeout(&self, duration: Duration) -> bool {
            std::thread::sleep(duration.min(Duration::from_millis(20)));
            self.is_cancelled()
        }
    }

    fn write_stub(directory: &Path, name: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a stub");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("an exec bit");
        }
        path
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("claude_agent")
            .join(name)
    }

    fn request(cwd: &Path) -> AgentTurnRequest {
        AgentTurnRequest {
            cwd: cwd.to_path_buf(),
            developer_instructions: "You propose video edits.".to_string(),
            prompt: "hidden context\nTrim the first clip.".to_string(),
            output_schema: json!({ "type": "object" }),
            session: None,
            project_dir: Some(cwd.to_path_buf()),
            cancellation_key: cwd.display().to_string(),
        }
    }

    fn transport(program: PathBuf) -> ClaudeTurnTransport {
        ClaudeTurnTransport::with_executable(
            program,
            PathBuf::from("/opt/video-creater-mcp-server"),
        )
    }

    #[test]
    fn a_replayed_success_stream_becomes_a_turn_outcome() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let stub = write_stub(
            directory.path(),
            "claude",
            &format!(
                "cat >/dev/null\ncat {}",
                fixture_path("success-with-mcp-tool.jsonl").display()
            ),
        );

        let outcome = transport(stub)
            .run_conversation_turn(
                &request(directory.path()),
                Instant::now() + Duration::from_secs(30),
                None,
            )
            .expect("an outcome");

        assert!(!outcome.session.is_empty());
        assert_eq!(outcome.model.as_deref(), Some("claude-haiku-4-5-20251001"));
        let proposal = outcome.proposal.expect("a proposal");
        assert!(proposal.get("summary").is_some());
        assert_eq!(outcome.transcript["mcpServers"][0]["status"], "connected");
        assert_eq!(outcome.transcript["permissionDenials"], json!([]));
    }

    #[test]
    fn the_prompt_reaches_stdin_as_one_line_and_stdin_is_then_closed() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let seen = directory.path().join("stdin.txt");
        // `cat` only ends when the writer closes the pipe, so this stub also proves the close.
        let stub = write_stub(
            directory.path(),
            "claude",
            &format!(
                "cat > {}\ncat {}",
                seen.display(),
                fixture_path("success-with-mcp-tool.jsonl").display()
            ),
        );

        transport(stub)
            .run_conversation_turn(
                &request(directory.path()),
                Instant::now() + Duration::from_secs(30),
                None,
            )
            .expect("an outcome");

        let written = fs::read_to_string(&seen).expect("the stdin capture");
        assert_eq!(written.lines().count(), 1, "{written}");
        let frame: serde_json::Value =
            serde_json::from_str(written.trim()).expect("a stream-json line");
        assert_eq!(frame["type"], "user");
        assert_eq!(
            frame["message"]["content"][0]["text"],
            "hidden context\nTrim the first clip."
        );
    }

    #[test]
    fn a_turn_that_outlives_its_deadline_is_killed() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let marker = directory.path().join("finished");
        let stub = write_stub(
            directory.path(),
            "claude",
            &format!("cat >/dev/null\nsleep 30\ntouch {}", marker.display()),
        );

        let error = transport(stub)
            .run_conversation_turn(
                &request(directory.path()),
                Instant::now() + Duration::from_millis(200),
                None,
            )
            .expect_err("a deadline");

        assert_eq!(error, AgentTurnError::Deadline);
        // The stub never reached its own end, so the child was signalled rather than waited on.
        std::thread::sleep(Duration::from_millis(300));
        assert!(!marker.exists(), "the child outlived the deadline");
    }

    #[test]
    fn a_cancelled_turn_is_interrupted_within_the_grace_period() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let marker = directory.path().join("finished");
        let stub = write_stub(
            directory.path(),
            "claude",
            &format!("cat >/dev/null\nsleep 30\ntouch {}", marker.display()),
        );
        let flag = Arc::new(AtomicBool::new(false));
        let cancellation = FlaggedCancellation(flag.clone());
        let flipper = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            flipper.store(true, Ordering::SeqCst);
        });

        let started = Instant::now();
        let error = transport(stub)
            .run_conversation_turn(
                &request(directory.path()),
                Instant::now() + Duration::from_secs(30),
                Some(&cancellation),
            )
            .expect_err("an interruption");

        assert_eq!(error, AgentTurnError::Interrupted);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "cancel was slow"
        );
        std::thread::sleep(Duration::from_millis(300));
        assert!(!marker.exists(), "the child survived the cancel");
    }

    #[test]
    fn a_missing_binary_reports_the_agent_as_unavailable() {
        let directory = tempfile::tempdir().expect("a temp dir");
        let stub = write_stub(
            directory.path(),
            "claude",
            "cat >/dev/null\necho 'claude: command not found' >&2\nexit 127",
        );

        let error = transport(stub)
            .run_conversation_turn(
                &request(directory.path()),
                Instant::now() + Duration::from_secs(30),
                None,
            )
            .expect_err("an unavailable agent");

        match error {
            AgentTurnError::Unavailable(detail) => {
                assert!(detail.starts_with("Claude didn't start"), "{detail}");
                assert!(detail.contains("command not found"), "{detail}");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_binary_that_cannot_be_spawned_at_all_is_unavailable() {
        let error = transport(PathBuf::from("/nonexistent/claude"))
            .run_conversation_turn(
                &request(Path::new(".")),
                Instant::now() + Duration::from_secs(5),
                None,
            )
            .expect_err("an unavailable agent");

        assert!(matches!(error, AgentTurnError::Unavailable(_)), "{error:?}");
    }

    #[test]
    fn an_authentication_failure_from_the_stream_wins_over_the_exit_code() {
        let directory = tempfile::tempdir().expect("a temp dir");
        // Verified against the real CLI: exit 1 alongside a well-formed error result.
        let stub = write_stub(
            directory.path(),
            "claude",
            &format!(
                "cat >/dev/null\ncat {}\nexit 1",
                fixture_path("auth-failed.jsonl").display()
            ),
        );

        let error = transport(stub)
            .run_conversation_turn(
                &request(directory.path()),
                Instant::now() + Duration::from_secs(30),
                None,
            )
            .expect_err("an authentication failure");

        assert!(
            matches!(error, AgentTurnError::NotAuthenticated(_)),
            "{error:?}"
        );
    }
}
