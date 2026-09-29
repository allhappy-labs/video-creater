use super::support::{
    conversation_fixture_project, media_item, sample_conversation_request, sample_skill_bundle,
    FakeTransport,
};
use serde_json::json;
use std::fs;
use video_creater_lib::codex::app_server::{
    build_codex_conversation_turn_request, start_codex_conversation_turn, CodexAppServerError,
};
use video_creater_lib::codex::context::build_codex_conversation_context;
use video_creater_lib::codex::conversation::CodexConversationError;
use video_creater_lib::project::model::TimelineItemKind;
use video_creater_lib::project::split::{
    load_agent_session_manifest, load_app_server_conversation_history, save_split_project,
};

#[test]
fn conversation_turn_request_is_preset_free_and_uses_conversation_schema() {
    let project = conversation_fixture_project();
    let request = sample_conversation_request();
    let context = build_codex_conversation_context(&project, &request, None);

    let turn = build_codex_conversation_turn_request(9, "thread-123", &context);

    assert_eq!(turn["method"], json!("turn/start"));
    assert_eq!(turn["params"]["threadId"], json!("thread-123"));
    assert_eq!(turn["params"]["approvalPolicy"], json!("never"));
    let text = turn["params"]["input"][0]["text"].as_str().expect("prompt");
    assert!(text.contains("Return only a structured project-edit proposal."));
    assert!(text.contains("Use projectActions for edits to the existing timeline."));
    assert!(text.contains("Include an EDL before adding a new primary video/audio cut."));
    assert!(text.contains("Do not infer a trailer, highlight, or story preset."));
    assert!(text.contains("Remove dead air from the interview."));
    assert!(text.contains("audio-1"));
    assert!(!text.contains("preset:"));
    assert!(!text.contains("targetDurationSeconds"));
    assert!(!text.contains("captionStyle"));
    assert!(!text.contains("languageMode"));

    let schema = &turn["params"]["outputSchema"];
    assert_eq!(
        schema["required"],
        json!(["summary", "edl", "projectActions", "renderReview"])
    );
    assert_eq!(schema["additionalProperties"], json!(false));
    assert!(schema["properties"]["edl"].get("minItems").is_none());
    assert!(schema["properties"].get("mediaId").is_none());
    assert!(schema["properties"]["projectActions"]["items"]["anyOf"]
        .as_array()
        .is_some_and(|variants| !variants.is_empty()));
    assert!(schema["properties"]["renderReview"]["anyOf"]
        .as_array()
        .is_some_and(|variants| variants.iter().any(|variant| variant["type"] == "null")));
}

#[test]
fn conversation_turn_validates_proposal_and_records_forward_compatible_history() {
    let dir = tempfile::tempdir().expect("project parent");
    let project_dir = dir.path().join("project");
    let mut project = conversation_fixture_project();
    project.schema_version = 2;
    save_split_project(&project_dir, &project).expect("save project");
    let proposal = json!({
        "summary": "Lowered the interview clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }],
        "renderReview": null,
    });
    let mut transport = FakeTransport::new(proposal);

    let result = start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        Some(&project_dir),
    )
    .expect("conversation turn");

    assert_eq!(result.thread_id, "thread-1");
    assert_eq!(project.codex_thread_id.as_deref(), Some("thread-1"));
    assert_eq!(
        result
            .proposal
            .as_ref()
            .map(|proposal| proposal.summary.as_str()),
        Some("Lowered the interview clip by 2 dB.")
    );
    assert_eq!(result.proposal_validation_issues, Some(Vec::new()));
    let turn_request = transport
        .requests
        .iter()
        .find(|request| request["method"] == "turn/start")
        .expect("turn request");
    assert_eq!(
        turn_request["params"]["outputSchema"]["required"][0],
        json!("summary")
    );

    let history = load_app_server_conversation_history(&project_dir).expect("history");
    assert_eq!(history.entries.len(), 1);
    let entry = &history.entries[0];
    assert_eq!(entry.prompt, "Remove dead air from the interview.");
    assert_eq!(
        entry.request["focus"]["timelineItemIds"],
        json!(["audio-1"])
    );
    assert!(entry.request.get("preset").is_none());
    assert!(entry.has_proposal);
    let sessions = load_agent_session_manifest(&project_dir).expect("sessions");
    assert_eq!(sessions.sessions.len(), 1);
    assert_eq!(
        sessions.sessions[0].title,
        "Remove dead air from the interview."
    );
    assert_eq!(sessions.sessions[0].created_at, "2026-07-25T00:00:00Z");
}

