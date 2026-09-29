//! Undo of a later agent batch while an earlier batch's generation keeps running: that
//! generation's progress (status, outputs, provider inputs and the unplaced output media
//! completion adds) is bookkeeping for it, so it doesn't block Undo and Undo keeps it (rule
//! version 4). Placing its output on the timeline is still an edit and still conflicts.

use super::support::{agent_history_json, conversation_fixture_project, saved_split_project};
use serde_json::{json, Value};
use std::path::Path;
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal, prepare_codex_conversation_proposal,
    CodexConversationApplyRequest, CodexConversationEditProposal,
};
use video_creater_lib::codex::proposal::CodexRenderReview;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{GeneratedAssetStatus, TrackKind, VideoProject};
use video_creater_lib::project::split::{
    agent_project_content_hash, apply_agent_project_action_batch,
    apply_project_actions_to_split_project, load_split_project, undo_latest_agent_project_batch,
    ProjectAgentUndoOutcome,
};

const ASSET: &str = "background-shot-1";
const JOB: &str = "job-background-shot-1";
const PLACEHOLDER: &str = "generated-placeholder-background-shot-1";
const OUTPUT_MEDIA: &str = "background-shot-1-output";
const PLACEHOLDER_START: f64 = 10.0;
const PROJECT_CHANGED: &str =
    "The project changed after this edit was applied, so undoing it would discard later changes.";

fn action(value: Value) -> ProjectAction {
    serde_json::from_value(value).expect("project action")
}

fn video_track_id(project: &VideoProject) -> String {
    project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .expect("video track")
        .id
        .clone()
}

/// What an approved agent bundle records for one timeline generation.
fn generation_bundle(project: &VideoProject, project_dir: &Path) -> Vec<ProjectAction> {
    vec![
        action(json!({
            "type": "recordGeneratedAsset",
            "asset": {
                "id": ASSET,
                "kind": "generated",
                "status": "queued",
                "name": "Lab bench wide shot",
                "placementIntent": "timeline",
                "prompt": "A lab bench in a bright laboratory",
                "model": { "provider": "fal.ai", "id": "fal-ai/veo3/fast" },
                "references": { "mediaIds": [], "firstFrameMediaId": null, "lastFrameMediaId": null },
                "settings": { "width": 1280, "height": 720, "durationSeconds": 4.0, "fps": 24.0, "timelineStartSeconds": PLACEHOLDER_START },
                "outputs": [],
                "createdAt": "2026-09-16T10:00:00Z",
                "parentAssetId": null,
                "retryOfAssetId": null,
            },
        })),
        action(json!({
            "type": "recordJob",
            "job": {
                "id": JOB,
                "kind": "generate_media",
                "status": "queued",
                "updatedAt": "2026-09-16T10:00:00Z",
                "startRequest": {
                    "workflowId": "generate-media-background-shot-1",
                    "workflowType": "generateMedia",
                    "taskQueue": "video-creater",
                    "input": { "projectId": project.id, "projectDir": project_dir.display().to_string(), "assetId": ASSET, "jobId": JOB, "mockMode": false },
                    "searchAttributes": {},
                    "activityTypes": ["runProvider"],
                    "idReusePolicy": "rejectDuplicate",
                },
            },
        })),
        action(json!({
            "type": "addItems",
            "targetTrackId": video_track_id(project),
            "items": [{
                "id": PLACEHOLDER,
                "kind": "generated_clip",
                "startSeconds": PLACEHOLDER_START,
                "durationSeconds": 4.0,
                "source": { "type": "generated", "artifactId": ASSET },
                "label": "Lab bench wide shot",
                "properties": { "generatedAssetId": ASSET, "generatedTimelinePlaceholder": true, "sourceIn": 0.0, "sourceOut": 4.0 },
            }],
        })),
    ]
}

