//! Project-scoped registration and cancellation of in-flight Codex turns.
//!
//! Video-edit and conversation turns share one registry keyed by the
//! canonical project folder (or Codex root when no folder is supplied), so a
//! project runs at most one Codex turn at a time and Stop reaches whichever
//! turn is running. Start commands register through
//! [`register_codex_project_turn`] and cancel commands resolve the same key,
//! so the two can never disagree about scope.

use super::app_server::{
    register_codex_turn_cancellation, request_codex_turn_cancellation, CodexAppServerError,
    CodexTurnCancellationGuard, CodexTurnCancellationToken,
};
use crate::process_supervisor::CancellationSignal;
use std::path::Path;

/// The registry key for a Codex turn: the canonical project folder when
/// supplied, otherwise the canonical Codex root.
pub fn codex_turn_scope_key(
    root: &Path,
    project_dir: Option<&Path>,
) -> Result<String, CodexAppServerError> {
    project_dir
        .unwrap_or(root)
        .canonicalize()
        .map_err(|error| CodexAppServerError::Io(error.to_string()))?
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| CodexAppServerError::Io("Codex project scope must be valid UTF-8".into()))
}

/// Registers an in-flight turn for the project. Fails when the project
/// already has one.
pub fn register_codex_project_turn(
    root: &Path,
    project_dir: Option<&Path>,
) -> Result<(CodexTurnCancellationGuard, CodexTurnCancellationToken), CodexAppServerError> {
    register_codex_turn_cancellation(codex_turn_scope_key(root, project_dir)?)
}

/// Asks the project's in-flight turn to stop. Returns `Ok(false)` when no
/// turn is registered (none started, or it already finished).
pub fn request_codex_project_turn_cancellation(
    root: &Path,
    project_dir: Option<&Path>,
) -> Result<bool, CodexAppServerError> {
    Ok(request_codex_turn_cancellation(&codex_turn_scope_key(
        root,
        project_dir,
    )?))
}

/// Ends a turn's cancellation window before its result is persisted.
///
/// The turn is unregistered first, so later Stop requests report no turn in
/// flight; then a Stop that landed before that point yields
/// [`CodexAppServerError::Interrupted`]. A turn the user stopped is therefore
/// never persisted, even when Codex answered just before the Stop.
pub fn close_codex_turn_cancellation(
    guard: CodexTurnCancellationGuard,
    token: &CodexTurnCancellationToken,
) -> Result<(), CodexAppServerError> {
    drop(guard);
    if token.is_cancelled() {
        return Err(CodexAppServerError::Interrupted);
    }
    Ok(())
}
