//! The Codex app-server behind the shared transport seam.
//!
//! Nothing about the Codex turn changes here: the same `initialize` → `thread/start` or
//! `thread/resume` → `turn/start` sequence runs in the same order, with the same model
//! selection, the same deadline and the same interrupt. This module only expresses it as an
//! [`AgentTurnTransport`] so the conversation command stops naming a backend.

use super::{
    AgentBackendKind, AgentTurnError, AgentTurnOutcome, AgentTurnRequest, AgentTurnTransport,
};
use crate::codex::app_server::{
    build_agent_thread_request, build_agent_turn_request, extract_thread_id,
    initialize_codex_app_server_until, send_request_wait, AppServerMessage, CodexAppServerError,
    CodexAppServerTransport, CodexThreadAction,
};
use crate::codex::conversation::{
    conversation_proposal_from_text, conversation_proposal_from_value,
};
use crate::codex::model_selection::{choose_codex_turn_model, run_codex_turn_with_supported_model};
use crate::process_supervisor::CancellationSignal;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::time::Instant;

/// Borrows a live app-server transport for the length of one turn.
pub struct CodexTurnTransport<'a, T: CodexAppServerTransport> {
    transport: &'a mut T,
    request_id_start: u64,
}

impl<'a, T: CodexAppServerTransport> CodexTurnTransport<'a, T> {
    pub fn new(transport: &'a mut T, request_id_start: u64) -> Self {
        Self {
            transport,
            request_id_start,
        }
    }
}

impl<T: CodexAppServerTransport> AgentTurnTransport for CodexTurnTransport<'_, T> {
    fn backend(&self) -> AgentBackendKind {
        AgentBackendKind::Codex
    }

    fn run_conversation_turn(
        &mut self,
        request: &AgentTurnRequest,
        deadline: Instant,
        cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<AgentTurnOutcome, AgentTurnError> {
        let cwd = request
            .cwd
            .to_str()
            .ok_or_else(|| AgentTurnError::TurnFailed("project root must be valid UTF-8".into()))?;
        let start = self.request_id_start;

        initialize_codex_app_server_until(self.transport, start, deadline, cancellation)
            .map_err(map_app_server)?;

        let action = match &request.session {
            Some(thread_id) => CodexThreadAction::Resume {
                thread_id: thread_id.clone(),
            },
            None => CodexThreadAction::Start,
        };
        let thread_request =
            build_agent_thread_request(start + 1, action, cwd, &request.developer_instructions);
        let mut retained: VecDeque<AppServerMessage> = VecDeque::new();
        let thread_response = send_request_wait(
            self.transport,
            thread_request,
            deadline,
            &mut retained,
            cancellation,
        )
        .map_err(map_app_server)?;
        let thread_id = extract_thread_id(&thread_response).map_err(map_app_server)?;

        let turn_request = build_agent_turn_request(
            start + 2,
            &thread_id,
            &request.prompt,
            request.output_schema.clone(),
        );
        let model_choice = choose_codex_turn_model(
            self.transport,
            start,
            &thread_response,
            deadline,
            cancellation,
        )
        .map_err(map_app_server)?;
        let (completed, model_notice) = run_codex_turn_with_supported_model(
            self.transport,
            start,
            turn_request,
            &thread_id,
            &model_choice,
            deadline,
            cancellation,
        )
        .map_err(map_app_server)?;

        let turn_response = completed.terminal_turn;
        // The proposal travels as untrusted JSON, exactly as Claude's does, so the command can
        // validate both the same way. Codex reports it inside the turn response or, older
        // models, as the turn's final text.
        let proposal = conversation_proposal_from_value(&turn_response)
            .or_else(|| conversation_proposal_from_text(&completed.final_text))
            .map(serde_json::to_value)
            .transpose()?;

        let model = thread_response
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string);

        Ok(AgentTurnOutcome {
            session: thread_id,
            proposal,
            model,
            notice: model_notice,
            // Both responses are scrubbed by the command, which owns the persistence rule.
            transcript: json!({
                "threadResponse": thread_response,
                "turnResponse": turn_response,
            }),
        })
    }
}

fn map_app_server(error: CodexAppServerError) -> AgentTurnError {
    match error {
        CodexAppServerError::MissingExecutable(detail) => AgentTurnError::Unavailable(detail),
        CodexAppServerError::Interrupted => AgentTurnError::Interrupted,
        CodexAppServerError::Deadline => AgentTurnError::Deadline,
        // Every other variant keeps its stable message, which the frontend already classifies.
        other => AgentTurnError::TurnFailed(other.to_string()),
    }
}
