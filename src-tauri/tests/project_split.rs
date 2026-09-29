use std::fs;

use video_creater_lib::project::action::{
    ProjectAction, ProjectActionCaptionRepair, ProjectActionGeneratedAsset,
    ProjectActionGeneratedAssetOutput, ProjectActionGeneratedAssetReferences,
    ProjectActionGeneratedAssetSettings, ProjectActionGenerationModel, ProjectActionMove,
    ProjectActionReorder, ProjectActionResize, ProjectActionSplit,
    ProjectActionTemplateOverrideUpdate, ProjectActionTranscriptWordEdit, ProjectActionTrim,
};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaAnalysisMoment, MediaAsset,
    MediaFolder, MediaKind, MediaSilenceRange, ProjectExportArtifact, ProjectExportArtifactKind,
    ProjectRenderReport, ProjectTemplateOverride, ProjectTimeline, RenderReportCheckStatus,
    RenderReportStatus, RenderReportStreams, TimelineItem, TimelineItemKind, TimelineSource,
    TimelineTransition, Transcript, TranscriptRepair, TranscriptWord, TransitionKind,
};
use video_creater_lib::project::split::{
    apply_agent_session_action, apply_project_action_to_split_project,
    apply_project_actions_to_split_project, default_split_manifest_for_project,
    load_agent_session_manifest, load_split_project, migrate_single_file_project,
    resolve_project_relative_path, save_split_project, split_project_manifest_path,
    validate_split_project, AgentSessionAction, SplitProjectError, SplitProjectLayout,
};

#[test]
fn named_agent_sessions_persist_reload_delete_restore_and_isolate_projects() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    let timestamp = "2026-07-11T12:00:00Z".to_string();
    let created = apply_agent_session_action(
        dir.path(),
        &project.id,
        AgentSessionAction::Create {
            id: "session-1".to_string(),
            title: "Launch cut".to_string(),
            thread_id: Some("thread-1".to_string()),
            timestamp: timestamp.clone(),
        },
    )
    .expect("create session");
    assert_eq!(created.active_session_id.as_deref(), Some("session-1"));
    let renamed = apply_agent_session_action(
        dir.path(),
        &project.id,
        AgentSessionAction::Rename {
            session_id: "session-1".to_string(),
            title: "Launch selects".to_string(),
            timestamp: timestamp.clone(),
        },
    )
    .expect("rename session");
    assert_eq!(renamed.sessions[0].title, "Launch selects");
    apply_agent_session_action(
        dir.path(),
        &project.id,
        AgentSessionAction::Delete {
            session_id: "session-1".to_string(),
            timestamp: timestamp.clone(),
        },
    )
    .expect("delete session");
    let deleted = load_agent_session_manifest(dir.path()).expect("reload deleted session state");
    assert!(deleted.sessions.is_empty());
    assert_eq!(deleted.deleted_sessions[0].id, "session-1");
    let restored = apply_agent_session_action(
        dir.path(),
        &project.id,
        AgentSessionAction::Restore {
            session_id: "session-1".to_string(),
            timestamp,
        },
    )
    .expect("restore session");
    assert_eq!(restored.active_session_id.as_deref(), Some("session-1"));
    assert!(dir.path().join("context/agent-sessions.json").exists());
    let error = apply_agent_session_action(
        dir.path(),
        "another-project",
        AgentSessionAction::Select {
            session_id: "session-1".to_string(),
        },
    )
    .expect_err("project isolation must reject mismatched project id");
    assert!(error.to_string().contains("does not match"));
}
use video_creater_lib::project::storage::save_project;

fn generated_asset() -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: "slow push-in on the product".to_string(),
        model: GenerationModel {
            provider: "seedance".to_string(),
            id: "seedance-2-fast".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["media-1".to_string()],
            first_frame_media_id: Some("media-1".to_string()),
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-shot-1-output".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1280,
            height: 720,
            duration_seconds: 4.0,
            fps: 24.0,
        }],
        created_at: "2026-06-22T10:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_media_output() -> MediaAsset {
    MediaAsset {
        id: "generated-shot-1-output".to_string(),
        name: None,
        relative_path: "generated/generated-shot-1/output.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 4.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: None,
    }
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
        checks: std::collections::BTreeMap::from([
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

fn workflow_job() -> JobSummary {
    JobSummary {
        id: "generate-shot-1".to_string(),
        kind: "generate_media".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-22T10:00:00Z".to_string(),
        workflow: Some(
            video_creater_lib::project::model::TemporalWorkflowMetadata {
                workflow_id: "video-creater/project-test/generate-media/generate-shot-1"
                    .to_string(),
                workflow_type: "VideoCreaterGenerateMediaWorkflow".to_string(),
                task_queue: "video-creater-workflows".to_string(),
                run_id: None,
                activity_types: vec![
                    "BuildFalGenerationRequest".to_string(),
                    "RunFalGeneration".to_string(),
                ],
            },
        ),
        start_request: Some(
            video_creater_lib::project::model::TemporalWorkflowStartRequest {
                workflow_id: "video-creater/project-test/generate-media/generate-shot-1"
                    .to_string(),
                workflow_type: "VideoCreaterGenerateMediaWorkflow".to_string(),
                task_queue: "video-creater-workflows".to_string(),
                input: serde_json::json!({
                    "assetId": "generate-shot-1",
                    "providerCredentialEnvVar": "FAL_KEY",
                    "projectDir": "/tmp/hidden-project-path"
                }),
                search_attributes: serde_json::json!({
                    "jobId": "generate-shot-1"
                }),
                activity_types: vec![
                    "BuildFalGenerationRequest".to_string(),
                    "RunFalGeneration".to_string(),
                ],
                id_reuse_policy: "rejectDuplicate".to_string(),
            },
        ),
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    }
}

fn template_override_json() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
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
        "avoid": "full-width opaque black slabs, centered title-card layout, default-font template look, and long unmoving holds"
    })
}

fn template_override_update() -> ProjectActionTemplateOverrideUpdate {
    ProjectActionTemplateOverrideUpdate {
        template_id: "kinetic-lower-third-v1".to_string(),
        name: "Kinetic Lower Third".to_string(),
        fields: std::collections::BTreeMap::from([
            ("headline".to_string(), "Launch day".to_string()),
            (
                "subline".to_string(),
                "Built with Video Creater".to_string(),
            ),
        ]),
        style: std::collections::BTreeMap::from([
            ("accentColor".to_string(), serde_json::json!("#22d3ee")),
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

fn template_override() -> ProjectTemplateOverride {
    serde_json::from_value(template_override_json()).expect("template override")
}

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
        properties: std::collections::BTreeMap::new(),
    }
}

fn generated_edit_video_item(id: &str, media_id: &str, start_seconds: f64) -> TimelineItem {
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds,
        duration_seconds: 4.0,
        source: TimelineSource::Media {
            media_id: media_id.to_string(),
        },
        label: "Generated edit clip".to_string(),
        properties: std::collections::BTreeMap::from([(
            "generatedEdit".to_string(),
            serde_json::json!(true),
        )]),
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

#[test]
fn split_manifest_defaults_to_agent_editable_file_layout() {
    let project = sample_project();

    let manifest = default_split_manifest_for_project(&project);

    assert_eq!(manifest.schema_version, 2);
    assert_eq!(manifest.id, "project-test");
    assert_eq!(manifest.layout, SplitProjectLayout::Split);
    assert_eq!(manifest.files.timeline, "timeline.json");
    assert_eq!(manifest.files.media, "media/index.json");
    assert_eq!(manifest.files.transcripts, "transcripts");
    assert_eq!(manifest.files.templates, "templates");
    assert_eq!(manifest.files.generated, "generated");
    assert_eq!(manifest.files.renders, "renders");
    assert_eq!(manifest.files.logs, "logs");
    assert_eq!(manifest.render_settings, project.render_settings);
    assert_eq!(manifest.codex_thread_id, None);
    assert!(manifest.jobs.is_empty());
    assert!(manifest.export_artifacts.is_empty());
}

#[test]
fn split_path_resolution_rejects_absolute_and_parent_paths() {
    let dir = tempfile::tempdir().expect("project dir");

    let absolute = resolve_project_relative_path(dir.path(), "/tmp/outside.json")
        .expect_err("absolute path must be rejected");
    assert_eq!(
        absolute,
        SplitProjectError::UnsafeManifestPath {
            field: "path".to_string(),
            value: "/tmp/outside.json".to_string(),
        }
    );

    let parent = resolve_project_relative_path(dir.path(), "../outside.json")
        .expect_err("parent path must be rejected");
    assert_eq!(
        parent,
        SplitProjectError::UnsafeManifestPath {
            field: "path".to_string(),
            value: "../outside.json".to_string(),
        }
    );

    let resolved = resolve_project_relative_path(dir.path(), "timeline.json")
        .expect("relative path inside project");
    assert_eq!(resolved, dir.path().join("timeline.json"));
}

#[test]
fn split_manifest_serializes_with_camel_case_contract() {
    let project = sample_project();
    let manifest = default_split_manifest_for_project(&project);

    let json = serde_json::to_value(&manifest).expect("manifest json");

    assert_eq!(json["schemaVersion"], serde_json::json!(2));
    assert_eq!(json["layout"], serde_json::json!("split"));
    assert_eq!(json["codexThreadId"], serde_json::Value::Null);
    assert_eq!(
        json["files"]["timeline"],
        serde_json::json!("timeline.json")
    );
    assert_eq!(json["exportArtifacts"], serde_json::json!([]));
}

#[test]
fn split_module_does_not_create_files_during_manifest_build() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();

    let _manifest = default_split_manifest_for_project(&project);

    let entries = fs::read_dir(dir.path())
        .expect("read temp dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("dir entries");
    assert!(entries.is_empty());
}

#[test]
fn save_split_project_writes_manifest_timeline_media_and_transcript_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Hello".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.4,
            confidence: Some(0.95),
            speaker: None,
        }],
    });

    let report = save_split_project(dir.path(), &project).expect("save split project");

    assert_eq!(
        report.report.manifest_path,
        dir.path()
            .join("video-creater.project.json")
            .display()
            .to_string()
    );
    assert!(dir.path().join("video-creater.project.json").exists());
    assert!(dir.path().join("timeline.json").exists());
    assert!(dir.path().join("media/index.json").exists());
    assert!(dir.path().join("transcripts/media-1.json").exists());
    assert!(dir.path().join("templates").is_dir());
    assert!(dir.path().join("generated").is_dir());
    assert!(dir.path().join("renders").is_dir());
    assert!(dir.path().join("logs").is_dir());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));
}

#[test]
fn save_split_project_preserves_existing_safe_manifest_file_layout() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("initial split project save");

    let manifest_path = dir.path().join("video-creater.project.json");
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest json"))
            .expect("parse manifest json");
    let custom_layout = [
        ("timeline", "timeline.json", "edit/timeline/main.json"),
        ("media", "media/index.json", "library/media-index.json"),
        ("transcripts", "transcripts", "text/transcripts"),
        ("templates", "templates", "design/template-overrides"),
        ("generated", "generated", "ai/generated"),
        ("renders", "renders", "exports/render-reports"),
        ("logs", "logs", "audit/logs"),
    ];
    // A real custom-layout package keeps its canonical files where the manifest points. Saves
    // read that canonical state before replacing it, so relocate the files with the manifest.
    for (field, default_path, custom_path) in custom_layout {
        let target = dir.path().join(custom_path);
        fs::create_dir_all(target.parent().expect("custom path parent"))
            .expect("custom path parent directory");
        fs::rename(dir.path().join(default_path), &target).expect("relocate package file");
        manifest_json["files"][field] = serde_json::json!(custom_path);
    }
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json).expect("custom manifest json"),
    )
    .expect("write custom manifest");
    let relocated = load_split_project(dir.path()).expect("load custom layout project");
    assert_eq!(relocated.timeline, project.timeline);

    let mut edited = relocated.clone();
    edited.timeline.tracks[0].items[0].label = "Custom layout edit".to_string();
    let report = save_split_project(dir.path(), &edited).expect("save with custom paths");

    let saved_manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("saved manifest json"))
            .expect("parse saved manifest json");
    assert_eq!(
        saved_manifest["files"]["timeline"],
        serde_json::json!("edit/timeline/main.json")
    );
    assert_eq!(
        saved_manifest["files"]["media"],
        serde_json::json!("library/media-index.json")
    );
    assert_eq!(
        saved_manifest["files"]["generated"],
        serde_json::json!("ai/generated")
    );
    assert!(dir.path().join("edit/timeline/main.json").exists());
    assert!(dir.path().join("library/media-index.json").exists());
    assert!(dir.path().join("text/transcripts").is_dir());
    assert!(dir.path().join("design/template-overrides").is_dir());
    assert!(dir.path().join("ai/generated").is_dir());
    assert!(dir.path().join("exports/render-reports").is_dir());
    assert!(dir.path().join("audit/logs").is_dir());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("edit/timeline/main.json")));
    assert!(!dir.path().join("timeline.json").exists());
    assert!(!dir.path().join("media/index.json").exists());
    let reloaded = load_split_project(dir.path()).expect("reload custom layout project");
    assert_eq!(
        reloaded.timeline.tracks[0].items[0].label,
        "Custom layout edit"
    );
}

#[test]
fn load_split_project_reconstructs_runtime_video_project() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: Some("transcripts/media-1.raw.json".to_string()),
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Hello".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.4,
            confidence: Some(0.95),
            speaker: None,
        }],
    });

    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.id, project.id);
    assert_eq!(loaded.media, project.media);
    assert_eq!(loaded.timeline, project.timeline);
    assert_eq!(loaded.transcripts, project.transcripts);
    assert_eq!(loaded.render_settings, project.render_settings);
}

#[test]
fn split_project_save_and_load_round_trip_track_transitions() {
    let dir = tempfile::tempdir().expect("project dir");
    save_split_project(dir.path(), &sample_project()).expect("save split project");

    let result = apply_project_actions_to_split_project(
        dir.path(),
        vec![
            ProjectAction::SplitItems {
                splits: vec![ProjectActionSplit {
                    item_id: "item-1".to_string(),
                    new_item_id: "item-1-b".to_string(),
                    split_seconds: 2.0,
                }],
            },
            ProjectAction::AddTransition {
                track_id: "track-video".to_string(),
                transition: TimelineTransition {
                    id: "dip-1".to_string(),
                    left_item_id: "item-1".to_string(),
                    right_item_id: "item-1-b".to_string(),
                    kind: TransitionKind::DipToWhite,
                    duration_seconds: 1.0,
                },
            },
        ],
    )
    .expect("add transition in split project");
    assert_eq!(result.project.timeline.tracks[0].transitions.len(), 1);

    let timeline_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("timeline.json")).expect("read timeline.json"),
    )
    .expect("parse timeline.json");
    assert_eq!(
        timeline_json["tracks"][0]["transitions"],
        serde_json::json!([{
            "id": "dip-1",
            "leftItemId": "item-1",
            "rightItemId": "item-1-b",
            "kind": "dipToWhite",
            "durationSeconds": 1.0
        }])
    );
    assert!(timeline_json["tracks"][1].get("transitions").is_none());

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.timeline, result.project.timeline);
    assert_eq!(
        reloaded.timeline.tracks[0].transitions,
        result.project.timeline.tracks[0].transitions
    );
    let active = reloaded
        .timelines
        .iter()
        .find(|entry| Some(&entry.id) == reloaded.active_timeline_id.as_ref())
        .expect("active timeline entry");
    assert_eq!(active.timeline.tracks[0].transitions.len(), 1);

    let moved = apply_project_actions_to_split_project(
        dir.path(),
        vec![ProjectAction::MoveItems {
            moves: vec![ProjectActionMove {
                item_id: "item-1-b".to_string(),
                target_track_id: "track-video".to_string(),
                start_seconds: 6.0,
            }],
        }],
    )
    .expect("move the right clip away in split project");
    assert!(moved.project.timeline.tracks[0].transitions.is_empty());
    let reloaded = load_split_project(dir.path()).expect("reload after move");
    assert!(reloaded.timeline.tracks[0].transitions.is_empty());
}