/// Batch 1: the approved generation bundle.
fn record_generation(project_dir: &Path) {
    let project = load_split_project(project_dir).expect("load project");
    let proposal = CodexConversationEditProposal {
        summary: "Generates a lab shot and places it at the end.".to_string(),
        edl: Vec::new(),
        project_actions: generation_bundle(&project, project_dir),
        render_review: Some(CodexRenderReview {
            duration_seconds: PLACEHOLDER_START + 4.0,
            stream_check_required: true,
            caption_alignment_required: false,
            overlay_timing_required: false,
            visual_frame_evidence_required: true,
            artifact_paths_required: false,
            log_reference_required: false,
        }),
    };
    let prepared =
        prepare_codex_conversation_proposal(&project, &proposal).expect("proposal prepares");
    apply_codex_conversation_proposal(
        project_dir,
        CodexConversationApplyRequest {
            proposal,
            action_ids: prepared.action_ids,
            review_approved: true,
            session_id: None,
        },
    )
    .expect("apply generation bundle");
}

/// Batch 2: returns its history entry id.
fn apply_batch(project_dir: &Path, actions: Vec<Value>) -> String {
    let ids = (0..actions.len())
        .map(|index| format!("batch-2-action-{index}"))
        .collect();
    apply_agent_project_action_batch(
        project_dir,
        actions.into_iter().map(action).collect(),
        ids,
        None,
    )
    .expect("apply batch 2")
    .history_entry_id
}

fn volume_change() -> Vec<Value> {
    vec![json!({ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 })]
}

/// A write that isn't an agent batch (the generation runner or the editor).
fn write(project_dir: &Path, actions: Vec<Value>) {
    apply_project_actions_to_split_project(project_dir, actions.into_iter().map(action).collect())
        .expect("background write");
}

fn run_generation(project_dir: &Path) {
    write(
        project_dir,
        vec![
            json!({ "type": "updateJobStatus", "jobId": JOB, "status": "running", "updatedAt": "2026-09-16T10:00:01Z" }),
            json!({ "type": "updateGeneratedAssetStatus", "assetId": ASSET, "status": "running" }),
            json!({
                "type": "updateGeneratedAssetReferences",
                "assetId": ASSET,
                "references": { "mediaIds": [], "firstFrameMediaId": null, "lastFrameMediaId": null, "providerInputUrls": ["https://fal.media/files/input.png"] },
            }),
        ],
    );
}

fn complete_generation(project_dir: &Path, replace_placeholder: bool) {
    let mut complete = json!({
        "type": "completeGeneratedAsset",
        "assetId": ASSET,
        "outputs": [{
            "mediaId": OUTPUT_MEDIA,
            "relativePath": "generated/background-shot-1/output.mp4",
            "sourceUrl": "https://fal.media/files/output.mp4",
            "width": 1280,
            "height": 720,
            "durationSeconds": 4.0,
            "fps": 24.0,
        }],
    });
    if replace_placeholder {
        complete["replacement"] = json!({ "itemId": PLACEHOLDER, "mediaId": OUTPUT_MEDIA });
    }
    write(
        project_dir,
        vec![
            complete,
            json!({ "type": "updateJobStatus", "jobId": JOB, "status": "completed", "updatedAt": "2026-09-16T10:00:09Z" }),
        ],
    );
}

fn audio_volume(project: &VideoProject) -> Option<Value> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.items)
        .find(|item| item.id == "audio-1")
        .and_then(|item| item.properties.get("volumeDb").cloned())
}

fn undo(project_dir: &Path, entry_id: &str) -> ProjectAgentUndoOutcome {
    undo_latest_agent_project_batch(project_dir, Some(entry_id)).expect("undo")
}

