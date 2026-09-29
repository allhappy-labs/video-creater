use serde_json::json;
use std::collections::BTreeMap;

use video_creater_lib::project::action::{
    apply_project_action, ProjectAction, ProjectActionCaptionRepair, ProjectActionColorGrade,
    ProjectActionEffect, ProjectActionError, ProjectActionGeneratedAsset,
    ProjectActionGeneratedAssetOutput, ProjectActionGeneratedAssetReferences,
    ProjectActionGeneratedAssetSettings, ProjectActionGenerationModel, ProjectActionKeyframe,
    ProjectActionKeyframeProperty, ProjectActionMove, ProjectActionReorder,
    ProjectActionReplaceGeneratedOutput, ProjectActionResize, ProjectActionRippleDeleteRange,
    ProjectActionSplit, ProjectActionTemplateOverrideUpdate, ProjectActionTemplateUpdate,
    ProjectActionTextOverlayUpdate, ProjectActionTranscriptWordEdit, ProjectActionTrim,
    ProjectActionVisualCrop,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::*;
use video_creater_lib::workflows::{
    temporal_generate_media_start_request, temporal_job_summary, TemporalWorkflowKind,
};

#[path = "project_action/audio_edits.rs"]
mod audio_edits;
#[path = "project_action/export_records.rs"]
mod export_records;
#[path = "project_action/job_failure.rs"]
mod job_failure;
#[path = "project_action/job_status.rs"]
mod job_status;
#[path = "project_action/reverse.rs"]
mod reverse;
#[path = "project_action/transition_maintenance.rs"]
mod transition_maintenance;
#[path = "project_action/transitions.rs"]
mod transitions;

fn caption_item(id: &str, start_seconds: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds,
        duration_seconds: 1.0,
        source: TimelineSource::Text {
            text: "Caption".to_string(),
        },
        label: "Caption".to_string(),
        properties: BTreeMap::new(),
    }
}

fn text_overlay_item(id: &str, start_seconds: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds,
        duration_seconds: 3.0,
        source: TimelineSource::Text {
            text: "Text overlay".to_string(),
        },
        label: "Text overlay".to_string(),
        properties: BTreeMap::from([("text".to_string(), serde_json::json!("Text overlay"))]),
    }
}

fn find_timeline_item<'a>(project: &'a VideoProject, item_id: &str) -> Option<&'a TimelineItem> {
    project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == item_id)
}

fn video_item(id: &str, media_id: &str, start_seconds: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds,
        duration_seconds: 2.0,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: "Video".to_string(),
        properties: BTreeMap::new(),
    }
}

fn audio_item(id: &str, media_id: &str, duration_seconds: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: "Audio".to_string(),
        properties: BTreeMap::new(),
    }
}

fn generated_edit_video_item(id: &str, media_id: &str, start_seconds: f64) -> TimelineItem {
    let mut item = video_item(id, media_id, start_seconds);
    item.properties
        .insert("generatedEdit".to_string(), serde_json::json!(true));
    item
}

fn template_item(id: &str, start_seconds: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds,
        duration_seconds: 1.0,
        source: TimelineSource::Generated {
            artifact_id: format!("template:kinetic-lower-third-v1:{id}"),
        },
        label: "Kinetic Lower Third".to_string(),
        properties: BTreeMap::from([
            (
                "templateId".to_string(),
                serde_json::json!("kinetic-lower-third-v1"),
            ),
            (
                "templateFields".to_string(),
                serde_json::json!({
                    "headline": "Original",
                    "subline": "Role"
                }),
            ),
        ]),
    }
}

fn transcript_with_words() -> Transcript {
    Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "Video".to_string(),
                start_seconds: 0.0,
                end_seconds: 0.4,
                confidence: Some(0.95),
                speaker: None,
            },
            TranscriptWord {
                text: "Creater".to_string(),
                start_seconds: 0.5,
                end_seconds: 0.9,
                confidence: Some(0.88),
                speaker: None,
            },
        ],
    }
}

fn caption_repair_action() -> ProjectAction {
    ProjectAction::ApplyCaptionRepair {
        repair: ProjectActionCaptionRepair {
            caption_item_id: "caption-1".to_string(),
            transcript_id: "transcript-media-1".to_string(),
            word_index: 1,
            text: "Creator".to_string(),
            start_seconds: 1.5,
            end_seconds: 1.9,
            repair_id: "repair-caption-1".to_string(),
            created_at: "2026-06-22T10:00:00Z".to_string(),
        },
    }
}

fn generated_asset_action() -> ProjectAction {
    ProjectAction::RecordGeneratedAsset {
        asset: Box::new(ProjectActionGeneratedAsset {
            id: "generated-shot-1".to_string(),
            kind: MediaKind::Generated,
            status: GeneratedAssetStatus::Completed,
            name: Some("  Hero product reveal  ".to_string()),
            target_folder_id: None,
            placement_intent: None,
            prompt: "slow push-in on the product".to_string(),
            model: ProjectActionGenerationModel {
                provider: "seedance".to_string(),
                id: "seedance-2-fast".to_string(),
            },
            references: ProjectActionGeneratedAssetReferences {
                media_ids: vec!["media-1".to_string()],
                first_frame_media_id: Some("media-1".to_string()),
                last_frame_media_id: None,
                provider_input_urls: Vec::new(),
                ..Default::default()
            },
            settings: ProjectActionGeneratedAssetSettings {
                width: Some(1080),
                height: Some(1920),
                duration_seconds: Some(8.0),
                fps: Some(24.0),
                aspect_ratio: Some("9:16".to_string()),
                resolution: Some("1080p".to_string()),
                generate_audio: Some(false),
                timeline_start_seconds: Some(6.0),
                ..ProjectActionGeneratedAssetSettings::default()
            },
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-1-output".to_string(),
                relative_path: "generated/generated-shot-1/output.mp4".to_string(),
                source_url: Some("https://fal.media/generated/output.mp4".to_string()),
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            created_at: "2026-06-22T10:00:00Z".to_string(),
            parent_asset_id: None,
            retry_of_asset_id: None,
        }),
    }
}

fn temporal_render_job(job_id: &str) -> JobSummary {
    temporal_job_summary(
        TemporalWorkflowKind::RenderDraft,
        "project-1",
        job_id,
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    )
}

#[test]
fn record_generated_asset_preserves_generation_settings() {
    let mut project = sample_project();

    apply_project_action(&mut project, generated_asset_action()).expect("record generated asset");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.settings.width, Some(1080));
    assert_eq!(generated.settings.height, Some(1920));
    assert_eq!(generated.settings.duration_seconds, Some(8.0));
    assert_eq!(generated.settings.fps, Some(24.0));
    assert_eq!(generated.settings.aspect_ratio.as_deref(), Some("9:16"));
    assert_eq!(generated.settings.resolution.as_deref(), Some("1080p"));
    assert_eq!(generated.settings.generate_audio, Some(false));
    assert_eq!(generated.settings.timeline_start_seconds, Some(6.0));
}

#[test]
fn record_generated_asset_accepts_provider_auto_aspect_ratio() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.settings.aspect_ratio = Some("auto".to_string());
    }

    apply_project_action(&mut project, action).expect("record generated asset with auto aspect");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.settings.aspect_ratio.as_deref(), Some("auto"));
}

#[test]
fn record_generated_asset_preserves_timeline_placement_intent() {
    let mut project = sample_project();
    let mut action_json = serde_json::to_value(generated_asset_action()).expect("action json");
    action_json["asset"]["placementIntent"] = serde_json::json!("timeline");
    let action = serde_json::from_value(action_json).expect("deserialize action");

    apply_project_action(&mut project, action).expect("record generated asset");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    let generated_json = serde_json::to_value(generated).expect("generated asset json");
    assert_eq!(generated_json["placementIntent"], "timeline");
}

#[test]
fn record_job_action_appends_workflow_backed_job() {
    let mut project = sample_project();
    let job = temporal_render_job("render-draft-1");

    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(job.clone()),
        },
    )
    .expect("record workflow job");

    assert_eq!(project.jobs, vec![job]);
}

#[test]
fn record_job_action_preserves_temporal_start_request() {
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "generated-shot-1",
        "generated-shot-1",
        true,
        None,
    ));

    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(job.clone()),
        },
    )
    .expect("record workflow job with start request");

    assert_eq!(project.jobs, vec![job]);
    let value = serde_json::to_value(project.jobs.first().expect("job")).expect("job json");
    assert_eq!(
        value["startRequest"]["workflowType"],
        "VideoCreaterGenerateMediaWorkflow"
    );
    assert!(
        value["startRequest"]["input"]["providerCredentialEnvVar"].is_null(),
        "persisted replay requests must resolve credentials at execution time"
    );
}

#[test]
fn update_job_status_action_sets_status_timestamp_and_workflow_run_id() {
    let mut project = sample_project();
    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(temporal_render_job("render-draft-1")),
        },
    )
    .expect("record workflow job");

    apply_project_action(
        &mut project,
        ProjectAction::UpdateJobStatus {
            job_id: "render-draft-1".to_string(),
            status: JobStatus::Running,
            updated_at: "2026-06-23T12:01:00Z".to_string(),
            run_id: Some("temporal-run-1".to_string()),
        },
    )
    .expect("update workflow job");

    let job = project.jobs.first().expect("job");
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(job.updated_at, "2026-06-23T12:01:00Z");
    assert_eq!(
        job.workflow
            .as_ref()
            .and_then(|workflow| workflow.run_id.as_deref()),
        Some("temporal-run-1")
    );
}

#[test]
fn update_job_provider_request_action_persists_fal_queue_metadata() {
    let mut project = sample_project();
    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(temporal_render_job("generated-shot-1")),
        },
    )
    .expect("record workflow job");

    let provider_request = JobProviderRequest {
        provider: "fal.ai".to_string(),
        request_id: "fal-request-1".to_string(),
        status_url: "https://queue.fal.run/fal-ai/model/requests/fal-request-1/status".to_string(),
        response_url: "https://queue.fal.run/fal-ai/model/requests/fal-request-1".to_string(),
        cancel_url: "https://queue.fal.run/fal-ai/model/requests/fal-request-1/cancel".to_string(),
        submitted_at: "2026-06-23T12:01:00Z".to_string(),
    };

    apply_project_action(
        &mut project,
        ProjectAction::UpdateJobProviderRequest {
            job_id: "generated-shot-1".to_string(),
            provider_request: provider_request.clone(),
        },
    )
    .expect("update provider request");

    let job = project.jobs.first().expect("job");
    assert_eq!(job.provider_request.as_ref(), Some(&provider_request));
    let job_json = serde_json::to_value(job).expect("job json");
    assert_eq!(
        job_json["providerRequest"]["cancelUrl"],
        "https://queue.fal.run/fal-ai/model/requests/fal-request-1/cancel"
    );
    assert!(job_json.get("provider_request").is_none());
}

#[test]
fn update_job_status_rejects_late_completion_after_cancelled_job() {
    let mut project = sample_project();
    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(temporal_render_job("render-draft-1")),
        },
    )
    .expect("record workflow job");
    apply_project_action(
        &mut project,
        ProjectAction::UpdateJobStatus {
            job_id: "render-draft-1".to_string(),
            status: JobStatus::Cancelled,
            updated_at: "2026-06-23T12:01:00Z".to_string(),
            run_id: Some("temporal-run-1".to_string()),
        },
    )
    .expect("cancel render job");

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateJobStatus {
            job_id: "render-draft-1".to_string(),
            status: JobStatus::Completed,
            updated_at: "2026-06-23T12:02:00Z".to_string(),
            run_id: Some("temporal-run-1".to_string()),
        },
        ProjectActionError::TerminalJobStatus("render-draft-1".to_string()),
    );
}

#[test]
fn record_job_action_rejects_duplicate_job_ids() {
    let mut project = sample_project();
    apply_project_action(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(temporal_render_job("render-draft-1")),
        },
    )
    .expect("record workflow job");

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RecordJob {
            job: Box::new(temporal_render_job("render-draft-1")),
        },
        ProjectActionError::DuplicateJobId("render-draft-1".to_string()),
    );
}

#[test]
fn record_job_action_rejects_invalid_workflow_metadata() {
    let mut project = sample_project();
    let mut job = temporal_render_job("render-draft-1");
    let workflow = job.workflow.as_mut().expect("workflow metadata");
    workflow.activity_types.clear();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RecordJob { job: Box::new(job) },
        ProjectActionError::MissingJobWorkflowMetadata("activityTypes".to_string()),
    );
}

#[test]
fn record_job_action_rejects_temporal_start_request_mismatched_to_workflow() {
    let mut project = sample_project();
    let mut job = temporal_job_summary(
        TemporalWorkflowKind::GenerateMedia,
        "project-1",
        "generated-shot-1",
        JobStatus::Queued,
        "2026-06-23T12:00:00Z",
    );
    job.start_request = Some(temporal_generate_media_start_request(
        "project-1",
        "/tmp/video-creater/project-1",
        "other-generated-shot",
        "other-generated-shot",
        true,
        None,
    ));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RecordJob { job: Box::new(job) },
        ProjectActionError::MismatchedJobStartRequest("workflowId".to_string()),
    );
}

fn render_report() -> ProjectRenderReport {
    ProjectRenderReport {
        schema_version: 1,
        id: "render-draft-1".to_string(),
        status: RenderReportStatus::Completed,
        output_path: "renders/render-draft-1/output.mp4".to_string(),
        duration_seconds: 42.5,
        quality: None,
        requested_width: None,
        requested_height: None,
        actual_width: None,
        actual_height: None,
        streams: RenderReportStreams {
            video: true,
            audio: true,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Passed,
            ),
            ("overlayTiming".to_string(), RenderReportCheckStatus::Passed),
            (
                "visualFrameEvidence".to_string(),
                RenderReportCheckStatus::Passed,
            ),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts: vec!["renders/render-draft-1/output.mp4".to_string()],
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: "logs/render-draft-1.log".to_string(),
        created_at: "2026-06-22T10:00:00Z".to_string(),
    }
}

fn export_artifact() -> ProjectExportArtifact {
    ProjectExportArtifact {
        schema_version: 1,
        id: "nle-export-premiere-1".to_string(),
        kind: ProjectExportArtifactKind::NleXml,
        format: "premiereXmeml".to_string(),
        path: "exports/project-test-premiere.xml".to_string(),
        mime_type: "application/xml".to_string(),
        job_id: Some("nle-export-premiere-1".to_string()),
        created_at: "2026-06-23T12:00:00Z".to_string(),
    }
}

fn template_override_update() -> ProjectActionTemplateOverrideUpdate {
    ProjectActionTemplateOverrideUpdate {
        template_id: "kinetic-lower-third-v1".to_string(),
        name: "Kinetic Lower Third".to_string(),
        fields: BTreeMap::from([
            ("headline".to_string(), "Launch day".to_string()),
            (
                "subline".to_string(),
                "Built with Video Creater".to_string(),
            ),
        ]),
        style: BTreeMap::from([
            ("accentColor".to_string(), serde_json::json!("#22d3ee")),
            (
                "backgroundColor".to_string(),
                serde_json::json!("rgba(2, 6, 23, 0.72)"),
            ),
            ("textColor".to_string(), serde_json::json!("#ffffff")),
        ]),
        visual_treatment:
            "compact lower-third block with translucent backing, accent rule, and strong hierarchy"
                .to_string(),
        motion: "slide-and-fade in over 8 frames, hold, then soft fade out".to_string(),
        safe_zone: "keep essential text inside 10% margins and below face/action priority areas"
            .to_string(),
        avoid:
            "full-width opaque black slabs, centered title-card layout, default-font template look"
                .to_string(),
    }
}

fn assert_action_error_leaves_project_unchanged(
    project: &mut VideoProject,
    action: ProjectAction,
    expected_error: ProjectActionError,
) {
    let before = project.clone();

    let error = apply_project_action(project, action).expect_err("action must fail");

    assert_eq!(error, expected_error);
    assert_eq!(*project, before);
}