#[test]
fn load_split_project_imports_native_palmier_package_generation_input() {
    let dir = tempfile::tempdir().expect("palmier package dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    fs::write(dir.path().join("media/generated-product.mp4"), b"mock mp4").expect("media file");
    fs::write(dir.path().join("media/reference.png"), b"mock png").expect("reference file");

    fs::write(
        dir.path().join("project.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "timelines": [{
                "id": "timeline-palmier-1",
                "name": "Hero timeline",
                "fps": 24,
                "width": 1280,
                "height": 720,
                "settingsConfigured": true,
                "tracks": []
            }],
            "activeTimelineId": "timeline-palmier-1"
        }))
        .expect("project json"),
    )
    .expect("write project json");
    fs::write(
        dir.path().join("media.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "version": 2,
            "folders": [
                {
                    "id": "folder-root",
                    "name": "Root",
                    "parentFolderId": null
                },
                {
                    "id": "folder-generations",
                    "name": "Generations",
                    "parentFolderId": "folder-root"
                }
            ],
            "entries": [
                {
                    "id": "reference-image-1",
                    "name": "Reference frame",
                    "type": "image",
                    "source": { "project": { "relativePath": "media/reference.png" } },
                    "duration": 0,
                    "sourceWidth": 1024,
                    "sourceHeight": 1024,
                    "sourceFPS": null,
                    "folderId": "folder-generations"
                },
                {
                    "id": "generated-output-1",
                    "name": "Generated product push",
                    "type": "video",
                    "source": { "project": { "relativePath": "media/generated-product.mp4" } },
                    "duration": 4,
                    "sourceWidth": 1280,
                    "sourceHeight": 720,
                    "sourceFPS": 24,
                    "folderId": "folder-generations",
                    "generationInput": {
                        "prompt": "floating product shot with crisp rim light",
                        "model": "fal-ai/wan-25-preview/text-to-video",
                        "duration": 4,
                        "aspectRatio": "16:9",
                        "resolution": "720p",
                        "quality": "standard",
                        "generateAudio": true,
                        "referenceImageURLs": ["https://provider.example/reference.png"],
                        "referenceImageAssetIds": ["reference-image-1"],
                        "createdAt": "2026-06-23T12:00:00Z",
                        "backendJobId": "palmier-job-1",
                        "outputIndex": 0,
                        "resultURLs": ["https://provider.example/generated-product.mp4"]
                    }
                }
            ]
        }))
        .expect("media json"),
    )
    .expect("write media json");

    let loaded = load_split_project(dir.path()).expect("load native Palmier package");

    assert_eq!(loaded.name, "Hero timeline");
    assert_eq!(loaded.render_settings.width, 1280);
    assert_eq!(loaded.render_settings.height, 720);
    assert_eq!(loaded.render_settings.fps, 24.0);
    assert_eq!(loaded.media_folders.len(), 2);
    assert_eq!(
        loaded.media_folders[1].parent_id.as_deref(),
        Some("folder-root")
    );
    assert_eq!(loaded.media.len(), 2);
    let generated_media = loaded
        .media
        .iter()
        .find(|asset| asset.id == "generated-output-1")
        .expect("generated media");
    assert_eq!(generated_media.kind, MediaKind::Generated);
    assert_eq!(generated_media.relative_path, "media/generated-product.mp4");
    assert_eq!(
        generated_media.folder_id.as_deref(),
        Some("folder-generations")
    );

    assert_eq!(loaded.generated_assets.len(), 1);
    let generated = &loaded.generated_assets[0];
    assert_eq!(generated.id, "palmier-generated-output-1");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);
    assert_eq!(generated.name.as_deref(), Some("Generated product push"));
    assert_eq!(
        generated.prompt,
        "floating product shot with crisp rim light"
    );
    assert_eq!(generated.model.provider, "fal.ai");
    assert_eq!(generated.model.id, "fal-ai/wan-25-preview/text-to-video");
    assert_eq!(
        generated.references.reference_image_media_refs,
        ["reference-image-1"]
    );
    assert_eq!(
        generated.references.provider_input_urls,
        ["https://provider.example/reference.png"]
    );
    assert_eq!(generated.settings.duration_seconds, Some(4.0));
    assert_eq!(generated.settings.aspect_ratio.as_deref(), Some("16:9"));
    assert_eq!(generated.settings.resolution.as_deref(), Some("720p"));
    assert_eq!(generated.settings.quality.as_deref(), Some("standard"));
    assert_eq!(generated.settings.generate_audio, Some(true));
    assert_eq!(generated.outputs.len(), 1);
    assert_eq!(generated.outputs[0].media_id, "generated-output-1");
    assert_eq!(
        generated.outputs[0].source_url.as_deref(),
        Some("https://provider.example/generated-product.mp4")
    );
}

#[test]
fn load_split_project_imports_native_palmier_clip_source_ranges_with_speed() {
    let dir = tempfile::tempdir().expect("palmier package dir");
    fs::create_dir_all(dir.path().join("media")).expect("media dir");
    fs::write(dir.path().join("media/source.mp4"), b"mock mp4").expect("media file");

    fs::write(
        dir.path().join("project.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "timelines": [{
                "id": "timeline-palmier-1",
                "name": "Trimmed retime",
                "fps": 24,
                "width": 1920,
                "height": 1080,
                "settingsConfigured": true,
                "tracks": [{
                    "id": "track-video-1",
                    "type": "video",
                    "muted": false,
                    "hidden": false,
                    "clips": [{
                        "id": "clip-retimed-1",
                        "mediaRef": "source-video-1",
                        "mediaType": "video",
                        "sourceClipType": "video",
                        "startFrame": 48,
                        "durationFrames": 96,
                        "trimStartFrame": 24,
                        "trimEndFrame": 12,
                        "speed": 2.0,
                        "linkGroupId": "link-av-1"
                    }]
                }]
            }],
            "activeTimelineId": "timeline-palmier-1"
        }))
        .expect("project json"),
    )
    .expect("write project json");
    fs::write(
        dir.path().join("media.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "version": 2,
            "folders": [],
            "entries": [{
                "id": "source-video-1",
                "name": "Source video",
                "type": "video",
                "source": { "project": { "relativePath": "media/source.mp4" } },
                "duration": 12,
                "sourceWidth": 1920,
                "sourceHeight": 1080,
                "sourceFPS": 24,
                "folderId": null
            }]
        }))
        .expect("media json"),
    )
    .expect("write media json");

    let loaded = load_split_project(dir.path()).expect("load native Palmier package");
    let item = &loaded.timeline.tracks[0].items[0];

    assert_eq!(item.id, "clip-retimed-1");
    assert_eq!(item.start_seconds, 2.0);
    assert_eq!(item.duration_seconds, 4.0);
    assert_eq!(
        item.source,
        TimelineSource::Media {
            media_id: "source-video-1".to_string()
        }
    );
    assert_eq!(item.properties["sourceIn"], serde_json::json!(1.0));
    assert_eq!(item.properties["sourceOut"], serde_json::json!(9.0));
    assert_eq!(item.properties["trimStartFrame"], serde_json::json!(24));
    assert_eq!(item.properties["trimEndFrame"], serde_json::json!(12));
    assert_eq!(item.properties["speed"], serde_json::json!(2.0));
    assert_eq!(
        item.properties["linkGroupId"],
        serde_json::json!("link-av-1")
    );
}

#[test]
fn save_and_load_split_project_preserves_media_library_folders() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media_folders = vec![
        MediaFolder {
            id: "folder-broll".to_string(),
            name: "B-roll".to_string(),
            parent_id: None,
        },
        MediaFolder {
            id: "folder-generated".to_string(),
            name: "Generated selects".to_string(),
            parent_id: None,
        },
    ];
    project.media[0].folder_id = Some("folder-broll".to_string());
    project.media.push(MediaAsset {
        id: "generated-shot-1-output".to_string(),
        name: None,
        relative_path: "generated/generated-shot-1/output.mp4".to_string(),
        kind: MediaKind::Generated,
        duration_seconds: 4.0,
        width: Some(1280),
        height: Some(720),
        fps: Some(24.0),
        folder_id: Some("folder-generated".to_string()),
    });

    save_split_project(dir.path(), &project).expect("save split project");

    let media_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("media/index.json")).expect("media index file"),
    )
    .expect("media index json");
    assert_eq!(
        media_json["folders"][1]["name"],
        serde_json::json!("Generated selects")
    );
    assert_eq!(
        media_json["assets"][0]["folderId"],
        serde_json::json!("folder-broll")
    );

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.media_folders, project.media_folders);
    assert_eq!(loaded.media, project.media);
}

#[test]
fn save_and_load_split_project_preserves_media_analysis_moments() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media_analysis = vec![MediaAnalysisMoment {
        media_id: "media-1".to_string(),
        source_in: 42.0,
        source_out: 45.5,
        visual_action_score: 0.94,
        audio_energy_score: 0.88,
        label: "real media analysis: fast product handling with music hit".to_string(),
    }];

    save_split_project(dir.path(), &project).expect("save split project");

    let media_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("media/index.json")).expect("media index file"),
    )
    .expect("media index json");
    assert_eq!(
        media_json["analysis"][0]["mediaId"],
        serde_json::json!("media-1")
    );
    assert_eq!(
        media_json["analysis"][0]["visualActionScore"],
        serde_json::json!(0.94)
    );

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.media_analysis, project.media_analysis);
}

#[test]
fn save_and_load_split_project_preserves_media_silence_ranges() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media_silence_ranges = vec![MediaSilenceRange {
        media_id: "media-1".to_string(),
        source_in: 2.0,
        source_out: 4.5,
        confidence: 0.91,
        label: "quiet speech-free section".to_string(),
    }];

    save_split_project(dir.path(), &project).expect("save split project");

    let media_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("media/index.json")).expect("media index file"),
    )
    .expect("media index json");
    assert_eq!(
        media_json["silenceRanges"][0]["mediaId"],
        serde_json::json!("media-1")
    );
    assert_eq!(
        media_json["silenceRanges"][0]["sourceIn"],
        serde_json::json!(2.0)
    );

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.media_silence_ranges, project.media_silence_ranges);
}

#[test]
fn save_split_project_removes_stale_transcript_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Remove me".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.4,
            confidence: Some(0.95),
            speaker: None,
        }],
    });
    save_split_project(dir.path(), &project).expect("save split project with transcript");
    assert!(dir.path().join("transcripts/media-1.json").exists());

    project.transcripts.clear();
    let report =
        save_split_project(dir.path(), &project).expect("save split project without transcript");

    assert!(!dir.path().join("transcripts/media-1.json").exists());
    assert!(report
        .report
        .removed_files
        .iter()
        .any(|path| path.ends_with("transcripts/media-1.json")));

    let loaded = load_split_project(dir.path()).expect("load split project");
    assert!(loaded.transcripts.is_empty());
}

#[test]
fn save_split_project_removes_stale_template_override_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.template_overrides.push(template_override());
    save_split_project(dir.path(), &project).expect("save split project with template");
    assert!(dir
        .path()
        .join("templates/kinetic-lower-third-v1.json")
        .exists());

    project.template_overrides.clear();
    let report =
        save_split_project(dir.path(), &project).expect("save split project without template");

    assert!(!dir
        .path()
        .join("templates/kinetic-lower-third-v1.json")
        .exists());
    assert!(report
        .report
        .removed_files
        .iter()
        .any(|path| path.ends_with("templates/kinetic-lower-third-v1.json")));
    let loaded = load_split_project(dir.path()).expect("load split project");
    assert!(loaded.template_overrides.is_empty());
}

#[test]
fn save_split_project_removes_stale_generated_asset_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project with generated asset");
    assert!(dir
        .path()
        .join("generated/generated-shot-1/asset.json")
        .exists());

    project.generated_assets.clear();
    project
        .media
        .retain(|media| media.id != "generated-shot-1-output");
    let report = save_split_project(dir.path(), &project)
        .expect("save split project without generated asset");

    assert!(!dir
        .path()
        .join("generated/generated-shot-1/asset.json")
        .exists());
    assert!(report
        .report
        .removed_files
        .iter()
        .any(|path| path.ends_with("generated/generated-shot-1/asset.json")));
    let loaded = load_split_project(dir.path()).expect("load split project");
    assert!(loaded.generated_assets.is_empty());
}

#[test]
fn save_split_project_removes_stale_render_report_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project with render report");
    assert!(dir
        .path()
        .join("renders/render-draft-1/report.json")
        .exists());

    project.render_reports.clear();
    let report =
        save_split_project(dir.path(), &project).expect("save split project without render report");

    assert!(!dir
        .path()
        .join("renders/render-draft-1/report.json")
        .exists());
    assert!(report
        .report
        .removed_files
        .iter()
        .any(|path| path.ends_with("renders/render-draft-1/report.json")));
    let loaded = load_split_project(dir.path()).expect("load split project");
    assert!(loaded.render_reports.is_empty());
}

#[test]
fn apply_project_action_to_split_project_persists_updated_timeline_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::ResizeItems {
            resizes: vec![ProjectActionResize {
                item_id: "item-1".to_string(),
                duration_seconds: 2.5,
            }],
        },
    )
    .expect("apply project action to split project");

    assert_eq!(result.project.timeline.duration_seconds, 2.5);
    assert_eq!(
        result.project.timeline.tracks[0].items[0].duration_seconds,
        2.5
    );
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.timeline.duration_seconds, 2.5);
    assert_eq!(reloaded.timeline.tracks[0].items[0].duration_seconds, 2.5);
}

#[test]
fn validate_split_project_rejects_cyclic_nested_timeline_sources() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source = TimelineSource::Timeline {
        timeline_id: "alternate".to_string(),
    };
    let mut alternate = project.timeline.clone();
    alternate.tracks[0].items[0].source = TimelineSource::Timeline {
        timeline_id: "main".to_string(),
    };
    project.timelines[0].timeline = project.timeline.clone();
    project.timelines.push(ProjectTimeline {
        id: "alternate".to_string(),
        name: "Alternate cut".to_string(),
        timeline: alternate,
    });
    save_split_project(dir.path(), &project).expect("save cyclic nested timelines");

    let validation = validate_split_project(dir.path()).expect("validate split project");
    assert!(!validation.ok);
    assert!(validation
        .issues
        .iter()
        .any(|issue| issue.message.contains("timeline nesting cycle")));
}

