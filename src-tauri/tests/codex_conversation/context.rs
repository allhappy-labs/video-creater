use super::support::{
    conversation_fixture_project, sample_conversation_request, sample_skill_bundle, FakeTransport,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use video_creater_lib::codex::app_server::{
    build_codex_conversation_turn_request, start_codex_conversation_turn,
};
use video_creater_lib::codex::context::{
    build_codex_conversation_context, CodexConversationContext,
};
use video_creater_lib::codex::conversation::{
    CodexConversationEditRequest, CodexConversationFocus, CodexConversationTurnResult,
};
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaAsset, MediaFolder,
    MediaKind, ProjectTemplateOverride, TimelineItem, TimelineItemKind, TimelineSource,
    TimelineTrack, TrackKind, Transcript, TranscriptWord, VideoProject,
};
use video_creater_lib::project::split::{load_app_server_conversation_history, save_split_project};
use video_creater_lib::settings::skills::MANDATORY_SKILLS;

const CONTEXT_PAYLOAD_MARKERS: [&str; 6] = [
    "Internal edit focus",
    "Project library:",
    "Project timeline:",
    "Transcript excerpts:",
    "truncated]",
    "- context:",
];

#[test]
fn codex_conversation_context_includes_every_entry_for_small_projects() {
    let mut project = conversation_fixture_project();
    project.media.push(media_asset("media-2", None));
    project.transcripts.push(transcript("media-1"));
    project.transcripts.push(transcript("media-2"));
    let request = CodexConversationEditRequest {
        prompt: "Tighten @Second.".to_string(),
        focus: CodexConversationFocus {
            media_ids: vec!["media-2".to_string()],
            ..CodexConversationFocus::default()
        },
        created_at: "2026-07-25T00:00:00Z".to_string(),
    };

    let context = build_codex_conversation_context(&project, &request, None);

    for summary in all_summaries(&context) {
        assert!(!summary.contains("truncated]"), "{summary}");
    }
    assert!(context
        .internal_focus_summary
        .contains("- context: complete project context"));
    for needle in ["- media media-1:", "- media media-2:"] {
        assert!(context.media_library_summary.contains(needle), "{needle}");
    }
    for needle in ["- item video-1:", "- item audio-1:"] {
        assert!(context.timeline_summary.contains(needle), "{needle}");
    }
    // Complete context keeps canonical project order; focus only ranks when
    // a collection has to be bounded.
    assert!(
        position(&context.media_library_summary, "- media media-1:")
            < position(&context.media_library_summary, "- media media-2:")
    );
    assert!(
        position(&context.transcript_excerpts_summary, "- media media-1:")
            < position(&context.transcript_excerpts_summary, "- media media-2:")
    );
}

#[test]
fn codex_conversation_context_ranks_focus_first_when_over_caps() {
    let project = oversized_project();
    let request = oversized_request(vec!["note-1".to_string(), "clip-140".to_string()]);

    let context = build_codex_conversation_context(&project, &request, None);

    assert!(context
        .internal_focus_summary
        .contains("- context: focus-first bounded subset"));

    let media = &context.media_library_summary;
    assert_eq!(
        first_line_with_prefix(media, "- media "),
        Some("- media media-095"),
        "explicitly mentioned media ranks first:\n{media}"
    );
    // Media referenced by focused timeline items outrank the rest of the timeline.
    assert!(position(media, "- media media-060:") < position(media, "- media media-000:"));
    assert!(position(media, "- media media-090:") < position(media, "- media media-000:"));
    // Mentions influence ranking without becoming the only context.
    assert!(media.contains("- media media-000:"));
    assert!(!media.contains("- media media-099:"));
    assert!(media.contains("- [20 media assets truncated]"));
    assert_eq!(
        first_line_with_prefix(media, "- folder "),
        Some("- folder folder-44")
    );
    assert!(media.contains("- [5 folders truncated]"));

    let timeline = &context.timeline_summary;
    assert!(
        position(timeline, "- track track-extra-08:") < position(timeline, "- track track-scenes:")
    );
    assert!(position(timeline, "- item clip-140:") < position(timeline, "- item clip-000:"));
    assert!(timeline.contains("- item note-1:"));
    assert!(timeline.contains("- [2 tracks truncated]"));
    assert!(timeline.contains("- [31 timeline items truncated]"));

    let transcripts = &context.transcript_excerpts_summary;
    assert!(
        position(transcripts, "- media media-095:") < position(transcripts, "- media media-000:")
    );
    assert!(transcripts.contains("- [2 transcripts truncated]"));

    let generated = &context.generated_assets_summary;
    assert_eq!(
        first_line_with_prefix(generated, "- asset "),
        Some("- asset generated-084")
    );
    assert!(generated.contains("- [5 generated assets truncated]"));

    let templates = &context.template_overrides_summary;
    assert_eq!(
        first_line_with_prefix(templates, "- template "),
        Some("- template template-084")
    );
    assert!(templates.contains("- [5 template overrides truncated]"));

    let jobs = &context.workflow_jobs_summary;
    assert_eq!(first_line_with_prefix(jobs, "- job "), Some("- job job-44"));
    assert!(!jobs.contains("- job job-00:"));
    assert!(jobs.contains("- [5 workflow jobs truncated]"));
}

