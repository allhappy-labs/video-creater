//! Atomic agent batch apply and conflict-aware snapshot Undo.
//!
//! A batch applies through the canonical split-project action path under one
//! mutation lease. The agent history entry stores the pre-apply snapshot and a
//! content hash of the committed result; Undo restores the snapshot only while
//! the project still hashes to that value. Job bookkeeping is neither compared
//! nor rolled back, and the background progress of the generations the batch
//! recorded doesn't block Undo, which removes them (see `agent_undo_content`).
//! If anything fails after the project transaction commits, the exact prior
//! project is restored, so the folder is left as it was.

use super::agent_history_snapshot::{
    retain_agent_edit_history_budget, AgentProjectSnapshot, MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES,
};
use super::agent_undo_background::background_generated_asset_ids;
use super::agent_undo_content::{
    added_generated_asset_ids, agent_project_content_hash_with_background, entry_matches_project,
    restored_agent_snapshot, SplitAgentBookkeepingIds, AGENT_CONTENT_HASH_VERSION,
};
use super::{
    apply_project_actions_to_split_project_with_lease, load_split_project, read_agent_edit_history,
    read_agent_session_manifest, save_split_project, save_split_project_metadata_transactionally,
    split_project_mutation_lease, write_agent_edit_history, write_agent_session_manifest,
    ProjectAgentUndoResult, ProjectWriteReport, SplitAgentEditHistoryEntry, SplitProjectError,
};
use crate::project::action::ProjectAction;
use crate::project::model::{GeneratedAssetStatus, JobStatus, JobSummary, VideoProject};
use serde::{Deserialize, Serialize};
use std::path::Path;

const APPLIED_STATUS: &str = "applied";
const UNDONE_STATUS: &str = "undone";
const NOTHING_TO_UNDO: &str = "There is no agent edit to undo.";
const ENTRY_NOT_IN_HISTORY: &str = "This edit is no longer in the undo history.";
const NEWER_EDIT_EXISTS: &str =
    "A newer agent edit was applied after this one. Undo the newer edit first.";
const PROJECT_CHANGED: &str =
    "The project changed after this edit was applied, so undoing it would discard later changes.";
const LEGACY_NOTHING_TO_UNDO: &str = "no agent edit history is available to undo";
const LEGACY_PROJECT_CHANGED: &str =
    "latest agent edit cannot be undone because the project changed after that batch";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentApplyResult {
    pub project: VideoProject,
    pub report: ProjectWriteReport,
    pub history_entry_id: String,
    pub action_ids: Vec<String>,
    /// Session bookkeeping problems after the edit and its Undo entry committed.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectAgentUndoOutcome {
    Undone {
        project: Box<VideoProject>,
        report: ProjectWriteReport,
        entry_id: String,
        action_count: usize,
        remaining_agent_history: usize,
        warnings: Vec<String>,
        /// Generated assets the undone project no longer has: the generations
        /// the batch recorded, with the output media completion added for them.
        #[serde(default)]
        removed_generated_asset_ids: Vec<String>,
    },
    /// Undo is refused; `message` is plain language for a disabled Undo control.
    Conflict {
        entry_id: String,
        message: String,
    },
    Unavailable {
        message: String,
    },
}

/// A generation Undo removed while its job was still queued or running, so
/// its run should be cancelled.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoneGenerationRun {
    pub project_id: String,
    pub asset_id: String,
    /// The provider of the asset's model, which a recorded provider request
    /// must match before it is cancelled.
    pub asset_provider: String,
    pub job: JobSummary,
}

/// Test seam: runs after the project transaction commits and before the
/// history write, so a failure there must restore the prior project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentBatchCheckpoint {
    ProjectCommitted,
}

type CheckpointHook<'a> = &'a mut dyn FnMut(AgentBatchCheckpoint) -> Result<(), SplitProjectError>;

/// Applies an exact action batch atomically and records a restorable history
/// entry. `session_id` (or the active session) and its last turn become
/// `applied` with `action_ids`; session problems are returned as warnings.
pub fn apply_agent_project_action_batch(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    action_ids: Vec<String>,
    session_id: Option<&str>,
) -> Result<ProjectAgentApplyResult, SplitProjectError> {
    apply_batch_with_hook(
        project_dir,
        actions,
        action_ids,
        session_id,
        &mut |_| Ok(()),
    )
}

