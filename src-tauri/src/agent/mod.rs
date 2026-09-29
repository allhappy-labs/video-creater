//! Backend-agnostic agent turn plumbing.
//!
//! The conversation turn is pluggable: the Codex app-server and the Claude CLI are two
//! transports behind one seam. Everything in this module is provider-neutral — a transport
//! produces untrusted proposal JSON and nothing more. Validation, risk classification,
//! deterministic action ids, apply and undo stay in `codex::conversation`.

pub mod claude_cli;
pub mod claude_frames;
pub mod claude_transport;
pub mod codex_transport;
pub mod prompt;
pub mod schema;
pub mod turn;

/// A scripted transport. It ships in the library rather than behind `#[cfg(test)]` because the
/// integration suites drive the conversation turn through it, and every behavioural test of a
/// backend runs on it so that none of them spend a token.
pub mod fake_transport;

use crate::process_supervisor::CancellationSignal;
use crate::settings::preferences::AgentBackendPreference;
use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;

/// Which agent produced (or will produce) a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentBackendKind {
    Codex,
    Claude,
}

impl AgentBackendKind {
    /// The stable wire name persisted in the session manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            AgentBackendKind::Codex => "codex",
            AgentBackendKind::Claude => "claude",
        }
    }

    /// Not `FromStr`: an unknown backend name in a stored manifest is a normal "this file came
    /// from a newer build" case, not a parse error worth a dedicated error type.
    #[expect(
        clippy::should_implement_trait,
        reason = "an unknown backend name is None, not an error"
    )]
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "codex" => Some(AgentBackendKind::Codex),
            "claude" => Some(AgentBackendKind::Claude),
            _ => None,
        }
    }
}

/// The backend a turn will actually use, plus the preference it had to abandon to get there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentBackendSelection {
    pub kind: AgentBackendKind,
    /// `Some(preferred)` when the user's preferred backend was not ready and the other one
    /// took the turn. The UI can explain the substitution without inventing a reason.
    pub fell_back_from: Option<AgentBackendKind>,
}

/// Everything a transport needs for one conversation turn. Backend-neutral by construction:
/// no field names a provider, and the prompt is already rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnRequest {
    /// The working directory the agent runs in — the project folder.
    pub cwd: PathBuf,
    /// The system/developer instructions. Sent once per session; a resumed session keeps the
    /// instructions it was created with.
    pub developer_instructions: String,
    /// The rendered hidden adaptive context plus the user's words. Never shown to the user.
    pub prompt: String,
    /// The JSON Schema the proposal must satisfy.
    pub output_schema: serde_json::Value,
    /// The provider session handle to resume, when the chat already has one.
    pub session: Option<String>,
    /// The project folder a read-only tool surface is scoped to, when the backend has one.
    pub project_dir: Option<PathBuf>,
    /// The key this turn is registered under in the cancellation registry.
    pub cancellation_key: String,
}

/// What a transport returns for a completed turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnOutcome {
    /// The provider session handle to store for the next turn of this chat.
    pub session: String,
    /// The proposal JSON, still untrusted. `None` means "the agent did not suggest an edit",
    /// which is a first-class app state, not a failure.
    pub proposal: Option<serde_json::Value>,
    /// The model the backend actually used, when it reported one.
    pub model: Option<String>,
    /// A plain-language note for the user (for example a model substitution).
    pub notice: Option<String>,
    /// What the app persists in `app-server-conversations.json`. The transport that produced
    /// it has already scrubbed the echoed hidden context out of it.
    pub transcript: serde_json::Value,
}