#[test]
fn apply_project_action_to_split_project_persists_inserted_items() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut later = project.timeline.tracks[0].items[0].clone();
    later.id = "item-later".to_string();
    later.start_seconds = 5.0;
    project.timeline.tracks[0].items.push(later);
    project.timeline.duration_seconds = 9.0;
    save_split_project(dir.path(), &project).expect("save split project");

    let mut inserted = project.timeline.tracks[0].items[0].clone();
    inserted.id = "item-inserted".to_string();

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::InsertItems {
            target_track_id: "track-video".to_string(),
            insert_seconds: 2.0,
            items: vec![inserted],
        },
    )
    .expect("insert item in split project");

    assert_eq!(result.project.timeline.tracks[0].items.len(), 4);
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let items = &reloaded.timeline.tracks[0].items;
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "item-1",
            "item-inserted",
            "item-1-overwrite-2000",
            "item-later",
        ]
    );
    assert_eq!(items[0].duration_seconds, 2.0);
    assert_eq!(items[1].start_seconds, 2.0);
    assert_eq!(items[2].start_seconds, 6.0);
    assert_eq!(items[2].duration_seconds, 2.0);
    assert_eq!(items[3].start_seconds, 9.0);
}

#[test]
fn apply_project_action_to_split_project_persists_reordered_items() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut second = project.timeline.tracks[0].items[0].clone();
    second.id = "item-second".to_string();
    second.start_seconds = 5.0;
    second.duration_seconds = 2.0;
    project.timeline.tracks[0].items.push(second);
    project.timeline.duration_seconds = 7.0;
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::ReorderItems {
            reorder: ProjectActionReorder {
                target_track_id: "track-video".to_string(),
                item_ids: vec!["item-second".to_string(), "item-1".to_string()],
                start_seconds: 0.5,
                gap_seconds: 0.5,
            },
        },
    )
    .expect("reorder split project items");

    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let items = &reloaded.timeline.tracks[0].items;
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["item-second", "item-1"]
    );
    assert_eq!(items[0].start_seconds, 0.5);
    assert_eq!(items[1].start_seconds, 3.0);
    assert_eq!(reloaded.timeline.duration_seconds, 7.0);
}

#[test]
fn apply_project_action_to_split_project_persists_split_items() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::SplitItems {
            splits: vec![ProjectActionSplit {
                item_id: "item-1".to_string(),
                new_item_id: "item-1-b".to_string(),
                split_seconds: 1.25,
            }],
        },
    )
    .expect("split item in split project");

    assert_eq!(result.project.timeline.tracks[0].items.len(), 2);
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let items = &reloaded.timeline.tracks[0].items;
    assert_eq!(items[0].id, "item-1");
    assert_eq!(items[0].duration_seconds, 1.25);
    assert_eq!(items[1].id, "item-1-b");
    assert_eq!(items[1].start_seconds, 1.25);
    assert_eq!(items[1].duration_seconds, 2.75);
}

#[test]
fn apply_project_action_to_split_project_persists_trimmed_source_ranges() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::TrimItems {
            trims: vec![ProjectActionTrim {
                item_id: "item-1".to_string(),
                start_seconds: 0.5,
                duration_seconds: 2.5,
                source_in: Some(1.0),
                source_out: Some(3.5),
            }],
        },
    )
    .expect("trim item in split project");

    assert_eq!(
        result.project.timeline.tracks[0].items[0].start_seconds,
        0.5
    );
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let item = &reloaded.timeline.tracks[0].items[0];
    assert_eq!(item.start_seconds, 0.5);
    assert_eq!(item.duration_seconds, 2.5);
    assert_eq!(item.properties["sourceIn"], serde_json::json!(1.0));
    assert_eq!(item.properties["sourceOut"], serde_json::json!(3.5));
}

#[test]
fn apply_project_action_to_split_project_persists_transcript_word_repairs() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Creater".to_string(),
            start_seconds: 0.5,
            end_seconds: 0.9,
            confidence: Some(0.88),
            speaker: None,
        }],
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::EditTranscriptWords {
            edits: vec![ProjectActionTranscriptWordEdit {
                transcript_id: "transcript-media-1".to_string(),
                word_index: 0,
                text: Some("Creator".to_string()),
                start_seconds: None,
                end_seconds: None,
                repair_id: "repair-1".to_string(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
            }],
        },
    )
    .expect("edit transcript word in split project");

    assert_eq!(result.project.transcripts[0].words[0].text, "Creator");
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("transcripts/media-1.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let transcript = &reloaded.transcripts[0];
    assert_eq!(transcript.words[0].text, "Creator");
    assert_eq!(transcript.repairs.len(), 1);
    assert_eq!(transcript.repairs[0].before.text, "Creater");
    assert_eq!(transcript.repairs[0].after.text, "Creator");
}

#[test]
fn apply_project_action_to_split_project_persists_caption_repairs() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[3]
        .items
        .push(caption_item("caption-1", 0.5));
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(dir.path(), caption_repair_action())
        .expect("apply caption repair in split project");

    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("timeline.json")));
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("transcripts/media-1.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let caption = &reloaded.timeline.tracks[3].items[0];
    assert_eq!(caption.start_seconds, 1.5);
    assert!((caption.duration_seconds - 0.4).abs() < f64::EPSILON);
    assert_eq!(
        caption.source,
        TimelineSource::Text {
            text: "Creator".to_string()
        }
    );
    assert_eq!(caption.properties["captionRepairId"], "repair-caption-1");

    let transcript = &reloaded.transcripts[0];
    assert_eq!(transcript.words[1].text, "Creator");
    assert_eq!(transcript.repairs.len(), 1);
    assert_eq!(transcript.repairs[0].id, "repair-caption-1");
}

#[test]
fn apply_project_action_to_split_project_persists_generated_asset_provenance() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::RecordGeneratedAsset {
            asset: Box::new(ProjectActionGeneratedAsset {
                id: "generated-shot-1".to_string(),
                kind: MediaKind::Generated,
                status: GeneratedAssetStatus::Completed,
                name: None,
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
                    width: Some(1280),
                    height: Some(720),
                    duration_seconds: Some(4.0),
                    fps: Some(24.0),
                    aspect_ratio: Some("auto".to_string()),
                    resolution: None,
                    generate_audio: None,
                    ..ProjectActionGeneratedAssetSettings::default()
                },
                outputs: vec![ProjectActionGeneratedAssetOutput {
                    media_id: "generated-shot-1-output".to_string(),
                    relative_path: "generated/generated-shot-1/output.mp4".to_string(),
                    source_url: None,
                    width: 1280,
                    height: 720,
                    duration_seconds: 4.0,
                    fps: 24.0,
                }],
                created_at: "2026-06-22T10:00:00Z".to_string(),
                parent_asset_id: None,
                retry_of_asset_id: None,
            }),
        },
    )
    .expect("record generated asset in split project");

    assert_eq!(result.project.generated_assets.len(), 1);
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| { path.ends_with("generated/generated-shot-1/asset.json") }));
    assert!(dir
        .path()
        .join("generated/generated-shot-1/asset.json")
        .exists());

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.generated_assets.len(), 1);
    assert_eq!(
        reloaded.generated_assets[0].prompt,
        "slow push-in on the product"
    );
    assert_eq!(
        reloaded.generated_assets[0]
            .settings
            .aspect_ratio
            .as_deref(),
        Some("auto")
    );
    assert!(reloaded
        .media
        .iter()
        .any(|media| media.id == "generated-shot-1-output" && media.kind == MediaKind::Generated));
}

#[test]
fn apply_project_action_to_split_project_completes_pending_generated_asset() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::RecordGeneratedAsset {
            asset: Box::new(ProjectActionGeneratedAsset {
                id: "generated-shot-1".to_string(),
                kind: MediaKind::Generated,
                status: GeneratedAssetStatus::Queued,
                name: None,
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
                    width: Some(1280),
                    height: Some(720),
                    duration_seconds: Some(4.0),
                    fps: Some(24.0),
                    aspect_ratio: Some("16:9".to_string()),
                    resolution: None,
                    generate_audio: None,
                    ..ProjectActionGeneratedAssetSettings::default()
                },
                outputs: Vec::new(),
                created_at: "2026-06-22T10:00:00Z".to_string(),
                parent_asset_id: None,
                retry_of_asset_id: None,
            }),
        },
    )
    .expect("record pending generated asset");

    let result = apply_project_action_to_split_project(
        dir.path(),
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
    .expect("complete pending generated asset");

    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("generated/generated-shot-1/asset.json")));

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    let generated = reloaded
        .generated_assets
        .iter()
        .find(|asset| asset.id == "generated-shot-1")
        .expect("generated asset");
    assert_eq!(generated.status, GeneratedAssetStatus::Completed);
    assert_eq!(generated.outputs.len(), 1);
    assert!(reloaded
        .media
        .iter()
        .any(|media| media.id == "generated-shot-1-output" && media.kind == MediaKind::Generated));
}

#[test]
fn apply_project_action_to_split_project_persists_render_reports() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::AttachRenderReport {
            report: ProjectRenderReport {
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
                checks: std::collections::BTreeMap::from([
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
            },
        },
    )
    .expect("attach render report in split project");

    assert_eq!(result.project.render_reports.len(), 1);
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("renders/render-draft-1/report.json")));
    assert!(dir
        .path()
        .join("renders/render-draft-1/report.json")
        .exists());

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.render_reports.len(), 1);
    assert_eq!(reloaded.render_reports[0].id, "render-draft-1");
    assert_eq!(reloaded.render_reports[0].duration_seconds, 42.5);
    assert_eq!(
        reloaded.render_reports[0].checks.get("visualFrameEvidence"),
        Some(&RenderReportCheckStatus::Passed)
    );
}

#[test]
fn save_and_load_split_project_preserves_export_artifacts_in_manifest() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());

    save_split_project(dir.path(), &project).expect("save split project with export artifact");

    let manifest_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("video-creater.project.json")).expect("manifest json"),
    )
    .expect("parse manifest json");
    assert_eq!(
        manifest_json["exportArtifacts"][0]["path"],
        serde_json::json!("exports/project-test-premiere.xml")
    );
    assert_eq!(
        manifest_json["exportArtifacts"][0]["format"],
        serde_json::json!("premiereXmeml")
    );

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.export_artifacts, project.export_artifacts);
}

#[test]
fn save_split_project_writes_export_artifact_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut earlier = export_artifact();
    earlier.id = "nle-export-premiere-1".to_string();
    earlier.created_at = "2026-06-23T12:00:00Z".to_string();
    let mut later = export_artifact();
    later.id = "mp4-export-draft-1".to_string();
    later.kind = ProjectExportArtifactKind::Mp4;
    later.format = "h264".to_string();
    later.path = "exports/project-test-draft.mp4".to_string();
    later.mime_type = "video/mp4".to_string();
    later.job_id = Some("render-draft-1".to_string());
    later.created_at = "2026-06-23T12:05:00Z".to_string();
    project.export_artifacts = vec![earlier, later];

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("exports/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("exports/index.json")));
    let index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(index_path).expect("export index json"))
            .expect("parse export index");

    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["artifacts"][0]["artifactId"],
        "mp4-export-draft-1"
    );
    assert_eq!(index_json["artifacts"][0]["kind"], "mp4");
    assert_eq!(index_json["artifacts"][0]["format"], "h264");
    assert_eq!(
        index_json["artifacts"][0]["path"],
        "exports/project-test-draft.mp4"
    );
    assert_eq!(index_json["artifacts"][0]["mimeType"], "video/mp4");
    assert_eq!(index_json["artifacts"][0]["jobId"], "render-draft-1");
    assert_eq!(
        index_json["artifacts"][0]["createdAt"],
        "2026-06-23T12:05:00Z"
    );
    assert_eq!(
        index_json["artifacts"][1]["artifactId"],
        "nle-export-premiere-1"
    );
}

#[test]
fn load_and_validate_split_project_ignore_export_artifact_index_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());

    save_split_project(dir.path(), &project).expect("save split project");

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.export_artifacts, project.export_artifacts);

    let validation = validate_split_project(dir.path()).expect("validate split project");
    assert!(validation.ok, "validation issues: {:?}", validation.issues);
}

#[test]
fn validate_split_project_reports_export_artifact_index_drift() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("exports/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("export index json"))
            .expect("parse export index");
    index_json["artifacts"][0]["kind"] = serde_json::json!("mp4");
    index_json["artifacts"][0]["format"] = serde_json::json!("staleFormat");
    index_json["artifacts"][0]["path"] = serde_json::json!("exports/stale-premiere.xml");
    index_json["artifacts"][0]["mimeType"] = serde_json::json!("video/mp4");
    index_json["artifacts"][0]["jobId"] = serde_json::json!("stale-job");
    index_json["artifacts"][0]["createdAt"] = serde_json::json!("2026-06-24T12:00:00Z");
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize export index json"),
    )
    .expect("write export index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1].kind"
            && issue.message.contains("must match export artifact sidecar")
            && issue.message.contains("nle_xml")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1].format"
            && issue.message.contains("must match export artifact sidecar")
            && issue.message.contains("premiereXmeml")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1].path"
            && issue.message.contains("must match export artifact sidecar")
            && issue.message.contains("exports/project-test-premiere.xml")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1].mimeType"
            && issue.message.contains("must match export artifact sidecar")
            && issue.message.contains("application/xml")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1].jobId"
            && issue.message.contains("must match export artifact sidecar")
            && issue.message.contains("nle-export-premiere-1")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1].createdAt"
            && issue.message.contains("must match export artifact sidecar")
            && issue.message.contains("2026-06-23T12:00:00Z")
    }));
}

#[test]
fn validate_split_project_reports_missing_export_artifact_index_entry() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("exports/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("export index json"))
            .expect("parse export index");
    index_json["artifacts"] = serde_json::json!([]);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize export index json"),
    )
    .expect("write export index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json artifacts[nle-export-premiere-1]"
            && issue
                .message
                .contains("missing export artifact index entry")
    }));
}

#[test]
fn validate_split_project_reports_invalid_export_artifact_index_json() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    fs::write(dir.path().join("exports/index.json"), "{not json").expect("write export index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/index.json"
            && issue
                .fix
                .contains("regenerate it from export artifact sidecars")
    }));
}

#[test]
fn save_split_project_writes_export_artifact_sidecars_for_file_editing() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let sidecar_path = dir
        .path()
        .join("exports/nle-export-premiere-1/artifact.json");
    assert!(sidecar_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("exports/nle-export-premiere-1/artifact.json")));
    let sidecar_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sidecar_path).expect("export sidecar json"))
            .expect("parse export sidecar");

    assert_eq!(sidecar_json["id"], "nle-export-premiere-1");
    assert_eq!(sidecar_json["kind"], "nle_xml");
    assert_eq!(sidecar_json["format"], "premiereXmeml");
    assert_eq!(
        sidecar_json["path"],
        serde_json::json!("exports/project-test-premiere.xml")
    );
}

