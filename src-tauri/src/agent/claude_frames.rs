//! Parsing the Claude stream-json turn.
//!
//! The parser takes lines, not a process, so every branch is testable against recorded
//! fixtures and no test ever spends a token.
//!
//! The one rule that matters: **classify on the `result` frame, never on the exit code.**
//! An unauthenticated run exits 1 *and* prints a well-formed error result; a budget-exhausted
//! run exits 0 with `is_error: true`. The frames are the truth.

use super::prompt::scrub_echoed_user_input;
use super::AgentTurnError;
use serde_json::{json, Value};

/// One line of the stream, classified.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaudeStreamFrame {
    /// `system`/`init`: the session id, the model, and the MCP servers that attached.
    Init(Value),
    /// An assistant message, possibly carrying `thinking`, `text` or `tool_use` blocks.
    Assistant(Value),
    /// A synthetic `user` frame: a tool result, or the CLI's own structured-output nudge.
    User(Value),
    /// A rate-limit report. Present on healthy turns too.
    RateLimit(Value),
    /// The terminal frame.
    Result(Value),
    /// Any other frame the CLI emits (`system`/`thinking_tokens`, and whatever comes next).
    Other(Value),
    /// A line that is not JSON at all. The CLI prints plain warnings to the same stream, so
    /// this is skipped rather than fatal.
    Unparsed,
}

/// Classify one line.
pub fn parse_claude_stream_frame(line: &str) -> ClaudeStreamFrame {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return ClaudeStreamFrame::Unparsed;
    };
    let Some(object) = value.as_object() else {
        return ClaudeStreamFrame::Unparsed;
    };

    match object.get("type").and_then(Value::as_str) {
        Some("system") if object.get("subtype").and_then(Value::as_str) == Some("init") => {
            ClaudeStreamFrame::Init(value)
        }
        Some("assistant") => ClaudeStreamFrame::Assistant(value),
        Some("user") => ClaudeStreamFrame::User(value),
        Some("rate_limit_event") => ClaudeStreamFrame::RateLimit(value),
        Some("result") => ClaudeStreamFrame::Result(value),
        _ => ClaudeStreamFrame::Other(value),
    }
}

/// What one turn produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeTurnSummary {
    pub session_id: String,
    pub model: Option<String>,
    /// `None` is "the agent did not suggest an edit", a first-class app state.
    pub proposal: Option<Value>,
    /// `(server name, status)` from the init frame. An empty list on a turn that needed the
    /// project is a wiring bug, not a user-facing error.
    pub mcp_servers: Vec<(String, String)>,
    pub tool_calls: Vec<String>,
    pub permission_denials: Vec<Value>,
    pub cost_usd: Option<f64>,
    pub notice: Option<String>,
    /// What the app persists, already scrubbed of the echoed hidden context.
    pub transcript: Value,
}

