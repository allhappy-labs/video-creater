//! How agent Undo history is stored: snapshots are written compressed
//! (`deflate-base64`), older plain snapshots still read and undo, and the
//! retained snapshot bytes stay under the history's size cap.

use super::support::{agent_history_json, conversation_fixture_project, saved_split_project};
use serde_json::{json, Value};
use std::path::Path;
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{
    Transcript, TranscriptSegment, TranscriptWord, VideoProject,
};
use video_creater_lib::project::split::{
    agent_undo_comparable_content, apply_agent_project_action_batch, load_split_project,
    save_split_project, undo_latest_agent_project_batch, ProjectAgentUndoOutcome,
    MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES,
};

const TRANSCRIPT_WORDS: usize = 50_000;
const APPLIES: usize = 20;

fn volume_change(project_dir: &Path, index: usize) -> String {
    let action: ProjectAction = serde_json::from_value(json!({
        "type": "updateAudioVolume",
        "itemId": "audio-1",
        "volumeDb": -1.0 - index as f64 * 0.25,
    }))
    .expect("volume action");
    apply_agent_project_action_batch(
        project_dir,
        vec![action],
        vec![format!("storage-action-{index}")],
        None,
    )
    .expect("apply volume change")
    .history_entry_id
}

fn snapshots(entry: &Value) -> Vec<&Value> {
    ["before", "after"]
        .into_iter()
        .filter_map(|key| entry.get(key))
        .collect()
}

fn same_content(left: &VideoProject, right: &VideoProject) -> bool {
    agent_undo_comparable_content(left, &[]) == agent_undo_comparable_content(right, &[])
}

#[test]
fn applied_batches_store_compressed_snapshots() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let entry_id = volume_change(&project_dir, 0);

    let history = agent_history_json(&project_dir);
    let snapshot = &history["entries"][0]["before"];
    assert_eq!(snapshot["encoding"], json!("deflate-base64"), "{snapshot}");
    assert!(snapshot["jsonBytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));
    assert!(snapshot["data"].is_string());
    assert!(snapshot.get("timeline").is_none());

    let outcome = undo_latest_agent_project_batch(&project_dir, Some(&entry_id)).expect("undo");

    assert!(
        matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }),
        "{outcome:?}"
    );
    let restored = load_split_project(&project_dir).expect("restored project");
    assert!(same_content(&restored, &before));
    assert_eq!(restored.timeline, before.timeline);
}

#[test]
fn a_history_file_with_plain_snapshots_still_undoes() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let mut changed = before.clone();
    changed.name = "Agent edit".to_string();
    let after = save_split_project(&project_dir, &changed)
        .expect("save after")
        .project;
    let legacy = json!({
        "schemaVersion": 1,
        "entries": [{ "id": "agent-edit-1", "actionCount": 1, "before": before, "after": after }],
    });
    std::fs::write(
        project_dir.join("context").join("agent-history.json"),
        serde_json::to_vec_pretty(&legacy).expect("legacy json"),
    )
    .expect("write legacy history");

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo");

    assert!(
        matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }),
        "{outcome:?}"
    );
    let restored = load_split_project(&project_dir).expect("restored project");
    assert_eq!(restored.name, before.name);
    assert!(same_content(&restored, &before));
}

fn long_transcript_project() -> VideoProject {
    let mut project = conversation_fixture_project();
    let words = (0..TRANSCRIPT_WORDS)
        .map(|index| TranscriptWord {
            text: format!("word{}", index % 997),
            start_seconds: index as f64 * 0.3,
            end_seconds: index as f64 * 0.3 + 0.25,
            confidence: Some(0.9 + (index % 10) as f64 * 0.005),
            speaker: Some(format!("speaker-{}", index % 3)),
        })
        .collect::<Vec<_>>();
    let segments = words
        .chunks(12)
        .map(|chunk| TranscriptSegment {
            text: chunk
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            start_seconds: chunk[0].start_seconds,
            end_seconds: chunk[chunk.len() - 1].end_seconds,
        })
        .collect();
    project.media[0].duration_seconds = TRANSCRIPT_WORDS as f64 * 0.3;
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("parakeet".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments,
        words,
    });
    project
}

#[test]
fn a_large_history_stays_under_the_retained_size_cap() {
    let (_dir, project_dir, before) = saved_split_project(long_transcript_project());
    for index in 0..APPLIES {
        volume_change(&project_dir, index);
    }

    let history = agent_history_json(&project_dir);
    let entries = history["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), APPLIES);
    let stored = entries.iter().flat_map(snapshots).collect::<Vec<_>>();
    assert!(stored
        .iter()
        .all(|snapshot| snapshot["encoding"] == json!("deflate-base64")));
    let data_bytes = stored
        .iter()
        .map(|snapshot| snapshot["data"].as_str().map_or(0, str::len))
        .sum::<usize>();
    let json_bytes = stored
        .iter()
        .map(|snapshot| snapshot["jsonBytes"].as_u64().unwrap_or(0) as usize)
        .sum::<usize>();
    let file_bytes = std::fs::metadata(project_dir.join("context").join("agent-history.json"))
        .expect("history file")
        .len();
    eprintln!(
        "agent history with {APPLIES} snapshots of a {TRANSCRIPT_WORDS}-word transcript: \
         {json_bytes} plain snapshot JSON bytes, {data_bytes} stored snapshot bytes, \
         {file_bytes} history file bytes"
    );
    assert!(data_bytes > 0 && data_bytes < json_bytes);
    assert!(data_bytes <= MAX_AGENT_EDIT_HISTORY_RETAINED_BYTES);

    let outcome = undo_latest_agent_project_batch(&project_dir, None).expect("undo");
    assert!(
        matches!(outcome, ProjectAgentUndoOutcome::Undone { .. }),
        "{outcome:?}"
    );
    let restored = load_split_project(&project_dir).expect("restored project");
    assert_eq!(restored.transcripts, before.transcripts);
}
