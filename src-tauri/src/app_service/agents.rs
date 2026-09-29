use crate::codex::proposal::{validate_codex_edit_proposal, CodexEditProposal, CodexProposalError};
use crate::edit::edl::RoughCutEdl;
use crate::edit::preset::EditJobRequest;
use crate::project::model::VideoProject;

use super::error::ServiceError;

pub struct AgentService;

impl AgentService {
    pub fn validate_edl_precedes_visuals(proposal: &CodexEditProposal) -> Result<(), ServiceError> {
        let has_visual_layers = !proposal.captions.is_empty()
            || !proposal.overlays.is_empty()
            || !proposal.hyperframes.is_empty()
            || !proposal.gpu_visuals.is_empty();
        if has_visual_layers && proposal.clips.is_empty() {
            return Err(ServiceError::invalid_input(
                "a real EDL with selected source ranges is required before visual layers",
            ));
        }
        if proposal
            .clips
            .iter()
            .any(|clip| !clip.source_in.is_finite() || clip.source_out <= clip.source_in)
        {
            return Err(ServiceError::invalid_input(
                "proposal source ranges must be finite and non-empty",
            ));
        }
        Ok(())
    }

    pub fn validate_proposal(
        project: &VideoProject,
        request: &EditJobRequest,
        proposal: &CodexEditProposal,
    ) -> Result<RoughCutEdl, ServiceError> {
        Self::validate_edl_precedes_visuals(proposal)?;
        validate_codex_edit_proposal(project, request, proposal).map_err(map_proposal_error)
    }
}

fn map_proposal_error(error: CodexProposalError) -> ServiceError {
    ServiceError::invalid_input(format!("{}: {error}", error.stable_code()))
}