#[test]
fn load_split_project_prefers_export_sidecars_over_manifest_mirror() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    let manifest_path = dir.path().join("video-creater.project.json");
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest json"))
            .expect("parse manifest");
    manifest_json["exportArtifacts"][0]["format"] = serde_json::json!("staleManifestFormat");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json).expect("serialize manifest json"),
    )
    .expect("write stale manifest");

    let sidecar_path = dir
        .path()
        .join("exports/nle-export-premiere-1/artifact.json");
    let mut sidecar_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar_path).expect("export sidecar json"))
            .expect("parse sidecar");
    sidecar_json["format"] = serde_json::json!("davinciResolveFcpXml");
    fs::write(
        sidecar_path,
        serde_json::to_string_pretty(&sidecar_json).expect("serialize sidecar json"),
    )
    .expect("write edited sidecar");

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.export_artifacts.len(), 1);
    assert_eq!(loaded.export_artifacts[0].format, "davinciResolveFcpXml");
}

#[test]
fn load_split_project_falls_back_to_manifest_export_artifacts_without_sidecars() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");
    fs::remove_file(
        dir.path()
            .join("exports/nle-export-premiere-1/artifact.json"),
    )
    .expect("remove export sidecar");

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.export_artifacts, project.export_artifacts);
}

#[test]
fn save_split_project_removes_stale_export_sidecar_without_deleting_exported_file() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project with export");
    fs::write(
        dir.path().join("exports/project-test-premiere.xml"),
        "<xml />",
    )
    .expect("write exported xml");

    project.export_artifacts.clear();
    let report =
        save_split_project(dir.path(), &project).expect("save split project without export");

    assert!(!dir
        .path()
        .join("exports/nle-export-premiere-1/artifact.json")
        .exists());
    assert!(dir
        .path()
        .join("exports/project-test-premiere.xml")
        .exists());
    assert!(report
        .report
        .removed_files
        .iter()
        .any(|path| path.ends_with("exports/nle-export-premiere-1/artifact.json")));
}

#[test]
fn validate_split_project_reports_invalid_export_artifact_sidecar_id() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    let sidecar_path = dir
        .path()
        .join("exports/nle-export-premiere-1/artifact.json");
    let mut sidecar_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar_path).expect("export sidecar json"))
            .expect("parse export sidecar");
    sidecar_json["id"] = serde_json::json!("bad/export");
    fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar_json).expect("serialize sidecar json"),
    )
    .expect("write invalid sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/nle-export-premiere-1/artifact.json id"
            && issue.message.contains("safe path segment")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/nle-export-premiere-1/artifact.json id"
            && issue.message.contains("must match containing directory")
    }));
}

#[test]
fn validate_split_project_reports_export_artifact_sidecar_path_escape() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    let sidecar_path = dir
        .path()
        .join("exports/nle-export-premiere-1/artifact.json");
    let mut sidecar_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar_path).expect("export sidecar json"))
            .expect("parse export sidecar");
    sidecar_json["path"] = serde_json::json!("../outside.xml");
    fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar_json).expect("serialize sidecar json"),
    )
    .expect("write invalid sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "exports/nle-export-premiere-1/artifact.json path"
            && issue.message.contains("unsafe project path")
    }));
}

#[test]
fn validate_split_project_falls_back_to_manifest_export_artifacts_without_sidecars() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut artifact = export_artifact();
    artifact.format = "".to_string();
    project.export_artifacts.push(artifact);
    save_split_project(dir.path(), &project).expect("save split project");
    fs::remove_file(
        dir.path()
            .join("exports/nle-export-premiere-1/artifact.json"),
    )
    .expect("remove export sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "video-creater.project.json exportArtifacts[0].format"
            && issue.message.contains("format is required")
    }));
}

#[test]
fn load_and_save_split_project_preserves_template_override_files() {
    let source_dir = tempfile::tempdir().expect("source project dir");
    let output_dir = tempfile::tempdir().expect("output project dir");
    let project = sample_project();
    save_split_project(source_dir.path(), &project).expect("save split project");
    fs::write(
        source_dir
            .path()
            .join("templates/kinetic-lower-third-v1.json"),
        serde_json::to_string_pretty(&template_override_json()).expect("template override json"),
    )
    .expect("write template override");

    let loaded = load_split_project(source_dir.path()).expect("load split project");
    let loaded_json = serde_json::to_value(&loaded).expect("loaded project json");

    assert_eq!(
        loaded_json["templateOverrides"][0]["templateId"],
        serde_json::json!("kinetic-lower-third-v1")
    );
    assert_eq!(
        loaded_json["templateOverrides"][0]["visualTreatment"],
        template_override_json()["visualTreatment"]
    );

    save_split_project(output_dir.path(), &loaded).expect("save loaded project");

    let saved_template_path = output_dir
        .path()
        .join("templates/kinetic-lower-third-v1.json");
    assert!(saved_template_path.exists());
    let saved_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(saved_template_path).expect("saved template"))
            .expect("saved template json");
    assert_eq!(
        saved_json["fields"]["headline"],
        serde_json::json!("Launch day")
    );
}

#[test]
fn save_split_project_writes_template_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.template_overrides.push(template_override());

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("templates/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("templates/index.json")));

    let index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(index_path).expect("template index"))
            .expect("template index json");
    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["templates"][0]["templateId"],
        serde_json::json!("kinetic-lower-third-v1")
    );
    assert_eq!(
        index_json["templates"][0]["path"],
        serde_json::json!("templates/kinetic-lower-third-v1.json")
    );
    assert_eq!(
        index_json["templates"][0]["visualTreatment"],
        template_override_json()["visualTreatment"]
    );
}

#[test]
fn load_and_validate_split_project_ignore_template_index_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.template_overrides.push(template_override());
    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");
    let report = validate_split_project(dir.path()).expect("validation report");

    assert_eq!(loaded.template_overrides.len(), 1);
    assert_eq!(
        loaded.template_overrides[0].template_id,
        "kinetic-lower-third-v1"
    );
    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn validate_split_project_reports_stale_template_index_visual_treatment() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.template_overrides.push(template_override());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("templates/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("template index"))
            .expect("template index json");
    index_json["templates"][0]["visualTreatment"] = serde_json::json!("stale visual treatment");
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize template index json"),
    )
    .expect("write template index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "templates/index.json templates[kinetic-lower-third-v1].visualTreatment"
            && issue.message.contains("compact lower-third")
            && issue.fix.contains("Regenerate templates/index.json")
    }));
}

#[test]
fn validate_split_project_reports_template_missing_from_index() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.template_overrides.push(template_override());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("templates/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("template index"))
            .expect("template index json");
    index_json["templates"] = serde_json::json!([]);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize template index json"),
    )
    .expect("write template index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "templates/index.json templates[kinetic-lower-third-v1]"
            && issue.message.contains("missing template index entry")
            && issue.fix.contains("Regenerate templates/index.json")
    }));
}

#[test]
fn save_split_project_writes_generated_asset_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut later_asset = generated_asset();
    later_asset.name = Some("B camera variation".to_string());
    later_asset.target_folder_id = Some("folder-generated".to_string());
    later_asset.placement_intent = Some("timeline".to_string());
    let mut earlier_asset = generated_asset();
    earlier_asset.id = "generated-shot-0".to_string();
    earlier_asset.outputs.clear();
    project.media.push(generated_media_output());
    project.generated_assets.push(later_asset);
    project.generated_assets.push(earlier_asset);

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("generated/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("generated/index.json")));

    let index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(index_path).expect("generated index"))
            .expect("generated index json");
    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["assets"][0]["assetId"],
        serde_json::json!("generated-shot-0")
    );
    assert_eq!(
        index_json["assets"][1]["assetId"],
        serde_json::json!("generated-shot-1")
    );
    assert_eq!(
        index_json["assets"][1]["path"],
        serde_json::json!("generated/generated-shot-1/asset.json")
    );
    assert_eq!(
        index_json["assets"][1]["name"],
        serde_json::json!("B camera variation")
    );
    assert_eq!(
        index_json["assets"][1]["status"],
        serde_json::json!("completed")
    );
    assert_eq!(
        index_json["assets"][1]["modelProvider"],
        serde_json::json!("seedance")
    );
    assert_eq!(
        index_json["assets"][1]["modelId"],
        serde_json::json!("seedance-2-fast")
    );
    assert_eq!(
        index_json["assets"][1]["targetFolderId"],
        serde_json::json!("folder-generated")
    );
    assert_eq!(
        index_json["assets"][1]["placementIntent"],
        serde_json::json!("timeline")
    );
    assert_eq!(index_json["assets"][1]["outputCount"], serde_json::json!(1));
    assert_eq!(
        index_json["assets"][1]["createdAt"],
        serde_json::json!("2026-06-22T10:00:00Z")
    );
}

#[test]
fn load_and_validate_split_project_ignore_generated_asset_index_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let asset = generated_asset();
    project.media.push(generated_media_output());
    project.generated_assets.push(asset.clone());
    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");
    let report = validate_split_project(dir.path()).expect("validation report");

    assert_eq!(loaded.generated_assets.len(), 1);
    assert_eq!(loaded.generated_assets[0], asset);
    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn validate_split_project_reports_stale_generated_asset_index_status() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("generated/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("generated index"))
            .expect("generated index json");
    index_json["assets"][0]["status"] = serde_json::json!("queued");
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize generated index json"),
    )
    .expect("write generated index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/index.json assets[generated-shot-1].status"
            && issue.message.contains("completed")
            && issue.fix.contains("Regenerate generated/index.json")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_missing_from_index() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("generated/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("generated index"))
            .expect("generated index json");
    index_json["assets"] = serde_json::json!([]);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize generated index json"),
    )
    .expect("write generated index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/index.json assets[generated-shot-1]"
            && issue
                .message
                .contains("missing generated asset index entry")
            && issue.fix.contains("Regenerate generated/index.json")
    }));
}

#[test]
fn save_split_project_writes_render_report_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut later_report = render_report();
    later_report.id = "render-draft-2".to_string();
    later_report.output_path = "renders/render-draft-2/output.mp4".to_string();
    later_report.artifacts = vec![
        "renders/render-draft-2/output.mp4".to_string(),
        "renders/render-draft-2/report.json".to_string(),
    ];
    later_report.log_path = "renders/render-draft-2/render.log".to_string();
    let mut earlier_report = render_report();
    earlier_report.id = "render-draft-0".to_string();
    earlier_report.status = RenderReportStatus::Failed;
    earlier_report.output_path = "renders/render-draft-0/output.mp4".to_string();
    earlier_report.artifacts = vec!["renders/render-draft-0/output.mp4".to_string()];
    earlier_report.log_path = "renders/render-draft-0/render.log".to_string();
    earlier_report
        .checks
        .insert("duration".to_string(), RenderReportCheckStatus::Failed);
    project.render_reports.push(later_report);
    project.render_reports.push(earlier_report);

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("renders/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("renders/index.json")));

    let index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(index_path).expect("render index"))
            .expect("render index json");
    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["reports"][0]["reportId"],
        serde_json::json!("render-draft-0")
    );
    assert_eq!(
        index_json["reports"][1]["reportId"],
        serde_json::json!("render-draft-2")
    );
    assert_eq!(
        index_json["reports"][1]["path"],
        serde_json::json!("renders/render-draft-2/report.json")
    );
    assert_eq!(
        index_json["reports"][1]["status"],
        serde_json::json!("completed")
    );
    assert_eq!(
        index_json["reports"][1]["outputPath"],
        serde_json::json!("renders/render-draft-2/output.mp4")
    );
    assert_eq!(
        index_json["reports"][1]["durationSeconds"],
        serde_json::json!(42.5)
    );
    assert_eq!(index_json["reports"][1]["video"], serde_json::json!(true));
    assert_eq!(index_json["reports"][1]["audio"], serde_json::json!(true));
    assert_eq!(index_json["reports"][1]["checkCount"], serde_json::json!(7));
    assert_eq!(
        index_json["reports"][0]["failedCheckCount"],
        serde_json::json!(1)
    );
    assert_eq!(
        index_json["reports"][1]["artifactCount"],
        serde_json::json!(2)
    );
    assert_eq!(
        index_json["reports"][1]["logPath"],
        serde_json::json!("renders/render-draft-2/render.log")
    );
    assert_eq!(
        index_json["reports"][1]["createdAt"],
        serde_json::json!("2026-06-22T10:00:00Z")
    );
}

#[test]
fn load_and_validate_split_project_ignore_render_report_index_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");
    let report = validate_split_project(dir.path()).expect("validation report");

    assert_eq!(loaded.render_reports.len(), 1);
    assert_eq!(loaded.render_reports[0].id, "render-draft-1");
    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn validate_split_project_reports_stale_render_report_index_status() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("renders/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("render index"))
            .expect("render index json");
    index_json["reports"][0]["status"] = serde_json::json!("failed");
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize render index json"),
    )
    .expect("write render index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/index.json reports[render-draft-1].status"
            && issue.message.contains("completed")
            && issue.fix.contains("Regenerate renders/index.json")
    }));
}

#[test]
fn validate_split_project_reports_render_report_missing_from_index() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("renders/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("render index"))
            .expect("render index json");
    index_json["reports"] = serde_json::json!([]);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize render index json"),
    )
    .expect("write render index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/index.json reports[render-draft-1]"
            && issue.message.contains("missing render report index entry")
            && issue.fix.contains("Regenerate renders/index.json")
    }));
}

