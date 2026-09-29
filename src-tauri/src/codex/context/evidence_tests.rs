use super::*;
use crate::project::model::{
    MediaAsset, MediaKind, ProjectRenderReport, ProjectTemplateOverride, RenderReportCheckStatus,
    RenderReportStatus, RenderReportStreams, Transcript, VideoProject,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn video_edit_context_summarizes_template_overrides_for_agents() {
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
    project.template_overrides.push(ProjectTemplateOverride {
        schema_version: 1,
        template_id: "kinetic-lower-third-v1".to_string(),
        name: "Launch Lower Third".to_string(),
        fields: BTreeMap::from([
            ("headline".to_string(), "Launch day".to_string()),
            (
                "subline".to_string(),
                "Built with Video Creater".to_string(),
            ),
        ]),
        style: BTreeMap::from([
            ("accentColor".to_string(), json!("#22d3ee")),
            ("backgroundColor".to_string(), json!("rgba(2, 6, 23, 0.72)")),
        ]),
        visual_treatment: "compact translucent lower third with cyan accent and strong hierarchy"
            .to_string(),
        motion: "slide in, hold, soft fade".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
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

    assert!(context.template_overrides_summary.contains(
        "- template kinetic-lower-third-v1: Launch Lower Third | fields: headline=Launch day, subline=Built with Video Creater | style: accentColor=#22d3ee, backgroundColor=rgba(2, 6, 23, 0.72) | visualTreatment: compact translucent lower third with cyan accent and strong hierarchy | motion: slide in, hold, soft fade | safeZone: keep essential text inside 10% margins | avoid: full-width opaque black slabs"
    ));
}

#[test]
fn video_edit_context_summarizes_render_reports_for_agents() {
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
    project.render_reports.push(ProjectRenderReport {
        schema_version: 1,
        id: "render-draft-1".to_string(),
        status: RenderReportStatus::Completed,
        output_path: "renders/render-draft-1/output.mp4".to_string(),
        duration_seconds: 45.0,
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
                RenderReportCheckStatus::Skipped,
            ),
            ("overlayTiming".to_string(), RenderReportCheckStatus::Passed),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts: vec![
            "renders/render-draft-1/output.mp4".to_string(),
            "renders/render-draft-1/report.json".to_string(),
        ],
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: "logs/render-draft-1.log".to_string(),
        created_at: "2026-06-23T10:00:00Z".to_string(),
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

    assert!(
        context.render_reports_summary.contains(
            "- report render-draft-1: completed | output: renders/render-draft-1/output.mp4 | duration: 45.000s | streams: video=true,audio=true | checks: artifactPaths=passed, captionAlignment=skipped, duration=passed, logPath=passed, overlayTiming=passed, streams=passed | visualEvidence: frameArtifacts=0 | artifacts: renders/render-draft-1/output.mp4, renders/render-draft-1/report.json | log: logs/render-draft-1.log"
        ),
        "{}",
        context.render_reports_summary
    );
}

#[test]
fn video_edit_context_summarizes_render_frame_evidence_for_agents() {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Codex Test".to_string(),
        "2026-07-04T00:00:00Z".to_string(),
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
    project.render_reports.push(ProjectRenderReport {
        schema_version: 1,
        id: "render-draft-1".to_string(),
        status: RenderReportStatus::Completed,
        output_path: "renders/render-draft-1/output.webm".to_string(),
        duration_seconds: 30.0,
        quality: None,
        requested_width: None,
        requested_height: None,
        actual_width: None,
        actual_height: None,
        streams: RenderReportStreams {
            video: true,
            audio: false,
        },
        checks: BTreeMap::from([
            ("duration".to_string(), RenderReportCheckStatus::Passed),
            (
                "captionAlignment".to_string(),
                RenderReportCheckStatus::Passed,
            ),
            ("overlayTiming".to_string(), RenderReportCheckStatus::Passed),
            ("artifactPaths".to_string(), RenderReportCheckStatus::Passed),
            ("streams".to_string(), RenderReportCheckStatus::Passed),
            ("logPath".to_string(), RenderReportCheckStatus::Passed),
        ]),
        artifacts: vec![
            "renders/render-draft-1/output.webm".to_string(),
            "renders/render-draft-1/report.json".to_string(),
            "renders/render-draft-1/graphics/proposal-caption-1/frames/frame-000000.png"
                .to_string(),
            "renders/render-draft-1/graphics/proposal-overlay-1/frames/frame-000000.png"
                .to_string(),
        ],
        preview_comparison_request: None,
        preview_comparison: None,
        log_path: "renders/render-draft-1/render.log".to_string(),
        created_at: "2026-07-04T10:00:00Z".to_string(),
    });

    let request = EditJobRequest {
        media_id: "media-1".to_string(),
        preset: crate::edit::preset::EditPreset::TrailerCut,
        prompt: "Make an action edit".to_string(),
        target_duration_seconds: Some(30.0),
        language_mode: crate::edit::preset::LanguageMode::English,
        caption_style: crate::edit::preset::CaptionStyle::Bold,
        created_at: "2026-07-04T00:00:00Z".to_string(),
    };

    let context = build_video_edit_context(&project, &request).expect("context");

    assert!(context
        .render_reports_summary
        .contains("visualEvidence: frameArtifacts=2"));
}

#[test]
fn developer_instructions_use_the_compiled_mandatory_skill_catalog() {
    let bundle = ProjectSkillBundle {
        agents_md: "project agents".to_string(),
        video_pipeline: "corrupted pipeline".to_string(),
        graphics: "corrupted graphics".to_string(),
        visuals: "corrupted visuals".to_string(),
        shader_background_catalog: "shader profiles".to_string(),
    };

    let instructions = build_codex_developer_instructions(&bundle);

    for definition in crate::settings::skills::MANDATORY_SKILLS {
        assert!(instructions.contains(&format!("### {}", definition.id)));
        assert!(instructions.contains(definition.bundled_content));
    }
    assert!(!instructions.contains("corrupted pipeline"));
    assert!(!instructions.contains("corrupted graphics"));
    assert!(!instructions.contains("corrupted visuals"));
}