#[test]
fn project_action_uses_camel_case_wire_contract() {
    let action = ProjectAction::MoveItems {
        moves: vec![ProjectActionMove {
            item_id: "item-1".to_string(),
            target_track_id: "track-video".to_string(),
            start_seconds: 2.25,
        }],
    };

    let json = serde_json::to_value(&action).expect("serialize action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "moveItems",
            "moves": [
                {
                    "itemId": "item-1",
                    "targetTrackId": "track-video",
                    "startSeconds": 2.25
                }
            ]
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn audio_fades_use_atomic_camel_case_wire_contract() {
    let action = ProjectAction::UpdateAudioFades {
        item_id: "audio-1".to_string(),
        fade_in_seconds: 0.5,
        fade_out_seconds: 0.75,
    };

    let json = serde_json::to_value(&action).expect("serialize audio fades");
    assert_eq!(
        json,
        serde_json::json!({
            "type": "updateAudioFades",
            "itemId": "audio-1",
            "fadeInSeconds": 0.5,
            "fadeOutSeconds": 0.75
        })
    );
    assert_eq!(
        serde_json::from_value::<ProjectAction>(json).expect("deserialize audio fades"),
        action
    );
}

#[test]
fn audio_fades_update_both_edges_atomically_and_validate_duration() {
    let mut project = sample_project();
    let audio_track = project
        .timeline
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Audio)
        .expect("audio track");
    audio_track
        .items
        .push(audio_item("audio-1", "media-1", 4.0));

    apply_project_action(
        &mut project,
        ProjectAction::UpdateAudioFades {
            item_id: "audio-1".to_string(),
            fade_in_seconds: 0.5,
            fade_out_seconds: 1.25,
        },
    )
    .expect("apply audio fades");
    let item = find_timeline_item(&project, "audio-1").expect("audio item");
    assert_eq!(item.properties["fadeInSeconds"], json!(0.5));
    assert_eq!(item.properties["fadeOutSeconds"], json!(1.25));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateAudioFades {
            item_id: "audio-1".to_string(),
            fade_in_seconds: 2.5,
            fade_out_seconds: 2.0,
        },
        ProjectActionError::InvalidAudioFades("audio-1".to_string()),
    );
}

