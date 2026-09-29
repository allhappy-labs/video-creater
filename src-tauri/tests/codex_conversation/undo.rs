//! Snapshot Undo for applied conversation batches, with conflict detection.

use super::apply::{ripple_proposal, speed_fixture_project};
use super::support::{
    agent_history_json, conversation_fixture_project, folder_snapshot, record_conversation_turn,
    saved_split_project,
};
use serde_json::json;
use std::path::Path;
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal, prepare_codex_conversation_proposal,
    CodexConversationApplyRequest, CodexConversationApplyResult, CodexConversationEditProposal,
};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::VideoProject;
use video_creater_lib::project::split::{
    load_agent_session_manifest, load_split_project, replace_split_project_if_revision,
    save_split_project, undo_latest_agent_project_batch, ProjectAgentUndoOutcome,
};

fn apply_safe(
    project_dir: &Path,
    project: &VideoProject,
    proposal: CodexConversationEditProposal,
) -> CodexConversationApplyResult {
    let prepared =
        prepare_codex_conversation_proposal(project, &proposal).expect("proposal prepares");
    apply_codex_conversation_proposal(
        project_dir,
        CodexConversationApplyRequest {
            proposal,
            action_ids: prepared.action_ids,
            review_approved: false,
            session_id: None,
        },
    )
    .expect("apply")
}

fn volume_proposal() -> CodexConversationEditProposal {
    CodexConversationEditProposal {
        summary: "Lowered the interview.".to_string(),
        edl: Vec::new(),
        project_actions: vec![ProjectAction::UpdateAudioVolume {
            item_id: "audio-1".to_string(),
            volume_db: Some(-2.0),
        }],
        render_review: None,
    }
}

/// The project with its CAS token cleared, for content comparisons.
fn content(project: &VideoProject) -> VideoProject {
    let mut project = project.clone();
    project.content_revision = 0;
    project
}

#[test]
fn undo_restores_the_pre_apply_snapshot_and_marks_the_turn_undone() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    record_conversation_turn(&project_dir, &before);
    let before = load_split_project(&project_dir).expect("load project");
    let applied = apply_safe(&project_dir, &before, volume_proposal());

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&applied.history_entry_id))
        .expect("undo");

    let ProjectAgentUndoOutcome::Undone {
        project,
        entry_id,
        action_count,
        remaining_agent_history,
        warnings,
        ..
    } = outcome
    else {
        panic!("expected undone outcome, got {outcome:?}");
    };
    let persisted = load_split_project(&project_dir).expect("load undone project");
    assert_eq!(*project, persisted);
    assert_eq!(content(&persisted), content(&before));
    assert_eq!(
        persisted.content_revision,
        applied.project.content_revision + 1
    );
    assert_eq!(entry_id, applied.history_entry_id);
    assert_eq!(action_count, 1);
    assert_eq!(remaining_agent_history, 0);
    assert!(warnings.is_empty());
    assert_eq!(agent_history_json(&project_dir)["entries"], json!([]));
    let sessions = load_agent_session_manifest(&project_dir).expect("sessions");
    let turn = sessions.sessions[0].turns.last().expect("turn");
    assert_eq!(turn.proposal_status, "undone");
    assert_eq!(sessions.sessions[0].proposal_status, "undone");
}

#[test]
fn undo_reports_a_conflict_after_a_later_user_edit_without_writing() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let applied = apply_safe(&project_dir, &before, volume_proposal());
    let mut edited = applied.project.clone();
    edited.timeline.tracks[0].items[0].label = "Manual edit".to_string();
    save_split_project(&project_dir, &edited).expect("user edit");
    let snapshot = folder_snapshot(&project_dir);

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&applied.history_entry_id))
        .expect("undo outcome");

    let ProjectAgentUndoOutcome::Conflict { entry_id, message } = outcome else {
        panic!("expected conflict, got {outcome:?}");
    };
    assert_eq!(entry_id, applied.history_entry_id);
    assert!(message.contains("changed after this edit was applied"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn undo_ignores_revision_and_thread_bookkeeping_saves() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let applied = apply_safe(&project_dir, &before, volume_proposal());
    // A later conversation turn only stores the thread id and bumps the revision.
    let mut bookkeeping = applied.project.clone();
    bookkeeping.codex_thread_id = Some("thread-2".to_string());
    replace_split_project_if_revision(&project_dir, bookkeeping, applied.project.content_revision)
        .expect("bookkeeping save");

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo");

    assert!(matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }));
    let persisted = load_split_project(&project_dir).expect("load undone project");
    assert_eq!(persisted.codex_thread_id.as_deref(), Some("thread-2"));
    assert_eq!(persisted.timeline, before.timeline);
}

#[test]
fn undo_of_an_older_entry_conflicts_with_the_newer_batch() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let first = apply_safe(&project_dir, &before, volume_proposal());
    let mut second_proposal = volume_proposal();
    second_proposal.project_actions = vec![ProjectAction::UpdateAudioVolume {
        item_id: "audio-1".to_string(),
        volume_db: Some(-6.0),
    }];
    let second = apply_safe(&project_dir, &first.project, second_proposal);
    assert_ne!(first.history_entry_id, second.history_entry_id);

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&first.history_entry_id))
        .expect("undo outcome");

    let ProjectAgentUndoOutcome::Conflict { entry_id, message } = outcome else {
        panic!("expected conflict, got {outcome:?}");
    };
    assert_eq!(entry_id, first.history_entry_id);
    assert!(message.contains("newer agent edit"));
}

#[test]
fn undo_without_history_is_unavailable() {
    let (_dir, project_dir, _before) = saved_split_project(conversation_fixture_project());

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo outcome");

    let ProjectAgentUndoOutcome::Unavailable { message } = outcome else {
        panic!("expected unavailable, got {outcome:?}");
    };
    assert!(message.contains("no agent edit to undo"));
}

#[test]
fn legacy_full_snapshot_history_entries_still_load_and_undo() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let mut changed = before.clone();
    changed.name = "Agent edit".to_string();
    let after = save_split_project(&project_dir, &changed)
        .expect("save after")
        .project;
    let legacy = json!({
        "schemaVersion": 1,
        "entries": [{ "id": "agent-edit-1", "actionCount": 1, "before": before, "after": after }],
    });
    std::fs::write(
        project_dir.join("context").join("agent-history.json"),
        serde_json::to_vec_pretty(&legacy).expect("legacy json"),
    )
    .expect("write legacy history");

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo");

    assert!(matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }));
    assert_eq!(
        load_split_project(&project_dir).expect("load").name,
        before.name
    );
}

#[test]
fn speed_aware_clips_survive_apply_and_undo() {
    let (_dir, project_dir, before) = saved_split_project(speed_fixture_project());
    apply_safe(&project_dir, &before, ripple_proposal());

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo");

    assert!(matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }));
    let restored = load_split_project(&project_dir).expect("load undone project");
    assert_eq!(restored.timeline, before.timeline);
    assert_eq!(restored.timelines, before.timelines);
    let video = &restored.timeline.tracks[0].items[0];
    assert_eq!(video.properties["speed"], json!(2.0));
    assert_eq!(video.properties["sourceOut"], json!(20.0));
}
