//! Video edit turns switch away from a configured model the bundled Codex can't use.

use super::{sample_edit_request, sample_project, sample_skill_bundle};
use serde_json::{json, Value};
use std::collections::VecDeque;
use video_creater_lib::codex::app_server::{
    start_codex_video_edit_turn, AppServerCleanupReport, AppServerMessage, CodexAppServerError,
    CodexAppServerTransport,
};

#[derive(Default)]
struct PinnedServer {
    requests: Vec<Value>,
    messages: VecDeque<AppServerMessage>,
}

impl CodexAppServerTransport for PinnedServer {
    fn send(&mut self, request: Value) -> Result<(), CodexAppServerError> {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default().to_string();
        let turn_model = request
            .pointer("/params/model")
            .and_then(Value::as_str)
            .unwrap_or("gpt-5.6-sol")
            .to_string();
        self.requests.push(request);
        let result = match method.as_str() {
            "initialize" => json!({ "userAgent": "codex-fixture" }),
            "thread/start" => json!({ "thread": { "id": "thread-1" }, "model": "gpt-5.6-sol" }),
            "model/list" => json!({
                "data": [{
                    "id": "gpt-5.5", "model": "gpt-5.5", "displayName": "gpt-5.5",
                    "description": "", "hidden": false, "isDefault": true,
                    "defaultReasoningEffort": "medium", "supportedReasoningEfforts": [],
                }],
                "nextCursor": null,
            }),
            "turn/start" => {
                assert_eq!(
                    turn_model, "gpt-5.5",
                    "turn must not use the unsupported model"
                );
                self.messages.push_back(AppServerMessage::Response {
                    id,
                    result: json!({ "turn": { "id": "turn-1", "items": [], "status": "inProgress" } }),
                });
                self.messages.push_back(AppServerMessage::Notification {
                    method: "turn/completed".into(),
                    params: json!({
                        "threadId": "thread-1",
                        "turn": { "id": "turn-1", "status": "completed", "items": [] },
                    }),
                });
                return Ok(());
            }
            other => panic!("unexpected method {other}"),
        };
        self.messages
            .push_back(AppServerMessage::Response { id, result });
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

#[test]
fn video_edit_turn_uses_the_default_supported_model_and_reports_a_notice() {
    let mut project = sample_project();
    let mut server = PinnedServer::default();

    let result = start_codex_video_edit_turn(
        &mut server,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_edit_request(),
        &sample_skill_bundle(),
        None,
    )
    .expect("turn on the supported model");

    let methods: Vec<_> = server
        .requests
        .iter()
        .map(|request| request["method"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        methods,
        ["initialize", "thread/start", "model/list", "turn/start"]
    );
    assert_eq!(
        result.model_notice.as_deref(),
        Some("Your Codex config selects a model this app's Codex can't use; using gpt-5.5.")
    );
}