#[test]
fn project_action_accepts_track_state_wire_contract() {
    let action = ProjectAction::SetTrackEnabled {
        track_id: "track-video".to_string(),
        enabled: false,
    };

    let json = serde_json::to_value(&action).expect("serialize track state action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "setTrackEnabled",
            "trackId": "track-video",
            "enabled": false
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_job_status_wire_contract() {
    let action = ProjectAction::UpdateJobStatus {
        job_id: "render-draft-1".to_string(),
        status: JobStatus::Running,
        updated_at: "2026-06-23T12:01:00Z".to_string(),
        run_id: Some("temporal-run-1".to_string()),
    };

    let json = serde_json::to_value(&action).expect("serialize job status action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "updateJobStatus",
            "jobId": "render-draft-1",
            "status": "running",
            "updatedAt": "2026-06-23T12:01:00Z",
            "runId": "temporal-run-1"
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_export_artifact_wire_contract() {
    let action = ProjectAction::RecordExportArtifact {
        artifact: export_artifact(),
    };

    let json = serde_json::to_value(&action).expect("serialize export artifact action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "recordExportArtifact",
            "artifact": {
                "schemaVersion": 1,
                "id": "nle-export-premiere-1",
                "kind": "nle_xml",
                "format": "premiereXmeml",
                "path": "exports/project-test-premiere.xml",
                "mimeType": "application/xml",
                "jobId": "nle-export-premiere-1",
                "createdAt": "2026-06-23T12:00:00Z"
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn record_export_artifact_adds_it_to_project_state() {
    let mut project = sample_project();
    let artifact = export_artifact();

    apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact {
            artifact: artifact.clone(),
        },
    )
    .expect("record export artifact");

    assert_eq!(project.export_artifacts, vec![artifact]);
}

#[test]
fn record_export_artifact_rejects_paths_outside_exports_folder() {
    let mut project = sample_project();
    let before = project.clone();
    let mut artifact = export_artifact();
    artifact.id = "mp4-export-1".to_string();
    artifact.kind = ProjectExportArtifactKind::Mp4;
    artifact.format = "mp4H264".to_string();
    artifact.path = "renders/project-test.mp4".to_string();
    artifact.mime_type = "video/mp4".to_string();

    let error = apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact { artifact },
    )
    .expect_err("export artifacts outside exports/ must be rejected");

    assert!(error
        .to_string()
        .contains("export artifact path must stay under exports/"));
    assert_eq!(project, before);
}

#[test]
fn record_export_artifact_accepts_mp4_h264_contract() {
    let mut project = sample_project();
    let mut artifact = export_artifact();
    artifact.id = "mp4-export-h264-1".to_string();
    artifact.kind = ProjectExportArtifactKind::Mp4;
    artifact.format = "mp4H264".to_string();
    artifact.path = "exports/project-test-h264.mp4".to_string();
    artifact.mime_type = "video/mp4".to_string();

    apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact {
            artifact: artifact.clone(),
        },
    )
    .expect("MP4/H.264 export artifact should satisfy the reviewed contract");

    assert_eq!(project.export_artifacts, vec![artifact]);
}

#[test]
fn record_export_artifact_rejects_mp4_h264_with_non_mp4_path() {
    let mut project = sample_project();
    let before = project.clone();
    let mut artifact = export_artifact();
    artifact.id = "mp4-export-h264-1".to_string();
    artifact.kind = ProjectExportArtifactKind::Mp4;
    artifact.format = "mp4H264".to_string();
    artifact.path = "exports/project-test-h264.webm".to_string();
    artifact.mime_type = "video/mp4".to_string();

    let error = apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact { artifact },
    )
    .expect_err("MP4/H.264 artifacts must use an mp4 path");

    assert!(error.to_string().contains("mp4H264"));
    assert_eq!(project, before);
}

#[test]
fn record_export_artifact_accepts_mp4_h265_contract() {
    let mut project = sample_project();
    let mut artifact = export_artifact();
    artifact.id = "mp4-export-h265-1".to_string();
    artifact.kind = ProjectExportArtifactKind::Mp4;
    artifact.format = "mp4H265".to_string();
    artifact.path = "exports/project-test-h265.mp4".to_string();
    artifact.mime_type = "video/mp4".to_string();

    apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact {
            artifact: artifact.clone(),
        },
    )
    .expect("MP4/H.265 export artifact should satisfy the reviewed contract");

    assert_eq!(project.export_artifacts, vec![artifact]);
}

#[test]
fn record_export_artifact_rejects_mp4_h265_with_non_mp4_contract() {
    let mut project = sample_project();
    let before = project.clone();
    let mut artifact = export_artifact();
    artifact.id = "mp4-export-h265-1".to_string();
    artifact.kind = ProjectExportArtifactKind::Webm;
    artifact.format = "mp4H265".to_string();
    artifact.path = "exports/project-test-h265.webm".to_string();
    artifact.mime_type = "video/webm".to_string();

    let error = apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact { artifact },
    )
    .expect_err("MP4/H.265 artifacts must use the MP4 artifact contract");

    assert!(error.to_string().contains("mp4H265"));
    assert_eq!(project, before);
}

#[test]
fn record_export_artifact_accepts_prores_mov_wire_contract() {
    let mut project = sample_project();
    let json = serde_json::json!({
        "type": "recordExportArtifact",
        "artifact": {
            "schemaVersion": 1,
            "id": "prores-export-1",
            "kind": "mov",
            "format": "proResMov",
            "path": "exports/project-test-prores.mov",
            "mimeType": "video/quicktime",
            "jobId": "prores-export-1",
            "createdAt": "2026-06-24T12:00:00Z"
        }
    });
    let action: ProjectAction =
        serde_json::from_value(json).expect("deserialize ProRes MOV export artifact");

    apply_project_action(&mut project, action)
        .expect("ProRes MOV export artifact should satisfy the reviewed contract");

    assert_eq!(
        project.export_artifacts[0].kind,
        ProjectExportArtifactKind::Mov
    );
    assert_eq!(project.export_artifacts[0].format, "proResMov");
    assert_eq!(project.export_artifacts[0].mime_type, "video/quicktime");
}

#[test]
fn record_export_artifact_rejects_prores_mov_with_mp4_contract() {
    let mut project = sample_project();
    let before = project.clone();
    let mut artifact = export_artifact();
    artifact.id = "prores-export-1".to_string();
    artifact.kind = ProjectExportArtifactKind::Mp4;
    artifact.format = "proResMov".to_string();
    artifact.path = "exports/project-test-prores.mp4".to_string();
    artifact.mime_type = "video/mp4".to_string();

    let error = apply_project_action(
        &mut project,
        ProjectAction::RecordExportArtifact { artifact },
    )
    .expect_err("ProRes MOV artifacts must use the MOV artifact contract");

    assert!(error.to_string().contains("proResMov"));
    assert_eq!(project, before);
}

#[test]
fn record_export_artifact_accepts_palmier_project_bundle_contract() {
    let mut project = sample_project();
    let json = serde_json::json!({
        "type": "recordExportArtifact",
        "artifact": {
            "schemaVersion": 1,
            "id": "palmier-project-export-1",
            "kind": "project_bundle",
            "format": "palmierProject",
            "path": "exports/test-project.palmier",
            "mimeType": "application/vnd.video-creater.project",
            "jobId": "palmier-project-export-1",
            "createdAt": "2026-07-02T12:00:00Z"
        }
    });
    let action: ProjectAction =
        serde_json::from_value(json).expect("deserialize Palmier project bundle export artifact");

    apply_project_action(&mut project, action)
        .expect("Palmier project bundle export artifact should satisfy the package contract");

    assert_eq!(
        project.export_artifacts[0].kind,
        ProjectExportArtifactKind::ProjectBundle
    );
    assert_eq!(project.export_artifacts[0].format, "palmierProject");
    assert_eq!(
        project.export_artifacts[0].mime_type,
        "application/vnd.video-creater.project"
    );
}

#[test]
fn project_action_accepts_resize_and_caption_edit_wire_contract() {
    let resize = ProjectAction::ResizeItems {
        resizes: vec![ProjectActionResize {
            item_id: "item-1".to_string(),
            duration_seconds: 2.25,
        }],
    };
    let resize_json = serde_json::to_value(&resize).expect("serialize resize action");

    assert_eq!(
        resize_json,
        serde_json::json!({
            "type": "resizeItems",
            "resizes": [
                {
                    "itemId": "item-1",
                    "durationSeconds": 2.25
                }
            ]
        })
    );
    let decoded_resize: ProjectAction =
        serde_json::from_value(resize_json).expect("deserialize resize action");
    assert_eq!(decoded_resize, resize);

    let edit = ProjectAction::EditCaptionText {
        item_id: "caption-1".to_string(),
        text: "Corrected caption".to_string(),
    };
    let edit_json = serde_json::to_value(&edit).expect("serialize caption edit action");

    assert_eq!(
        edit_json,
        serde_json::json!({
            "type": "editCaptionText",
            "itemId": "caption-1",
            "text": "Corrected caption"
        })
    );
    let decoded_edit: ProjectAction =
        serde_json::from_value(edit_json).expect("deserialize caption edit action");
    assert_eq!(decoded_edit, edit);

    let text_edit = ProjectAction::EditTextItem {
        item_id: "overlay-1".to_string(),
        text: "Updated overlay".to_string(),
    };
    let text_edit_json = serde_json::to_value(&text_edit).expect("serialize text edit action");

    assert_eq!(
        text_edit_json,
        serde_json::json!({
            "type": "editTextItem",
            "itemId": "overlay-1",
            "text": "Updated overlay"
        })
    );
    let decoded_text_edit: ProjectAction =
        serde_json::from_value(text_edit_json).expect("deserialize text edit action");
    assert_eq!(decoded_text_edit, text_edit);

    let overlay_update = ProjectAction::UpdateTextOverlayItems {
        updates: vec![ProjectActionTextOverlayUpdate {
            item_id: "overlay-1".to_string(),
            start_seconds: 2.25,
            duration_seconds: 4.5,
            text: "Launch title".to_string(),
            visual_treatment: "bold upper-left title with transparent backing".to_string(),
            motion: "fade in quickly, hold, then drift upward".to_string(),
            safe_zone: "keep inside title safe margins".to_string(),
            avoid: "covering faces or using opaque full-width slabs".to_string(),
            font_name: None,
            font_size: None,
            color: None,
            alignment: None,
        }],
    };
    let overlay_update_json =
        serde_json::to_value(&overlay_update).expect("serialize text overlay update action");

    assert_eq!(
        overlay_update_json,
        serde_json::json!({
            "type": "updateTextOverlayItems",
            "updates": [
                {
                    "itemId": "overlay-1",
                    "startSeconds": 2.25,
                    "durationSeconds": 4.5,
                    "text": "Launch title",
                    "visualTreatment": "bold upper-left title with transparent backing",
                    "motion": "fade in quickly, hold, then drift upward",
                    "safeZone": "keep inside title safe margins",
                    "avoid": "covering faces or using opaque full-width slabs"
                }
            ]
        })
    );
    let decoded_overlay_update: ProjectAction = serde_json::from_value(overlay_update_json)
        .expect("deserialize text overlay update action");
    assert_eq!(decoded_overlay_update, overlay_update);
}

#[test]
fn project_action_accepts_caption_repair_wire_contract() {
    let action = caption_repair_action();

    let json = serde_json::to_value(&action).expect("serialize caption repair action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "applyCaptionRepair",
            "repair": {
                "captionItemId": "caption-1",
                "transcriptId": "transcript-media-1",
                "wordIndex": 1,
                "text": "Creator",
                "startSeconds": 1.5,
                "endSeconds": 1.9,
                "repairId": "repair-caption-1",
                "createdAt": "2026-06-22T10:00:00Z"
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_template_update_wire_contract() {
    let action = ProjectAction::UpdateTemplateItems {
        updates: vec![ProjectActionTemplateUpdate {
            item_id: "template-1".to_string(),
            start_seconds: 4.5,
            duration_seconds: 1.25,
            template_fields: BTreeMap::from([
                ("headline".to_string(), "Olha API".to_string()),
                ("subline".to_string(), "Founder".to_string()),
            ]),
        }],
    };

    let json = serde_json::to_value(&action).expect("serialize template update");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "updateTemplateItems",
            "updates": [
                {
                    "itemId": "template-1",
                    "startSeconds": 4.5,
                    "durationSeconds": 1.25,
                    "templateFields": {
                        "headline": "Olha API",
                        "subline": "Founder"
                    }
                }
            ]
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_template_override_wire_contract() {
    let action = ProjectAction::UpdateTemplateOverride {
        override_: template_override_update(),
    };

    let json = serde_json::to_value(&action).expect("serialize template override action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "updateTemplateOverride",
            "override": {
                "templateId": "kinetic-lower-third-v1",
                "name": "Kinetic Lower Third",
                "fields": {
                    "headline": "Launch day",
                    "subline": "Built with Video Creater"
                },
                "style": {
                    "accentColor": "#22d3ee",
                    "backgroundColor": "rgba(2, 6, 23, 0.72)",
                    "textColor": "#ffffff"
                },
                "visualTreatment": "compact lower-third block with translucent backing, accent rule, and strong hierarchy",
                "motion": "slide-and-fade in over 8 frames, hold, then soft fade out",
                "safeZone": "keep essential text inside 10% margins and below face/action priority areas",
                "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look"
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_split_item_wire_contract() {
    let action = ProjectAction::SplitItems {
        splits: vec![ProjectActionSplit {
            item_id: "item-1".to_string(),
            new_item_id: "item-1-b".to_string(),
            split_seconds: 1.75,
        }],
    };

    let json = serde_json::to_value(&action).expect("serialize split action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "splitItems",
            "splits": [
                {
                    "itemId": "item-1",
                    "newItemId": "item-1-b",
                    "splitSeconds": 1.75
                }
            ]
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_trim_item_wire_contract() {
    let action = ProjectAction::TrimItems {
        trims: vec![ProjectActionTrim {
            item_id: "item-1".to_string(),
            start_seconds: 1.0,
            duration_seconds: 3.0,
            source_in: Some(2.0),
            source_out: Some(5.0),
        }],
    };

    let json = serde_json::to_value(&action).expect("serialize trim action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "trimItems",
            "trims": [
                {
                    "itemId": "item-1",
                    "startSeconds": 1.0,
                    "durationSeconds": 3.0,
                    "sourceIn": 2.0,
                    "sourceOut": 5.0
                }
            ]
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_insert_items_wire_contract() {
    let action = ProjectAction::InsertItems {
        target_track_id: "track-video".to_string(),
        insert_seconds: 1.5,
        items: vec![video_item("item-inserted", "media-1", 0.0)],
    };

    let json = serde_json::to_value(&action).expect("serialize insert action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "insertItems",
            "targetTrackId": "track-video",
            "insertSeconds": 1.5,
            "items": [
                {
                    "id": "item-inserted",
                    "kind": "video_clip",
                    "startSeconds": 0.0,
                    "durationSeconds": 2.0,
                    "source": {
                        "type": "media",
                        "mediaId": "media-1"
                    },
                    "label": "Video",
                    "properties": {}
                }
            ]
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_reorder_items_wire_contract() {
    let action = ProjectAction::ReorderItems {
        reorder: ProjectActionReorder {
            target_track_id: "track-video".to_string(),
            item_ids: vec!["item-b".to_string(), "item-a".to_string()],
            start_seconds: 1.25,
            gap_seconds: 0.2,
        },
    };

    let json = serde_json::to_value(&action).expect("serialize reorder action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "reorderItems",
            "reorder": {
                "targetTrackId": "track-video",
                "itemIds": ["item-b", "item-a"],
                "startSeconds": 1.25,
                "gapSeconds": 0.2
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_transcript_word_edit_wire_contract() {
    let action = ProjectAction::EditTranscriptWords {
        edits: vec![ProjectActionTranscriptWordEdit {
            transcript_id: "transcript-media-1".to_string(),
            word_index: 1,
            text: Some("creator".to_string()),
            start_seconds: Some(0.55),
            end_seconds: Some(0.95),
            repair_id: "repair-1".to_string(),
            created_at: "2026-06-22T10:00:00Z".to_string(),
        }],
    };

    let json = serde_json::to_value(&action).expect("serialize transcript edit");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "editTranscriptWords",
            "edits": [
                {
                    "transcriptId": "transcript-media-1",
                    "wordIndex": 1,
                    "text": "creator",
                    "startSeconds": 0.55,
                    "endSeconds": 0.95,
                    "repairId": "repair-1",
                    "createdAt": "2026-06-22T10:00:00Z"
                }
            ]
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_generated_asset_wire_contract() {
    let action = generated_asset_action();

    let json = serde_json::to_value(&action).expect("serialize generated asset action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "recordGeneratedAsset",
            "asset": {
                "id": "generated-shot-1",
                "kind": "generated",
                "status": "completed",
                "name": "  Hero product reveal  ",
                "prompt": "slow push-in on the product",
                "model": {
                    "provider": "seedance",
                    "id": "seedance-2-fast"
                },
                "references": {
                    "mediaIds": ["media-1"],
                    "firstFrameMediaId": "media-1",
                    "lastFrameMediaId": null
                },
                "settings": {
                    "width": 1080,
                    "height": 1920,
                    "durationSeconds": 8.0,
                    "fps": 24.0,
                    "aspectRatio": "9:16",
                    "resolution": "1080p",
                    "timelineStartSeconds": 6.0,
                    "generateAudio": false
                },
                "outputs": [
                    {
                        "mediaId": "generated-shot-1-output",
                        "relativePath": "generated/generated-shot-1/output.mp4",
                        "sourceUrl": "https://fal.media/generated/output.mp4",
                        "width": 1280,
                        "height": 720,
                        "durationSeconds": 4.0,
                        "fps": 24.0
                    }
                ],
                "createdAt": "2026-06-22T10:00:00Z",
                "parentAssetId": null,
                "retryOfAssetId": null
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_generated_asset_status_wire_contract() {
    let action = ProjectAction::UpdateGeneratedAssetStatus {
        asset_id: "generated-shot-1".to_string(),
        status: GeneratedAssetStatus::Running,
    };

    let json = serde_json::to_value(&action).expect("serialize generated asset status action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "updateGeneratedAssetStatus",
            "assetId": "generated-shot-1",
            "status": "running"
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_generated_asset_references_wire_contract() {
    let action = ProjectAction::UpdateGeneratedAssetReferences {
        asset_id: "generated-shot-1".to_string(),
        references: ProjectActionGeneratedAssetReferences {
            media_ids: vec![
                "media-1".to_string(),
                "media-video-ref".to_string(),
                "media-audio-ref".to_string(),
            ],
            source_video_media_ref: Some("media-1".to_string()),
            first_frame_media_id: Some("media-1".to_string()),
            last_frame_media_id: None,
            reference_image_media_refs: vec!["media-1".to_string()],
            reference_video_media_refs: vec!["media-video-ref".to_string()],
            reference_audio_media_refs: vec!["media-audio-ref".to_string()],
            provider_input_urls: vec!["https://fal.media/uploads/source.png".to_string()],
        },
    };

    let json = serde_json::to_value(&action).expect("serialize generated asset references action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "updateGeneratedAssetReferences",
            "assetId": "generated-shot-1",
            "references": {
                "mediaIds": ["media-1", "media-video-ref", "media-audio-ref"],
                "sourceVideoMediaRef": "media-1",
                "firstFrameMediaId": "media-1",
                "lastFrameMediaId": null,
                "referenceImageMediaRefs": ["media-1"],
                "referenceVideoMediaRefs": ["media-video-ref"],
                "referenceAudioMediaRefs": ["media-audio-ref"],
                "providerInputUrls": ["https://fal.media/uploads/source.png"]
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_generated_output_replacement_wire_contract() {
    let action = ProjectAction::ReplaceTimelineItemWithGeneratedOutput {
        replacement: ProjectActionReplaceGeneratedOutput {
            item_id: "generated-clip-1".to_string(),
            media_id: "generated-shot-1-output".to_string(),
        },
    };

    let json =
        serde_json::to_value(&action).expect("serialize generated output replacement action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "replaceTimelineItemWithGeneratedOutput",
            "replacement": {
                "itemId": "generated-clip-1",
                "mediaId": "generated-shot-1-output"
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_complete_generated_asset_replacement_wire_contract() {
    let action = ProjectAction::CompleteGeneratedAsset {
        asset_id: "generated-shot-1".to_string(),
        outputs: vec![ProjectActionGeneratedAssetOutput {
            media_id: "generated-shot-1-output".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 4.0,
            fps: 24.0,
        }],
        completion: None,
        replacement: Some(ProjectActionReplaceGeneratedOutput {
            item_id: "item-1".to_string(),
            media_id: "generated-shot-1-output".to_string(),
        }),
    };

    let json = serde_json::to_value(&action)
        .expect("serialize complete generated asset replacement action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "completeGeneratedAsset",
            "assetId": "generated-shot-1",
            "outputs": [
                {
                    "mediaId": "generated-shot-1-output",
                    "relativePath": "generated/generated-shot-1/output.mp4",
                    "width": 1280,
                    "height": 720,
                    "durationSeconds": 4.0,
                    "fps": 24.0
                }
            ],
            "replacement": {
                "itemId": "item-1",
                "mediaId": "generated-shot-1-output"
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_assign_media_folder_wire_contract() {
    let action = ProjectAction::AssignMediaFolder {
        media_id: "media-1".to_string(),
        folder_id: Some("folder-generated".to_string()),
    };

    let json = serde_json::to_value(&action).expect("serialize assign media folder action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "assignMediaFolder",
            "mediaId": "media-1",
            "folderId": "folder-generated"
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn project_action_accepts_media_folder_management_wire_contract() {
    let actions = vec![
        (
            ProjectAction::CreateMediaFolder {
                folder: MediaFolder {
                    id: "folder-generated".to_string(),
                    name: "Generated selects".to_string(),
                    parent_id: None,
                },
            },
            serde_json::json!({
                "type": "createMediaFolder",
                "folder": {
                    "id": "folder-generated",
                    "name": "Generated selects",
                    "parentId": null
                }
            }),
        ),
        (
            ProjectAction::RenameMediaFolder {
                folder_id: "folder-generated".to_string(),
                name: "Final selects".to_string(),
            },
            serde_json::json!({
                "type": "renameMediaFolder",
                "folderId": "folder-generated",
                "name": "Final selects"
            }),
        ),
        (
            ProjectAction::DeleteMediaFolder {
                folder_id: "folder-generated".to_string(),
            },
            serde_json::json!({
                "type": "deleteMediaFolder",
                "folderId": "folder-generated"
            }),
        ),
    ];

    for (action, expected_json) in actions {
        let json = serde_json::to_value(&action).expect("serialize media folder action");
        assert_eq!(json, expected_json);

        let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
        assert_eq!(decoded, action);
    }
}

#[test]
fn project_action_accepts_media_and_settings_management_wire_contracts() {
    let actions = vec![
        (
            ProjectAction::RenameMedia {
                media_id: "media-1".to_string(),
                name: "Hero take".to_string(),
            },
            serde_json::json!({
                "type": "renameMedia",
                "mediaId": "media-1",
                "name": "Hero take"
            }),
        ),
        (
            ProjectAction::DeleteMedia {
                media_ids: vec!["unused-media".to_string()],
            },
            serde_json::json!({
                "type": "deleteMedia",
                "mediaIds": ["unused-media"]
            }),
        ),
        (
            ProjectAction::RemoveTracks {
                track_ids: vec!["track-overlay".to_string()],
            },
            serde_json::json!({
                "type": "removeTracks",
                "trackIds": ["track-overlay"]
            }),
        ),
        (
            ProjectAction::UpdateRenderSettings {
                settings: RenderSettings {
                    width: 1080,
                    height: 1920,
                    fps: 30.0,
                    loudness_lufs: -16.0,
                    captions: CaptionRenderMode::BurnIn,
                },
            },
            serde_json::json!({
                "type": "updateRenderSettings",
                "settings": {
                    "width": 1080,
                    "height": 1920,
                    "fps": 30.0,
                    "loudnessLufs": -16.0,
                    "captions": "burn_in"
                }
            }),
        ),
    ];

    for (action, expected_json) in actions {
        let json = serde_json::to_value(&action).expect("serialize media/settings action");
        assert_eq!(json, expected_json);

        let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
        assert_eq!(decoded, action);
    }
}

#[test]
fn project_action_accepts_visual_effect_and_color_wire_contracts() {
    let actions = vec![
        (
            ProjectAction::UpdateItemEffects {
                item_ids: vec!["item-1".to_string()],
                effects: vec![ProjectActionEffect {
                    effect_instance_id: String::new(),
                    effect_type: "stylize.glow".to_string(),
                    enabled: true,
                    params: BTreeMap::from([("intensity".to_string(), serde_json::json!(0.65))]),
                }],
            },
            serde_json::json!({
                "type": "updateItemEffects",
                "itemIds": ["item-1"],
                "effects": [
                    {
                        "effectInstanceId": "",
                        "effectType": "stylize.glow",
                        "enabled": true,
                        "params": { "intensity": 0.65 }
                    }
                ]
            }),
        ),
        (
            ProjectAction::UpdateItemColorGrade {
                item_ids: vec!["item-1".to_string()],
                reset: false,
                grade: ProjectActionColorGrade {
                    exposure: Some(0.2),
                    contrast: Some(1.12),
                    saturation: Some(0.9),
                    temperature: None,
                    tint: None,
                    extra: BTreeMap::new(),
                },
            },
            serde_json::json!({
                "type": "updateItemColorGrade",
                "itemIds": ["item-1"],
                "reset": false,
                "grade": {
                    "exposure": 0.2,
                    "contrast": 1.12,
                    "saturation": 0.9
                }
            }),
        ),
    ];

    for (action, expected_json) in actions {
        let json = serde_json::to_value(&action).expect("serialize effect/color action");
        assert_eq!(json, expected_json);

        let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
        assert_eq!(decoded, action);
    }
}

#[test]
fn project_action_accepts_render_report_wire_contract() {
    let action = ProjectAction::AttachRenderReport {
        report: render_report(),
    };

    let json = serde_json::to_value(&action).expect("serialize render report action");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "attachRenderReport",
            "report": {
                "schemaVersion": 1,
                "id": "render-draft-1",
                "status": "completed",
                "outputPath": "renders/render-draft-1/output.mp4",
                "durationSeconds": 42.5,
                "streams": {
                    "video": true,
                    "audio": true
                },
                "checks": {
                    "artifactPaths": "passed",
                    "captionAlignment": "passed",
                    "duration": "passed",
                    "logPath": "passed",
                    "streams": "passed",
                    "overlayTiming": "passed",
                    "visualFrameEvidence": "passed"
                },
                "artifacts": ["renders/render-draft-1/output.mp4"],
                "logPath": "logs/render-draft-1.log",
                "createdAt": "2026-06-22T10:00:00Z"
            }
        })
    );

    let decoded: ProjectAction = serde_json::from_value(json).expect("deserialize action");
    assert_eq!(decoded, action);
}

#[test]
fn add_items_appends_to_compatible_track_and_recalculates_duration() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-captions".to_string(),
            items: vec![caption_item("caption-new", 4.5)],
        },
    )
    .expect("add item");

    let captions = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.id == "track-captions")
        .expect("caption track");
    assert_eq!(captions.items[0].id, "caption-new");
    assert_eq!(project.timeline.duration_seconds, 5.5);
}

#[test]
fn add_items_overwrites_same_track_landing_region() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(0.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(4.0));

    let mut added = video_item("item-added", "media-1", 1.0);
    added.duration_seconds = 1.0;
    added
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(6.0));
    added
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(7.0));

    apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![added],
        },
    )
    .expect("overwrite add item");

    let items = &project.timeline.tracks[0].items;
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["item-1", "item-added", "item-1-overwrite-2000"]
    );
    assert_eq!(items[0].start_seconds, 0.0);
    assert_eq!(items[0].duration_seconds, 1.0);
    assert_eq!(items[0].properties["sourceIn"], serde_json::json!(0.0));
    assert_eq!(items[0].properties["sourceOut"], serde_json::json!(1.0));
    assert_eq!(items[1].start_seconds, 1.0);
    assert_eq!(items[1].duration_seconds, 1.0);
    assert_eq!(items[2].start_seconds, 2.0);
    assert_eq!(items[2].duration_seconds, 2.0);
    assert_eq!(items[2].properties["sourceIn"], serde_json::json!(2.0));
    assert_eq!(items[2].properties["sourceOut"], serde_json::json!(4.0));
    assert_eq!(project.timeline.duration_seconds, 4.0);
}

#[test]
fn add_items_rejects_duplicate_item_id_and_missing_media_reference() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![video_item("item-1", "media-1", 4.0)],
        },
        ProjectActionError::DuplicateItemId("item-1".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![video_item("item-new", "missing-media", 4.0)],
        },
        ProjectActionError::MissingMedia("missing-media".to_string()),
    );
}

#[test]
fn add_items_rejects_generated_edit_video_without_selected_source_range() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![generated_edit_video_item(
                "generated-edit-1",
                "media-1",
                4.0,
            )],
        },
        ProjectActionError::MissingGeneratedEditSourceRange("generated-edit-1".to_string()),
    );

    let mut full_source = generated_edit_video_item("generated-edit-1", "media-1", 4.0);
    full_source.duration_seconds = 12.0;
    full_source
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(0.0));
    full_source
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(12.0));
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![full_source],
        },
        ProjectActionError::GeneratedEditFullSourcePassThrough("generated-edit-1".to_string()),
    );
}

#[test]
fn add_items_accepts_generated_edit_source_range_scaled_by_clip_speed() {
    let mut project = sample_project();
    let mut fast = generated_edit_video_item("generated-edit-1", "media-1", 4.0);
    fast.properties
        .insert("sourceIn".to_string(), serde_json::json!(1.0));
    fast.properties
        .insert("sourceOut".to_string(), serde_json::json!(5.0));
    fast.properties
        .insert("speed".to_string(), serde_json::json!(2.0));

    apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![fast],
        },
    )
    .expect("add speed-adjusted generated edit");

    let item = find_timeline_item(&project, "generated-edit-1").expect("added item");
    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(item.properties["sourceOut"], serde_json::json!(5.0));
}

#[test]
fn add_items_rejects_generated_edit_source_range_that_ignores_clip_speed() {
    let mut project = sample_project();
    let mut fast = generated_edit_video_item("generated-edit-1", "media-1", 4.0);
    fast.properties
        .insert("sourceIn".to_string(), serde_json::json!(1.0));
    fast.properties
        .insert("sourceOut".to_string(), serde_json::json!(3.0));
    fast.properties
        .insert("speed".to_string(), serde_json::json!(2.0));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![fast],
        },
        ProjectActionError::SourceRangeDurationMismatch("generated-edit-1".to_string()),
    );
}

#[test]
fn add_items_rejects_track_type_mismatch_and_locked_track() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-captions".to_string(),
            items: vec![video_item("video-new", "media-1", 4.0)],
        },
        ProjectActionError::TrackTypeMismatch {
            item_kind: TimelineItemKind::VideoClip,
            track_kind: TrackKind::Caption,
        },
    );

    project.timeline.tracks[3].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-captions".to_string(),
            items: vec![caption_item("caption-new", 4.0)],
        },
        ProjectActionError::TrackLocked("track-captions".to_string()),
    );
}

#[test]
fn create_track_inserts_empty_track_after_target_track() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::CreateTrack {
            track: TimelineTrack::empty("track-video-2", "Video 2", TrackKind::Video),
            after_track_id: Some("track-video".to_string()),
        },
    )
    .expect("create track");

    assert_eq!(project.timeline.tracks[1].id, "track-video-2");
    assert_eq!(project.timeline.tracks[1].name, "Video 2");
    assert_eq!(project.timeline.tracks[1].kind, TrackKind::Video);
    assert!(project.timeline.tracks[1].items.is_empty());
}

