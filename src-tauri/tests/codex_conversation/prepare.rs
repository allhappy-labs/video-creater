//! Rust-prepared proposal bundles: exact actions, action IDs, impact, and the
//! fail-closed risk allowlist.

use super::support::{
    conversation_fixture_project, media_item, sample_conversation_request, sample_skill_bundle,
    FakeTransport,
};
use serde_json::{json, Value};
use video_creater_lib::codex::app_server::start_codex_conversation_turn;
use video_creater_lib::codex::conversation::{
    classify_codex_proposal_risk, is_safe_local_action, prepare_codex_conversation_proposal,
    CodexConversationEditProposal, CodexPreparedProposal, CodexProposalRiskLevel,
};
use video_creater_lib::project::action::{
    ProjectAction, ProjectActionRippleDeleteRange, ProjectActionTrim,
};
use video_creater_lib::project::model::{TimelineItemKind, TrackKind, VideoProject};

fn proposal_with_actions(actions: Vec<ProjectAction>) -> CodexConversationEditProposal {
    CodexConversationEditProposal {
        summary: "Tightened the interview.".to_string(),
        edl: Vec::new(),
        project_actions: actions,
        render_review: None,
    }
}

fn prepare_on(project: &VideoProject, actions: Vec<ProjectAction>) -> CodexPreparedProposal {
    prepare_codex_conversation_proposal(project, &proposal_with_actions(actions))
        .expect("proposal should prepare")
}

fn prepare_with_actions(actions: Vec<ProjectAction>) -> CodexPreparedProposal {
    prepare_on(&conversation_fixture_project(), actions)
}

fn action(value: Value) -> ProjectAction {
    serde_json::from_value(value).expect("canonical project action")
}

fn ranges(prepared: &CodexPreparedProposal) -> Vec<(f64, f64)> {
    prepared
        .impact
        .affected_ranges
        .iter()
        .map(|range| (range.start_seconds, range.end_seconds))
        .collect()
}

