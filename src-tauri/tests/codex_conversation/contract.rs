use super::support::{
    clip, conversation_fixture_project, edl_proposal, empty_timeline_project, media_item,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use video_creater_lib::codex::conversation::{
    prepare_codex_conversation_proposal, CodexConversationEditProposal,
    CodexConversationEditRequest, CodexConversationError, CodexConversationFocus,
    CodexConversationRange,
};
use video_creater_lib::project::action::ProjectAction;
use video_creater_lib::project::model::{TimelineItem, TimelineItemKind, TimelineSource};

#[test]
fn conversation_request_has_no_creative_preset() {
    let request = CodexConversationEditRequest {
        prompt: "Balance the dialogue.".to_string(),
        focus: CodexConversationFocus::default(),
        created_at: "2026-07-25T00:00:00Z".to_string(),
    };
    let value = serde_json::to_value(request).expect("serialize request");
    assert_eq!(value["prompt"], "Balance the dialogue.");
    assert!(value.get("preset").is_none());
    assert!(value.get("targetDurationSeconds").is_none());
    assert!(value.get("captionStyle").is_none());
    assert!(value.get("mediaId").is_none());
    assert_eq!(value["focus"]["mediaIds"], json!([]));
    assert_eq!(value["focus"]["timelineItemIds"], json!([]));
}

#[test]
fn conversation_request_deserializes_without_focus() {
    let request: CodexConversationEditRequest = serde_json::from_value(json!({
        "prompt": "Tighten the pacing",
        "createdAt": "2026-07-25T00:00:00Z",
    }))
    .expect("focus defaults");
    assert_eq!(request.focus, CodexConversationFocus::default());
}

#[test]
fn direct_existing_item_edit_does_not_require_an_edl() {
    let project = conversation_fixture_project();
    let proposal = CodexConversationEditProposal {
        summary: "Lowered the interview clip by 2 dB.".to_string(),
        edl: Vec::new(),
        project_actions: vec![ProjectAction::UpdateAudioVolume {
            item_id: "audio-1".to_string(),
            volume_db: Some(-2.0),
        }],
        render_review: None,
    };
    let prepared = prepare_codex_conversation_proposal(&project, &proposal)
        .expect("direct edit should validate");
    assert_eq!(prepared.actions.len(), 1);
    let value = serde_json::to_value(&proposal).expect("serialize proposal");
    assert_eq!(value["edl"], json!([]));
    assert_eq!(value["renderReview"], Value::Null);
}

#[test]
fn newly_added_primary_video_item_requires_an_edl() {
    let project = conversation_fixture_project();
    let proposal = CodexConversationEditProposal {
        summary: "Added a second shot.".to_string(),
        edl: Vec::new(),
        project_actions: vec![ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![media_item(
                "video-2",
                TimelineItemKind::VideoClip,
                10.0,
                2.0,
            )],
        }],
        render_review: None,
    };
    let error = prepare_codex_conversation_proposal(&project, &proposal)
        .expect_err("new primary media needs an EDL");
    assert_eq!(error, CodexConversationError::MissingEdl);
    assert_eq!(error.validation_issue().path, "edl");
}

#[test]
fn empty_proposal_is_rejected() {
    let project = conversation_fixture_project();
    let proposal = CodexConversationEditProposal {
        summary: "Nothing to do.".to_string(),
        edl: Vec::new(),
        project_actions: Vec::new(),
        render_review: None,
    };
    assert_eq!(
        prepare_codex_conversation_proposal(&project, &proposal),
        Err(CodexConversationError::EmptyActions)
    );
}

#[test]
fn edl_ranges_must_reference_existing_media_with_ordered_ranges() {
    let project = empty_timeline_project();
    let mut proposal = edl_proposal(vec![clip("media-missing", 0.0, 2.0)], 2.0);
    assert_eq!(
        prepare_codex_conversation_proposal(&project, &proposal),
        Err(CodexConversationError::EdlMediaNotFound(
            "media-missing".to_string()
        ))
    );

    proposal.edl = vec![clip("media-1", 4.0, 4.0)];
    assert!(matches!(
        prepare_codex_conversation_proposal(&project, &proposal),
        Err(CodexConversationError::InvalidEdlRange { index: 0, .. })
    ));

    proposal.edl = vec![clip("media-1", 0.0, 500.0)];
    assert!(matches!(
        prepare_codex_conversation_proposal(&project, &proposal),
        Err(CodexConversationError::InvalidEdlRange { index: 0, .. })
    ));
}

