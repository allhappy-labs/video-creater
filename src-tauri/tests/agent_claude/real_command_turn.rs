//! One real turn on the path the app itself takes, with the preference set to Claude.
//!
//! The other real-CLI test proves the argv and the schema survive contact with the binary.
//! This one proves the *wiring*: the stored `agentBackend` preference is what chooses the
//! backend, the stored model is what reaches `--model`, the app's own MCP sidecar attaches,
//! and the proposal that comes back goes through the same validation every Codex turn does.
//!
//! It spends money, so it is gated twice, exactly as `real_turn.rs` is: `#[ignore]` keeps it
//! out of every normal run and `VIDEO_CREATER_CLAUDE_REAL_TURN=1` keeps it out of an
//! `--include-ignored` sweep.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use video_creater_lib::agent::claude_cli::{
    claude_allowed_tools_argument, resolve_claude_executable,
};
use video_creater_lib::agent::turn::{
    agent_turn_preferences, build_agent_conversation_request, plan_agent_turn,
    run_agent_conversation_turn, AgentTurnBackendPlan,
};
use video_creater_lib::agent::AgentBackendKind;
use video_creater_lib::codex::context::load_project_skill_bundle;
use video_creater_lib::codex::conversation::{
    conversation_proposal_from_value, CodexConversationEditRequest, CodexConversationFocus,
};
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use video_creater_lib::project::split::save_split_project;
use video_creater_lib::settings::agent::claude_turn_readiness;
use video_creater_lib::settings::preferences::{
    AgentBackendPreference, AppPreferencesPatch, AppPreferencesStore, ClaudeModelPreference,
};

#[test]
#[ignore = "spends money against the real Claude CLI; set VIDEO_CREATER_CLAUDE_REAL_TURN=1"]
fn a_stored_claude_preference_runs_a_real_turn_end_to_end() {
    if std::env::var("VIDEO_CREATER_CLAUDE_REAL_TURN").as_deref() != Ok("1") {
        eprintln!("skipped: VIDEO_CREATER_CLAUDE_REAL_TURN is not 1");
        return;
    }
    let sidecar = mcp_sidecar_path();
    assert!(
        sidecar.is_file(),
        "build the sidecar first: cargo build --bin video-creater-mcp-server --features mcp-server"
    );

    let temp = tempfile::tempdir().expect("a temp dir");
    let root = temp.path().to_path_buf();
    // The projects root's own policy file, which `load_project_skill_bundle` requires and the
    // real command always finds. Kept short so the turn's cost is the wiring, not the prose.
    fs::write(
        root.join("AGENTS.md"),
        "# Project policy\n\nPropose edits only. Never invent ids.\n",
    )
    .expect("a project policy");
    let project_dir = root.join("Claude Wiring Probe");
    fs::create_dir_all(&project_dir).expect("a project dir");
    let project = probe_project();
    save_split_project(&project_dir, &project).expect("a saved project");

    // The setting, stored exactly as Advanced → Agent stores it.
    let store = AppPreferencesStore::new(root.join("preferences.json"));
    store
        .update(AppPreferencesPatch {
            agent_backend: Some(AgentBackendPreference::Claude),
            claude_model: Some(ClaudeModelPreference::Haiku),
            ..AppPreferencesPatch::default()
        })
        .expect("the preference must be stored");
    let preferences = agent_turn_preferences(&store);
    assert!(
        resolve_claude_executable(preferences.claude_executable.as_deref()).is_some(),
        "the claude CLI must be installed"
    );

    // Codex is reported ready, so only the stored preference can send this turn to Claude.
    // Readiness comes from the real zero-token probes, as it does in the command.
    let readiness = claude_turn_readiness(preferences.claude_executable_arg());
    assert!(
        readiness.is_ready(),
        "the claude CLI must be signed in: {readiness:?}"
    );
    let (selection, plan) =
        plan_agent_turn(&preferences, true, readiness, sidecar).expect("the turn must be planned");
    assert_eq!(selection.kind, AgentBackendKind::Claude);
    let AgentTurnBackendPlan::Claude(mut transport) = plan else {
        panic!("the stored preference must route the turn to Claude");
    };

    let request = CodexConversationEditRequest {
        prompt: "Trim the first clip to its first two seconds.".to_string(),
        focus: CodexConversationFocus {
            primary_media_id: None,
            media_ids: Vec::new(),
            timeline_item_ids: Vec::new(),
            timeline_range: None,
        },
        created_at: "2026-09-18T00:00:00Z".to_string(),
    };
    let skills = load_project_skill_bundle(&root).expect("a skill bundle");
    let turn_request = build_agent_conversation_request(
        &project,
        &request,
        &root,
        Some(&project_dir),
        &skills,
        None,
    );

    let turn = run_agent_conversation_turn(
        &mut *transport,
        &project,
        &request,
        &turn_request,
        Instant::now() + Duration::from_secs(300),
        None,
    )
    .expect("the real turn must complete");

    let transcript = &turn.record.turn_response;
    eprintln!(
        "costUsd={:?} mcpServers={} toolCalls={}",
        transcript.get("costUsd"),
        transcript.get("mcpServers").unwrap_or(&Value::Null),
        transcript.get("toolCalls").unwrap_or(&Value::Null),
    );
    retain_stream(transcript);

    let proposal = turn
        .proposal
        .as_ref()
        .expect("the turn must carry a proposal");
    assert!(
        turn.prepared_proposal.is_some(),
        "the proposal must validate: {:?}",
        turn.proposal_validation_issues
    );
    assert_real_turn_evidence(transcript, &proposal.summary);
}