#[test]
fn ripple_dead_air_edit_is_safe_and_reports_duration_change() {
    let prepared = prepare_with_actions(vec![ProjectAction::RippleDeleteRanges {
        ranges: vec![ProjectActionRippleDeleteRange {
            start_seconds: 4.0,
            end_seconds: 6.0,
            track_ids: vec!["track-video".to_string(), "track-audio".to_string()],
        }],
    }]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert!(prepared.risk.reasons.is_empty());
    assert_eq!(prepared.impact.affected_ranges[0].start_seconds, 4.0);
    assert!(prepared.impact.after_duration_seconds < prepared.impact.before_duration_seconds);
    assert_eq!(prepared.impact.before_duration_seconds, 10.0);
    assert_eq!(prepared.impact.after_duration_seconds, 8.0);
    assert_eq!(ranges(&prepared), vec![(4.0, 10.0)]);
    assert_eq!(prepared.impact.preview_timestamp, 4.0);
    for id in [
        "video-1",
        "audio-1",
        "video-1-ripple-6000",
        "audio-1-ripple-6000",
    ] {
        assert!(
            prepared
                .impact
                .affected_item_ids
                .iter()
                .any(|item| item == id),
            "{id} should be affected: {:?}",
            prepared.impact.affected_item_ids
        );
    }
    assert!(!prepared.impact.summary.contains("video-1"));
}

#[test]
fn full_timeline_removal_requires_review() {
    let prepared = prepare_with_actions(vec![ProjectAction::RemoveItems {
        item_ids: vec!["video-1".to_string(), "audio-1".to_string()],
    }]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Review);
    assert!(prepared
        .risk
        .reasons
        .iter()
        .any(|reason| reason.code == "deletesExistingItems"));
    assert_eq!(prepared.impact.after_duration_seconds, 0.0);
    assert_eq!(ranges(&prepared), vec![(0.0, 10.0)]);
    assert_eq!(prepared.impact.preview_timestamp, 0.0);
}

#[test]
fn risk_review_classes_cover_destructive_generation_job_export_settings_and_templates() {
    let render_settings = json!({
        "width": 1280, "height": 720, "fps": 30.0, "loudnessLufs": -16.0, "captions": "burn_in",
    });
    let cases = [
        (
            json!({ "type": "removeItems", "itemIds": ["video-1"] }),
            "deletesExistingItems",
        ),
        (
            json!({ "type": "deleteMedia", "mediaIds": ["media-1"] }),
            "deletesMedia",
        ),
        (
            json!({ "type": "removeTracks", "trackIds": ["track-video"] }),
            "removesTracks",
        ),
        (
            json!({ "type": "deleteTimeline", "timelineId": "timeline-2" }),
            "deletesTimeline",
        ),
        (
            json!({ "type": "deleteMediaFolder", "folderId": "folder-1" }),
            "deletesMediaFolder",
        ),
        (
            json!({ "type": "decomposeTimelineItem", "itemId": "video-1" }),
            "decomposesItem",
        ),
        (
            json!({ "type": "updateRenderSettings", "settings": render_settings }),
            "changesProjectSettings",
        ),
        (
            json!({ "type": "updateProjectSettings", "name": "Renamed", "renderSettings": render_settings }),
            "changesProjectSettings",
        ),
        (
            json!({ "type": "updateGeneratedAssetStatus", "assetId": "asset-1", "status": "queued" }),
            "changesGeneratedAssets",
        ),
        (
            json!({ "type": "updateJobStatus", "jobId": "job-1", "status": "queued", "updatedAt": "2026-07-25T00:00:00Z" }),
            "changesJobs",
        ),
        (
            json!({ "type": "recordExportArtifact", "artifact": {
                "schemaVersion": 1, "id": "export-1", "kind": "mp4", "format": "mp4",
                "path": "exports/export-1.mp4", "mimeType": "video/mp4",
                "createdAt": "2026-07-25T00:00:00Z",
            }}),
            "changesRenderMetadata",
        ),
        (
            json!({ "type": "updateTemplateOverride", "override": {
                "templateId": "lower-third", "name": "Lower third", "visualTreatment": "clean",
                "motion": "slide", "safeZone": "lower", "avoid": "faces",
            }}),
            "changesTemplateOverride",
        ),
        (
            json!({ "type": "createTimeline", "timelineId": "timeline-2", "name": "Alt", "duplicateActive": true }),
            "unclassifiedAction",
        ),
        (
            json!({ "type": "renameMedia", "mediaId": "media-1", "name": "Interview A" }),
            "unclassifiedAction",
        ),
    ];
    for (value, code) in cases {
        let action = action(value);
        assert!(
            !is_safe_local_action(&action),
            "{action:?} must not be safe"
        );
        let risk = classify_codex_proposal_risk(std::slice::from_ref(&action));
        assert_eq!(risk.level, CodexProposalRiskLevel::Review, "{action:?}");
        assert_eq!(risk.reasons.len(), 1, "{action:?}");
        assert_eq!(risk.reasons[0].code, code, "{action:?}");
        assert!(!risk.reasons[0].message.trim().is_empty());
    }
}

#[test]
fn risk_safe_allowlist_covers_local_edits_and_additions() {
    let item = serde_json::to_value(media_item(
        "video-2",
        TimelineItemKind::VideoClip,
        10.0,
        2.0,
    ))
    .expect("item json");
    let cases = [
        json!({ "type": "addItems", "targetTrackId": "track-video", "items": [item] }),
        json!({ "type": "trimItems", "trims": [{ "itemId": "audio-1", "startSeconds": 0.0, "durationSeconds": 6.0 }] }),
        json!({ "type": "splitItems", "splits": [{ "itemId": "audio-1", "newItemId": "audio-2", "splitSeconds": 5.0 }] }),
        json!({ "type": "rippleDeleteRanges", "ranges": [{ "startSeconds": 1.0, "endSeconds": 2.0, "trackIds": ["track-audio"] }] }),
        json!({ "type": "rippleTrimItem", "itemId": "audio-1", "edge": "right", "deltaSeconds": -1.0, "propagateLinked": false }),
        json!({ "type": "moveItems", "moves": [{ "itemId": "audio-1", "targetTrackId": "track-audio", "startSeconds": 1.0 }] }),
        json!({ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }),
        json!({ "type": "editCaptionText", "itemId": "caption-1", "text": "Hello" }),
        json!({ "type": "editTextItem", "itemId": "title-1", "text": "Hello" }),
        json!({ "type": "updateVisualClipSpeed", "itemId": "video-1", "speed": 2.0 }),
        json!({ "type": "updateAudioClipSpeed", "itemId": "audio-1", "speed": 2.0 }),
        json!({ "type": "detachAudio", "itemId": "video-1", "audioItemId": "video-1-audio", "targetTrackId": "track-audio", "linkGroupId": "link-video-1" }),
        json!({ "type": "updateClipReverse", "itemId": "video-1", "reverse": true }),
        json!({ "type": "updateItemEffects", "itemIds": ["video-1"], "effects": [] }),
        json!({ "type": "setTrackEnabled", "trackId": "track-audio", "enabled": false }),
        json!({ "type": "setTrackLocked", "trackId": "track-audio", "locked": true }),
        json!({ "type": "createTrack", "track": {
            "id": "track-b-roll", "name": "B-roll", "kind": "video", "locked": false, "items": [],
        }}),
        json!({ "type": "linkItems", "itemIds": ["video-1", "audio-1"], "linkGroupId": "link-1" }),
        json!({ "type": "unlinkItems", "itemIds": ["video-1", "audio-1"] }),
        json!({ "type": "deleteItemKeyframe", "itemId": "video-1", "property": "opacity", "atSeconds": 1.0 }),
        json!({ "type": "addTransition", "trackId": "track-video", "transition": {
            "id": "fade-1", "leftItemId": "video-1", "rightItemId": "video-2",
            "kind": "crossfade", "durationSeconds": 0.5,
        }}),
        json!({ "type": "updateTransition", "trackId": "track-video", "transitionId": "fade-1", "kind": "wipe" }),
        json!({ "type": "removeTransition", "trackId": "track-video", "transitionId": "fade-1" }),
    ];
    for value in cases {
        let action = action(value);
        assert!(is_safe_local_action(&action), "{action:?} should be safe");
        let risk = classify_codex_proposal_risk(std::slice::from_ref(&action));
        assert_eq!(risk.level, CodexProposalRiskLevel::Safe, "{action:?}");
        assert!(risk.reasons.is_empty());
    }
}

/// `video-1` [0, 5) and `video-2` [5, 10) meet at 5 s with 5 s of handles.
fn adjacent_clips_project() -> VideoProject {
    let mut project = conversation_fixture_project();
    let track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .expect("video track");
    track.items = vec![
        media_item("video-1", TimelineItemKind::VideoClip, 0.0, 5.0),
        media_item("video-2", TimelineItemKind::VideoClip, 5.0, 5.0),
    ];
    project.timelines[0].timeline = project.timeline.clone();
    project
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

#[test]
fn transition_only_proposals_are_safe_and_report_the_cut_window() {
    let mut project = adjacent_clips_project();
    let track_id = video_track_id(&project);
    let add = action(
        json!({ "type": "addTransition", "trackId": track_id, "transition": {
            "id": "fade-1", "leftItemId": "video-1", "rightItemId": "video-2",
            "kind": "dipToBlack", "durationSeconds": 1.0,
        }}),
    );

    let prepared = prepare_on(&project, vec![add.clone()]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert!(prepared.risk.reasons.is_empty());
    assert_eq!(ranges(&prepared), vec![(4.5, 5.5)]);
    assert_eq!(
        prepared.impact.affected_item_ids,
        vec!["video-1", "video-2"]
    );
    assert_eq!(prepared.impact.preview_timestamp, 4.5);
    assert_eq!(
        prepared.impact.summary,
        "Changes 2 items; the timeline stays 10.0s."
    );

    video_creater_lib::project::action::apply_project_action(&mut project, add)
        .expect("transition applies");
    for value in [
        json!({ "type": "updateTransition", "trackId": track_id, "transitionId": "fade-1", "durationSeconds": 2.0 }),
        json!({ "type": "removeTransition", "trackId": track_id, "transitionId": "fade-1" }),
    ] {
        let prepared = prepare_on(&project, vec![action(value)]);
        assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
        assert_eq!(
            prepared.impact.affected_item_ids,
            vec!["video-1", "video-2"]
        );
        assert!(!prepared.impact.affected_ranges.is_empty());
    }
}

#[test]
fn invalid_transition_proposals_do_not_prepare() {
    let project = adjacent_clips_project();
    let track_id = video_track_id(&project);
    let proposal = proposal_with_actions(vec![action(json!({
        "type": "addTransition", "trackId": track_id, "transition": {
            "id": "fade-1", "leftItemId": "video-1", "rightItemId": "video-2",
            "kind": "crossfade", "durationSeconds": 12.0,
        }
    }))]);

    let error = prepare_codex_conversation_proposal(&project, &proposal)
        .expect_err("an overlong transition is invalid");
    assert!(
        error
            .to_string()
            .contains("Transition duration cannot be longer than 5.0s."),
        "{error}"
    );
}

#[test]
fn risk_any_review_action_escalates_and_reasons_are_deduplicated() {
    let risk = classify_codex_proposal_risk(&[
        action(json!({ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 })),
        action(json!({ "type": "removeItems", "itemIds": ["video-1"] })),
        action(json!({ "type": "removeItems", "itemIds": ["audio-1"] })),
        action(json!({ "type": "deleteMedia", "mediaIds": ["media-1"] })),
    ]);
    assert_eq!(risk.level, CodexProposalRiskLevel::Review);
    let codes = risk
        .reasons
        .iter()
        .map(|reason| reason.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(codes, vec!["deletesExistingItems", "deletesMedia"]);
}

#[test]
fn local_trim_reports_only_the_trimmed_tail() {
    let prepared = prepare_with_actions(vec![ProjectAction::TrimItems {
        trims: vec![ProjectActionTrim {
            item_id: "audio-1".to_string(),
            start_seconds: 0.0,
            duration_seconds: 6.0,
            source_in: Some(0.0),
            source_out: Some(6.0),
        }],
    }]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert_eq!(prepared.impact.affected_item_ids, vec!["audio-1"]);
    assert_eq!(ranges(&prepared), vec![(6.0, 10.0)]);
    assert_eq!(prepared.impact.after_duration_seconds, 10.0);
    assert_eq!(prepared.impact.preview_timestamp, 6.0);
}

#[test]
fn head_trim_impact_respects_clip_speed() {
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
    project.timelines[0].timeline = project.timeline.clone();

    // At 2x, trimming 2 timeline seconds from the head skips 4 source seconds,
    // so the remaining footage keeps its exact timeline position.
    let prepared = prepare_on(
        &project,
        vec![ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "video-1".to_string(),
                start_seconds: 2.0,
                duration_seconds: 8.0,
                source_in: Some(4.0),
                source_out: Some(20.0),
            }],
        }],
    );
    assert_eq!(ranges(&prepared), vec![(0.0, 2.0)]);
    assert_eq!(prepared.impact.affected_item_ids, vec!["video-1"]);
}

#[test]
fn property_edit_reports_the_whole_item_span_without_duration_change() {
    let prepared = prepare_with_actions(vec![ProjectAction::UpdateAudioVolume {
        item_id: "audio-1".to_string(),
        volume_db: Some(-2.0),
    }]);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert_eq!(prepared.impact.affected_item_ids, vec!["audio-1"]);
    assert_eq!(ranges(&prepared), vec![(0.0, 10.0)]);
    assert_eq!(
        prepared.impact.before_duration_seconds,
        prepared.impact.after_duration_seconds
    );
    assert_eq!(prepared.impact.preview_timestamp, 0.0);
}

#[test]
fn preview_timestamp_is_clamped_below_the_resulting_duration() {
    let trim = |item_id: &str| ProjectActionTrim {
        item_id: item_id.to_string(),
        start_seconds: 0.0,
        duration_seconds: 6.0,
        source_in: Some(0.0),
        source_out: Some(6.0),
    };
    let prepared = prepare_with_actions(vec![ProjectAction::TrimItems {
        trims: vec![trim("video-1"), trim("audio-1")],
    }]);
    assert_eq!(ranges(&prepared), vec![(6.0, 10.0)]);
    assert_eq!(prepared.impact.after_duration_seconds, 6.0);
    assert!(prepared.impact.preview_timestamp < 6.0);
    assert!(prepared.impact.preview_timestamp >= 5.9);
}

#[test]
fn prepared_bundle_has_stable_action_ids_and_camel_case_contract() {
    let actions = vec![
        ProjectAction::UpdateAudioVolume {
            item_id: "audio-1".to_string(),
            volume_db: Some(-2.0),
        },
        ProjectAction::UpdateAudioVolume {
            item_id: "audio-1".to_string(),
            volume_db: Some(-2.0),
        },
    ];
    let prepared = prepare_with_actions(actions.clone());
    let again = prepare_with_actions(actions.clone());
    assert_eq!(prepared.actions, actions);
    assert_eq!(prepared.action_ids.len(), 2);
    assert_ne!(prepared.action_ids[0], prepared.action_ids[1]);
    assert_eq!(prepared.action_ids, again.action_ids);

    let value = serde_json::to_value(&prepared).expect("serialize prepared proposal");
    assert_eq!(value["actions"][0]["type"], "updateAudioVolume");
    assert_eq!(value["actionIds"], json!(prepared.action_ids));
    assert_eq!(value["risk"], json!({ "level": "safe", "reasons": [] }));
    assert_eq!(value["impact"]["beforeDurationSeconds"], json!(10.0));
    assert_eq!(value["impact"]["afterDurationSeconds"], json!(10.0));
    assert_eq!(value["impact"]["affectedItemIds"], json!(["audio-1"]));
    assert_eq!(
        value["impact"]["affectedRanges"],
        json!([{ "startSeconds": 0.0, "endSeconds": 10.0 }])
    );
    assert_eq!(value["impact"]["previewTimestamp"], json!(0.0));
    assert!(value["impact"]["summary"].is_string());
    let round_trip: CodexPreparedProposal =
        serde_json::from_value(value).expect("prepared proposal deserializes");
    assert_eq!(round_trip, prepared);

    let review = prepare_with_actions(vec![ProjectAction::RemoveItems {
        item_ids: vec!["audio-1".to_string()],
    }]);
    let value = serde_json::to_value(&review.risk).expect("serialize risk");
    assert_eq!(value["level"], "review");
    assert_eq!(value["reasons"][0]["code"], "deletesExistingItems");
}

#[test]
fn conversation_turn_returns_the_rust_prepared_bundle() {
    let mut project = conversation_fixture_project();
    let mut transport = FakeTransport::new(json!({
        "summary": "Lowered the interview clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-1", "volumeDb": -2.0 }],
        "renderReview": null,
    }));
    let result = start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        None,
    )
    .expect("conversation turn");

    assert_eq!(result.proposal_validation_issues, Some(Vec::new()));
    let prepared = result
        .prepared_proposal
        .expect("valid proposals are prepared");
    assert_eq!(prepared.actions.len(), 1);
    assert_eq!(prepared.action_ids.len(), 1);
    assert_eq!(prepared.risk.level, CodexProposalRiskLevel::Safe);
    assert_eq!(prepared.impact.affected_item_ids, vec!["audio-1"]);
}

#[test]
fn conversation_turn_withholds_prepared_bundle_for_invalid_proposals() {
    let mut project = conversation_fixture_project();
    let mut transport = FakeTransport::new(json!({
        "summary": "Lowered a missing clip.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "audio-9", "volumeDb": -2.0 }],
        "renderReview": null,
    }));
    let result = start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request(),
        &sample_skill_bundle(),
        None,
    )
    .expect("invalid proposals stay reviewable");

    assert!(result.proposal.is_some());
    assert!(result.prepared_proposal.is_none());
    let issues = result
        .proposal_validation_issues
        .expect("validation should run");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].path, "projectActions");
}