#[test]
fn codex_conversation_context_ranks_selection_chip_items_first() {
    let project = oversized_project();
    // The editor's selection chip sends `selectedItemIds` as timelineItemIds.
    let selected_item_ids = vec!["clip-130".to_string(), "clip-007".to_string()];
    let request = CodexConversationEditRequest {
        prompt: "Make this part punchier.".to_string(),
        focus: CodexConversationFocus {
            timeline_item_ids: selected_item_ids,
            ..CodexConversationFocus::default()
        },
        created_at: "2026-09-15T00:00:00Z".to_string(),
    };

    let context = build_codex_conversation_context(&project, &request, None);

    let item_lines = context
        .timeline_summary
        .lines()
        .filter(|line| line.starts_with("- item "))
        .take(2)
        .collect::<Vec<_>>();
    assert!(
        item_lines[0].starts_with("- item clip-007:"),
        "{item_lines:?}"
    );
    assert!(
        item_lines[1].starts_with("- item clip-130:"),
        "{item_lines:?}"
    );
    assert_eq!(
        first_line_with_prefix(&context.timeline_summary, "- track "),
        Some("- track track-video")
    );
}

#[test]
fn codex_conversation_presentation_result_contains_no_context_payload() {
    let dir = tempfile::tempdir().expect("project parent");
    let project_dir = dir.path().join("project");
    let mut project = oversized_project();
    project.schema_version = 2;
    project.codex_thread_id = Some("thread-1".to_string());
    save_split_project(&project_dir, &project).expect("save project");
    let request = CodexConversationEditRequest {
        prompt: "Make this part punchier.".to_string(),
        focus: CodexConversationFocus {
            timeline_item_ids: vec!["clip-130".to_string()],
            ..CodexConversationFocus::default()
        },
        created_at: "2026-09-15T00:00:00Z".to_string(),
    };
    let prior_turn_text = "Earlier request\n\nInternal edit focus:\n- timelineItemIds: clip-007\n\nProject library:\n- [20 media assets truncated]";
    let mut transport = FakeTransport::echoing_user_input(
        json!({
            "summary": "Lowered the late clip by 2 dB.",
            "edl": [],
            "projectActions": [{ "type": "updateAudioVolume", "itemId": "clip-130", "volumeDb": -2.0 }],
            "renderReview": null,
        }),
        prior_turn_text,
    );

    let result = start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        request,
        &sample_skill_bundle(),
        Some(&project_dir),
    )
    .expect("conversation turn");

    let turn_text = transport
        .requests
        .iter()
        .find(|request| request["method"] == "turn/start")
        .and_then(|request| request.pointer("/params/input/0/text"))
        .and_then(Value::as_str)
        .expect("turn text")
        .to_string();
    assert!(
        turn_text.contains("Internal edit focus"),
        "model still gets context"
    );

    // Exhaustive destructuring: adding a context field to the turn result
    // fails to compile here.
    let CodexConversationTurnResult {
        thread_id,
        thread_response,
        turn_response,
        proposal,
        prepared_proposal,
        proposal_validation_issues,
        model_notice: _,
    } = result;
    // Mirrors the fields of main.rs `CodexConversationEditCommandResult`.
    let presentation = serde_json::to_string(&json!({
        "project": project,
        "threadId": thread_id,
        "threadResponse": thread_response,
        "turnResponse": turn_response,
        "proposal": proposal,
        "preparedProposal": prepared_proposal,
        "proposalValidationIssues": proposal_validation_issues,
    }))
    .expect("serialize presentation");
    for marker in CONTEXT_PAYLOAD_MARKERS {
        assert!(
            !presentation.contains(marker),
            "presentation leaked context marker {marker:?}: {presentation}"
        );
    }
    assert!(presentation.contains("Lowered the late clip by 2 dB."));

    let history = serde_json::to_string(
        &load_app_server_conversation_history(&project_dir)
            .expect("history")
            .entries,
    )
    .expect("serialize history");
    for marker in CONTEXT_PAYLOAD_MARKERS {
        assert!(!history.contains(marker), "history leaked {marker:?}");
    }
}

