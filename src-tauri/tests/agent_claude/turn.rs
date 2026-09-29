//! A whole conversation turn, driven by a scripted transport.
//!
//! This is the test that proves the seam did not weaken anything: a Claude-shaped outcome and a
//! Codex-shaped outcome go through exactly the same validation, produce exactly the same action
//! ids and the same risk, and the guards downstream of the turn still refuse what they always
//! refused.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use video_creater_lib::agent::fake_transport::FakeAgentTurnTransport;
use video_creater_lib::agent::turn::{
    build_agent_conversation_request, run_agent_conversation_turn,
};
use video_creater_lib::agent::{
    AgentBackendKind, AgentTurnError, AgentTurnOutcome, AgentTurnRequest,
};
use video_creater_lib::codex::context::ProjectSkillBundle;
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal, CodexConversationApplyRequest, CodexConversationEditRequest,
    CodexConversationFocus, CodexProposalRiskLevel,
};
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use video_creater_lib::project::split::{
    load_app_server_conversation_history, record_app_server_conversation_turn, save_split_project,
};

const HIDDEN_CONTEXT_MARKER: &str = "Remove dead air from the interview.";

fn fixture_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-conversation".to_string(),
        "Conversation".to_string(),
        "2026-07-25T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: Some("Interview".to_string()),
        relative_path: "media/interview.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 30.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    project.timeline.duration_seconds = 10.0;
    for track in &mut project.timeline.tracks {
        match track.kind {
            TrackKind::Video => track
                .items
                .push(item("video-1", TimelineItemKind::VideoClip)),
            TrackKind::Audio => track
                .items
                .push(item("audio-1", TimelineItemKind::AudioClip)),
            _ => {}
        }
    }
    project.timelines[0].timeline = project.timeline.clone();
    project
}

fn item(id: &str, kind: TimelineItemKind) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds: 0.0,
        duration_seconds: 10.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Interview".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.0)),
            ("sourceOut".to_string(), json!(10.0)),
        ]),
    }
}

fn conversation_request() -> CodexConversationEditRequest {
    CodexConversationEditRequest {
        prompt: HIDDEN_CONTEXT_MARKER.to_string(),
        focus: CodexConversationFocus {
            primary_media_id: None,
            media_ids: Vec::new(),
            timeline_item_ids: vec!["audio-1".to_string()],
            timeline_range: None,
        },
        created_at: "2026-07-25T00:00:00Z".to_string(),
    }
}

/// A clip-property edit: allow-listed, so it classifies as safe.
fn safe_proposal() -> Value {
    json!({
        "summary": "Lowered the interview clip's volume.",
        "edl": [],
        "projectActions": [{
            "type": "updateAudioVolume",
            "itemId": "audio-1",
            "volume": 0.6
        }],
        "renderReview": Value::Null
    })
}

/// A delete: the risk allowlist sends it to review.
fn review_proposal() -> Value {
    json!({
        "summary": "Removed the interview clip.",
        "edl": [],
        "projectActions": [{
            "type": "removeItems",
            "itemIds": ["audio-1"]
        }],
        "renderReview": Value::Null
    })
}

fn outcome(backend: AgentBackendKind, proposal: Value) -> AgentTurnOutcome {
    let session = match backend {
        AgentBackendKind::Codex => "thread-1".to_string(),
        AgentBackendKind::Claude => "0406a3dc-a644-4397-ae6e-a9552d78429d".to_string(),
    };
    let transcript = match backend {
        AgentBackendKind::Codex => json!({
            "threadResponse": { "thread": { "id": session, "preview": HIDDEN_CONTEXT_MARKER } },
            "turnResponse": {
                "id": "turn-1",
                "status": "completed",
                "items": [
                    { "type": "userMessage", "text": HIDDEN_CONTEXT_MARKER },
                    { "type": "agentMessage", "text": "Done." }
                ]
            }
        }),
        AgentBackendKind::Claude => json!({
            "frames": [{ "type": "system", "subtype": "init", "session_id": session }],
            "toolCalls": ["mcp__video-creater__video_creater_get_timeline"],
            "permissionDenials": [],
        }),
    };
    AgentTurnOutcome {
        session,
        proposal: Some(proposal),
        model: Some("a-model".to_string()),
        notice: None,
        transcript,
    }
}

fn request_for(project: &VideoProject, root: &Path) -> AgentTurnRequest {
    build_agent_conversation_request(
        project,
        &conversation_request(),
        root,
        Some(root),
        &ProjectSkillBundle {
            agents_md: String::new(),
            video_pipeline: String::new(),
            graphics: String::new(),
            visuals: String::new(),
            shader_background_catalog: String::new(),
        },
        None,
    )
}