fn apply_batch_with_hook(
    project_dir: &Path,
    actions: Vec<ProjectAction>,
    action_ids: Vec<String>,
    session_id: Option<&str>,
    hook: CheckpointHook<'_>,
) -> Result<ProjectAgentApplyResult, SplitProjectError> {
    if actions.is_empty() || action_ids.len() != actions.len() {
        return Err(SplitProjectError::ProjectAction {
            message: "an agent batch needs at least one action and exactly one ID per action"
                .to_string(),
        });
    }
    let lease = split_project_mutation_lease(project_dir)?;
    let before = load_split_project(project_dir)?;
    let mut history = read_agent_edit_history(project_dir)?;
    let mut warnings = Vec::new();
    let session = resolve_session_turn(project_dir, &before, session_id, &mut warnings);
    let action_count = actions.len();

    let write = apply_project_actions_to_split_project_with_lease(project_dir, actions, &lease)?;
    let committed = (|| {
        hook(AgentBatchCheckpoint::ProjectCommitted)?;
        let after = load_split_project(project_dir)?;
        let entry_id = agent_edit_entry_id(&history.entries, after.content_revision);
        history.entries.push(batch_history_entry(
            entry_id.clone(),
            &before,
            &after,
            action_count,
            action_ids.clone(),
            session.clone(),
        )?);
        retain_agent_edit_history_budget(
            &mut history.entries,
            MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES,
        );
        let path = write_agent_edit_history(project_dir, &history)?;
        Ok((after, entry_id, path))
    })();
    let (after, history_entry_id, history_path) = match committed {
        Ok(committed) => committed,
        Err(error) => return Err(restore_exact_project(project_dir, &before, error)),
    };

    let mut report = write.report;
    report
        .written_files
        .push(history_path.display().to_string());
    if let Some((session_id, turn_id)) = &session {
        let update = mark_session_turn(
            project_dir,
            &after.id,
            session_id,
            turn_id.as_deref(),
            APPLIED_STATUS,
            Some(&action_ids),
        );
        if let Err(error) = update {
            warnings.push(format!(
                "The edit was applied, but its conversation status was not saved: {error}"
            ));
        }
    }
    Ok(ProjectAgentApplyResult {
        project: after,
        report,
        history_entry_id,
        action_ids,
        warnings,
    })
}

/// The hashed history entry of a committed batch: the pre-apply snapshot, the
/// content hash of `after` with its rule version, the bookkeeping and generated
/// assets the batch added, and the earlier generations it left alone. `session` is the session ID and its last
/// turn ID.
/// The history id of an agent edit that committed `content_revision`.
///
/// Content revisions only advance, so ids never repeat once older entries are dropped. The
/// suffix covers an entry recorded without a new revision.
pub(super) fn agent_edit_entry_id(
    entries: &[SplitAgentEditHistoryEntry],
    content_revision: u64,
) -> String {
    let base = format!("agent-edit-r{content_revision}");
    let taken = |id: &str| entries.iter().any(|entry| entry.id == id);
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|attempt| format!("{base}-{attempt}"))
        .find(|id| !taken(id))
        .expect("an unused agent edit id")
}

pub(super) fn batch_history_entry(
    entry_id: String,
    before: &VideoProject,
    after: &VideoProject,
    action_count: usize,
    action_ids: Vec<String>,
    session: Option<(String, Option<String>)>,
) -> Result<SplitAgentEditHistoryEntry, SplitProjectError> {
    let generated_asset_ids = added_generated_asset_ids(before, after);
    let background_ids = background_generated_asset_ids(before, after);
    let (session_id, turn_id) = session.unzip();
    Ok(SplitAgentEditHistoryEntry {
        id: entry_id,
        action_count,
        before: AgentProjectSnapshot::from_project(before)?,
        after: None,
        action_ids,
        after_content_revision: Some(after.content_revision),
        after_content_hash: Some(agent_project_content_hash_with_background(
            after,
            &generated_asset_ids,
            &background_ids,
        )?),
        after_content_hash_version: Some(AGENT_CONTENT_HASH_VERSION),
        added_bookkeeping_ids: SplitAgentBookkeepingIds::added_between(before, after),
        added_generated_asset_ids: generated_asset_ids,
        background_generated_asset_ids: background_ids,
        session_id,
        turn_id: turn_id.flatten(),
    })
}

/// Undoes the latest agent batch when the project still matches what that
/// batch committed. `expected_entry_id` guards a UI Undo bound to one result.
pub fn undo_latest_agent_project_batch(
    project_dir: &Path,
    expected_entry_id: Option<&str>,
) -> Result<ProjectAgentUndoOutcome, SplitProjectError> {
    undo_batch_with_hook(project_dir, expected_entry_id, &mut |_| Ok(()))
        .map(|(outcome, _)| outcome)
}

