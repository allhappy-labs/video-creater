//! Guarded apply of a prepared conversation proposal.
//!
//! The client's risk label and action array are never trusted: under the
//! project lease Rust re-prepares the proposal against the freshly loaded
//! project, requires the recomputed action IDs to match the ones the user saw,
//! refuses a `Review` bundle without explicit approval, and then applies the
//! exact bundle atomically with a restorable history entry.

use super::{
    prepare_codex_conversation_proposal, CodexConversationEditProposal, CodexConversationError,
    CodexProposalImpact, CodexProposalRisk, CodexProposalRiskLevel,
};
use crate::project::model::VideoProject;
use crate::project::mutation::acquire_split_project_mutation_lease;
use crate::project::split::{
    apply_agent_project_action_batch, load_split_project, ProjectWriteReport, SplitProjectError,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationApplyRequest {
    pub proposal: CodexConversationEditProposal,
    /// The prepared action IDs the user saw; they must match the recomputed bundle.
    pub action_ids: Vec<String>,
    #[serde(default)]
    pub review_approved: bool,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversationApplyResult {
    pub project: VideoProject,
    pub report: ProjectWriteReport,
    pub history_entry_id: String,
    pub action_ids: Vec<String>,
    /// Risk and impact recomputed from the project the edit was applied to.
    pub risk: CodexProposalRisk,
    pub impact: CodexProposalImpact,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error, PartialEq)]
pub enum CodexConversationApplyError {
    #[error("This edit needs your review before it can be applied.")]
    ReviewRequired,
    #[error(
        "The project changed after this edit was prepared, so it was not applied. Ask for the edit again."
    )]
    StaleProposal,
    #[error("This edit no longer fits the project, so it was not applied: {0}")]
    NoLongerValid(CodexConversationError),
    #[error("{0}")]
    Project(SplitProjectError),
}

impl CodexConversationApplyError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::ReviewRequired => "codex.conversation.reviewRequired",
            Self::StaleProposal => "codex.conversation.staleProposal",
            Self::NoLongerValid(_) => "codex.conversation.proposalNoLongerValid",
            Self::Project(_) => "codex.conversation.applyFailed",
        }
    }
}

pub fn apply_codex_conversation_proposal(
    project_dir: &Path,
    request: CodexConversationApplyRequest,
) -> Result<CodexConversationApplyResult, CodexConversationApplyError> {
    let _lease = acquire_split_project_mutation_lease(project_dir).map_err(|message| {
        CodexConversationApplyError::Project(SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message,
        })
    })?;
    let project = load_split_project(project_dir).map_err(CodexConversationApplyError::Project)?;
    let prepared = prepare_codex_conversation_proposal(&project, &request.proposal)
        .map_err(CodexConversationApplyError::NoLongerValid)?;
    if prepared.action_ids != request.action_ids {
        return Err(CodexConversationApplyError::StaleProposal);
    }
    if prepared.risk.level == CodexProposalRiskLevel::Review && !request.review_approved {
        return Err(CodexConversationApplyError::ReviewRequired);
    }
    let applied = apply_agent_project_action_batch(
        project_dir,
        prepared.actions,
        prepared.action_ids,
        request.session_id.as_deref(),
    )
    .map_err(CodexConversationApplyError::Project)?;
    Ok(CodexConversationApplyResult {
        project: applied.project,
        report: applied.report,
        history_entry_id: applied.history_entry_id,
        action_ids: applied.action_ids,
        risk: prepared.risk,
        impact: prepared.impact,
        warnings: applied.warnings,
    })
}