#[test]
fn create_track_rejects_duplicate_track_id() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::CreateTrack {
            track: TimelineTrack::empty("track-video", "Video 2", TrackKind::Video),
            after_track_id: Some("track-video".to_string()),
        },
        ProjectActionError::DuplicateTrackId("track-video".to_string()),
    );
}

#[test]
fn timeline_library_actions_create_switch_and_rename_an_alternate_cut() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::CreateTimeline {
            timeline_id: "alt-cut".to_string(),
            name: "Alternate cut".to_string(),
            duplicate_active: true,
            source_timeline_id: None,
        },
    )
    .expect("create alternate cut");
    assert_eq!(project.active_timeline_id.as_deref(), Some("alt-cut"));
    assert_eq!(project.timelines.len(), 2);
    assert_eq!(project.timeline.tracks[0].items[0].id, "item-1");

    apply_project_action(
        &mut project,
        ProjectAction::RenameTimeline {
            timeline_id: "alt-cut".to_string(),
            name: "Social cut".to_string(),
        },
    )
    .expect("rename alternate cut");
    apply_project_action(
        &mut project,
        ProjectAction::SetActiveTimeline {
            timeline_id: "main".to_string(),
        },
    )
    .expect("switch to main timeline");

    assert_eq!(project.active_timeline_id.as_deref(), Some("main"));
    assert_eq!(
        project
            .timelines
            .iter()
            .find(|timeline| timeline.id == "alt-cut")
            .map(|timeline| timeline.name.as_str()),
        Some("Social cut")
    );
}

#[test]
fn timeline_library_actions_duplicate_a_background_timeline_and_delete_the_active_cut() {
    let mut project = sample_project();
    let main_timeline = project.timeline.clone();

    apply_project_action(
        &mut project,
        ProjectAction::CreateTimeline {
            timeline_id: "alternate".to_string(),
            name: "Alternate cut".to_string(),
            duplicate_active: false,
            source_timeline_id: None,
        },
    )
    .expect("create alternate timeline");
    let alternate_timeline = project.timeline.clone();
    apply_project_action(
        &mut project,
        ProjectAction::SetActiveTimeline {
            timeline_id: "main".to_string(),
        },
    )
    .expect("switch to main");
    apply_project_action(
        &mut project,
        ProjectAction::CreateTimeline {
            timeline_id: "alternate-copy".to_string(),
            name: "Copy of Alternate cut".to_string(),
            duplicate_active: true,
            source_timeline_id: Some("alternate".to_string()),
        },
    )
    .expect("duplicate background timeline");
    assert_eq!(project.timeline, alternate_timeline);
    assert_ne!(project.timeline, main_timeline);

    apply_project_action(
        &mut project,
        ProjectAction::DeleteTimeline {
            timeline_id: "alternate-copy".to_string(),
        },
    )
    .expect("delete active timeline");
    assert_eq!(project.active_timeline_id.as_deref(), Some("main"));
    assert_eq!(project.timeline, main_timeline);
    assert!(!project
        .timelines
        .iter()
        .any(|timeline| timeline.id == "alternate-copy"));
}

#[test]
fn timeline_library_actions_reject_deleting_the_last_or_a_nested_timeline() {
    let mut project = sample_project();
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::DeleteTimeline {
            timeline_id: "main".to_string(),
        },
        ProjectActionError::InvalidEffectParam(
            "a project must keep at least one timeline".to_string(),
        ),
    );

    apply_project_action(
        &mut project,
        ProjectAction::CreateTimeline {
            timeline_id: "alternate".to_string(),
            name: "Alternate cut".to_string(),
            duplicate_active: true,
            source_timeline_id: None,
        },
    )
    .expect("create alternate timeline");
    apply_project_action(
        &mut project,
        ProjectAction::SetActiveTimeline {
            timeline_id: "main".to_string(),
        },
    )
    .expect("switch to main");
    apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![TimelineItem {
                id: "main-nests-alternate".to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 4.0,
                duration_seconds: 1.0,
                source: TimelineSource::Timeline {
                    timeline_id: "alternate".to_string(),
                },
                label: "Alternate sequence".to_string(),
                properties: BTreeMap::new(),
            }],
        },
    )
    .expect("nest alternate in main");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::DeleteTimeline {
            timeline_id: "alternate".to_string(),
        },
        ProjectActionError::InvalidEffectParam(
            "timeline `alternate` is nested by timeline `main` and cannot be deleted".to_string(),
        ),
    );
}

#[test]
fn timeline_library_actions_reject_nested_timeline_cycles() {
    let mut project = sample_project();
    apply_project_action(
        &mut project,
        ProjectAction::CreateTimeline {
            timeline_id: "alternate".to_string(),
            name: "Alternate cut".to_string(),
            duplicate_active: true,
            source_timeline_id: None,
        },
    )
    .expect("create alternate timeline");
    apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![TimelineItem {
                id: "alternate-nests-main".to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 4.0,
                duration_seconds: 1.0,
                source: TimelineSource::Timeline {
                    timeline_id: "main".to_string(),
                },
                label: "Main sequence".to_string(),
                properties: BTreeMap::new(),
            }],
        },
    )
    .expect("alternate may nest main");
    apply_project_action(
        &mut project,
        ProjectAction::SetActiveTimeline {
            timeline_id: "main".to_string(),
        },
    )
    .expect("switch to main");

    let error = apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: vec![TimelineItem {
                id: "main-nests-alternate".to_string(),
                kind: TimelineItemKind::VideoClip,
                start_seconds: 4.0,
                duration_seconds: 1.0,
                source: TimelineSource::Timeline {
                    timeline_id: "alternate".to_string(),
                },
                label: "Alternate sequence".to_string(),
                properties: BTreeMap::new(),
            }],
        },
    )
    .expect_err("cycle should be rejected before mutation");

    assert!(error.to_string().contains("nested timeline cycle"));
}

#[test]
fn decompose_timeline_item_expands_clipped_child_tracks_without_losing_wrapper_properties() {
    let mut project = sample_project();
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), json!(0.0));
    alternate.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), json!(4.0));
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.start_seconds = 1.0;
    wrapper.duration_seconds = 3.0;
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties.clear();

    apply_project_action(
        &mut project,
        ProjectAction::DecomposeTimelineItem {
            item_id: "item-1".to_string(),
        },
    )
    .expect("decompose nested sequence");

    assert!(project.timeline.tracks[0].items.is_empty());
    let decomposed_track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.id.starts_with("decomposed-item-1-track-video"))
        .expect("decomposed video track");
    assert_eq!(decomposed_track.items.len(), 1);
    assert_eq!(decomposed_track.items[0].start_seconds, 1.0);
    assert_eq!(decomposed_track.items[0].duration_seconds, 3.0);
    assert_eq!(
        decomposed_track.items[0].properties.get("sourceOut"),
        Some(&json!(3.0))
    );
}

#[test]
fn decompose_timeline_item_rejects_styled_wrapper_to_preserve_its_semantics() {
    let mut project = sample_project();
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: project.timeline.clone(),
    });
    let wrapper = &mut project.timeline.tracks[0].items[0];
    wrapper.source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    wrapper.properties.insert("opacity".to_string(), json!(0.5));

    let error = apply_project_action(
        &mut project,
        ProjectAction::DecomposeTimelineItem {
            item_id: "item-1".to_string(),
        },
    )
    .expect_err("styled wrapper should not be flattened destructively");

    assert!(matches!(error, ProjectActionError::InvalidEffectParam(_)));
}

#[test]
fn remove_items_deletes_existing_items_and_recalculates_duration() {
    let mut project = sample_project();
    project.timeline.tracks[3]
        .items
        .push(caption_item("caption-new", 6.0));
    project.timeline.duration_seconds = 7.0;

    apply_project_action(
        &mut project,
        ProjectAction::RemoveItems {
            item_ids: vec!["caption-new".to_string()],
        },
    )
    .expect("remove item");

    assert!(project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .all(|item| item.id != "caption-new"));
    assert_eq!(project.timeline.duration_seconds, 4.0);
}

#[test]
fn link_and_unlink_items_persist_link_group_ids_through_canonical_actions() {
    let mut project = sample_project();
    project.timeline.tracks[0]
        .items
        .push(video_item("item-linked", "media-1", 4.0));

    apply_project_action(
        &mut project,
        ProjectAction::LinkItems {
            item_ids: vec!["item-1".to_string(), "item-linked".to_string()],
            link_group_id: "link-av-1".to_string(),
        },
    )
    .expect("link selected items");

    for item in &project.timeline.tracks[0].items {
        if ["item-1", "item-linked"].contains(&item.id.as_str()) {
            assert_eq!(
                item.properties["linkGroupId"],
                serde_json::json!("link-av-1")
            );
        }
    }

    apply_project_action(
        &mut project,
        ProjectAction::UnlinkItems {
            item_ids: vec!["item-1".to_string(), "item-linked".to_string()],
        },
    )
    .expect("unlink selected items");

    assert!(project.timeline.tracks[0].items.iter().all(|item| {
        if ["item-1", "item-linked"].contains(&item.id.as_str()) {
            !item.properties.contains_key("linkGroupId")
        } else {
            true
        }
    }));
}

#[test]
fn insert_items_places_new_items_and_ripples_later_track_items() {
    let mut project = sample_project();
    project.timeline.tracks[0]
        .items
        .push(video_item("item-later", "media-1", 5.0));
    project.timeline.duration_seconds = 7.0;

    apply_project_action(
        &mut project,
        ProjectAction::InsertItems {
            target_track_id: "track-video".to_string(),
            insert_seconds: 3.0,
            items: vec![
                video_item("item-inserted-a", "media-1", 0.0),
                video_item("item-inserted-b", "media-1", 0.0),
            ],
        },
    )
    .expect("insert items");

    let items = &project.timeline.tracks[0].items;
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "item-1",
            "item-inserted-a",
            "item-inserted-b",
            "item-1-overwrite-3000",
            "item-later",
        ]
    );
    assert_eq!(items[0].start_seconds, 0.0);
    assert_eq!(items[0].duration_seconds, 3.0);
    assert_eq!(items[1].start_seconds, 3.0);
    assert_eq!(items[2].start_seconds, 5.0);
    assert_eq!(items[3].start_seconds, 7.0);
    assert_eq!(items[3].duration_seconds, 1.0);
    assert_eq!(items[4].start_seconds, 9.0);
    assert_eq!(project.timeline.duration_seconds, 11.0);
}

#[test]
fn ripple_delete_ranges_splits_source_clip_and_closes_gap() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::RippleDeleteRanges {
            ranges: vec![ProjectActionRippleDeleteRange {
                start_seconds: 1.0,
                end_seconds: 1.5,
                track_ids: vec!["track-video".to_string()],
            }],
        },
    )
    .expect("ripple delete range");

    let items = &project.timeline.tracks[0].items;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "item-1");
    assert_eq!(items[0].start_seconds, 0.0);
    assert_eq!(items[0].duration_seconds, 1.0);
    assert_eq!(items[0].properties["sourceIn"], serde_json::json!(0.0));
    assert_eq!(items[0].properties["sourceOut"], serde_json::json!(1.0));
    assert_eq!(items[1].id, "item-1-ripple-1500");
    assert_eq!(items[1].start_seconds, 1.0);
    assert_eq!(items[1].duration_seconds, 2.5);
    assert_eq!(items[1].properties["sourceIn"], serde_json::json!(1.5));
    assert_eq!(items[1].properties["sourceOut"], serde_json::json!(4.0));
    assert_eq!(project.timeline.duration_seconds, 3.5);
}

#[test]
fn move_items_moves_to_compatible_track_and_sorts_items() {
    let mut project = sample_project();
    project.timeline.tracks.push(TimelineTrack::empty(
        "track-video-2",
        "Video 2",
        TrackKind::Video,
    ));

    apply_project_action(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![ProjectActionMove {
                item_id: "item-1".to_string(),
                target_track_id: "track-video-2".to_string(),
                start_seconds: 3.0,
            }],
        },
    )
    .expect("move item");

    assert!(project.timeline.tracks[0].items.is_empty());
    let moved = &project.timeline.tracks[5].items[0];
    assert_eq!(moved.id, "item-1");
    assert_eq!(moved.start_seconds, 3.0);
    assert_eq!(project.timeline.duration_seconds, 7.0);
}

#[test]
fn visual_clip_kinds_deserialize_and_move_on_video_tracks() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "image-1".to_string(),
        name: Some("Still".to_string()),
        relative_path: "media/still.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1080),
        height: Some(1080),
        fps: None,
        folder_id: None,
    });
    project.media.push(MediaAsset {
        id: "lottie-1".to_string(),
        name: Some("Motion".to_string()),
        relative_path: "media/motion.lottie".to_string(),
        kind: MediaKind::Lottie,
        duration_seconds: 2.0,
        width: Some(1080),
        height: Some(1080),
        fps: Some(30.0),
        folder_id: None,
    });
    project.timeline.tracks.push(TimelineTrack::empty(
        "track-video-2",
        "Video 2",
        TrackKind::Video,
    ));

    let visual_items: Vec<TimelineItem> = serde_json::from_value(serde_json::json!([
        {
            "id": "image-clip-1",
            "kind": "image_clip",
            "startSeconds": 0.0,
            "durationSeconds": 2.0,
            "source": { "type": "media", "mediaId": "image-1" },
            "label": "Image beat",
            "properties": {}
        },
        {
            "id": "lottie-clip-1",
            "kind": "lottie_clip",
            "startSeconds": 2.25,
            "durationSeconds": 2.0,
            "source": { "type": "media", "mediaId": "lottie-1" },
            "label": "Lottie beat",
            "properties": {}
        },
        {
            "id": "generated-clip-1",
            "kind": "generated_clip",
            "startSeconds": 4.5,
            "durationSeconds": 2.0,
            "source": { "type": "generated", "artifactId": "generated-shot-1" },
            "label": "Generated beat",
            "properties": {}
        }
    ]))
    .expect("deserialize first-class visual clip kinds");

    apply_project_action(
        &mut project,
        ProjectAction::AddItems {
            target_track_id: "track-video".to_string(),
            items: visual_items,
        },
    )
    .expect("add first-class visual clip kinds to video track");

    apply_project_action(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![ProjectActionMove {
                item_id: "lottie-clip-1".to_string(),
                target_track_id: "track-video-2".to_string(),
                start_seconds: 3.0,
            }],
        },
    )
    .expect("move lottie visual clip to another video track");

    assert_eq!(
        find_timeline_item(&project, "image-clip-1")
            .expect("image clip")
            .kind,
        TimelineItemKind::ImageClip
    );
    assert_eq!(
        find_timeline_item(&project, "lottie-clip-1")
            .expect("lottie clip")
            .kind,
        TimelineItemKind::LottieClip
    );
    assert_eq!(
        find_timeline_item(&project, "generated-clip-1")
            .expect("generated clip")
            .kind,
        TimelineItemKind::GeneratedClip
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![ProjectActionMove {
                item_id: "image-clip-1".to_string(),
                target_track_id: "track-audio".to_string(),
                start_seconds: 1.0,
            }],
        },
        ProjectActionError::TrackTypeMismatch {
            item_kind: TimelineItemKind::ImageClip,
            track_kind: TrackKind::Audio,
        },
    );
}

