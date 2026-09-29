//! Guarded, atomic apply of prepared conversation proposals.

use super::support::{
    agent_history_json, conversation_fixture_project, folder_snapshot, record_conversation_turn,
    saved_split_project,
};
use serde_json::json;
use video_creater_lib::codex::conversation::{
    apply_codex_conversation_proposal, prepare_codex_conversation_proposal,
    CodexConversationApplyError, CodexConversationApplyRequest, CodexConversationEditProposal,
    CodexProposalRiskLevel,
};
use video_creater_lib::project::action::{
    ProjectAction, ProjectActionRippleDeleteRange, ProjectActionTrim,
};
use video_creater_lib::project::model::{TrackKind, VideoProject};
use video_creater_lib::project::split::{
    apply_agent_project_action_batch, load_agent_session_manifest, load_split_project,
    save_split_project, validate_split_project,
};

fn proposal(actions: Vec<ProjectAction>) -> CodexConversationEditProposal {
    CodexConversationEditProposal {
        summary: "Tightened the interview.".to_string(),
        edl: Vec::new(),
        project_actions: actions,
        render_review: None,
    }
}

fn volume_proposal() -> CodexConversationEditProposal {
    proposal(vec![ProjectAction::UpdateAudioVolume {
        item_id: "audio-1".to_string(),
        volume_db: Some(-2.0),
    }])
}

fn removal_proposal() -> CodexConversationEditProposal {
    proposal(vec![ProjectAction::RemoveItems {
        item_ids: vec!["video-1".to_string()],
    }])
}

fn apply_request(
    project: &VideoProject,
    proposal: CodexConversationEditProposal,
    review_approved: bool,
) -> CodexConversationApplyRequest {
    let prepared =
        prepare_codex_conversation_proposal(project, &proposal).expect("proposal prepares");
    CodexConversationApplyRequest {
        proposal,
        action_ids: prepared.action_ids,
        review_approved,
        session_id: None,
    }
}

fn audio_volume(project: &VideoProject) -> Option<f64> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.items)
        .find(|item| item.id == "audio-1")
        .and_then(|item| item.properties.get("volumeDb"))
        .and_then(serde_json::Value::as_f64)
}

#[test]
fn safe_agent_project_action_batch_applies_records_history_and_marks_the_session() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    record_conversation_turn(&project_dir, &before);
    let request = apply_request(&before, volume_proposal(), false);
    let expected_ids = request.action_ids.clone();

    let result = apply_codex_conversation_proposal(&project_dir, request).expect("safe apply");

    let persisted = load_split_project(&project_dir).expect("load applied project");
    assert_eq!(result.project, persisted);
    assert_eq!(audio_volume(&persisted), Some(-2.0));
    assert_eq!(persisted.content_revision, before.content_revision + 1);
    assert_eq!(result.action_ids, expected_ids);
    assert_eq!(result.risk.level, CodexProposalRiskLevel::Safe);
    assert_eq!(result.impact.affected_item_ids, vec!["audio-1"]);
    assert!(result.warnings.is_empty());

    let history = agent_history_json(&project_dir);
    let entries = history["entries"].as_array().expect("history entries");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], json!(result.history_entry_id));
    assert_eq!(entries[0]["actionIds"], json!(expected_ids));
    assert_eq!(entries[0]["actionCount"], json!(1));
    assert!(entries[0]["afterContentHash"]
        .as_str()
        .is_some_and(|hash| hash.starts_with("sha256:")));
    assert!(entries[0].get("after").is_none(), "no second full snapshot");

    let sessions = load_agent_session_manifest(&project_dir).expect("sessions");
    let session = &sessions.sessions[0];
    assert_eq!(session.proposal_status, "applied");
    assert_eq!(session.applied_action_ids, expected_ids);
    let turn = session.turns.last().expect("last turn");
    assert_eq!(turn.proposal_status, "applied");
    assert_eq!(turn.applied_action_ids, expected_ids);
}

