use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use video_creater_lib::codex::app_server::{
    AppServerCleanupReport, AppServerMessage, CodexAppServerError, CodexAppServerTransport,
};
use video_creater_lib::codex::context::ProjectSkillBundle;
use video_creater_lib::codex::conversation::{
    CodexConversationEditProposal, CodexConversationEditRequest, CodexConversationFocus,
};
use video_creater_lib::codex::proposal::{CodexProposalClip, CodexRenderReview};
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, TimelineItem, TimelineItemKind, TimelineSource, TrackKind, VideoProject,
};
use video_creater_lib::project::split::{
    load_split_project, record_app_server_conversation_turn, save_split_project,
    AppServerConversationTurn,
};

pub fn sample_conversation_request() -> CodexConversationEditRequest {
    CodexConversationEditRequest {
        prompt: "Remove dead air from the interview.".to_string(),
        focus: CodexConversationFocus {
            primary_media_id: None,
            media_ids: Vec::new(),
            timeline_item_ids: vec!["audio-1".to_string()],
            timeline_range: None,
        },
        created_at: "2026-07-25T00:00:00Z".to_string(),
    }
}

pub fn empty_timeline_project() -> VideoProject {
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
    project
}

pub fn conversation_fixture_project() -> VideoProject {
    let mut project = empty_timeline_project();
    project.timeline.duration_seconds = 10.0;
    for track in &mut project.timeline.tracks {
        match track.kind {
            TrackKind::Video => track.items.push(media_item(
                "video-1",
                TimelineItemKind::VideoClip,
                0.0,
                10.0,
            )),
            TrackKind::Audio => track.items.push(media_item(
                "audio-1",
                TimelineItemKind::AudioClip,
                0.0,
                10.0,
            )),
            _ => {}
        }
    }
    project.timelines[0].timeline = project.timeline.clone();
    project
}

pub fn media_item(id: &str, kind: TimelineItemKind, start: f64, duration: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Interview".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(start)),
            ("sourceOut".to_string(), json!(start + duration)),
        ]),
    }
}

pub fn clip(media_id: &str, source_in: f64, source_out: f64) -> CodexProposalClip {
    CodexProposalClip {
        media_id: media_id.to_string(),
        source_in,
        source_out,
        reason: "Keeps the strongest answer.".to_string(),
    }
}

pub fn edl_proposal(
    edl: Vec<CodexProposalClip>,
    duration_seconds: f64,
) -> CodexConversationEditProposal {
    CodexConversationEditProposal {
        summary: "Built a tighter cut.".to_string(),
        edl,
        project_actions: Vec::new(),
        render_review: Some(CodexRenderReview {
            duration_seconds,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        }),
    }
}

pub fn sample_skill_bundle() -> ProjectSkillBundle {
    ProjectSkillBundle {
        agents_md: "## Video Creater Skill Policy".to_string(),
        video_pipeline: "# Video Creater Video Pipeline".to_string(),
        graphics: "# Video Creater Graphics".to_string(),
        visuals: "# Video Creater Visuals".to_string(),
        shader_background_catalog: "- shadertoy-octagrams-v1".to_string(),
    }
}

pub struct FakeTransport {
    proposal: Value,
    pub requests: Vec<Value>,
    messages: VecDeque<AppServerMessage>,
    echo_user_input: Option<String>,
}

impl FakeTransport {
    pub fn new(proposal: Value) -> Self {
        Self {
            proposal,
            requests: Vec::new(),
            messages: VecDeque::new(),
            echo_user_input: None,
        }
    }

    /// Mirrors a real app-server that echoes user input items back: the
    /// resumed thread carries `prior_turn_text` and the completed turn carries
    /// the text sent with `turn/start`.
    pub fn echoing_user_input(proposal: Value, prior_turn_text: &str) -> Self {
        Self {
            echo_user_input: Some(prior_turn_text.to_string()),
            ..Self::new(proposal)
        }
    }
}

fn user_message_item(id: &str, text: &str) -> Value {
    json!({
        "id": id,
        "type": "userMessage",
        "content": [{ "type": "text", "text": text }],
    })
}

