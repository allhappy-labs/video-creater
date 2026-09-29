use super::*;
use crate::project::model::{
    GeneratedAsset, GeneratedAssetOutput, GeneratedAssetReferences, GeneratedAssetSettings,
    GeneratedAssetStatus, GenerationModel, JobStatus, JobSummary, MediaAsset, MediaKind,
    TemporalWorkflowMetadata, TimelineItem, TimelineItemKind, TimelineSource, Transcript,
    VideoProject,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn video_edit_context_summarizes_current_timeline_for_agents() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: Vec::new(),
    });
    project.timeline.duration_seconds = 3.6;
    project.timeline.tracks[0].items.push(TimelineItem {
        id: "clip-1".to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: 0.0,
        duration_seconds: 3.6,
        source: TimelineSource::Media {
            media_id: "media-1".to_string(),
        },
        label: "Hook clip".to_string(),
        properties: BTreeMap::from([
            ("sourceIn".to_string(), json!(0.2)),
            ("sourceOut".to_string(), json!(3.8)),
            ("reason".to_string(), json!("hook")),
        ]),
    });

    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: crate::edit::preset::EditPreset::TrailerCut,
        prompt: "Make an action edit".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: crate::edit::preset::LanguageMode::English,
        caption_style: crate::edit::preset::CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let context = build_video_edit_context(&project, &request).expect("context");

    assert!(context
        .timeline_summary
        .contains("- durationSeconds: 3.600"));
    assert!(context
        .timeline_summary
        .contains("- track track-video: Video | video | unlocked | enabled | items: 1"));
    assert!(context.timeline_summary.contains(
        "- item clip-1: video_clip | 0.000-3.600s | source: media media-1 | label: Hook clip | sourceIn: 0.200 | sourceOut: 3.800 | reason: hook"
    ));
}

#[test]
fn video_edit_context_lists_split_project_indexes_for_agents() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.schema_version = 2;
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: Vec::new(),
    });

    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: crate::edit::preset::EditPreset::TrailerCut,
        prompt: "Make an action edit".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: crate::edit::preset::LanguageMode::English,
        caption_style: crate::edit::preset::CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let context =
        build_video_edit_context_with_project_dir(&project, &request, Path::new("/tmp/edit"))
            .expect("context");

    for expected_path in [
        "/tmp/edit/timeline.json",
        "/tmp/edit/media/index.json",
        "/tmp/edit/transcripts/index.json",
        "/tmp/edit/templates/index.json",
        "/tmp/edit/generated/index.json",
        "/tmp/edit/renders/index.json",
        "/tmp/edit/jobs/index.json",
        "/tmp/edit/exports/index.json",
        "/tmp/edit/context/project.json",
        "/tmp/edit/transcripts/<media-id>.json",
        "/tmp/edit/templates/<template-id>.json",
        "/tmp/edit/generated/<asset-id>/asset.json",
        "/tmp/edit/renders/<render-id>/report.json",
        "/tmp/edit/jobs/<job-id>/job.json",
        "/tmp/edit/exports/<export-id>/artifact.json",
    ] {
        assert!(
            context.project_files_summary.contains(expected_path),
            "missing project file path {expected_path}"
        );
    }
    assert!(context
        .project_files_summary
        .contains("scan indexes before opening canonical sidecars"));
}