#[test]
fn reorder_items_places_existing_items_sequentially_on_target_track() {
    let mut project = sample_project();
    project.timeline.tracks[0]
        .items
        .push(video_item("item-a", "media-1", 7.0));
    project.timeline.tracks[0]
        .items
        .push(video_item("item-b", "media-1", 5.0));
    project.timeline.duration_seconds = 9.0;

    apply_project_action(
        &mut project,
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-video".to_string(),
                item_ids: vec![
                    "item-b".to_string(),
                    "item-1".to_string(),
                    "item-a".to_string(),
                ],
                start_seconds: 1.0,
                gap_seconds: 0.25,
            },
        },
    )
    .expect("reorder items");

    let items = &project.timeline.tracks[0].items;
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["item-b", "item-1", "item-a"]
    );
    assert_eq!(items[0].start_seconds, 1.0);
    assert_eq!(items[1].start_seconds, 3.25);
    assert_eq!(items[2].start_seconds, 7.5);
    assert_eq!(project.timeline.duration_seconds, 9.5);
}

#[test]
fn resize_items_updates_duration_and_recalculates_project_duration() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id: "item-1".to_string(),
                duration_seconds: 6.5,
            }],
        },
    )
    .expect("resize item");

    assert_eq!(project.timeline.tracks[0].items[0].duration_seconds, 6.5);
    assert_eq!(project.timeline.duration_seconds, 6.5);
}

#[test]
fn split_items_splits_clip_and_preserves_media_source_offsets() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(2.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(6.0));

    apply_project_action(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "item-1".to_string(),
                new_item_id: "item-1-b".to_string(),
                split_seconds: 1.5,
            }],
        },
    )
    .expect("split item");

    let items = &project.timeline.tracks[0].items;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "item-1");
    assert_eq!(items[0].start_seconds, 0.0);
    assert_eq!(items[0].duration_seconds, 1.5);
    assert_eq!(items[0].properties["sourceIn"], serde_json::json!(2.0));
    assert_eq!(items[0].properties["sourceOut"], serde_json::json!(3.5));

    assert_eq!(items[1].id, "item-1-b");
    assert_eq!(items[1].start_seconds, 1.5);
    assert_eq!(items[1].duration_seconds, 2.5);
    assert_eq!(items[1].source, items[0].source);
    assert_eq!(items[1].properties["sourceIn"], serde_json::json!(3.5));
    assert_eq!(items[1].properties["sourceOut"], serde_json::json!(6.0));
    assert_eq!(project.timeline.duration_seconds, 4.0);
}

#[test]
fn trim_items_updates_timing_and_media_source_range() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 1.0,
                duration_seconds: 3.0,
                source_in: Some(2.0),
                source_out: Some(5.0),
            }],
        },
    )
    .expect("trim item");

    let item = &project.timeline.tracks[0].items[0];
    assert_eq!(item.start_seconds, 1.0);
    assert_eq!(item.duration_seconds, 3.0);
    assert_eq!(item.properties["sourceIn"], serde_json::json!(2.0));
    assert_eq!(item.properties["sourceOut"], serde_json::json!(5.0));
    assert_eq!(project.timeline.duration_seconds, 4.0);
}

#[test]
fn trim_items_accepts_source_range_scaled_by_clip_speed() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), serde_json::json!(2.0));

    apply_project_action(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source_in: Some(0.0),
                source_out: Some(4.0),
            }],
        },
    )
    .expect("trim speed-adjusted item");

    let item = &project.timeline.tracks[0].items[0];
    assert_eq!(item.duration_seconds, 2.0);
    assert_eq!(item.properties["sourceIn"], serde_json::json!(0.0));
    assert_eq!(item.properties["sourceOut"], serde_json::json!(4.0));
}

#[test]
fn trim_items_rejects_source_range_that_ignores_clip_speed() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("speed".to_string(), serde_json::json!(2.0));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source_in: Some(0.0),
                source_out: Some(2.0),
            }],
        },
        ProjectActionError::SourceRangeDurationMismatch("item-1".to_string()),
    );
}

#[test]
fn edit_caption_text_trims_text_and_marks_caption_edited() {
    let mut project = sample_project();
    project.timeline.tracks[3]
        .items
        .push(caption_item("caption-1", 1.0));

    apply_project_action(
        &mut project,
        ProjectAction::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "  Corrected caption  ".to_string(),
        },
    )
    .expect("edit caption text");

    let item = find_timeline_item(&project, "caption-1").expect("caption item");
    assert_eq!(
        item.source,
        TimelineSource::Text {
            text: "Corrected caption".to_string()
        }
    );
    assert_eq!(item.properties["textEdited"], serde_json::json!(true));
}

#[test]
fn edit_text_item_trims_text_syncs_properties_and_marks_text_edited() {
    let mut project = sample_project();
    project.timeline.tracks[2]
        .items
        .push(text_overlay_item("overlay-1", 1.0));

    apply_project_action(
        &mut project,
        ProjectAction::EditTextItem {
            item_id: "overlay-1".to_string(),
            text: "  Updated overlay  ".to_string(),
        },
    )
    .expect("edit text item");

    let item = find_timeline_item(&project, "overlay-1").expect("text overlay item");
    assert_eq!(
        item.source,
        TimelineSource::Text {
            text: "Updated overlay".to_string()
        }
    );
    assert_eq!(item.label, "Updated overlay");
    assert_eq!(
        item.properties["text"],
        serde_json::json!("Updated overlay")
    );
    assert_eq!(item.properties["textEdited"], serde_json::json!(true));
}

#[test]
fn set_item_keyframes_allows_empty_array_to_clear_property_track() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::SetItemKeyframes {
            item_id: "item-1".to_string(),
            property: ProjectActionKeyframeProperty::Opacity,
            keyframes: vec![ProjectActionKeyframe {
                at_seconds: 0.0,
                value: 0.5,
                easing: Some("linear".to_string()),
            }],
        },
    )
    .expect("seed opacity keyframes");

    apply_project_action(
        &mut project,
        ProjectAction::SetItemKeyframes {
            item_id: "item-1".to_string(),
            property: ProjectActionKeyframeProperty::Opacity,
            keyframes: Vec::new(),
        },
    )
    .expect("clear opacity keyframes");

    let item = find_timeline_item(&project, "item-1").expect("item");
    assert!(!item.properties.contains_key("keyframes"));
}

#[test]
fn update_visual_clip_transform_merges_partial_transform_properties() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].properties.insert(
        "transform".to_string(),
        serde_json::json!({
            "centerX": 0.5,
            "centerY": 0.5,
            "width": 1.0,
            "height": 1.0
        }),
    );
    let action: ProjectAction = serde_json::from_value(serde_json::json!({
        "type": "updateVisualClipTransform",
        "itemId": "item-1",
        "transform": {
            "centerY": 0.82,
            "flipHorizontal": true
        }
    }))
    .expect("visual transform action should deserialize");

    apply_project_action(&mut project, action).expect("update visual transform");

    let item = find_timeline_item(&project, "item-1").expect("item");
    assert_eq!(
        item.properties["transform"],
        serde_json::json!({
            "centerX": 0.5,
            "centerY": 0.82,
            "width": 1.0,
            "height": 1.0,
            "flipHorizontal": true
        })
    );
}

#[test]
fn update_visual_clip_crop_merges_edges_and_removes_zero_values() {
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("cropTop".to_string(), serde_json::json!(0.1));

    let action: ProjectAction = serde_json::from_value(serde_json::json!({
        "type": "updateVisualClipCrop",
        "itemId": "item-1",
        "crop": { "cropRight": 0.25, "cropTop": 0.0 }
    }))
    .expect("visual crop action should deserialize");

    apply_project_action(&mut project, action).expect("update visual crop");

    let item = find_timeline_item(&project, "item-1").expect("item");
    assert!(!item.properties.contains_key("cropTop"));
    assert_eq!(item.properties["cropRight"], serde_json::json!(0.25));
}

#[test]
fn update_visual_clip_crop_rejects_empty_or_fully_cropped_axis() {
    let mut project = sample_project();
    let empty = ProjectAction::UpdateVisualClipCrop {
        item_id: "item-1".to_string(),
        crop: ProjectActionVisualCrop {
            crop_top: None,
            crop_right: None,
            crop_bottom: None,
            crop_left: None,
        },
    };
    assert!(matches!(
        apply_project_action(&mut project, empty),
        Err(ProjectActionError::InvalidVisualClipCrop(_))
    ));

    let invalid = ProjectAction::UpdateVisualClipCrop {
        item_id: "item-1".to_string(),
        crop: ProjectActionVisualCrop {
            crop_top: Some(0.5),
            crop_right: None,
            crop_bottom: Some(0.5),
            crop_left: None,
        },
    };
    assert!(matches!(
        apply_project_action(&mut project, invalid),
        Err(ProjectActionError::InvalidVisualClipCrop(_))
    ));
}

#[test]
fn update_visual_clip_fades_persists_both_edges_and_rejects_overflow() {
    let mut project = sample_project();
    apply_project_action(
        &mut project,
        ProjectAction::UpdateVisualClipFades {
            item_id: "item-1".to_string(),
            fade_in_seconds: 0.4,
            fade_out_seconds: 0.6,
        },
    )
    .expect("visual fades");

    let item = find_timeline_item(&project, "item-1").expect("item");
    assert_eq!(item.properties["fadeInSeconds"], serde_json::json!(0.4));
    assert_eq!(item.properties["fadeOutSeconds"], serde_json::json!(0.6));

    let error = apply_project_action(
        &mut project,
        ProjectAction::UpdateVisualClipFades {
            item_id: "item-1".to_string(),
            fade_in_seconds: 10.0,
            fade_out_seconds: 10.0,
        },
    )
    .expect_err("fades cannot exceed clip duration");
    assert!(matches!(
        error,
        ProjectActionError::InvalidVisualClipFades(_)
    ));
}

#[test]
fn update_text_overlay_items_applies_optional_text_style_fields() {
    let mut project = sample_project();
    project.timeline.tracks[2]
        .items
        .push(text_overlay_item("overlay-1", 1.0));

    let action: ProjectAction = serde_json::from_value(serde_json::json!({
        "type": "updateTextOverlayItems",
        "updates": [
            {
                "itemId": "overlay-1",
                "startSeconds": 2.25,
                "durationSeconds": 4.5,
                "text": "Launch title",
                "visualTreatment": "compact lower third with translucent backing and accent stroke",
                "motion": "quick slide-in, two-beat hold, soft fade-out",
                "safeZone": "keep inside 10% margins and away from faces",
                "avoid": "full-width opaque black slabs and default-font boxes",
                "fontName": "Helvetica-Bold",
                "fontSize": 72,
                "color": "#ffffff",
                "alignment": "center"
            }
        ]
    }))
    .expect("style-bearing text overlay action should deserialize");

    apply_project_action(&mut project, action).expect("update text overlay style");

    let item = find_timeline_item(&project, "overlay-1").expect("text overlay item");
    assert_eq!(
        item.properties["fontName"],
        serde_json::json!("Helvetica-Bold")
    );
    assert_eq!(item.properties["fontSize"], serde_json::json!(72.0));
    assert_eq!(item.properties["color"], serde_json::json!("#ffffff"));
    assert_eq!(item.properties["alignment"], serde_json::json!("center"));
}

#[test]
fn update_text_overlay_items_updates_timing_text_and_visual_metadata() {
    let mut project = sample_project();
    project.timeline.tracks[2]
        .items
        .push(text_overlay_item("overlay-1", 1.0));

    apply_project_action(
        &mut project,
        ProjectAction::UpdateTextOverlayItems {
            updates: vec![ProjectActionTextOverlayUpdate {
                item_id: "overlay-1".to_string(),
                start_seconds: 2.25,
                duration_seconds: 4.5,
                text: "  Launch title  ".to_string(),
                visual_treatment: "  bold upper-left title with transparent backing  ".to_string(),
                motion: "  fade in quickly, hold, then drift upward  ".to_string(),
                safe_zone: "  keep inside title safe margins  ".to_string(),
                avoid: "  covering faces or using opaque full-width slabs  ".to_string(),
                font_name: None,
                font_size: None,
                color: None,
                alignment: None,
            }],
        },
    )
    .expect("update text overlay");

    let item = find_timeline_item(&project, "overlay-1").expect("text overlay item");
    assert_eq!(item.start_seconds, 2.25);
    assert_eq!(item.duration_seconds, 4.5);
    assert_eq!(item.label, "Launch title");
    assert_eq!(
        item.source,
        TimelineSource::Text {
            text: "Launch title".to_string()
        }
    );
    assert_eq!(item.properties["text"], serde_json::json!("Launch title"));
    assert_eq!(
        item.properties["visualTreatment"],
        serde_json::json!("bold upper-left title with transparent backing")
    );
    assert_eq!(
        item.properties["motion"],
        serde_json::json!("fade in quickly, hold, then drift upward")
    );
    assert_eq!(
        item.properties["safeZone"],
        serde_json::json!("keep inside title safe margins")
    );
    assert_eq!(
        item.properties["avoid"],
        serde_json::json!("covering faces or using opaque full-width slabs")
    );
    assert_eq!(item.properties["textEdited"], serde_json::json!(true));
}

#[test]
fn edit_transcript_words_updates_word_and_records_repair_metadata() {
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());

    apply_project_action(
        &mut project,
        ProjectAction::EditTranscriptWords {
            edits: vec![ProjectActionTranscriptWordEdit {
                transcript_id: "transcript-media-1".to_string(),
                word_index: 1,
                text: Some("  Creator  ".to_string()),
                start_seconds: Some(0.55),
                end_seconds: Some(0.95),
                repair_id: "repair-1".to_string(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
            }],
        },
    )
    .expect("edit transcript word");

    let transcript = &project.transcripts[0];
    assert_eq!(transcript.words[1].text, "Creator");
    assert_eq!(transcript.words[1].start_seconds, 0.55);
    assert_eq!(transcript.words[1].end_seconds, 0.95);
    assert_eq!(transcript.repairs.len(), 1);
    let repair = &transcript.repairs[0];
    assert_eq!(repair.id, "repair-1");
    assert_eq!(repair.word_index, 1);
    assert_eq!(repair.before.text, "Creater");
    assert_eq!(repair.after.text, "Creator");
    assert_eq!(repair.created_at, "2026-06-22T10:00:00Z");
}

#[test]
fn apply_caption_repair_updates_caption_and_transcript_repair_metadata() {
    let mut project = sample_project();
    project.timeline.tracks[3]
        .items
        .push(caption_item("caption-1", 0.5));
    project.transcripts.push(transcript_with_words());

    apply_project_action(&mut project, caption_repair_action()).expect("apply caption repair");

    let caption = find_timeline_item(&project, "caption-1").expect("caption item");
    assert_eq!(caption.start_seconds, 1.5);
    assert!((caption.duration_seconds - 0.4).abs() < f64::EPSILON);
    assert_eq!(
        caption.source,
        TimelineSource::Text {
            text: "Creator".to_string()
        }
    );
    assert_eq!(caption.properties["captionRepairId"], "repair-caption-1");
    assert_eq!(caption.properties["transcriptId"], "transcript-media-1");
    assert_eq!(caption.properties["wordIndex"], serde_json::json!(1));

    let transcript = &project.transcripts[0];
    assert_eq!(transcript.words[1].text, "Creator");
    assert_eq!(transcript.words[1].start_seconds, 1.5);
    assert_eq!(transcript.words[1].end_seconds, 1.9);
    assert_eq!(transcript.repairs.len(), 1);
    assert_eq!(transcript.repairs[0].id, "repair-caption-1");
    assert_eq!(
        transcript.repairs[0].kind,
        TranscriptRepairKind::WordTextAndTiming
    );
    assert_eq!(transcript.repairs[0].before.text, "Creater");
    assert_eq!(transcript.repairs[0].after.text, "Creator");
}

