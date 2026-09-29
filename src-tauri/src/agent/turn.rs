//! One conversation turn, from any backend.
//!
//! This is the whole of what the Tauri command does between taking its lease and writing the
//! result: render the hidden context, run the turn through whichever transport was chosen, and
//! put the untrusted proposal through the existing validation path. Nothing here knows which
//! agent answered, which is the point — risk classification, deterministic ids, atomic apply
//! and snapshot undo stay exactly where they were and treat every backend alike.

use super::claude_cli::{
    resolve_claude_executable, ClaudeAuthMode, ClaudeTurnReadiness, CLAUDE_DEFAULT_MODEL,
    CLAUDE_FALLBACK_MODEL,
};
use super::claude_transport::ClaudeTurnTransport;
use super::prompt::{render_agent_turn_prompt, scrub_echoed_user_input};
use super::schema::codex_conversation_proposal_output_schema;
use super::{
    preferred_agent_backend, resolve_agent_backend, AgentBackendKind, AgentBackendSelection,
    AgentTurnError, AgentTurnOutcome, AgentTurnRequest, AgentTurnTransport,
};
use crate::codex::context::{
    build_codex_conversation_context, build_codex_developer_instructions, ProjectSkillBundle,
};
use crate::codex::conversation::{
    conversation_proposal_from_value, prepare_codex_conversation_proposal,
    CodexConversationEditProposal, CodexConversationEditRequest, CodexPreparedProposal,
};
use crate::process_supervisor::CancellationSignal;
use crate::project::model::VideoProject;
use crate::project::split::{
    codex_thread_for_next_turn, load_agent_session_manifest, AppServerConversationTurn,
    ProjectValidationIssue,
};
use crate::settings::preferences::{AppPreferencesStore, AppPreferencesV2};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A completed turn, ready for the command to persist and return.
#[derive(Debug, Clone)]
pub struct AgentConversationTurn {
    pub thread_id: String,
    pub thread_response: Value,
    pub turn_response: Value,
    pub proposal: Option<CodexConversationEditProposal>,
    /// The only bundle the client may apply. `None` whenever validation failed.
    pub prepared_proposal: Option<CodexPreparedProposal>,
    pub proposal_validation_issues: Option<Vec<ProjectValidationIssue>>,
    /// The record to hand `record_app_server_conversation_turn`, already scrubbed.
    pub record: AppServerConversationTurn,
    pub notice: Option<String>,
}

/// Everything the stored preferences say about the agent a turn runs on.
///
/// This is the whole of the settings surface a turn reads. It is a plain value rather than a
/// borrowed `AppPreferencesV2` so the command can take it once, outside the blocking task, and
/// so a test can build one without a preferences file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnPreferences {
    /// The user's backend choice. `None` is "Automatic", which names no backend and resolves
    /// to whichever one is ready.
    pub backend: Option<AgentBackendKind>,
    /// The `--model` alias for a Claude turn.
    pub claude_model: String,
    /// The `--fallback-model` alias, when there is a cheaper one to fall back to.
    pub claude_fallback_model: Option<String>,
    /// The user's explicit `claude` path, when they set one.
    pub claude_executable: Option<PathBuf>,
}

impl Default for AgentTurnPreferences {
    fn default() -> Self {
        Self::from_stored(None)
    }
}

impl AgentTurnPreferences {
    /// Read the agent settings out of the stored preferences.
    ///
    /// `None` — no preferences file, an unreadable one, or one that fails to parse — is the
    /// same as the shipped defaults: Automatic on the default model. A preferences problem
    /// must never be the reason a turn cannot run.
    pub fn from_stored(stored: Option<&AppPreferencesV2>) -> Self {
        let Some(stored) = stored else {
            return Self {
                backend: None,
                claude_model: CLAUDE_DEFAULT_MODEL.to_string(),
                claude_fallback_model: Some(CLAUDE_FALLBACK_MODEL.to_string()),
                claude_executable: None,
            };
        };
        Self {
            backend: preferred_agent_backend(stored.agent_backend),
            claude_model: stored.claude_model.alias().to_string(),
            claude_fallback_model: stored.claude_model.fallback_alias().map(str::to_string),
            claude_executable: stored.claude_executable_override().map(PathBuf::from),
        }
    }