/// `undo_latest_agent_project_batch`, plus the removed generations whose jobs
/// were still queued or running when Undo committed.
pub fn undo_latest_agent_project_batch_with_runs(
    project_dir: &Path,
    expected_entry_id: Option<&str>,
) -> Result<(ProjectAgentUndoOutcome, Vec<UndoneGenerationRun>), SplitProjectError> {
    undo_batch_with_hook(project_dir, expected_entry_id, &mut |_| Ok(()))
}

fn undo_batch_with_hook(
    project_dir: &Path,
    expected_entry_id: Option<&str>,
    hook: CheckpointHook<'_>,
) -> Result<(ProjectAgentUndoOutcome, Vec<UndoneGenerationRun>), SplitProjectError> {
    let _lease = split_project_mutation_lease(project_dir)?;
    let mut history = read_agent_edit_history(project_dir)?;
    let Some(latest) = history.entries.last() else {
        return Ok((
            ProjectAgentUndoOutcome::Unavailable {
                message: NOTHING_TO_UNDO.to_string(),
            },
            Vec::new(),
        ));
    };
    if let Some(expected) = expected_entry_id.filter(|expected| *expected != latest.id) {
        let refusal = if history.entries.iter().any(|entry| entry.id == expected) {
            ProjectAgentUndoOutcome::Conflict {
                entry_id: expected.to_string(),
                message: NEWER_EDIT_EXISTS.to_string(),
            }
        } else {
            ProjectAgentUndoOutcome::Unavailable {
                message: ENTRY_NOT_IN_HISTORY.to_string(),
            }
        };
        return Ok((refusal, Vec::new()));
    }
    let current = load_split_project(project_dir)?;
    if !entry_matches_project(latest, &current)? {
        return Ok((
            ProjectAgentUndoOutcome::Conflict {
                entry_id: latest.id.clone(),
                message: PROJECT_CHANGED.to_string(),
            },
            Vec::new(),
        ));
    }

    let Some(entry) = history.entries.pop() else {
        unreachable!("the latest entry was read above");
    };
    let restored = restored_agent_snapshot(&entry, &current)?;
    let mut save = save_split_project(project_dir, &restored)?;
    let committed = (|| {
        hook(AgentBatchCheckpoint::ProjectCommitted)?;
        write_agent_edit_history(project_dir, &history)
    })();
    let history_path = match committed {
        Ok(path) => path,
        Err(error) => return Err(restore_exact_project(project_dir, &current, error)),
    };
    save.report
        .written_files
        .push(history_path.display().to_string());

    let mut warnings = Vec::new();
    if let Some(session_id) = &entry.session_id {
        let update = mark_session_turn(
            project_dir,
            &current.id,
            session_id,
            entry.turn_id.as_deref(),
            UNDONE_STATUS,
            None,
        );
        if let Err(error) = update {
            warnings.push(format!(
                "The edit was undone, but its conversation status was not saved: {error}"
            ));
        }
    }
    let removed_generated_asset_ids = current
        .generated_assets
        .iter()
        .filter(|asset| {
            !save
                .project
                .generated_assets
                .iter()
                .any(|kept| kept.id == asset.id)
        })
        .map(|asset| asset.id.clone())
        .collect::<Vec<_>>();
    let runs = in_flight_generation_runs(&current, &removed_generated_asset_ids);
    Ok((
        ProjectAgentUndoOutcome::Undone {
            project: Box::new(save.project),
            report: save.report,
            entry_id: entry.id,
            action_count: entry.action_count,
            remaining_agent_history: history.entries.len(),
            warnings,
            removed_generated_asset_ids,
        },
        runs,
    ))
}