/// Why a turn did not produce an outcome.
#[derive(Debug, Error)]
pub enum AgentTurnError {
    /// No agent could run the turn: nothing installed, or the binary would not start.
    #[error("{0}")]
    Unavailable(String),
    /// The agent is installed but the user is not signed in.
    #[error("{0}")]
    NotAuthenticated(String),
    /// The account's usage limit is reached.
    #[error("{detail}")]
    UsageLimit { detail: String },
    /// The user cancelled the turn.
    #[error("the agent turn was interrupted")]
    Interrupted,
    /// The turn ran past its deadline.
    #[error("the agent turn timed out")]
    Deadline,
    /// The agent ran and failed for its own reasons.
    #[error("{0}")]
    TurnFailed(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

impl PartialEq for AgentTurnError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AgentTurnError::Unavailable(left), AgentTurnError::Unavailable(right)) => {
                left == right
            }
            (AgentTurnError::NotAuthenticated(left), AgentTurnError::NotAuthenticated(right)) => {
                left == right
            }
            (
                AgentTurnError::UsageLimit { detail: left },
                AgentTurnError::UsageLimit { detail: right },
            ) => left == right,
            (AgentTurnError::Interrupted, AgentTurnError::Interrupted) => true,
            (AgentTurnError::Deadline, AgentTurnError::Deadline) => true,
            (AgentTurnError::TurnFailed(left), AgentTurnError::TurnFailed(right)) => left == right,
            _ => false,
        }
    }
}

/// The one seam between the conversation code and an agent.
pub trait AgentTurnTransport {
    fn backend(&self) -> AgentBackendKind;

    fn run_conversation_turn(
        &mut self,
        request: &AgentTurnRequest,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<AgentTurnOutcome, AgentTurnError>;
}

/// The message a user sees when neither agent can take the turn.
pub const NO_AGENT_AVAILABLE: &str =
    "No AI agent is available. Install one and sign in, then open Agent settings to check it.";

/// Pick the backend for a turn.
///
/// `preference` is `None` for the "Automatic" setting. Automatic prefers Claude: it is the
/// backend the product is built around, it runs on the user's own subscription with nothing
/// bundled and no API key, and it does not depend on a pinned sidecar. Codex takes the turn
/// when Claude is not ready. An explicit preference is honoured whenever that backend is
/// ready, and falls back to the other one rather than failing the turn.
///
/// Amended 2026-09-18: Automatic preferred Codex when the seam first landed.
pub fn resolve_agent_backend(
    preference: Option<AgentBackendKind>,
    codex_ready: bool,
    claude_ready: bool,
) -> Result<AgentBackendSelection, AgentTurnError> {
    let ready = |kind: AgentBackendKind| match kind {
        AgentBackendKind::Codex => codex_ready,
        AgentBackendKind::Claude => claude_ready,
    };
    let other = |kind: AgentBackendKind| match kind {
        AgentBackendKind::Codex => AgentBackendKind::Claude,
        AgentBackendKind::Claude => AgentBackendKind::Codex,
    };

    let preferred = preference.unwrap_or(AgentBackendKind::Claude);
    if ready(preferred) {
        return Ok(AgentBackendSelection {
            kind: preferred,
            fell_back_from: None,
        });
    }

    let alternative = other(preferred);
    if ready(alternative) {
        // Automatic has no preference to abandon, so it never reports a fallback.
        return Ok(AgentBackendSelection {
            kind: alternative,
            fell_back_from: preference.map(|_| preferred),
        });
    }

    Err(AgentTurnError::Unavailable(NO_AGENT_AVAILABLE.to_string()))
}

/// The stored preference as `resolve_agent_backend`'s input. `Automatic` carries
/// no preference, so it resolves to whichever agent is ready rather than naming
/// one here.
pub fn preferred_agent_backend(preference: AgentBackendPreference) -> Option<AgentBackendKind> {
    match preference {
        AgentBackendPreference::Automatic => None,
        AgentBackendPreference::Codex => Some(AgentBackendKind::Codex),
        AgentBackendPreference::Claude => Some(AgentBackendKind::Claude),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(
        preference: Option<AgentBackendKind>,
        codex_ready: bool,
        claude_ready: bool,
    ) -> Result<AgentBackendSelection, AgentTurnError> {
        resolve_agent_backend(preference, codex_ready, claude_ready)
    }

    #[test]
    fn the_stored_preference_maps_onto_a_backend_choice() {
        assert_eq!(
            preferred_agent_backend(AgentBackendPreference::Automatic),
            None
        );
        assert_eq!(
            preferred_agent_backend(AgentBackendPreference::Codex),
            Some(AgentBackendKind::Codex)
        );
        assert_eq!(
            preferred_agent_backend(AgentBackendPreference::Claude),
            Some(AgentBackendKind::Claude)
        );
    }

    #[test]
    fn automatic_prefers_claude_when_both_are_ready() {
        assert_eq!(
            selection(None, true, true).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Claude,
                fell_back_from: None,
            }
        );
    }

    #[test]
    fn automatic_uses_claude_when_codex_is_not_ready() {
        assert_eq!(
            selection(None, false, true).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Claude,
                fell_back_from: None,
            }
        );
    }