#[test]
fn conversation_turn_returns_validation_issues_for_invalid_proposals() {
    let mut project = conversation_fixture_project();
    let proposal = json!({
        "summary": "Added a second shot.",
        "edl": [],
        "projectActions": [{
            "type": "addItems",
            "targetTrackId": "track-video",
            "items": [serde_json::to_value(media_item("video-2", TimelineItemKind::VideoClip, 10.0, 2.0)).unwrap()],
        }],
        "renderReview": null,
    });
    let mut transport = FakeTransport::new(proposal);

    let result = start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        None,
    )
    .expect("invalid proposals stay reviewable");

    assert!(result.proposal.is_some());
    let issues = result
        .proposal_validation_issues
        .expect("validation should run");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].path, "edl");
}

#[test]
fn conversation_turn_rejects_invalid_request_before_contacting_codex() {
    let mut project = conversation_fixture_project();
    let mut request = sample_conversation_request();
    request.prompt = " ".to_string();
    let mut transport = FakeTransport::new(json!({}));

    let error = start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        request,
        &sample_skill_bundle(),
        None,
    )
    .expect_err("empty prompt is rejected");

    assert_eq!(
        error,
        CodexAppServerError::ConversationRequest(CodexConversationError::EmptyPrompt)
    );
    assert!(transport.requests.is_empty());
}

#[test]
fn legacy_edit_job_history_and_conversation_history_both_load() {
    let dir = tempfile::tempdir().expect("project parent");
    let project_dir = dir.path().join("project");
    let mut project = conversation_fixture_project();
    project.schema_version = 2;
    save_split_project(&project_dir, &project).expect("save project");
    let history_path = project_dir.join("context/app-server-conversations.json");
    fs::create_dir_all(history_path.parent().expect("context dir")).expect("context dir");
    fs::write(
        &history_path,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "entries": [{
                "id": "app-server-turn-1",
                "projectId": project.id,
                "threadId": "thread-legacy",
                "turnId": "turn-legacy",
                "turnStatus": "completed",
                "prompt": "Build a concise cut",
                "request": {
                    "mediaId": "media-1",
                    "preset": "trailer_cut",
                    "prompt": "Build a concise cut",
                    "targetDurationSeconds": 45.0,
                    "languageMode": "auto",
                    "captionStyle": "bold",
                    "createdAt": "2026-07-19T13:00:00Z"
                },
                "hasProposal": true,
                "threadResponse": {},
                "turnResponse": {}
            }]
        }))
        .expect("legacy history json"),
    )
    .expect("write legacy history");

    let mut transport = FakeTransport::new(json!({
        "summary": "Lowered the interview clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }],
        "renderReview": null,
    }));
    start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        Some(&project_dir),
    )
    .expect("conversation turn");

    let history = load_app_server_conversation_history(&project_dir).expect("history loads");
    assert_eq!(history.entries.len(), 2);
    assert_eq!(history.entries[0].prompt, "Build a concise cut");
    assert_eq!(history.entries[0].request["preset"], json!("trailer_cut"));
    assert_eq!(
        history.entries[1].prompt,
        "Remove dead air from the interview."
    );
    assert_eq!(
        history.entries[1].request["createdAt"],
        json!("2026-07-25T00:00:00Z")
    );
}