#[test]
fn an_earlier_generations_progress_doesnt_block_a_later_batch() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    record_generation(&project_dir);
    run_generation(&project_dir);
    let before_batch = load_split_project(&project_dir).expect("before batch 2");
    let entry_id = apply_batch(&project_dir, volume_change());
    complete_generation(&project_dir, false);

    let outcome = undo(&project_dir, &entry_id);

    assert!(
        matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }),
        "{outcome:?}"
    );
    let restored = load_split_project(&project_dir).expect("restored project");
    assert_eq!(audio_volume(&restored), audio_volume(&before_batch));
    let asset = restored
        .generated_assets
        .iter()
        .find(|asset| asset.id == ASSET)
        .expect("earlier generation");
    assert_eq!(asset.status, GeneratedAssetStatus::Completed);
    assert_eq!(asset.outputs.len(), 1);
    assert_eq!(asset.outputs[0].media_id, OUTPUT_MEDIA);
    assert!(restored.media.iter().any(|media| media.id == OUTPUT_MEDIA));
    assert_eq!(restored.timeline, before_batch.timeline);
}

#[test]
fn placing_an_earlier_generations_output_still_conflicts() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    record_generation(&project_dir);
    run_generation(&project_dir);
    let entry_id = apply_batch(&project_dir, volume_change());
    complete_generation(&project_dir, true);

    match undo(&project_dir, &entry_id) {
        ProjectAgentUndoOutcome::Conflict { message, .. } => assert_eq!(message, PROJECT_CHANGED),
        outcome => panic!("expected conflict, got {outcome:?}"),
    }
}

#[test]
fn a_batch_that_changes_another_generations_status_restores_it() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    record_generation(&project_dir);
    let entry_id = apply_batch(
        &project_dir,
        vec![
            json!({ "type": "updateGeneratedAssetStatus", "assetId": ASSET, "status": "cancelled" }),
        ],
    );

    let entries = agent_history_json(&project_dir)["entries"].clone();
    let entry = entries
        .as_array()
        .expect("entries")
        .last()
        .expect("batch 2");
    assert_eq!(entry["afterContentHashVersion"], json!(4));
    assert!(entry
        .get("backgroundGeneratedAssetIds")
        .and_then(Value::as_array)
        .is_none_or(|ids| !ids.contains(&json!(ASSET))));

    let outcome = undo(&project_dir, &entry_id);

    assert!(
        matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }),
        "{outcome:?}"
    );
    let restored = load_split_project(&project_dir).expect("restored project");
    let asset = restored
        .generated_assets
        .iter()
        .find(|asset| asset.id == ASSET)
        .expect("generation");
    assert_eq!(asset.status, GeneratedAssetStatus::Queued);
}

#[test]
fn version_3_entries_keep_their_rule() {
    let (_dir, project_dir, _project) = saved_split_project(conversation_fixture_project());
    record_generation(&project_dir);
    run_generation(&project_dir);
    let entry_id = apply_batch(&project_dir, volume_change());
    let after = load_split_project(&project_dir).expect("after batch 2");

    let path = project_dir.join("context").join("agent-history.json");
    let mut history = agent_history_json(&project_dir);
    let entry = history["entries"]
        .as_array_mut()
        .expect("entries")
        .last_mut()
        .expect("batch 2");
    assert_eq!(entry["backgroundGeneratedAssetIds"], json!([ASSET]));
    let added = serde_json::from_value::<Vec<String>>(
        entry
            .get("addedGeneratedAssetIds")
            .cloned()
            .unwrap_or(json!([])),
    )
    .expect("added ids");
    let object = entry.as_object_mut().expect("entry object");
    object.remove("backgroundGeneratedAssetIds");
    object.insert("afterContentHashVersion".to_string(), json!(3));
    object.insert(
        "afterContentHash".to_string(),
        json!(agent_project_content_hash(&after, &added).expect("version 3 hash")),
    );
    std::fs::write(&path, serde_json::to_vec_pretty(&history).expect("json")).expect("write");
    complete_generation(&project_dir, false);

    match undo(&project_dir, &entry_id) {
        ProjectAgentUndoOutcome::Conflict { message, .. } => assert_eq!(message, PROJECT_CHANGED),
        outcome => panic!("expected conflict, got {outcome:?}"),
    }
}