#[test]
fn video_edit_context_summarizes_generated_asset_provenance_for_agents() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: Vec::new(),
    });
    project.generated_assets.push(GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Completed,
        name: Some("Hero product reveal".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "slow push-in on the product with warm sunset light".to_string(),
        model: GenerationModel {
            provider: "seedance".to_string(),
            id: "seedance-2-fast".to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: vec!["media-style-ref".to_string()],
            first_frame_media_id: Some("media-first-frame".to_string()),
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..GeneratedAssetReferences::default()
        },
        settings: GeneratedAssetSettings {
            width: Some(1080),
            height: Some(1920),
            duration_seconds: Some(4.0),
            fps: Some(30.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
        outputs: vec![GeneratedAssetOutput {
            media_id: "generated-shot-1-output".to_string(),
            relative_path: "generated/generated-shot-1/output.mp4".to_string(),
            source_url: None,
            width: 1080,
            height: 1920,
            duration_seconds: 4.0,
            fps: 30.0,
        }],
        created_at: "2026-06-22T10:00:00Z".to_string(),
        parent_asset_id: Some("generated-parent-1".to_string()),
        retry_of_asset_id: None,
    });

    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: crate::edit::preset::EditPreset::TrailerCut,
        prompt: "Make an action edit".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: crate::edit::preset::LanguageMode::English,
        caption_style: crate::edit::preset::CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let context = build_video_edit_context(&project, &request).expect("context");

    assert!(context.generated_assets_summary.contains(
        "- asset generated-shot-1: generated | completed | model: seedance/seedance-2-fast | settings: 1080x1920, 4.000s, 30.000fps, aspect 9:16 | prompt: slow push-in on the product with warm sunset light | firstFrame: media-first-frame | lastFrame: none | references: media-style-ref | parent: generated-parent-1 | retryOf: none"
    ));
    assert!(context.generated_assets_summary.contains(
        "- output generated-shot-1-output: generated/generated-shot-1/output.mp4 | 1080x1920 | 4.000s | fps: 30.000"
    ));
}

#[test]
fn video_edit_context_summarizes_workflow_jobs_for_agents() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: Vec::new(),
    });
    project.jobs.push(JobSummary {
        id: "generate-shot-1".to_string(),
        kind: "generate_media".to_string(),
        status: JobStatus::Running,
        updated_at: "2026-06-23T12:00:00Z".to_string(),
        workflow: Some(TemporalWorkflowMetadata {
            workflow_id: "video-creater/project-1/generate-media/generate-shot-1".to_string(),
            workflow_type: "VideoCreaterGenerateMediaWorkflow".to_string(),
            task_queue: "video-creater-workflows".to_string(),
            run_id: Some("run-123".to_string()),
            activity_types: vec![
                "BuildFalGenerationRequest".to_string(),
                "RunMediaProviderGeneration".to_string(),
            ],
        }),
        start_request: None,
        provider_request: None,
        failure_reason: None,
        export_settings: None,
    });

    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: crate::edit::preset::EditPreset::TrailerCut,
        prompt: "Make an action edit".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: crate::edit::preset::LanguageMode::English,
        caption_style: crate::edit::preset::CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let context = build_video_edit_context(&project, &request).expect("context");

    assert!(context.workflow_jobs_summary.contains(
        "Use recordJob before starting durable work, updateJobStatus as Temporal workflows progress, and updateGeneratedAssetStatus when generation assets move between queued, running, failed, or completed."
    ));
    assert!(context.workflow_jobs_summary.contains(
        "- job generate-shot-1: generate_media | running | updated: 2026-06-23T12:00:00Z | workflow: VideoCreaterGenerateMediaWorkflow @ video-creater-workflows | workflowId: video-creater/project-1/generate-media/generate-shot-1 | runId: run-123 | activities: BuildFalGenerationRequest, RunMediaProviderGeneration"
    ));
}

#[test]
fn video_edit_context_summarizes_export_capabilities_for_agents() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-06-12T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1080),
        height: Some(1920),
        fps: Some(30.0),
        folder_id: None,
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        repairs: Vec::new(),
        segments: Vec::new(),
        words: Vec::new(),
    });

    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: crate::edit::preset::EditPreset::TrailerCut,
        prompt: "Make an action edit".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: crate::edit::preset::LanguageMode::English,
        caption_style: crate::edit::preset::CaptionStyle::Bold,
        created_at: "2026-06-12T00:00:00Z".to_string(),
    };

    let context = build_video_edit_context(&project, &request).expect("context");

    assert!(context
        .export_capabilities_summary
        .contains("Supported exports: draft WebM, final WebM, Premiere XMEML, DaVinci FCPXML."));
    assert!(context.export_capabilities_summary.contains(
        "NLE XML command: export_nle_xml_to_split_project_folder(format: premiereXmeml | davinciFcpxml)."
    ));
    assert!(context.export_capabilities_summary.contains(
        "Temporal workflow: export_nle_xml uses VideoCreaterExportNleXmlWorkflow on video-creater-workflows."
    ));
    #[cfg(target_os = "macos")]
    assert!(context.export_capabilities_summary.contains(
        "MP4/H.264/H.265/ProRes: use the bundled macOS AVFoundation exporter when available; reviewed GStreamer factories remain the compatibility fallback."
    ));
    #[cfg(not(target_os = "macos"))]
    {
        assert!(!context.export_capabilities_summary.contains("AVFoundation"));
        assert!(context
            .export_capabilities_summary
            .contains("H.265 is available only when a VA-API hardware encoder is present"));
        assert!(context
            .export_capabilities_summary
            .contains("ProRes uses the FFmpeg ProRes encoder"));
    }
}