/// Reduce a stream to a summary, or to the failure it reported.
pub fn summarize_claude_stream<I, S>(lines: I) -> Result<ClaudeTurnSummary, AgentTurnError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut session_id = String::new();
    let mut model = None;
    let mut mcp_servers = Vec::new();
    let mut tool_calls = Vec::new();
    let mut rate_limit: Option<Value> = None;
    let mut assistant_error: Option<String> = None;
    let mut result: Option<Value> = None;
    let mut transcript = Vec::new();

    for line in lines {
        let line = line.as_ref().trim();
        if line.is_empty() {
            continue;
        }
        match parse_claude_stream_frame(line) {
            ClaudeStreamFrame::Unparsed => continue,
            ClaudeStreamFrame::Init(frame) => {
                session_id = string_field(&frame, "session_id").unwrap_or(session_id);
                model = string_field(&frame, "model").or(model);
                mcp_servers = frame
                    .get("mcp_servers")
                    .and_then(Value::as_array)
                    .map(|servers| {
                        servers
                            .iter()
                            .filter_map(|server| {
                                Some((
                                    string_field(server, "name")?,
                                    string_field(server, "status")
                                        .unwrap_or_else(|| "unknown".to_string()),
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                transcript.push(frame);
            }
            ClaudeStreamFrame::Assistant(frame) => {
                if let Some(error) = string_field(&frame, "error") {
                    assistant_error = Some(error);
                }
                tool_calls.extend(tool_names(&frame));
                transcript.push(frame);
            }
            ClaudeStreamFrame::User(frame) => transcript.push(frame),
            ClaudeStreamFrame::RateLimit(frame) => {
                rate_limit = Some(frame.clone());
                transcript.push(frame);
            }
            ClaudeStreamFrame::Result(frame) => {
                session_id = string_field(&frame, "session_id").unwrap_or(session_id);
                result = Some(frame);
            }
            ClaudeStreamFrame::Other(_) => continue,
        }
    }

    // A stream that ends without a `result` frame is the verified cancel signal: SIGTERM ends
    // the process at exit 143 with the frames it had already written and nothing more.
    let Some(result) = result else {
        return Err(AgentTurnError::Interrupted);
    };

    if result.get("is_error").and_then(Value::as_bool) == Some(true) {
        return Err(classify_error(
            &result,
            assistant_error.as_deref(),
            rate_limit.as_ref(),
        ));
    }

    let proposal = result
        .get("structured_output")
        .filter(|value| !value.is_null())
        .cloned()
        .or_else(|| {
            // The result text carries the same JSON as a string. Reading it back is a
            // robustness fallback, not the contract.
            string_field(&result, "result")
                .and_then(|text| serde_json::from_str::<Value>(text.trim()).ok())
                .filter(Value::is_object)
        });

    let permission_denials = result
        .get("permission_denials")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cost_usd = result.get("total_cost_usd").and_then(Value::as_f64);
    let notice = usage_notice(rate_limit.as_ref());

    transcript.push(result);
    let transcript = drop_echoed_user_text(transcript);

    Ok(ClaudeTurnSummary {
        session_id,
        model,
        proposal,
        mcp_servers,
        tool_calls,
        permission_denials,
        cost_usd,
        notice,
        transcript: scrub_echoed_user_input(json!({ "frames": transcript })),
    })
}

/// Drop the frames that echo text back at the user.
///
/// Claude reports the turn's own input as synthetic `user` frames carrying `text` blocks. For
/// a conversation turn that text is the hidden adaptive context, which must never reach stored
/// history — the same rule `scrub_echoed_user_input` enforces for Codex's `userMessage` items,
/// in the shape Claude uses. `tool_result` frames are kept: they are the agent's own reading of
/// the project, not an echo of the prompt.
fn drop_echoed_user_text(frames: Vec<Value>) -> Vec<Value> {
    frames
        .into_iter()
        .filter(|frame| {
            if frame.get("type").and_then(Value::as_str) != Some("user") {
                return true;
            }
            frame
                .pointer("/message/content")
                .and_then(Value::as_array)
                .is_some_and(|blocks| {
                    blocks
                        .iter()
                        .any(|block| block.get("type").and_then(Value::as_str) != Some("text"))
                })
        })
        .collect()
}

fn classify_error(
    result: &Value,
    assistant_error: Option<&str>,
    rate_limit: Option<&Value>,
) -> AgentTurnError {
    let detail = string_field(result, "result")
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| {
            string_field(result, "subtype").unwrap_or_else(|| "the turn failed".to_string())
        });

    if assistant_error == Some("authentication_failed") || mentions_sign_in(&detail) {
        // The CLI's own words, so settings can echo exactly what the user must fix.
        return AgentTurnError::NotAuthenticated(detail);
    }

    if mentions_usage_limit(&detail) || rate_limit_exhausted(rate_limit) {
        let reset = rate_limit.and_then(reset_at);
        let detail = match reset {
            Some(reset) if !detail.contains(&reset) => format!("{detail} (resets at {reset})"),
            _ => detail,
        };
        return AgentTurnError::UsageLimit { detail };
    }

    AgentTurnError::TurnFailed(detail)
}

fn mentions_sign_in(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("not logged in") || detail.contains("please run /login")
}

fn mentions_usage_limit(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    detail.contains("usage limit") || detail.contains("rate limit")
}

fn rate_limit_exhausted(rate_limit: Option<&Value>) -> bool {
    rate_limit
        .and_then(|frame| frame.pointer("/rate_limit_info/status"))
        .and_then(Value::as_str)
        .is_some_and(|status| status == "exhausted" || status == "rejected")
}

fn reset_at(rate_limit: &Value) -> Option<String> {
    rate_limit
        .pointer("/rate_limit_info/resetsAt")
        .and_then(Value::as_i64)
        .map(|seconds| seconds.to_string())
}

/// A healthy turn that is close to its limit still deserves a plain-language heads-up.
fn usage_notice(rate_limit: Option<&Value>) -> Option<String> {
    let status = rate_limit
        .and_then(|frame| frame.pointer("/rate_limit_info/status"))
        .and_then(Value::as_str)?;
    (status != "allowed").then(|| format!("Claude reported its usage window as {status}."))
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn tool_names(frame: &Value) -> Vec<String> {
    frame
        .pointer("/message/content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
                .filter_map(|block| string_field(block, "name"))
                .collect()
        })
        .unwrap_or_default()
}