#[test]
fn record_generated_asset_adds_provenance_and_generated_media_outputs() {
    let mut project = sample_project();

    apply_project_action(&mut project, generated_asset_action()).expect("record generated asset");

    assert_eq!(project.generated_assets.len(), 1);
    let generated = &project.generated_assets[0];
    assert_eq!(generated.id, "generated-shot-1");
    assert_eq!(generated.prompt, "slow push-in on the product");
    assert_eq!(generated.model.provider, "seedance");
    assert_eq!(
        generated.references.first_frame_media_id.as_deref(),
        Some("media-1")
    );

    let output = project
        .media
        .iter()
        .find(|media| media.id == "generated-shot-1-output")
        .expect("generated media output");
    assert_eq!(output.kind, MediaKind::Generated);
    assert_eq!(
        output.relative_path,
        "generated/generated-shot-1/output.mp4"
    );
    assert_eq!(output.duration_seconds, 4.0);
    assert_eq!(output.width, Some(1280));
    assert_eq!(output.height, Some(720));
    assert_eq!(output.fps, Some(24.0));
}

#[test]
fn replace_timeline_item_with_generated_output_swaps_clip_media() {
    let mut project = sample_project();
    apply_project_action(&mut project, generated_asset_action()).expect("record generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::ReplaceTimelineItemWithGeneratedOutput {
            replacement: ProjectActionReplaceGeneratedOutput {
                item_id: "item-1".to_string(),
                media_id: "generated-shot-1-output".to_string(),
            },
        },
    )
    .expect("replace timeline item with generated output");

    let item = find_timeline_item(&project, "item-1").expect("timeline item");
    assert_eq!(item.start_seconds, 0.0);
    assert_eq!(item.duration_seconds, 4.0);
    assert_eq!(
        item.source,
        TimelineSource::Media {
            media_id: "generated-shot-1-output".to_string()
        }
    );
    assert_eq!(item.properties["sourceIn"], serde_json::json!(0.0));
    assert_eq!(item.properties["sourceOut"], serde_json::json!(4.0));
    assert_eq!(
        item.properties["reason"],
        serde_json::json!("generated replacement")
    );
    assert_eq!(
        item.properties["generatedAssetId"],
        serde_json::json!("generated-shot-1")
    );
    assert_eq!(
        item.properties["generatedOutputMediaId"],
        serde_json::json!("generated-shot-1-output")
    );
}

#[test]
fn replace_timeline_item_with_generated_still_preserves_clip_duration() {
    let mut project = sample_project();
    let original_duration_seconds = find_timeline_item(&project, "item-1")
        .expect("original timeline item")
        .duration_seconds;
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.outputs = vec![ProjectActionGeneratedAssetOutput {
            media_id: "generated-still-output".to_string(),
            relative_path: "generated/generated-shot-1/output.png".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 0.0,
            fps: 0.0,
        }];
    }
    apply_project_action(&mut project, action).expect("record generated still asset");

    apply_project_action(
        &mut project,
        ProjectAction::ReplaceTimelineItemWithGeneratedOutput {
            replacement: ProjectActionReplaceGeneratedOutput {
                item_id: "item-1".to_string(),
                media_id: "generated-still-output".to_string(),
            },
        },
    )
    .expect("replace timeline item with generated still output");

    let item = find_timeline_item(&project, "item-1").expect("timeline item");
    assert_eq!(item.duration_seconds, original_duration_seconds);
    assert_eq!(
        item.source,
        TimelineSource::Media {
            media_id: "generated-still-output".to_string()
        }
    );
    assert_eq!(item.properties["sourceIn"], serde_json::json!(0.0));
    assert_eq!(
        item.properties["sourceOut"],
        serde_json::json!(original_duration_seconds)
    );
    assert_eq!(
        item.properties["generatedAssetId"],
        serde_json::json!("generated-shot-1")
    );
    assert_eq!(
        item.properties["generatedOutputMediaId"],
        serde_json::json!("generated-still-output")
    );
}

#[test]
fn record_generated_asset_allows_promptless_source_video_audio_asset() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.id = "generated-video-music-1".to_string();
        asset.status = GeneratedAssetStatus::Queued;
        asset.prompt = "   ".to_string();
        asset.model = ProjectActionGenerationModel {
            provider: "fal.ai".to_string(),
            id: "sonilo/v1.1/video-to-music".to_string(),
        };
        asset.references = ProjectActionGeneratedAssetReferences {
            media_ids: vec!["media-1".to_string()],
            source_video_media_ref: Some("media-1".to_string()),
            ..ProjectActionGeneratedAssetReferences::default()
        };
        asset.settings = ProjectActionGeneratedAssetSettings {
            width: None,
            height: None,
            duration_seconds: Some(4.0),
            fps: None,
            aspect_ratio: None,
            generate_audio: None,
            ..ProjectActionGeneratedAssetSettings::default()
        };
        asset.outputs.clear();
    }

    apply_project_action(&mut project, action)
        .expect("promptless source-video audio generated asset should be accepted");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-video-music-1")
        .expect("generated video music asset");
    assert_eq!(generated.prompt, "");
    assert_eq!(
        generated.references.source_video_media_ref.as_deref(),
        Some("media-1")
    );
}

#[test]
fn assign_media_folder_updates_media_folder_id() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-generated".to_string(),
        name: "Generated selects".to_string(),
        parent_id: None,
    });

    apply_project_action(
        &mut project,
        ProjectAction::AssignMediaFolder {
            media_id: "media-1".to_string(),
            folder_id: Some("folder-generated".to_string()),
        },
    )
    .expect("assign media folder");
    assert_eq!(
        project.media[0].folder_id.as_deref(),
        Some("folder-generated")
    );

    apply_project_action(
        &mut project,
        ProjectAction::AssignMediaFolder {
            media_id: "media-1".to_string(),
            folder_id: None,
        },
    )
    .expect("clear media folder");
    assert_eq!(project.media[0].folder_id, None);
}

#[test]
fn assign_media_folder_rejects_missing_folder_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AssignMediaFolder {
            media_id: "media-1".to_string(),
            folder_id: Some("missing-folder".to_string()),
        },
        ProjectActionError::MissingMediaFolder("missing-folder".to_string()),
    );
}

#[test]
fn media_folder_management_updates_folder_list_and_clears_deleted_assignments() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::CreateMediaFolder {
            folder: MediaFolder {
                id: "folder-generated".to_string(),
                name: "Generated selects".to_string(),
                parent_id: None,
            },
        },
    )
    .expect("create media folder");
    assert_eq!(project.media_folders[0].name, "Generated selects");

    apply_project_action(
        &mut project,
        ProjectAction::AssignMediaFolder {
            media_id: "media-1".to_string(),
            folder_id: Some("folder-generated".to_string()),
        },
    )
    .expect("assign media folder");

    apply_project_action(
        &mut project,
        ProjectAction::RenameMediaFolder {
            folder_id: "folder-generated".to_string(),
            name: "Final selects".to_string(),
        },
    )
    .expect("rename media folder");
    assert_eq!(project.media_folders[0].name, "Final selects");
    assert_eq!(
        project.media[0].folder_id.as_deref(),
        Some("folder-generated")
    );

    apply_project_action(
        &mut project,
        ProjectAction::DeleteMediaFolder {
            folder_id: "folder-generated".to_string(),
        },
    )
    .expect("delete media folder");
    assert!(project.media_folders.is_empty());
    assert_eq!(project.media[0].folder_id, None);
}

#[test]
fn media_folder_management_rejects_invalid_inputs_without_mutating_project() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-generated".to_string(),
        name: "Generated selects".to_string(),
        parent_id: None,
    });

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::CreateMediaFolder {
            folder: MediaFolder {
                id: "folder-generated".to_string(),
                name: "Duplicate".to_string(),
                parent_id: None,
            },
        },
        ProjectActionError::DuplicateMediaFolderId("folder-generated".to_string()),
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RenameMediaFolder {
            folder_id: "folder-generated".to_string(),
            name: " ".to_string(),
        },
        ProjectActionError::EmptyMediaFolderName,
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::DeleteMediaFolder {
            folder_id: "missing-folder".to_string(),
        },
        ProjectActionError::MissingMediaFolder("missing-folder".to_string()),
    );
}

#[test]
fn media_management_updates_names_and_deletes_unreferenced_assets() {
    let mut project = sample_project();
    project.media.push(MediaAsset {
        id: "unused-media".to_string(),
        name: None,
        relative_path: "media/unused.png".to_string(),
        kind: MediaKind::Image,
        duration_seconds: 0.0,
        width: Some(1920),
        height: Some(1080),
        fps: None,
        folder_id: Some("folder-generated".to_string()),
    });
    project.media_folders.push(MediaFolder {
        id: "folder-generated".to_string(),
        name: "Generated selects".to_string(),
        parent_id: None,
    });

    apply_project_action(
        &mut project,
        ProjectAction::RenameMedia {
            media_id: "media-1".to_string(),
            name: "Hero take".to_string(),
        },
    )
    .expect("rename media");
    assert_eq!(project.media[0].name.as_deref(), Some("Hero take"));

    apply_project_action(
        &mut project,
        ProjectAction::DeleteMedia {
            media_ids: vec!["unused-media".to_string()],
        },
    )
    .expect("delete unused media");
    assert!(!project.media.iter().any(|media| media.id == "unused-media"));
}

#[test]
fn media_management_deletes_referenced_media_and_timeline_items() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::DeleteMedia {
            media_ids: vec!["media-1".to_string()],
        },
    )
    .expect("delete referenced media");

    assert!(!project.media.iter().any(|media| media.id == "media-1"));
    assert!(project.timeline.tracks.iter().all(|track| {
        track.items.iter().all(|item| {
            !matches!(
                &item.source,
                TimelineSource::Media { media_id } if media_id == "media-1"
            )
        })
    }));
}

#[test]
fn media_management_rejects_invalid_media_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RenameMedia {
            media_id: "media-1".to_string(),
            name: " ".to_string(),
        },
        ProjectActionError::EmptyMediaName,
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::DeleteMedia {
            media_ids: vec!["missing-media".to_string()],
        },
        ProjectActionError::MissingMedia("missing-media".to_string()),
    );
}

#[test]
fn remove_tracks_and_update_render_settings_apply_editor_level_changes() {
    let mut project = sample_project();
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-overlay".to_string(),
        name: "Overlay".to_string(),
        kind: TrackKind::Overlay,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: Vec::new(),
    });

    apply_project_action(
        &mut project,
        ProjectAction::RemoveTracks {
            track_ids: vec!["track-overlay".to_string()],
        },
    )
    .expect("remove track");
    assert!(!project
        .timeline
        .tracks
        .iter()
        .any(|track| track.id == "track-overlay"));

    apply_project_action(
        &mut project,
        ProjectAction::UpdateRenderSettings {
            settings: RenderSettings {
                width: 1080,
                height: 1920,
                fps: 30.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::BurnIn,
            },
        },
    )
    .expect("update render settings");
    assert_eq!(project.render_settings.width, 1080);
    assert_eq!(project.render_settings.height, 1920);
    assert_eq!(project.render_settings.fps, 30.0);
}

#[test]
fn remove_tracks_and_update_render_settings_reject_invalid_inputs_without_mutation() {
    let mut project = sample_project();
    project.timeline.tracks[0].locked = true;

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RemoveTracks {
            track_ids: vec!["missing-track".to_string()],
        },
        ProjectActionError::TrackNotFound("missing-track".to_string()),
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RemoveTracks {
            track_ids: vec!["track-video".to_string()],
        },
        ProjectActionError::TrackLocked("track-video".to_string()),
    );
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateRenderSettings {
            settings: RenderSettings {
                width: 0,
                height: 1920,
                fps: 30.0,
                loudness_lufs: -16.0,
                captions: CaptionRenderMode::BurnIn,
            },
        },
        ProjectActionError::InvalidRenderSettings("width must be greater than zero".to_string()),
    );
}

#[test]
fn visual_effect_and_color_actions_update_editable_effect_stack() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::UpdateItemEffects {
            item_ids: vec!["item-1".to_string()],
            effects: vec![ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: "stylize.glow".to_string(),
                enabled: true,
                params: BTreeMap::from([
                    ("intensity".to_string(), serde_json::json!(0.8)),
                    ("radius".to_string(), serde_json::json!(12.0)),
                ]),
            }],
        },
    )
    .expect("apply effect");

    let item = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("video item");
    assert_eq!(
        item.properties["effects"],
        serde_json::json!([
            {
                "effectInstanceId": "legacy:stylize.glow:1",
                "effectType": "stylize.glow",
                "enabled": true,
                "params": {
                    "intensity": 0.8,
                    "radius": 12.0
                }
            }
        ])
    );

    apply_project_action(
        &mut project,
        ProjectAction::UpdateItemColorGrade {
            item_ids: vec!["item-1".to_string()],
            reset: false,
            grade: ProjectActionColorGrade {
                exposure: Some(0.25),
                contrast: Some(1.1),
                saturation: None,
                temperature: Some(7200.0),
                tint: None,
                extra: BTreeMap::new(),
            },
        },
    )
    .expect("apply color grade");

    let item = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("video item");
    assert_eq!(
        item.properties["colorGrade"],
        serde_json::json!({
            "exposure": 0.25,
            "contrast": 1.1,
            "temperature": 7200.0
        })
    );
    assert_eq!(
        item.properties["effects"][0]["effectType"],
        serde_json::json!("stylize.glow")
    );

    apply_project_action(
        &mut project,
        ProjectAction::UpdateItemColorGrade {
            item_ids: vec!["item-1".to_string()],
            reset: true,
            grade: ProjectActionColorGrade {
                exposure: None,
                contrast: None,
                saturation: Some(0.75),
                temperature: None,
                tint: None,
                extra: BTreeMap::new(),
            },
        },
    )
    .expect("reset color grade");
    let item = project
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("video item");
    assert_eq!(
        item.properties["colorGrade"],
        serde_json::json!({ "saturation": 0.75 })
    );
}

#[test]
fn visual_effect_and_color_actions_reject_invalid_targets_without_mutating_project() {
    let mut project = sample_project();
    project.timeline.tracks[0].locked = true;

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateItemEffects {
            item_ids: vec!["item-1".to_string()],
            effects: vec![ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: "stylize.glow".to_string(),
                enabled: true,
                params: BTreeMap::new(),
            }],
        },
        ProjectActionError::TrackLocked("track-video".to_string()),
    );

    project.timeline.tracks[0].locked = false;
    project.timeline.tracks.push(TimelineTrack {
        transitions: Vec::new(),
        id: "track-audio".to_string(),
        name: "Audio".to_string(),
        kind: TrackKind::Audio,
        locked: false,
        sync_locked: false,
        enabled: true,
        items: vec![TimelineItem {
            id: "item-audio".to_string(),
            kind: TimelineItemKind::AudioClip,
            start_seconds: 0.0,
            duration_seconds: 4.0,
            source: TimelineSource::Media {
                media_id: "media-1".to_string(),
            },
            label: "Audio".to_string(),
            properties: BTreeMap::new(),
        }],
    });
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateItemEffects {
            item_ids: vec!["item-audio".to_string()],
            effects: vec![ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: "stylize.glow".to_string(),
                enabled: true,
                params: BTreeMap::new(),
            }],
        },
        ProjectActionError::NotVisualClip("item-audio".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateItemColorGrade {
            item_ids: vec!["item-1".to_string()],
            reset: false,
            grade: ProjectActionColorGrade {
                exposure: Some(4.0),
                contrast: None,
                saturation: None,
                temperature: None,
                tint: None,
                extra: BTreeMap::new(),
            },
        },
        ProjectActionError::InvalidColorGrade("exposure must be between -3 and 3".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateItemEffects {
            item_ids: vec!["item-1".to_string()],
            effects: vec![ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: "future.hologram".to_string(),
                enabled: true,
                params: BTreeMap::from([("wobble".to_string(), serde_json::json!(0.5))]),
            }],
        },
        ProjectActionError::InvalidEffectStack("unknown effect: future.hologram".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateItemEffects {
            item_ids: vec!["item-1".to_string()],
            effects: vec![ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: "stylize.glow".to_string(),
                enabled: true,
                params: BTreeMap::from([("unknown".to_string(), serde_json::json!(0.5))]),
            }],
        },
        ProjectActionError::InvalidEffectStack(
            "stylize.glow unknown effect parameter: unknown".to_string(),
        ),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateItemEffects {
            item_ids: vec!["item-1".to_string()],
            effects: vec![ProjectActionEffect {
                effect_instance_id: String::new(),
                effect_type: "stylize.glow".to_string(),
                enabled: true,
                params: BTreeMap::from([("intensity".to_string(), serde_json::json!(2.0))]),
            }],
        },
        ProjectActionError::InvalidEffectStack(
            "stylize.glow intensity must be between 0 and 1".to_string(),
        ),
    );
}

