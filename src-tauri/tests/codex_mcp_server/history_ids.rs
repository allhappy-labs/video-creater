//! MCP agent edits get history entry ids that never repeat, even after the 20-entry history
//! starts dropping its oldest entries, and Undo bound to an id still finds its entry.

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;
use video_creater_lib::codex::tools::call_codex_local_tool;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::split::{
    apply_agent_project_action_batch, load_split_project, record_agent_project_action_batch,
    save_split_project, undo_latest_agent_project_batch, ProjectAgentUndoOutcome,
};

const APPLIES: usize = 25;

fn mcp_move(dir: &Path, index: usize) -> String {
    let project = load_split_project(dir).expect("load project");
    let apply = call_codex_local_tool(
        &project,
        "video_creater.apply_project_actions",
        json!({
            "projectDir": dir.display().to_string(),
            "actions": [{
                "type": "moveItems",
                "moves": [{
                    "itemId": "item-1",
                    "targetTrackId": "track-video",
                    "startSeconds": 1.0 + index as f64 * 0.25
                }]
            }]
        }),
    )
    .expect("apply project actions");
    apply.payload["agentHistory"]["latestEntryId"]
        .as_str()
        .expect("latest entry id")
        .to_string()
}

fn history_ids(dir: &Path) -> Vec<String> {
    let history: Value = serde_json::from_slice(
        &std::fs::read(dir.join("context").join("agent-history.json")).expect("agent history"),
    )
    .expect("agent history json");
    history["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| entry["id"].as_str().expect("entry id").to_string())
        .collect()
}

#[test]
fn mcp_applies_never_reuse_a_history_entry_id() {
    let dir = tempfile::tempdir().expect("project dir");
    save_split_project(dir.path(), &sample_project()).expect("save split project");

    let mut returned = Vec::new();
    for index in 0..APPLIES {
        let entry_id = mcp_move(dir.path(), index);
        let revision = load_split_project(dir.path())
            .expect("load moved project")
            .content_revision;
        assert_eq!(entry_id, format!("agent-edit-r{revision}"));
        returned.push(entry_id);
    }

    let distinct: BTreeSet<_> = returned.iter().collect();
    assert_eq!(distinct.len(), APPLIES, "{returned:?}");
    let retained = history_ids(dir.path());
    assert_eq!(retained.len(), 20);
    assert_eq!(retained, returned[APPLIES - 20..].to_vec());
}

#[test]
fn undo_bound_to_an_earlier_mcp_entry_does_not_undo_a_later_one_after_the_history_is_full() {
    let dir = tempfile::tempdir().expect("project dir");
    save_split_project(dir.path(), &sample_project()).expect("save split project");
    for index in 0..APPLIES - 2 {
        mcp_move(dir.path(), index);
    }
    let earlier = mcp_move(dir.path(), APPLIES - 2);
    let latest = mcp_move(dir.path(), APPLIES - 1);

    let refused = undo_latest_agent_project_batch(dir.path(), Some(&earlier)).expect("stale undo");
    assert!(
        !matches!(refused, ProjectAgentUndoOutcome::Undone { .. }),
        "an Undo bound to an earlier edit must not undo the latest edit: {refused:?}"
    );

    match undo_latest_agent_project_batch(dir.path(), Some(&latest)).expect("undo") {
        ProjectAgentUndoOutcome::Undone { entry_id, .. } => assert_eq!(entry_id, latest),
        other => panic!("expected the latest MCP edit to be undone, got {other:?}"),
    }
}

#[test]
fn a_history_with_count_based_ids_still_loads_and_takes_new_mcp_entries() {
    let dir = tempfile::tempdir().expect("project dir");
    save_split_project(dir.path(), &sample_project()).expect("save split project");
    for index in 0..3 {
        mcp_move(dir.path(), index);
    }
    // Rewrite the stored ids the way MCP applies used to number them.
    let path = dir.path().join("context").join("agent-history.json");
    let mut history: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("agent history")).expect("json");
    for (index, entry) in history["entries"]
        .as_array_mut()
        .expect("entries")
        .iter_mut()
        .enumerate()
    {
        entry["id"] = json!(format!("agent-edit-{}", index + 1));
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&history).expect("json")).expect("write");

    match undo_latest_agent_project_batch(dir.path(), Some("agent-edit-3")).expect("legacy undo") {
        ProjectAgentUndoOutcome::Undone { entry_id, .. } => assert_eq!(entry_id, "agent-edit-3"),
        other => panic!("expected the legacy entry to be undone, got {other:?}"),
    }

    let entry_id = mcp_move(dir.path(), 10);
    assert!(entry_id.starts_with("agent-edit-r"), "{entry_id}");
    assert_eq!(
        history_ids(dir.path()),
        vec![
            "agent-edit-1".to_string(),
            "agent-edit-2".to_string(),
            entry_id.clone()
        ]
    );
    let editor = apply_agent_project_action_batch(
        dir.path(),
        vec![serde_json::from_value(json!({
            "type": "moveItems",
            "moves": [{ "itemId": "item-1", "targetTrackId": "track-video", "startSeconds": 9.0 }]
        }))
        .expect("move action")],
        vec!["editor-move".to_string()],
        None,
    )
    .expect("editor apply");
    assert_ne!(editor.history_entry_id, entry_id);
}

#[test]
fn recording_twice_at_one_content_revision_still_gives_distinct_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = save_split_project(dir.path(), &sample_project())
        .expect("save split project")
        .project;

    let first = record_agent_project_action_batch(dir.path(), project.clone(), project.clone(), 1)
        .expect("first record")
        .latest_entry_id;
    let second = record_agent_project_action_batch(dir.path(), project.clone(), project.clone(), 1)
        .expect("second record")
        .latest_entry_id;

    let revision = project.content_revision;
    assert_eq!(first, format!("agent-edit-r{revision}"));
    assert_eq!(second, format!("agent-edit-r{revision}-2"));
    assert_eq!(history_ids(dir.path()), vec![first, second]);
}