#[test]
fn codex_conversation_context_keeps_truncation_markers_and_mandatory_guidance_in_model_prompt() {
    let mut project = oversized_project();
    let request = oversized_request(vec!["clip-140".to_string()]);
    let context = build_codex_conversation_context(&project, &request, None);

    let turn = build_codex_conversation_turn_request(3, "thread-1", &context);
    let text = turn["params"]["input"][0]["text"]
        .as_str()
        .expect("turn text");
    for marker in [
        "Internal edit focus:",
        "- context: focus-first bounded subset",
        "- [20 media assets truncated]",
        "- [31 timeline items truncated]",
        "- [2 tracks truncated]",
        "Use video-creater-video-pipeline first.",
        "Use video-creater-graphics for captions",
    ] {
        assert!(text.contains(marker), "prompt is missing {marker:?}");
    }

    let mut transport = FakeTransport::new(json!({
        "summary": "Lowered the late clip by 2 dB.",
        "edl": [],
        "projectActions": [{ "type": "updateAudioVolume", "itemId": "clip-140", "volumeDb": -2.0 }],
        "renderReview": null,
    }));
    start_codex_conversation_turn(
        &mut transport,
        1,
        "/tmp/video-creater",
        &mut project,
        sample_conversation_request_for("clip-140"),
        &sample_skill_bundle(),
        None,
    )
    .expect("conversation turn");
    let thread_request = transport
        .requests
        .iter()
        .find(|request| request["method"] == "thread/start")
        .expect("thread request");
    let instructions = thread_request["params"]["developerInstructions"]
        .as_str()
        .expect("developer instructions");
    for definition in MANDATORY_SKILLS {
        assert!(instructions.contains(&format!("### {}", definition.id)));
        assert!(instructions.contains(definition.bundled_content));
    }
}

fn sample_conversation_request_for(item_id: &str) -> CodexConversationEditRequest {
    let mut request = sample_conversation_request();
    request.focus.timeline_item_ids = vec![item_id.to_string()];
    request
}

fn oversized_request(timeline_item_ids: Vec<String>) -> CodexConversationEditRequest {
    CodexConversationEditRequest {
        prompt: "Tighten @Take-95 around the selected clips.".to_string(),
        focus: CodexConversationFocus {
            primary_media_id: None,
            media_ids: vec!["media-095".to_string()],
            timeline_item_ids,
            timeline_range: None,
        },
        created_at: "2026-07-25T00:00:00Z".to_string(),
    }
}

fn all_summaries(context: &CodexConversationContext) -> [&str; 10] {
    [
        &context.media_library_summary,
        &context.timeline_summary,
        &context.transcript_excerpts_summary,
        &context.generated_assets_summary,
        &context.template_overrides_summary,
        &context.render_reports_summary,
        &context.workflow_jobs_summary,
        &context.export_artifacts_summary,
        &context.export_capabilities_summary,
        &context.project_files_summary,
    ]
}

fn position(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("missing {needle:?} in:\n{haystack}"))
}

fn first_line_with_prefix<'a>(summary: &'a str, prefix: &str) -> Option<&'a str> {
    summary
        .lines()
        .find(|line| line.starts_with(prefix))
        .and_then(|line| line.split(':').next())
}