#[test]
fn record_generated_asset_rejects_invalid_inputs_without_mutating_project() {
    let mut project = sample_project();

    let mut missing_reference = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut missing_reference {
        asset.references.media_ids = vec!["missing-media".to_string()];
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        missing_reference,
        ProjectActionError::MissingMedia("missing-media".to_string()),
    );

    let mut empty_reference = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut empty_reference {
        asset.references.media_ids = vec!["  ".to_string()];
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        empty_reference,
        ProjectActionError::EmptyGeneratedMediaReference,
    );

    let mut unsafe_asset_id = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut unsafe_asset_id {
        asset.id = "generated-shot-1/nested".to_string();
        asset.outputs[0].relative_path = "generated/generated-shot-1/nested/output.mp4".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        unsafe_asset_id,
        ProjectActionError::InvalidGeneratedAssetId("generated-shot-1/nested".to_string()),
    );

    let mut empty_outputs = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut empty_outputs {
        asset.outputs.clear();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        empty_outputs,
        ProjectActionError::EmptyItems,
    );

    let mut duplicate_output = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut duplicate_output {
        asset.outputs[0].media_id = "media-1".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        duplicate_output,
        ProjectActionError::DuplicateMediaId("media-1".to_string()),
    );

    let mut empty_output_media_id = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut empty_output_media_id {
        asset.outputs[0].media_id = "  ".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        empty_output_media_id,
        ProjectActionError::EmptyGeneratedOutputMediaId,
    );

    let mut wrong_kind = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut wrong_kind {
        asset.kind = MediaKind::Audio;
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        wrong_kind,
        ProjectActionError::InvalidGeneratedAssetKind(MediaKind::Audio),
    );

    project.media.push(MediaAsset {
        id: "audio-1".to_string(),
        name: None,
        relative_path: "media/audio.wav".to_string(),
        kind: MediaKind::Audio,
        duration_seconds: 4.0,
        width: None,
        height: None,
        fps: None,
        folder_id: None,
    });
    let mut audio_first_frame = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut audio_first_frame {
        asset.references.first_frame_media_id = Some("audio-1".to_string());
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        audio_first_frame,
        ProjectActionError::InvalidGeneratedFrameReference("audio-1".to_string()),
    );

    let mut output_outside_asset = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut output_outside_asset {
        asset.outputs[0].relative_path = "generated/other-shot/output.mp4".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        output_outside_asset,
        ProjectActionError::GeneratedOutputOutsideAssetDirectory(
            "generated/other-shot/output.mp4".to_string(),
        ),
    );

    let mut output_asset_directory = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut output_asset_directory {
        asset.outputs[0].relative_path = "generated/generated-shot-1".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        output_asset_directory,
        ProjectActionError::GeneratedOutputOutsideAssetDirectory(
            "generated/generated-shot-1".to_string(),
        ),
    );

    let mut output_asset_sidecar = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut output_asset_sidecar {
        asset.outputs[0].relative_path = "generated/generated-shot-1/asset.json".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        output_asset_sidecar,
        ProjectActionError::GeneratedOutputReservedPath(
            "generated/generated-shot-1/asset.json".to_string(),
        ),
    );

    let mut output_path_escape = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut output_path_escape {
        asset.outputs[0].relative_path =
            "generated/generated-shot-1/../other/output.mp4".to_string();
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        output_path_escape,
        ProjectActionError::GeneratedOutputOutsideAssetDirectory(
            "generated/generated-shot-1/../other/output.mp4".to_string(),
        ),
    );

    let mut missing_parent = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut missing_parent {
        asset.parent_asset_id = Some("missing-parent".to_string());
    }
    assert_action_error_leaves_project_unchanged(
        &mut project,
        missing_parent,
        ProjectActionError::MissingGeneratedAssetReference("missing-parent".to_string()),
    );

    apply_project_action(&mut project, generated_asset_action()).expect("record generated asset");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        generated_asset_action(),
        ProjectActionError::DuplicateGeneratedAssetId("generated-shot-1".to_string()),
    );
}

#[test]
fn record_generated_asset_allows_pending_without_outputs() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }

    apply_project_action(&mut project, action).expect("record pending generated asset");

    assert_eq!(project.generated_assets.len(), 1);
    let generated = &project.generated_assets[0];
    assert_eq!(generated.id, "generated-shot-1");
    assert_eq!(generated.name.as_deref(), Some("Hero product reveal"));
    assert_eq!(generated.status, GeneratedAssetStatus::Queued);
    assert!(generated.outputs.is_empty());
    assert!(!project
        .media
        .iter()
        .any(|media| media.id == "generated-shot-1-output"));
}

#[test]
fn update_generated_asset_status_marks_pending_asset_running() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::UpdateGeneratedAssetStatus {
            asset_id: "generated-shot-1".to_string(),
            status: GeneratedAssetStatus::Running,
        },
    )
    .expect("mark generated asset running");

    assert_eq!(
        project.generated_assets[0].status,
        GeneratedAssetStatus::Running
    );
    assert!(project.generated_assets[0].outputs.is_empty());
}

#[test]
fn update_generated_asset_status_rejects_missing_asset_without_mutating_project() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateGeneratedAssetStatus {
            asset_id: "missing-generated-shot".to_string(),
            status: GeneratedAssetStatus::Running,
        },
        ProjectActionError::MissingGeneratedAssetReference("missing-generated-shot".to_string()),
    );
}

#[test]
fn update_generated_asset_references_persists_provider_input_url_on_pending_asset() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::UpdateGeneratedAssetReferences {
            asset_id: "generated-shot-1".to_string(),
            references: ProjectActionGeneratedAssetReferences {
                media_ids: vec!["media-1".to_string()],
                first_frame_media_id: Some("media-1".to_string()),
                last_frame_media_id: None,
                provider_input_urls: vec![
                    "  https://fal.media/uploads/source.png  ".to_string(),
                    " ".to_string(),
                ],
                ..Default::default()
            },
        },
    )
    .expect("update generated asset references");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.references.media_ids, vec!["media-1"]);
    assert_eq!(
        generated.references.first_frame_media_id.as_deref(),
        Some("media-1")
    );
    assert_eq!(
        generated.references.provider_input_urls,
        vec!["https://fal.media/uploads/source.png"]
    );
}

#[test]
fn update_generated_asset_references_rejects_missing_media_without_mutating_project() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateGeneratedAssetReferences {
            asset_id: "generated-shot-1".to_string(),
            references: ProjectActionGeneratedAssetReferences {
                media_ids: vec!["missing-media".to_string()],
                first_frame_media_id: None,
                last_frame_media_id: None,
                provider_input_urls: vec!["https://fal.media/uploads/source.png".to_string()],
                ..Default::default()
            },
        },
        ProjectActionError::MissingMedia("missing-media".to_string()),
    );
}

#[test]
fn record_generated_asset_omits_blank_name() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.name = Some("  ".to_string());
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }

    apply_project_action(&mut project, action).expect("record unnamed generated asset");

    assert_eq!(project.generated_assets[0].name, None);
}

#[test]
fn complete_generated_asset_files_outputs_in_target_folder() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-generated".to_string(),
        name: "Generated selects".to_string(),
        parent_id: None,
    });
    let mut action = serde_json::to_value(generated_asset_action()).expect("action json");
    action["asset"]["status"] = serde_json::json!("queued");
    action["asset"]["outputs"] = serde_json::json!([]);
    action["asset"]["targetFolderId"] = serde_json::json!("folder-generated");
    let action: ProjectAction = serde_json::from_value(action).expect("target folder action");
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-1".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-1-output".to_string(),
                relative_path: "generated/generated-shot-1/output.mp4".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            completion: None,
            replacement: None,
        },
    )
    .expect("complete generated asset");

    let output_media = project
        .media
        .iter()
        .find(|media| media.id == "generated-shot-1-output")
        .expect("generated output media");
    assert_eq!(output_media.folder_id.as_deref(), Some("folder-generated"));
    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.outputs[0].source_url.as_deref(), None);
}

#[test]
fn record_generated_asset_rejects_missing_target_folder() {
    let mut project = sample_project();
    let mut action = serde_json::to_value(generated_asset_action()).expect("action json");
    action["asset"]["status"] = serde_json::json!("queued");
    action["asset"]["outputs"] = serde_json::json!([]);
    action["asset"]["targetFolderId"] = serde_json::json!("missing-folder");
    let action: ProjectAction = serde_json::from_value(action).expect("target folder action");

    let error = apply_project_action(&mut project, action).expect_err("missing target folder");

    assert_eq!(
        error,
        ProjectActionError::MissingMediaFolder("missing-folder".to_string())
    );
}

#[test]
fn delete_media_folder_clears_generated_asset_target_folder() {
    let mut project = sample_project();
    project.media_folders.push(MediaFolder {
        id: "folder-generated".to_string(),
        name: "Generated selects".to_string(),
        parent_id: None,
    });
    let mut action = serde_json::to_value(generated_asset_action()).expect("action json");
    action["asset"]["status"] = serde_json::json!("queued");
    action["asset"]["outputs"] = serde_json::json!([]);
    action["asset"]["targetFolderId"] = serde_json::json!("folder-generated");
    let action: ProjectAction = serde_json::from_value(action).expect("target folder action");
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::DeleteMediaFolder {
            folder_id: "folder-generated".to_string(),
        },
    )
    .expect("delete target folder");

    assert_eq!(project.generated_assets[0].target_folder_id, None);
}

#[test]
fn complete_generated_asset_adds_outputs_to_pending_asset() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-1".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-1-output".to_string(),
                relative_path: "generated/generated-shot-1/output.mp4".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            completion: None,
            replacement: None,
        },
    )
    .expect("complete generated asset");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);
    assert_eq!(generated.outputs.len(), 1);
    assert!(project
        .media
        .iter()
        .any(|media| media.id == "generated-shot-1-output" && media.kind == MediaKind::Generated));
}

#[test]
fn complete_generated_asset_can_replace_timeline_item_with_completed_output() {
    let mut project = sample_project();
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-1".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-1-output".to_string(),
                relative_path: "generated/generated-shot-1/output.mp4".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            completion: None,
            replacement: Some(ProjectActionReplaceGeneratedOutput {
                item_id: "item-1".to_string(),
                media_id: "generated-shot-1-output".to_string(),
            }),
        },
    )
    .expect("complete generated asset and replace timeline item");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);

    let item = find_timeline_item(&project, "item-1").expect("timeline item");
    assert_eq!(
        item.source,
        TimelineSource::Media {
            media_id: "generated-shot-1-output".to_string()
        }
    );
    assert_eq!(item.duration_seconds, 4.0);
    assert_eq!(
        item.properties["generatedAssetId"],
        serde_json::json!("generated-shot-1")
    );
}

#[test]
fn complete_generated_asset_replaces_linked_items_sharing_source_media() {
    let mut project = sample_project();
    let link_group_id = "linked-replacement-1";
    let linked_item = TimelineItem {
        id: "item-1-linked".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 4.0,
        duration_seconds: 3.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Linked source".to_string(),
        properties: BTreeMap::from([
            ("linkGroupId".to_string(), serde_json::json!(link_group_id)),
            ("sourceIn".to_string(), serde_json::json!(1.0)),
            ("sourceOut".to_string(), serde_json::json!(4.0)),
        ]),
    };
    let item = project.timeline.tracks[0]
        .items
        .iter_mut()
        .find(|item| item.id == "item-1")
        .expect("sample timeline item");
    item.properties
        .insert("linkGroupId".to_string(), serde_json::json!(link_group_id));
    item.properties
        .insert("sourceIn".to_string(), serde_json::json!(0.0));
    item.properties
        .insert("sourceOut".to_string(), serde_json::json!(4.0));
    project.timeline.tracks[0].items.push(linked_item);

    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-1".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-1-output".to_string(),
                relative_path: "generated/generated-shot-1/output.mp4".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            completion: None,
            replacement: Some(ProjectActionReplaceGeneratedOutput {
                item_id: "item-1".to_string(),
                media_id: "generated-shot-1-output".to_string(),
            }),
        },
    )
    .expect("complete generated asset and replace linked timeline items");

    for item_id in ["item-1", "item-1-linked"] {
        let item = find_timeline_item(&project, item_id).expect("timeline item");
        assert_eq!(
            item.source,
            TimelineSource::Media {
                media_id: "generated-shot-1-output".to_string()
            }
        );
        assert_eq!(
            item.properties["generatedAssetId"],
            serde_json::json!("generated-shot-1")
        );
        assert_eq!(
            item.properties["generatedOutputMediaId"],
            serde_json::json!("generated-shot-1-output")
        );
    }
}

#[test]
fn complete_generated_asset_can_replace_timeline_item_with_still_output() {
    let mut project = sample_project();
    let original_duration_seconds = find_timeline_item(&project, "item-1")
        .expect("original timeline item")
        .duration_seconds;
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    apply_project_action(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-1".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-still-output".to_string(),
                relative_path: "generated/generated-shot-1/output.png".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 0.0,
                fps: 0.0,
            }],
            completion: None,
            replacement: Some(ProjectActionReplaceGeneratedOutput {
                item_id: "item-1".to_string(),
                media_id: "generated-still-output".to_string(),
            }),
        },
    )
    .expect("complete generated still asset and replace timeline item");

    let generated = project
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);
    assert_eq!(generated.outputs[0].duration_seconds, 0.0);

    let item = find_timeline_item(&project, "item-1").expect("timeline item");
    assert_eq!(item.duration_seconds, original_duration_seconds);
    assert_eq!(
        item.source,
        TimelineSource::Media {
            media_id: "generated-still-output".to_string()
        }
    );
    assert_eq!(
        item.properties["sourceOut"],
        serde_json::json!(original_duration_seconds)
    );
    assert_eq!(
        item.properties["generatedOutputMediaId"],
        serde_json::json!("generated-still-output")
    );
}

#[test]
fn complete_generated_asset_rejects_replacement_media_not_in_completed_outputs() {
    let mut project = sample_project();
    apply_project_action(&mut project, generated_asset_action()).expect("record other output");
    let mut action = generated_asset_action();
    if let ProjectAction::RecordGeneratedAsset { asset } = &mut action {
        asset.id = "generated-shot-2".to_string();
        asset.status = GeneratedAssetStatus::Queued;
        asset.outputs.clear();
        asset.parent_asset_id = Some("generated-shot-1".to_string());
        asset.retry_of_asset_id = Some("generated-shot-1".to_string());
    }
    apply_project_action(&mut project, action).expect("record pending generated asset");

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-2".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-2-output".to_string(),
                relative_path: "generated/generated-shot-2/output.mp4".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            completion: None,
            replacement: Some(ProjectActionReplaceGeneratedOutput {
                item_id: "item-1".to_string(),
                media_id: "generated-shot-1-output".to_string(),
            }),
        },
        ProjectActionError::CompletionReplacementOutputMismatch(
            "generated-shot-1-output".to_string(),
        ),
    );
}

