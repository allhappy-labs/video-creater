//! Undo of an approved "Generate & place" batch while its generations run in
//! the background: status, outputs, provider requests and the output that
//! completion adds (placed over the batch's placeholder) never block Undo, and
//! Undo removes them. Real edits still conflict.

use super::support::{agent_history_json, conversation_fixture_project, saved_split_project};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal, prepare_codex_conversation_proposal,
    undo_codex_conversation_edit_with, CodexConversationApplyRequest, CodexConversationApplyResult,
    CodexConversationEditProposal,
};
use video_creater_lib::codex::proposal::CodexRenderReview;
use video_creater_lib::generation::cancel::register_generation_cancellation;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{
    JobProviderRequest, TimelineItem, TimelineSource, TrackKind, VideoProject,
};
use video_creater_lib::project::split::{
    apply_project_actions_to_split_project, load_split_project, save_split_project,
    undo_latest_agent_project_batch, ProjectAgentUndoOutcome,
};

const ASSET: &str = "agent-shot-1";
const JOB: &str = "job-agent-shot-1";
const PLACEHOLDER: &str = "generated-placeholder-agent-shot-1";
const OUTPUT_MEDIA: &str = "agent-shot-1-output";
const OUTPUT_ITEM: &str = "generated-output-agent-shot-1-output-k1";
const PLACEHOLDER_START: f64 = 10.0;

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

/// What an approved agent bundle records for one timeline generation (Rust
/// `generation_record_actions` plus the editor's placeholder clip).
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
                "createdAt": "2026-09-15T10:00:00Z",
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
                "updatedAt": "2026-09-15T10:00:00Z",
                "startRequest": {
                    "workflowId": "generate-media-agent-shot-1",
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

fn apply_approved(project_dir: &Path, actions: Vec<ProjectAction>) -> CodexConversationApplyResult {
    let project = load_split_project(project_dir).expect("load project");
    let proposal = CodexConversationEditProposal {
        summary: "Generates a lab shot and places it at the end.".to_string(),
        edl: Vec::new(),
        project_actions: actions,
        // A new visual layer (the placeholder) needs a render review.
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
    .expect("apply")
}

/// A saved project with the generation bundle applied; returns the project
/// before the apply.
fn applied_generation() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    VideoProject,
    CodexConversationApplyResult,
) {
    let (dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let applied = apply_approved(&project_dir, generation_bundle(&before, &project_dir));
    (dir, project_dir, before, applied)
}

fn write(project_dir: &Path, actions: Vec<Value>) {
    apply_project_actions_to_split_project(project_dir, actions.into_iter().map(action).collect())
        .expect("background write");
}

fn provider_request() -> JobProviderRequest {
    JobProviderRequest {
        provider: "fal.ai".to_string(),
        request_id: "request-1".to_string(),
        status_url: "https://queue.fal.run/fal-ai/veo3/requests/request-1/status".to_string(),
        response_url: "https://queue.fal.run/fal-ai/veo3/requests/request-1".to_string(),
        cancel_url: "https://queue.fal.run/fal-ai/veo3/requests/request-1/cancel".to_string(),
        submitted_at: "2026-09-15T10:00:02Z".to_string(),
    }
}

/// What the runner writes while it works: running status, the provider
/// request, and uploaded provider inputs.
fn run_generation(project_dir: &Path) {
    write(
        project_dir,
        vec![
            json!({ "type": "updateJobStatus", "jobId": JOB, "status": "running", "updatedAt": "2026-09-15T10:00:01Z" }),
            json!({ "type": "updateGeneratedAssetStatus", "assetId": ASSET, "status": "running" }),
        ],
    );
    write(
        project_dir,
        vec![
            json!({ "type": "updateJobProviderRequest", "jobId": JOB, "providerRequest": provider_request() }),
            json!({
                "type": "updateGeneratedAssetReferences",
                "assetId": ASSET,
                "references": { "mediaIds": [], "firstFrameMediaId": null, "lastFrameMediaId": null, "providerInputUrls": ["https://fal.media/files/input.png"] },
            }),
        ],
    );
}

fn output() -> Value {
    json!({
        "mediaId": OUTPUT_MEDIA,
        "relativePath": "generated/agent-shot-1/output.mp4",
        "sourceUrl": "https://fal.media/files/output.mp4",
        "width": 1280,
        "height": 720,
        "durationSeconds": 4.0,
        "fps": 24.0,
    })
}

/// Completion: the asset's outputs and their library media, optionally
/// replacing the placeholder in place.
fn complete_generation(project_dir: &Path, replace_placeholder: bool) {
    let mut complete =
        json!({ "type": "completeGeneratedAsset", "assetId": ASSET, "outputs": [output()] });
    if replace_placeholder {
        complete["replacement"] = json!({ "itemId": PLACEHOLDER, "mediaId": OUTPUT_MEDIA });
    }
    write(
        project_dir,
        vec![
            complete,
            json!({ "type": "updateJobStatus", "jobId": JOB, "status": "completed", "updatedAt": "2026-09-15T10:00:09Z" }),
        ],
    );
}

/// The editor's placement once the completed asset merges
/// (`generatedOutputTimelineActions`): the placeholder is swapped for an
/// output clip at the same spot.
fn place_output_over_placeholder(project_dir: &Path) {
    let project = load_split_project(project_dir).expect("load");
    write(
        project_dir,
        vec![
            json!({ "type": "removeItems", "itemIds": [PLACEHOLDER] }),
            json!({
                "type": "addItems",
                "targetTrackId": video_track_id(&project),
                "items": [{
                    "id": OUTPUT_ITEM,
                    "kind": "video_clip",
                    "startSeconds": PLACEHOLDER_START,
                    "durationSeconds": 4.0,
                    "source": { "type": "media", "mediaId": OUTPUT_MEDIA },
                    "label": "Lab bench wide shot",
                    "properties": { "generatedAssetId": ASSET, "generatedOutputMediaId": OUTPUT_MEDIA, "sourceIn": 0.0, "sourceOut": 4.0 },
                }],
            }),
        ],
    );
}

/// A user edit saved through the editor's snapshot path.
fn user_edit(project_dir: &Path, edit: impl FnOnce(&mut VideoProject)) {
    let mut project = load_split_project(project_dir).expect("load");
    edit(&mut project);
    project.timelines[0].timeline = project.timeline.clone();
    save_split_project(project_dir, &project).expect("user edit");
}

fn item_mut<'a>(project: &'a mut VideoProject, id: &str) -> &'a mut TimelineItem {
    project
        .timeline
        .tracks
        .iter_mut()
        .flat_map(|track| track.items.iter_mut())
        .find(|item| item.id == id)
        .expect("timeline item")
}

