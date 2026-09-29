//! Frame parsing, against recorded CLI streams. One fixture per branch, no process, no tokens.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use video_creater_lib::agent::claude_frames::{
    parse_claude_stream_frame, summarize_claude_stream, ClaudeStreamFrame, ClaudeTurnSummary,
};
use video_creater_lib::agent::AgentTurnError;

fn fixture(name: &str) -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("claude_agent")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is unreadable: {error}", path.display()))
        .lines()
        .map(str::to_string)
        .collect()
}

fn summary(name: &str) -> ClaudeTurnSummary {
    summarize_claude_stream(fixture(name))
        .unwrap_or_else(|error| panic!("{name} should summarize: {error}"))
}

#[test]
fn a_successful_turn_with_an_mcp_tool_call_yields_a_proposal() {
    let summary = summary("success-with-mcp-tool.jsonl");

    assert!(!summary.session_id.is_empty());
    assert_eq!(summary.model.as_deref(), Some("claude-haiku-4-5-20251001"));
    assert_eq!(
        summary.mcp_servers,
        vec![("video-creater".to_string(), "connected".to_string())]
    );
    assert!(summary
        .tool_calls
        .iter()
        .any(|tool| tool.starts_with("mcp__video-creater__")));
    assert!(summary.tool_calls.contains(&"StructuredOutput".to_string()));
    assert!(summary.permission_denials.is_empty());
    assert!(summary.cost_usd.is_some_and(|cost| cost > 0.0));
    assert_eq!(summary.notice, None);

    let proposal = summary.proposal.expect("a proposal");
    assert!(proposal.get("summary").and_then(Value::as_str).is_some());
    assert!(proposal
        .get("projectActions")
        .and_then(Value::as_array)
        .is_some());
}

#[test]
fn a_plain_warning_line_is_skipped_rather_than_fatal() {
    // The CLI prints warnings such as "Warning: no stdin data received in 3s…" to the same
    // stream as the frames. The fixture carries one.
    assert!(fixture("success-with-mcp-tool.jsonl")
        .iter()
        .any(|line| line.starts_with("Warning:")));
    assert!(summarize_claude_stream(fixture("success-with-mcp-tool.jsonl")).is_ok());

    assert_eq!(
        parse_claude_stream_frame("Warning: no stdin data received in 3s, waiting..."),
        ClaudeStreamFrame::Unparsed
    );
    assert_eq!(
        parse_claude_stream_frame("[1,2,3]"),
        ClaudeStreamFrame::Unparsed
    );
}

#[test]
fn a_successful_turn_without_structured_output_has_no_proposal_and_no_error() {
    let summary = summary("success-no-proposal.jsonl");

    assert_eq!(summary.proposal, None);
    assert!(summary.permission_denials.is_empty());
}

#[test]
fn a_result_string_is_read_back_when_structured_output_is_missing() {
    let mut lines = fixture("success-no-proposal.jsonl");
    let last = lines.pop().expect("a result frame");
    let mut result: Value = serde_json::from_str(&last).expect("valid JSON");
    result["result"] = Value::String(r#"{"summary":"Trimmed","projectActions":[]}"#.to_string());
    lines.push(result.to_string());

    let summary = summarize_claude_stream(lines).expect("a summary");
    let proposal = summary.proposal.expect("the fallback proposal");
    assert_eq!(proposal["summary"], "Trimmed");
}

#[test]
fn structured_output_wins_over_the_result_string() {
    let mut lines = fixture("success-with-mcp-tool.jsonl");
    let last = lines.pop().expect("a result frame");
    let mut result: Value = serde_json::from_str(&last).expect("valid JSON");
    result["structured_output"] = serde_json::json!({ "summary": "parsed", "projectActions": [] });
    result["result"] =
        Value::String(r#"{"summary":"stringified","projectActions":[]}"#.to_string());
    lines.push(result.to_string());

    let summary = summarize_claude_stream(lines).expect("a summary");
    assert_eq!(summary.proposal.expect("a proposal")["summary"], "parsed");
}

#[test]
fn an_authentication_failure_surfaces_the_clis_own_words() {
    let error = summarize_claude_stream(fixture("auth-failed.jsonl")).expect_err("an error");

    match error {
        // Settings echo this, so it has to be the CLI's message rather than one of ours.
        AgentTurnError::NotAuthenticated(detail) => {
            assert!(detail.contains("Please run /login"), "{detail}");
        }
        other => panic!("expected NotAuthenticated, got {other:?}"),
    }
}

#[test]
fn a_rate_limited_turn_reports_the_usage_limit_with_its_reset() {
    let error = summarize_claude_stream(fixture("rate-limited.jsonl")).expect_err("an error");

    match error {
        AgentTurnError::UsageLimit { detail } => {
            assert!(detail.to_lowercase().contains("usage limit"), "{detail}");
            assert!(detail.contains("reset"), "{detail}");
        }
        other => panic!("expected UsageLimit, got {other:?}"),
    }
}

#[test]
fn a_stream_that_ends_without_a_result_frame_is_a_cancellation() {
    // Verified: SIGTERM ends the process at exit 143 with no `result` frame. That absence is
    // the signal — the exit code never is.
    let error =
        summarize_claude_stream(fixture("cancelled-no-result.jsonl")).expect_err("an error");
    assert!(matches!(error, AgentTurnError::Interrupted), "{error:?}");
}

#[test]
fn the_retained_transcript_carries_no_echoed_user_text() {
    let summary = summary("success-with-mcp-tool.jsonl");
    let frames = summary.transcript["frames"].as_array().expect("frames");

    for frame in frames {
        if frame.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let blocks = frame
            .pointer("/message/content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            blocks
                .iter()
                .any(|block| block.get("type").and_then(Value::as_str) != Some("text")),
            "a user frame made of plain text survived the scrub: {frame}"
        );
    }

    // The raw stream did contain such a frame, so the assertion above is not vacuous.
    let raw_text_frames = fixture("success-with-mcp-tool.jsonl")
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|frame| frame.get("type").and_then(Value::as_str) == Some("user"))
        .filter(|frame| {
            frame
                .pointer("/message/content")
                .and_then(Value::as_array)
                .is_some_and(|blocks| {
                    blocks
                        .iter()
                        .all(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                })
        })
        .count();
    assert!(
        raw_text_frames > 0,
        "the fixture no longer exercises the scrub"
    );
}

#[test]
fn frame_classification_covers_every_type_the_cli_emits() {
    assert!(matches!(
        parse_claude_stream_frame(r#"{"type":"system","subtype":"init","session_id":"s"}"#),
        ClaudeStreamFrame::Init(_)
    ));
    assert!(matches!(
        parse_claude_stream_frame(r#"{"type":"system","subtype":"thinking_tokens"}"#),
        ClaudeStreamFrame::Other(_)
    ));
    assert!(matches!(
        parse_claude_stream_frame(r#"{"type":"assistant","message":{}}"#),
        ClaudeStreamFrame::Assistant(_)
    ));
    assert!(matches!(
        parse_claude_stream_frame(r#"{"type":"user","message":{}}"#),
        ClaudeStreamFrame::User(_)
    ));
    assert!(matches!(
        parse_claude_stream_frame(r#"{"type":"rate_limit_event"}"#),
        ClaudeStreamFrame::RateLimit(_)
    ));
    assert!(matches!(
        parse_claude_stream_frame(r#"{"type":"result","is_error":false}"#),
        ClaudeStreamFrame::Result(_)
    ));
}