    /// Whether the user's own `claude` is where the app can find it. Installed is not the same
    /// as ready — see [`claude_turn_readiness`], which also asks whether it is signed in.
    pub fn claude_installed(&self) -> bool {
        resolve_claude_executable(self.claude_executable.as_deref()).is_some()
    }

    /// The user's explicit `claude` path in the form the readiness probe takes.
    pub fn claude_executable_arg(&self) -> Option<&str> {
        self.claude_executable.as_deref().and_then(Path::to_str)
    }
}

/// Read the agent settings without letting a broken store stop the turn.
pub fn agent_turn_preferences(store: &AppPreferencesStore) -> AgentTurnPreferences {
    AgentTurnPreferences::from_stored(store.load_saved_or_default().ok().as_ref())
}

/// How a turn will reach its backend, decided before anything is spawned.
///
/// The Claude transport is a value the caller runs directly; Codex needs a live app-server
/// process, which only the command can spawn, so its arm carries nothing. Keeping the choice
/// here — rather than inside the command — is what makes the routing testable.
pub enum AgentTurnBackendPlan {
    Codex,
    Claude(Box<ClaudeTurnTransport>),
}

/// Choose the backend for one turn and build its transport.
///
/// The preference is honoured whenever that backend is ready and falls back to the other one
/// rather than failing; Automatic prefers Claude, which runs on the user's own subscription
/// with nothing bundled, and takes Codex when Claude is not ready. Readiness is passed in
/// rather than probed here so the routing rule stays a pure decision a test can drive.
/// The Claude transport is built here so both the model preference and the auth mode reach the
/// child on exactly the path a real turn takes.
pub fn plan_agent_turn(
    preferences: &AgentTurnPreferences,
    codex_ready: bool,
    claude_readiness: ClaudeTurnReadiness,
    claude_mcp_server: PathBuf,
) -> Result<(AgentBackendSelection, AgentTurnBackendPlan), AgentTurnError> {
    let selection = resolve_agent_backend(
        preferences.backend,
        codex_ready,
        claude_readiness.is_ready(),
    )?;
    let plan = match selection.kind {
        AgentBackendKind::Codex => AgentTurnBackendPlan::Codex,
        AgentBackendKind::Claude => AgentTurnBackendPlan::Claude(Box::new(
            ClaudeTurnTransport::new(preferences.claude_executable.as_deref(), claude_mcp_server)?
                .with_model(
                    preferences.claude_model.clone(),
                    preferences.claude_fallback_model.clone(),
                )
                .with_auth(
                    claude_readiness
                        .auth()
                        .unwrap_or(ClaudeAuthMode::Subscription),
                ),
        )),
    };
    Ok((selection, plan))
}

/// The provider session this chat continues, if any.
///
/// Codex continues its thread, so nothing about existing projects changes. Claude continues the
/// UUID the chat stored, and a chat that has none — a new chat, or one that just switched
/// backend — starts a fresh session.
pub fn agent_session_for_next_turn(
    backend: AgentBackendKind,
    project: &VideoProject,
    project_dir: Option<&Path>,
) -> Result<Option<String>, AgentTurnError> {
    match backend {
        AgentBackendKind::Codex => match project_dir {
            Some(project_dir) => codex_thread_for_next_turn(project_dir, project)
                .map_err(|error| AgentTurnError::TurnFailed(error.to_string())),
            None => Ok(project.codex_thread_id.clone()),
        },
        AgentBackendKind::Claude => Ok(project_dir
            .and_then(|project_dir| load_agent_session_manifest(project_dir).ok())
            .and_then(|manifest| {
                let active = manifest.active_session_id.clone()?;
                manifest
                    .sessions
                    .into_iter()
                    .find(|session| session.id == active)
                    .and_then(|session| session.provider_session_id)
            })),
    }
}

