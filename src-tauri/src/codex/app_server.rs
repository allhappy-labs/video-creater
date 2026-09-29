pub use crate::agent::prompt::{
    render_agent_turn_prompt as build_codex_conversation_prompt,
    scrub_echoed_user_input as without_conversation_user_input,
};
pub use crate::agent::schema::{
    codex_conversation_proposal_output_schema, codex_edit_proposal_output_schema,
    project_action_schema,
};

use super::context::{
    build_codex_conversation_context, build_codex_developer_instructions, CodexConversationContext,
    VideoEditContext,
};
use super::conversation::{
    conversation_proposal_from_text, conversation_proposal_from_value,
    prepare_codex_conversation_proposal, CodexConversationEditRequest, CodexConversationError,
    CodexConversationTurnResult,
};
use super::model_selection::{choose_codex_turn_model, run_codex_turn_with_supported_model};
use super::proposal::{validate_codex_edit_proposal, CodexEditProposal, CodexProposalError};
use crate::edit::preset::EditJobRequest;
use crate::gpu_graphics::profile::shadertoy_profile_templates;
use crate::process_supervisor::{
    spawn_framed_session, CancellationSignal, CleanupReport, SupervisedCommand, SupervisedSession,
    SupervisionError, SupervisionPolicy,
};
use crate::project::model::VideoProject;
use crate::project::split::{
    codex_thread_for_next_turn, record_app_server_conversation_turn, AppServerConversationTurn,
    ProjectValidationIssue,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAppServerCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexThreadAction {
    Start,
    Resume { thread_id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexVideoEditTurnResult {
    pub thread_id: String,
    pub thread_response: Value,
    pub turn_response: Value,
    pub proposal: Option<CodexEditProposal>,
    pub proposal_validation_issues: Option<Vec<ProjectValidationIssue>>,
    /// Set when the configured model was replaced by a supported one.
    pub model_notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppServerMessage {
    Response {
        id: Value,
        result: Value,
    },
    ErrorResponse {
        id: Value,
        code: i64,
        message: String,
    },
    Notification {
        method: String,
        params: Value,
    },
    ServerRequest {
        id: Value,
        method: String,
        params: Value,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppServerCleanupReport {
    pub reaped: bool,
    pub stderr: String,
    pub stderr_truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletedCodexTurn {
    pub start_response: Value,
    pub terminal_turn: Value,
    pub final_agent_messages: Vec<Value>,
    pub final_text: String,
    pub retained_messages: Vec<AppServerMessage>,
}

#[derive(Debug, Error, PartialEq)]
pub enum CodexAppServerError {
    #[error("failed to build Codex video edit context: {0}")]
    Context(super::context::CodexContextError),
    #[error("Codex edit proposal failed validation: {0}")]
    Proposal(CodexProposalError),
    #[error("Codex conversation request is invalid: {0}")]
    ConversationRequest(CodexConversationError),
    #[error("app-server response did not include thread.id")]
    MissingThreadId,
    #[error("app-server response id did not match request id")]
    ResponseIdMismatch,
    #[error("app-server response did not include result")]
    MissingResult,
    #[error("app-server error {code}: {message}")]
    RpcError { code: i64, message: String },
    #[error("app-server transport json error: {0}")]
    Json(String),
    #[error("app-server transport io error: {0}")]
    Io(String),
    #[error("app-server process did not expose stdio")]
    MissingStdio,
    #[error("app-server transport failed: {0}")]
    Transport(String),
    #[error("Codex executable was not found: {0}")]
    MissingExecutable(String),
    #[error("app-server initialize response is incompatible: {0}")]
    IncompatibleProtocol(String),
    #[error("app-server turn/start response did not include turn.id")]
    MissingTurnId,
    #[error("app-server turn exceeded its deadline")]
    Deadline,
    #[error("app-server turn was interrupted")]
    Interrupted,
    #[error("app-server turn failed: {0}")]
    TurnFailed(String),
}

pub trait CodexAppServerTransport {
    fn send(&mut self, message: Value) -> Result<(), CodexAppServerError>;
    fn send_until(
        &mut self,
        message: Value,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<(), CodexAppServerError> {
        if cancellation.is_some_and(CancellationSignal::is_cancelled) {
            return Err(CodexAppServerError::Interrupted);
        }
        if Instant::now() >= deadline {
            return Err(CodexAppServerError::Deadline);
        }
        self.send(message)
    }
    fn recv_until(&mut self, deadline: Instant) -> Result<AppServerMessage, CodexAppServerError>;
    fn terminate(&mut self) -> Result<AppServerCleanupReport, CodexAppServerError>;
}

pub struct StdioCodexAppServerTransport {
    session: SupervisedSession,
}

const APP_SERVER_TURN_TIMEOUT: Duration = Duration::from_secs(180);
const APP_SERVER_POLL_INTERVAL: Duration = Duration::from_millis(25);
const APP_SERVER_INTERRUPT_GRACE: Duration = Duration::from_millis(100);
const APP_SERVER_MAX_LINE_BYTES: usize = 1024 * 1024;
const APP_SERVER_MAX_RETAINED_EVENTS: usize = 128;
const APP_SERVER_MAX_FINAL_TEXT_BYTES: usize = 1024 * 1024;

pub fn codex_app_server_deadline() -> Instant {
    Instant::now() + APP_SERVER_TURN_TIMEOUT
}

struct CodexTurnCancellationState {
    cancelled: AtomicBool,
    wake: Condvar,
    lock: Mutex<()>,
}
#[derive(Clone)]
pub struct CodexTurnCancellationToken(Arc<CodexTurnCancellationState>);
pub struct CodexTurnCancellationGuard {
    key: String,
    state: Arc<CodexTurnCancellationState>,
}
static ACTIVE_CODEX_TURNS: OnceLock<Mutex<HashMap<String, Arc<CodexTurnCancellationState>>>> =
    OnceLock::new();

pub fn register_codex_turn_cancellation(
    key: String,
) -> Result<(CodexTurnCancellationGuard, CodexTurnCancellationToken), CodexAppServerError> {
    let state = Arc::new(CodexTurnCancellationState {
        cancelled: AtomicBool::new(false),
        wake: Condvar::new(),
        lock: Mutex::new(()),
    });
    let mut active = ACTIVE_CODEX_TURNS
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| {
            CodexAppServerError::Transport("Codex cancellation registry lock failed".into())
        })?;
    if active.contains_key(&key) {
        return Err(CodexAppServerError::Transport(
            "a Codex turn is already active for this project".into(),
        ));
    }
    active.insert(key.clone(), state.clone());
    Ok((
        CodexTurnCancellationGuard {
            key,
            state: state.clone(),
        },
        CodexTurnCancellationToken(state),
    ))
}

pub fn request_codex_turn_cancellation(key: &str) -> bool {
    let Ok(active) = ACTIVE_CODEX_TURNS.get_or_init(Default::default).lock() else {
        return false;
    };
    let Some(state) = active.get(key) else {
        return false;
    };
    state.cancelled.store(true, Ordering::Release);
    state.wake.notify_all();
    true
}

impl CancellationSignal for CodexTurnCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    fn wait_timeout(&self, duration: Duration) -> bool {
        if self.is_cancelled() {
            return true;
        }
        if let Ok(guard) = self.0.lock.lock() {
            let _ = self.0.wake.wait_timeout(guard, duration);
        }
        self.is_cancelled()
    }
}

impl Drop for CodexTurnCancellationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE_CODEX_TURNS.get_or_init(Default::default).lock() {
            if active
                .get(&self.key)
                .is_some_and(|state| Arc::ptr_eq(state, &self.state))
            {
                active.remove(&self.key);
            }
        }
    }
}

pub fn codex_app_server_command(codex_binary: &str) -> CodexAppServerCommand {
    CodexAppServerCommand {
        program: codex_binary.to_string(),
        args: vec!["app-server".to_string(), "--stdio".to_string()],
    }
}

pub fn bundled_codex_app_server_command() -> Result<CodexAppServerCommand, CodexAppServerError> {
    let executable = env::current_exe().map_err(|error| {
        CodexAppServerError::Io(format!("failed to resolve the app executable: {error}"))
    })?;
    Ok(bundled_codex_app_server_command_for_executable(&executable))
}

pub fn bundled_codex_executable() -> Result<PathBuf, CodexAppServerError> {
    bundled_codex_app_server_command().map(|command| PathBuf::from(command.program))
}

fn bundled_codex_app_server_command_for_executable(executable: &Path) -> CodexAppServerCommand {
    let directory = executable.parent().unwrap_or_else(|| Path::new("."));
    codex_app_server_command(&directory.join("video-creater-codex").display().to_string())
}

pub fn resolve_codex_executable(codex_binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH");
    resolve_codex_executable_in_path(codex_binary, path.as_deref())
}

fn resolve_codex_executable_in_path(codex_binary: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    let requested = Path::new(codex_binary);
    // Preserve explicit configured paths even when they are not executable so
    // the launch boundary can report a launch failure instead of "missing".
    if requested.components().count() > 1 {
        return requested.is_file().then(|| requested.to_path_buf());
    }

    path.and_then(|path| {
        env::split_paths(path)
            .map(|directory| directory.join(requested))
            .find(|candidate| is_path_executable(candidate))
    })
}

pub(crate) fn is_path_executable(candidate: &Path) -> bool {
    if !candidate.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let Ok(path) = CString::new(candidate.as_os_str().as_bytes()) else {
            return false;
        };
        // Let the OS apply the current uid/gid permission class and ACLs.
        unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
    }
    #[cfg(not(unix))]
    true
}

pub fn resolve_codex_app_server_command(
    codex_binary: &str,
) -> Result<CodexAppServerCommand, CodexAppServerError> {
    let program = resolve_codex_executable(codex_binary)
        .ok_or_else(|| CodexAppServerError::MissingExecutable(codex_binary.to_string()))?;
    Ok(codex_app_server_command(&program.display().to_string()))
}

pub fn build_app_server_initialize_request(request_id: u64) -> Value {
    json!({
        "id": request_id,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "video-creater",
                "title": "Video Creater",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "experimentalApi": true,
            }
        }
    })
}

const BUNDLED_CODEX_EXECUTABLE_NAME: &str = "video-creater-codex";
const CODEX_PATH_RESOURCE: &str = "codex-runtime/codex-path";
const LINUX_BUNDLE_PRODUCT_NAME: &str = "Video Creater";

/// Candidate directories holding the Linux Codex helper tools (`rg`) staged by
/// `scripts/build-codex-sidecar.mjs`. The Tauri Linux resource directory is
/// `<exe dir>/../lib/<productName>` for .deb and AppImage installs and the
/// Cargo output directory during development.
fn linux_codex_path_resource_candidates(
    executable_dir: &Path,
    appdir: Option<&OsStr>,
) -> Vec<PathBuf> {
    let mut candidates = vec![
        executable_dir
            .join("../lib")
            .join(LINUX_BUNDLE_PRODUCT_NAME)
            .join(CODEX_PATH_RESOURCE),
        executable_dir.join(CODEX_PATH_RESOURCE),
    ];
    if let Some(appdir) = appdir {
        candidates.push(
            Path::new(appdir)
                .join("usr/lib")
                .join(LINUX_BUNDLE_PRODUCT_NAME)
                .join(CODEX_PATH_RESOURCE),
        );
    }
    candidates
}

/// Environment overrides for the bundled Linux Codex sidecar. The flat sidecar
/// layout disables Codex's own vendor-tool discovery, so the staged `rg`
/// directory is put ahead of the inherited PATH. That directory never contains
/// `bwrap`; the sandbox keeps using the system `/usr/bin/bwrap`, which Ubuntu's
/// AppArmor policy allows to create unprivileged user namespaces.
fn bundled_codex_environment(
    program: &Path,
    inherited_path: Option<&OsStr>,
    appdir: Option<&OsStr>,
) -> BTreeMap<String, String> {
    let mut environment = BTreeMap::new();
    if !cfg!(target_os = "linux")
        || program.file_name() != Some(OsStr::new(BUNDLED_CODEX_EXECUTABLE_NAME))
    {
        return environment;
    }
    let Some(executable_dir) = program.parent() else {
        return environment;
    };
    let Some(tools_dir) = linux_codex_path_resource_candidates(executable_dir, appdir)
        .into_iter()
        .find(|candidate| is_path_executable(&candidate.join("rg")))
    else {
        return environment;
    };
    let mut paths = vec![tools_dir];
    if let Some(inherited_path) = inherited_path {
        paths.extend(env::split_paths(inherited_path));
    }
    if let Ok(joined) = env::join_paths(paths) {
        if let Some(joined) = joined.to_str() {
            environment.insert("PATH".to_string(), joined.to_string());
        }
    }
    environment
}

impl StdioCodexAppServerTransport {
    pub fn spawn(command: &CodexAppServerCommand) -> Result<Self, CodexAppServerError> {
        Self::spawn_in(command, None)
    }

    pub fn spawn_in(
        command: &CodexAppServerCommand,
        cwd: Option<&Path>,
    ) -> Result<Self, CodexAppServerError> {
        let command = resolve_codex_app_server_command(&command.program)?;
        let program = PathBuf::from(command.program);
        let environment = bundled_codex_environment(
            &program,
            env::var_os("PATH").as_deref(),
            env::var_os("APPDIR").as_deref(),
        );
        let session = spawn_framed_session(
            SupervisedCommand {
                program,
                args: command.args,
                cwd: cwd.map(Path::to_path_buf),
                clear_env: false,
                env: environment,
                env_remove: Vec::new(),
                stdin: Vec::new(),
                stdout_limit: 0,
                stderr_limit: 64 * 1024,
            },
            SupervisionPolicy {
                deadline: APP_SERVER_TURN_TIMEOUT,
                ..SupervisionPolicy::default()
            },
            APP_SERVER_MAX_LINE_BYTES,
        )
        .map_err(map_supervision_error)?;
        Ok(Self { session })
    }

    pub fn spawn_until(
        command: &CodexAppServerCommand,
        cwd: Option<&Path>,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<Self, CodexAppServerError> {
        if cancellation.is_some_and(CancellationSignal::is_cancelled) {
            return Err(CodexAppServerError::Interrupted);
        }
        if Instant::now() >= deadline {
            return Err(CodexAppServerError::Deadline);
        }
        let mut transport = Self::spawn_in(command, cwd)?;
        if cancellation.is_some_and(CancellationSignal::is_cancelled) {
            let _ = transport.terminate();
            return Err(CodexAppServerError::Interrupted);
        }
        if Instant::now() >= deadline {
            let _ = transport.terminate();
            return Err(CodexAppServerError::Deadline);
        }
        Ok(transport)
    }
}

impl CodexAppServerTransport for StdioCodexAppServerTransport {
    fn send(&mut self, message: Value) -> Result<(), CodexAppServerError> {
        self.session
            .send_json(&message)
            .map_err(map_supervision_error)
    }

    fn send_until(
        &mut self,
        message: Value,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<(), CodexAppServerError> {
        self.session
            .send_json_until(&message, deadline, cancellation)
            .map_err(map_supervision_error)
    }

    fn recv_until(&mut self, deadline: Instant) -> Result<AppServerMessage, CodexAppServerError> {
        let value = self
            .session
            .recv_json_until(deadline)
            .map_err(map_supervision_error)?;
        decode_app_server_message(value)
    }

    fn terminate(&mut self) -> Result<AppServerCleanupReport, CodexAppServerError> {
        let CleanupReport {
            reaped,
            stderr,
            stderr_truncated,
        } = self.session.terminate_and_reap();
        Ok(AppServerCleanupReport {
            reaped,
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            stderr_truncated,
        })
    }
}

fn map_supervision_error(error: SupervisionError) -> CodexAppServerError {
    match error {
        SupervisionError::Deadline { .. } => CodexAppServerError::Deadline,
        SupervisionError::Cancelled { .. } => CodexAppServerError::Interrupted,
        other => CodexAppServerError::Transport(other.to_string()),
    }
}

pub fn decode_app_server_message(message: Value) -> Result<AppServerMessage, CodexAppServerError> {
    let id = message.get("id").cloned();
    if let Some(method) = message.get("method").and_then(Value::as_str) {
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        return Ok(match id {
            Some(id) => AppServerMessage::ServerRequest {
                id,
                method: method.to_string(),
                params,
            },
            None => AppServerMessage::Notification {
                method: method.to_string(),
                params,
            },
        });
    }
    let id = id.ok_or_else(|| {
        CodexAppServerError::Transport("app-server message has neither method nor id".into())
    })?;
    if let Some(error) = message.get("error") {
        return Ok(AppServerMessage::ErrorResponse {
            id,
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown app-server error")
                .to_string(),
        });
    }
    let result = message
        .get("result")
        .cloned()
        .ok_or(CodexAppServerError::MissingResult)?;
    Ok(AppServerMessage::Response { id, result })
}

pub fn decode_app_server_response_result(
    expected_id: u64,
    message: Value,
) -> Result<Value, CodexAppServerError> {
    if message.get("id").and_then(Value::as_u64) != Some(expected_id) {
        return Err(CodexAppServerError::ResponseIdMismatch);
    }

    if let Some(error) = message.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown app-server error")
            .to_string();
        return Err(CodexAppServerError::RpcError { code, message });
    }

    message
        .get("result")
        .cloned()
        .ok_or(CodexAppServerError::MissingResult)
}

pub fn decode_app_server_initialize_response(
    expected_id: u64,
    message: Value,
) -> Result<Value, CodexAppServerError> {
    let result = decode_app_server_response_result(expected_id, message)?;
    ensure_app_server_initialize_result(result)
}

pub fn initialize_codex_app_server<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id: u64,
) -> Result<Value, CodexAppServerError> {
    initialize_codex_app_server_until(transport, request_id, codex_app_server_deadline(), None)
}

pub fn initialize_codex_app_server_until<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id: u64,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<Value, CodexAppServerError> {
    let mut retained = VecDeque::new();
    let result = send_request_wait(
        transport,
        build_app_server_initialize_request(request_id),
        deadline,
        &mut retained,
        cancellation,
    )?;
    ensure_app_server_initialize_result(result)
}

/// Sends one app-server request while continuing to service interleaved
/// notifications and server requests until the matching response arrives.
pub fn request_codex_app_server<T: CodexAppServerTransport>(
    transport: &mut T,
    request: Value,
    deadline: Instant,
) -> Result<Value, CodexAppServerError> {
    let mut retained = VecDeque::new();
    send_request_wait(transport, request, deadline, &mut retained, None)
}

pub fn request_codex_app_server_until<T: CodexAppServerTransport>(
    transport: &mut T,
    request: Value,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<Value, CodexAppServerError> {
    let mut retained = VecDeque::new();
    send_request_wait(transport, request, deadline, &mut retained, cancellation)
}

fn ensure_app_server_initialize_result(result: Value) -> Result<Value, CodexAppServerError> {
    if result.is_object() {
        Ok(result)
    } else {
        Err(CodexAppServerError::IncompatibleProtocol(
            "initialize result must be a JSON object".to_string(),
        ))
    }
}

fn retain_event(retained: &mut VecDeque<AppServerMessage>, message: AppServerMessage) {
    let message_bytes = format!("{message:?}").len();
    while retained.len() >= APP_SERVER_MAX_RETAINED_EVENTS
        || retained
            .iter()
            .map(|item| format!("{item:?}").len())
            .sum::<usize>()
            + message_bytes
            > APP_SERVER_MAX_FINAL_TEXT_BYTES
    {
        if retained.pop_front().is_none() {
            break;
        }
    }
    retained.push_back(message);
}

fn respond_to_server_request<T: CodexAppServerTransport>(
    transport: &mut T,
    id: Value,
    method: &str,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<bool, CodexAppServerError> {
    let is_approval = matches!(
        method,
        "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "execCommandApproval"
            | "applyPatchApproval"
    );
    if is_approval {
        transport.send_until(
            json!({ "id": id, "result": { "decision": "decline" } }),
            deadline,
            cancellation,
        )?;
        return Ok(true);
    }
    transport.send_until(
        json!({
            "id": id,
            "error": { "code": -32601, "message": format!("unsupported app-server request: {method}") }
        }),
        deadline,
        cancellation,
    )?;
    Ok(false)
}

fn interrupt_and_terminate<T: CodexAppServerTransport>(
    transport: &mut T,
    thread_id: &str,
    turn_id: Option<&str>,
) {
    if let Some(turn_id) = turn_id {
        let grace_deadline = Instant::now() + APP_SERVER_INTERRUPT_GRACE;
        let _ = transport.send_until(
            json!({
                "id": format!("interrupt-{turn_id}"),
                "method": "turn/interrupt",
                "params": {"threadId": thread_id, "turnId": turn_id}
            }),
            grace_deadline,
            None,
        );
        if let Ok(AppServerMessage::ServerRequest { id, method, .. }) =
            transport.recv_until(grace_deadline)
        {
            let _ = respond_to_server_request(transport, id, &method, grace_deadline, None);
        }
    }
    let _ = transport.terminate();
}

pub(crate) fn send_request_wait<T: CodexAppServerTransport>(
    transport: &mut T,
    request: Value,
    deadline: Instant,
    retained: &mut VecDeque<AppServerMessage>,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<Value, CodexAppServerError> {
    let expected_id = request
        .get("id")
        .cloned()
        .ok_or_else(|| CodexAppServerError::Transport("request id is required".into()))?;
    if let Err(error) = transport.send_until(request, deadline, cancellation) {
        let _ = transport.terminate();
        return Err(error);
    }
    loop {
        if cancellation.is_some_and(CancellationSignal::is_cancelled) {
            let _ = transport.terminate();
            return Err(CodexAppServerError::Interrupted);
        }
        if Instant::now() >= deadline {
            let _ = transport.terminate();
            return Err(CodexAppServerError::Deadline);
        }
        let slice = deadline.min(Instant::now() + APP_SERVER_POLL_INTERVAL);
        let message = match transport.recv_until(slice) {
            Err(CodexAppServerError::Deadline) if Instant::now() < deadline => continue,
            Err(error) => {
                let _ = transport.terminate();
                return Err(error);
            }
            Ok(message) => message,
        };
        match message {
            AppServerMessage::Response { id, result } if id == expected_id => return Ok(result),
            AppServerMessage::ErrorResponse { id, code, message } if id == expected_id => {
                return Err(CodexAppServerError::RpcError { code, message });
            }
            AppServerMessage::ServerRequest { id, method, params } => {
                let supported =
                    match respond_to_server_request(transport, id, &method, deadline, cancellation)
                    {
                        Ok(supported) => supported,
                        Err(error) => {
                            let _ = transport.terminate();
                            return Err(error);
                        }
                    };
                retain_event(
                    retained,
                    AppServerMessage::ServerRequest {
                        id: Value::Null,
                        method: format!("{method}:responded={supported}"),
                        params,
                    },
                );
            }
            other => retain_event(retained, other),
        }
    }
}

fn turn_from_start_response(response: &Value) -> Result<Value, CodexAppServerError> {
    response
        .get("turn")
        .cloned()
        .ok_or(CodexAppServerError::MissingTurnId)
}

fn append_bounded_text(target: &mut String, delta: &str) {
    let remaining = APP_SERVER_MAX_FINAL_TEXT_BYTES.saturating_sub(target.len());
    if remaining == 0 {
        return;
    }
    let end = delta
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= remaining)
        .last()
        .unwrap_or(0);
    if delta.len() <= remaining {
        target.push_str(delta);
    } else if end > 0 {
        target.push_str(&delta[..end]);
    }
}

fn collect_agent_message(item: &Value, messages: &mut Vec<Value>) {
    if item.get("type").and_then(Value::as_str) == Some("agentMessage") {
        let item_bytes = serde_json::to_vec(item)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        while messages
            .iter()
            .map(|message| {
                serde_json::to_vec(message)
                    .map(|bytes| bytes.len())
                    .unwrap_or(0)
            })
            .sum::<usize>()
            + item_bytes
            > APP_SERVER_MAX_FINAL_TEXT_BYTES
        {
            if messages.is_empty() {
                return;
            }
            messages.remove(0);
        }
        messages.push(item.clone());
        if messages.len() > APP_SERVER_MAX_RETAINED_EVENTS {
            messages.remove(0);
        }
    }
}

fn finish_completed_turn(
    start_response: Value,
    terminal_turn: Value,
    mut completed_messages: Vec<Value>,
    delta_text: String,
    retained: VecDeque<AppServerMessage>,
) -> Result<CompletedCodexTurn, CodexAppServerError> {
    match terminal_turn.get("status").and_then(Value::as_str) {
        Some("completed") => {}
        Some("interrupted") => return Err(CodexAppServerError::Interrupted),
        Some("failed") => {
            return Err(CodexAppServerError::TurnFailed(
                terminal_turn
                    .get("error")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "unknown error".into()),
            ))
        }
        Some(status) => {
            return Err(CodexAppServerError::TurnFailed(format!(
                "unexpected terminal status {status}"
            )))
        }
        None => {
            return Err(CodexAppServerError::TurnFailed(
                "terminal turn omitted status".into(),
            ))
        }
    }
    if let Some(items) = terminal_turn.get("items").and_then(Value::as_array) {
        for item in items {
            collect_agent_message(item, &mut completed_messages);
        }
    }
    let final_text = completed_messages
        .iter()
        .rev()
        .find(|item| item.get("phase").and_then(Value::as_str) == Some("final_answer"))
        .or_else(|| completed_messages.last())
        .and_then(|item| item.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or(delta_text);
    Ok(CompletedCodexTurn {
        start_response,
        terminal_turn,
        final_agent_messages: completed_messages,
        final_text,
        retained_messages: retained.into(),
    })
}

pub fn run_codex_turn_pump<T: CodexAppServerTransport>(
    transport: &mut T,
    request: Value,
    thread_id: &str,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<CompletedCodexTurn, CodexAppServerError> {
    let expected_id = request
        .get("id")
        .cloned()
        .ok_or_else(|| CodexAppServerError::Transport("request id is required".into()))?;
    if let Err(error) = transport.send_until(request, deadline, cancellation) {
        let _ = transport.terminate();
        return Err(error);
    }
    let mut retained = VecDeque::new();
    let mut start_response = None;
    let mut turn_id = None::<String>;
    let mut completed_messages = Vec::new();
    let mut delta_text = String::new();
    loop {
        if cancellation.is_some_and(CancellationSignal::is_cancelled) {
            interrupt_and_terminate(transport, thread_id, turn_id.as_deref());
            return Err(CodexAppServerError::Interrupted);
        }
        if Instant::now() >= deadline {
            interrupt_and_terminate(transport, thread_id, turn_id.as_deref());
            return Err(CodexAppServerError::Deadline);
        }
        let slice = deadline.min(Instant::now() + APP_SERVER_POLL_INTERVAL);
        let message = match transport.recv_until(slice) {
            Err(CodexAppServerError::Deadline) if Instant::now() < deadline => continue,
            Err(error) => {
                let _ = transport.terminate();
                return Err(error);
            }
            Ok(message) => message,
        };
        match message {
            AppServerMessage::Response { id, result } if id == expected_id => {
                let turn = turn_from_start_response(&result)?;
                let id = turn
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or(CodexAppServerError::MissingTurnId)?
                    .to_string();
                turn_id = Some(id);
                start_response = Some(result);
                if matches!(
                    turn.get("status").and_then(Value::as_str),
                    Some("completed" | "failed" | "interrupted")
                ) {
                    return finish_completed_turn(
                        start_response.unwrap(),
                        turn,
                        completed_messages,
                        delta_text,
                        retained,
                    );
                }
            }
            AppServerMessage::ErrorResponse { id, code, message } if id == expected_id => {
                return Err(CodexAppServerError::RpcError { code, message })
            }
            AppServerMessage::ServerRequest { id, method, params } => {
                let supported =
                    match respond_to_server_request(transport, id, &method, deadline, cancellation)
                    {
                        Ok(supported) => supported,
                        Err(error) => {
                            interrupt_and_terminate(transport, thread_id, turn_id.as_deref());
                            return Err(error);
                        }
                    };
                retain_event(
                    &mut retained,
                    AppServerMessage::ServerRequest {
                        id: Value::Null,
                        method: method.clone(),
                        params,
                    },
                );
                if !supported {
                    interrupt_and_terminate(transport, thread_id, turn_id.as_deref());
                    return Err(CodexAppServerError::Transport(format!(
                        "unsupported app-server request: {method}"
                    )));
                }
            }
            AppServerMessage::Notification { method, params } => {
                let matches_turn = params.get("threadId").and_then(Value::as_str)
                    == Some(thread_id)
                    && turn_id.as_deref().is_some_and(|id| {
                        params.get("turnId").and_then(Value::as_str) == Some(id)
                            || params.pointer("/turn/id").and_then(Value::as_str) == Some(id)
                    });
                if matches_turn && method == "item/agentMessage/delta" {
                    if let Some(delta) = params.get("delta").and_then(Value::as_str) {
                        append_bounded_text(&mut delta_text, delta);
                    }
                } else if matches_turn && method == "item/completed" {
                    if let Some(item) = params.get("item") {
                        collect_agent_message(item, &mut completed_messages);
                    }
                } else if matches_turn && method == "turn/completed" {
                    let terminal = params.get("turn").cloned().ok_or_else(|| {
                        CodexAppServerError::TurnFailed("turn/completed omitted turn".into())
                    })?;
                    return finish_completed_turn(
                        start_response.ok_or(CodexAppServerError::MissingTurnId)?,
                        terminal,
                        completed_messages,
                        delta_text,
                        retained,
                    );
                }
                retain_event(
                    &mut retained,
                    AppServerMessage::Notification { method, params },
                );
            }
            other => retain_event(&mut retained, other),
        }
    }
}

pub fn start_codex_video_edit_turn<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: EditJobRequest,
    skills: &super::context::ProjectSkillBundle,
    project_dir: Option<&Path>,
) -> Result<CodexVideoEditTurnResult, CodexAppServerError> {
    start_codex_video_edit_turn_internal(
        transport,
        request_id_start,
        cwd,
        project,
        request,
        skills,
        project_dir,
        true,
        codex_app_server_deadline(),
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the unpersisted turn entry point mirrors the internal turn parameters"
)]
pub fn start_codex_video_edit_turn_unpersisted_until<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: EditJobRequest,
    skills: &super::context::ProjectSkillBundle,
    project_dir: Option<&Path>,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<CodexVideoEditTurnResult, CodexAppServerError> {
    start_codex_video_edit_turn_internal(
        transport,
        request_id_start,
        cwd,
        project,
        request,
        skills,
        project_dir,
        false,
        deadline,
        cancellation,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "persisted and unpersisted turn entry points share this parameter set"
)]
fn start_codex_video_edit_turn_internal<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: EditJobRequest,
    skills: &super::context::ProjectSkillBundle,
    project_dir: Option<&Path>,
    persist_conversation: bool,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<CodexVideoEditTurnResult, CodexAppServerError> {
    initialize_codex_app_server_until(transport, request_id_start, deadline, cancellation)?;

    let thread_action = next_turn_thread_action(project, project_dir)?;
    let thread_request =
        build_video_thread_request(request_id_start + 1, thread_action, cwd, skills);
    let mut retained = VecDeque::new();
    let thread_response = send_request_wait(
        transport,
        thread_request,
        deadline,
        &mut retained,
        cancellation,
    )?;
    let thread_id = extract_thread_id(&thread_response)?;
    project.codex_thread_id = Some(thread_id.clone());

    let context = match project_dir {
        Some(project_dir) => super::context::build_video_edit_context_with_project_dir(
            project,
            &request,
            project_dir,
        ),
        None => super::context::build_video_edit_context(project, &request),
    }
    .map_err(CodexAppServerError::Context)?;
    let turn_request = build_video_edit_turn_request(request_id_start + 2, &thread_id, &context);
    let model_choice = choose_codex_turn_model(
        transport,
        request_id_start,
        &thread_response,
        deadline,
        cancellation,
    )?;
    let (completed, model_notice) = run_codex_turn_with_supported_model(
        transport,
        request_id_start,
        turn_request,
        &thread_id,
        &model_choice,
        deadline,
        cancellation,
    )?;
    let turn_response = completed.terminal_turn;
    let proposal =
        proposal_from_value(&turn_response).or_else(|| proposal_from_text(&completed.final_text));
    let proposal_validation_issues = proposal.as_ref().map(|proposal| {
        validate_codex_edit_proposal(project, &request, proposal)
            .err()
            .map(|error| vec![proposal_validation_issue(&error)])
            .unwrap_or_default()
    });
    if persist_conversation {
        if let Some(project_dir) = project_dir {
            record_app_server_conversation_turn(
                project_dir,
                &project.id,
                &thread_id,
                AppServerConversationTurn {
                    turn_id: extract_turn_string(&turn_response, "id"),
                    turn_status: extract_turn_string(&turn_response, "status"),
                    prompt: request.prompt.clone(),
                    created_at: request.created_at.clone(),
                    request: serde_json::to_value(&request)
                        .map_err(|error| CodexAppServerError::Json(error.to_string()))?,
                    thread_response: thread_response.clone(),
                    turn_response: turn_response.clone(),
                    has_proposal: proposal.is_some(),
                    // The Codex path keeps naming its chat by `thread_id`.
                    provider: None,
                    provider_session_id: None,
                },
            )
            .map_err(|error| CodexAppServerError::Transport(error.to_string()))?;
        }
    }

    Ok(CodexVideoEditTurnResult {
        thread_id,
        thread_response,
        turn_response,
        proposal,
        proposal_validation_issues,
        model_notice,
    })
}

pub fn start_codex_conversation_turn<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: CodexConversationEditRequest,
    skills: &super::context::ProjectSkillBundle,
    project_dir: Option<&Path>,
) -> Result<CodexConversationTurnResult, CodexAppServerError> {
    start_codex_conversation_turn_internal(
        transport,
        request_id_start,
        cwd,
        project,
        request,
        skills,
        project_dir,
        true,
        codex_app_server_deadline(),
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the unpersisted conversation entry point mirrors the internal turn parameters"
)]
pub fn start_codex_conversation_turn_unpersisted_until<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: CodexConversationEditRequest,
    skills: &super::context::ProjectSkillBundle,
    project_dir: Option<&Path>,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<CodexConversationTurnResult, CodexAppServerError> {
    start_codex_conversation_turn_internal(
        transport,
        request_id_start,
        cwd,
        project,
        request,
        skills,
        project_dir,
        false,
        deadline,
        cancellation,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "persisted and unpersisted conversation entry points share this parameter set"
)]
fn start_codex_conversation_turn_internal<T: CodexAppServerTransport>(
    transport: &mut T,
    request_id_start: u64,
    cwd: &str,
    project: &mut VideoProject,
    request: CodexConversationEditRequest,
    skills: &super::context::ProjectSkillBundle,
    project_dir: Option<&Path>,
    persist_conversation: bool,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<CodexConversationTurnResult, CodexAppServerError> {
    let request = request
        .validate(project)
        .map_err(CodexAppServerError::ConversationRequest)?;
    initialize_codex_app_server_until(transport, request_id_start, deadline, cancellation)?;

    let thread_action = next_turn_thread_action(project, project_dir)?;
    let thread_request =
        build_video_thread_request(request_id_start + 1, thread_action, cwd, skills);
    let mut retained = VecDeque::new();
    let thread_response = send_request_wait(
        transport,
        thread_request,
        deadline,
        &mut retained,
        cancellation,
    )?;
    let thread_id = extract_thread_id(&thread_response)?;
    project.codex_thread_id = Some(thread_id.clone());

    let context = build_codex_conversation_context(project, &request, project_dir);
    let turn_request =
        build_codex_conversation_turn_request(request_id_start + 2, &thread_id, &context);
    let model_choice = choose_codex_turn_model(
        transport,
        request_id_start,
        &thread_response,
        deadline,
        cancellation,
    )?;
    let (completed, model_notice) = run_codex_turn_with_supported_model(
        transport,
        request_id_start,
        turn_request,
        &thread_id,
        &model_choice,
        deadline,
        cancellation,
    )?;
    let turn_response = completed.terminal_turn;
    let proposal = conversation_proposal_from_value(&turn_response)
        .or_else(|| conversation_proposal_from_text(&completed.final_text));
    let (prepared_proposal, proposal_validation_issues) = match proposal.as_ref() {
        None => (None, None),
        Some(proposal) => match prepare_codex_conversation_proposal(project, proposal) {
            Ok(prepared) => (Some(prepared), Some(Vec::new())),
            Err(error) => (None, Some(vec![error.validation_issue()])),
        },
    };
    let thread_response = without_conversation_user_input(thread_response);
    let turn_response = without_conversation_user_input(turn_response);
    if persist_conversation {
        if let Some(project_dir) = project_dir {
            record_app_server_conversation_turn(
                project_dir,
                &project.id,
                &thread_id,
                AppServerConversationTurn {
                    turn_id: extract_turn_string(&turn_response, "id"),
                    turn_status: extract_turn_string(&turn_response, "status"),
                    prompt: request.prompt.clone(),
                    created_at: request.created_at.clone(),
                    request: serde_json::to_value(&request)
                        .map_err(|error| CodexAppServerError::Json(error.to_string()))?,
                    thread_response: thread_response.clone(),
                    turn_response: turn_response.clone(),
                    has_proposal: proposal.is_some(),
                    // The Codex path keeps naming its chat by `thread_id`.
                    provider: None,
                    provider_session_id: None,
                },
            )
            .map_err(|error| CodexAppServerError::Transport(error.to_string()))?;
        }
    }

    Ok(CodexConversationTurnResult {
        thread_id,
        thread_response,
        turn_response,
        proposal,
        prepared_proposal,
        proposal_validation_issues,
        model_notice,
    })
}

fn proposal_validation_issue(error: &CodexProposalError) -> ProjectValidationIssue {
    let (path, fix) = match error {
        CodexProposalError::InvalidRequest(_) => (
            "request",
            "Fix the edit request before reviewing the proposal",
        ),
        CodexProposalError::MediaIdMismatch => (
            "mediaId",
            "Use the requested canonical media id as the proposal media id",
        ),
        CodexProposalError::MediaNotFound(_) => (
            "clips",
            "Use canonical media ids that exist in the project library",
        ),
        CodexProposalError::Edl(_) => (
            "clips",
            "Select a real rough cut that omits source material before adding visual layers",
        ),
        CodexProposalError::IncompleteRenderReview
        | CodexProposalError::InvalidRenderReviewDuration(_) => (
            "renderReview",
            "Supply complete render-review criteria that match the EDL duration",
        ),
        CodexProposalError::UnknownTemplateId(_)
        | CodexProposalError::EmptyTemplateField(_)
        | CodexProposalError::MissingTemplateMetadata(_)
        | CodexProposalError::UnsupportedTemplateKind(_)
        | CodexProposalError::InvalidTemplateTiming(_)
        | CodexProposalError::InvalidOverlayTiming(_)
        | CodexProposalError::BannedVisualTreatment("overlay", _) => (
            "overlays",
            "Fix the overlay kind, timing, template fields, and visual metadata",
        ),
        CodexProposalError::MissingHyperframeMetadata(_)
        | CodexProposalError::UnsupportedHyperframeKind(_)
        | CodexProposalError::InvalidHyperframeTiming(_)
        | CodexProposalError::BannedVisualTreatment("hyperframe", _) => (
            "hyperframes",
            "Fix the HyperFrame kind, timing, and visual metadata",
        ),
        CodexProposalError::InvalidGpuVisualTiming(_) => (
            "gpuVisuals",
            "Fix the GPU visual timing so it stays within the EDL",
        ),
        CodexProposalError::InvalidCaptionTiming(_) => {
            ("captions", "Fix caption timing so it stays within the EDL")
        }
        CodexProposalError::ProjectAction(_) | CodexProposalError::TrackNotFound(_) => (
            "projectActions",
            "Use canonical unlocked targets and valid project actions",
        ),
        CodexProposalError::BannedVisualTreatment(_, _) => (
            "layers",
            "Replace the banned generic treatment with a concrete visual treatment",
        ),
    };
    ProjectValidationIssue {
        path: path.to_string(),
        message: error.to_string(),
        fix: fix.to_string(),
    }
}

pub fn proposal_from_value(value: &Value) -> Option<CodexEditProposal> {
    serde_json::from_value::<CodexEditProposal>(value.clone())
        .ok()
        .or_else(|| {
            value.get("proposal").and_then(|proposal| {
                serde_json::from_value::<CodexEditProposal>(proposal.clone()).ok()
            })
        })
        .or_else(|| value.get("structuredOutput").and_then(proposal_from_value))
        .or_else(|| value.get("output").and_then(proposal_from_value))
        .or_else(|| {
            value
                .get("output")
                .and_then(Value::as_str)
                .and_then(proposal_from_text)
        })
}

pub fn proposal_from_text(text: &str) -> Option<CodexEditProposal> {
    let trimmed = trim_json_markdown(text.trim());
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<CodexEditProposal>(trimmed)
        .ok()
        .or_else(|| {
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .and_then(|value| proposal_from_value(&value))
        })
}

fn trim_json_markdown(text: &str) -> &str {
    text.strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .or_else(|| {
            text.strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
        })
        .map(str::trim)
        .unwrap_or(text)
}

/// Resumes the active chat's Codex thread, so a new chat's turns are not filed under the first
/// chat, and starts a thread when the chat has none. A turn without a project folder (a project
/// that has no chat manifest to read) keeps using the project's own thread.
fn next_turn_thread_action(
    project: &VideoProject,
    project_dir: Option<&Path>,
) -> Result<CodexThreadAction, CodexAppServerError> {
    let thread_id = match project_dir {
        Some(project_dir) => codex_thread_for_next_turn(project_dir, project)
            .map_err(|error| CodexAppServerError::Transport(error.to_string()))?,
        None => project.codex_thread_id.clone(),
    };
    Ok(thread_id
        .map(|thread_id| CodexThreadAction::Resume { thread_id })
        .unwrap_or(CodexThreadAction::Start))
}

pub fn build_video_thread_request(
    request_id: u64,
    action: CodexThreadAction,
    cwd: &str,
    skills: &super::context::ProjectSkillBundle,
) -> Value {
    build_agent_thread_request(
        request_id,
        action,
        cwd,
        &build_codex_developer_instructions(skills),
    )
}

/// The same thread request, from instructions that have already been rendered.
///
/// The transport seam hands a backend its developer instructions as a string, because
/// rendering them from the project's skills is the caller's job, not the transport's.
pub fn build_agent_thread_request(
    request_id: u64,
    action: CodexThreadAction,
    cwd: &str,
    developer_instructions: &str,
) -> Value {
    let mut params = json!({
        "cwd": cwd,
        "developerInstructions": developer_instructions,
        "sandbox": "read-only",
        "approvalPolicy": "never",
        "personality": "pragmatic",
    });

    let method = match action {
        CodexThreadAction::Start => {
            params["sessionStartSource"] = json!("startup");
            params["threadSource"] = json!("user");
            params["ephemeral"] = json!(false);
            "thread/start"
        }
        CodexThreadAction::Resume { thread_id } => {
            params["threadId"] = json!(thread_id);
            "thread/resume"
        }
    };

    json!({
        "id": request_id,
        "method": method,
        "params": params,
    })
}

pub fn build_video_edit_turn_request(
    request_id: u64,
    thread_id: &str,
    context: &VideoEditContext,
) -> Value {
    json!({
        "id": request_id,
        "method": "turn/start",
        "params": {
            "threadId": thread_id,
            "input": [
                {
                    "type": "text",
                    "text": build_video_edit_prompt(context),
                }
            ],
            "approvalPolicy": "never",
            "outputSchema": codex_edit_proposal_output_schema(),
        }
    })
}

pub fn build_codex_conversation_turn_request(
    request_id: u64,
    thread_id: &str,
    context: &CodexConversationContext,
) -> Value {
    build_agent_turn_request(
        request_id,
        thread_id,
        &build_codex_conversation_prompt(context),
        codex_conversation_proposal_output_schema(),
    )
}

/// The same turn request, from a prompt and a schema that are already rendered.
pub fn build_agent_turn_request(
    request_id: u64,
    thread_id: &str,
    prompt: &str,
    output_schema: Value,
) -> Value {
    json!({
        "id": request_id,
        "method": "turn/start",
        "params": {
            "threadId": thread_id,
            "input": [
                {
                    "type": "text",
                    "text": prompt,
                }
            ],
            "approvalPolicy": "never",
            "outputSchema": output_schema,
        }
    })
}

fn build_video_edit_prompt(context: &VideoEditContext) -> String {
    let shadertoy_profiles = shadertoy_profile_templates()
        .into_iter()
        .map(|template| {
            format!(
                "- {profile_id}: {title}. kind shader_background. {visual_treatment}",
                profile_id = template.profile_id,
                title = template.title,
                visual_treatment = template.visual_treatment
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\
Return only a structured Codex edit proposal that matches the provided JSON schema.

Project:
- id: {project_id}
- name: {project_name}

Project files:
{project_files_summary}

Edit request:
- mediaId: {media_id}
- preset: {preset:?}
- targetDurationSeconds: {target_duration}
- languageMode: {language_mode}
- captionStyle: {caption_style:?}
- prompt: {prompt}

Project library:
{media_library_summary}

Project timeline:
{timeline_summary}

Generated assets:
{generated_assets_summary}

Project template overrides:
{template_overrides_summary}

Render reports:
{render_reports_summary}

Workflow jobs:
{workflow_jobs_summary}

Export artifacts:
{export_artifacts_summary}

Export capabilities:
{export_capabilities_summary}

Media:
- relativePath: {relative_path}
- durationSeconds: {duration:.3}
- dimensions: {width}x{height}
- fps: {fps}

Transcript excerpt:
{transcript_excerpt}

Available motion templates:
- id: kinetic-lower-third-v1
  name: Kinetic Lower Third
  category: lower_thirds
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 2.4
  required fields: headline, subline
  visualTreatment: compact lower-third block with translucent backing, accent rule, and strong hierarchy
  motion: slide-and-fade in over 8 frames, hold, then soft fade out
  safeZone: keep essential text inside 10% margins and below face/action priority areas
  avoid: full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds
- id: punchy-caption-v1
  name: Punchy Caption
  category: captions
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 1.8
  required fields: headline
  visualTreatment: large phone-readable caption lockup with accent underline and soft backing
  motion: scale pop in over 5 frames, underline wipe, then snap fade out
  safeZone: keep caption block inside 10% margins and above bottom controls
  avoid: subtitle slabs, tiny type, centered static paragraphs, and covering faces
- id: metric-callout-v1
  name: Metric Callout
  category: callouts
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 2.2
  required fields: headline, subline
  visualTreatment: floating metric tile with high-contrast number, caption, and directional accent
  motion: count-up feel, accent sweep, hold, then slide out
  safeZone: keep tile inside 10% margins and away from lower captions
  avoid: spreadsheet-like boxes, decorative-only badges, and unreadable dense labels
- id: chapter-card-v1
  name: Chapter Card
  category: titles
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 2.6
  required fields: headline, subline
  visualTreatment: left-weighted chapter marker with translucent panel and vertical reveal line
  motion: vertical line wipe, text type-on, short hold, then mask out
  safeZone: keep text inside 10% margins and leave center action visible
  avoid: full-frame static slides, plain centered text, and long title holds
- id: tracking-highlight-v1
  name: Tracking Highlight
  category: callouts
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 1.6
  required fields: headline
  visualTreatment: thin tracking ring with compact label and pointer line
  motion: ring draws on, label slides from pointer, then both fade
  safeZone: keep label inside 10% margins while pointer can track the visual target
  avoid: large opaque callout boxes, covering hands or product details, and static arrows
- id: holographic-logo-cutout-v1
  name: Holographic Logo Cutout
  category: titles
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 3.2
  required fields: logoAssetId
  default logoAssetId: builtin:v-photo-light
  visualTreatment: full-frame dark gradient background with a crisp holographic logo cutout, pearlescent shader bands, and fine controlled grain
  motion: shader shimmer drifts through the logo mask with a clean first-frame hold and no bevel, fake depth, or rectangular light bars
  safeZone: keep the logo inside the central 80% safe zone with no essential detail near edges
  avoid: plain boxes, static text-only cards, default-font logo substitutes, opaque caption slabs, harsh strobes, and long unmoving holds
- id: gradient-background-loop-v1
  name: Gradient Background Loop
  category: text
  kind: overlay
  intendedTrack: Overlays
  defaultDurationSeconds: 4.0
  required fields: headline
  default headline: Love\\nwins.
  visualTreatment: full-frame vertical blue gradient panels with oversized bold white text centered across the columns
  motion: seamless loop with subtle vertical panel drift, breathing blue gradients, and a steady high-contrast text hold
  safeZone: keep headline inside 10% margins while allowing the panel background to fill frame
  avoid: plain boxes, opaque caption slabs, tiny text, default-font title cards, hard cuts between panel colors, and strobing

Available motion presets:
- slide-fade-up-v1: lower-third slide/fade entry with soft exit.
- snap-pop-v1: caption scale pop with overshoot and snap fade.
- underline-wipe-v1: accent underline wipe paired with text reveal.
- metric-count-pop-v1: metric tile pop with directional accent sweep.
- vertical-reveal-v1: chapter marker reveal with text type-on feel.
- tracking-draw-v1: highlight ring draw-on with label slide.
- spring-pop-v2: fast scale and opacity entry with bounded overshoot.
- slide-rotate-settle-v2: side entry with subtle rotation settling to zero.
- mask-wipe-v2: rectangular clip reveal with fade-safe edges.
- line-draw-v2: line/path draw-on with optional glow emphasis.
- word-pop-stagger-v2: short word-level text reveal with pop timing.
- soft-depth-card-v2: translucent card depth with shadow, glow, and settle.
- pulse-emphasis-v2: one or two restrained emphasis pulses.
- exit-snap-v2: quick exit with opacity, blur, and directional movement.

Template guidance:
- Prefer templateId, fields, and motionPresetId for overlays before custom nodes.
- Use raw nodes only when no listed template can express the visual beat.

Animated primitive nodes:
- For custom motion graphics, overlays may include nodes with type text, rect, roundedRect, polygon, line, or imageRef.
- Use polygon nodes for filled isometric faces, angled panels, arrows, and diagram surfaces that cannot be expressed with boxes.
- Each node may include animate.keyframes with at values from 0 to 1 and x/y translation offsets from the base box, plus scale, scaleX, scaleY, opacity, rotationDegrees, blurRadius, shadowOpacity, glowOpacity, clipProgress, or pathProgress values.
- Use animate.origin for rotation/scale pivots. Allowed origin keywords are left/center/right for x and top/center/bottom for y, or finite pixel coordinates.
- Text nodes may include textReveal with mode whole, line, word, or character; reserve character reveal for title-card style beats, not ordinary subtitle captions.
- Supported animate.ease values: linear, inQuad, outQuad, inOutQuad, inCubic, outCubic, inOutCubic, outBack, outElastic.
- Use animated primitive nodes for agent-created graphics that templates cannot express; keep node ids stable and errors actionable.

GPU visuals:
- Use gpuVisuals only for generated full-frame or mostly full-frame shader/3D beats after the EDL is real.
- Default GPU visual quality profile: hq-neon-wireframe-shader-v1.
- Use kind: hybrid_scene and set qualityProfile to hq-neon-wireframe-shader-v1.
- For that default, use a restrained animated gradient shader background, crisp neon wireframe 3D primitives, perspective depth, and subtle motion trails.
- Rust expands the canonical shader background and wireframe cube/grid primitive scene for this profile.
- Collected Shadertoy background profiles use kind: shader_background and one of these qualityProfile values:
{shadertoy_profiles}
- Built-in shader background configs live at src-tauri/assets/shader-background-templates/builtin/<template-id>/template.json with shader.frag beside them.
- User-generated shader backgrounds must use the same template.json plus shader.frag shape with sourceKind user in a project-local shader-background-templates folder before Rust loads them.
- Avoid oversized saturated blobs, low-resolution draft looks, filled abstract shapes that do not read as 3D, and heavy compression artifacts.
- Include timing, sourceBeat, dimensions, positive frameRate, alpha, visualTreatment, motion, safeZone, and avoid.
- Do not provide raw GLSL, shader, or primitives for these profiles; Rust owns each canonical GPU scene.
- Avoid strobing, unsafe high-frequency noise, unreadable patterns, and generic static slides.

ProjectAction workflow:
- Use projectActions for direct project-file edits that should be reviewable and replayable against split project files.
- Return an ordered projectActions array after the EDL and visual layer proposal; Rust validates every action against a cloned project before anything is written.
- Use [] when no direct project-file action is needed.
- Supported ProjectAction type values: addItems, insertItems, removeItems, moveItems, reorderItems, resizeItems, trimItems, rippleDeleteRanges, splitItems, createTrack, setTrackLocked, setTrackEnabled, removeTracks, editCaptionText, editTextItem, updateAudioFades, updateAudioFadeOut, updateAudioVolume, updateAudioClipSpeed, detachAudio, updateClipReverse, updateVisualClipOpacity, updateVisualClipTransform, updateVisualClipCrop, setItemKeyframes, updateItemEffects, updateItemColorGrade, updateTextOverlayItems, applyCaptionRepair, editTranscriptWords, recordGeneratedAsset, updateGeneratedAssetStatus, updateGeneratedAssetReferences, completeGeneratedAsset, replaceTimelineItemWithGeneratedOutput, assignMediaFolder, createMediaFolder, renameMediaFolder, deleteMediaFolder, renameMedia, deleteMedia, updateRenderSettings, recordJob, updateJobStatus, attachRenderReport, recordExportArtifact, updateTemplateItems, updateTemplateOverride, addTransition, updateTransition, removeTransition.
- Prefer applyCaptionRepair for transcript-backed caption fixes, rippleDeleteRanges for source-backed transcript word removals that should close timeline gaps, editTextItem for simple text-backed item copy updates, updateTextOverlayItems for manual text overlay timing, copy, visualTreatment, motion, safeZone, and avoid edits, updateAudioFadeOut and updateAudioVolume for selected audio clip fades and gain, updateVisualClipOpacity, updateVisualClipTransform, and updateVisualClipCrop for selected video, overlay, or HyperFrames clip opacity, static transform, and crop, setItemKeyframes for opacity ramps or audio volume automation, updateItemEffects and updateItemColorGrade for editable visual looks, createTrack/removeTracks for timeline lane changes, setTrackLocked and setTrackEnabled for explicit track protection or visibility changes, editTranscriptWords for transcript-only repairs, recordGeneratedAsset, updateGeneratedAssetStatus, updateGeneratedAssetReferences, and completeGeneratedAsset for generated media provenance, replaceTimelineItemWithGeneratedOutput for timeline-native generated-output swaps, assignMediaFolder plus createMediaFolder/renameMediaFolder/deleteMediaFolder/renameMedia/deleteMedia for project-library organization, updateRenderSettings for render target changes, recordJob and updateJobStatus for workflow-backed render, generation, transcription, or Codex jobs, attachRenderReport for render-review artifacts, recordExportArtifact for durable project-relative export outputs, updateTemplateOverride for reusable template style edits, and addTransition, updateTransition, and removeTransition for crossfade, dipToBlack, dipToWhite, or wipe transitions centered on the cut between two adjacent clips on one track.
- For recordGeneratedAsset, include prompt, model, references, settings width/height/durationSeconds/fps/aspectRatio when known, outputs, createdAt, parentAssetId, and retryOfAssetId.
- For completeGeneratedAsset, include assetId and outputs; when a generation completion should swap a timeline clip, include replacement with itemId and a mediaId from the same outputs array.
- For recordJob, include id, kind, status, updatedAt, and optional workflow metadata with workflowId, workflowType, taskQueue, runId, and activityTypes. Use updateJobStatus when an existing workflow job moves between queued, running, completed, failed, or cancelled.

Rules:
- Use video-creater-video-pipeline first.
- Build a real EDL with selected sourceIn/sourceOut ranges before visual layers.
- Reject styled full-source pass-through edits.
- Return clips with reasons and renderReview criteria for Rust validation.

Visual quality guardrails:
- Use video-creater-graphics for captions, overlays, title cards, lower thirds, and HyperFrames.
- Avoid full-width opaque black caption slabs, generic text-on-box layouts, static text-only cards, default-font template looks, and long unmoving holds.
- Prefer prompt-native art direction: shaped translucent materials, kinetic typography, accent strokes, depth, responsive placement, and quick in/out motion.
- Keep captions phone-size legible while preserving the source action; do not cover faces, hands, or critical visual details unless the beat explicitly requires it.
- Every caption, overlay, HyperFrame, and GPU visual must include sourceBeat, dimensions, positive frameRate, alpha, visualTreatment, motion, safeZone, and avoid without invented defaults.
",
        project_id = context.project_id,
        project_name = context.project_name,
        project_files_summary = context.project_files_summary,
        media_id = context.request.media_id,
        preset = context.request.preset,
        target_duration = context
            .request
            .target_duration_seconds
            .map(|duration| format!("{duration:.3}"))
            .unwrap_or_else(|| "default".to_string()),
        language_mode = context.request.language_mode.as_code(),
        caption_style = context.request.caption_style,
        prompt = context.request.prompt,
        media_library_summary = context.media_library_summary,
        timeline_summary = context.timeline_summary,
        generated_assets_summary = context.generated_assets_summary,
        template_overrides_summary = context.template_overrides_summary,
        render_reports_summary = context.render_reports_summary,
        workflow_jobs_summary = context.workflow_jobs_summary,
        export_artifacts_summary = context.export_artifacts_summary,
        export_capabilities_summary = context.export_capabilities_summary,
        relative_path = context.media_relative_path,
        duration = context.media_duration_seconds,
        width = context
            .media_width
            .map(|width| width.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        height = context
            .media_height
            .map(|height| height.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        fps = context
            .media_fps
            .map(|fps| format!("{fps:.3}"))
            .unwrap_or_else(|| "unknown".to_string()),
        transcript_excerpt = context.transcript_excerpt,
        shadertoy_profiles = shadertoy_profiles,
    )
}

pub(crate) fn extract_thread_id(response: &Value) -> Result<String, CodexAppServerError> {
    response
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(CodexAppServerError::MissingThreadId)
}

fn extract_turn_string(response: &Value, field: &str) -> Option<String> {
    response
        .get("turn")
        .and_then(|turn| turn.get(field))
        .or_else(|| response.get(field))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::preset::{CaptionStyle, EditPreset, LanguageMode};
    use crate::project::fixtures::sample_project;
    use crate::project::model::{Transcript, TranscriptWord};
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn bundled_codex_command_resolves_next_to_the_app_executable() {
        let command = bundled_codex_app_server_command_for_executable(Path::new(
            "/Applications/Video Creater.app/Contents/MacOS/Video Creater",
        ));

        assert_eq!(
            command.program,
            "/Applications/Video Creater.app/Contents/MacOS/video-creater-codex",
        );
        assert_eq!(command.args, ["app-server", "--stdio"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bundled_linux_codex_prepends_staged_tools_without_dropping_system_path() {
        let temp = tempfile::tempdir().expect("temp");
        let bin = temp.path().join("usr/bin");
        let tools = temp
            .path()
            .join("usr/lib/Video Creater/codex-runtime/codex-path");
        fs::create_dir_all(&bin).expect("bin dir");
        fs::create_dir_all(&tools).expect("tools dir");
        let rg = tools.join("rg");
        fs::write(&rg, "#!/bin/sh\n").expect("rg");
        fs::set_permissions(&rg, fs::Permissions::from_mode(0o755)).expect("chmod rg");
        let program = bin.join("video-creater-codex");

        let environment =
            bundled_codex_environment(&program, Some(OsStr::new("/usr/local/bin:/usr/bin")), None);
        let path = environment.get("PATH").expect("PATH override");
        let entries = env::split_paths(OsStr::new(path)).collect::<Vec<_>>();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].join("rg").is_file());
        assert_eq!(entries[1], Path::new("/usr/local/bin"));
        assert_eq!(entries[2], Path::new("/usr/bin"));

        let appimage_root = temp.path().join("appimage");
        let appimage_tools = appimage_root.join("usr/lib/Video Creater/codex-runtime/codex-path");
        fs::create_dir_all(&appimage_tools).expect("appimage tools");
        fs::copy(&rg, appimage_tools.join("rg")).expect("appimage rg");
        let mounted = temp.path().join("elsewhere/video-creater-codex");
        let environment = bundled_codex_environment(
            &mounted,
            Some(OsStr::new("/usr/bin")),
            Some(appimage_root.as_os_str()),
        );
        assert!(environment["PATH"].starts_with(appimage_tools.to_str().unwrap()));

        assert!(
            bundled_codex_environment(&bin.join("codex"), Some(OsStr::new("/usr/bin")), None)
                .is_empty()
        );
        fs::remove_file(&rg).expect("remove rg");
        assert!(bundled_codex_environment(&program, Some(OsStr::new("/usr/bin")), None).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn path_resolution_skips_non_executable_candidate_before_valid_codex() {
        let temp = tempfile::tempdir().expect("temp");
        let shadow_dir = temp.path().join("shadow");
        let valid_dir = temp.path().join("valid");
        fs::create_dir_all(&shadow_dir).expect("shadow dir");
        fs::create_dir_all(&valid_dir).expect("valid dir");
        let shadow = shadow_dir.join("codex");
        let valid = valid_dir.join("codex");
        write_executable_fixture(&shadow, 0o600);
        write_executable_fixture(&valid, 0o700);
        let path = std::env::join_paths([&shadow_dir, &valid_dir]).expect("fixture PATH");

        let resolved = resolve_codex_executable_in_path("codex", Some(&path));

        assert_eq!(resolved, Some(valid));
    }

    #[cfg(unix)]
    #[test]
    fn path_resolution_skips_current_user_shadows_with_inapplicable_execute_classes() {
        let temp = tempfile::tempdir().expect("temp");
        let other_only_dir = temp.path().join("other-only");
        let group_only_dir = temp.path().join("group-only");
        let valid_dir = temp.path().join("valid");
        fs::create_dir_all(&other_only_dir).expect("other-only dir");
        fs::create_dir_all(&group_only_dir).expect("group-only dir");
        fs::create_dir_all(&valid_dir).expect("valid dir");
        let other_only = other_only_dir.join("codex");
        let group_only = group_only_dir.join("codex");
        let valid = valid_dir.join("codex");
        write_executable_fixture(&other_only, 0o001);
        write_executable_fixture(&group_only, 0o010);
        write_executable_fixture(&valid, 0o700);
        let path = std::env::join_paths([&other_only_dir, &group_only_dir, &valid_dir])
            .expect("fixture PATH");

        let resolved = resolve_codex_executable_in_path("codex", Some(&path));

        assert_eq!(resolved, Some(valid));
    }

    #[cfg(unix)]
    #[test]
    fn explicit_non_executable_path_is_preserved_for_launch_diagnostics() {
        let temp = tempfile::tempdir().expect("temp");
        let configured = temp.path().join("configured-codex");
        write_executable_fixture(&configured, 0o600);

        let resolved = resolve_codex_executable(&configured.display().to_string());

        assert_eq!(resolved, Some(configured));
    }

    #[cfg(unix)]
    fn write_executable_fixture(path: &Path, mode: u32) {
        fs::write(path, "#!/bin/sh\nexit 0\n").expect("write fixture");
        let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
        permissions.set_mode(mode);
        fs::set_permissions(path, permissions).expect("fixture permissions");
    }

    #[cfg(unix)]
    fn write_app_server_fixture(path: &Path, body: &str) {
        fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("write fixture");
        let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("fixture permissions");
    }

    struct FakeTransport {
        messages: std::collections::VecDeque<AppServerMessage>,
        sent: Vec<Value>,
        terminated: bool,
        cancel_on_empty: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    }

    impl CodexAppServerTransport for FakeTransport {
        fn send(&mut self, message: Value) -> Result<(), CodexAppServerError> {
            self.sent.push(message);
            Ok(())
        }

        fn recv_until(
            &mut self,
            _deadline: std::time::Instant,
        ) -> Result<AppServerMessage, CodexAppServerError> {
            if let Some(message) = self.messages.pop_front() {
                return Ok(message);
            }
            if let Some(cancelled) = &self.cancel_on_empty {
                cancelled.store(true, std::sync::atomic::Ordering::Release);
            }
            Err(CodexAppServerError::Deadline)
        }

        fn terminate(&mut self) -> Result<AppServerCleanupReport, CodexAppServerError> {
            self.terminated = true;
            Ok(AppServerCleanupReport::default())
        }
    }

    fn sample_request() -> EditJobRequest {
        EditJobRequest {
            media_id: "media-1".to_string(),
            preset: EditPreset::TrailerCut,
            prompt: "Make a tight trailer cut.".to_string(),
            target_duration_seconds: None,
            language_mode: LanguageMode::English,
            caption_style: CaptionStyle::Bold,
            created_at: "2026-06-23T00:00:00Z".to_string(),
        }
    }

    fn sample_skills() -> super::super::context::ProjectSkillBundle {
        super::super::context::ProjectSkillBundle {
            agents_md: "agents".to_string(),
            video_pipeline: "pipeline".to_string(),
            graphics: "graphics".to_string(),
            visuals: "visuals".to_string(),
            shader_background_catalog: "profiles".to_string(),
        }
    }

    fn project_with_transcript() -> VideoProject {
        let mut project = sample_project();
        project.transcripts.push(Transcript {
            id: "transcript-1".to_string(),
            media_id: "media-1".to_string(),
            engine: Some("test".to_string()),
            raw_artifact_path: None,
            repairs: Vec::new(),
            segments: Vec::new(),
            words: vec![TranscriptWord {
                text: "hello".to_string(),
                start_seconds: 0.0,
                end_seconds: 0.5,
                confidence: Some(0.9),
                speaker: None,
            }],
        });
        project
    }

    #[test]
    fn project_action_schema_includes_visual_control_actions() {
        let schema = project_action_schema();
        let variants = schema
            .get("anyOf")
            .and_then(Value::as_array)
            .expect("project action schema should list variants");

        let has_visual_opacity = variants.iter().any(|variant| {
            variant
                .pointer("/properties/type/enum/0")
                .and_then(Value::as_str)
                == Some("updateVisualClipOpacity")
        });
        let crop_schema = variants
            .iter()
            .find(|variant| {
                variant
                    .pointer("/properties/type/enum/0")
                    .and_then(Value::as_str)
                    == Some("updateVisualClipCrop")
            })
            .expect("crop action schema");

        assert!(has_visual_opacity);
        assert_eq!(
            crop_schema.pointer("/properties/crop/properties/cropTop"),
            Some(&json!({
                "type": ["number", "null"],
                "minimum": 0,
                "exclusiveMaximum": 1
            }))
        );
    }

    #[test]
    fn project_action_schema_includes_transition_actions() {
        let schema = project_action_schema();
        let variants = schema
            .get("anyOf")
            .and_then(Value::as_array)
            .expect("project action schema should list variants");
        let variant = |action_type: &str| {
            variants
                .iter()
                .find(|variant| {
                    variant
                        .pointer("/properties/type/enum/0")
                        .and_then(Value::as_str)
                        == Some(action_type)
                })
                .unwrap_or_else(|| panic!("{action_type} action schema"))
        };
        let kinds = json!(["crossfade", "dipToBlack", "dipToWhite", "wipe"]);

        let add = variant("addTransition");
        assert_eq!(add["required"], json!(["type", "trackId", "transition"]));
        assert_eq!(
            add.pointer("/properties/transition/required"),
            Some(&json!([
                "id",
                "leftItemId",
                "rightItemId",
                "kind",
                "durationSeconds"
            ]))
        );
        assert_eq!(
            add.pointer("/properties/transition/properties/kind/enum"),
            Some(&kinds)
        );
        let update = variant("updateTransition");
        assert_eq!(
            update["required"],
            json!(["type", "trackId", "transitionId", "kind", "durationSeconds"])
        );
        assert_eq!(
            update.pointer("/properties/kind/anyOf/0/enum"),
            Some(&kinds)
        );
        assert_eq!(
            update.pointer("/properties/durationSeconds/type"),
            Some(&json!(["number", "null"]))
        );
        let remove = variant("removeTransition");
        assert_eq!(
            remove["required"],
            json!(["type", "trackId", "transitionId"])
        );
        for variant in [add, update, remove] {
            assert_eq!(variant["additionalProperties"], json!(false));
        }

        // Structured output that satisfies the schemas is a canonical action.
        for value in [
            json!({ "type": "addTransition", "trackId": "track-video", "transition": {
                "id": "transition-1", "leftItemId": "clip-1", "rightItemId": "clip-2",
                "kind": "dipToWhite", "durationSeconds": 0.5,
            }}),
            json!({ "type": "updateTransition", "trackId": "track-video",
                "transitionId": "transition-1", "kind": null, "durationSeconds": 1.0 }),
            json!({ "type": "removeTransition", "trackId": "track-video", "transitionId": "transition-1" }),
        ] {
            let action: crate::project::action::ProjectAction =
                serde_json::from_value(value.clone()).expect("transition action");
            let round_trip = serde_json::to_value(&action).expect("serialize action");
            assert_eq!(round_trip["type"], value["type"]);
        }
    }

    #[test]
    fn start_turn_returns_invalid_structured_proposal_for_ui_review() {
        let mut project = project_with_transcript();
        let mut transport = FakeTransport {
            messages: std::collections::VecDeque::from(vec![
                AppServerMessage::Response {
                    id: json!(1),
                    result: json!({ "ok": true }),
                },
                AppServerMessage::Response {
                    id: json!(2),
                    result: json!({ "thread": { "id": "thread-1" } }),
                },
                AppServerMessage::Response {
                    id: json!(3),
                    result: json!({ "turn": { "id": "turn-1", "items": [], "status": "inProgress" } }),
                },
                AppServerMessage::Notification {
                    method: "turn/completed".to_string(),
                    params: json!({
                        "threadId": "thread-1",
                        "turn": {
                          "id": "turn-1",
                          "status": "completed",
                          "items": [{"id":"message-1", "type":"agentMessage", "phase":"final_answer", "text": serde_json::to_string(&json!({
                        "mediaId": "media-1",
                        "clips": [
                            { "sourceIn": 0.0, "sourceOut": 12.0, "reason": "full source pass-through" }
                        ],
                        "captions": [],
                        "overlays": [],
                        "hyperframes": [],
                        "gpuVisuals": [],
                        "projectActions": [],
                        "renderReview": {
                            "durationSeconds": 12.0,
                            "streamCheckRequired": true,
                            "captionAlignmentRequired": true,
                            "overlayTimingRequired": true,
                            "visualFrameEvidenceRequired": true,
                            "artifactPathsRequired": true,
                            "logReferenceRequired": true
                        }})).unwrap()}]
                        }
                    }),
                },
            ]),
            sent: Vec::new(),
            terminated: false,
            cancel_on_empty: None,
        };

        let result = start_codex_video_edit_turn(
            &mut transport,
            1,
            "/tmp/video-creater",
            &mut project,
            sample_request(),
            &sample_skills(),
            None,
        )
        .expect("invalid proposals should be reviewable");

        let issues = result
            .proposal_validation_issues
            .expect("proposal validation should run");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, "clips");
        assert!(issues[0].message.contains("full source"));
        assert!(result.proposal.is_some());
        assert_eq!(
            result.turn_response.pointer("/status"),
            Some(&json!("completed"))
        );
    }

    #[test]
    fn protocol_pump_routes_interleaved_messages_and_denies_approval() {
        let proposal = serde_json::to_string(&json!({
            "mediaId":"media-1", "clips":[], "captions":[], "overlays":[],
            "hyperframes":[], "projectActions":[], "renderReview": {
                "expectedDurationSeconds":0.0, "requireVideoStream":true,
                "requireAudioStream":true, "captionsAligned":true,
                "overlaysAligned":true, "visualFrameEvidenceRequired":true,
                "artifactPathsRequired":true, "logReferenceRequired":true
            }
        }))
        .unwrap();
        let mut transport = FakeTransport {
            messages: std::collections::VecDeque::from(vec![
                AppServerMessage::Notification {
                    method: "turn/started".into(),
                    params: json!({"threadId":"thread-1"}),
                },
                AppServerMessage::Response {
                    id: json!(99),
                    result: json!({"unrelated":true}),
                },
                AppServerMessage::Response {
                    id: json!(3),
                    result: json!({"turn":{"id":"turn-1","items":[],"status":"inProgress"}}),
                },
                AppServerMessage::Notification {
                    method: "item/agentMessage/delta".into(),
                    params: json!({"threadId":"thread-1","turnId":"turn-1","delta":"fallback"}),
                },
                AppServerMessage::ServerRequest {
                    id: json!("approval-1"),
                    method: "item/commandExecution/requestApproval".into(),
                    params: json!({}),
                },
                AppServerMessage::Notification {
                    method: "item/completed".into(),
                    params: json!({"threadId":"thread-1","turnId":"turn-1","item":{"id":"message-1","type":"agentMessage","phase":"final_answer","text":proposal},"completedAtMs":1}),
                },
                AppServerMessage::Notification {
                    method: "turn/completed".into(),
                    params: json!({"threadId":"thread-1","turn":{"id":"turn-1","items":[],"status":"completed"}}),
                },
            ]),
            sent: Vec::new(),
            terminated: false,
            cancel_on_empty: None,
        };

        let completed = run_codex_turn_pump(
            &mut transport,
            json!({"id":3,"method":"turn/start","params":{}}),
            "thread-1",
            std::time::Instant::now() + std::time::Duration::from_secs(1),
            None,
        )
        .unwrap();

        assert_eq!(completed.terminal_turn["status"], "completed");
        assert_eq!(completed.final_text, proposal);
        assert!(completed.retained_messages.iter().any(
            |message| matches!(message, AppServerMessage::Response { id, .. } if id == &json!(99))
        ));
        assert!(transport
            .sent
            .iter()
            .any(|message| message == &json!({"id":"approval-1","result":{"decision":"decline"}})));
    }

    #[test]
    fn proposal_threads_are_read_only_and_never_request_approval() {
        let request =
            build_video_thread_request(1, CodexThreadAction::Start, "/tmp", &sample_skills());
        assert_eq!(
            request.pointer("/params/sandbox"),
            Some(&json!("read-only"))
        );
        assert_eq!(
            request.pointer("/params/approvalPolicy"),
            Some(&json!("never"))
        );
    }

    struct TestSignal(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl CancellationSignal for TestSignal {
        fn is_cancelled(&self) -> bool {
            self.0.load(std::sync::atomic::Ordering::Acquire)
        }
        fn wait_timeout(&self, _duration: Duration) -> bool {
            self.is_cancelled()
        }
    }

    #[test]
    fn cancellation_after_turn_id_interrupts_exact_turn_and_terminates() {
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut transport = FakeTransport {
            messages: std::collections::VecDeque::from([AppServerMessage::Response {
                id: json!(3),
                result: json!({"turn":{"id":"turn-7","items":[],"status":"inProgress"}}),
            }]),
            sent: Vec::new(),
            terminated: false,
            cancel_on_empty: Some(cancelled.clone()),
        };
        let error = run_codex_turn_pump(
            &mut transport,
            json!({"id":3}),
            "thread-4",
            Instant::now() + Duration::from_secs(1),
            Some(&TestSignal(cancelled)),
        )
        .unwrap_err();
        assert_eq!(error, CodexAppServerError::Interrupted);
        assert!(transport
            .sent
            .iter()
            .any(|message| message.pointer("/params/turnId") == Some(&json!("turn-7"))));
        assert!(transport.terminated);
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_during_initialize_terminates_before_starting_a_thread() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("codex-init-hang");
        write_app_server_fixture(&binary, "read request\nsleep 30");
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut transport = StdioCodexAppServerTransport::spawn_until(
            &codex_app_server_command(&binary.display().to_string()),
            None,
            deadline,
            Some(&TestSignal(cancelled.clone())),
        )
        .unwrap();
        let cancel = cancelled.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            cancel.store(true, Ordering::Release);
        });
        let mut project = project_with_transcript();
        let started = Instant::now();

        let error = start_codex_video_edit_turn_unpersisted_until(
            &mut transport,
            1,
            "/tmp/video-creater",
            &mut project,
            sample_request(),
            &sample_skills(),
            None,
            deadline,
            Some(&TestSignal(cancelled)),
        )
        .unwrap_err();

        assert_eq!(error, CodexAppServerError::Interrupted);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[cfg(unix)]
    #[test]
    fn cancellation_during_thread_resume_terminates_before_starting_a_turn() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("codex-resume-hang");
        write_app_server_fixture(
            &binary,
            "read initialize\nprintf '%s\\n' '{\"id\":1,\"result\":{\"userAgent\":\"fixture\"}}'\nread resume\nsleep 30",
        );
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut transport = StdioCodexAppServerTransport::spawn_until(
            &codex_app_server_command(&binary.display().to_string()),
            None,
            deadline,
            Some(&TestSignal(cancelled.clone())),
        )
        .unwrap();
        let cancel = cancelled.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            cancel.store(true, Ordering::Release);
        });
        let mut project = project_with_transcript();
        project.codex_thread_id = Some("existing-thread".into());
        let started = Instant::now();

        let error = start_codex_video_edit_turn_unpersisted_until(
            &mut transport,
            1,
            "/tmp/video-creater",
            &mut project,
            sample_request(),
            &sample_skills(),
            None,
            deadline,
            Some(&TestSignal(cancelled)),
        )
        .unwrap_err();

        assert_eq!(error, CodexAppServerError::Interrupted);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    struct DelayedTransport {
        messages: VecDeque<AppServerMessage>,
        response_delay: Duration,
        terminated: bool,
    }

    impl CodexAppServerTransport for DelayedTransport {
        fn send(&mut self, _message: Value) -> Result<(), CodexAppServerError> {
            Ok(())
        }

        fn recv_until(
            &mut self,
            deadline: Instant,
        ) -> Result<AppServerMessage, CodexAppServerError> {
            let remaining = deadline.saturating_duration_since(Instant::now());
            std::thread::sleep(self.response_delay.min(remaining));
            if remaining < self.response_delay {
                return Err(CodexAppServerError::Deadline);
            }
            self.messages
                .pop_front()
                .ok_or(CodexAppServerError::Deadline)
        }

        fn terminate(&mut self) -> Result<AppServerCleanupReport, CodexAppServerError> {
            self.terminated = true;
            Ok(AppServerCleanupReport::default())
        }
    }

    #[test]
    fn one_deadline_bounds_initialize_thread_and_turn_sequence() {
        let mut transport = DelayedTransport {
            messages: VecDeque::from([
                AppServerMessage::Response {
                    id: json!(1),
                    result: json!({"userAgent":"test"}),
                },
                AppServerMessage::Response {
                    id: json!(2),
                    result: json!({"thread":{"id":"thread-1"}}),
                },
            ]),
            response_delay: Duration::from_millis(25),
            terminated: false,
        };
        let mut project = project_with_transcript();
        let started = Instant::now();

        let error = start_codex_video_edit_turn_unpersisted_until(
            &mut transport,
            1,
            "/tmp/video-creater",
            &mut project,
            sample_request(),
            &sample_skills(),
            None,
            started + Duration::from_millis(40),
            None,
        )
        .unwrap_err();

        assert_eq!(error, CodexAppServerError::Deadline);
        assert!(transport.terminated);
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn terminal_statuses_are_explicit_and_already_completed_start_is_supported() {
        let failed = finish_completed_turn(
            json!({}),
            json!({"id":"t","status":"failed","error":{"message":"boom"}}),
            vec![],
            String::new(),
            VecDeque::new(),
        )
        .unwrap_err();
        assert!(
            matches!(failed, CodexAppServerError::TurnFailed(message) if message.contains("boom"))
        );
        assert_eq!(
            finish_completed_turn(
                json!({}),
                json!({"id":"t","status":"interrupted"}),
                vec![],
                String::new(),
                VecDeque::new()
            )
            .unwrap_err(),
            CodexAppServerError::Interrupted
        );
        let mut transport = FakeTransport {
            messages: std::collections::VecDeque::from([AppServerMessage::Response {
                id: json!(3),
                result: json!({"turn":{"id":"t","status":"completed","items":[]}}),
            }]),
            sent: vec![],
            terminated: false,
            cancel_on_empty: None,
        };
        assert_eq!(
            run_codex_turn_pump(
                &mut transport,
                json!({"id":3}),
                "thread",
                Instant::now() + Duration::from_secs(1),
                None
            )
            .unwrap()
            .terminal_turn["status"],
            "completed"
        );
    }

    #[test]
    fn pinned_codex_initialize_handshake_uses_production_transport_without_a_turn() {
        let Ok(binary) = std::env::var("VIDEO_CREATER_PINNED_CODEX_BINARY") else {
            return;
        };
        let version = std::process::Command::new(&binary)
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&version.stdout).trim(),
            "codex-cli 0.141.0"
        );
        let mut transport =
            StdioCodexAppServerTransport::spawn(&codex_app_server_command(&binary)).unwrap();
        let result = initialize_codex_app_server(&mut transport, 1).unwrap();
        for field in ["userAgent", "codexHome", "platformFamily", "platformOs"] {
            assert!(
                result.get(field).and_then(Value::as_str).is_some(),
                "missing {field}"
            );
        }
        assert!(transport.terminate().unwrap().reaped);
    }
}