fn undo(project_dir: &Path, applied: &CodexConversationApplyResult) -> ProjectAgentUndoOutcome {
    undo_latest_agent_project_batch(project_dir, Some(&applied.history_entry_id)).expect("undo")
}

fn assert_removed(outcome: ProjectAgentUndoOutcome, project_dir: &Path, before: &VideoProject) {
    let ProjectAgentUndoOutcome::Undone {
        project,
        removed_generated_asset_ids,
        ..
    } = outcome
    else {
        panic!("expected undone outcome, got {outcome:?}");
    };
    let persisted = load_split_project(project_dir).expect("load undone project");
    assert_eq!(*project, persisted);
    assert_eq!(removed_generated_asset_ids, vec![ASSET.to_string()]);
    assert_eq!(persisted.timeline, before.timeline);
    assert_eq!(persisted.timelines, before.timelines);
    assert!(persisted.generated_assets.is_empty());
    assert_eq!(persisted.media, before.media, "output media is removed");
    assert!(persisted.jobs.iter().all(|job| job.id != JOB));
    assert_eq!(agent_history_json(project_dir)["entries"], json!([]));
}

fn assert_conflict(outcome: ProjectAgentUndoOutcome) {
    assert!(
        matches!(outcome, ProjectAgentUndoOutcome::Conflict { .. }),
        "expected conflict, got {outcome:?}"
    );
}

#[test]
fn apply_records_the_generated_assets_the_batch_added() {
    let (_dir, project_dir, _before, _applied) = applied_generation();

    let entry = &agent_history_json(&project_dir)["entries"][0];

    assert_eq!(entry["afterContentHashVersion"], json!(4));
    assert_eq!(entry["addedGeneratedAssetIds"], json!([ASSET]));
    assert_eq!(entry["addedBookkeepingIds"], json!({ "jobs": [JOB] }));
}

#[test]
fn undo_removes_a_generation_that_ran_completed_and_was_placed_over_its_placeholder() {
    let (_dir, project_dir, before, applied) = applied_generation();
    run_generation(&project_dir);
    complete_generation(&project_dir, false);
    place_output_over_placeholder(&project_dir);

    assert_removed(undo(&project_dir, &applied), &project_dir, &before);
}

