//! Stopping an in-flight conversation turn from the editor.

use super::support::{
    agent_history_json, conversation_fixture_project, folder_snapshot, sample_conversation_request,
    sample_skill_bundle, saved_split_project, FakeTransport,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use video_creater_lib::codex::app_server::{
    start_codex_conversation_turn_unpersisted_until, AppServerCleanupReport, AppServerMessage,
    CodexAppServerError, CodexAppServerTransport,
};
use video_creater_lib::codex::turn_cancel::{
    close_codex_turn_cancellation, register_codex_project_turn,
    request_codex_project_turn_cancellation,
};
use video_creater_lib::project::split::load_split_project;

/// An app-server whose turn stays in progress. The first time the editor
/// would be waiting on it, the user presses Stop through the cancel entry
/// point the command uses.
struct StoppedTurnTransport {
    root: PathBuf,
    project_dir: PathBuf,
    requests: Vec<Value>,
    messages: VecDeque<AppServerMessage>,
    cancel_outcome: Option<bool>,
    terminated: bool,
}

impl StoppedTurnTransport {
    fn new(root: &Path, project_dir: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            project_dir: project_dir.to_path_buf(),
            requests: Vec::new(),
            messages: VecDeque::new(),
            cancel_outcome: None,
            terminated: false,
        }
    }
}

impl CodexAppServerTransport for StoppedTurnTransport {
    fn send(&mut self, request: Value) -> Result<(), CodexAppServerError> {
        let id = request["id"].clone();
        let result = match request["method"].as_str().unwrap_or_default() {
            "initialize" => Some(json!({ "userAgent": "codex-fixture" })),
            "thread/start" | "thread/resume" => Some(json!({ "thread": { "id": "thread-1" } })),
            "turn/start" => {
                Some(json!({ "turn": { "id": "turn-1", "items": [], "status": "inProgress" } }))
            }
            _ => None,
        };
        self.requests.push(request);
        if let Some(result) = result {
            self.messages
                .push_back(AppServerMessage::Response { id, result });
        }
        Ok(())
    }

    fn recv_until(&mut self, _deadline: Instant) -> Result<AppServerMessage, CodexAppServerError> {
        if let Some(message) = self.messages.pop_front() {
            return Ok(message);
        }
        if self.cancel_outcome.is_none() {
            self.cancel_outcome = Some(
                request_codex_project_turn_cancellation(&self.root, Some(&self.project_dir))
                    .expect("cancel request"),
            );
        }
        Err(CodexAppServerError::Deadline)
    }

    fn terminate(&mut self) -> Result<AppServerCleanupReport, CodexAppServerError> {
        self.terminated = true;
        Ok(AppServerCleanupReport::default())
    }
}

#[test]
fn stopping_an_in_flight_conversation_turn_interrupts_it_and_leaves_the_project_untouched() {
    let (_dir, project_dir, mut project) = saved_split_project(conversation_fixture_project());
    let root = tempfile::tempdir().expect("codex root");
    let before = folder_snapshot(&project_dir);
    let (guard, token) =
        register_codex_project_turn(root.path(), Some(&project_dir)).expect("register turn");
    let mut transport = StoppedTurnTransport::new(root.path(), &project_dir);

    let error = start_codex_conversation_turn_unpersisted_until(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        Some(&project_dir),
        Instant::now() + Duration::from_secs(5),
        Some(&token),
    )
    .expect_err("a stopped turn does not return a proposal");

    assert_eq!(error, CodexAppServerError::Interrupted);
    assert_eq!(error.to_string(), "app-server turn was interrupted");
    assert_eq!(transport.cancel_outcome, Some(true));
    assert!(transport.terminated);
    assert!(transport.requests.iter().any(|request| {
        request["method"] == "turn/interrupt" && request["params"]["turnId"] == "turn-1"
    }));
    // The command closes the window before persisting; a stopped turn is refused.
    assert_eq!(
        close_codex_turn_cancellation(guard, &token),
        Err(CodexAppServerError::Interrupted)
    );
    assert_eq!(folder_snapshot(&project_dir), before);
    assert_eq!(agent_history_json(&project_dir), json!({ "entries": [] }));
    assert_eq!(
        load_split_project(&project_dir)
            .expect("reload")
            .codex_thread_id,
        None
    );
}

#[test]
fn stopping_when_no_turn_is_in_flight_is_a_no_op() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    let root = tempfile::tempdir().expect("codex root");
    let before = folder_snapshot(&project_dir);

    assert_eq!(
        request_codex_project_turn_cancellation(root.path(), Some(&project_dir)),
        Ok(false)
    );
    assert_eq!(
        request_codex_project_turn_cancellation(root.path(), None),
        Ok(false)
    );

    // A finished turn is unregistered, so a late Stop is a no-op too.
    let (guard, token) =
        register_codex_project_turn(root.path(), Some(&project_dir)).expect("register turn");
    assert_eq!(close_codex_turn_cancellation(guard, &token), Ok(()));
    assert_eq!(
        request_codex_project_turn_cancellation(root.path(), Some(&project_dir)),
        Ok(false)
    );
    assert_eq!(folder_snapshot(&project_dir), before);
}

#[test]
fn a_stop_that_lands_after_codex_answers_still_withholds_the_proposal() {
    let (_dir, project_dir, mut project) = saved_split_project(conversation_fixture_project());
    let root = tempfile::tempdir().expect("codex root");
    let (guard, token) =
        register_codex_project_turn(root.path(), Some(&project_dir)).expect("register turn");
    let mut transport = FakeTransport::new(json!({
        "summary": "Lowered the interview clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }],
        "renderReview": null,
    }));

    let result = start_codex_conversation_turn_unpersisted_until(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        Some(&project_dir),
        Instant::now() + Duration::from_secs(5),
        Some(&token),
    )
    .expect("turn completes before the stop");
    assert!(result.prepared_proposal.is_some());

    assert_eq!(
        request_codex_project_turn_cancellation(root.path(), Some(&project_dir)),
        Ok(true)
    );
    assert_eq!(
        close_codex_turn_cancellation(guard, &token),
        Err(CodexAppServerError::Interrupted)
    );
}

#[test]
fn a_project_allows_one_codex_turn_at_a_time_and_scopes_stop_to_that_project() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    let (_other_dir, other_project_dir, _other) =
        saved_split_project(conversation_fixture_project());
    let root = tempfile::tempdir().expect("codex root");
    let (guard, token) =
        register_codex_project_turn(root.path(), Some(&project_dir)).expect("register turn");

    assert!(matches!(
        register_codex_project_turn(root.path(), Some(&project_dir)),
        Err(CodexAppServerError::Transport(_))
    ));
    assert_eq!(
        request_codex_project_turn_cancellation(root.path(), Some(&other_project_dir)),
        Ok(false)
    );
    assert_eq!(close_codex_turn_cancellation(guard, &token), Ok(()));
}
