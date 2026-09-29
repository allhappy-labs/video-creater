//! MCP agent edits: `video_creater.apply_project_actions` records hashed batch entries, and
//! `video_creater.undo_agent_edit` cancels the removed generations like editor Undo while
//! keeping its legacy refusal errors.

use serde_json::{json, Value};
use std::path::Path;
use video_creater_lib::codex::tools::{call_codex_local_tool, undo_agent_edit_payload_with};
use video_creater_lib::generation::cancel::register_generation_cancellation;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{JobProviderRequest, VideoProject};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project, save_split_project,
};

const ASSET: &str = "mcp-shot-1";
const JOB: &str = "job-mcp-shot-1";

fn saved_project() -> (tempfile::TempDir, VideoProject) {
    let dir = tempfile::tempdir().expect("project dir");
    save_split_project(dir.path(), &sample_project()).expect("save split project");
    let project = load_split_project(dir.path()).expect("load project");
    (dir, project)
}

fn project_dir_arg(dir: &Path) -> Value {
    json!(dir.display().to_string())
}

/// A queued fal.ai generation and its `generate_media` job, as an agent records them.
fn generation_actions(project: &VideoProject, project_dir: &Path) -> Value {
    json!([
        {
            "type": "recordGeneratedAsset",
            "asset": {
                "id": ASSET,
                "kind": "generated",
                "status": "queued",
                "name": "Lab bench wide shot",
                "placementIntent": "library",
                "prompt": "A lab bench in a bright laboratory",
                "model": { "provider": "fal.ai", "id": "fal-ai/veo3/fast" },
                "references": { "mediaIds": [], "firstFrameMediaId": null, "lastFrameMediaId": null },
                "settings": { "width": 1280, "height": 720, "durationSeconds": 4.0, "fps": 24.0 },
                "outputs": [],
                "createdAt": "2026-09-16T10:00:00Z",
                "parentAssetId": null,
                "retryOfAssetId": null,
            },
        },
        {
            "type": "recordJob",
            "job": {
                "id": JOB,
                "kind": "generate_media",
                "status": "queued",
                "updatedAt": "2026-09-16T10:00:00Z",
                "startRequest": {
                    "workflowId": "generate-media-mcp-shot-1",
                    "workflowType": "generateMedia",
                    "taskQueue": "video-creater",
                    "input": { "projectId": project.id, "projectDir": project_dir.display().to_string(), "assetId": ASSET, "jobId": JOB, "mockMode": false },
                    "searchAttributes": {},
                    "activityTypes": ["runProvider"],
                    "idReusePolicy": "rejectDuplicate",
                },
            },
        },
    ])
}

fn mcp_apply_generation(dir: &Path, project: &VideoProject) {
    call_codex_local_tool(
        project,
        "video_creater.apply_project_actions",
        json!({ "projectDir": project_dir_arg(dir), "actions": generation_actions(project, dir) }),
    )
    .expect("apply generation actions");
}

fn agent_history(dir: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(dir.join("context").join("agent-history.json")).expect("agent history"),
    )
    .expect("agent history json")
}

/// What the runner writes while the generation works (not an agent write).
fn run_generation(dir: &Path, provider_request: Option<JobProviderRequest>) {
    let mut actions: Vec<Value> = vec![
        json!({ "type": "updateJobStatus", "jobId": JOB, "status": "running", "updatedAt": "2026-09-16T10:00:01Z" }),
        json!({ "type": "updateGeneratedAssetStatus", "assetId": ASSET, "status": "running" }),
    ];
    if let Some(request) = provider_request {
        actions.push(
            json!({ "type": "updateJobProviderRequest", "jobId": JOB, "providerRequest": request }),
        );
    }
    let actions = actions
        .into_iter()
        .map(|value| serde_json::from_value::<ProjectAction>(value).expect("project action"))
        .collect();
    apply_project_actions_to_split_project(dir, actions).expect("runner write");
}

#[test]
fn mcp_apply_records_a_hashed_batch_entry() {
    let (dir, project) = saved_project();

    mcp_apply_generation(dir.path(), &project);

    let entry = &agent_history(dir.path())["entries"][0];
    assert!(entry["afterContentHash"].is_string(), "{entry}");
    assert_eq!(entry["afterContentHashVersion"], json!(4));
    assert_eq!(entry["addedGeneratedAssetIds"], json!([ASSET]));
    assert_eq!(entry["addedBookkeepingIds"]["jobs"], json!([JOB]));
    assert!(entry.get("after").is_none(), "{entry}");
}

#[test]
fn mcp_undo_cancels_the_in_process_run_of_a_removed_generation() {
    let (dir, project) = saved_project();
    mcp_apply_generation(dir.path(), &project);
    run_generation(dir.path(), None);
    let run = register_generation_cancellation(&project.id, JOB).expect("register run");
    let current = load_split_project(dir.path()).expect("load running project");

    let undo = call_codex_local_tool(
        &current,
        "video_creater.undo_agent_edit",
        json!({ "projectDir": project_dir_arg(dir.path()) }),
    )
    .expect("undo agent edit");

    assert_eq!(undo.payload["undone"], json!(true));
    assert_eq!(undo.payload["removedGeneratedAssetIds"], json!([ASSET]));
    assert_eq!(undo.payload["warnings"], json!([]));
    assert!(run.token().is_cancelled());
    let restored = load_split_project(dir.path()).expect("load restored project");
    assert!(restored
        .generated_assets
        .iter()
        .all(|asset| asset.id != ASSET));
    assert!(restored.jobs.iter().all(|job| job.id != JOB));
}

#[test]
fn mcp_undo_keeps_its_legacy_refusal_errors() {
    let (dir, project) = saved_project();
    let empty = call_codex_local_tool(
        &project,
        "video_creater.undo_agent_edit",
        json!({ "projectDir": project_dir_arg(dir.path()) }),
    )
    .expect_err("nothing to undo");
    assert!(
        empty.to_string().contains("no agent edit history"),
        "{empty}"
    );

    mcp_apply_generation(dir.path(), &project);
    let mut changed = load_split_project(dir.path()).expect("load applied project");
    changed.timeline.tracks[0].items[0].label = "Manual edit".to_string();
    save_split_project(dir.path(), &changed).expect("save manual edit");
    let conflict = call_codex_local_tool(
        &changed,
        "video_creater.undo_agent_edit",
        json!({ "projectDir": project_dir_arg(dir.path()) }),
    )
    .expect_err("changed project");
    assert!(
        conflict
            .to_string()
            .contains("project changed after that batch"),
        "{conflict}"
    );
}

#[test]
fn mcp_undo_reports_a_provider_cancel_failure_as_a_warning() {
    let (dir, project) = saved_project();
    mcp_apply_generation(dir.path(), &project);
    run_generation(
        dir.path(),
        Some(JobProviderRequest {
            provider: "fal.ai".to_string(),
            request_id: "request-1".to_string(),
            status_url: "https://queue.fal.run/fal-ai/veo3/requests/request-1/status".to_string(),
            response_url: "https://queue.fal.run/fal-ai/veo3/requests/request-1".to_string(),
            cancel_url: "https://queue.fal.run/fal-ai/veo3/requests/request-1/cancel".to_string(),
            submitted_at: "2026-09-16T10:00:02Z".to_string(),
        }),
    );

    let payload =
        undo_agent_edit_payload_with(dir.path(), &mut |_| Err("provider is offline".to_string()))
            .expect("undo agent edit");

    assert_eq!(payload["undone"], json!(true));
    let warnings = payload["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0]
        .as_str()
        .is_some_and(|warning| warning.contains("provider is offline")));
}
