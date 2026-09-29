//! Undo of an applied conversation batch, cancelling the generations it
//! removes.
//!
//! Approving a "Generate & place" bundle starts its generations. Undo removes
//! them from the project (see `project::split::agent_undo_content`), so any
//! that are still queued or running are cancelled here, right after Undo
//! commits and while their job records are still at hand: the editor's cancel
//! commands read the job from the project folder, which no longer has it.
//!
//! - In-process runs are cancelled through the generation cancellation
//!   registry; a run stops at its next checkpoint and writes nothing, since
//!   its job and asset are gone.
//! - A provider request recorded for fal.ai or Replicate is cancelled at the
//!   provider. A failure is returned as a warning; the edit stays undone.
//! - Temporal runs can't be cancelled from here. A run that finishes later
//!   fails to attach its output to the missing asset, so nothing comes back.

use crate::generation::cancel::request_generation_cancellation;
use crate::project::model::JobProviderRequest;
use crate::project::split::{
    undo_latest_agent_project_batch_with_runs, ProjectAgentUndoOutcome, SplitProjectError,
};
use crate::provider_credentials::resolve_provider_credential;
use crate::workflows::temporal_generate_media_cancel_provider_activity_with_client;
use std::path::Path;
use std::time::Duration;

/// Providers whose recorded request can be cancelled remotely, as in the
/// editor's generation cancel.
const PROVIDER_CANCEL_PROVIDERS: [&str; 2] = ["fal.ai", "replicate"];
const PROVIDER_CANCEL_TIMEOUT: Duration = Duration::from_secs(15);

/// Undoes the latest agent batch (`undo_latest_agent_project_batch`) and
/// cancels the removed generations that were still in flight.
pub fn undo_codex_conversation_edit(
    project_dir: &Path,
    expected_entry_id: Option<&str>,
) -> Result<ProjectAgentUndoOutcome, SplitProjectError> {
    undo_codex_conversation_edit_with(project_dir, expected_entry_id, &mut cancel_at_provider)
}

/// `undo_codex_conversation_edit` with the provider-side cancel injected.
pub fn undo_codex_conversation_edit_with(
    project_dir: &Path,
    expected_entry_id: Option<&str>,
    cancel_provider: &mut dyn FnMut(&JobProviderRequest) -> Result<(), String>,
) -> Result<ProjectAgentUndoOutcome, SplitProjectError> {
    let (mut outcome, runs) =
        undo_latest_agent_project_batch_with_runs(project_dir, expected_entry_id)?;
    let ProjectAgentUndoOutcome::Undone { warnings, .. } = &mut outcome else {
        return Ok(outcome);
    };
    for run in runs {
        request_generation_cancellation(&run.project_id, &run.job.id);
        let Some(request) = run.job.provider_request.as_ref().filter(|request| {
            request.provider == run.asset_provider
                && PROVIDER_CANCEL_PROVIDERS.contains(&request.provider.as_str())
                && !request.cancel_url.trim().is_empty()
        }) else {
            continue;
        };
        if let Err(error) = cancel_provider(request) {
            warnings.push(format!(
                "The edit was undone, but {} didn't confirm cancelling the generation it was running: {error}",
                request.provider
            ));
        }
    }
    Ok(outcome)
}

fn cancel_at_provider(request: &JobProviderRequest) -> Result<(), String> {
    let credential = resolve_provider_credential(&request.provider)
        .map_err(|error| error.to_string())?
        .into_secret();
    let client = reqwest::blocking::Client::builder()
        .timeout(PROVIDER_CANCEL_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())?;
    temporal_generate_media_cancel_provider_activity_with_client(
        &client,
        &request.cancel_url,
        &credential,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}
