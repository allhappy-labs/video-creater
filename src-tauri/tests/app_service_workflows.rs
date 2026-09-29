use std::sync::Arc;

use serde_json::json;
use video_creater_lib::app_service::agents::AgentService;
use video_creater_lib::app_service::error::ServiceErrorCode;
use video_creater_lib::app_service::events::FakeEventSink;
use video_creater_lib::app_service::exports::{ExportService, RenderReviewEvidence};
use video_creater_lib::app_service::jobs::{JobProgress, JobService};
use video_creater_lib::codex::proposal::{CodexEditProposal, CodexRenderReview};

#[test]
fn visual_proposal_without_a_real_edl_is_rejected_before_materialization() {
    let proposal = CodexEditProposal {
        media_id: "media-1".into(),
        clips: Vec::new(),
        captions: vec![json!({
            "text": "Looks finished",
            "startSeconds": 0.0,
            "durationSeconds": 2.0,
            "visualTreatment": "compact caption",
            "motion": "quick fade",
            "safeZone": "ten percent margins",
            "avoid": "opaque slabs"
        })],
        overlays: Vec::new(),
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 2.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    };

    let error = AgentService::validate_edl_precedes_visuals(&proposal)
        .expect_err("visual layers without selected source ranges must fail");
    assert_eq!(error.code(), ServiceErrorCode::InvalidInput);
}

#[test]
fn render_completion_requires_duration_stream_timing_artifact_and_log_evidence() {
    let complete = RenderReviewEvidence {
        expected_duration_seconds: 12.0,
        actual_duration_seconds: 12.01,
        has_video_stream: true,
        has_audio_stream: true,
        captions_aligned: true,
        overlays_aligned: true,
        artifact_id: Some("artifact-1".into()),
        log_reference: Some("render-log-1".into()),
    };
    ExportService::validate_render_review(&complete).expect("complete render evidence");

    let error = ExportService::validate_render_review(&RenderReviewEvidence {
        artifact_id: None,
        ..complete
    })
    .expect_err("missing artifact must fail");
    assert_eq!(error.code(), ServiceErrorCode::InvalidInput);
}

#[test]
fn job_progress_uses_the_shared_ordered_event_sink() {
    let events = Arc::new(FakeEventSink::default());
    let jobs = JobService::new(events.clone());
    jobs.publish_progress(JobProgress {
        project_id: "project-1".into(),
        job_id: "job-1".into(),
        fraction: 0.25,
        state: "running".into(),
    })
    .expect("progress event");

    let recorded = events.recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].topic, "job.progress");
    assert_eq!(recorded[0].sequence, 1);
}
