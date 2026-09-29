use super::*;
use crate::project::fixtures::sample_project;
use std::collections::BTreeMap;

fn folder_snapshot(project_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read project folder") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).expect("relative");
                files.insert(
                    relative.display().to_string(),
                    std::fs::read(&path).expect("read file"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(project_dir, project_dir, &mut files);
    files
}

fn move_action() -> ProjectAction {
    serde_json::from_value(serde_json::json!({
        "type": "moveItems",
        "moves": [{ "itemId": "item-1", "targetTrackId": "track-video", "startSeconds": 1.25 }],
    }))
    .expect("move action")
}

fn fail_after_commit(checkpoint: AgentBatchCheckpoint) -> Result<(), SplitProjectError> {
    assert_eq!(checkpoint, AgentBatchCheckpoint::ProjectCommitted);
    Err(SplitProjectError::Io {
        path: "agent-history.json".to_string(),
        message: "injected history write failure".to_string(),
    })
}

#[test]
fn agent_project_action_batch_restores_the_project_when_history_write_fails() {
    let temp = tempfile::tempdir().expect("temporary project");
    let project_dir = temp.path().join("project");
    save_split_project(&project_dir, &sample_project()).expect("save project");
    let snapshot = folder_snapshot(&project_dir);

    let error = apply_batch_with_hook(
        &project_dir,
        vec![move_action()],
        vec!["codex-action-1-a".to_string()],
        None,
        &mut fail_after_commit,
    )
    .expect_err("history failure fails the batch");

    assert!(error.to_string().contains("injected history write failure"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn agent_project_batch_undo_restores_the_applied_project_when_history_write_fails() {
    let temp = tempfile::tempdir().expect("temporary project");
    let project_dir = temp.path().join("project");
    save_split_project(&project_dir, &sample_project()).expect("save project");
    apply_agent_project_action_batch(
        &project_dir,
        vec![move_action()],
        vec!["codex-action-1-a".to_string()],
        None,
    )
    .expect("apply batch");
    let snapshot = folder_snapshot(&project_dir);

    let error = undo_batch_with_hook(&project_dir, None, &mut fail_after_commit)
        .expect_err("history failure fails the undo");

    assert!(error.to_string().contains("injected history write failure"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}
