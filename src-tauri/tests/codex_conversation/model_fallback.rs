//! A Codex config can select a model the bundled app-server can't use. The turn
//! must switch to a supported model and say so, instead of failing.

use super::support::{
    conversation_fixture_project, sample_conversation_request, sample_skill_bundle,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use video_creater_lib::codex::app_server::{
    start_codex_conversation_turn, AppServerCleanupReport, AppServerMessage, CodexAppServerError,
    CodexAppServerTransport,
};

const NEWER_CODEX_ERROR: &str = "The 'gpt-5.6-sol' model requires a newer version of Codex. \
Please upgrade to the latest app or CLI and try again.";

/// Answers by method like the pinned app-server: `thread/start` reports the
/// configured model, `model/list` reports what this build supports, and a turn
/// on an unsupported model fails with the server's upgrade error.
struct ModelServer {
    configured_model: Option<&'static str>,
    /// `model/list` pages; a request with cursor `page-N` gets page N (1-based).
    model_list: Result<Vec<Value>, (i64, &'static str)>,
    failure_message: &'static str,
    unsupported_models: Vec<&'static str>,
    turn_start_rpc_error: bool,
    requests: Vec<Value>,
    messages: VecDeque<AppServerMessage>,
}

impl ModelServer {
    fn new(configured_model: Option<&'static str>) -> Self {
        Self {
            configured_model,
            model_list: Ok(vec![json!({
                "data": [
                    model_entry("gpt-5.5", false, true),
                    model_entry("codex-auto-review", true, false),
                ],
                "nextCursor": null,
            })]),
            failure_message: NEWER_CODEX_ERROR,
            unsupported_models: vec!["gpt-5.6-sol"],
            turn_start_rpc_error: false,
            requests: Vec::new(),
            messages: VecDeque::new(),
        }
    }

    fn methods(&self) -> Vec<&str> {
        self.requests
            .iter()
            .map(|request| request["method"].as_str().unwrap_or_default())
            .collect()
    }

    fn turn_requests(&self) -> Vec<&Value> {
        self.requests
            .iter()
            .filter(|request| request["method"] == "turn/start")
            .collect()
    }
}

fn model_entry(model: &str, hidden: bool, is_default: bool) -> Value {
    json!({
        "id": model,
        "model": model,
        "displayName": model,
        "description": "",
        "hidden": hidden,
        "isDefault": is_default,
        "defaultReasoningEffort": "medium",
        "supportedReasoningEfforts": [],
    })
}

fn proposal_text() -> String {
    json!({
        "summary": "Lowered the interview clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }],
        "renderReview": null,
    })
    .to_string()
}

impl CodexAppServerTransport for ModelServer {
    fn send(&mut self, request: Value) -> Result<(), CodexAppServerError> {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default().to_string();
        let page = request
            .pointer("/params/cursor")
            .and_then(Value::as_str)
            .and_then(|cursor| cursor.strip_prefix("page-"))
            .and_then(|page| page.parse::<usize>().ok())
            .unwrap_or(1);
        let turn_model = request
            .pointer("/params/model")
            .and_then(Value::as_str)
            .or(self.configured_model)
            .map(str::to_string);
        self.requests.push(request);
        match method.as_str() {
            "initialize" => self.messages.push_back(AppServerMessage::Response {
                id,
                result: json!({ "userAgent": "codex-fixture" }),
            }),
            "thread/start" | "thread/resume" => {
                let mut result = json!({ "thread": { "id": "thread-1" } });
                if let Some(model) = self.configured_model {
                    result["model"] = json!(model);
                }
                self.messages
                    .push_back(AppServerMessage::Response { id, result });
            }
            "model/list" => self.messages.push_back(match &self.model_list {
                Ok(pages) => AppServerMessage::Response {
                    id,
                    result: pages[page - 1].clone(),
                },
                Err((code, message)) => AppServerMessage::ErrorResponse {
                    id,
                    code: *code,
                    message: message.to_string(),
                },
            }),
            "turn/start" => {
                let unsupported = turn_model
                    .as_deref()
                    .is_some_and(|model| self.unsupported_models.contains(&model));
                let turn_id = format!("turn-{}", self.turn_requests().len());
                if unsupported && self.turn_start_rpc_error {
                    self.messages.push_back(AppServerMessage::ErrorResponse {
                        id,
                        code: -32600,
                        message: self.failure_message.to_string(),
                    });
                    return Ok(());
                }
                self.messages.push_back(AppServerMessage::Response {
                    id,
                    result: json!({ "turn": { "id": turn_id, "items": [], "status": "inProgress" } }),
                });
                let turn = if unsupported {
                    json!({
                        "id": turn_id,
                        "status": "failed",
                        "items": [],
                        "error": {
                            "message": self.failure_message,
                            "codexErrorInfo": { "responseStreamConnectionFailed": { "httpStatusCode": 400 } },
                        },
                    })
                } else {
                    json!({
                        "id": turn_id,
                        "status": "completed",
                        "items": [{
                            "id": "message-1",
                            "type": "agentMessage",
                            "phase": "final_answer",
                            "text": proposal_text(),
                        }],
                    })
                };
                self.messages.push_back(AppServerMessage::Notification {
                    method: "turn/completed".to_string(),
                    params: json!({ "threadId": "thread-1", "turn": turn }),
                });
            }
            _ => self.messages.push_back(AppServerMessage::ErrorResponse {
                id,
                code: -32601,
                message: format!("unexpected fixture method {method}"),
            }),
        }
        Ok(())
    }

    fn recv_until(
        &mut self,
        _deadline: std::time::Instant,
    ) -> Result<AppServerMessage, CodexAppServerError> {
        self.messages
            .pop_front()
            .ok_or_else(|| CodexAppServerError::Transport("missing fake Codex response".into()))
    }

    fn terminate(&mut self) -> Result<AppServerCleanupReport, CodexAppServerError> {
        Ok(AppServerCleanupReport::default())
    }
}

fn run_turn(
    server: &mut ModelServer,
) -> Result<video_creater_lib::codex::conversation::CodexConversationTurnResult, CodexAppServerError>
{
    let mut project = conversation_fixture_project();
    start_codex_conversation_turn(
        server,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        None,
    )
}

const NOTICE: &str = "Your Codex config selects a model this app's Codex can't use; using gpt-5.5.";

#[test]
fn unsupported_configured_model_starts_the_turn_on_the_default_supported_model() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));

    let result = run_turn(&mut server).expect("turn on the supported model");

    assert_eq!(
        server.methods(),
        ["initialize", "thread/start", "model/list", "turn/start"]
    );
    assert_eq!(
        server.requests[2]["params"]["includeHidden"],
        json!(true),
        "a hidden model is still usable"
    );
    assert_eq!(
        server.turn_requests()[0]["params"]["model"],
        json!("gpt-5.5")
    );
    assert_eq!(result.model_notice.as_deref(), Some(NOTICE));
    assert!(result.proposal.is_some());
}

