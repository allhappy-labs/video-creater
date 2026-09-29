//! A scripted [`AgentTurnTransport`] for tests.
//!
//! Every behavioural test of the conversation turn runs through this rather than through a
//! real agent, so the suites never spend tokens and never depend on a binary being installed.

use super::{
    AgentBackendKind, AgentTurnError, AgentTurnOutcome, AgentTurnRequest, AgentTurnTransport,
};
use crate::process_supervisor::CancellationSignal;
use std::collections::VecDeque;
use std::time::Instant;

pub struct FakeAgentTurnTransport {
    backend: AgentBackendKind,
    outcomes: VecDeque<Result<AgentTurnOutcome, AgentTurnError>>,
    /// Every request the transport was handed, in order.
    pub seen: Vec<AgentTurnRequest>,
}

impl FakeAgentTurnTransport {
    pub fn new(
        backend: AgentBackendKind,
        outcomes: Vec<Result<AgentTurnOutcome, AgentTurnError>>,
    ) -> Self {
        Self {
            backend,
            outcomes: outcomes.into(),
            seen: Vec::new(),
        }
    }

    /// The common single-turn case.
    pub fn once(
        backend: AgentBackendKind,
        outcome: Result<AgentTurnOutcome, AgentTurnError>,
    ) -> Self {
        Self::new(backend, vec![outcome])
    }
}

impl AgentTurnTransport for FakeAgentTurnTransport {
    fn backend(&self) -> AgentBackendKind {
        self.backend
    }

    fn run_conversation_turn(
        &mut self,
        request: &AgentTurnRequest,
        _deadline: Instant,
        _cancellation: Option<&dyn CancellationSignal>,
    ) -> Result<AgentTurnOutcome, AgentTurnError> {
        self.seen.push(request.clone());
        self.outcomes.pop_front().unwrap_or_else(|| {
            Err(AgentTurnError::TurnFailed(
                "the fake transport ran out of scripted outcomes".to_string(),
            ))
        })
    }
}
