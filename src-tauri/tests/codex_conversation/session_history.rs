//! Each recorded conversation turn names the chat session it was filed under.

use super::support::{
    conversation_fixture_project, sample_conversation_request, sample_skill_bundle,
    saved_split_project, FakeTransport,
};
use serde_json::{json, Value};
use std::path::Path;
use video_creater_lib::codex::app_server::start_codex_conversation_turn;
use video_creater_lib::project::model::VideoProject;
use video_creater_lib::project::split::{
    apply_agent_session_action, load_agent_session_manifest, load_app_server_conversation_history,
    record_app_server_conversation_turn, AgentSessionAction, AppServerConversationTurn,
    SplitAppServerConversationFile,
};

fn record_turn(project_dir: &Path, project: &VideoProject, thread_id: &str, turn_id: &str) {
    record_app_server_conversation_turn(
        project_dir,
        &project.id,
        thread_id,
        AppServerConversationTurn {
            turn_id: Some(turn_id.to_string()),
            turn_status: Some("completed".to_string()),
            prompt: format!("Prompt for {turn_id}."),
            created_at: "2026-09-16T10:00:00Z".to_string(),
            request: json!({ "prompt": format!("Prompt for {turn_id}.") }),
            thread_response: json!({}),
            turn_response: json!({}),
            has_proposal: false,
            provider: None,
            provider_session_id: None,
        },
    )
    .expect("record conversation turn");
}

fn create_session(project_dir: &Path, project: &VideoProject, id: &str) {
    apply_agent_session_action(
        project_dir,
        &project.id,
        AgentSessionAction::Create {
            id: id.to_string(),
            title: "Second chat".to_string(),
            thread_id: None,
            timestamp: "2026-09-16T10:05:00Z".to_string(),
        },
    )
    .expect("create session");
    apply_agent_session_action(
        project_dir,
        &project.id,
        AgentSessionAction::Select {
            session_id: id.to_string(),
        },
    )
    .expect("select session");
}

fn recorded_session_ids(project_dir: &Path) -> Vec<Option<String>> {
    load_app_server_conversation_history(project_dir)
        .expect("conversation history")
        .entries
        .into_iter()
        .map(|entry| entry.session_id)
        .collect()
}

#[test]
fn a_turn_records_the_session_it_was_filed_under() {
    let (_dir, project_dir, project) = saved_split_project(conversation_fixture_project());

    record_turn(&project_dir, &project, "thread-1", "turn-1");

    assert_eq!(
        recorded_session_ids(&project_dir),
        vec![Some("agent-session-1".to_string())]
    );
}

#[test]
fn a_turn_in_a_new_active_chat_records_that_chat() {
    let (_dir, project_dir, project) = saved_split_project(conversation_fixture_project());
    record_turn(&project_dir, &project, "thread-1", "turn-1");
    create_session(&project_dir, &project, "chat-b");

    record_turn(&project_dir, &project, "thread-2", "turn-2");

    assert_eq!(
        recorded_session_ids(&project_dir),
        vec![
            Some("agent-session-1".to_string()),
            Some("chat-b".to_string())
        ]
    );
    let manifest = load_agent_session_manifest(&project_dir).expect("session manifest");
    let chat_b = manifest
        .sessions
        .iter()
        .find(|session| session.id == "chat-b")
        .expect("chat b");
    assert_eq!(chat_b.thread_id.as_deref(), Some("thread-2"));
}

#[test]
fn a_turn_on_an_earlier_thread_records_that_threads_session() {
    let (_dir, project_dir, project) = saved_split_project(conversation_fixture_project());
    record_turn(&project_dir, &project, "thread-1", "turn-1");
    create_session(&project_dir, &project, "chat-b");

    record_turn(&project_dir, &project, "thread-1", "turn-2");

    assert_eq!(
        recorded_session_ids(&project_dir),
        vec![
            Some("agent-session-1".to_string()),
            Some("agent-session-1".to_string())
        ]
    );
}

#[test]
fn legacy_entries_without_a_session_still_load() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    let legacy = json!({
        "schemaVersion": 1,
        "entries": [{
            "id": "app-server-turn-1",
            "projectId": "project-1",
            "threadId": "thread-1",
            "prompt": "Tighten the intro.",
            "request": { "prompt": "Tighten the intro." },
            "hasProposal": false,
            "threadResponse": {},
            "turnResponse": {}
        }]
    });
    let path = project_dir
        .join("context")
        .join("app-server-conversations.json");
    std::fs::create_dir_all(path.parent().expect("context dir")).expect("context dir");
    std::fs::write(&path, serde_json::to_vec_pretty(&legacy).expect("json")).expect("write");

    let history = load_app_server_conversation_history(&project_dir).expect("history");
    assert_eq!(history.entries[0].session_id, None);
    let reserialized = serde_json::to_value::<&SplitAppServerConversationFile>(&history)
        .expect("serialize history");
    assert!(reserialized["entries"][0].get("sessionId").is_none());
}