#[test]
fn complete_generated_asset_rejects_completed_asset_without_mutating_project() {
    let mut project = sample_project();
    apply_project_action(&mut project, generated_asset_action()).expect("record generated asset");

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::CompleteGeneratedAsset {
            asset_id: "generated-shot-1".to_string(),
            outputs: vec![ProjectActionGeneratedAssetOutput {
                media_id: "generated-shot-1-replacement".to_string(),
                relative_path: "generated/generated-shot-1/replacement.mp4".to_string(),
                source_url: None,
                width: 1280,
                height: 720,
                duration_seconds: 4.0,
                fps: 24.0,
            }],
            completion: None,
            replacement: None,
        },
        ProjectActionError::GeneratedAssetAlreadyCompleted("generated-shot-1".to_string()),
    );
}

#[test]
fn attach_render_report_records_review_artifact() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: render_report(),
        },
    )
    .expect("attach render report");

    assert_eq!(project.render_reports.len(), 1);
    let report = &project.render_reports[0];
    assert_eq!(report.id, "render-draft-1");
    assert_eq!(report.status, RenderReportStatus::Completed);
    assert_eq!(report.duration_seconds, 42.5);
    assert!(report.streams.video);
    assert!(report.streams.audio);
    assert_eq!(
        report.checks["captionAlignment"],
        RenderReportCheckStatus::Passed
    );
}

#[test]
fn attach_render_report_rejects_invalid_reports_without_mutating_project() {
    let mut project = sample_project();

    let mut empty_output_path = render_report();
    empty_output_path.output_path.clear();
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: empty_output_path,
        },
        ProjectActionError::EmptyRenderOutputPath,
    );

    let mut invalid_duration = render_report();
    invalid_duration.duration_seconds = 0.0;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: invalid_duration,
        },
        ProjectActionError::NonPositiveDuration,
    );

    let mut missing_streams = render_report();
    missing_streams.streams.video = false;
    missing_streams.streams.audio = false;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_streams,
        },
        ProjectActionError::EmptyRenderStreams,
    );

    let mut missing_artifacts = render_report();
    missing_artifacts.artifacts.clear();
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_artifacts,
        },
        ProjectActionError::EmptyRenderArtifacts,
    );

    let mut missing_output_artifact = render_report();
    missing_output_artifact.artifacts = vec!["renders/render-draft-1/sidecar.json".to_string()];
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_output_artifact,
        },
        ProjectActionError::EmptyRenderArtifacts,
    );

    let mut missing_check = render_report();
    missing_check.checks.remove("artifactPaths");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_check,
        },
        ProjectActionError::MissingRenderCheck("artifactPaths".to_string()),
    );

    let mut missing_stream_check = render_report();
    missing_stream_check.checks.remove("streams");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_stream_check,
        },
        ProjectActionError::MissingRenderCheck("streams".to_string()),
    );

    let mut missing_log_check = render_report();
    missing_log_check.checks.remove("logPath");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_log_check,
        },
        ProjectActionError::MissingRenderCheck("logPath".to_string()),
    );

    let mut missing_visual_frame_evidence_check = render_report();
    missing_visual_frame_evidence_check
        .checks
        .remove("visualFrameEvidence");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: missing_visual_frame_evidence_check,
        },
        ProjectActionError::MissingRenderCheck("visualFrameEvidence".to_string()),
    );

    apply_project_action(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: render_report(),
        },
    )
    .expect("attach render report");
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::AttachRenderReport {
            report: render_report(),
        },
        ProjectActionError::DuplicateRenderReportId("render-draft-1".to_string()),
    );
}

#[test]
fn update_template_items_updates_timing_fields_and_duration() {
    let mut project = sample_project();
    project.timeline.tracks[2]
        .items
        .push(template_item("template-1", 1.0));

    apply_project_action(
        &mut project,
        ProjectAction::UpdateTemplateItems {
            updates: vec![ProjectActionTemplateUpdate {
                item_id: "template-1".to_string(),
                start_seconds: 4.5,
                duration_seconds: 1.25,
                template_fields: BTreeMap::from([
                    ("headline".to_string(), "Olha API".to_string()),
                    ("subline".to_string(), "Founder".to_string()),
                ]),
            }],
        },
    )
    .expect("update template item");

    let item = find_timeline_item(&project, "template-1").expect("template item");
    assert_eq!(item.start_seconds, 4.5);
    assert_eq!(item.duration_seconds, 1.25);
    assert_eq!(
        item.properties["templateFields"],
        serde_json::json!({
            "headline": "Olha API",
            "subline": "Founder"
        })
    );
    assert_eq!(project.timeline.duration_seconds, 5.75);
}

#[test]
fn update_template_items_rejects_invalid_updates_without_mutating_project() {
    let mut project = sample_project();
    project.timeline.tracks[2]
        .items
        .push(template_item("template-1", 1.0));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateItems { updates: vec![] },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateItems {
            updates: vec![ProjectActionTemplateUpdate {
                item_id: "template-1".to_string(),
                start_seconds: -0.1,
                duration_seconds: 1.0,
                template_fields: BTreeMap::new(),
            }],
        },
        ProjectActionError::NegativeTime,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateItems {
            updates: vec![ProjectActionTemplateUpdate {
                item_id: "template-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 0.0,
                template_fields: BTreeMap::new(),
            }],
        },
        ProjectActionError::NonPositiveDuration,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateItems {
            updates: vec![ProjectActionTemplateUpdate {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 1.0,
                template_fields: BTreeMap::new(),
            }],
        },
        ProjectActionError::NotTemplateItem("item-1".to_string()),
    );

    project.timeline.tracks[2].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateItems {
            updates: vec![ProjectActionTemplateUpdate {
                item_id: "template-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 1.0,
                template_fields: BTreeMap::new(),
            }],
        },
        ProjectActionError::TrackLocked("track-overlays".to_string()),
    );
}

#[test]
fn update_template_override_upserts_project_template_metadata() {
    let mut project = sample_project();

    apply_project_action(
        &mut project,
        ProjectAction::UpdateTemplateOverride {
            override_: template_override_update(),
        },
    )
    .expect("update template override");

    assert_eq!(project.template_overrides.len(), 1);
    let template = &project.template_overrides[0];
    assert_eq!(template.schema_version, 1);
    assert_eq!(template.template_id, "kinetic-lower-third-v1");
    assert_eq!(template.fields["headline"], "Launch day");
    assert_eq!(template.style["accentColor"], serde_json::json!("#22d3ee"));
    assert!(template.safe_zone.contains("10% margins"));

    let mut update = template_override_update();
    update
        .fields
        .insert("headline".to_string(), "Updated".to_string());
    apply_project_action(
        &mut project,
        ProjectAction::UpdateTemplateOverride { override_: update },
    )
    .expect("replace template override");

    assert_eq!(project.template_overrides.len(), 1);
    assert_eq!(project.template_overrides[0].fields["headline"], "Updated");
}

#[test]
fn update_template_override_rejects_invalid_visual_metadata_without_mutating_project() {
    let mut project = sample_project();

    let mut missing_template_id = template_override_update();
    missing_template_id.template_id = " ".to_string();
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateOverride {
            override_: missing_template_id,
        },
        ProjectActionError::EmptyTemplateId,
    );

    let mut missing_safe_zone = template_override_update();
    missing_safe_zone.safe_zone = " ".to_string();
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTemplateOverride {
            override_: missing_safe_zone,
        },
        ProjectActionError::MissingTemplateVisualMetadata("safeZone".to_string()),
    );
}

#[test]
fn resize_and_caption_edit_reject_invalid_inputs_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id: "item-1".to_string(),
                duration_seconds: 0.0,
            }],
        },
        ProjectActionError::NonPositiveDuration,
    );

    project.timeline.tracks[3]
        .items
        .push(caption_item("caption-1", 1.0));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditCaptionText {
            item_id: "caption-1".to_string(),
            text: "   ".to_string(),
        },
        ProjectActionError::EmptyCaptionText,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditCaptionText {
            item_id: "item-1".to_string(),
            text: "Wrong target".to_string(),
        },
        ProjectActionError::NotCaptionItem("item-1".to_string()),
    );

    project.timeline.tracks[2]
        .items
        .push(text_overlay_item("overlay-1", 1.0));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTextItem {
            item_id: "overlay-1".to_string(),
            text: "   ".to_string(),
        },
        ProjectActionError::EmptyTextItemText,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTextItem {
            item_id: "item-1".to_string(),
            text: "Wrong target".to_string(),
        },
        ProjectActionError::NotTextItem("item-1".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTextOverlayItems { updates: vec![] },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTextOverlayItems {
            updates: vec![ProjectActionTextOverlayUpdate {
                item_id: "overlay-1".to_string(),
                start_seconds: 1.0,
                duration_seconds: 3.0,
                text: "   ".to_string(),
                visual_treatment: "bold title".to_string(),
                motion: "fade".to_string(),
                safe_zone: "title safe".to_string(),
                avoid: "faces".to_string(),
                font_name: None,
                font_size: None,
                color: None,
                alignment: None,
            }],
        },
        ProjectActionError::EmptyTextItemText,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTextOverlayItems {
            updates: vec![ProjectActionTextOverlayUpdate {
                item_id: "overlay-1".to_string(),
                start_seconds: 1.0,
                duration_seconds: 3.0,
                text: "Launch title".to_string(),
                visual_treatment: "   ".to_string(),
                motion: "fade".to_string(),
                safe_zone: "title safe".to_string(),
                avoid: "faces".to_string(),
                font_name: None,
                font_size: None,
                color: None,
                alignment: None,
            }],
        },
        ProjectActionError::MissingTextOverlayVisualMetadata("visualTreatment".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::UpdateTextOverlayItems {
            updates: vec![ProjectActionTextOverlayUpdate {
                item_id: "item-1".to_string(),
                start_seconds: 1.0,
                duration_seconds: 3.0,
                text: "Launch title".to_string(),
                visual_treatment: "bold title".to_string(),
                motion: "fade".to_string(),
                safe_zone: "title safe".to_string(),
                avoid: "faces".to_string(),
                font_name: None,
                font_size: None,
                color: None,
                alignment: None,
            }],
        },
        ProjectActionError::NotTextOverlayItem("item-1".to_string()),
    );
}

#[test]
fn edit_transcript_words_rejects_invalid_repairs_without_mutating_project() {
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTranscriptWords { edits: vec![] },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTranscriptWords {
            edits: vec![ProjectActionTranscriptWordEdit {
                transcript_id: "missing-transcript".to_string(),
                word_index: 0,
                text: Some("Creator".to_string()),
                start_seconds: None,
                end_seconds: None,
                repair_id: "repair-1".to_string(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
            }],
        },
        ProjectActionError::TranscriptNotFound("missing-transcript".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTranscriptWords {
            edits: vec![ProjectActionTranscriptWordEdit {
                transcript_id: "transcript-media-1".to_string(),
                word_index: 99,
                text: Some("Creator".to_string()),
                start_seconds: None,
                end_seconds: None,
                repair_id: "repair-2".to_string(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
            }],
        },
        ProjectActionError::TranscriptWordNotFound {
            transcript_id: "transcript-media-1".to_string(),
            word_index: 99,
        },
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTranscriptWords {
            edits: vec![ProjectActionTranscriptWordEdit {
                transcript_id: "transcript-media-1".to_string(),
                word_index: 1,
                text: Some("   ".to_string()),
                start_seconds: None,
                end_seconds: None,
                repair_id: "repair-3".to_string(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
            }],
        },
        ProjectActionError::EmptyTranscriptText,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::EditTranscriptWords {
            edits: vec![ProjectActionTranscriptWordEdit {
                transcript_id: "transcript-media-1".to_string(),
                word_index: 1,
                text: Some("Creator".to_string()),
                start_seconds: Some(0.2),
                end_seconds: Some(0.95),
                repair_id: "repair-4".to_string(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
            }],
        },
        ProjectActionError::NonMonotonicTranscriptTiming("transcript-media-1".to_string()),
    );
}

#[test]
fn split_items_rejects_invalid_splits_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::SplitItems { splits: vec![] },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "item-1".to_string(),
                new_item_id: "item-1-b".to_string(),
                split_seconds: 0.0,
            }],
        },
        ProjectActionError::SplitPointOutsideItem("item-1".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "item-1".to_string(),
                new_item_id: "item-1".to_string(),
                split_seconds: 1.0,
            }],
        },
        ProjectActionError::DuplicateItemId("item-1".to_string()),
    );

    project.timeline.tracks[0].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "item-1".to_string(),
                new_item_id: "item-1-b".to_string(),
                split_seconds: 1.0,
            }],
        },
        ProjectActionError::TrackLocked("track-video".to_string()),
    );
}

#[test]
fn trim_items_rejects_invalid_trims_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::TrimItems { trims: vec![] },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 0.0,
                source_in: Some(0.0),
                source_out: Some(0.0),
            }],
        },
        ProjectActionError::NonPositiveDuration,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 3.0,
                source_in: Some(2.0),
                source_out: Some(6.0),
            }],
        },
        ProjectActionError::SourceRangeDurationMismatch("item-1".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source_in: Some(11.0),
                source_out: Some(13.0),
            }],
        },
        ProjectActionError::SourceRangeOutsideMedia("media-1".to_string()),
    );

    project.timeline.tracks[0].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.0,
                duration_seconds: 2.0,
                source_in: Some(1.0),
                source_out: Some(3.0),
            }],
        },
        ProjectActionError::TrackLocked("track-video".to_string()),
    );
}

#[test]
fn insert_items_rejects_invalid_inputs_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::InsertItems {
            target_track_id: "track-video".to_string(),
            insert_seconds: 1.0,
            items: vec![],
        },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::InsertItems {
            target_track_id: "track-video".to_string(),
            insert_seconds: -0.1,
            items: vec![video_item("item-inserted", "media-1", 0.0)],
        },
        ProjectActionError::NegativeTime,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::InsertItems {
            target_track_id: "track-captions".to_string(),
            insert_seconds: 1.0,
            items: vec![video_item("item-inserted", "media-1", 0.0)],
        },
        ProjectActionError::TrackTypeMismatch {
            item_kind: TimelineItemKind::VideoClip,
            track_kind: TrackKind::Caption,
        },
    );

    project.timeline.tracks[0].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::InsertItems {
            target_track_id: "track-video".to_string(),
            insert_seconds: 1.0,
            items: vec![video_item("item-inserted", "media-1", 0.0)],
        },
        ProjectActionError::TrackLocked("track-video".to_string()),
    );
}

#[test]
fn reorder_items_rejects_invalid_inputs_without_mutating_project() {
    let mut project = sample_project();
    project.timeline.tracks[0]
        .items
        .push(video_item("item-a", "media-1", 5.0));

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-video".to_string(),
                item_ids: vec![],
                start_seconds: 0.0,
                gap_seconds: 0.0,
            },
        },
        ProjectActionError::EmptyItems,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-video".to_string(),
                item_ids: vec!["item-1".to_string()],
                start_seconds: 0.0,
                gap_seconds: -0.1,
            },
        },
        ProjectActionError::NegativeTime,
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-video".to_string(),
                item_ids: vec!["item-1".to_string(), "item-1".to_string()],
                start_seconds: 0.0,
                gap_seconds: 0.0,
            },
        },
        ProjectActionError::DuplicateItemId("item-1".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-captions".to_string(),
                item_ids: vec!["item-1".to_string()],
                start_seconds: 0.0,
                gap_seconds: 0.0,
            },
        },
        ProjectActionError::TrackTypeMismatch {
            item_kind: TimelineItemKind::VideoClip,
            track_kind: TrackKind::Caption,
        },
    );

    project.timeline.tracks[0].locked = true;
    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-video".to_string(),
                item_ids: vec!["item-1".to_string(), "item-a".to_string()],
                start_seconds: 0.0,
                gap_seconds: 0.0,
            },
        },
        ProjectActionError::TrackLocked("track-video".to_string()),
    );
}

#[test]
fn move_and_remove_reject_missing_items_without_mutating_project() {
    let mut project = sample_project();

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::MoveItems {
            moves: vec![ProjectActionMove {
                item_id: "missing-item".to_string(),
                target_track_id: "track-video".to_string(),
                start_seconds: 1.0,
            }],
        },
        ProjectActionError::ItemNotFound("missing-item".to_string()),
    );

    assert_action_error_leaves_project_unchanged(
        &mut project,
        ProjectAction::RemoveItems {
            item_ids: vec!["missing-item".to_string()],
        },
        ProjectActionError::ItemNotFound("missing-item".to_string()),
    );
}