/// Replay the retained transcript through the same checks the real turn runs.
///
/// The real turn costs money and runs once; this keeps its conclusions under test for free, and
/// keeps the checks themselves honest — they have to pass against a real stream, not a hand-made
/// one.
#[test]
fn the_retained_command_turn_still_satisfies_the_evidence_checks() {
    let transcript: Value = serde_json::from_str(
        &fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/claude_agent/real-command-turn-2026-09-18.json"),
        )
        .expect("the retained transcript"),
    )
    .expect("the retained transcript must be JSON");

    let proposal = conversation_proposal_from_value(
        transcript
            .get("frames")
            .and_then(Value::as_array)
            .and_then(|frames| frames.last())
            .and_then(|result| result.get("structured_output"))
            .expect("the retained result frame must carry structured output"),
    )
    .expect("the retained proposal must parse");
    assert_eq!(proposal.project_actions.len(), 1);

    assert_real_turn_evidence(&transcript, &proposal.summary);
}

/// What one real turn through the command path has to show.
fn assert_real_turn_evidence(transcript: &Value, summary: &str) {
    assert!(
        !summary.trim().is_empty(),
        "the proposal must carry a summary"
    );

    // The app's own MCP sidecar attached under the name the allowlist is written against.
    let servers = transcript
        .get("mcpServers")
        .and_then(Value::as_array)
        .expect("the transcript must record the MCP servers");
    assert!(
        servers.iter().any(|server| {
            server.get("name").and_then(Value::as_str) == Some("video-creater")
                && server.get("status").and_then(Value::as_str) == Some("connected")
        }),
        "the app's MCP sidecar must be connected: {servers:?}"
    );

    // Whether the turn *needs* to read the project is the model's judgment — the hidden context
    // often already answers the question. What is not the model's judgment is what it is allowed
    // to call, so every tool it did call must be on the read-only surface, and nothing may have
    // been refused.
    let allowed = claude_allowed_tools_argument();
    for tool in transcript
        .get("toolCalls")
        .and_then(Value::as_array)
        .expect("the transcript must record the tool calls")
        .iter()
        .filter_map(Value::as_str)
    {
        assert!(
            tool == "StructuredOutput" || allowed.split(',').any(|permitted| permitted == tool),
            "the turn called a tool outside the read-only surface: {tool}"
        );
    }
    assert_eq!(
        transcript
            .get("permissionDenials")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "nothing outside the read-only surface may have been attempted"
    );

    // Nothing internal reaches what the user reads.
    let session = transcript
        .get("frames")
        .and_then(Value::as_array)
        .and_then(|frames| frames.first())
        .and_then(|init| init.get("session_id"))
        .and_then(Value::as_str)
        .expect("the init frame must carry the session id");
    for internal in [session, "mcp__", "media-1", "video-1", "StructuredOutput"] {
        assert!(
            !summary.contains(internal),
            "the summary leaks {internal:?}: {summary}"
        );
    }
}

fn mcp_sidecar_path() -> PathBuf {
    if let Some(configured) = std::env::var_os("VIDEO_CREATER_CLAUDE_REAL_TURN_MCP_SERVER") {
        return PathBuf::from(configured);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("video-creater-mcp-server")
}

/// Keep the persisted transcript beside the raw stream the other real turn retained: this one
/// is what the app actually stores, so it is the evidence that history carries no echoed
/// context.
fn retain_stream(transcript: &Value) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("output")
        .join("claude-agent-backend");
    fs::create_dir_all(&directory).expect("an output dir");
    fs::write(
        directory.join("real-command-turn-2026-09-18.json"),
        serde_json::to_vec_pretty(transcript).expect("a serializable transcript"),
    )
    .expect("the transcript must be retained");
}

fn probe_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "claude-wiring-probe".to_string(),
        "Claude Wiring Probe".to_string(),
        "2026-09-18T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: Some("input.mp4".to_string()),
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 12.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    // One real clip, so "trim the first clip" is an edit the project can actually take.
    project.timeline.duration_seconds = 12.0;
    for track in &mut project.timeline.tracks {
        if track.kind == TrackKind::Video {
            track.items.push(TimelineItem {
                id: "video-1".to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 0.0,
                duration_seconds: 12.0,
                source: TimelineSource::Media {
                    media_id: "media-1".to_string(),
                },
                label: "Input".to_string(),
                properties: std::collections::BTreeMap::from([
                    ("sourceIn".to_string(), serde_json::json!(0.0)),
                    ("sourceOut".to_string(), serde_json::json!(12.0)),
                ]),
            });
        }
    }
    project.timelines[0].timeline = project.timeline.clone();
    project
}