#[test]
fn save_split_project_writes_workflow_job_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(JobSummary {
        id: "older-render-job".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Completed,
        updated_at: "2026-06-22T09:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    project.jobs.push(JobSummary {
        id: "generate-shot-1".to_string(),
        kind: "generate_media".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-22T10:00:00Z".to_string(),
        workflow: Some(
            video_creater_lib::project::model::TemporalWorkflowMetadata {
                workflow_id: "video-creater/project-test/generate-media/generate-shot-1"
                    .to_string(),
                workflow_type: "VideoCreaterGenerateMediaWorkflow".to_string(),
                task_queue: "video-creater-workflows".to_string(),
                run_id: None,
                activity_types: vec![
                    "BuildFalGenerationRequest".to_string(),
                    "RunFalGeneration".to_string(),
                ],
            },
        ),
        start_request: Some(
            video_creater_lib::project::model::TemporalWorkflowStartRequest {
                workflow_id: "video-creater/project-test/generate-media/generate-shot-1"
                    .to_string(),
                workflow_type: "VideoCreaterGenerateMediaWorkflow".to_string(),
                task_queue: "video-creater-workflows".to_string(),
                input: serde_json::json!({
                    "assetId": "generate-shot-1",
                    "providerCredentialEnvVar": "FAL_KEY",
                    "projectDir": "/tmp/hidden-project-path"
                }),
                search_attributes: serde_json::json!({
                    "jobId": "generate-shot-1"
                }),
                activity_types: vec![
                    "BuildFalGenerationRequest".to_string(),
                    "RunFalGeneration".to_string(),
                ],
                id_reuse_policy: "rejectDuplicate".to_string(),
            },
        ),
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("jobs/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("jobs/index.json")));

    let raw_index = fs::read_to_string(index_path).expect("workflow job index");
    assert!(!raw_index.contains("projectDir"));
    assert!(!raw_index.contains("searchAttributes"));
    assert!(!raw_index.contains("/tmp/hidden-project-path"));
    let index_json: serde_json::Value =
        serde_json::from_str(&raw_index).expect("workflow job index json");
    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["jobs"][0]["jobId"],
        serde_json::json!("generate-shot-1")
    );
    assert_eq!(
        index_json["jobs"][1]["jobId"],
        serde_json::json!("older-render-job")
    );
    assert_eq!(index_json["jobs"][0]["status"], serde_json::json!("queued"));
    assert_eq!(
        index_json["jobs"][0]["workflowId"],
        serde_json::json!("video-creater/project-test/generate-media/generate-shot-1")
    );
    assert_eq!(
        index_json["jobs"][0]["workflowType"],
        serde_json::json!("VideoCreaterGenerateMediaWorkflow")
    );
    assert_eq!(
        index_json["jobs"][0]["taskQueue"],
        serde_json::json!("video-creater-workflows")
    );
    assert_eq!(index_json["jobs"][0]["activityCount"], serde_json::json!(2));
    assert_eq!(
        index_json["jobs"][0]["hasStartRequest"],
        serde_json::json!(true)
    );
    assert_eq!(
        index_json["jobs"][0]["startRequestReady"],
        serde_json::json!(true)
    );
    assert_eq!(
        index_json["jobs"][0]["idReusePolicy"],
        serde_json::json!("rejectDuplicate")
    );
    assert!(
        index_json["jobs"][0]["credentialEnvVar"].is_null(),
        "agent discovery indexes must not persist credential environment names"
    );
}

#[test]
fn load_and_validate_split_project_ignore_workflow_job_index_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(JobSummary {
        id: "render-draft-1".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-23T12:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");
    let report = validate_split_project(dir.path()).expect("validation report");

    assert_eq!(loaded.jobs.len(), 1);
    assert_eq!(loaded.jobs[0].id, "render-draft-1");
    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn validate_split_project_reports_stale_workflow_job_index_status() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("jobs/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("workflow job index"))
            .expect("workflow job index json");
    index_json["jobs"][0]["status"] = serde_json::json!("completed");
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize workflow job index json"),
    )
    .expect("write workflow job index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "jobs/index.json jobs[generate-shot-1].status"
            && issue.message.contains("queued")
            && issue.fix.contains("Regenerate jobs/index.json")
    }));
}

#[test]
fn validate_split_project_reports_workflow_job_missing_from_index() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("jobs/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("workflow job index"))
            .expect("workflow job index json");
    index_json["jobs"] = serde_json::json!([]);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize workflow job index json"),
    )
    .expect("write workflow job index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "jobs/index.json jobs[generate-shot-1]"
            && issue.message.contains("missing workflow job index entry")
            && issue.fix.contains("Regenerate jobs/index.json")
    }));
}

#[test]
fn save_split_project_writes_project_context_summary_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.render_reports.push(render_report());
    project.jobs.push(workflow_job());
    project.export_artifacts.push(export_artifact());

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let context_path = dir.path().join("context/project.json");
    assert!(context_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("context/project.json")));

    let raw_context = fs::read_to_string(context_path).expect("project context json");
    assert!(!raw_context.contains("projectDir"));
    assert!(!raw_context.contains("/tmp/hidden-project-path"));
    assert!(!raw_context.contains("providerCredentialEnvVar"));

    let context_json: serde_json::Value =
        serde_json::from_str(&raw_context).expect("parse project context");
    assert_eq!(context_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(context_json["projectId"], serde_json::json!("project-test"));
    assert_eq!(context_json["name"], serde_json::json!("Test Project"));
    assert_eq!(
        context_json["files"]["manifest"],
        serde_json::json!("video-creater.project.json")
    );
    assert_eq!(
        context_json["files"]["timeline"],
        serde_json::json!("timeline.json")
    );
    assert_eq!(context_json["files"]["jobs"], serde_json::json!("jobs"));
    assert_eq!(
        context_json["files"]["exports"],
        serde_json::json!("exports")
    );
    assert_eq!(
        context_json["indexes"]["generated"],
        serde_json::json!("generated/index.json")
    );
    assert_eq!(
        context_json["indexes"]["jobs"],
        serde_json::json!("jobs/index.json")
    );
    assert_eq!(
        context_json["indexes"]["exports"],
        serde_json::json!("exports/index.json")
    );
    assert_eq!(context_json["counts"]["media"], serde_json::json!(2));
    assert_eq!(context_json["counts"]["mediaFolders"], serde_json::json!(0));
    assert_eq!(
        context_json["counts"]["timelineTracks"],
        serde_json::json!(5)
    );
    assert_eq!(
        context_json["counts"]["timelineItems"],
        serde_json::json!(1)
    );
    assert_eq!(
        context_json["counts"]["generatedAssets"],
        serde_json::json!(1)
    );
    assert_eq!(
        context_json["counts"]["renderReports"],
        serde_json::json!(1)
    );
    assert_eq!(context_json["counts"]["workflowJobs"], serde_json::json!(1));
    assert_eq!(
        context_json["counts"]["exportArtifacts"],
        serde_json::json!(1)
    );
    assert_eq!(
        context_json["latest"]["generatedAssetId"],
        serde_json::json!("generated-shot-1")
    );
    assert_eq!(
        context_json["latest"]["renderReportId"],
        serde_json::json!("render-draft-1")
    );
    assert_eq!(
        context_json["latest"]["workflowJobId"],
        serde_json::json!("generate-shot-1")
    );
    assert_eq!(
        context_json["latest"]["exportArtifactId"],
        serde_json::json!("nle-export-premiere-1")
    );
}

#[test]
fn load_and_validate_split_project_ignore_project_context_summary_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.render_reports.push(render_report());
    project.jobs.push(workflow_job());
    project.export_artifacts.push(export_artifact());
    save_split_project(dir.path(), &project).expect("save split project");

    fs::write(
        dir.path().join("context/project.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "projectId": "wrong-project",
            "counts": {
                "media": 999,
                "workflowJobs": 999
            }
        }))
        .expect("serialize edited context"),
    )
    .expect("edit project context");

    let loaded = load_split_project(dir.path()).expect("load split project");
    let report = validate_split_project(dir.path()).expect("validation report");

    assert_eq!(loaded.id, "project-test");
    assert_eq!(loaded.generated_assets.len(), 1);
    assert_eq!(loaded.render_reports.len(), 1);
    assert_eq!(loaded.jobs.len(), 1);
    assert_eq!(loaded.export_artifacts.len(), 1);
    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn save_split_project_writes_workflow_job_sidecars_for_file_editing() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let sidecar_path = dir.path().join("jobs/generate-shot-1/job.json");
    assert!(sidecar_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("jobs/generate-shot-1/job.json")));
    let sidecar_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(sidecar_path).expect("workflow job sidecar json"))
            .expect("parse workflow job sidecar");

    assert_eq!(sidecar_json["id"], "generate-shot-1");
    assert_eq!(sidecar_json["kind"], "generate_media");
    assert_eq!(sidecar_json["status"], "queued");
    assert_eq!(
        sidecar_json["startRequest"]["input"]["providerCredentialEnvVar"],
        "FAL_KEY"
    );
    assert_eq!(
        sidecar_json["startRequest"]["workflowType"],
        "VideoCreaterGenerateMediaWorkflow"
    );
}

#[test]
fn load_split_project_prefers_workflow_job_sidecars_over_manifest_mirror() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project");

    let manifest_path = dir.path().join("video-creater.project.json");
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest json"))
            .expect("parse manifest");
    manifest_json["jobs"][0]["status"] = serde_json::json!("completed");
    manifest_json["jobs"][0]["updatedAt"] = serde_json::json!("2026-06-22T09:00:00Z");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json).expect("serialize manifest json"),
    )
    .expect("write stale manifest");

    let sidecar_path = dir.path().join("jobs/generate-shot-1/job.json");
    let mut sidecar_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&sidecar_path).expect("workflow job sidecar json"),
    )
    .expect("parse sidecar");
    sidecar_json["status"] = serde_json::json!("running");
    sidecar_json["updatedAt"] = serde_json::json!("2026-06-22T10:30:00Z");
    fs::write(
        sidecar_path,
        serde_json::to_string_pretty(&sidecar_json).expect("serialize sidecar json"),
    )
    .expect("write edited sidecar");

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.jobs.len(), 1);
    assert_eq!(loaded.jobs[0].status, JobStatus::Running);
    assert_eq!(loaded.jobs[0].updated_at, "2026-06-22T10:30:00Z");
}

#[test]
fn load_split_project_falls_back_to_manifest_jobs_without_sidecars() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project");
    fs::remove_file(dir.path().join("jobs/generate-shot-1/job.json"))
        .expect("remove workflow job sidecar");

    let loaded = load_split_project(dir.path()).expect("load split project");

    assert_eq!(loaded.jobs, project.jobs);
}

#[test]
fn save_split_project_removes_stale_workflow_job_sidecar() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project with job");

    project.jobs.clear();
    let report = save_split_project(dir.path(), &project).expect("save split project without job");

    assert!(!dir.path().join("jobs/generate-shot-1/job.json").exists());
    assert!(dir.path().join("jobs/index.json").exists());
    assert!(report
        .report
        .removed_files
        .iter()
        .any(|path| path.ends_with("jobs/generate-shot-1/job.json")));
}

#[test]
fn save_split_project_writes_transcript_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: Some("transcripts/media-1.raw.json".to_string()),
        repairs: vec![TranscriptRepair {
            id: "repair-1".to_string(),
            kind: video_creater_lib::project::model::TranscriptRepairKind::WordText,
            word_index: 0,
            before: TranscriptWord {
                text: "helo".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.3,
                confidence: Some(0.7),
                speaker: Some("speaker-1".to_string()),
            },
            after: TranscriptWord {
                text: "hello".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.3,
                confidence: Some(0.99),
                speaker: Some("speaker-1".to_string()),
            },
            created_at: "2026-06-22T10:00:00Z".to_string(),
        }],
        segments: vec![video_creater_lib::project::model::TranscriptSegment {
            text: "hello there".to_string(),
            start_seconds: 1.0,
            end_seconds: 2.5,
        }],
        words: vec![
            TranscriptWord {
                text: "hello".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.3,
                confidence: Some(0.99),
                speaker: Some("speaker-1".to_string()),
            },
            TranscriptWord {
                text: "there".to_string(),
                start_seconds: 2.0,
                end_seconds: 2.5,
                confidence: Some(0.98),
                speaker: Some("speaker-1".to_string()),
            },
        ],
    });

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("transcripts/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("transcripts/index.json")));

    let raw_index = fs::read_to_string(index_path).expect("transcript index");
    assert!(!raw_index.contains("hello"));
    assert!(!raw_index.contains("helo"));
    let index_json: serde_json::Value =
        serde_json::from_str(&raw_index).expect("transcript index json");
    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["transcripts"][0]["transcriptId"],
        serde_json::json!("transcript-media-1")
    );
    assert_eq!(
        index_json["transcripts"][0]["mediaId"],
        serde_json::json!("media-1")
    );
    assert_eq!(
        index_json["transcripts"][0]["path"],
        serde_json::json!("transcripts/media-1.json")
    );
    assert_eq!(
        index_json["transcripts"][0]["engine"],
        serde_json::json!("nvidia/parakeet-tdt-0.6b-v3")
    );
    assert_eq!(
        index_json["transcripts"][0]["rawArtifactPath"],
        serde_json::json!("transcripts/media-1.raw.json")
    );
    assert_eq!(
        index_json["transcripts"][0]["repairCount"],
        serde_json::json!(1)
    );
    assert_eq!(
        index_json["transcripts"][0]["segmentCount"],
        serde_json::json!(1)
    );
    assert_eq!(
        index_json["transcripts"][0]["wordCount"],
        serde_json::json!(2)
    );
    assert_eq!(
        index_json["transcripts"][0]["startSeconds"],
        serde_json::json!(1.0)
    );
    assert_eq!(
        index_json["transcripts"][0]["endSeconds"],
        serde_json::json!(2.5)
    );
}

#[test]
fn save_split_project_writes_search_index_for_agent_discovery() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: Some("transcripts/media-1.raw.json".to_string()),
        repairs: Vec::new(),
        segments: vec![video_creater_lib::project::model::TranscriptSegment {
            text: "founder says launch day".to_string(),
            start_seconds: 0.2,
            end_seconds: 1.4,
        }],
        words: vec![
            TranscriptWord {
                text: "founder".to_string(),
                start_seconds: 0.2,
                end_seconds: 0.5,
                confidence: Some(0.99),
                speaker: None,
            },
            TranscriptWord {
                text: "launch".to_string(),
                start_seconds: 0.8,
                end_seconds: 1.1,
                confidence: Some(0.98),
                speaker: None,
            },
        ],
    });

    let report = save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("search/index.json");
    assert!(index_path.exists());
    assert!(report
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("search/index.json")));

    let index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(index_path).expect("search index"))
            .expect("search index json");
    assert_eq!(index_json["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        index_json["visualStatus"],
        serde_json::json!("notInstalled")
    );
    assert_eq!(index_json["spokenStatus"], serde_json::json!("ready"));
    assert_eq!(
        index_json["spoken"][0]["mediaId"],
        serde_json::json!("media-1")
    );
    assert_eq!(
        index_json["metadata"][0]["mediaId"],
        serde_json::json!("media-1")
    );

    let context_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("context/project.json")).expect("project context"),
    )
    .expect("project context json");
    assert_eq!(
        context_json["indexes"]["search"],
        serde_json::json!("search/index.json")
    );
}

#[test]
fn load_and_validate_split_project_ignore_transcript_index_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Ready".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.5,
            confidence: Some(0.98),
            speaker: None,
        }],
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let loaded = load_split_project(dir.path()).expect("load split project");
    let report = validate_split_project(dir.path()).expect("validation report");

    assert_eq!(loaded.transcripts.len(), 1);
    assert_eq!(loaded.transcripts[0].media_id, "media-1");
    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn validate_split_project_reports_stale_transcript_index_word_count() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("transcripts/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("transcript index"))
            .expect("transcript index json");
    index_json["transcripts"][0]["wordCount"] = serde_json::json!(9);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize transcript index json"),
    )
    .expect("write transcript index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/index.json transcripts[media-1].wordCount"
            && issue.message.contains("2")
            && issue.fix.contains("Regenerate transcripts/index.json")
    }));
}

#[test]
fn validate_split_project_reports_transcript_missing_from_index() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let index_path = dir.path().join("transcripts/index.json");
    let mut index_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index_path).expect("transcript index"))
            .expect("transcript index json");
    index_json["transcripts"] = serde_json::json!([]);
    fs::write(
        &index_path,
        serde_json::to_string_pretty(&index_json).expect("serialize transcript index json"),
    )
    .expect("write transcript index");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/index.json transcripts[media-1]"
            && issue.message.contains("missing transcript index entry")
            && issue.fix.contains("Regenerate transcripts/index.json")
    }));
}

#[test]
fn apply_project_action_to_split_project_persists_template_override_files() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::UpdateTemplateOverride {
            override_: template_override_update(),
        },
    )
    .expect("update template override in split project");

    assert_eq!(result.project.template_overrides.len(), 1);
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("templates/kinetic-lower-third-v1.json")));

    let saved_template_path = dir.path().join("templates/kinetic-lower-third-v1.json");
    assert!(saved_template_path.exists());
    let saved_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(saved_template_path).expect("saved template"))
            .expect("saved template json");
    assert_eq!(
        saved_json["visualTreatment"],
        template_override_json()["visualTreatment"]
    );

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.template_overrides.len(), 1);
    assert_eq!(
        reloaded.template_overrides[0].template_id,
        "kinetic-lower-third-v1"
    );
}