#[test]
fn undo_removes_a_generation_while_it_is_queued_running_or_after_it_failed() {
    type Background = fn(&Path);
    let cases: [(&str, Background); 3] = [
        ("queued", |_| {}),
        ("running", run_generation),
        ("failed", |project_dir| {
            run_generation(project_dir);
            write(
                project_dir,
                vec![
                    json!({ "type": "updateJobStatus", "jobId": JOB, "status": "failed", "updatedAt": "2026-09-15T10:00:05Z" }),
                    json!({ "type": "updateGeneratedAssetStatus", "assetId": ASSET, "status": "failed" }),
                ],
            );
        }),
    ];
    for (name, background) in cases {
        let (_dir, project_dir, before, applied) = applied_generation();
        background(&project_dir);

        let outcome = undo(&project_dir, &applied);

        assert!(
            matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }),
            "{name}: {outcome:?}"
        );
        assert_removed(outcome, &project_dir, &before);
    }
}

#[test]
fn undo_removes_a_generation_whose_completion_replaced_the_placeholder_in_place() {
    let (_dir, project_dir, before, applied) = applied_generation();
    run_generation(&project_dir);
    complete_generation(&project_dir, true);
    let completed = load_split_project(&project_dir).expect("load");
    assert!(completed.media.iter().any(|media| media.id == OUTPUT_MEDIA));

    assert_removed(undo(&project_dir, &applied), &project_dir, &before);
}

#[test]
fn undo_conflicts_after_the_user_moves_the_placeholder() {
    let (_dir, project_dir, _before, applied) = applied_generation();
    run_generation(&project_dir);
    user_edit(&project_dir, |project| {
        item_mut(project, PLACEHOLDER).start_seconds = 12.0;
    });

    assert_conflict(undo(&project_dir, &applied));
}

#[test]
fn undo_conflicts_after_the_user_edits_the_placed_output() {
    let (_dir, project_dir, _before, applied) = applied_generation();
    run_generation(&project_dir);
    complete_generation(&project_dir, false);
    place_output_over_placeholder(&project_dir);
    user_edit(&project_dir, |project| {
        item_mut(project, OUTPUT_ITEM)
            .properties
            .insert("opacity".to_string(), json!(0.5));
    });

    assert_conflict(undo(&project_dir, &applied));
}

#[test]
fn undo_conflicts_after_the_user_places_or_renames_the_output_media() {
    let place: fn(&mut VideoProject) = |project| {
        let track = project
            .timeline
            .tracks
            .iter_mut()
            .find(|track| track.kind == TrackKind::Video)
            .expect("video track");
        let mut copy = track.items[0].clone();
        copy.id = "user-placed-output".to_string();
        copy.start_seconds = 20.0;
        copy.duration_seconds = 4.0;
        copy.source = TimelineSource::Media {
            media_id: OUTPUT_MEDIA.to_string(),
        };
        track.items.push(copy);
    };
    let rename: fn(&mut VideoProject) = |project| {
        let media = project
            .media
            .iter_mut()
            .find(|media| media.id == OUTPUT_MEDIA)
            .expect("output media");
        media.name = Some("Hero shot".to_string());
    };
    for edit in [place, rename] {
        let (_dir, project_dir, _before, applied) = applied_generation();
        complete_generation(&project_dir, false);
        place_output_over_placeholder(&project_dir);
        user_edit(&project_dir, edit);

        assert_conflict(undo(&project_dir, &applied));
    }
}

#[test]
fn undo_conflicts_after_the_user_edits_another_clip() {
    let (_dir, project_dir, _before, applied) = applied_generation();
    run_generation(&project_dir);
    user_edit(&project_dir, |project| {
        item_mut(project, "video-1").label = "Manual edit".to_string();
    });

    assert_conflict(undo(&project_dir, &applied));
}