#[test]
fn review_bundle_is_refused_without_approval_and_leaves_the_folder_unchanged() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let snapshot = folder_snapshot(&project_dir);
    let request = apply_request(&before, removal_proposal(), false);

    let error = apply_codex_conversation_proposal(&project_dir, request)
        .expect_err("review bundles need approval");

    assert_eq!(error, CodexConversationApplyError::ReviewRequired);
    assert_eq!(error.stable_code(), "codex.conversation.reviewRequired");
    assert!(error.to_string().contains("needs your review"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn review_bundle_applies_once_approved() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let request = apply_request(&before, removal_proposal(), true);

    let result = apply_codex_conversation_proposal(&project_dir, request).expect("approved apply");

    assert_eq!(result.risk.level, CodexProposalRiskLevel::Review);
    let persisted = load_split_project(&project_dir).expect("load applied project");
    assert!(persisted
        .timeline
        .tracks
        .iter()
        .flat_map(|track| &track.items)
        .all(|item| item.id != "video-1"));
    assert_eq!(
        agent_history_json(&project_dir)["entries"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn tampered_action_ids_are_rejected_without_writing() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let snapshot = folder_snapshot(&project_dir);
    let mut request = apply_request(&before, volume_proposal(), false);
    request.action_ids = vec!["codex-action-1-000000000000".to_string()];

    let error =
        apply_codex_conversation_proposal(&project_dir, request).expect_err("stale action ids");

    assert_eq!(error, CodexConversationApplyError::StaleProposal);
    assert!(error
        .to_string()
        .contains("changed after this edit was prepared"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn proposal_that_no_longer_fits_the_project_is_rejected_without_writing() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    let request = apply_request(&before, volume_proposal(), false);
    // The user deletes the targeted clip after the proposal was prepared.
    let mut edited = before.clone();
    for track in &mut edited.timeline.tracks {
        track.items.retain(|item| item.id != "audio-1");
    }
    save_split_project(&project_dir, &edited).expect("user edit");
    let snapshot = folder_snapshot(&project_dir);

    let error =
        apply_codex_conversation_proposal(&project_dir, request).expect_err("stale project");

    assert!(matches!(
        error,
        CodexConversationApplyError::NoLongerValid(_)
    ));
    assert!(error.to_string().contains("no longer fits the project"));
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn failing_action_in_agent_project_action_batch_leaves_the_folder_unchanged() {
    let (_dir, project_dir, before) = saved_split_project(conversation_fixture_project());
    record_conversation_turn(&project_dir, &before);
    let snapshot = folder_snapshot(&project_dir);

    let error = apply_agent_project_action_batch(
        &project_dir,
        vec![
            ProjectAction::UpdateAudioVolume {
                item_id: "audio-1".to_string(),
                volume_db: Some(-2.0),
            },
            ProjectAction::TrimItems {
                trims: vec![ProjectActionTrim {
                    item_id: "missing-item".to_string(),
                    start_seconds: 0.0,
                    duration_seconds: 1.0,
                    source_in: None,
                    source_out: None,
                }],
            },
        ],
        vec![
            "codex-action-1-a".to_string(),
            "codex-action-2-b".to_string(),
        ],
        None,
    );

    assert!(error.is_err());
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

#[test]
fn agent_project_action_batch_requires_one_id_per_action() {
    let (_dir, project_dir, _before) = saved_split_project(conversation_fixture_project());
    let snapshot = folder_snapshot(&project_dir);

    let error = apply_agent_project_action_batch(
        &project_dir,
        vec![ProjectAction::UpdateAudioVolume {
            item_id: "audio-1".to_string(),
            volume_db: Some(-2.0),
        }],
        Vec::new(),
        None,
    );

    assert!(error.is_err());
    assert_eq!(folder_snapshot(&project_dir), snapshot);
}

pub fn speed_fixture_project() -> VideoProject {
    let mut project = conversation_fixture_project();
    let video = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .and_then(|track| track.items.first_mut())
        .expect("video item");
    video.properties.insert("speed".to_string(), json!(2.0));
    video
        .properties
        .insert("sourceOut".to_string(), json!(20.0));
    project.media[0].duration_seconds = 30.0;
    project.timelines[0].timeline = project.timeline.clone();
    project
}

pub fn ripple_proposal() -> CodexConversationEditProposal {
    proposal(vec![ProjectAction::RippleDeleteRanges {
        ranges: vec![ProjectActionRippleDeleteRange {
            start_seconds: 4.0,
            end_seconds: 6.0,
            track_ids: vec!["track-video".to_string(), "track-audio".to_string()],
        }],
    }])
}

#[test]
fn speed_aware_clips_keep_speed_and_scaled_source_ranges_after_apply() {
    let (_dir, project_dir, before) = saved_split_project(speed_fixture_project());
    let request = apply_request(&before, ripple_proposal(), false);

    apply_codex_conversation_proposal(&project_dir, request).expect("ripple apply");

    let persisted = load_split_project(&project_dir).expect("load applied project");
    assert!(validate_split_project(&project_dir).expect("validate").ok);
    let video_items = persisted
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .map(|track| track.items.clone())
        .expect("video track");
    assert_eq!(video_items.len(), 2);
    for item in &video_items {
        assert_eq!(item.properties["speed"], json!(2.0));
        let source_in = item.properties["sourceIn"].as_f64().expect("sourceIn");
        let source_out = item.properties["sourceOut"].as_f64().expect("sourceOut");
        assert!((source_out - source_in - item.duration_seconds * 2.0).abs() < 1e-9);
    }
    // The tail piece resumes after the removed 2 timeline seconds = 4 source seconds.
    assert_eq!(video_items[1].properties["sourceIn"], json!(12.0));
}