fn run(
    backend: AgentBackendKind,
    result: Result<AgentTurnOutcome, AgentTurnError>,
    project: &VideoProject,
    root: &Path,
) -> (
    Result<video_creater_lib::agent::turn::AgentConversationTurn, AgentTurnError>,
    Vec<AgentTurnRequest>,
) {
    let mut transport = FakeAgentTurnTransport::once(backend, result);
    let turn_request = request_for(project, root);
    let outcome = run_agent_conversation_turn(
        &mut transport,
        project,
        &conversation_request(),
        &turn_request,
        Instant::now(),
        None,
    );
    (outcome, transport.seen)
}

fn project_on_disk() -> (tempfile::TempDir, std::path::PathBuf, VideoProject) {
    let directory = tempfile::tempdir().expect("a temp dir");
    let root = directory.path().join("Project");
    fs::create_dir_all(&root).expect("a project dir");
    let project = fixture_project();
    save_split_project(&root, &project).expect("a saved project");
    let project = video_creater_lib::project::split::load_split_project(&root).expect("a project");
    (directory, root, project)
}

#[test]
fn both_backends_produce_the_same_prepared_proposal() {
    let (_guard, root, project) = project_on_disk();

    let (codex, _) = run(
        AgentBackendKind::Codex,
        Ok(outcome(AgentBackendKind::Codex, safe_proposal())),
        &project,
        &root,
    );
    let (claude, _) = run(
        AgentBackendKind::Claude,
        Ok(outcome(AgentBackendKind::Claude, safe_proposal())),
        &project,
        &root,
    );

    let codex = codex.expect("a Codex turn");
    let claude = claude.expect("a Claude turn");

    assert_eq!(codex.proposal, claude.proposal);
    let codex_prepared = codex.prepared_proposal.expect("a Codex bundle");
    let claude_prepared = claude.prepared_proposal.expect("a Claude bundle");
    assert_eq!(codex_prepared.action_ids, claude_prepared.action_ids);
    assert_eq!(codex_prepared.actions, claude_prepared.actions);
    assert_eq!(codex_prepared.risk.level, claude_prepared.risk.level);
    assert_eq!(codex_prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert!(codex_prepared
        .action_ids
        .iter()
        .all(|id| id.starts_with("codex-action-")));
}

#[test]
fn a_delete_classifies_as_review_for_either_backend() {
    let (_guard, root, project) = project_on_disk();

    for backend in [AgentBackendKind::Codex, AgentBackendKind::Claude] {
        let (turn, _) = run(
            backend,
            Ok(outcome(backend, review_proposal())),
            &project,
            &root,
        );
        let prepared = turn.expect("a turn").prepared_proposal.expect("a bundle");
        assert_eq!(
            prepared.risk.level,
            CodexProposalRiskLevel::Review,
            "{backend:?}"
        );
    }
}

#[test]
fn the_transport_sees_the_rendered_context_and_the_real_schema() {
    let (_guard, root, project) = project_on_disk();

    let (_, seen) = run(
        AgentBackendKind::Claude,
        Ok(outcome(AgentBackendKind::Claude, safe_proposal())),
        &project,
        &root,
    );

    let seen = seen.first().expect("one request");
    // The prompt is the hidden adaptive context, not the user's words on their own.
    assert!(seen.prompt.contains(HIDDEN_CONTEXT_MARKER));
    assert_ne!(seen.prompt.trim(), HIDDEN_CONTEXT_MARKER);
    assert!(seen.prompt.len() > HIDDEN_CONTEXT_MARKER.len() * 4);
    assert!(!seen.developer_instructions.is_empty());

    // The real schema, not a stand-in: the action union is the big one.
    let actions = seen.output_schema["properties"]["projectActions"]["items"]["anyOf"]
        .as_array()
        .expect("the action union");
    assert!(actions.len() > 40, "only {} variants", actions.len());
}

#[test]
fn an_interruption_withholds_the_proposal_and_writes_no_history() {
    let (_guard, root, project) = project_on_disk();

    let (turn, _) = run(
        AgentBackendKind::Claude,
        Err(AgentTurnError::Interrupted),
        &project,
        &root,
    );

    assert!(matches!(
        turn.expect_err("an interruption"),
        AgentTurnError::Interrupted
    ));
    let history = load_app_server_conversation_history(&root).expect("a history file");
    assert!(history.entries.is_empty());
}

#[test]
fn an_authentication_failure_surfaces_as_an_unavailable_agent() {
    let (_guard, root, project) = project_on_disk();

    let (turn, _) = run(
        AgentBackendKind::Claude,
        Err(AgentTurnError::NotAuthenticated(
            "Not logged in · Please run /login".to_string(),
        )),
        &project,
        &root,
    );

    match turn.expect_err("a failure") {
        AgentTurnError::NotAuthenticated(detail) => {
            assert!(detail.contains("/login"), "{detail}")
        }
        other => panic!("expected NotAuthenticated, got {other:?}"),
    }
}

#[test]
fn the_persisted_history_carries_no_echoed_hidden_context() {
    let (_guard, root, project) = project_on_disk();

    let (turn, _) = run(
        AgentBackendKind::Codex,
        Ok(outcome(AgentBackendKind::Codex, safe_proposal())),
        &project,
        &root,
    );
    let turn = turn.expect("a turn");
    record_app_server_conversation_turn(&root, &project.id, "thread-1", turn.record)
        .expect("a recorded turn");

    let raw = fs::read_to_string(root.join("context").join("app-server-conversations.json"))
        .expect("the history file");
    let history: Value = serde_json::from_str(&raw).expect("valid JSON");
    let entry = &history["entries"][0];

    // The prompt field is the user's own words and stays. The echoed context does not: no
    // `userMessage` item and no thread preview survive.
    assert_eq!(entry["prompt"], HIDDEN_CONTEXT_MARKER);
    assert_eq!(entry["threadResponse"]["thread"].get("preview"), None);
    let items = entry["turnResponse"]["items"]
        .as_array()
        .expect("the turn items");
    assert!(items
        .iter()
        .all(|item| item["type"].as_str() != Some("userMessage")));
}

#[test]
fn apply_still_refuses_a_mutated_action_id_list() {
    let (_guard, root, project) = project_on_disk();
    let (turn, _) = run(
        AgentBackendKind::Claude,
        Ok(outcome(AgentBackendKind::Claude, safe_proposal())),
        &project,
        &root,
    );
    let turn = turn.expect("a turn");
    let proposal = turn.proposal.expect("a proposal");
    let mut action_ids = turn.prepared_proposal.expect("a bundle").action_ids;
    action_ids[0] = format!("{}-tampered", action_ids[0]);

    let error = apply_codex_conversation_proposal(
        &root,
        CodexConversationApplyRequest {
            proposal,
            action_ids,
            review_approved: false,
            session_id: None,
        },
    )
    .expect_err("a stale proposal");

    assert!(
        format!("{error:?}").contains("Stale"),
        "expected a stale-proposal refusal, got {error:?}"
    );
}

#[test]
fn apply_still_refuses_an_unapproved_review_bundle() {
    let (_guard, root, project) = project_on_disk();
    let (turn, _) = run(
        AgentBackendKind::Claude,
        Ok(outcome(AgentBackendKind::Claude, review_proposal())),
        &project,
        &root,
    );
    let turn = turn.expect("a turn");
    let proposal = turn.proposal.expect("a proposal");
    let action_ids = turn.prepared_proposal.expect("a bundle").action_ids;

    let error = apply_codex_conversation_proposal(
        &root,
        CodexConversationApplyRequest {
            proposal,
            action_ids,
            review_approved: false,
            session_id: None,
        },
    )
    .expect_err("an unapproved review bundle");

    assert!(
        format!("{error:?}").contains("Review"),
        "expected a review refusal, got {error:?}"
    );
}

#[test]
fn a_claude_turn_records_its_provider_and_a_codex_turn_does_not() {
    let (_guard, root, project) = project_on_disk();

    let (claude, _) = run(
        AgentBackendKind::Claude,
        Ok(outcome(AgentBackendKind::Claude, safe_proposal())),
        &project,
        &root,
    );
    let claude = claude.expect("a turn").record;
    assert_eq!(claude.provider.as_deref(), Some("claude"));
    assert_eq!(
        claude.provider_session_id.as_deref(),
        Some("0406a3dc-a644-4397-ae6e-a9552d78429d")
    );

    let (codex, _) = run(
        AgentBackendKind::Codex,
        Ok(outcome(AgentBackendKind::Codex, safe_proposal())),
        &project,
        &root,
    );
    let codex = codex.expect("a turn").record;
    assert_eq!(codex.provider, None);
    assert_eq!(codex.provider_session_id, None);
}