impl CodexAppServerTransport for FakeTransport {
    fn send(&mut self, request: Value) -> Result<(), CodexAppServerError> {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default().to_string();
        let input_text = request
            .pointer("/params/input/0/text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        // A resumed thread keeps its id; each started thread gets the next `thread-N`.
        let thread_id = match method.as_str() {
            "thread/resume" | "turn/start" => request["params"]["threadId"]
                .as_str()
                .unwrap_or("thread-1")
                .to_string(),
            _ => {
                let started = self
                    .requests
                    .iter()
                    .filter(|sent| sent["method"] == "thread/start")
                    .count();
                format!("thread-{}", started + 1)
            }
        };
        self.requests.push(request);
        let result = match method.as_str() {
            "initialize" => json!({ "userAgent": "codex-fixture" }),
            "thread/start" | "thread/resume" => match &self.echo_user_input {
                Some(prior_text) => json!({
                    "thread": {
                        "id": thread_id,
                        "preview": prior_text,
                        "turns": [{
                            "id": "turn-0",
                            "status": "completed",
                            "items": [
                                user_message_item("user-0", prior_text),
                                { "id": "message-0", "type": "agentMessage", "text": "Done." },
                            ],
                        }],
                    }
                }),
                None => json!({ "thread": { "id": thread_id } }),
            },
            "turn/start" => {
                let mut items = Vec::new();
                if self.echo_user_input.is_some() {
                    items.push(user_message_item("user-1", &input_text));
                }
                items.push(json!({
                    "id": "message-1",
                    "type": "agentMessage",
                    "phase": "final_answer",
                    "text": serde_json::to_string(&self.proposal).expect("proposal text"),
                }));
                self.messages.push_back(AppServerMessage::Response {
                    id,
                    result: json!({ "turn": { "id": "turn-1", "items": [], "status": "inProgress" } }),
                });
                self.messages.push_back(AppServerMessage::Notification {
                    method: "turn/completed".to_string(),
                    params: json!({
                        "threadId": thread_id,
                        "turn": {
                            "id": "turn-1",
                            "status": "completed",
                            "items": items,
                        },
                    }),
                });
                return Ok(());
            }
            _ => json!({}),
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

/// Saves `project` as a split project in a fresh temporary folder and returns
/// the guard, the project folder, and the canonical project as loaded back.
pub fn saved_split_project(project: VideoProject) -> (tempfile::TempDir, PathBuf, VideoProject) {
    let dir = tempfile::tempdir().expect("project parent");
    let project_dir = dir.path().join("project");
    let mut project = project;
    project.schema_version = 2;
    save_split_project(&project_dir, &project).expect("save project");
    let loaded = load_split_project(&project_dir).expect("load saved project");
    (dir, project_dir, loaded)
}

/// Every file under the project folder with its exact bytes.
pub fn folder_snapshot(project_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read project folder") {
            let path = entry.expect("project folder entry").path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).expect("relative path");
                files.insert(
                    relative.display().to_string(),
                    std::fs::read(&path).expect("read project file"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(project_dir, project_dir, &mut files);
    files
}

pub fn agent_history_json(project_dir: &Path) -> Value {
    let path = project_dir.join("context").join("agent-history.json");
    if !path.exists() {
        return json!({ "entries": [] });
    }
    serde_json::from_slice(&std::fs::read(path).expect("read agent history"))
        .expect("agent history json")
}

/// Records one conversation turn so the project has an active agent session.
pub fn record_conversation_turn(project_dir: &Path, project: &VideoProject) {
    record_app_server_conversation_turn(
        project_dir,
        &project.id,
        "thread-1",
        AppServerConversationTurn {
            turn_id: Some("turn-1".to_string()),
            turn_status: Some("completed".to_string()),
            prompt: "Remove dead air from the interview.".to_string(),
            created_at: "2026-07-25T00:00:00Z".to_string(),
            request: json!({ "prompt": "Remove dead air from the interview." }),
            thread_response: json!({}),
            turn_response: json!({}),
            has_proposal: true,
            provider: None,
            provider_session_id: None,
        },
    )
    .expect("record conversation turn");
}