#[test]
fn apply_project_action_to_split_project_leaves_files_unchanged_when_action_fails() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    let timeline_path = dir.path().join("timeline.json");
    let timeline_before = fs::read_to_string(&timeline_path).expect("timeline before");

    let error = apply_project_action_to_split_project(
        dir.path(),
        ProjectAction::RemoveItems {
            item_ids: vec!["missing-item".to_string()],
        },
    )
    .expect_err("missing item action should fail");

    assert!(error
        .to_string()
        .contains("timeline item was not found: missing-item"));
    assert_eq!(
        fs::read_to_string(&timeline_path).expect("timeline after"),
        timeline_before
    );
    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.timeline.tracks[0].items[0].id, "item-1");
}

#[test]
fn apply_project_actions_to_split_project_persists_ordered_actions_once() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let result = apply_project_actions_to_split_project(
        dir.path(),
        vec![
            ProjectAction::EditTranscriptWords {
                edits: vec![ProjectActionTranscriptWordEdit {
                    transcript_id: "transcript-media-1".to_string(),
                    word_index: 1,
                    text: Some("Creator".to_string()),
                    start_seconds: None,
                    end_seconds: None,
                    repair_id: "repair-batch-word".to_string(),
                    created_at: "2026-06-22T10:00:00Z".to_string(),
                }],
            },
            ProjectAction::ResizeItems {
                resizes: vec![ProjectActionResize {
                    item_id: "item-1".to_string(),
                    duration_seconds: 1.5,
                }],
            },
        ],
    )
    .expect("batch actions should apply");

    assert_eq!(result.project.transcripts[0].words[1].text, "Creator");
    assert!(result
        .report
        .written_files
        .iter()
        .any(|path| path.ends_with("transcripts/media-1.json")));
    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.transcripts[0].words[1].text, "Creator");
    let resized = reloaded
        .timeline
        .tracks
        .iter()
        .flat_map(|track| track.items.iter())
        .find(|item| item.id == "item-1")
        .expect("item-1 should remain");
    assert_eq!(resized.duration_seconds, 1.5);
}

#[test]
fn apply_project_actions_to_split_project_is_atomic_when_later_action_fails() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");
    let transcript_path = dir.path().join("transcripts/media-1.json");
    let transcript_before = fs::read_to_string(&transcript_path).expect("transcript before");

    let error = apply_project_actions_to_split_project(
        dir.path(),
        vec![
            ProjectAction::EditTranscriptWords {
                edits: vec![ProjectActionTranscriptWordEdit {
                    transcript_id: "transcript-media-1".to_string(),
                    word_index: 1,
                    text: Some("Creator".to_string()),
                    start_seconds: None,
                    end_seconds: None,
                    repair_id: "repair-batch-word".to_string(),
                    created_at: "2026-06-22T10:00:00Z".to_string(),
                }],
            },
            ProjectAction::RemoveItems {
                item_ids: vec!["missing-item".to_string()],
            },
        ],
    )
    .expect_err("later invalid action should reject the whole batch");

    assert!(error
        .to_string()
        .contains("timeline item was not found: missing-item"));
    assert_eq!(
        fs::read_to_string(&transcript_path).expect("transcript after"),
        transcript_before
    );
    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.transcripts[0].words[1].text, "Creater");
    assert!(reloaded.transcripts[0].repairs.is_empty());
}

#[test]
fn split_project_manifest_path_uses_existing_project_file_name() {
    let dir = tempfile::tempdir().expect("project dir");

    assert_eq!(
        split_project_manifest_path(dir.path()),
        dir.path().join("video-creater.project.json")
    );
}

#[test]
fn validate_split_project_reports_missing_manifest_references() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    std::fs::remove_file(dir.path().join("timeline.json")).expect("remove timeline");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.path == "timeline.json" && issue.message.contains("missing")));
}

#[test]
fn validate_split_project_reports_duplicate_manifest_job_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(JobSummary {
        id: "render-draft-1".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-23T12:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    project.jobs.push(JobSummary {
        id: "render-draft-1".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Running,
        updated_at: "2026-06-23T12:01:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    save_split_project(dir.path(), &project).expect("save split project");
    fs::remove_file(dir.path().join("jobs/render-draft-1/job.json"))
        .expect("remove workflow job sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "video-creater.project.json jobs[1].id"
            && issue.message.contains("duplicate job id")
            && issue.message.contains("render-draft-1")
    }));
}

#[test]
fn validate_split_project_reports_invalid_manifest_job_workflow_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(JobSummary {
        id: "render-draft-1".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-23T12:00:00Z".to_string(),
        workflow: Some(
            video_creater_lib::project::model::TemporalWorkflowMetadata {
                workflow_id: "video-creater/project-test/render-draft/render-draft-1".to_string(),
                workflow_type: "".to_string(),
                task_queue: "video-creater-workflows".to_string(),
                run_id: Some(" ".to_string()),
                activity_types: Vec::new(),
            },
        ),
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    save_split_project(dir.path(), &project).expect("save split project");
    fs::remove_file(dir.path().join("jobs/render-draft-1/job.json"))
        .expect("remove workflow job sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "video-creater.project.json jobs[0].workflow.workflowType"
            && issue.message.contains("workflowType is required")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "video-creater.project.json jobs[0].workflow.runId"
            && issue.message.contains("runId cannot be empty")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "video-creater.project.json jobs[0].workflow.activityTypes"
            && issue.message.contains("activityTypes")
    }));
}

#[test]
fn validate_split_project_reports_invalid_workflow_job_sidecar_id() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project");

    let sidecar_path = dir.path().join("jobs/generate-shot-1/job.json");
    let mut sidecar_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&sidecar_path).expect("workflow job sidecar json"),
    )
    .expect("parse workflow job sidecar");
    sidecar_json["id"] = serde_json::json!("bad/job");
    fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar_json).expect("serialize sidecar json"),
    )
    .expect("write invalid sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "jobs/generate-shot-1/job.json id"
            && issue.message.contains("safe path segment")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "jobs/generate-shot-1/job.json id"
            && issue.message.contains("must match containing directory")
    }));
}

#[test]
fn validate_split_project_reports_mismatched_workflow_job_sidecar_start_request() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(workflow_job());
    save_split_project(dir.path(), &project).expect("save split project");

    let sidecar_path = dir.path().join("jobs/generate-shot-1/job.json");
    let mut sidecar_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&sidecar_path).expect("workflow job sidecar json"),
    )
    .expect("parse workflow job sidecar");
    sidecar_json["startRequest"]["taskQueue"] = serde_json::json!("other-task-queue");
    fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&sidecar_json).expect("serialize sidecar json"),
    )
    .expect("write mismatched sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "jobs/generate-shot-1/job.json startRequest"
            && issue.message.contains("must match workflow metadata")
    }));
}

#[test]
fn validate_split_project_falls_back_to_manifest_jobs_without_sidecars() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.jobs.push(JobSummary {
        id: "render-draft-1".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Queued,
        updated_at: "2026-06-23T12:00:00Z".to_string(),
        workflow: Some(
            video_creater_lib::project::model::TemporalWorkflowMetadata {
                workflow_id: "video-creater/project-test/render-draft/render-draft-1".to_string(),
                workflow_type: "".to_string(),
                task_queue: "video-creater-workflows".to_string(),
                run_id: None,
                activity_types: vec!["RenderDraft".to_string()],
            },
        ),
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    save_split_project(dir.path(), &project).expect("save split project");
    fs::remove_file(dir.path().join("jobs/render-draft-1/job.json"))
        .expect("remove workflow job sidecar");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "video-creater.project.json jobs[0].workflow.workflowType"
            && issue.message.contains("workflowType is required")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_media_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let duplicate = project.media[0].clone();
    project.media.push(duplicate);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json assets" && issue.message.contains("duplicate")
    }));
}

#[test]
fn validate_split_project_reports_invalid_media_asset_id() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][0]["id"] = serde_json::json!("bad/media");
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json assets[bad/media].id"
            && issue.message.contains("must be a safe path segment")
    }));
}

#[test]
fn validate_split_project_reports_empty_media_asset_and_folder_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][0]["id"] = serde_json::json!("");
    media_json["folders"] = serde_json::json!([
        { "id": "", "name": "B-roll", "parentId": null }
    ]);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json assets[].id"
            && issue.message.contains("media asset id is required")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json folders[].id"
            && issue.message.contains("media folder id is required")
    }));
}

#[test]
fn validate_split_project_reports_media_path_escape() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][0]["relativePath"] = serde_json::json!("../outside.mp4");
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json assets[media-1].relativePath"
            && issue.message.contains("unsafe project path")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_media_folder_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["folders"] = serde_json::json!([
        { "id": "folder-broll", "name": "B-roll", "parentId": null },
        { "id": "folder-broll", "name": "Duplicate B-roll", "parentId": null }
    ]);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json folders"
            && issue.message.contains("duplicate media folder id")
            && issue.message.contains("folder-broll")
    }));
}

#[test]
fn validate_split_project_reports_invalid_media_folder_id() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["folders"] = serde_json::json!([
        { "id": "../folder-broll", "name": "B-roll", "parentId": null }
    ]);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json folders[../folder-broll].id"
            && issue.message.contains("must be a safe path segment")
    }));
}

#[test]
fn validate_split_project_reports_missing_media_folder_parent() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["folders"] = serde_json::json!([
        { "id": "folder-broll", "name": "B-roll", "parentId": "missing-folder" }
    ]);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json folders[folder-broll].parentId"
            && issue.message.contains("missing-folder")
    }));
}

#[test]
fn validate_split_project_reports_self_parented_media_folder() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["folders"] = serde_json::json!([
        { "id": "folder-broll", "name": "B-roll", "parentId": "folder-broll" }
    ]);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json folders[folder-broll].parentId"
            && issue.message.contains("cannot reference itself")
    }));
}

#[test]
fn validate_split_project_reports_media_folder_parent_cycle() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["folders"] = serde_json::json!([
        { "id": "folder-a", "name": "Folder A", "parentId": "folder-b" },
        { "id": "folder-b", "name": "Folder B", "parentId": "folder-a" }
    ]);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json folders[folder-a].parentId"
            && issue.message.contains("cycle")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_track_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let duplicate = project.timeline.tracks[0].clone();
    project.timeline.tracks.push(duplicate);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[5].id" && issue.message.contains("duplicate track id")
    }));
}

#[test]
fn validate_split_project_reports_item_track_kind_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0]
        .items
        .push(caption_item("caption-on-video", 4.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[1].kind"
            && issue.message.contains("caption")
            && issue.message.contains("video")
    }));
}

#[test]
fn validate_split_project_reports_caption_item_with_non_text_source() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[3].items.push(TimelineItem {
        id: "caption-media-source".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 1.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Caption".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[3].items[0].source"
            && issue.message.contains("caption")
            && issue.message.contains("text source")
    }));
}

#[test]
fn validate_split_project_reports_caption_item_with_blank_text() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[3].items.push(TimelineItem {
        id: "caption-blank-text".to_string(),
        kind: TimelineItemKind::Caption,
        start_seconds: 1.0,
        duration_seconds: 1.0,
        source: TimelineSource::Text {
            text: "   ".to_string(),
        },
        label: "Caption".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[3].items[0].source.text"
            && issue.message.contains("caption")
            && issue.message.contains("text is required")
    }));
}

#[test]
fn validate_split_project_reports_video_item_with_text_source() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source = TimelineSource::Text {
        text: "not video media".to_string(),
    };
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].source"
            && issue.message.contains("video")
            && issue
                .message
                .contains("media, generated, or timeline source")
    }));
}

#[test]
fn validate_split_project_reports_audio_item_with_text_source() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "audio-text-source".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Text {
            text: "not audio media".to_string(),
        },
        label: "Audio".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[4].items[0].source"
            && issue.message.contains("audio")
            && issue.message.contains("media or generated source")
    }));
}

#[test]
fn validate_split_project_allows_audio_item_referencing_video_media_for_linked_audio() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[4].items.push(TimelineItem {
        id: "linked-video-audio".to_string(),
        kind: TimelineItemKind::AudioClip,
        start_seconds: 0.0,
        duration_seconds: 1.0,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Linked camera audio".to_string(),
        properties: std::collections::BTreeMap::from([
            ("linkGroupId".to_string(), serde_json::json!("link-item-1")),
            ("sourceClipType".to_string(), serde_json::json!("audio")),
        ]),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(
        report.ok,
        "unexpected validation issues: {:?}",
        report.issues
    );
}

#[test]
fn validate_split_project_reports_timeline_duration_shorter_than_items() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.duration_seconds = 0.25;
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json durationSeconds"
            && issue.message.contains("must cover")
            && issue.message.contains("item-1")
    }));
}

#[test]
fn validate_split_project_reports_overlapping_timeline_items_on_same_track() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut overlapping = project.timeline.tracks[0].items[0].clone();
    overlapping.id = "item-overlap".to_string();
    overlapping.start_seconds = 0.5;
    project.timeline.tracks[0].items.push(overlapping);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[1].startSeconds"
            && issue.message.contains("overlaps")
            && issue.message.contains("item-1")
    }));
}

#[test]
fn validate_split_project_reports_out_of_order_timeline_items_on_same_track() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].start_seconds = 5.0;
    project.timeline.tracks[0].items[0].duration_seconds = 1.0;

    let mut earlier = project.timeline.tracks[0].items[0].clone();
    earlier.id = "item-earlier".to_string();
    earlier.start_seconds = 0.0;
    project.timeline.tracks[0].items.push(earlier);
    project.timeline.duration_seconds = 6.0;
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[1].startSeconds"
            && issue.message.contains("chronological")
            && issue.message.contains("item-1")
    }));
}

#[test]
fn validate_split_project_reports_missing_media_references() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].source =
        video_creater_lib::project::model::TimelineSource::Media {
            media_id: "missing-media".to_string(),
        };
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].source.mediaId"
            && issue.message.contains("missing-media")
    }));
}

#[test]
fn validate_split_project_reports_video_item_referencing_audio_media() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
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
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "audio-1".to_string(),
    };
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].source.mediaId"
            && issue.message.contains("video")
            && issue.message.contains("audio-1")
    }));
}

#[test]
fn validate_split_project_reports_missing_generated_timeline_references() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[2].items.push(TimelineItem {
        id: "generated-overlay".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Generated {
            artifact_id: "missing-generated-shot".to_string(),
        },
        label: "Generated overlay".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[2].items[0].source.artifactId"
            && issue.message.contains("missing-generated-shot")
    }));
}

#[test]
fn validate_split_project_reports_generated_timeline_reference_not_completed() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    let mut asset = generated_asset();
    asset.status = GeneratedAssetStatus::Failed;
    project.generated_assets.push(asset);
    project.timeline.tracks[2].items.push(TimelineItem {
        id: "generated-overlay".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Generated {
            artifact_id: "generated-shot-1".to_string(),
        },
        label: "Generated overlay".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[2].items[0].source.artifactId"
            && issue.message.contains("generated-shot-1")
            && issue.message.contains("completed")
    }));
}