/// One conversation turn on `transport`, returning its thread id and the `thread/start` or
/// `thread/resume` request it sent. Reusing one transport gives every started thread a new id.
fn conversation_turn(
    transport: &mut FakeTransport,
    project_dir: &Path,
    project: &mut VideoProject,
) -> (String, Value) {
    let before = transport.requests.len();
    let result = start_codex_conversation_turn(
        transport,
        1,
        "/tmp/video-creater",
        project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        Some(project_dir),
    )
    .expect("conversation turn");
    let thread_request = transport.requests[before..]
        .iter()
        .find(|request| {
            request["method"] == "thread/start" || request["method"] == "thread/resume"
        })
        .map(|request| json!({ "method": request["method"], "threadId": request["params"]["threadId"] }))
        .expect("thread request");
    (result.thread_id, thread_request)
}

fn conversation_transport() -> FakeTransport {
    FakeTransport::new(json!({
        "summary": "Lowered the interview clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }],
        "renderReview": null,
    }))
}

fn session_thread(project_dir: &Path, session_id: &str) -> Option<String> {
    load_agent_session_manifest(project_dir)
        .expect("session manifest")
        .sessions
        .into_iter()
        .find(|session| session.id == session_id)
        .and_then(|session| session.thread_id)
}

#[test]
fn a_new_chat_starts_its_own_thread_instead_of_resuming_the_project_thread() {
    let (_dir, project_dir, mut project) = saved_split_project(conversation_fixture_project());
    let mut transport = conversation_transport();
    let (first_thread, _) = conversation_turn(&mut transport, &project_dir, &mut project);
    assert_eq!(first_thread, "thread-1");
    assert_eq!(project.codex_thread_id.as_deref(), Some("thread-1"));
    create_session(&project_dir, &project, "chat-b");

    // The project thread still names chat A's thread; chat B has none yet.
    let (chat_b_thread, request) = conversation_turn(&mut transport, &project_dir, &mut project);

    assert_eq!(
        request,
        json!({ "method": "thread/start", "threadId": null })
    );
    assert_ne!(chat_b_thread, first_thread);
    assert_eq!(
        recorded_session_ids(&project_dir),
        vec![
            Some("agent-session-1".to_string()),
            Some("chat-b".to_string())
        ]
    );
    assert_eq!(
        session_thread(&project_dir, "agent-session-1").as_deref(),
        Some("thread-1")
    );
    assert_eq!(
        session_thread(&project_dir, "chat-b"),
        Some(chat_b_thread.clone())
    );

    // Later turns resume the active chat's own thread.
    let (_, request) = conversation_turn(&mut transport, &project_dir, &mut project);
    assert_eq!(
        request,
        json!({ "method": "thread/resume", "threadId": chat_b_thread })
    );
    apply_agent_session_action(
        &project_dir,
        &project.id,
        AgentSessionAction::Select {
            session_id: "agent-session-1".to_string(),
        },
    )
    .expect("select chat a");
    let (_, request) = conversation_turn(&mut transport, &project_dir, &mut project);
    assert_eq!(
        request,
        json!({ "method": "thread/resume", "threadId": "thread-1" })
    );
    assert_eq!(
        recorded_session_ids(&project_dir),
        vec![
            Some("agent-session-1".to_string()),
            Some("chat-b".to_string()),
            Some("chat-b".to_string()),
            Some("agent-session-1".to_string())
        ]
    );
}

#[test]
fn a_project_without_chats_resumes_its_legacy_project_thread() {
    let mut legacy = conversation_fixture_project();
    legacy.codex_thread_id = Some("thread-legacy".to_string());
    let (_dir, project_dir, mut project) = saved_split_project(legacy);
    let mut transport = conversation_transport();

    let (thread_id, request) = conversation_turn(&mut transport, &project_dir, &mut project);

    assert_eq!(
        request,
        json!({ "method": "thread/resume", "threadId": "thread-legacy" })
    );
    assert_eq!(thread_id, "thread-legacy");
    assert_eq!(
        session_thread(&project_dir, "agent-session-1").as_deref(),
        Some("thread-legacy")
    );
}