fn media_asset(id: &str, folder_id: Option<&str>) -> MediaAsset {
    MediaAsset {
        id: id.to_string(),
        name: None,
        relative_path: format!("media/{id}.mp4"),
        kind: MediaKind::Video,
        duration_seconds: 30.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: folder_id.map(str::to_string),
    }
}

fn transcript(media_id: &str) -> Transcript {
    Transcript {
        id: format!("transcript-{media_id}"),
        media_id: media_id.to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "hello".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.4,
            confidence: None,
            speaker: None,
        }],
    }
}

fn clip_item(id: &str, media_id: &str, start: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: start,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: id.to_string(),
        properties: BTreeMap::new(),
    }
}

/// Every bounded collection exceeds its cap. The focused objects sit at the
/// end of canonical order so a plain prefix would drop them.
fn oversized_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-large".to_string(),
        "Large".to_string(),
        "2026-07-25T00:00:00Z".to_string(),
    );
    project.media_folders = (0..45)
        .map(|index| MediaFolder {
            id: format!("folder-{index:02}"),
            name: format!("Folder {index:02}"),
            parent_id: None,
        })
        .collect();
    project.media = (0..100)
        .map(|index| {
            let folder = if index == 95 {
                "folder-44"
            } else {
                "folder-00"
            };
            media_asset(&format!("media-{index:03}"), Some(folder))
        })
        .collect();
    project.transcripts = (0..9)
        .map(|index| transcript(&format!("media-{index:03}")))
        .chain(std::iter::once(transcript("media-095")))
        .collect();

    for index in 0..9 {
        project.timeline.tracks.push(TimelineTrack::empty(
            &format!("track-extra-{index:02}"),
            "Extra",
            TrackKind::Overlay,
        ));
    }
    let video = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-video")
        .expect("video track");
    video.items = (0..150)
        .map(|index| {
            clip_item(
                &format!("clip-{index:03}"),
                &format!("media-{:03}", index % 80),
                f64::from(index),
            )
        })
        .collect();
    // `clip-140` references media-060 through `index % 80`.
    let mut note = clip_item("note-1", "media-090", 0.0);
    note.properties
        .insert("templateId".to_string(), json!("template-084"));
    project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.id == "track-extra-08")
        .expect("extra track")
        .items
        .push(note);
    project.timeline.duration_seconds = 150.0;
    project.timelines[0].timeline = project.timeline.clone();

    project.generated_assets = (0..85).map(generated_asset).collect();
    project.template_overrides = (0..85)
        .map(|index| ProjectTemplateOverride {
            schema_version: 1,
            template_id: format!("template-{index:03}"),
            name: format!("Template {index:03}"),
            fields: BTreeMap::new(),
            style: BTreeMap::new(),
            visual_treatment: String::new(),
            motion: String::new(),
            safe_zone: String::new(),
            avoid: String::new(),
        })
        .collect();
    project.jobs = (0..45)
        .map(|index| JobSummary {
            id: format!("job-{index:02}"),
            kind: "generate_media".to_string(),
            status: JobStatus::Completed,
            updated_at: format!("2026-07-01T00:{index:02}:00Z"),
            workflow: None,
            start_request: None,
            provider_request: None,
            failure_reason: None,
            export_settings: None,
        })
        .collect();
    project
}

fn generated_asset(index: usize) -> GeneratedAsset {
    let id = format!("generated-{index:03}");
    // The last asset produced the mentioned media, so it ranks first.
    let output_media_id = if index == 84 {
        "media-095".to_string()
    } else {
        format!("{id}-output")
    };
    GeneratedAsset {
        schema_version: 1,
        id: id.clone(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: "shot".to_string(),
        model: GenerationModel {
            provider: "fixture".to_string(),
            id: "fixture-model".to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings: GeneratedAssetSettings::default(),
        outputs: vec![GeneratedAssetOutput {
            media_id: output_media_id,
            relative_path: format!("generated/{id}/output.mp4"),
            source_url: None,
            width: 1920,
            height: 1080,
            duration_seconds: 2.0,
            fps: 24.0,
        }],
        created_at: "2026-07-25T00:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}