#[test]
fn validate_split_project_reports_generated_timeline_reference_without_outputs() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[2].items.push(TimelineItem {
        id: "generated-overlay".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.0,
        duration_seconds: 2.0,
        source: TimelineSource::Generated {
            artifact_id: "generated-shot-1".to_string(),
        },
        label: "Generated overlay".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"] = serde_json::json!([]);
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[2].items[0].source.artifactId"
            && issue.message.contains("generated-shot-1")
            && issue.message.contains("no outputs")
    }));
}

#[test]
fn validate_split_project_reports_generated_timeline_duration_longer_than_outputs() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[2].items.push(TimelineItem {
        id: "generated-overlay".to_string(),
        kind: TimelineItemKind::Overlay,
        start_seconds: 1.0,
        duration_seconds: 5.0,
        source: TimelineSource::Generated {
            artifact_id: "generated-shot-1".to_string(),
        },
        label: "Generated overlay".to_string(),
        properties: std::collections::BTreeMap::new(),
    });
    project.timeline.duration_seconds = 6.0;
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[2].items[0].durationSeconds"
            && issue.message.contains("generated-shot-1")
            && issue.message.contains("exceeds generated output duration")
    }));
}

#[test]
fn validate_split_project_reports_missing_generated_asset_metadata_on_timeline_item() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "generated-shot-1-output".to_string(),
    };
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedAssetId".to_string(),
        serde_json::json!("missing-generated-shot"),
    );
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedOutputMediaId".to_string(),
        serde_json::json!("generated-shot-1-output"),
    );
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.generatedAssetId"
            && issue.message.contains("missing-generated-shot")
    }));
}

#[test]
fn validate_split_project_reports_generated_output_metadata_without_asset_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "generated-shot-1-output".to_string(),
    };
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedOutputMediaId".to_string(),
        serde_json::json!("generated-shot-1-output"),
    );
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.generatedOutputMediaId"
            && issue.message.contains("generatedAssetId")
            && issue.message.contains("generated-shot-1-output")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_metadata_without_output_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "generated-shot-1-output".to_string(),
    };
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedAssetId".to_string(),
        serde_json::json!("generated-shot-1"),
    );
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.generatedOutputMediaId"
            && issue.message.contains("generated-shot-1-output")
            && issue.message.contains("generatedAssetId")
    }));
}

#[test]
fn validate_split_project_reports_generated_output_metadata_not_on_asset() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "generated-shot-1-output".to_string(),
    };
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedAssetId".to_string(),
        serde_json::json!("generated-shot-1"),
    );
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedOutputMediaId".to_string(),
        serde_json::json!("missing-output"),
    );
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.generatedOutputMediaId"
            && issue.message.contains("missing-output")
            && issue.message.contains("generated-shot-1")
    }));
}

#[test]
fn validate_split_project_reports_generated_output_metadata_source_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    project.timeline.tracks[0].items[0].source = TimelineSource::Media {
        media_id: "other-generated-output".to_string(),
    };
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedAssetId".to_string(),
        serde_json::json!("generated-shot-1"),
    );
    project.timeline.tracks[0].items[0].properties.insert(
        "generatedOutputMediaId".to_string(),
        serde_json::json!("generated-shot-1-output"),
    );
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.generatedOutputMediaId"
            && issue.message.contains("generated-shot-1-output")
            && issue.message.contains("other-generated-output")
    }));
}

#[test]
fn validate_split_project_reports_source_ranges_outside_media_duration() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(0.2));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(999.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.sourceOut"
            && issue.message.contains("inside media duration")
    }));
}

#[test]
fn validate_split_project_reports_imported_source_range_duration_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0].items[0].duration_seconds = 3.0;
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceIn".to_string(), serde_json::json!(1.0));
    project.timeline.tracks[0].items[0]
        .properties
        .insert("sourceOut".to_string(), serde_json::json!(5.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.sourceOut"
            && issue
                .message
                .contains("source range duration must match timeline duration")
    }));
}

#[test]
fn validate_split_project_accepts_source_range_scaled_by_clip_speed() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 2.0;
    item.properties
        .insert("sourceIn".to_string(), serde_json::json!(1.0));
    item.properties
        .insert("sourceOut".to_string(), serde_json::json!(5.0));
    item.properties
        .insert("speed".to_string(), serde_json::json!(2.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(
        !report.issues.iter().any(|issue| issue
            .message
            .contains("source range duration must match timeline duration")),
        "unexpected issues: {:?}",
        report.issues
    );
}

#[test]
fn validate_split_project_reports_source_range_that_ignores_clip_speed() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 2.0;
    item.properties
        .insert("sourceIn".to_string(), serde_json::json!(1.0));
    item.properties
        .insert("sourceOut".to_string(), serde_json::json!(3.0));
    item.properties
        .insert("speed".to_string(), serde_json::json!(2.0));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[0].properties.sourceOut"
            && issue
                .message
                .contains("source range duration must match timeline duration")
    }));
}

#[test]
fn validate_split_project_reports_generated_edit_without_selected_source_range() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.timeline.tracks[0]
        .items
        .push(generated_edit_video_item(
            "generated-edit-1",
            "media-1",
            4.0,
        ));
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "timeline.json tracks[0].items[1].properties.sourceIn"
            && issue
                .message
                .contains("generated edit video clips require sourceIn and sourceOut")
    }));
}

#[test]
fn validate_split_project_reports_transcript_raw_artifact_path_escape() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let transcript_path = dir.path().join("transcripts/media-1.json");
    let mut transcript_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&transcript_path).expect("transcript file"))
            .expect("transcript json");
    transcript_json["rawArtifactPath"] = serde_json::json!("../outside.raw.json");
    fs::write(
        &transcript_path,
        serde_json::to_string_pretty(&transcript_json).expect("serialize transcript json"),
    )
    .expect("write transcript file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json rawArtifactPath"
            && issue.message.contains("unsafe project path")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_transcript_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut second_media = project.media[0].clone();
    second_media.id = "media-2".to_string();
    second_media.relative_path = "media/source-2.mp4".to_string();
    project.media.push(second_media);

    let mut first_transcript = transcript_with_words();
    first_transcript.id = "transcript-duplicate".to_string();
    first_transcript.media_id = "media-1".to_string();
    let mut second_transcript = transcript_with_words();
    second_transcript.id = "transcript-duplicate".to_string();
    second_transcript.media_id = "media-2".to_string();
    project.transcripts.push(first_transcript);
    project.transcripts.push(second_transcript);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-2.json id"
            && issue.message.contains("duplicate transcript id")
            && issue.message.contains("transcript-duplicate")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_transcript_media_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let transcript_path = dir.path().join("transcripts/media-1.json");
    let mut duplicate_transcript: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&transcript_path).expect("transcript file"))
            .expect("transcript json");
    duplicate_transcript["id"] = serde_json::json!("transcript-media-1-copy");
    fs::write(
        dir.path().join("transcripts/media-1-copy.json"),
        serde_json::to_string_pretty(&duplicate_transcript).expect("serialize transcript json"),
    )
    .expect("write duplicate transcript file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json mediaId"
            && issue.message.contains("duplicate transcript mediaId")
            && issue.message.contains("media-1")
    }));
}

#[test]
fn validate_split_project_reports_transcript_filename_media_id_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut second_media = project.media[0].clone();
    second_media.id = "media-2".to_string();
    second_media.relative_path = "media/source-2.mp4".to_string();
    project.media.push(second_media);
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let transcript_path = dir.path().join("transcripts/media-1.json");
    let mut transcript_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&transcript_path).expect("transcript file"))
            .expect("transcript json");
    transcript_json["mediaId"] = serde_json::json!("media-2");
    fs::write(
        &transcript_path,
        serde_json::to_string_pretty(&transcript_json).expect("serialize transcript json"),
    )
    .expect("write transcript file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json mediaId"
            && issue.message.contains("media-2")
            && issue.message.contains("media-1")
    }));
}

#[test]
fn validate_split_project_reports_transcript_repair_word_index_out_of_range() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let transcript_path = dir.path().join("transcripts/media-1.json");
    let mut transcript_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&transcript_path).expect("transcript file"))
            .expect("transcript json");
    transcript_json["repairs"] = serde_json::json!([
        {
            "id": "repair-out-of-range",
            "kind": "word_text",
            "wordIndex": 99,
            "before": {
                "text": "Creater",
                "startSeconds": 0.5,
                "endSeconds": 0.9,
                "confidence": 0.88,
                "speaker": null
            },
            "after": {
                "text": "Creator",
                "startSeconds": 0.5,
                "endSeconds": 0.9,
                "confidence": 0.88,
                "speaker": null
            },
            "createdAt": "2026-06-22T10:00:00Z"
        }
    ]);
    fs::write(
        &transcript_path,
        serde_json::to_string_pretty(&transcript_json).expect("serialize transcript json"),
    )
    .expect("write transcript file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json repairs[0].wordIndex"
            && issue.message.contains("outside transcript words")
    }));
}

#[test]
fn validate_split_project_reports_duplicate_transcript_repair_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(transcript_with_words());
    save_split_project(dir.path(), &project).expect("save split project");

    let transcript_path = dir.path().join("transcripts/media-1.json");
    let mut transcript_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&transcript_path).expect("transcript file"))
            .expect("transcript json");
    transcript_json["repairs"] = serde_json::json!([
        {
            "id": "repair-duplicate",
            "kind": "word_text",
            "wordIndex": 0,
            "before": {
                "text": "Video",
                "startSeconds": 0.0,
                "endSeconds": 0.4,
                "confidence": 0.95,
                "speaker": null
            },
            "after": {
                "text": "video",
                "startSeconds": 0.0,
                "endSeconds": 0.4,
                "confidence": 0.95,
                "speaker": null
            },
            "createdAt": "2026-06-22T10:00:00Z"
        },
        {
            "id": "repair-duplicate",
            "kind": "word_text",
            "wordIndex": 1,
            "before": {
                "text": "Creater",
                "startSeconds": 0.5,
                "endSeconds": 0.9,
                "confidence": 0.88,
                "speaker": null
            },
            "after": {
                "text": "Creator",
                "startSeconds": 0.5,
                "endSeconds": 0.9,
                "confidence": 0.88,
                "speaker": null
            },
            "createdAt": "2026-06-22T10:01:00Z"
        }
    ]);
    fs::write(
        &transcript_path,
        serde_json::to_string_pretty(&transcript_json).expect("serialize transcript json"),
    )
    .expect("write transcript file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json repairs[1].id"
            && issue.message.contains("duplicate transcript repair id")
            && issue.message.contains("repair-duplicate")
    }));
}

#[test]
fn validate_split_project_reports_transcript_words_out_of_order() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "later".to_string(),
                start_seconds: 1.0,
                end_seconds: 1.2,
                confidence: None,
                speaker: None,
            },
            TranscriptWord {
                text: "earlier".to_string(),
                start_seconds: 0.5,
                end_seconds: 0.7,
                confidence: None,
                speaker: None,
            },
        ],
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json words[1].startSeconds"
            && issue.message.contains("monotonic")
    }));
}

#[test]
fn validate_split_project_reports_overlapping_transcript_words() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![
            TranscriptWord {
                text: "first".to_string(),
                start_seconds: 0.0,
                end_seconds: 0.8,
                confidence: None,
                speaker: None,
            },
            TranscriptWord {
                text: "overlap".to_string(),
                start_seconds: 0.5,
                end_seconds: 1.0,
                confidence: None,
                speaker: None,
            },
        ],
    });
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "transcripts/media-1.json words[1].startSeconds"
            && issue.message.contains("monotonic")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_missing_media_reference() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["references"]["mediaIds"] = serde_json::json!(["missing-media"]);
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json references.mediaIds[0]"
            && issue.message.contains("missing-media")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_audio_first_frame_reference() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
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
    project.media.push(generated_media_output());
    let mut asset = generated_asset();
    asset.references.first_frame_media_id = Some("audio-1".to_string());
    project.generated_assets.push(asset);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json references.firstFrameMediaId"
            && issue.message.contains("audio-1")
            && issue.message.contains("visual")
    }));
}

#[test]
fn validate_split_project_allows_pending_generated_asset_without_outputs() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut asset = generated_asset();
    asset.status = GeneratedAssetStatus::Queued;
    asset.outputs = Vec::new();
    project.generated_assets.push(asset);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(
        report.ok,
        "pending generated assets should be valid before outputs exist: {:?}",
        report.issues
    );
}

#[test]
fn validate_split_project_allows_promptless_source_video_audio_asset() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    let mut asset = generated_asset();
    asset.id = "generated-video-music-1".to_string();
    asset.status = GeneratedAssetStatus::Queued;
    asset.prompt = "   ".to_string();
    asset.model = GenerationModel {
        provider: "fal.ai".to_string(),
        id: "sonilo/v1.1/video-to-music".to_string(),
    };
    asset.references = GeneratedAssetReferences {
        media_ids: vec!["media-1".to_string()],
        source_video_media_ref: Some("media-1".to_string()),
        ..GeneratedAssetReferences::default()
    };
    asset.settings = GeneratedAssetSettings {
        width: None,
        height: None,
        duration_seconds: Some(4.0),
        fps: None,
        aspect_ratio: None,
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    };
    asset.outputs = Vec::new();
    project.generated_assets.push(asset);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(
        report.ok,
        "promptless source-video audio generated assets should be valid: {:?}",
        report.issues
    );
}

