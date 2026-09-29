//! Chats map one-to-one onto provider sessions.
//!
//! `thread_id` stays Codex's, so every project written before backends were pluggable keeps
//! working untouched. A Claude chat additionally stores the UUID the app minted, which the
//! next turn passes to `--resume`.

use serde_json::json;
use std::fs;
use std::path::Path;
use video_creater_lib::agent::claude_cli::{
    build_claude_turn_argv, claude_mcp_config_json, ClaudeSessionHandle, ClaudeTurnInvocation,
};
use video_creater_lib::project::model::VideoProject;
use video_creater_lib::project::split::{
    load_agent_session_manifest, record_app_server_conversation_turn, save_split_project,
    AppServerConversationTurn,
};

fn project_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().expect("a temp dir");
    let path = directory.path().join("Project");
    fs::create_dir_all(&path).expect("a project dir");
    let project = VideoProject::new_empty(
        "project-1".to_string(),
        "Sessions".to_string(),
        "2026-09-18T00:00:00Z".to_string(),
    );
    save_split_project(&path, &project).expect("a saved project");
    (directory, path)
}

fn turn(
    prompt: &str,
    provider: Option<&str>,
    provider_session_id: Option<&str>,
) -> AppServerConversationTurn {
    AppServerConversationTurn {
        turn_id: Some(format!("turn-{prompt}")),
        turn_status: Some("completed".to_string()),
        prompt: prompt.to_string(),
        created_at: "2026-09-18T00:00:00Z".to_string(),
        request: json!({ "prompt": prompt }),
        thread_response: json!({ "thread": { "id": "thread-1" } }),
        turn_response: json!({ "id": "turn-1", "status": "completed" }),
        has_proposal: false,
        provider: provider.map(str::to_string),
        provider_session_id: provider_session_id.map(str::to_string),
    }
}

fn manifest_path(dir: &Path) -> std::path::PathBuf {
    dir.join("context").join("agent-sessions.json")
}

fn record(dir: &Path, thread_id: &str, turn: AppServerConversationTurn) {
    record_app_server_conversation_turn(dir, "project-1", thread_id, turn)
        .expect("a recorded turn");
}

#[test]
fn a_claude_chat_stores_its_provider_and_a_uuid_session_handle() {
    let (_guard, dir) = project_dir();
    let session_id = uuid::Uuid::new_v4().to_string();

    record(
        &dir,
        "thread-1",
        turn("Trim the intro.", Some("claude"), Some(&session_id)),
    );

    let manifest = load_agent_session_manifest(&dir).expect("a manifest");
    assert_eq!(manifest.sessions.len(), 1);
    let session = &manifest.sessions[0];
    assert_eq!(session.provider.as_deref(), Some("claude"));
    assert_eq!(
        session.provider_session_id.as_deref(),
        Some(session_id.as_str())
    );
    assert!(
        uuid::Uuid::parse_str(session.provider_session_id.as_deref().expect("a handle")).is_ok()
    );
}

#[test]
fn a_second_turn_reuses_the_handle_and_the_argv_then_resumes_it() {
    let (_guard, dir) = project_dir();
    let session_id = uuid::Uuid::new_v4().to_string();

    record(
        &dir,
        "thread-1",
        turn("First.", Some("claude"), Some(&session_id)),
    );
    record(
        &dir,
        "thread-1",
        turn("Second.", Some("claude"), Some(&session_id)),
    );

    let manifest = load_agent_session_manifest(&dir).expect("a manifest");
    assert_eq!(
        manifest.sessions.len(),
        1,
        "the second turn started a new chat"
    );
    assert_eq!(manifest.sessions[0].turns.len(), 2);

    let stored = manifest.sessions[0]
        .provider_session_id
        .clone()
        .expect("a stored handle");
    assert_eq!(stored, session_id);

    let args = build_claude_turn_argv(&ClaudeTurnInvocation {
        model: "sonnet".to_string(),
        fallback_model: None,
        developer_instructions: "instructions".to_string(),
        output_schema: json!({ "type": "object" }),
        session: ClaudeSessionHandle::Resume(stored.clone()),
        mcp_config: claude_mcp_config_json(Path::new("/opt/sidecar"), &dir),
        budget_usd: 0.5,
    });
    let resume = args
        .iter()
        .position(|arg| arg == "--resume")
        .expect("--resume");
    assert_eq!(args[resume + 1], stored);
    assert!(!args.iter().any(|arg| arg == "--session-id"));
}

#[test]
fn a_codex_chat_keeps_writing_thread_id_and_leaves_the_new_fields_unset() {
    let (_guard, dir) = project_dir();

    record(&dir, "thread-1", turn("Trim the intro.", None, None));

    let manifest = load_agent_session_manifest(&dir).expect("a manifest");
    let session = &manifest.sessions[0];
    assert_eq!(session.thread_id.as_deref(), Some("thread-1"));
    assert_eq!(session.provider, None);
    assert_eq!(session.provider_session_id, None);

    // Nothing new is serialized either, so a downgrade reads the file unchanged.
    let raw = fs::read_to_string(manifest_path(&dir)).expect("the manifest file");
    assert!(!raw.contains("provider"), "{raw}");
}

#[test]
fn switching_the_backend_inside_one_chat_records_the_new_provider_and_keeps_the_thread() {
    let (_guard, dir) = project_dir();
    record(&dir, "thread-1", turn("Codex first.", None, None));

    let claude_session = uuid::Uuid::new_v4().to_string();
    record(
        &dir,
        "thread-1",
        turn("Claude next.", Some("claude"), Some(&claude_session)),
    );

    let manifest = load_agent_session_manifest(&dir).expect("a manifest");
    assert_eq!(manifest.sessions.len(), 1, "the switch forked the chat");
    let session = &manifest.sessions[0];
    assert_eq!(session.thread_id.as_deref(), Some("thread-1"));
    assert_eq!(session.provider.as_deref(), Some("claude"));
    assert_eq!(
        session.provider_session_id.as_deref(),
        Some(claude_session.as_str())
    );
}

#[test]
fn a_manifest_written_before_this_change_still_loads() {
    let (_guard, dir) = project_dir();
    let path = manifest_path(&dir);
    fs::create_dir_all(path.parent().expect("a context dir")).expect("a context dir");
    fs::write(
        &path,
        json!({
            "schemaVersion": 1,
            "projectId": "project-1",
            "activeSessionId": "agent-session-1",
            "sessions": [{
                "id": "agent-session-1",
                "title": "Old chat",
                "threadId": "thread-legacy",
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z",
                "turns": [],
                "proposalStatus": "none",
                "appliedActionIds": []
            }]
        })
        .to_string(),
    )
    .expect("a legacy manifest");

    let manifest = load_agent_session_manifest(&dir).expect("a manifest");
    assert_eq!(manifest.sessions.len(), 1);
    assert_eq!(manifest.sessions[0].provider, None);
    assert_eq!(manifest.sessions[0].provider_session_id, None);
}