#[test]
fn supported_configured_model_is_left_alone_without_a_notice() {
    let mut server = ModelServer::new(Some("codex-auto-review"));

    let result = run_turn(&mut server).expect("turn");

    assert_eq!(
        server.methods(),
        ["initialize", "thread/start", "model/list", "turn/start"]
    );
    assert!(server.turn_requests()[0]["params"].get("model").is_none());
    assert_eq!(result.model_notice, None);
}

#[test]
fn model_list_pages_are_followed_before_deciding_support() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));
    server.unsupported_models = Vec::new();
    server.model_list = Ok(vec![
        json!({ "data": [model_entry("gpt-5.5", false, true)], "nextCursor": "page-2" }),
        json!({ "data": [model_entry("gpt-5.6-sol", false, false)], "nextCursor": null }),
    ]);

    let result = run_turn(&mut server).expect("turn");

    assert_eq!(
        server.methods(),
        [
            "initialize",
            "thread/start",
            "model/list",
            "model/list",
            "turn/start"
        ]
    );
    assert_eq!(server.requests[3]["params"]["cursor"], json!("page-2"));
    assert!(server.turn_requests()[0]["params"].get("model").is_none());
    assert_eq!(result.model_notice, None);
}

#[test]
fn unavailable_model_list_retries_an_unsupported_model_turn_once_with_the_pinned_default() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));
    server.model_list = Err((-32601, "method not found: model/list"));

    let result = run_turn(&mut server).expect("retried turn");

    assert_eq!(
        server.methods(),
        [
            "initialize",
            "thread/start",
            "model/list",
            "turn/start",
            "turn/start"
        ]
    );
    let turns = server.turn_requests();
    assert!(turns[0]["params"].get("model").is_none());
    assert_eq!(turns[1]["params"]["model"], json!("gpt-5.5"));
    assert_ne!(turns[0]["id"], turns[1]["id"], "the retry is a new request");
    assert_eq!(result.model_notice.as_deref(), Some(NOTICE));
    assert!(result.proposal.is_some());
}

#[test]
fn listed_model_rejected_by_the_server_retries_once_with_the_listed_default() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));
    server.model_list = Ok(vec![json!({
        "data": [model_entry("gpt-5.5", false, true), model_entry("gpt-5.6-sol", true, false)],
        "nextCursor": null,
    })]);

    let result = run_turn(&mut server).expect("retried turn");

    let turns = server.turn_requests();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1]["params"]["model"], json!("gpt-5.5"));
    assert_eq!(result.model_notice.as_deref(), Some(NOTICE));
}

#[test]
fn turn_start_rpc_error_for_an_unsupported_model_is_retried_once() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));
    server.model_list = Err((-32601, "method not found: model/list"));
    server.turn_start_rpc_error = true;

    let result = run_turn(&mut server).expect("retried turn");

    let turns = server.turn_requests();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1]["params"]["model"], json!("gpt-5.5"));
    assert_eq!(result.model_notice.as_deref(), Some(NOTICE));
}

#[test]
fn unsupported_model_error_is_not_retried_twice() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));
    server.unsupported_models = vec!["gpt-5.6-sol", "gpt-5.5"];

    let error = run_turn(&mut server).expect_err("override also rejected");

    assert_eq!(
        server.turn_requests().len(),
        1,
        "no retry after an override"
    );
    assert!(error
        .to_string()
        .contains("requires a newer version of Codex"));
}

#[test]
fn other_turn_failures_are_not_retried() {
    let mut server = ModelServer::new(Some("gpt-5.6-sol"));
    server.model_list = Err((-32601, "method not found: model/list"));
    server.failure_message = "You've hit your usage limit.";

    let error = run_turn(&mut server).expect_err("usage limit is not a model problem");

    assert_eq!(server.turn_requests().len(), 1);
    assert!(error.to_string().contains("usage limit"));
}

#[test]
fn thread_without_a_reported_model_skips_the_model_check() {
    let mut server = ModelServer::new(None);

    let result = run_turn(&mut server).expect("turn");

    assert_eq!(
        server.methods(),
        ["initialize", "thread/start", "turn/start"]
    );
    assert_eq!(result.model_notice, None);
}