#[test]
fn validate_split_project_reports_generated_asset_missing_target_folder() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["targetFolderId"] = serde_json::json!("missing-folder");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json targetFolderId"
            && issue.message.contains("missing-folder")
            && issue.message.contains("target folder")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_wrong_kind() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["kind"] = serde_json::json!("audio");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json kind"
            && issue.message.contains("generated asset kind")
            && issue.message.contains("generated")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_directory_id_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["id"] = serde_json::json!("generated-shot-renamed");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json id"
            && issue.message.contains("generated-shot-renamed")
            && issue.message.contains("generated-shot-1")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_missing_lineage_reference() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["parentAssetId"] = serde_json::json!("missing-parent");
    asset_json["retryOfAssetId"] = serde_json::json!("missing-retry");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json parentAssetId"
            && issue.message.contains("missing-parent")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json retryOfAssetId"
            && issue.message.contains("missing-retry")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_lineage_cycle() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());

    let mut second_output = generated_media_output();
    second_output.id = "generated-shot-2-output".to_string();
    second_output.relative_path = "generated/generated-shot-2/output.mp4".to_string();
    project.media.push(second_output);

    let mut second_asset = generated_asset();
    second_asset.id = "generated-shot-2".to_string();
    second_asset.outputs[0].media_id = "generated-shot-2-output".to_string();
    second_asset.outputs[0].relative_path = "generated/generated-shot-2/output.mp4".to_string();
    project.generated_assets.push(generated_asset());
    project.generated_assets.push(second_asset);
    save_split_project(dir.path(), &project).expect("save split project");

    let first_asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut first_asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&first_asset_path).expect("first asset file"))
            .expect("first asset json");
    first_asset_json["parentAssetId"] = serde_json::json!("generated-shot-2");
    fs::write(
        &first_asset_path,
        serde_json::to_string_pretty(&first_asset_json).expect("serialize first asset json"),
    )
    .expect("write first asset file");

    let second_asset_path = dir.path().join("generated/generated-shot-2/asset.json");
    let mut second_asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&second_asset_path).expect("second asset file"))
            .expect("second asset json");
    second_asset_json["retryOfAssetId"] = serde_json::json!("generated-shot-1");
    fs::write(
        &second_asset_path,
        serde_json::to_string_pretty(&second_asset_json).expect("serialize second asset json"),
    )
    .expect("write second asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json parentAssetId"
            && issue.message.contains("lineage cycle")
            && issue.message.contains("generated-shot-2")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_duplicate_output_media_ids() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    let duplicate_output = asset_json["outputs"][0].clone();
    asset_json["outputs"]
        .as_array_mut()
        .expect("outputs array")
        .push(duplicate_output);
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[1].mediaId"
            && issue
                .message
                .contains("duplicate generated output media id")
            && issue.message.contains("generated-shot-1-output")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_media_id_used_by_another_asset() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());

    let mut second_asset = generated_asset();
    second_asset.id = "generated-shot-2".to_string();
    second_asset.outputs[0].media_id = "generated-shot-1-output".to_string();
    second_asset.outputs[0].relative_path = "generated/generated-shot-1/output.mp4".to_string();
    project.generated_assets.push(second_asset);
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-2/asset.json outputs[0].mediaId"
            && issue.message.contains("generated-shot-1-output")
            && issue.message.contains("generated-shot-1")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_imported_media_reference() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"][0]["mediaId"] = serde_json::json!("media-1");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].mediaId"
            && issue.message.contains("generated media")
            && issue.message.contains("media-1")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_media_path_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"][0]["relativePath"] =
        serde_json::json!("generated/generated-shot-1/alternate.mp4");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].relativePath"
            && issue.message.contains("generated-shot-1-output")
            && issue.message.contains("media/index.json")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_outside_asset_directory() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"][0]["relativePath"] = serde_json::json!("generated/other-shot/output.mp4");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][1]["relativePath"] = serde_json::json!("generated/other-shot/output.mp4");
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].relativePath"
            && issue.message.contains("generated/generated-shot-1")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_as_asset_directory() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"][0]["relativePath"] = serde_json::json!("generated/generated-shot-1");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][1]["relativePath"] = serde_json::json!("generated/generated-shot-1");
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].relativePath"
            && issue
                .message
                .contains("generated output path must stay under")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_as_asset_sidecar() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"][0]["relativePath"] =
        serde_json::json!("generated/generated-shot-1/asset.json");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][1]["relativePath"] =
        serde_json::json!("generated/generated-shot-1/asset.json");
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].relativePath"
            && issue
                .message
                .contains("generated output path cannot use reserved asset sidecar")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_media_metadata_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let media_path = dir.path().join("media/index.json");
    let mut media_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&media_path).expect("media index file"))
            .expect("media index json");
    media_json["assets"][1]["durationSeconds"] = serde_json::json!(3.5);
    media_json["assets"][1]["width"] = serde_json::json!(1920);
    media_json["assets"][1]["height"] = serde_json::json!(1080);
    media_json["assets"][1]["fps"] = serde_json::json!(30.0);
    fs::write(
        &media_path,
        serde_json::to_string_pretty(&media_json).expect("serialize media index json"),
    )
    .expect("write media index file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].durationSeconds"
            && issue.message.contains("generated-shot-1-output")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].width"
            && issue.message.contains("generated-shot-1-output")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].height"
            && issue.message.contains("generated-shot-1-output")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].fps"
            && issue.message.contains("generated-shot-1-output")
    }));
}

#[test]
fn validate_split_project_reports_generated_media_without_asset_output() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "media/index.json assets[generated-shot-1-output].kind"
            && issue.message.contains("generated-shot-1-output")
            && issue.message.contains("generated asset output")
    }));
}

#[test]
fn validate_split_project_reports_generated_asset_output_path_escape() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.media.push(generated_media_output());
    project.generated_assets.push(generated_asset());
    save_split_project(dir.path(), &project).expect("save split project");

    let asset_path = dir.path().join("generated/generated-shot-1/asset.json");
    let mut asset_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&asset_path).expect("generated asset file"))
            .expect("generated asset json");
    asset_json["outputs"][0]["relativePath"] = serde_json::json!("../outside.mp4");
    fs::write(
        &asset_path,
        serde_json::to_string_pretty(&asset_json).expect("serialize generated asset json"),
    )
    .expect("write generated asset file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "generated/generated-shot-1/asset.json outputs[0].relativePath"
            && issue.message.contains("unsafe project path")
    }));
}

#[test]
fn validate_split_project_reports_template_override_filename_id_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.template_overrides.push(template_override());
    save_split_project(dir.path(), &project).expect("save split project");

    let template_path = dir.path().join("templates/kinetic-lower-third-v1.json");
    let mut template_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&template_path).expect("template override file"))
            .expect("template override json");
    template_json["templateId"] = serde_json::json!("renamed-template");
    fs::write(
        &template_path,
        serde_json::to_string_pretty(&template_json).expect("serialize template override json"),
    )
    .expect("write template override");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "templates/kinetic-lower-third-v1.json templateId"
            && issue.message.contains("renamed-template")
            && issue.message.contains("kinetic-lower-third-v1")
    }));
}

#[test]
fn validate_split_project_reports_render_report_missing_required_check() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let report_path = dir.path().join("renders/render-draft-1/report.json");
    let mut report_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("render report file"))
            .expect("render report json");
    report_json["checks"]
        .as_object_mut()
        .expect("checks object")
        .remove("artifactPaths");
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&report_json).expect("serialize render report json"),
    )
    .expect("write render report file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json checks.artifactPaths"
            && issue.message.contains("required")
    }));
}

#[test]
fn validate_split_project_reports_render_report_missing_stream_and_log_checks() {
    for required_check in ["streams", "logPath"] {
        let dir = tempfile::tempdir().expect("project dir");
        let mut project = sample_project();
        project.render_reports.push(render_report());
        save_split_project(dir.path(), &project).expect("save split project");

        let report_path = dir.path().join("renders/render-draft-1/report.json");
        let mut report_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report_path).expect("render report file"))
                .expect("render report json");
        report_json["checks"]
            .as_object_mut()
            .expect("checks object")
            .remove(required_check);
        fs::write(
            &report_path,
            serde_json::to_string_pretty(&report_json).expect("serialize render report json"),
        )
        .expect("write render report file");

        let report = validate_split_project(dir.path()).expect("validation report");

        assert!(!report.ok);
        assert!(report.issues.iter().any(|issue| {
            issue.path == format!("renders/render-draft-1/report.json checks.{required_check}")
                && issue.message.contains("required")
        }));
    }
}

#[test]
fn validate_split_project_reports_render_report_directory_id_mismatch() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let report_path = dir.path().join("renders/render-draft-1/report.json");
    let mut report_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("render report file"))
            .expect("render report json");
    report_json["id"] = serde_json::json!("renamed-render");
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&report_json).expect("serialize render report json"),
    )
    .expect("write render report file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json id"
            && issue.message.contains("renamed-render")
            && issue.message.contains("render-draft-1")
    }));
}

#[test]
fn validate_split_project_reports_render_output_outside_render_directory() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let report_path = dir.path().join("renders/render-draft-1/report.json");
    let mut report_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("render report file"))
            .expect("render report json");
    report_json["outputPath"] = serde_json::json!("renders/other-render/output.mp4");
    report_json["artifacts"][0] = serde_json::json!("renders/other-render/output.mp4");
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&report_json).expect("serialize render report json"),
    )
    .expect("write render report file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json outputPath"
            && issue.message.contains("renders/render-draft-1")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json artifacts[0]"
            && issue.message.contains("renders/render-draft-1")
    }));
}

#[test]
fn validate_split_project_reports_render_output_missing_from_artifacts() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let report_path = dir.path().join("renders/render-draft-1/report.json");
    let mut report_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("render report file"))
            .expect("render report json");
    report_json["artifacts"] = serde_json::json!(["renders/render-draft-1/sidecar.json"]);
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&report_json).expect("serialize render report json"),
    )
    .expect("write render report file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json artifacts"
            && issue.message.contains("outputPath")
            && issue.message.contains("renders/render-draft-1/output.mp4")
    }));
}

#[test]
fn validate_split_project_reports_render_report_path_escape() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.render_reports.push(render_report());
    save_split_project(dir.path(), &project).expect("save split project");

    let report_path = dir.path().join("renders/render-draft-1/report.json");
    let mut report_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("render report file"))
            .expect("render report json");
    report_json["outputPath"] = serde_json::json!("/tmp/outside.mp4");
    report_json["artifacts"][0] = serde_json::json!("../outside.mp4");
    report_json["logPath"] = serde_json::json!("../outside.log");
    fs::write(
        &report_path,
        serde_json::to_string_pretty(&report_json).expect("serialize render report json"),
    )
    .expect("write render report file");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json outputPath"
            && issue.message.contains("unsafe project path")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json artifacts[0]"
            && issue.message.contains("unsafe project path")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.path == "renders/render-draft-1/report.json logPath"
            && issue.message.contains("unsafe project path")
    }));
}

#[test]
fn validate_split_project_reports_template_override_missing_visual_metadata() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let mut template_json = template_override_json();
    template_json["safeZone"] = serde_json::json!("");
    fs::write(
        dir.path().join("templates/kinetic-lower-third-v1.json"),
        serde_json::to_string_pretty(&template_json).expect("template override json"),
    )
    .expect("write template override");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(!report.ok);
    assert!(report.issues.iter().any(|issue| {
        issue.path == "templates/kinetic-lower-third-v1.json safeZone"
            && issue.message.contains("required")
    }));
}

#[test]
fn validate_split_project_accepts_valid_split_project() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let report = validate_split_project(dir.path()).expect("validation report");

    assert!(report.ok);
    assert!(report.issues.is_empty());
}

#[test]
fn migrate_single_file_project_writes_split_files_and_preserves_runtime_data() {
    let dir = tempfile::tempdir().expect("project dir");
    let mut project = sample_project();
    project.codex_thread_id = Some("thread-123".to_string());
    project.jobs.push(JobSummary {
        id: "job-render-1".to_string(),
        kind: "render".to_string(),
        status: JobStatus::Completed,
        updated_at: "2026-06-21T12:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-media-1".to_string(),
        media_id: "media-1".to_string(),
        engine: Some("nvidia/parakeet-tdt-0.6b-v3".to_string()),
        raw_artifact_path: Some("transcripts/media-1.raw.json".to_string()),
        repairs: Vec::new(),
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "Migrated".to_string(),
            start_seconds: 0.0,
            end_seconds: 0.5,
            confidence: Some(0.98),
            speaker: Some("speaker-1".to_string()),
        }],
    });
    save_project(dir.path(), &project).expect("save schema-v1 project");

    let report = migrate_single_file_project(dir.path()).expect("migrate project");

    assert!(dir.path().join("timeline.json").exists());
    assert!(dir.path().join("media/index.json").exists());
    assert!(dir.path().join("transcripts/media-1.json").exists());
    assert!(report
        .written_files
        .iter()
        .any(|path| path.contains("logs/project-migration-") && path.ends_with(".json")));

    let manifest_json =
        std::fs::read_to_string(dir.path().join("video-creater.project.json")).expect("manifest");
    assert!(manifest_json.contains(r#""schemaVersion": 2"#));
    assert!(manifest_json.contains(r#""layout": "split""#));
    assert!(!manifest_json.contains(r#""media": ["#));

    let loaded = load_split_project(dir.path()).expect("load migrated split project");

    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.content_revision, project.content_revision + 1);
    assert_eq!(loaded.id, project.id);
    assert_eq!(loaded.media, project.media);
    assert_eq!(loaded.timeline, project.timeline);
    assert_eq!(loaded.transcripts, project.transcripts);
    assert_eq!(loaded.codex_thread_id, project.codex_thread_id);
    assert_eq!(loaded.jobs, project.jobs);
}

#[test]
fn migrate_single_file_project_rejects_already_split_projects_without_rewriting() {
    let dir = tempfile::tempdir().expect("project dir");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");

    let error = migrate_single_file_project(dir.path()).expect_err("split project must reject");

    assert_eq!(error, SplitProjectError::AlreadySplitProject);
}

#[test]
fn split_project_keeps_a_recorded_absolute_export_artifact() {
    use video_creater_lib::edit::render_plan::RenderQuality;
    use video_creater_lib::project::export_destination::ExportOutputRequest;
    use video_creater_lib::project::export_options::{ExportRenderOptions, JobExportSettings};
    use video_creater_lib::project::export_profiles::ExportProfile;

    let dir = tempfile::tempdir().expect("project dir");
    let outside = tempfile::tempdir().expect("chosen export folder");
    let project = sample_project();
    save_split_project(dir.path(), &project).expect("save split project");
    let artifact_path = outside.path().join("Edison intro (2).mp4");
    fs::write(&artifact_path, b"rendered").expect("exported file");
    let artifact_path = artifact_path.display().to_string();
    let job = JobSummary {
        id: "export-mp4H264-1".to_string(),
        kind: "render_draft".to_string(),
        status: JobStatus::Completed,
        updated_at: "2026-09-17T10:00:00Z".to_string(),
        workflow: None,
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: Some(JobExportSettings {
            options: ExportRenderOptions::new(
                ExportProfile::Mp4H264,
                RenderQuality::Final,
                1920,
                1080,
            )
            .expect("options"),
            output: Some(ExportOutputRequest {
                file_name: "Edison intro".to_string(),
                directory: Some(outside.path().display().to_string()),
            }),
        }),
    };
    let artifact = ProjectExportArtifact {
        schema_version: 1,
        id: "export-mp4H264-1".to_string(),
        kind: ProjectExportArtifactKind::Mp4,
        format: "mp4H264".to_string(),
        path: artifact_path.clone(),
        mime_type: "video/mp4".to_string(),
        job_id: Some("export-mp4H264-1".to_string()),
        created_at: "2026-09-17T10:05:00Z".to_string(),
    };

    apply_project_actions_to_split_project(
        dir.path(),
        vec![
            ProjectAction::RecordJob {
                job: Box::new(job.clone()),
            },
            ProjectAction::RecordExportArtifact {
                artifact: artifact.clone(),
            },
        ],
    )
    .expect("record the job and its absolute artifact");

    let reloaded = load_split_project(dir.path()).expect("reload split project");
    assert_eq!(reloaded.export_artifacts, vec![artifact]);
    assert_eq!(
        reloaded
            .jobs
            .iter()
            .find(|candidate| candidate.id == job.id),
        Some(&job)
    );
    let report = validate_split_project(dir.path()).expect("validation report");
    let artifact_issues = report
        .issues
        .iter()
        .filter(|issue| {
            issue.message.contains(&artifact_path)
                || issue.path.contains("export")
                || issue.message.contains("export artifact")
        })
        .collect::<Vec<_>>();
    assert!(artifact_issues.is_empty(), "{artifact_issues:?}");
}