#[test]
fn edl_primary_items_materialize_before_visual_layer_actions() {
    let project = empty_timeline_project();
    let mut proposal = edl_proposal(
        vec![clip("media-1", 2.0, 5.0), clip("media-1", 8.0, 9.0)],
        4.0,
    );
    proposal.project_actions = vec![ProjectAction::AddItems {
        target_track_id: "track-captions".to_string(),
        items: vec![TimelineItem {
            id: "caption-1".to_string(),
            kind: TimelineItemKind::Caption,
            start_seconds: 0.0,
            duration_seconds: 1.5,
            source: TimelineSource::Text {
                text: "Hello".to_string(),
            },
            label: "Hello".to_string(),
            properties: BTreeMap::new(),
        }],
    }];

    let prepared =
        prepare_codex_conversation_proposal(&project, &proposal).expect("EDL cut validates");

    assert_eq!(prepared.actions.len(), 3);
    let ProjectAction::AddItems {
        target_track_id,
        items,
    } = &prepared.actions[0]
    else {
        panic!("first action should add EDL video items");
    };
    assert_eq!(target_track_id, "track-video");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].start_seconds, 0.0);
    assert_eq!(items[0].duration_seconds, 3.0);
    assert_eq!(items[1].start_seconds, 3.0);
    assert_eq!(items[0].properties["sourceIn"], json!(2.0));
    let ProjectAction::AddItems {
        target_track_id, ..
    } = &prepared.actions[1]
    else {
        panic!("second action should add EDL audio items");
    };
    assert_eq!(target_track_id, "track-audio");
    let ProjectAction::AddItems {
        target_track_id, ..
    } = &prepared.actions[2]
    else {
        panic!("visual layer follows the EDL");
    };
    assert_eq!(target_track_id, "track-captions");
}

#[test]
fn edl_or_new_visual_layer_requires_render_review() {
    let project = empty_timeline_project();
    let mut proposal = edl_proposal(vec![clip("media-1", 0.0, 3.0)], 3.0);
    proposal.render_review = None;
    assert_eq!(
        prepare_codex_conversation_proposal(&project, &proposal),
        Err(CodexConversationError::MissingRenderReview)
    );

    let project = conversation_fixture_project();
    let proposal = CodexConversationEditProposal {
        summary: "Added a title.".to_string(),
        edl: Vec::new(),
        project_actions: vec![ProjectAction::AddItems {
            target_track_id: "track-overlays".to_string(),
            items: vec![TimelineItem {
                id: "title-1".to_string(),
                kind: TimelineItemKind::Overlay,
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source: TimelineSource::Text {
                    text: "Chapter one".to_string(),
                },
                label: "Chapter one".to_string(),
                properties: BTreeMap::new(),
            }],
        }],
        render_review: None,
    };
    assert_eq!(
        prepare_codex_conversation_proposal(&project, &proposal),
        Err(CodexConversationError::MissingRenderReview)
    );
}

#[test]
fn proposal_actions_must_apply_to_a_cloned_project() {
    let project = conversation_fixture_project();
    let proposal = CodexConversationEditProposal {
        summary: "Lowered a missing clip.".to_string(),
        edl: Vec::new(),
        project_actions: vec![ProjectAction::UpdateAudioVolume {
            item_id: "audio-missing".to_string(),
            volume_db: Some(-2.0),
        }],
        render_review: None,
    };
    let error = prepare_codex_conversation_proposal(&project, &proposal)
        .expect_err("missing target is invalid");
    assert!(matches!(error, CodexConversationError::ProjectAction(_)));
    assert_eq!(error.validation_issue().path, "projectActions");
}

#[test]
fn request_validation_normalizes_focus_and_rejects_unknown_targets() {
    let project = conversation_fixture_project();
    let request = CodexConversationEditRequest {
        prompt: "  Remove dead air  ".to_string(),
        focus: CodexConversationFocus {
            primary_media_id: Some("media-1".to_string()),
            media_ids: vec!["media-1".to_string(), " media-1 ".to_string()],
            timeline_item_ids: vec![
                "video-1".to_string(),
                "audio-1".to_string(),
                "video-1".to_string(),
            ],
            timeline_range: Some(CodexConversationRange {
                start_seconds: 1.0,
                end_seconds: 4.0,
            }),
        },
        created_at: "2026-07-25T00:00:00Z".to_string(),
    };
    let normalized = request.validate(&project).expect("valid request");
    assert_eq!(normalized.prompt, "  Remove dead air  ");
    assert!(normalized.focus.media_ids.is_empty());
    assert_eq!(
        normalized.focus.timeline_item_ids,
        vec!["video-1", "audio-1"]
    );

    let mut empty = request.clone();
    empty.prompt = "   ".to_string();
    assert_eq!(
        empty.validate(&project),
        Err(CodexConversationError::EmptyPrompt)
    );

    let mut unknown_media = request.clone();
    unknown_media.focus.media_ids = vec!["media-9".to_string()];
    assert_eq!(
        unknown_media.validate(&project),
        Err(CodexConversationError::FocusMediaNotFound(
            "media-9".to_string()
        ))
    );

    let mut unknown_item = request.clone();
    unknown_item.focus.timeline_item_ids = vec!["clip-9".to_string()];
    assert_eq!(
        unknown_item.validate(&project),
        Err(CodexConversationError::FocusTimelineItemNotFound(
            "clip-9".to_string()
        ))
    );

    for (start, end) in [(-1.0, 2.0), (3.0, 2.0), (0.0, f64::NAN), (0.0, 60.0)] {
        let mut bad_range = request.clone();
        bad_range.focus.timeline_range = Some(CodexConversationRange {
            start_seconds: start,
            end_seconds: end,
        });
        assert!(
            matches!(
                bad_range.validate(&project),
                Err(CodexConversationError::InvalidTimelineRange(_))
            ),
            "range {start}..{end} should be rejected"
        );
    }
}