#[test]
fn undo_conflicts_when_a_generation_the_batch_did_not_record_is_edited() {
    let (_dir, project_dir, _saved) = saved_split_project(conversation_fixture_project());
    let saved = load_split_project(&project_dir).expect("load");
    let mut earlier = generation_bundle(&saved, &project_dir);
    earlier.truncate(1);
    let ProjectAction::RecordGeneratedAsset { asset } = &mut earlier[0] else {
        panic!("record action");
    };
    asset.id = "user-generation".to_string();
    apply_project_actions_to_split_project(&project_dir, earlier).expect("earlier generation");
    let before = load_split_project(&project_dir).expect("load");
    let applied = apply_approved(&project_dir, generation_bundle(&before, &project_dir));
    // Its progress is background to the batch (rule version 4, see
    // `undo_background_generations`), but any other change is an edit.
    user_edit(&project_dir, |project| {
        let asset = project
            .generated_assets
            .iter_mut()
            .find(|asset| asset.id == "user-generation")
            .expect("earlier generation");
        asset.prompt = "A lab bench at night".to_string();
    });

    assert_conflict(undo(&project_dir, &applied));
}

/// The version 2 rule: job bookkeeping aside, generated assets are hashed.
fn version_2_hash(project: &VideoProject) -> String {
    let mut content = project.clone();
    content.content_revision = 0;
    content.codex_thread_id = None;
    content.updated_at = String::new();
    content.jobs.clear();
    content.render_reports.clear();
    content.export_artifacts.clear();
    let digest = Sha256::digest(serde_json::to_vec(&content).expect("project json"));
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[test]
fn version_2_entries_keep_the_rule_that_generation_status_blocks_undo() {
    for run in [false, true] {
        let (_dir, project_dir, before, applied) = applied_generation();
        let after = load_split_project(&project_dir).expect("load applied");
        let path = project_dir.join("context").join("agent-history.json");
        let mut history = agent_history_json(&project_dir);
        let entry = history["entries"][0].as_object_mut().expect("entry");
        entry.remove("addedGeneratedAssetIds");
        entry.insert("afterContentHashVersion".to_string(), json!(2));
        entry.insert(
            "afterContentHash".to_string(),
            json!(version_2_hash(&after)),
        );
        std::fs::write(&path, serde_json::to_vec_pretty(&history).expect("json")).expect("write");
        if run {
            run_generation(&project_dir);
        }

        let outcome = undo(&project_dir, &applied);

        if run {
            assert_conflict(outcome);
        } else {
            assert_removed(outcome, &project_dir, &before);
        }
    }
}

#[test]
fn conversation_undo_cancels_the_generations_it_removed_while_they_were_in_flight() {
    let (_dir, project_dir, before, applied) = applied_generation();
    run_generation(&project_dir);
    let run = register_generation_cancellation(&before.id, JOB).expect("register run");
    let cancelled = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&cancelled);

    let outcome = undo_codex_conversation_edit_with(
        &project_dir,
        Some(&applied.history_entry_id),
        &mut |request: &JobProviderRequest| {
            seen.lock().expect("lock").push(request.cancel_url.clone());
            Err("provider is offline".to_string())
        },
    )
    .expect("undo");

    assert!(
        run.token().is_cancelled(),
        "the in-process run is cancelled"
    );
    assert_eq!(
        *cancelled.lock().expect("lock"),
        vec![provider_request().cancel_url]
    );
    let ProjectAgentUndoOutcome::Undone { warnings, .. } = &outcome else {
        panic!("expected undone outcome, got {outcome:?}");
    };
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("provider is offline"));
    assert_removed(outcome, &project_dir, &before);
}

#[test]
fn conversation_undo_leaves_finished_generations_and_refused_undos_alone() {
    let (_dir, project_dir, before, applied) = applied_generation();
    run_generation(&project_dir);
    user_edit(&project_dir, |project| {
        item_mut(project, "video-1").label = "Manual edit".to_string();
    });
    let run = register_generation_cancellation(&before.id, JOB).expect("register run");
    let mut provider_calls = 0;

    let outcome = undo_codex_conversation_edit_with(
        &project_dir,
        Some(&applied.history_entry_id),
        &mut |_: &JobProviderRequest| {
            provider_calls += 1;
            Ok(())
        },
    )
    .expect("undo");

    assert_conflict(outcome);
    assert!(!run.token().is_cancelled());
    drop(run);

    let (_dir, project_dir, before, applied) = applied_generation();
    run_generation(&project_dir);
    complete_generation(&project_dir, false);
    let outcome = undo_codex_conversation_edit_with(
        &project_dir,
        Some(&applied.history_entry_id),
        &mut |_: &JobProviderRequest| {
            provider_calls += 1;
            Ok(())
        },
    )
    .expect("undo");

    assert_removed(outcome, &project_dir, &before);
    assert_eq!(provider_calls, 0);
}