/// Build the backend-neutral request for one turn.
pub fn build_agent_conversation_request(
    project: &VideoProject,
    request: &CodexConversationEditRequest,
    root: &Path,
    project_dir: Option<&Path>,
    skills: &ProjectSkillBundle,
    session: Option<String>,
) -> AgentTurnRequest {
    AgentTurnRequest {
        cwd: root.to_path_buf(),
        developer_instructions: build_codex_developer_instructions(skills),
        // The hidden adaptive context rides the prompt, never the instructions: Claude records
        // its system prompt once per session and reuses it on every resume, and Codex has
        // always put the context in the turn's input.
        prompt: render_agent_turn_prompt(&build_codex_conversation_context(
            project,
            request,
            project_dir,
        )),
        output_schema: codex_conversation_proposal_output_schema(),
        session,
        project_dir: project_dir.map(Path::to_path_buf),
        cancellation_key: root.display().to_string(),
    }
}

/// Run one turn through a transport and validate whatever it produced.
pub fn run_agent_conversation_turn(
    transport: &mut dyn AgentTurnTransport,
    project: &VideoProject,
    request: &CodexConversationEditRequest,
    turn_request: &AgentTurnRequest,
    deadline: Instant,
    cancellation: Option<&dyn CancellationSignal>,
) -> Result<AgentConversationTurn, AgentTurnError> {
    let backend = transport.backend();
    let outcome = transport.run_conversation_turn(turn_request, deadline, cancellation)?;
    Ok(finish_agent_conversation_turn(
        backend, project, request, &outcome,
    ))
}

/// Turn a transport's outcome into a validated turn. Split out so the validation path can be
/// exercised without a transport at all.
pub fn finish_agent_conversation_turn(
    backend: AgentBackendKind,
    project: &VideoProject,
    request: &CodexConversationEditRequest,
    outcome: &AgentTurnOutcome,
) -> AgentConversationTurn {
    let (thread_response, turn_response) = agent_turn_responses(outcome);

    let proposal = outcome
        .proposal
        .as_ref()
        .and_then(conversation_proposal_from_value);
    let (prepared_proposal, proposal_validation_issues) = match proposal.as_ref() {
        None => (None, None),
        Some(proposal) => match prepare_codex_conversation_proposal(project, proposal) {
            Ok(prepared) => (Some(prepared), Some(Vec::new())),
            Err(error) => (None, Some(vec![error.validation_issue()])),
        },
    };

    let record = AppServerConversationTurn {
        turn_id: turn_response
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string),
        turn_status: turn_response
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string),
        prompt: request.prompt.clone(),
        created_at: request.created_at.clone(),
        request: serde_json::to_value(request).unwrap_or(Value::Null),
        thread_response: thread_response.clone(),
        turn_response: turn_response.clone(),
        has_proposal: proposal.is_some(),
        // Codex keeps naming its chat by `thread_id`, so it writes neither field and every
        // project written before backends were pluggable reads back identically.
        provider: match backend {
            AgentBackendKind::Codex => None,
            AgentBackendKind::Claude => Some(backend.as_str().to_string()),
        },
        provider_session_id: match backend {
            AgentBackendKind::Codex => None,
            AgentBackendKind::Claude => Some(outcome.session.clone()),
        },
    };

    AgentConversationTurn {
        thread_id: outcome.session.clone(),
        thread_response,
        turn_response,
        proposal,
        prepared_proposal,
        proposal_validation_issues,
        record,
        notice: outcome.notice.clone(),
    }
}

/// Split a turn outcome into the two response values the command's result carries.
///
/// The Codex transport names both; any other backend hands back one transcript, which becomes
/// the turn response under a thread response the app synthesizes from the session handle.
/// Both are scrubbed here, because the echoed hidden context must never reach stored history
/// whatever produced it.
fn agent_turn_responses(outcome: &AgentTurnOutcome) -> (Value, Value) {
    let thread_response = outcome
        .transcript
        .get("threadResponse")
        .cloned()
        .unwrap_or_else(|| json!({ "thread": { "id": outcome.session } }));
    let turn_response = outcome
        .transcript
        .get("turnResponse")
        .cloned()
        .unwrap_or_else(|| outcome.transcript.clone());
    (
        scrub_echoed_user_input(thread_response),
        scrub_echoed_user_input(turn_response),
    )
}