    #[test]
    fn automatic_falls_back_to_codex_when_claude_is_not_ready() {
        assert_eq!(
            selection(None, true, false).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Codex,
                // Automatic named no backend, so taking Codex abandons no preference.
                fell_back_from: None,
            }
        );
    }

    #[test]
    fn automatic_fails_when_no_agent_is_ready() {
        assert_eq!(
            selection(None, false, false).unwrap_err(),
            AgentTurnError::Unavailable(NO_AGENT_AVAILABLE.to_string())
        );
    }

    #[test]
    fn codex_preference_is_honoured_when_codex_is_ready() {
        assert_eq!(
            selection(Some(AgentBackendKind::Codex), true, true).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Codex,
                fell_back_from: None,
            }
        );
    }

    #[test]
    fn codex_preference_falls_back_to_claude() {
        assert_eq!(
            selection(Some(AgentBackendKind::Codex), false, true).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Claude,
                fell_back_from: Some(AgentBackendKind::Codex),
            }
        );
    }

    #[test]
    fn claude_preference_is_honoured_when_claude_is_ready() {
        assert_eq!(
            selection(Some(AgentBackendKind::Claude), true, true).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Claude,
                fell_back_from: None,
            }
        );
    }

    #[test]
    fn claude_preference_falls_back_to_codex() {
        assert_eq!(
            selection(Some(AgentBackendKind::Claude), true, false).unwrap(),
            AgentBackendSelection {
                kind: AgentBackendKind::Codex,
                fell_back_from: Some(AgentBackendKind::Claude),
            }
        );
    }

    #[test]
    fn claude_preference_fails_when_no_agent_is_ready() {
        assert_eq!(
            selection(Some(AgentBackendKind::Claude), false, false).unwrap_err(),
            AgentTurnError::Unavailable(NO_AGENT_AVAILABLE.to_string())
        );
    }

    #[test]
    fn backend_kinds_round_trip_through_their_wire_names() {
        for kind in [AgentBackendKind::Codex, AgentBackendKind::Claude] {
            assert_eq!(AgentBackendKind::from_str(kind.as_str()), Some(kind));
        }
        assert_eq!(AgentBackendKind::from_str("gemini"), None);
    }

    #[test]
    fn the_fake_transport_replays_scripted_outcomes_and_records_requests() {
        use crate::agent::fake_transport::FakeAgentTurnTransport;

        let outcome = AgentTurnOutcome {
            session: "session-1".to_string(),
            proposal: Some(serde_json::json!({ "summary": "ok" })),
            model: Some("haiku".to_string()),
            notice: None,
            transcript: serde_json::json!([]),
        };
        let mut transport = FakeAgentTurnTransport::new(
            AgentBackendKind::Claude,
            vec![Ok(outcome.clone()), Err(AgentTurnError::Interrupted)],
        );
        let request = AgentTurnRequest {
            cwd: PathBuf::from("/tmp/project"),
            developer_instructions: "instructions".to_string(),
            prompt: "hidden context".to_string(),
            output_schema: serde_json::json!({ "type": "object" }),
            session: None,
            project_dir: Some(PathBuf::from("/tmp/project")),
            cancellation_key: "/tmp/project".to_string(),
        };
        let deadline = Instant::now();

        assert_eq!(transport.backend(), AgentBackendKind::Claude);
        assert_eq!(
            transport
                .run_conversation_turn(&request, deadline, None)
                .unwrap(),
            outcome
        );
        assert_eq!(
            transport
                .run_conversation_turn(&request, deadline, None)
                .unwrap_err(),
            AgentTurnError::Interrupted
        );
        assert_eq!(transport.seen, vec![request.clone(), request.clone()]);
        // An exhausted script is a test bug, and it must surface as one rather than silently
        // replaying the last outcome.
        assert!(transport
            .run_conversation_turn(&request, deadline, None)
            .is_err());
    }
}