/// The `generate_media` jobs of `removed` assets in `project` that were still
/// queued or running. A job names its asset in its start request (`assetId`),
/// or by its own id.
fn in_flight_generation_runs(
    project: &VideoProject,
    removed: &[String],
) -> Vec<UndoneGenerationRun> {
    project
        .jobs
        .iter()
        .filter(|job| {
            job.kind == "generate_media"
                && matches!(
                    job.status,
                    JobStatus::Queued
                        | JobStatus::Running
                        | JobStatus::Progress
                        | JobStatus::Blocked
                )
        })
        .filter_map(|job| {
            let asset_id = job
                .start_request
                .as_ref()
                .and_then(|request| request.input.get("assetId"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&job.id);
            let asset = project
                .generated_assets
                .iter()
                .find(|asset| asset.id == asset_id && removed.contains(&asset.id))?;
            matches!(
                asset.status,
                GeneratedAssetStatus::Queued | GeneratedAssetStatus::Running
            )
            .then(|| UndoneGenerationRun {
                project_id: project.id.clone(),
                asset_id: asset.id.clone(),
                asset_provider: asset.model.provider.trim().to_string(),
                job: job.clone(),
            })
        })
        .collect()
}

/// The legacy Codex local tool error for a refused Undo, or `None` when it was undone.
pub fn legacy_agent_undo_refusal(outcome: &ProjectAgentUndoOutcome) -> Option<SplitProjectError> {
    let message = match outcome {
        ProjectAgentUndoOutcome::Undone { .. } => return None,
        ProjectAgentUndoOutcome::Conflict { .. } => LEGACY_PROJECT_CHANGED,
        ProjectAgentUndoOutcome::Unavailable { .. } => LEGACY_NOTHING_TO_UNDO,
    };
    Some(SplitProjectError::ProjectAction {
        message: message.to_string(),
    })
}

/// Legacy Undo contract used by the Codex local tools: refusals are errors.
pub fn undo_latest_agent_project_action(
    project_dir: &Path,
) -> Result<ProjectAgentUndoResult, SplitProjectError> {
    let outcome = undo_latest_agent_project_batch(project_dir, None)?;
    if let Some(refusal) = legacy_agent_undo_refusal(&outcome) {
        return Err(refusal);
    }
    let ProjectAgentUndoOutcome::Undone {
        project,
        report,
        entry_id,
        action_count,
        remaining_agent_history,
        ..
    } = outcome
    else {
        unreachable!("refusals returned above");
    };
    Ok(ProjectAgentUndoResult {
        project: *project,
        report,
        entry_id,
        action_count,
        remaining_agent_history,
    })
}

/// Rewrites `snapshot` exactly (including its revision) after a post-commit
/// failure, returning the original error or a combined one if restoring fails.
fn restore_exact_project(
    project_dir: &Path,
    snapshot: &VideoProject,
    error: SplitProjectError,
) -> SplitProjectError {
    match save_split_project_metadata_transactionally(project_dir, snapshot, &mut |_| Ok(())) {
        Ok(_) => error,
        Err(restore_error) => SplitProjectError::Io {
            path: project_dir.display().to_string(),
            message: format!(
                "agent edit failed: {error}; restoring the previous project failed: {restore_error}"
            ),
        },
    }
}

/// Resolves the session to mark: the requested one, else the active one.
/// Returns the session ID and its last turn ID.
fn resolve_session_turn(
    project_dir: &Path,
    project: &VideoProject,
    session_id: Option<&str>,
    warnings: &mut Vec<String>,
) -> Option<(String, Option<String>)> {
    let manifest = match read_agent_session_manifest(project_dir, &project.id) {
        Ok(manifest) => manifest,
        Err(error) => {
            warnings.push(format!(
                "The conversation sessions could not be read: {error}"
            ));
            return None;
        }
    };
    let target = session_id.or(manifest.active_session_id.as_deref())?;
    match manifest
        .sessions
        .iter()
        .find(|session| session.id == target)
    {
        Some(session) => Some((
            session.id.clone(),
            session.turns.last().map(|turn| turn.id.clone()),
        )),
        None => {
            if session_id.is_some() {
                warnings.push(
                    "The conversation session was not found, so its status was not updated."
                        .to_string(),
                );
            }
            None
        }
    }
}

fn mark_session_turn(
    project_dir: &Path,
    project_id: &str,
    session_id: &str,
    turn_id: Option<&str>,
    status: &str,
    action_ids: Option<&[String]>,
) -> Result<(), SplitProjectError> {
    let mut manifest = read_agent_session_manifest(project_dir, project_id)?;
    let Some(session) = manifest
        .sessions
        .iter_mut()
        .find(|session| session.id == session_id)
    else {
        return Ok(());
    };
    let is_last_turn = session.turns.last().map(|turn| turn.id.as_str()) == turn_id;
    if let Some(turn) = turn_id.and_then(|turn_id| {
        session
            .turns
            .iter_mut()
            .rev()
            .find(|turn| turn.id == turn_id)
    }) {
        turn.proposal_status = status.to_string();
        if let Some(action_ids) = action_ids {
            turn.applied_action_ids = action_ids.to_vec();
        }
    }
    if is_last_turn {
        session.proposal_status = status.to_string();
        if let Some(action_ids) = action_ids {
            session.applied_action_ids = action_ids.to_vec();
        }
    }
    write_agent_session_manifest(project_dir, &manifest)?;
    Ok(())
}

#[cfg(test)]
#[path = "agent_batch_tests.rs"]
mod tests;
