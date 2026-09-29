use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use video_creater_lib::codex::proposal::{CodexEditProposal, CodexProposalClip, CodexRenderReview};
use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use video_creater_lib::edit::render_plan::{
    RenderClip, RenderOutputProfile, RenderPlan, RenderQuality, RenderQualityProfile,
};
use video_creater_lib::gpu_graphics::error::{GpuGraphicsError, GpuGraphicsErrorCode};
use video_creater_lib::gpu_graphics::ir::GpuGraphicRole;
use video_creater_lib::graphics::error::{ActionableError, GraphicsErrorCode};
use video_creater_lib::graphics::ir::{
    Dimensions, Easing, GraphicNode, GraphicRole, GraphicsLayer,
};
use video_creater_lib::graphics::manifest::{graphics_playback_manifest, GraphicsArtifactManifest};
use video_creater_lib::graphics::validation::validate_graphics_layer;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::project::model::{
    MediaAsset, MediaKind, RenderReportCheckStatus, RenderReportStatus, TimelineItemKind,
    TrackKind, VideoProject,
};
use video_creater_lib::render_pipeline::backend::RenderBackend;
use video_creater_lib::render_pipeline::codex_e2e::{
    parse_codex_e2e_args, CodexE2eConfig, CodexE2eReport, CodexE2eToolAcceptance,
};
use video_creater_lib::render_pipeline::codex_e2e::{
    run_codex_e2e_cli_with_client, run_codex_e2e_with_client, CodexE2eAppServerClient,
    CodexE2eTurnOutput,
};
use video_creater_lib::render_pipeline::combined_e2e::{
    build_combined_project, parse_combined_e2e_args, run_combined_e2e_with_runner,
    CombinedE2eConfig,
};
use video_creater_lib::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use video_creater_lib::render_pipeline::graphics_cache::{
    graphics_layer_fingerprint, validate_graphics_cache_hit, write_graphics_cache_metadata,
    GraphicsCacheLookup,
};
#[cfg(all(feature = "ges-render", target_os = "macos"))]
use video_creater_lib::render_pipeline::gstreamer_backend::{
    encoding_profile_summary_for_test, probe_media_with_gstreamer,
};
use video_creater_lib::render_pipeline::gstreamer_backend::{
    generate_fixture_source_with_gstreamer, GstreamerGesRenderBackend,
};
#[cfg(feature = "ges-render")]
use video_creater_lib::render_pipeline::gstreamer_backend::{
    required_input_decode_factories_for_platform_for_test,
    required_input_decode_factories_for_test, webm_encoding_profile_summary_for_test,
};
use video_creater_lib::render_pipeline::output_profile::{
    gstreamer_output_profile_target, GstreamerAudioMode,
};
use video_creater_lib::render_pipeline::performance::RenderPerformanceRecorder;
use video_creater_lib::render_pipeline::plugin_policy::{
    evaluate_gstreamer_factory, policy_error_for_factory, GstFactoryInfo, PluginPolicyVerdict,
};
use video_creater_lib::render_pipeline::probe::{
    validate_rendered_media, AudioProbe, ExpectedMedia, MediaProbe, VideoProbe,
};
use video_creater_lib::render_pipeline::process::{
    CommandSpec, ProcessOutput, ProcessRunner, SystemProcessRunner,
};
use video_creater_lib::render_pipeline::project_export::{
    build_preview_render_comparison_request, extract_rendered_frame_samples,
    project_render_report_from_pipeline_report, rendered_frame_sample_times_for_graphics_starts,
    ProjectRenderReportEvidence, ProjectWebmRenderPaths,
};
use video_creater_lib::render_pipeline::proposal::{
    build_render_proposal_preview, codex_report_from_json, parse_render_proposal_args,
    probe_source_video_with_cache, proposal_gpu_visuals_to_layers_for_duration,
    proposal_to_render_plan, proposal_visuals_to_graphics_layers, render_proposal_graphics,
    render_proposal_graphics_with_gpu_renderer, run_render_proposal, CodexProposalReport,
    RenderProposalConfig,
};
use video_creater_lib::render_pipeline::quality::effective_quality_settings;
use video_creater_lib::render_pipeline::report::{
    write_json_report, write_markdown_report, RenderGraphicsReport, RenderPerformanceSummary,
    RenderPreviewComparison, RenderPreviewComparisonFrame, RenderPreviewComparisonRequest,
    RenderReport, RenderReportStreams, RenderReportSummary, RenderStageReport,
};
use video_creater_lib::render_pipeline::source_probe_cache::{
    validate_source_probe_cache_hit, write_source_probe_cache, SourceProbeCacheLookup,
};
#[cfg(feature = "ges-render")]
use video_creater_lib::render_runtime::start_render_process_runtime;

#[path = "render_pipeline/master_encoding.rs"]
mod master_encoding;
#[path = "render_pipeline/reverse.rs"]
mod reverse;
#[path = "render_pipeline/transition_backends.rs"]
mod transition_backends;
#[path = "render_pipeline/transition_fixtures.rs"]
mod transition_fixtures;
#[path = "render_pipeline/transition_plan.rs"]
mod transition_plan;

#[test]
fn render_quality_serializes_profile_neutral_values() {
    assert_eq!(
        serde_json::to_value(RenderQuality::Draft).expect("draft quality"),
        serde_json::json!("draft")
    );
    assert_eq!(
        serde_json::to_value(RenderQuality::Final).expect("final quality"),
        serde_json::json!("final")
    );
}

#[test]
fn render_output_profile_serializes_profile_neutral_values() {
    assert_eq!(
        serde_json::to_value(RenderOutputProfile::Mp4Primary).expect("mp4 primary profile"),
        serde_json::json!("mp4Primary")
    );
    assert_eq!(
        serde_json::to_value(RenderOutputProfile::QuicktimeInterchange)
            .expect("quicktime interchange profile"),
        serde_json::json!("quicktimeInterchange")
    );
}

#[cfg(target_os = "macos")]
#[test]
fn mp4_h264_profile_resolves_to_audiotoolbox_aac_target() {
    let target = gstreamer_output_profile_target(ExportProfile::Mp4H264).expect("mp4 h264 target");

    assert_eq!(target.extension, "mp4");
    assert_eq!(target.mime_type, "video/mp4");
    assert_eq!(target.container_factory, "mp4mux");
    assert_eq!(target.video_factory, "vtenc_h264");
    assert_eq!(target.audio_factory, Some("atenc"));
    assert_eq!(target.parser_factories, vec!["aacparse"]);
    assert_eq!(
        target.required_factories(),
        vec!["mp4mux", "vtenc_h264", "atenc", "aacparse"]
    );
}

#[cfg(target_os = "macos")]
#[test]
fn prores_profile_resolves_to_quicktime_prores_pcm_target() {
    let target =
        gstreamer_output_profile_target(ExportProfile::ProResMov).expect("prores mov target");

    assert_eq!(target.extension, "mov");
    assert_eq!(target.mime_type, "video/quicktime");
    assert_eq!(target.container_factory, "qtmux");
    assert_eq!(target.video_factory, "vtenc_prores");
    assert_eq!(target.audio_factory, None);
    assert_eq!(target.audio_mode, GstreamerAudioMode::RawPcm);
    assert_eq!(target.required_factories(), vec!["qtmux", "vtenc_prores"]);
}

#[test]
fn pipeline_error_serializes_for_agent_repair() {
    let error = PipelineError::new(
        PipelineErrorCode::RenderBackendUnavailable,
        "render.jobs[0]",
        "GStreamer/GES backend is not available.",
        "Install the LGPL GStreamer runtime and Editing Services plug-ins.",
    )
    .with_detail("logPath", "renders/job-1/gstreamer.log");

    let json = serde_json::to_value(&error).expect("pipeline error should serialize");

    assert_eq!(json["code"], "RENDER_BACKEND_UNAVAILABLE");
    assert_eq!(json["path"], "render.jobs[0]");
    assert_eq!(json["message"], "GStreamer/GES backend is not available.");
    assert_eq!(
        json["fix"],
        "Install the LGPL GStreamer runtime and Editing Services plug-ins."
    );
    assert_eq!(json["details"]["logPath"], "renders/job-1/gstreamer.log");
}

#[test]
fn render_report_serializes_optional_performance_summary() {
    let report = RenderReport {
        job_id: "perf-report".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(2.5),
            output_path: Some("renders/out.webm".to_string()),
            ..RenderReportSummary::default()
        },
        command: CommandSpec::new("gstreamer-ges").arg("--quality=draftWebm"),
        stdout: String::new(),
        stderr: String::new(),
        streams: None,
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: Vec::new(),
        performance: Some(RenderPerformanceSummary {
            total_duration_ms: 42,
            stages: vec![RenderStageReport {
                name: "sourceProbe".to_string(),
                status: "succeeded".to_string(),
                duration_ms: 7,
                details: BTreeMap::from([("path".to_string(), "source.webm".to_string())]),
            }],
        }),
        preview_comparison_request: None,
        preview_comparison: None,
    };

    let json = serde_json::to_value(&report).expect("report serializes");

    assert_eq!(json["performance"]["totalDurationMs"], 42);
    assert_eq!(json["performance"]["stages"][0]["name"], "sourceProbe");
    assert_eq!(
        json["performance"]["stages"][0]["details"]["path"],
        "source.webm"
    );
}

#[test]
fn markdown_report_includes_performance_section() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("render-report.md");
    let report = RenderReport {
        job_id: "perf-markdown".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: None,
            output_path: None,
            ..RenderReportSummary::default()
        },
        command: CommandSpec::new("gstreamer-ges"),
        stdout: String::new(),
        stderr: String::new(),
        streams: Some(RenderReportStreams {
            video: true,
            audio: false,
        }),
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: Vec::new(),
        performance: Some(RenderPerformanceSummary {
            total_duration_ms: 125,
            stages: vec![RenderStageReport {
                name: "graphics".to_string(),
                status: "succeeded".to_string(),
                duration_ms: 90,
                details: BTreeMap::from([("cache".to_string(), "miss".to_string())]),
            }],
        }),
        preview_comparison_request: None,
        preview_comparison: None,
    };

    write_markdown_report(&path, &report).expect("write markdown report");
    let markdown = std::fs::read_to_string(path).expect("read markdown");

    assert!(markdown.contains("## Performance"));
    assert!(markdown.contains("- Total: 125ms"));
    assert!(markdown.contains("| graphics | succeeded | 90ms | cache=miss |"));
    assert!(markdown.contains("## Streams"));
    assert!(markdown.contains("- video=true"));
    assert!(markdown.contains("- audio=false"));
}

#[test]
fn performance_recorder_records_successful_stage() {
    let mut recorder = RenderPerformanceRecorder::start();

    let value = recorder
        .measure_stage(
            "sourceProbe",
            BTreeMap::from([("path".to_string(), "in.webm".to_string())]),
            || Ok::<_, Vec<PipelineError>>(7),
        )
        .expect("stage should succeed");

    let summary = recorder.finish();

    assert_eq!(value, 7);
    assert_eq!(summary.stages.len(), 1);
    assert_eq!(summary.stages[0].name, "sourceProbe");
    assert_eq!(summary.stages[0].status, "succeeded");
    assert_eq!(summary.stages[0].details["path"], "in.webm");
}

#[test]
fn performance_recorder_records_failed_stage_without_rewriting_error() {
    let mut recorder = RenderPerformanceRecorder::start();
    let error = PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "stage",
        "stage failed",
        "fix stage",
    );

    let errors = recorder
        .measure_stage("gesRender", BTreeMap::new(), || {
            Err::<(), _>(vec![error.clone()])
        })
        .expect_err("stage should fail");
    let summary = recorder.finish();

    assert_eq!(errors, vec![error]);
    assert_eq!(summary.stages.len(), 1);
    assert_eq!(summary.stages[0].name, "gesRender");
    assert_eq!(summary.stages[0].status, "failed");
}

#[test]
fn graphics_cache_misses_when_metadata_is_missing() {
    let temp = tempfile::tempdir().expect("temp dir");
    let layer = sample_cache_graphics_layer("cache-layer");
    let fingerprint = graphics_layer_fingerprint(&layer, "rust", None).expect("fingerprint");

    let result = validate_graphics_cache_hit(temp.path(), &layer, "rust", None, &fingerprint)
        .expect("cache miss is not an error");

    assert_eq!(
        result,
        GraphicsCacheLookup::Miss("metadata missing".to_string())
    );
}

#[test]
fn graphics_cache_fingerprint_changes_for_motion_v2_fields() {
    let mut base = sample_cache_graphics_layer("motion-v2-cache");
    base.nodes = vec![serde_json::from_value(serde_json::json!({
        "type": "text",
        "id": "headline",
        "text": "Cache",
        "box": { "x": 20.0, "y": 20.0, "width": 180.0, "height": 48.0 },
        "fontSize": 28.0,
        "fontWeight": 700,
        "align": "left",
        "fill": "#FFFFFFFF",
        "maxLines": 1
    }))
    .expect("cache text node")];
    let mut changed = base.clone();

    if let GraphicNode::Text(text) = &mut base.nodes[0] {
        text.animate = Some(
            serde_json::from_value(serde_json::json!({
                "keyframes": [
                    { "at": 0.0, "rotationDegrees": 0.0 },
                    { "at": 1.0, "rotationDegrees": 0.0 }
                ]
            }))
            .expect("base animation"),
        );
    }
    if let GraphicNode::Text(text) = &mut changed.nodes[0] {
        text.animate = Some(
            serde_json::from_value(serde_json::json!({
                "keyframes": [
                    { "at": 0.0, "rotationDegrees": 0.0 },
                    { "at": 1.0, "rotationDegrees": 6.0 }
                ]
            }))
            .expect("changed animation"),
        );
    }

    let base_fingerprint =
        graphics_layer_fingerprint(&base, "rust", None).expect("base fingerprint");
    let changed_fingerprint =
        graphics_layer_fingerprint(&changed, "rust", None).expect("changed fingerprint");

    assert_ne!(base_fingerprint, changed_fingerprint);
}

#[test]
fn graphics_cache_hits_complete_matching_artifact() {
    let temp = tempfile::tempdir().expect("temp dir");
    let layer = sample_cache_graphics_layer("cache-layer");
    let fingerprint = graphics_layer_fingerprint(&layer, "rust", None).expect("fingerprint");
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "cache-layer.preview".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count: 1,
        frames_pattern: "frames/frame-%06d.png".to_string(),
        playback: graphics_playback_manifest(
            1,
            layer.fps,
            layer.duration_seconds,
            false,
            "frames/frame-%06d.png",
        ),
        preview_path: PathBuf::from("preview.png"),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };
    let frames_dir = temp.path().join("frames");
    std::fs::create_dir_all(&frames_dir).expect("frames dir");
    std::fs::write(temp.path().join("preview.png"), b"preview").expect("preview");
    std::fs::write(frames_dir.join("frame-000000.png"), b"frame").expect("frame");
    write_graphics_cache_metadata(temp.path(), &fingerprint, "rust", None).expect("metadata");
    std::fs::write(
        temp.path().join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest json"),
    )
    .expect("manifest");

    let result = validate_graphics_cache_hit(temp.path(), &layer, "rust", None, &fingerprint)
        .expect("cache lookup");

    assert_eq!(result, GraphicsCacheLookup::Hit(manifest));
}

#[test]
fn graphics_cache_misses_on_stale_fingerprint() {
    let temp = tempfile::tempdir().expect("temp dir");
    let layer = sample_cache_graphics_layer("cache-layer");
    write_graphics_cache_metadata(temp.path(), "stale", "rust", None).expect("metadata");

    let result = validate_graphics_cache_hit(temp.path(), &layer, "rust", None, "fresh")
        .expect("cache lookup");

    assert_eq!(
        result,
        GraphicsCacheLookup::Miss("fingerprint mismatch".to_string())
    );
}

#[test]
fn source_probe_cache_misses_when_metadata_is_missing() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("source.webm");
    std::fs::write(&source_path, b"source").expect("source file");

    let result =
        validate_source_probe_cache_hit(&temp.path().join("source-probe-cache.json"), &source_path)
            .expect("cache miss is not an error");

    assert_eq!(
        result,
        SourceProbeCacheLookup::Miss("metadata missing".to_string())
    );
}

#[test]
fn source_probe_cache_hits_matching_source_signature() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("source.webm");
    let cache_path = temp.path().join("source-probe-cache.json");
    std::fs::write(&source_path, b"source").expect("source file");
    let probe = media_probe(1280, 720, 4.0, true);

    write_source_probe_cache(&cache_path, &source_path, &probe).expect("write cache");
    let result = validate_source_probe_cache_hit(&cache_path, &source_path).expect("cache lookup");

    assert_eq!(result, SourceProbeCacheLookup::Hit(probe));
}

#[test]
fn source_probe_cache_misses_when_source_size_changes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("source.webm");
    let cache_path = temp.path().join("source-probe-cache.json");
    std::fs::write(&source_path, b"source").expect("source file");
    let probe = media_probe(1280, 720, 4.0, true);

    write_source_probe_cache(&cache_path, &source_path, &probe).expect("write cache");
    std::fs::write(&source_path, b"source changed").expect("rewrite source file");
    let result = validate_source_probe_cache_hit(&cache_path, &source_path).expect("cache lookup");

    assert_eq!(
        result,
        SourceProbeCacheLookup::Miss("source signature mismatch".to_string())
    );
}

#[test]
fn probe_source_video_with_cache_uses_cached_probe_without_running_gstreamer() {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_path = temp.path().join("source.webm");
    let cache_path = temp.path().join("source-probe-cache.json");
    std::fs::write(&source_path, b"source").expect("source file");
    let probe = media_probe(1280, 720, 4.0, true);
    write_source_probe_cache(&cache_path, &source_path, &probe).expect("write cache");
    let fallback_called = Cell::new(false);

    let (cached_probe, output, cache_status) =
        probe_source_video_with_cache(&source_path, &cache_path, || {
            fallback_called.set(true);
            Ok((
                media_probe(640, 360, 1.0, false),
                ProcessOutput {
                    status_code: Some(0),
                    stdout: "{}".to_string(),
                    stderr: String::new(),
                },
            ))
        })
        .expect("cached source probe should load");

    assert!(!fallback_called.get());
    assert_eq!(cached_probe, probe);
    assert_eq!(output.status_code, Some(0));
    assert!(output.stdout.contains("durationSeconds"));
    assert_eq!(output.stderr, "");
    assert_eq!(cache_status, "hit");
}

#[test]
fn draft_quality_settings_retain_dimensions_and_cap_fps() {
    let settings = effective_quality_settings(RenderQuality::Draft, 1920, 1080, 60.0);

    assert_eq!(settings.output_width, 1920);
    assert_eq!(settings.output_height, 1080);
    assert_eq!(settings.output_fps, 24.0);
    assert_eq!(settings.speed_hint, "draft-fast");
}

#[test]
fn final_quality_settings_preserve_source_dimensions_and_fps() {
    let settings = effective_quality_settings(RenderQuality::Final, 1920, 1080, 60.0);

    assert_eq!(settings.output_width, 1920);
    assert_eq!(settings.output_height, 1080);
    assert_eq!(settings.output_fps, 60.0);
    assert_eq!(settings.video_bitrate_kbps, Some(12_000));
    assert_eq!(settings.speed_hint, "final-quality");
}

#[test]
fn graphics_errors_convert_to_pipeline_errors_without_losing_action() {
    let graphics_error = ActionableError::new(
        GraphicsErrorCode::GraphicsImageRefUnauthorized,
        "layers[2].nodes[0].imageRef",
        "Image reference points outside the project media directory.",
        "Choose an imported project asset or import this image first.",
    )
    .with_detail("assetPath", "../private/image.png")
    .with_cause(GraphicsErrorCode::GraphicsImageRefMissing);

    let pipeline_errors = PipelineError::from_graphics_errors(vec![graphics_error]);

    assert_eq!(pipeline_errors.len(), 1);
    assert_eq!(
        pipeline_errors[0].code,
        PipelineErrorCode::GraphicsImageRefUnauthorized
    );
    let json = serde_json::to_value(&pipeline_errors[0]).expect("pipeline error serializes");
    assert_eq!(json["code"], "GRAPHICS_IMAGE_REF_UNAUTHORIZED");
    assert_eq!(pipeline_errors[0].path, "layers[2].nodes[0].imageRef");
    assert_eq!(
        pipeline_errors[0].message,
        "Image reference points outside the project media directory."
    );
    assert_eq!(
        pipeline_errors[0].fix,
        "Choose an imported project asset or import this image first."
    );
    assert_eq!(
        pipeline_errors[0].details.get("assetPath"),
        Some(&"../private/image.png".to_string())
    );
    assert_eq!(
        pipeline_errors[0].details.get("graphicsCode"),
        Some(&"GRAPHICS_IMAGE_REF_UNAUTHORIZED".to_string())
    );
    assert_eq!(
        pipeline_errors[0].details.get("graphicsCause"),
        Some(&"GRAPHICS_IMAGE_REF_MISSING".to_string())
    );
}

#[test]
fn pipeline_result_alias_collects_actionable_errors() {
    fn collect_errors() -> PipelineResult<()> {
        Err(vec![
            PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                "edl.clips[0]",
                "Clip sourceOut must be greater than sourceIn.",
                "Choose a positive source range for the clip.",
            ),
            PipelineError::new(
                PipelineErrorCode::RenderArtifactEmpty,
                "graphics.layers[0].frames",
                "No rendered frames were produced for the graphics layer.",
                "Render at least one frame before attaching the overlay.",
            )
            .with_detail("layerId", "title-card"),
        ])
    }

    let errors = collect_errors().expect_err("pipeline result should carry errors");
    let details = BTreeMap::from([("layerId".to_string(), "title-card".to_string())]);

    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[1].code, PipelineErrorCode::RenderArtifactEmpty);
    let json = serde_json::to_value(&errors[0]).expect("pipeline error serializes");
    assert_eq!(json["code"], "PIPELINE_INPUT_INVALID");
    assert_eq!(errors[1].details, details);
}

#[test]
fn codex_report_loader_requires_top_level_proposal() {
    let errors = codex_report_from_json(
        r#"{
          "generatedAt": "2026-06-17T00:00:00Z"
        }"#,
    )
    .expect_err("report without top-level proposal should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "proposal");
    assert!(
        errors[0].message.contains("proposal"),
        "message should name missing proposal: {}",
        errors[0].message
    );
}

#[test]
fn draft_proposal_render_plan_uses_selected_clip_ranges_and_hd_defaults() {
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 3840;
    project.render_settings.height = 2160;
    let request = sample_proposal_request();
    let proposal = sample_codex_proposal();

    let plan = proposal_to_render_plan(
        &project,
        &request,
        &proposal,
        Path::new("media/input.mp4"),
        Path::new("renders/draft.mp4"),
        RenderQuality::Draft,
    )
    .expect("valid proposal should become a render plan");

    assert_eq!(plan.input_path, "media/input.mp4");
    assert_eq!(plan.output_path, "renders/draft.mp4");
    assert_eq!(plan.width, 1280);
    assert_eq!(plan.height, 720);
    assert_eq!(plan.fps, 24.0);
    assert_eq!(plan.quality, RenderQuality::Draft);
    assert_eq!(
        plan.clips,
        vec![
            RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: BTreeMap::new(),
                timeline_track_index: 0,
                source_in: 1.5,
                source_out: 29.0,
            },
            RenderClip {
                source_path: None,
                timeline_start_seconds: None,
                properties: BTreeMap::new(),
                timeline_track_index: 0,
                source_in: 45.25,
                source_out: 62.75,
            },
        ]
    );
}

#[test]
fn proposal_render_plan_rejects_caption_outside_selected_duration() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut proposal = sample_codex_proposal();
    proposal.captions[0]["startSeconds"] = serde_json::json!(44.0);
    proposal.captions[0]["durationSeconds"] = serde_json::json!(2.0);

    let errors = proposal_to_render_plan(
        &project,
        &request,
        &proposal,
        Path::new("media/input.mp4"),
        Path::new("renders/draft.mp4"),
        RenderQuality::Draft,
    )
    .expect_err("caption that exceeds the selected edit duration should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "captions[0].durationSeconds");
    assert!(
        errors[0].fix.contains("selected edit duration"),
        "fix should tell the agent to keep timing within the selected edit duration: {}",
        errors[0].fix
    );
}

#[test]
fn proposal_render_plan_rejects_plain_overlay_outside_selected_duration() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut proposal = sample_codex_proposal();
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("sample overlay is an object");
    overlay.remove("templateId");
    overlay.insert("kind".to_string(), serde_json::json!("overlay"));
    overlay.insert("startSeconds".to_string(), serde_json::json!(44.0));
    overlay.insert("durationSeconds".to_string(), serde_json::json!(2.0));

    let errors = proposal_to_render_plan(
        &project,
        &request,
        &proposal,
        Path::new("media/input.mp4"),
        Path::new("renders/draft.mp4"),
        RenderQuality::Draft,
    )
    .expect_err("plain overlay that exceeds the selected edit duration should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "overlays[0].durationSeconds");
    assert!(
        errors[0].fix.contains("selected edit duration"),
        "fix should tell the agent to keep timing within the selected edit duration: {}",
        errors[0].fix
    );
}

#[test]
fn proposal_render_plan_rejects_hyperframe_outside_selected_duration() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut proposal = sample_codex_proposal();
    proposal.clips = vec![CodexProposalClip {
        media_id: "media-1".to_string(),
        source_in: 0.2,
        source_out: 30.2,
        reason: "validated selected primary EDL range".to_string(),
    }];
    proposal.render_review.duration_seconds = 30.0;
    proposal.captions = Vec::new();
    proposal.overlays = Vec::new();
    proposal.hyperframes = vec![sample_hyperframe_layer()];
    proposal.hyperframes[0]["startSeconds"] = serde_json::json!(29.0);
    proposal.hyperframes[0]["durationSeconds"] = serde_json::json!(2.0);

    let errors = proposal_to_render_plan(
        &project,
        &request,
        &proposal,
        Path::new("media/input.mp4"),
        Path::new("renders/draft.mp4"),
        RenderQuality::Draft,
    )
    .expect_err("HyperFrame that exceeds the selected edit duration should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "hyperframes[0].durationSeconds");
    assert!(
        errors[0].fix.contains("selected edit duration"),
        "fix should tell the agent to keep timing within the selected edit duration: {}",
        errors[0].fix
    );
}

#[test]
fn proposal_render_plan_accepts_visual_ending_at_selected_duration() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut proposal = sample_codex_proposal();
    proposal.clips = vec![CodexProposalClip {
        media_id: "media-1".to_string(),
        source_in: 0.2,
        source_out: 2.8,
        reason: "short exact-duration clip".to_string(),
    }];
    proposal.captions = Vec::new();
    proposal.overlays[0]["startSeconds"] = serde_json::json!(0.0);
    proposal.overlays[0]["durationSeconds"] = serde_json::json!(2.6);
    proposal.render_review.duration_seconds = 2.6;

    proposal_to_render_plan(
        &project,
        &request,
        &proposal,
        Path::new("media/input.mp4"),
        Path::new("renders/draft.mp4"),
        RenderQuality::Draft,
    )
    .expect("visual ending exactly at selected duration should be accepted");
}

#[test]
fn proposal_visuals_convert_to_valid_graphics_layers() {
    let proposal = sample_codex_proposal();

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("proposal visuals should convert");

    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].role, GraphicRole::Caption);
    assert_eq!(layers[1].role, GraphicRole::LowerThird);
    assert!(layers.iter().all(|layer| layer.alpha));
    assert!(layers.iter().all(|layer| layer.dimensions
        == (Dimensions {
            width: 1280,
            height: 720
        })));
    assert!(layers.iter().all(|layer| layer.fps == 30.0));
    for layer in &layers {
        validate_graphics_layer(layer).expect("converted layer should validate");
    }
}

#[test]
fn proposal_default_visuals_animate_captions_and_plain_overlays() {
    let mut proposal = sample_codex_proposal();
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("sample overlay is an object");
    overlay.remove("templateId");
    overlay.insert("kind".to_string(), serde_json::json!("overlay"));

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("proposal visuals should convert");

    let animated_node_count = |layer: &GraphicsLayer| {
        layer
            .nodes
            .iter()
            .filter(|node| match node {
                GraphicNode::Text(node) => node.animate.is_some(),
                GraphicNode::RoundedRect(node) => node.animate.is_some(),
                GraphicNode::Rect(node) => node.animate.is_some(),
                GraphicNode::Polygon(node) => node.animate.is_some(),
                GraphicNode::Line(node) => node.animate.is_some(),
                GraphicNode::ImageRef(node) => node.animate.is_some(),
                GraphicNode::HolographicLogo(node) => node.animate.is_some(),
            })
            .count()
    };

    assert_eq!(layers[0].role, GraphicRole::Caption);
    assert_eq!(layers[1].role, GraphicRole::Overlay);
    assert!(
        animated_node_count(&layers[0]) >= 2,
        "default captions should animate multiple nodes"
    );
    assert!(
        animated_node_count(&layers[1]) >= 2,
        "plain overlays should animate multiple nodes"
    );
}

#[test]
fn proposal_gpu_visuals_convert_to_gpu_layers() {
    let mut proposal = sample_codex_proposal();
    proposal.gpu_visuals = vec![sample_gpu_visual()];

    let layers = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect("gpu visuals should convert");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "shader-hook-bg");
    assert_eq!(layers[0].duration_seconds, 1.0);
    assert!(layers[0].background.is_some());
    assert!(layers[0].scene.is_some());
}

#[test]
fn proposal_gpu_visuals_reject_timing_outside_selected_duration() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["startSeconds"] = serde_json::json!(44.5);
    gpu_visual["durationSeconds"] = serde_json::json!(2.0);
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("gpu visual should be inside selected duration");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "gpuVisuals[0].durationSeconds");
}

#[test]
fn proposal_gpu_visuals_reject_missing_quality_profile() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual
        .as_object_mut()
        .expect("gpu visual object")
        .remove("qualityProfile");
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("missing quality profile should fail");

    assert_eq!(errors[0].path, "gpuVisuals[0].qualityProfile");
    assert!(errors[0].message.contains("quality profile"));
}

#[test]
fn proposal_gpu_visuals_reject_unknown_quality_profile() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["qualityProfile"] = serde_json::json!("experimental-profile");
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("unknown quality profile should fail");

    assert_eq!(errors[0].path, "gpuVisuals[0].qualityProfile");
    assert!(errors[0].fix.contains("hq-neon-wireframe-shader-v1"));
}

#[test]
fn proposal_gpu_visuals_expand_hq_profile_to_canonical_layer() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["shader"]["fragmentSource"] = serde_json::json!(
        "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(1.0, 0.0, 0.0, 1.0); }"
    );
    gpu_visual["visualTreatment"] = serde_json::json!("generic blob field");
    proposal.gpu_visuals = vec![gpu_visual];

    let layers = proposal_gpu_visuals_to_layers_for_duration(&proposal, 1920, 1080, 30.0, 45.0)
        .expect("hq profile should expand");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "shader-hook-bg");
    assert!(layers[0]
        .visual_treatment
        .contains("restrained animated gradient shader background"));
    assert!(layers[0].avoid.contains("oversized saturated blobs"));
    assert!(layers[0]
        .background
        .as_ref()
        .expect("background")
        .fragment_source
        .contains("hq_neon_palette"));
    let fragment_source = &layers[0]
        .background
        .as_ref()
        .expect("background")
        .fragment_source;
    assert!(fragment_source.contains("u_resolution.x"));
    assert!(!fragment_source.contains("1.7777778"));
    let scene = layers[0].scene.as_ref().expect("scene");
    assert!(scene
        .primitives
        .iter()
        .any(|primitive| primitive.primitive_type == "cube"));
    assert!(scene
        .primitives
        .iter()
        .any(|primitive| primitive.primitive_type == "grid"));
}

#[test]
fn proposal_gpu_visuals_expand_hq_profile_without_submitted_payload() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    let gpu_visual_object = gpu_visual.as_object_mut().expect("gpu visual object");
    gpu_visual_object.remove("shader");
    gpu_visual_object.remove("primitives");
    proposal.gpu_visuals = vec![gpu_visual];

    let layers = proposal_gpu_visuals_to_layers_for_duration(&proposal, 1920, 1080, 30.0, 45.0)
        .expect("hq profile should expand without submitted shader or primitives");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "shader-hook-bg");
    assert!(layers[0]
        .background
        .as_ref()
        .expect("background")
        .fragment_source
        .contains("hq_neon_palette"));
    let scene = layers[0].scene.as_ref().expect("scene");
    assert!(scene.primitives.len() >= 3);
    assert!(scene
        .primitives
        .iter()
        .any(|primitive| primitive.primitive_type == "cube"));
    assert!(scene
        .primitives
        .iter()
        .any(|primitive| primitive.primitive_type == "grid"));
}

#[test]
fn proposal_gpu_visuals_expand_collected_shadertoy_profile_to_shader_background() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["id"] = serde_json::json!("octagrams-bg");
    gpu_visual["kind"] = serde_json::json!("shader_background");
    gpu_visual["qualityProfile"] = serde_json::json!("shadertoy-octagrams-v1");
    proposal.gpu_visuals = vec![gpu_visual];

    let layers = proposal_gpu_visuals_to_layers_for_duration(&proposal, 1280, 720, 30.0, 45.0)
        .expect("collected Shadertoy profile should expand");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "octagrams-bg");
    assert_eq!(layers[0].role, GpuGraphicRole::ShaderBackground);
    assert_eq!(
        layers[0].quality_profile.as_deref(),
        Some("shadertoy-octagrams-v1")
    );
    assert!(layers[0].scene.is_none());
    let fragment_source = &layers[0]
        .background
        .as_ref()
        .expect("background")
        .fragment_source;
    assert!(fragment_source.contains("video_creater_fragment"));
    assert!(fragment_source.contains("vc_original_octagram_tunnel"));
    assert!(!fragment_source.contains("void mainImage"));
}

#[test]
fn proposal_gpu_visuals_reject_collected_shadertoy_profile_hybrid_kind() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["qualityProfile"] = serde_json::json!("shadertoy-octagrams-v1");
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("collected shader profile should require shader_background kind");

    assert_eq!(errors[0].path, "gpuVisuals[0].kind");
    assert!(errors[0].fix.contains("shader_background"));
    assert!(errors[0].fix.contains("shadertoy-octagrams-v1"));
}

#[test]
fn proposal_gpu_visuals_reject_missing_or_blank_source_beat() {
    for source_beat in [None, Some("   ")] {
        let mut proposal = sample_codex_proposal();
        let mut gpu_visual = sample_gpu_visual();
        match source_beat {
            Some(value) => gpu_visual["sourceBeat"] = serde_json::json!(value),
            None => {
                gpu_visual
                    .as_object_mut()
                    .expect("gpu visual object")
                    .remove("sourceBeat");
            }
        }
        proposal.gpu_visuals = vec![gpu_visual];

        let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
            .expect_err("missing or blank sourceBeat should fail");

        assert_eq!(errors[0].path, "gpuVisuals[0].sourceBeat");
        assert!(errors[0].message.contains("source beat"));
    }
}

#[test]
fn proposal_gpu_visuals_reject_hq_profile_shader_background_kind() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["kind"] = serde_json::json!("shader_background");
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("hq profile should require hybrid_scene kind");

    assert_eq!(errors[0].path, "gpuVisuals[0].kind");
    assert!(errors[0].message.contains("quality profile"));
    assert!(errors[0].fix.contains("hybrid_scene"));
}

#[test]
fn proposal_visuals_preserve_overlay_subline_fields() {
    let proposal = sample_codex_proposal();

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("proposal visuals should convert");

    let text_nodes = layers[1]
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphicNode::Text(text) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text_values = text_nodes
        .iter()
        .map(|text| text.text.as_str())
        .collect::<Vec<_>>();

    assert!(
        text_values.contains(&"Proposal Pipeline"),
        "overlay should render the headline text, got {text_values:?}"
    );
    assert!(
        text_values.contains(&"Rust render path"),
        "overlay should render the subline text, got {text_values:?}"
    );
    for text in text_nodes {
        assert!(
            text.box_rect.height >= text.font_size * 1.2,
            "text box should fit one rendered line for '{}': height {}, font {}",
            text.text,
            text.box_rect.height,
            text.font_size
        );
    }
}

#[test]
fn proposal_visuals_preserve_text_overlay_source_beat_fallback() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "fields": {
            "headline": "Fallback callout"
        },
        "visualTreatment": "clean callout card",
        "motion": "quick fade",
        "safeZone": "lower third",
        "avoid": "generic slabs"
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("proposal visuals should convert");

    assert_eq!(layers[1].source_beat, "Overlay callout: Fallback callout");
}

#[test]
fn proposal_visuals_accept_custom_overlay_primitive_nodes() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "draw a small arcade enemy formation",
        "visualTreatment": "pixel-art primitive overlay",
        "motion": "quick arcade blink",
        "safeZone": "keep gameplay primitives inside the center safe area",
        "avoid": "generic text-only callouts",
        "nodes": [
            {
                "type": "rect",
                "id": "player-one-ship",
                "box": { "x": 210.0, "y": 560.0, "width": 64.0, "height": 24.0 },
                "fill": "#39D98AFF"
            },
            {
                "type": "line",
                "id": "player-one-shot",
                "points": [
                    { "x": 242.0, "y": 548.0 },
                    { "x": 242.0, "y": 500.0 }
                ],
                "stroke": "#FFFFFFFF",
                "strokeWidth": 4.0
            }
        ]
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("custom primitive overlay should convert");

    assert_eq!(layers.len(), 2);
    assert_eq!(layers[1].role, GraphicRole::Overlay);
    assert_eq!(layers[1].nodes.len(), 2);
    assert!(matches!(layers[1].nodes[0], GraphicNode::Rect(_)));
    assert!(matches!(layers[1].nodes[1], GraphicNode::Line(_)));
}

#[test]
fn proposal_visuals_accept_custom_overlay_polygon_nodes() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "diagram",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "draw a filled isometric panel",
        "visualTreatment": "filled angled primitive face with crisp edge stroke",
        "motion": "short panel reveal",
        "safeZone": "keep diagram inside the center safe area",
        "avoid": "wireframe-only 3D panels",
        "nodes": [
            {
                "type": "polygon",
                "id": "top-face",
                "points": [
                    { "x": 260.0, "y": 280.0 },
                    { "x": 340.0, "y": 230.0 },
                    { "x": 610.0, "y": 230.0 },
                    { "x": 530.0, "y": 280.0 }
                ],
                "fill": "#39D98ACC",
                "stroke": "#FFFFFFFF",
                "strokeWidth": 3.0
            }
        ]
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("custom polygon overlay should convert");

    assert_eq!(layers.len(), 2);
    assert_eq!(layers[1].role, GraphicRole::Diagram);
    assert!(matches!(layers[1].nodes[0], GraphicNode::Polygon(_)));
}

#[test]
fn proposal_visuals_preserve_custom_node_animation() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "animate a shot across the frame",
        "visualTreatment": "pixel-art primitive overlay",
        "motion": "shot glides with linear easing",
        "safeZone": "center action lane",
        "avoid": "static marker",
        "nodes": [
            {
                "type": "rect",
                "id": "player-shot",
                "box": { "x": 210.0, "y": 560.0, "width": 64.0, "height": 24.0 },
                "fill": "#39D98AFF",
                "animate": {
                    "ease": "linear",
                    "keyframes": [
                        { "at": 0.0, "x": -80.0, "opacity": 0.0 },
                        { "at": 1.0, "x": 80.0, "opacity": 1.0 }
                    ]
                }
            }
        ]
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("animated primitive overlay should convert");

    let GraphicNode::Rect(rect) = &layers[1].nodes[0] else {
        panic!("expected animated rect node");
    };
    let animation = rect
        .animate
        .as_ref()
        .expect("animation should be preserved");
    assert_eq!(animation.ease, Some(Easing::Linear));
    assert_eq!(animation.keyframes.len(), 2);
    assert_eq!(animation.keyframes[0].x, Some(-80.0));
    assert_eq!(animation.keyframes[1].opacity, Some(1.0));
}

#[test]
fn proposal_visuals_apply_motion_preset_to_custom_nodes_without_animation() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "animate a custom metric badge",
        "visualTreatment": "primitive badge with preset motion",
        "motion": "metric tile pop with directional accent sweep",
        "motionPresetId": "metric-count-pop-v1",
        "safeZone": "keep badge inside 10% margins",
        "avoid": "static marker",
        "nodes": [
            {
                "type": "rect",
                "id": "metric-backing",
                "box": { "x": 220.0, "y": 180.0, "width": 360.0, "height": 160.0 },
                "fill": "#101820CC"
            },
            {
                "type": "text",
                "id": "metric-headline",
                "text": "42%",
                "box": { "x": 260.0, "y": 210.0, "width": 280.0, "height": 84.0 },
                "fontSize": 64.0,
                "fontWeight": 900,
                "align": "left",
                "fill": "#FFFFFFFF",
                "maxLines": 1
            }
        ]
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("custom primitive overlay should convert");

    assert_eq!(layers.len(), 2);
    assert!(
        layers[1]
            .nodes
            .iter()
            .all(|node| node.animation().is_some()),
        "custom nodes without animate should inherit the overlay motionPresetId"
    );
    let GraphicNode::Text(text) = &layers[1].nodes[1] else {
        panic!("expected custom text node");
    };
    let animation = text
        .animate
        .as_ref()
        .expect("text should inherit preset animation");
    assert_eq!(animation.ease, Some(Easing::OutBack));
    assert_eq!(animation.keyframes[0].opacity, Some(0.0));
}

#[test]
fn proposal_visuals_apply_motion_v2_preset_to_custom_nodes_without_animation() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "draw a metric badge with v2 motion",
        "visualTreatment": "primitive badge with v2 depth motion",
        "motion": "soft depth card v2 preset",
        "motionPresetId": "soft-depth-card-v2",
        "safeZone": "keep badge inside 10% margins",
        "avoid": "static marker",
        "nodes": [{
            "type": "rect",
            "id": "metric-backing",
            "box": { "x": 220.0, "y": 180.0, "width": 360.0, "height": 160.0 },
            "fill": "#101820CC"
        }]
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("custom primitive overlay should convert");
    let animation = layers[1].nodes[0].animation().expect("v2 preset animation");

    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.shadow_opacity.is_some()));
    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.rotation_degrees.is_some()));
}

#[test]
fn proposal_visuals_reject_invalid_custom_overlay_nodes_actionably() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "draw arcade primitives",
        "visualTreatment": "pixel-art primitive overlay",
        "motion": "quick arcade blink",
        "safeZone": "keep primitives inside the center safe area",
        "avoid": "malformed nodes",
        "nodes": [
            {
                "type": "rect",
                "id": "bad-rect",
                "fill": "#39D98AFF"
            }
        ]
    })];

    let errors = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect_err("malformed primitive nodes should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "overlays[0].nodes");
    assert!(
        errors[0]
            .fix
            .contains("text, rect, roundedRect, polygon, line, or imageRef"),
        "fix should tell the agent which primitive node types are allowed: {}",
        errors[0].fix
    );
}

#[test]
fn proposal_visuals_require_precise_visual_metadata() {
    let mut proposal = sample_codex_proposal();
    proposal.captions[0]["motion"] = serde_json::Value::Null;

    let errors = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect_err("missing caption motion should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "captions[0].motion");
    assert!(
        errors[0].fix.contains("motion"),
        "fix should name the missing field: {}",
        errors[0].fix
    );
}

#[test]
fn proposal_visuals_reject_unknown_motion_preset_id() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays[0]["motionPresetId"] = serde_json::json!("bad-preset");

    let errors = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect_err("unknown motion preset should fail");

    assert_eq!(errors[0].path, "overlays[0].motionPresetId");
    assert!(errors[0].fix.contains("slide-fade-up-v1"));
}

#[test]
fn proposal_visuals_reject_overlay_without_visible_text() {
    let mut proposal = sample_codex_proposal();
    let overlay = proposal.overlays[0]
        .as_object_mut()
        .expect("sample overlay is an object");
    overlay.remove("brief");
    overlay.remove("headline");
    overlay.remove("title");
    overlay.remove("label");
    overlay.remove("text");
    overlay.insert(
        "fields".to_string(),
        serde_json::json!({ "subline": "No headline" }),
    );

    let errors = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect_err("overlay without proposal text should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "overlays[0].visibleText");
    assert!(
        errors[0].message.contains("visible text"),
        "message should describe missing visible text: {}",
        errors[0].message
    );
    for field in [
        "brief",
        "fields.headline",
        "headline",
        "title",
        "label",
        "text",
    ] {
        assert!(
            errors[0].fix.contains(field),
            "fix should name {field}: {}",
            errors[0].fix
        );
    }
}

#[test]
fn render_proposal_template_hyperframe_converts_to_transparent_overlay_layer() {
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![sample_template_hyperframe_layer()];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("template HyperFrame should render as a transparent overlay layer");

    assert_eq!(layers.len(), 3);
    let hyperframe = layers
        .iter()
        .find(|layer| layer.id == "proposal-hyperframe-1")
        .expect("HyperFrame layer");
    assert!(hyperframe.alpha);
    assert_eq!(hyperframe.timeline_start, 0.0);
    assert_eq!(hyperframe.duration_seconds, 2.0);
    assert_eq!(hyperframe.visual_treatment, "kinetic editorial title scene");
    assert_eq!(hyperframe.motion, "fast type-on with camera push");
    validate_graphics_layer(hyperframe).expect("HyperFrame template layer should validate");
}

#[test]
fn render_proposal_title_card_hyperframe_converts_to_full_frame_layer() {
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![sample_hyperframe_layer()];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("title-card HyperFrame should render as a full-frame scene layer");

    let hyperframe = layers
        .iter()
        .find(|layer| layer.id == "proposal-hyperframe-1")
        .expect("HyperFrame layer");
    assert_eq!(hyperframe.role, GraphicRole::TitleCard);
    assert_eq!(hyperframe.timeline_start, 0.0);
    assert_eq!(hyperframe.duration_seconds, 2.0);
    assert_eq!(hyperframe.visual_treatment, "kinetic editorial title scene");
    validate_graphics_layer(hyperframe).expect("title-card HyperFrame layer should validate");
}

#[test]
fn render_proposal_lower_third_hyperframe_converts_to_lower_third_layer() {
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "lower_third",
        "role": "lower_third",
        "startSeconds": 0.75,
        "durationSeconds": 2.0,
        "brief": "Identify the speaker before the quote.",
        "fields": {
            "headline": "Olha API",
            "subline": "Creator and editor"
        },
        "visualTreatment": "compact translucent lower third with cyan accent and strong hierarchy",
        "motion": "slide in, hold, soft fade",
        "safeZone": "keep essential text inside lower-third safe margins",
        "avoid": "full-width opaque black slabs and static name cards"
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("lower-third HyperFrame should render as a lower-third graphics layer");

    let hyperframe = layers
        .iter()
        .find(|layer| layer.id == "proposal-hyperframe-1")
        .expect("HyperFrame layer");
    assert_eq!(hyperframe.role, GraphicRole::LowerThird);
    assert_eq!(hyperframe.timeline_start, 0.75);
    assert_eq!(hyperframe.duration_seconds, 2.0);
    assert_eq!(
        hyperframe.source_beat,
        "Identify the speaker before the quote."
    );
    validate_graphics_layer(hyperframe).expect("lower-third HyperFrame layer should validate");
}

#[test]
fn render_proposal_diagram_hyperframe_converts_to_diagram_layer() {
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "diagram",
        "role": "diagram",
        "startSeconds": 0.5,
        "durationSeconds": 2.25,
        "brief": "Explain the three-step workflow.",
        "fields": {
            "headline": "Plan -> Cut -> Review",
            "subline": "Every graphic follows a selected source range"
        },
        "visualTreatment": "compact editorial process diagram with translucent panels and a bright connector accent",
        "motion": "staggered node reveal with a short connector wipe",
        "safeZone": "keep diagram inside center 80 percent with captions unobscured",
        "avoid": "static slide layout and dense paragraph text"
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("diagram HyperFrame should render as a diagram graphics layer");

    let hyperframe = layers
        .iter()
        .find(|layer| layer.id == "proposal-hyperframe-1")
        .expect("HyperFrame layer");
    assert_eq!(hyperframe.role, GraphicRole::Diagram);
    assert_eq!(hyperframe.timeline_start, 0.5);
    assert_eq!(hyperframe.duration_seconds, 2.25);
    assert_eq!(hyperframe.source_beat, "Explain the three-step workflow.");
    validate_graphics_layer(hyperframe).expect("diagram HyperFrame layer should validate");
}

#[test]
fn render_proposal_transition_hyperframe_converts_to_transition_layer() {
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "transition",
        "role": "transition",
        "startSeconds": 2.0,
        "durationSeconds": 1.0,
        "brief": "Bridge the setup into the payoff.",
        "fields": {
            "headline": "Next: The Payoff"
        },
        "visualTreatment": "short full-frame kinetic color wipe with large readable cue text",
        "motion": "fast panel wipe with text hold and clean exit",
        "safeZone": "keep cue text inside the center safe area",
        "avoid": "long static interstitials and plain title slides"
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("transition HyperFrame should render as a transition graphics layer");

    let hyperframe = layers
        .iter()
        .find(|layer| layer.id == "proposal-hyperframe-1")
        .expect("HyperFrame layer");
    assert_eq!(hyperframe.role, GraphicRole::Transition);
    assert_eq!(hyperframe.timeline_start, 2.0);
    assert_eq!(hyperframe.duration_seconds, 1.0);
    assert_eq!(hyperframe.source_beat, "Bridge the setup into the payoff.");
    validate_graphics_layer(hyperframe).expect("transition HyperFrame layer should validate");
}

#[test]
fn render_proposal_immersive_hyperframe_converts_to_full_frame_scene_layer() {
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open inside an abstract generated editing room.",
        "fields": {
            "headline": "INSIDE THE CUT"
        },
        "visualTreatment": "full-frame immersive editorial scene with dimensional color panels and sparse cue text",
        "motion": "slow parallax drift with a fast entrance and clean exit",
        "safeZone": "keep cue text inside center safe area and edges available for motion",
        "avoid": "static slide design, opaque black slabs, and tiny background detail"
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("immersive HyperFrame should render as a full-frame graphics layer");

    let hyperframe = layers
        .iter()
        .find(|layer| layer.id == "proposal-hyperframe-1")
        .expect("HyperFrame layer");
    assert_eq!(hyperframe.role, GraphicRole::TitleCard);
    assert_eq!(hyperframe.timeline_start, 0.0);
    assert_eq!(hyperframe.duration_seconds, 2.0);
    assert_eq!(
        hyperframe.source_beat,
        "Open inside an abstract generated editing room."
    );
    validate_graphics_layer(hyperframe).expect("immersive HyperFrame layer should validate");
}

#[test]
fn render_proposal_immersive_logo_scene_uses_holographic_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with a holographic brand logo reveal.",
        "sourceBeat": "Brand reveal before the product proof.",
        "visualTreatment": "full-frame holographic logo cutout with pearlescent shader bands and controlled grain",
        "motion": "logo shimmer drifts through the mask, holds, then fades cleanly",
        "safeZone": "keep logo inside central safe area",
        "avoid": "plain centered text, static logo cards, and opaque black slabs"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("logo immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("logo immersive HyperFrame artifact should be present");
    assert_eq!(
        hyperframe.template_id.as_deref(),
        Some("holographic-logo-cutout-v1")
    );
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("pulse-emphasis-v2")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_metric_scene_uses_metric_callout_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with the retention metric before the proof.",
        "sourceBeat": "Show the key data point that explains why this cut matters.",
        "fields": {
            "headline": "42%",
            "subline": "higher retention"
        },
        "visualTreatment": "floating metric tile with a high-contrast number and directional accent",
        "motion": "metric count-up feel, accent sweep, hold, then slide out",
        "safeZone": "keep the metric tile inside the upper-right safe area",
        "avoid": "spreadsheet boxes, dense labels, and static title cards"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("metric immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("metric immersive HyperFrame artifact should be present");
    assert_eq!(hyperframe.template_id.as_deref(), Some("metric-callout-v1"));
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("metric-count-pop-v1")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_detail_scene_uses_tracking_highlight_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Call out the small product detail before the reveal.",
        "sourceBeat": "Direct attention to the visual detail that proves the claim.",
        "fields": {
            "headline": "Look here",
            "subline": "key detail"
        },
        "visualTreatment": "thin tracking ring with a compact label and pointer line",
        "motion": "tracking highlight draws on, label slides from pointer, then fades",
        "safeZone": "keep the label inside the safe area while the pointer tracks the detail",
        "avoid": "large opaque callout boxes, covering hands, and static arrows"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("detail immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("detail immersive HyperFrame artifact should be present");
    assert_eq!(
        hyperframe.template_id.as_deref(),
        Some("tracking-highlight-v1")
    );
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("tracking-draw-v1")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_quote_scene_uses_punchy_caption_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Punch in on the quote that sells the transformation.",
        "sourceBeat": "Emphasize the key quote before the payoff cut.",
        "fields": {
            "headline": "THIS CHANGES EVERYTHING"
        },
        "visualTreatment": "large phone-readable quote caption lockup with accent underline and soft backing",
        "motion": "snap pop in, underline wipe, hold, then quick fade",
        "safeZone": "keep the quote inside 10% margins and above bottom controls",
        "avoid": "subtitle slabs, tiny type, and centered static paragraphs"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("quote immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("quote immersive HyperFrame artifact should be present");
    assert_eq!(hyperframe.template_id.as_deref(), Some("punchy-caption-v1"));
    assert_eq!(hyperframe.motion_preset_id.as_deref(), Some("snap-pop-v1"));
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_chapter_scene_uses_chapter_card_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Reset the story with a chapter card before the proof segment.",
        "sourceBeat": "Introduce the next chapter of the story before the payoff.",
        "fields": {
            "headline": "CHAPTER 02",
            "subline": "The proof"
        },
        "visualTreatment": "left-weighted chapter marker with translucent panel and vertical reveal line",
        "motion": "vertical line wipe, text type-on, short hold, then mask out",
        "safeZone": "keep chapter text inside 10% margins and leave center action visible",
        "avoid": "full-frame static slides, plain centered text, and long title holds"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("chapter immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("chapter immersive HyperFrame artifact should be present");
    assert_eq!(hyperframe.template_id.as_deref(), Some("chapter-card-v1"));
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("vertical-reveal-v1")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_comparison_scene_uses_metric_callout_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Show a before vs after comparison for the workflow.",
        "sourceBeat": "Compare the old workflow against the new outcome before the proof cut.",
        "fields": {
            "headline": "2x",
            "subline": "faster review"
        },
        "visualTreatment": "split comparison tile with two concise labels and a directional accent",
        "motion": "left label enters, right label counters, accent sweep bridges the two states",
        "safeZone": "keep comparison labels inside 10% margins and away from captions",
        "avoid": "generic gradient loop, full-screen static table, and dense paragraphs"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("comparison immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("comparison immersive HyperFrame artifact should be present");
    assert_eq!(hyperframe.template_id.as_deref(), Some("metric-callout-v1"));
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("metric-count-pop-v1")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_process_scene_uses_chapter_card_template() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![serde_json::json!({
        "kind": "immersive_scene",
        "role": "scene",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Show a compact process roadmap before the proof cut.",
        "sourceBeat": "Explain the rollout process before the final proof cut.",
        "fields": {
            "headline": "Plan -> Build -> Review",
            "subline": "three-stage process"
        },
        "visualTreatment": "staged roadmap with three milestone markers and a vertical reveal line",
        "motion": "step markers reveal in sequence, connector line wipes through the process",
        "safeZone": "keep process labels inside 10% margins and away from captions",
        "avoid": "generic gradient loop, dense flowchart boxes, and static slide design"
    })];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("process immersive scene should render as scene-specific graphics");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("process immersive HyperFrame artifact should be present");
    assert_eq!(hyperframe.template_id.as_deref(), Some("chapter-card-v1"));
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("vertical-reveal-v1")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
}

#[test]
fn render_proposal_immersive_testimonial_feature_and_launch_scenes_use_scene_specific_templates() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 640;
    project.render_settings.height = 360;
    project.render_settings.fps = 4.0;
    let mut report = sample_codex_report();
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    report.proposal.hyperframes = vec![
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 0.0,
            "durationSeconds": 2.0,
            "brief": "Lift a customer testimonial interview beat.",
            "sourceBeat": "Lift the customer testimonial into a phone-readable interview beat.",
            "fields": {
                "headline": "IT FINALLY CLICKED"
            },
            "visualTreatment": "testimonial lockup with speaker treatment and compact backing",
            "motion": "testimonial text snaps in, speaker line wipes, then fades",
            "safeZone": "keep testimonial text inside 10% margins",
            "avoid": "plain subtitle slabs and dense paragraphs"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 2.1,
            "durationSeconds": 2.0,
            "brief": "Call out the product feature spec closeup.",
            "sourceBeat": "Show the product feature and spec before the proof cut.",
            "fields": {
                "headline": "Auto sync",
                "subline": "frame-accurate"
            },
            "visualTreatment": "precise feature lens with leader line and compact spec label",
            "motion": "feature lens draws in, spec label slides from the edge, then clears",
            "safeZone": "keep spec label away from captions and faces",
            "avoid": "large opaque boxes and generic gradients"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 4.2,
            "durationSeconds": 2.0,
            "brief": "Introduce the launch announcement beat.",
            "sourceBeat": "Introduce the launch announcement before the reveal.",
            "fields": {
                "headline": "LAUNCH READY",
                "subline": "new workflow"
            },
            "visualTreatment": "editorial announcement marker with vertical reveal and subline",
            "motion": "announcement line wipes up, headline reveals, short hold, then masks out",
            "safeZone": "keep announcement copy inside 10% margins",
            "avoid": "static title slide and centered plain text"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 6.3,
            "durationSeconds": 2.0,
            "brief": "Draw the route map before the arrival reveal.",
            "sourceBeat": "Show the location route before the arrival reveal.",
            "fields": {
                "headline": "Downtown route",
                "subline": "12 min"
            },
            "visualTreatment": "animated city map with a place marker and route line",
            "motion": "route line draws across the map, place marker pulses, text slides in",
            "safeZone": "keep map labels inside 10% margins",
            "avoid": "dense map screenshots and static location cards"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 8.4,
            "durationSeconds": 2.0,
            "brief": "Show the surprised reaction beat.",
            "sourceBeat": "Show the emotional reaction before the next cut.",
            "fields": {
                "headline": "WAIT, WHAT?"
            },
            "visualTreatment": "large reaction typography with expressive accent stroke",
            "motion": "reaction word pops on, accent stroke snaps, then clears quickly",
            "safeZone": "keep reaction text inside 10% margins",
            "avoid": "plain subtitle slabs and long holds"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 10.5,
            "durationSeconds": 2.0,
            "brief": "Show the buyer proof beat.",
            "sourceBeat": "Show the buyer proof before the close.",
            "fields": {
                "headline": "Pricing savings",
                "subline": "Budget impact and ROI"
            },
            "visualTreatment": "compact proof tile with directional accent",
            "motion": "figure counts up, delta sweeps, then settles",
            "safeZone": "keep pricing proof inside 10% margins",
            "avoid": "spreadsheet screenshots and static pricing cards"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 12.6,
            "durationSeconds": 2.0,
            "brief": "Flag the compliance risk warning.",
            "sourceBeat": "Flag the security risk before the remediation step.",
            "fields": {
                "headline": "RISK FLAG"
            },
            "visualTreatment": "urgent warning caption with compact compliance marker",
            "motion": "alert word snaps on, warning rule wipes, then clears",
            "safeZone": "keep warning text inside 10% margins",
            "avoid": "generic process cards and static warning slides"
        }),
        serde_json::json!({
            "kind": "immersive_scene",
            "role": "scene",
            "startSeconds": 14.7,
            "durationSeconds": 2.0,
            "brief": "Show the deadline schedule beat.",
            "sourceBeat": "Show the calendar deadline and due date.",
            "fields": {
                "headline": "Friday",
                "subline": "submission deadline"
            },
            "visualTreatment": "editorial schedule marker with date lockup and reveal line",
            "motion": "date marker wipes in, deadline label reveals, then masks out",
            "safeZone": "keep date and deadline copy inside 10% margins",
            "avoid": "generic gradient loop and static calendar screenshots"
        }),
    ];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("immersive scenes should render as scene-specific graphics");

    let template_ids = graphics
        .iter()
        .filter(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id.starts_with("proposal-hyperframe-"))
        })
        .map(|artifact| artifact.template_id.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        template_ids,
        vec![
            Some("punchy-caption-v1"),
            Some("tracking-highlight-v1"),
            Some("chapter-card-v1"),
            Some("tracking-highlight-v1"),
            Some("punchy-caption-v1"),
            Some("metric-callout-v1"),
            Some("punchy-caption-v1"),
            Some("chapter-card-v1"),
        ]
    );
}

#[test]
fn render_proposal_rejects_unknown_hyperframe_kind_by_name() {
    let mut proposal = sample_codex_proposal();
    let mut hyperframe = sample_hyperframe_layer();
    hyperframe
        .as_object_mut()
        .expect("hyperframe object")
        .insert("kind".to_string(), serde_json::json!("world_model_scene"));
    proposal.hyperframes = vec![hyperframe];

    let errors = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect_err("unknown HyperFrame kind should fail with actionable feedback");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "hyperframes[0].kind");
    assert_eq!(errors[0].message, "HyperFrame kind is not renderable yet.");
    assert!(errors[0].fix.contains("template_overlay"));
    assert_eq!(
        errors[0].details.get("kind").map(String::as_str),
        Some("world_model_scene")
    );
}

#[test]
fn render_proposal_timing_accepts_template_hyperframes() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut proposal = sample_codex_proposal();
    proposal.hyperframes = vec![sample_template_hyperframe_layer()];

    proposal_to_render_plan(
        &project,
        &request,
        &proposal,
        Path::new("media/input.mp4"),
        Path::new("renders/draft.mp4"),
        RenderQuality::Draft,
    )
    .expect("duration-aware proposal validation should accept renderable HyperFrames");
}

#[test]
fn render_proposal_args_parse_required_paths() {
    let config = parse_render_proposal_args([
        "video-creater-render-proposal",
        "--project-root",
        "/tmp/video-project",
        "--source-video",
        "/tmp/video-project/media/input.mp4",
        "--report",
        "/tmp/video-project/proposal.json",
        "--output-dir",
        "/tmp/video-project/renders/proposal",
        "--final-name",
        "accepted.mp4",
    ])
    .expect("complete render proposal args should parse");

    assert_eq!(config.project_root, Path::new("/tmp/video-project"));
    assert_eq!(
        config.source_video_path,
        Path::new("/tmp/video-project/media/input.mp4")
    );
    assert_eq!(
        config.codex_report_path,
        Path::new("/tmp/video-project/proposal.json")
    );
    assert_eq!(
        config.output_dir,
        Path::new("/tmp/video-project/renders/proposal")
    );
    assert_eq!(config.final_name, "accepted.mp4");
    assert_eq!(config.quality_profile, RenderQualityProfile::DraftWebm);
}

#[test]
fn render_proposal_args_parse_final_quality_profile() {
    let config = parse_render_proposal_args([
        "video-creater-render-proposal",
        "--project-root",
        "/tmp/video-project",
        "--source-video",
        "/tmp/video-project/media/input.mp4",
        "--report",
        "/tmp/video-project/proposal.json",
        "--output-dir",
        "/tmp/video-project/renders/proposal",
        "--final-name",
        "accepted.mp4",
        "--quality",
        "finalWebm",
    ])
    .expect("render proposal args with quality should parse");

    assert_eq!(config.quality_profile, RenderQualityProfile::FinalWebm);
}

#[test]
fn render_proposal_args_reject_missing_report_path() {
    let errors = parse_render_proposal_args(["video-creater-render-proposal"])
        .expect_err("report is required");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "args.report");
}

#[test]
fn render_proposal_args_reject_unknown_flag() {
    let errors = parse_render_proposal_args([
        "video-creater-render-proposal",
        "--project-root",
        "/tmp/video-project",
        "--source-video",
        "/tmp/video-project/media/input.mp4",
        "--report",
        "/tmp/video-project/proposal.json",
        "--output-dir",
        "/tmp/video-project/renders/proposal",
        "--surprise",
        "value",
    ])
    .expect_err("unknown flags should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "args.surprise");
}

#[test]
fn render_proposal_args_reject_missing_value() {
    let errors = parse_render_proposal_args([
        "video-creater-render-proposal",
        "--project-root",
        "/tmp/video-project",
        "--source-video",
        "/tmp/video-project/media/input.mp4",
        "--report",
    ])
    .expect_err("missing flag values should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "args.report");
}

#[test]
fn package_scripts_use_rust_bins_for_pipeline_e2e() {
    let package_json_path = repo_root().join("package.json");
    let package_json =
        std::fs::read_to_string(&package_json_path).expect("package.json should be readable");
    let package: serde_json::Value =
        serde_json::from_str(&package_json).expect("package.json should parse");
    let scripts = package["scripts"]
        .as_object()
        .expect("package.json scripts should be an object");

    assert_eq!(
        scripts
            .get("e2e:combined")
            .and_then(serde_json::Value::as_str),
        Some("cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-e2e-combined --")
    );
    assert_eq!(
        scripts.get("e2e:codex").and_then(serde_json::Value::as_str),
        Some(
            "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-codex-e2e -- --codex fixture"
        )
    );
    assert_eq!(
        scripts
            .get("render:proposal")
            .and_then(serde_json::Value::as_str),
        Some(
            "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-render-proposal --"
        )
    );
    assert_eq!(
        scripts
            .get("render:template")
            .and_then(serde_json::Value::as_str),
        Some(
            "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-render-template --"
        )
    );
    assert_eq!(
        scripts
            .get("check:temporal-worker")
            .and_then(serde_json::Value::as_str),
        Some(
            "cargo check --manifest-path src-tauri/Cargo.toml --bin video-creater-temporal-worker"
        )
    );
}

#[test]
fn non_frontend_node_pipeline_harnesses_are_removed() {
    let scripts_dir = repo_root().join("scripts");

    for harness in [
        ["render", "codex", "funny", "draft.mjs"].join("-"),
        ["e2e", "combined", "video.mjs"].join("-"),
        ["e2e", "codex", "app", "server", "funny.mjs"].join("-"),
    ] {
        assert!(
            !scripts_dir.join(&harness).exists(),
            "{harness} should be removed from live scripts/"
        );
    }
}

#[test]
fn render_proposal_config_builds_expected_output_paths() {
    let config = RenderProposalConfig {
        project_root: "/tmp/video-project".into(),
        source_video_path: "/tmp/video-project/media/input.mp4".into(),
        codex_report_path: "/tmp/video-project/proposal.json".into(),
        output_dir: "/tmp/video-project/renders/proposal".into(),
        final_name: "final-cut.mp4".to_string(),
        quality_profile: RenderQualityProfile::DraftWebm,
    };

    assert_eq!(
        config.final_path(),
        Path::new("/tmp/video-project/renders/proposal/final-cut.mp4")
    );
    assert_eq!(
        config.render_report_path(),
        Path::new("/tmp/video-project/renders/proposal/render-report.json")
    );
}

#[test]
#[cfg(feature = "ges-render")]
fn render_proposal_runner_writes_final_webm_and_report_before_success() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let tempdir = tempfile::tempdir().expect("temp render proposal dir");
    let source_path = tempdir.path().join("source.webm");
    generate_tiny_source_video(&source_path);

    let report_path = tempdir.path().join("codex-report.json");
    let codex_report = tiny_source_codex_report_with_graphics();
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&codex_report).expect("codex report json"),
    )
    .expect("write codex report");

    let output_dir = tempdir.path().join("proposal-render");
    let config = RenderProposalConfig {
        project_root: tempdir.path().to_path_buf(),
        source_video_path: source_path,
        codex_report_path: report_path,
        output_dir,
        final_name: "accepted.webm".to_string(),
        quality_profile: RenderQualityProfile::DraftWebm,
    };

    let result =
        run_render_proposal(&config).expect("valid proposal render should produce final artifacts");

    assert_eq!(result.final_path, config.final_path());
    assert_eq!(result.render_report_path, config.render_report_path());
    assert!(result.final_path.exists(), "final WebM should exist");
    assert!(
        result.render_report_path.exists(),
        "render report JSON should exist before success is returned"
    );

    let report_json =
        std::fs::read_to_string(&result.render_report_path).expect("render report should read");
    let report: RenderReport =
        serde_json::from_str(&report_json).expect("render report should deserialize");
    assert_eq!(report.job_id, "render-proposal");
    assert_eq!(report.summary.status, "succeeded");
    assert_eq!(
        report.summary.output_path,
        Some(result.final_path.display().to_string())
    );
    assert_eq!(report.command.program, "gstreamer-ges");
    assert!(report
        .artifacts
        .contains(&result.final_path.display().to_string()));
    assert!(report
        .artifacts
        .contains(&result.render_report_path.display().to_string()));
    assert!(report.artifacts.contains(
        &config
            .output_dir
            .join("graphics/proposal-caption-1/manifest.json")
            .display()
            .to_string()
    ));
    assert!(report.artifacts.contains(
        &config
            .output_dir
            .join("graphics/proposal-caption-1/preview.png")
            .display()
            .to_string()
    ));
    assert!(report.artifacts.contains(
        &config
            .output_dir
            .join("graphics/proposal-caption-1/frames/frame-000000.png")
            .display()
            .to_string()
    ));
}

#[cfg(feature = "ges-render")]
#[test]
fn render_proposal_runner_composites_gpu_visual_when_gpu_available() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let tempdir = tempfile::tempdir().expect("temp render proposal dir");
    let source_path = tempdir.path().join("source.webm");
    generate_tiny_source_video(&source_path);

    let report_path = tempdir.path().join("codex-report.json");
    let mut codex_report = tiny_source_codex_report();
    codex_report.proposal.captions = Vec::new();
    codex_report.proposal.overlays = Vec::new();
    codex_report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&codex_report).expect("codex report json"),
    )
    .expect("write codex report");

    let output_dir = tempdir.path().join("proposal-render");
    let config = RenderProposalConfig {
        project_root: tempdir.path().to_path_buf(),
        source_video_path: source_path,
        codex_report_path: report_path,
        output_dir,
        final_name: "accepted.webm".to_string(),
        quality_profile: RenderQualityProfile::DraftWebm,
    };

    let result = run_render_proposal(&config);

    match result {
        Ok(result) => {
            assert!(result.final_path.exists(), "final WebM should exist");
            assert!(config
                .output_dir
                .join("graphics/shader-hook-bg/frames/frame-000000.png")
                .exists());
            assert!(config
                .output_dir
                .join("graphics/shader-hook-bg/frames/frame-000003.png")
                .exists());
            let report_json = std::fs::read_to_string(&result.render_report_path)
                .expect("render report should read");
            let report: RenderReport =
                serde_json::from_str(&report_json).expect("render report should deserialize");
            assert!(report
                .artifacts
                .iter()
                .any(|artifact| artifact.ends_with("graphics/shader-hook-bg/manifest.json")));
        }
        Err(errors)
            if errors.iter().any(|error| {
                error
                    .details
                    .get("gpuGraphicsCode")
                    .is_some_and(|code| code == "GPU_GRAPHICS_DEVICE_UNAVAILABLE")
            }) =>
        {
            eprintln!("Skipping GPU visual e2e because no compatible adapter is available");
        }
        Err(errors) => panic!("unexpected GPU visual e2e errors: {errors:?}"),
    }
}

#[test]
fn render_proposal_runner_rejects_missing_source_without_writing_report() {
    let tempdir = tempfile::tempdir().expect("temp render proposal dir");
    let report_path = tempdir.path().join("codex-report.json");
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&tiny_source_codex_report()).expect("codex report json"),
    )
    .expect("write codex report");

    let config = RenderProposalConfig {
        project_root: tempdir.path().to_path_buf(),
        source_video_path: tempdir.path().join("missing-source.mp4"),
        codex_report_path: report_path,
        output_dir: tempdir.path().join("proposal-render"),
        final_name: "accepted.mp4".to_string(),
        quality_profile: RenderQualityProfile::DraftWebm,
    };

    let errors = run_render_proposal(&config).expect_err("missing source should fail rendering");

    assert!(
        errors
            .iter()
            .any(|error| matches!(error.code, PipelineErrorCode::PipelineInputInvalid)),
        "missing source should fail input preflight before media discovery, got {errors:?}"
    );
    assert!(
        !config.render_report_path().exists(),
        "failed render proposal run must not write a success report"
    );
}

#[test]
fn render_proposal_cli_does_not_succeed_after_parse_only() {
    let tempdir = tempfile::tempdir().expect("temp render proposal dir");
    let output_dir = tempdir.path().join("renders");
    let output = StdCommand::new(render_proposal_bin_path())
        .args([
            "--project-root",
            tempdir.path().to_str().expect("temp path should be utf8"),
            "--source-video",
            tempdir
                .path()
                .join("missing-source.mp4")
                .to_str()
                .expect("source path should be utf8"),
            "--report",
            tempdir
                .path()
                .join("missing-report.json")
                .to_str()
                .expect("report path should be utf8"),
            "--output-dir",
            output_dir.to_str().expect("output path should be utf8"),
        ])
        .output()
        .expect("render proposal binary should run");

    assert!(
        !output.status.success(),
        "binary must fail instead of returning ok:true after argument parsing; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("render.runtime.startup_required"),
        "render proposal CLI must initialize the shared runtime before render preflight: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output_dir.join("render-report.json").exists(),
        "failed render proposal run must not write a success report"
    );
}

#[test]
fn codex_e2e_args_parse_report_and_project_root() {
    let config = parse_codex_e2e_args([
        "video-creater-codex-e2e",
        "--project-root",
        "/tmp/video-project",
        "--report",
        "/tmp/video-project/output/codex-report.json",
        "--codex",
        "custom-codex",
    ])
    .expect("complete codex e2e args should parse");

    assert_eq!(config.project_root, Path::new("/tmp/video-project"));
    assert_eq!(
        config.report_path,
        Path::new("/tmp/video-project/output/codex-report.json")
    );
    assert_eq!(config.codex_binary, "custom-codex");
}

#[test]
fn codex_e2e_report_contains_render_proposal_compatible_proposal() {
    let report = CodexE2eReport {
        generated_at: "2026-06-17T00:00:00Z".to_string(),
        thread_id: "thread-codex-e2e".to_string(),
        proposal: sample_codex_proposal(),
        tool_acceptance: codex_tool_acceptance(),
    };

    let json = serde_json::to_string(&report).expect("codex e2e report should serialize");
    let proposal_report =
        codex_report_from_json(&json).expect("codex e2e report should load as proposal report");

    assert_eq!(proposal_report.generated_at, report.generated_at);
    assert_eq!(proposal_report.proposal, report.proposal);
}

#[test]
fn codex_e2e_config_default_report_path_is_pipeline_output() {
    let config = parse_codex_e2e_args([
        "video-creater-codex-e2e",
        "--project-root",
        "/tmp/video-project",
    ])
    .expect("codex e2e args should parse with default report");

    assert_eq!(config.project_root, Path::new("/tmp/video-project"));
    assert_eq!(
        config.report_path,
        Path::new("output/e2e-codex/codex-report.json")
    );
    assert_eq!(
        config.codex_binary,
        std::env::var("VIDEO_CREATER_CODEX").unwrap_or_else(|_| "codex".to_string())
    );
}

#[test]
fn codex_e2e_args_default_project_root_to_current_dir() {
    let config = parse_codex_e2e_args(["video-creater-codex-e2e"])
        .expect("default project root should parse");

    assert_eq!(
        config.project_root,
        std::env::current_dir().expect("current dir")
    );
    assert_eq!(
        config.report_path,
        Path::new("output/e2e-codex/codex-report.json")
    );
}

#[test]
fn codex_e2e_runner_writes_report_after_valid_proposal() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let report_path = tempdir.path().join("nested/codex-report.json");
    let config = codex_e2e_config_for_report(&report_path);
    let proposal = valid_codex_e2e_proposal();
    let assistant_text = serde_json::to_string(&proposal).expect("proposal json");
    let mut client = FakeCodexE2eClient::with_assistant_text(assistant_text);

    let result = run_codex_e2e_with_client(&config, &mut client)
        .expect("valid Codex e2e proposal should produce a report");

    assert_eq!(result.report_path, report_path);
    assert_eq!(result.thread_id, "thread-e2e");
    assert_eq!(client.requests.borrow().len(), 3);
    assert_eq!(
        client.requests.borrow()[0]["method"],
        serde_json::json!("initialize")
    );
    assert_eq!(
        client.requests.borrow()[1]["method"],
        serde_json::json!("thread/start")
    );
    assert_eq!(
        client.requests.borrow()[2]["method"],
        serde_json::json!("turn/start")
    );
    let report_json = std::fs::read_to_string(&report_path).expect("report should be written");
    let report_value: serde_json::Value =
        serde_json::from_str(&report_json).expect("report should parse as JSON");
    assert_eq!(
        report_value["toolAcceptance"]["buildRenderStartRequest"],
        serde_json::json!(true),
        "Codex E2E acceptance should prove render workflow start request coverage"
    );
    let report: CodexE2eReport =
        serde_json::from_str(&report_json).expect("report should deserialize");
    assert_eq!(report.thread_id, "thread-e2e");
    assert_eq!(report.proposal, proposal);
    assert_eq!(report.tool_acceptance, codex_tool_acceptance());
}

#[test]
fn codex_e2e_runner_rejects_missing_proposal_without_writing_report() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let report_path = tempdir.path().join("codex-report.json");
    let config = codex_e2e_config_for_report(&report_path);
    let mut client = FakeCodexE2eClient::with_assistant_text("not json".to_string());

    let errors =
        run_codex_e2e_with_client(&config, &mut client).expect_err("missing proposal should fail");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::CodexProposalInvalid);
    assert!(
        !report_path.exists(),
        "invalid proposal must not write report"
    );
}

#[test]
fn codex_e2e_cli_with_false_binary_does_not_succeed_after_parse_only() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let report_path = tempdir.path().join("codex-report.json");
    let repo_root = repo_root();
    let mut client = FakeCodexE2eClient::with_assistant_text(String::new());

    let errors = run_codex_e2e_cli_with_client(
        [
            "video-creater-codex-e2e".to_string(),
            "--project-root".to_string(),
            repo_root.display().to_string(),
            "--report".to_string(),
            report_path.display().to_string(),
            "--codex".to_string(),
            "false".to_string(),
        ],
        &mut client,
    )
    .expect_err("runner invocation should fail without a proposal");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::CodexProposalInvalid);
    assert!(
        client.was_invoked.get(),
        "CLI helper should call the e2e runner, not stop at argument parsing"
    );
    assert!(!report_path.exists(), "failed run must not write report");
}

#[test]
fn combined_e2e_args_default_to_output_directory() {
    let config = parse_combined_e2e_args(["video-creater-e2e-combined"])
        .expect("combined e2e args should parse with defaults");

    assert_eq!(
        config.output_dir,
        std::env::current_dir()
            .expect("current dir")
            .join("output/e2e-combined")
    );
}

#[test]
fn combined_e2e_project_has_real_edl_source_ranges() {
    let project = build_combined_project(36.0);

    assert_eq!(project.id, "combined-e2e-validation");
    assert_eq!(project.name, "Combined E2E Validation");
    assert_eq!(project.media.len(), 1);
    assert_eq!(project.media[0].id, "media-raw");
    assert_eq!(project.media[0].duration_seconds, 36.0);
    assert!(project.timeline.duration_seconds > 0.0);

    let video_track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .expect("combined e2e project should include a video track");
    let video_clips = video_track
        .items
        .iter()
        .filter(|item| item.kind == TimelineItemKind::VideoClip)
        .collect::<Vec<_>>();

    assert!(
        video_clips.len() >= 2,
        "combined e2e project should prove selected ranges, not full-source pass-through"
    );
    for item in video_clips {
        let source_in = item.properties["sourceIn"]
            .as_f64()
            .expect("video clip should carry sourceIn");
        let source_out = item.properties["sourceOut"]
            .as_f64()
            .expect("video clip should carry sourceOut");

        assert!(
            source_out > source_in,
            "video clip {} should have a positive source range",
            item.id
        );
        assert!(source_out <= 36.0);
    }

    let caption_track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Caption)
        .expect("combined e2e project should include a caption track");
    assert!(
        caption_track
            .items
            .iter()
            .any(|item| item.kind == TimelineItemKind::Caption),
        "combined e2e project should include at least one caption layer"
    );
    let overlay_track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Overlay)
        .expect("combined e2e project should include an overlay track");
    let overlay = overlay_track
        .items
        .iter()
        .find(|item| item.kind == TimelineItemKind::Overlay)
        .expect("combined e2e project should include one visual overlay");
    for key in ["visualTreatment", "motion", "safeZone", "avoid"] {
        assert!(
            overlay
                .properties
                .get(key)
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty()),
            "overlay should include {key}"
        );
    }
}

#[test]
fn combined_e2e_config_paths_match_existing_report_contract() {
    let config = CombinedE2eConfig {
        output_dir: "/tmp/video-project/renders/combined".into(),
    };

    assert_eq!(
        config.raw_path(),
        Path::new("/tmp/video-project/renders/combined/raw-footage.webm")
    );
    assert_eq!(
        config.final_path(),
        Path::new("/tmp/video-project/renders/combined/final-combined-validation.webm")
    );
    assert_eq!(
        config.json_report_path(),
        Path::new("/tmp/video-project/renders/combined/validation-report.json")
    );
    assert_eq!(
        config.markdown_report_path(),
        Path::new("/tmp/video-project/renders/combined/validation-report.md")
    );
    assert_eq!(
        config.log_path(),
        Path::new("/tmp/video-project/renders/combined/render.log")
    );
}

#[test]
fn combined_e2e_args_reject_missing_value() {
    let errors = parse_combined_e2e_args(["video-creater-e2e-combined", "--output-dir"])
        .expect_err("missing flag values should be rejected");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "args.outputDir");
}

#[test]
fn combined_e2e_project_normalizes_invalid_duration_consistently() {
    for raw_duration_seconds in [0.0, -4.0, f64::NAN, f64::INFINITY] {
        let project = build_combined_project(raw_duration_seconds);
        let media_duration = project.media[0].duration_seconds;

        assert!(
            media_duration.is_finite() && media_duration > 0.0,
            "media duration should be normalized for input {raw_duration_seconds:?}"
        );

        let video_track = project
            .timeline
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .expect("combined e2e project should include a video track");
        for item in &video_track.items {
            let source_in = item.properties["sourceIn"]
                .as_f64()
                .expect("video clip should carry sourceIn");
            let source_out = item.properties["sourceOut"]
                .as_f64()
                .expect("video clip should carry sourceOut");

            assert!(source_in.is_finite());
            assert!(source_out.is_finite());
            assert!(source_in >= 0.0);
            assert!(source_out > source_in);
            assert!(source_out <= media_duration);
        }
    }
}

#[test]
#[cfg(feature = "ges-render")]
fn combined_e2e_runner_reports_artifacts_after_successful_run() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let dir = tempfile::tempdir().expect("temp combined e2e dir");
    let config = CombinedE2eConfig {
        output_dir: dir.path().to_path_buf(),
    };

    let result = run_combined_e2e_with_runner(&config, &PanicRunner)
        .expect("successful combined e2e run should report artifacts");

    assert_eq!(result.final_path, config.final_path());
    assert_eq!(result.json_report_path, config.json_report_path());
    assert_eq!(result.markdown_report_path, config.markdown_report_path());
    assert_eq!(result.log_path, config.log_path());
    assert!(config.raw_path().exists());
    assert!(config.final_path().exists());
    assert!(config.json_report_path().exists());
    assert!(config.markdown_report_path().exists());
    assert!(config.log_path().exists());
    let required_artifacts = vec![
        config.raw_path(),
        config.final_path(),
        config.json_report_path(),
        config.markdown_report_path(),
        config.log_path(),
    ];
    for artifact in &required_artifacts {
        assert!(
            result.artifacts.contains(artifact),
            "combined e2e artifacts should include {}",
            artifact.display()
        );
    }
    assert!(
        result
            .artifacts
            .iter()
            .any(|artifact| artifact.ends_with("graphics/caption-e2e-hook/manifest.json")),
        "combined e2e artifacts should include caption graphics manifest"
    );
    assert!(
        result
            .artifacts
            .iter()
            .any(|artifact| artifact.ends_with("graphics/overlay-e2e-proof/manifest.json")),
        "combined e2e artifacts should include overlay graphics manifest"
    );
    let project_report = result
        .project
        .render_reports
        .first()
        .expect("combined e2e should attach render report to project");
    assert_eq!(project_report.status, RenderReportStatus::Completed);
    assert!(project_report.streams.video);
    assert!(project_report.streams.audio);
    assert_eq!(
        project_report.checks.get("overlayTiming"),
        Some(&RenderReportCheckStatus::Passed)
    );
    assert_eq!(
        project_report.checks.get("sourceRanges"),
        Some(&RenderReportCheckStatus::Passed)
    );
    assert_eq!(
        project_report.checks.get("logPath"),
        Some(&RenderReportCheckStatus::Passed)
    );
    assert_eq!(
        project_report.log_path,
        config.log_path().display().to_string()
    );

    let report_json =
        std::fs::read_to_string(config.json_report_path()).expect("json report should be readable");
    assert!(report_json.contains("combined-e2e"));
    assert!(report_json.contains("final-combined-validation.webm"));
    assert!(report_json.contains("gstreamer-ges"));
    let report_value: serde_json::Value =
        serde_json::from_str(&report_json).expect("combined e2e report should be JSON");
    let graphics = report_value["graphics"]
        .as_array()
        .expect("combined e2e report should include graphics evidence");
    assert!(
        graphics
            .iter()
            .any(|entry| entry["layerId"] == serde_json::json!("caption-e2e-hook")),
        "combined e2e report should prove caption graphics timing"
    );
    assert!(
        graphics
            .iter()
            .any(|entry| entry["layerId"] == serde_json::json!("overlay-e2e-proof")),
        "combined e2e report should prove overlay graphics timing"
    );
}

#[test]
fn render_proposal_dry_run_builds_graphics_and_gstreamer_command() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let report = sample_codex_report();
    let config = RenderProposalConfig {
        project_root: "/tmp/video-project".into(),
        source_video_path: "/tmp/video-project/media/input.mp4".into(),
        codex_report_path: "/tmp/video-project/proposal.json".into(),
        output_dir: "/tmp/video-project/renders/proposal".into(),
        final_name: "final.mp4".to_string(),
        quality_profile: RenderQualityProfile::DraftWebm,
    };

    let preview = build_render_proposal_preview(&project, &request, &report, &config)
        .expect("valid proposal should build a dry-run command");

    assert_eq!(preview.graphics_layer_count, 2);
    assert_eq!(preview.quality_profile, "draftWebm");
    assert_eq!(preview.effective_width, 1280);
    assert_eq!(preview.effective_height, 720);
    assert_eq!(preview.effective_fps, 24.0);
    assert_eq!(preview.command.program, "gstreamer-ges");
    assert!(preview
        .command
        .args
        .contains(&"--size=1280x720".to_string()));
    assert!(preview.command.args.contains(&"--fps=24.000".to_string()));
    assert!(preview.command.args.iter().any(|arg| {
        arg
            == "--overlay=/tmp/video-project/renders/proposal/graphics/proposal-caption-1/frames/frame-000000.png@2.000+2.200"
    }));
    assert!(preview
        .command
        .args
        .contains(&"--output=/tmp/video-project/renders/proposal/final.mp4".to_string()));
}

#[test]
fn render_proposal_dry_run_uses_final_quality_profile_from_config() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let report = sample_codex_report();
    let config = RenderProposalConfig {
        project_root: "/tmp/video-project".into(),
        source_video_path: "/tmp/video-project/media/input.mp4".into(),
        codex_report_path: "/tmp/video-project/proposal.json".into(),
        output_dir: "/tmp/video-project/renders/proposal".into(),
        final_name: "final.webm".to_string(),
        quality_profile: RenderQualityProfile::FinalWebm,
    };

    let preview = build_render_proposal_preview(&project, &request, &report, &config)
        .expect("valid proposal should build a final-quality dry-run command");

    assert_eq!(preview.quality_profile, "finalWebm");
    assert!(preview
        .command
        .args
        .contains(&"--quality=finalWebm".to_string()));
}

#[test]
fn render_proposal_dry_run_counts_gpu_visual_layers() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    let config = RenderProposalConfig {
        project_root: "/tmp/video-project".into(),
        source_video_path: "/tmp/video-project/media/input.mp4".into(),
        codex_report_path: "/tmp/video-project/proposal.json".into(),
        output_dir: "/tmp/video-project/renders/proposal".into(),
        final_name: "final.webm".to_string(),
        quality_profile: RenderQualityProfile::DraftWebm,
    };

    let preview = build_render_proposal_preview(&project, &request, &report, &config)
        .expect("valid proposal should build a dry-run command");

    assert_eq!(preview.graphics_layer_count, 3);
    assert!(preview.command.args.iter().any(|arg| {
        arg == "--overlay=/tmp/video-project/renders/proposal/graphics/shader-hook-bg/frames/frame-000000.png@0.000+1.000"
    }));
}

#[test]
fn render_proposal_graphics_writes_rust_artifacts() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let project = sample_project_for_proposal();
    let report = sample_codex_report();

    let overlays = render_proposal_graphics(&project, &report, dir.path())
        .expect("valid proposal graphics should render");

    assert_eq!(overlays.len(), 2);
    assert!(
        overlays[0].manifest.frame_count > 1,
        "animated default captions should render frame sequences"
    );
    assert_eq!(
        overlays[0].artifact_dir,
        dir.path().join("proposal-caption-1")
    );
    assert_eq!(overlays[0].manifest.frames_pattern, "frames/frame-%06d.png");
    assert!(dir.path().join("proposal-caption-1/preview.png").exists());
    assert!(dir.path().join("proposal-caption-1/manifest.json").exists());
    assert!(dir
        .path()
        .join("proposal-caption-1/frames/frame-000000.png")
        .exists());
    assert!(dir
        .path()
        .join(format!(
            "proposal-caption-1/frames/frame-{:06}.png",
            overlays[0].manifest.frame_count - 1
        ))
        .exists());
}

#[test]
fn render_proposal_graphics_writes_template_hyperframe_artifact() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let project = sample_project_for_proposal();
    let mut report = sample_codex_report();
    report.proposal.hyperframes = vec![sample_template_hyperframe_layer()];

    let graphics = render_proposal_graphics(&project, &report, dir.path())
        .expect("template HyperFrame graphics should render");

    let hyperframe = graphics
        .iter()
        .find(|artifact| {
            artifact
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-hyperframe-1")
        })
        .expect("rendered HyperFrame artifact");
    assert_eq!(
        hyperframe.artifact_dir,
        dir.path().join("proposal-hyperframe-1")
    );
    assert!(hyperframe.manifest.alpha);
    assert!(dir
        .path()
        .join("proposal-hyperframe-1/preview.png")
        .exists());
    assert!(dir
        .path()
        .join("proposal-hyperframe-1/manifest.json")
        .exists());
    assert_eq!(hyperframe.template_id.as_deref(), Some("chapter-card-v1"));
    assert_eq!(
        hyperframe.motion_preset_id.as_deref(),
        Some("vertical-reveal-v1")
    );
    assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
    assert!(
        !hyperframe.sampled_frames.is_empty(),
        "rendered HyperFrame artifacts should include sampled frame evidence"
    );
    for sampled_frame in &hyperframe.sampled_frames {
        assert!(
            dir.path()
                .join("proposal-hyperframe-1")
                .join(sampled_frame)
                .exists(),
            "sampled HyperFrame frame should exist as an artifact: {sampled_frame}"
        );
    }
}

#[test]
fn render_proposal_graphics_reports_frame_evidence_for_builtin_hyperframe_kinds() {
    let cases = [
        (
            "title_card",
            serde_json::json!({
                "kind": "title_card",
                "role": "title_card",
                "startSeconds": 0.0,
                "durationSeconds": 0.5,
                "brief": "Open with a full-frame HyperFrame title scene.",
                "fields": {
                    "headline": "Launch",
                    "subline": "Opening beat"
                },
                "visualTreatment": "kinetic editorial title scene",
                "motion": "fast type-on with camera push",
                "safeZone": "keep title inside 10% margins",
                "avoid": "static text-only cards"
            }),
            "chapter-card-v1",
            Some("vertical-reveal-v1"),
        ),
        (
            "diagram",
            serde_json::json!({
                "kind": "diagram",
                "role": "diagram",
                "startSeconds": 0.0,
                "durationSeconds": 0.5,
                "brief": "Show the proof structure.",
                "sourceBeat": "Explain the three-step workflow.",
                "fields": {
                    "headline": "Three steps",
                    "subline": "Cut render"
                },
                "visualTreatment": "clean diagram card with connected steps",
                "motion": "staggered node reveal with connector wipe",
                "safeZone": "keep all labels inside 10% margins",
                "avoid": "dense labels or static text-only cards"
            }),
            "metric-callout-v1",
            Some("metric-count-pop-v1"),
        ),
        (
            "lower_third",
            serde_json::json!({
                "kind": "lower_third",
                "role": "lower_third",
                "startSeconds": 0.0,
                "durationSeconds": 0.5,
                "brief": "Identify the speaker before the quote.",
                "sourceBeat": "Identify the speaker before the quote.",
                "fields": {
                    "headline": "Olha API",
                    "subline": "Creator and editor"
                },
                "visualTreatment": "compact translucent lower third with cyan accent and strong hierarchy",
                "motion": "slide in, hold, soft fade",
                "safeZone": "keep essential text inside lower-third safe margins",
                "avoid": "full-width opaque black slabs and static name cards"
            }),
            "kinetic-lower-third-v1",
            Some("slide-fade-up-v1"),
        ),
        (
            "transition",
            serde_json::json!({
                "kind": "transition",
                "role": "transition",
                "startSeconds": 0.0,
                "durationSeconds": 1.0,
                "brief": "Bridge the setup into the payoff.",
                "sourceBeat": "Bridge the setup into the payoff.",
                "fields": {
                    "headline": "Next",
                    "subline": "Payoff"
                },
                "visualTreatment": "short full-frame kinetic color wipe with large readable cue text",
                "motion": "fast panel wipe with a clean exit",
                "safeZone": "keep text inside title-safe margins",
                "avoid": "long static holds"
            }),
            "punchy-caption-v1",
            Some("snap-pop-v1"),
        ),
        (
            "immersive_scene",
            serde_json::json!({
                "kind": "immersive_scene",
                "role": "scene",
                "startSeconds": 0.0,
                "durationSeconds": 2.0,
                "brief": "Open with a dimensional generated scene.",
                "sourceBeat": "Introduce the dimensional environment.",
                "fields": {
                    "headline": "Scene",
                    "subline": "Scene"
                },
                "visualTreatment": "full-frame immersive editorial scene with dimensional color panels and sparse cue text",
                "motion": "slow parallax drift with clean entrance and exit",
                "safeZone": "keep cue text inside 10% margins",
                "avoid": "static text-only cards"
            }),
            "gradient-background-loop-v1",
            None,
        ),
    ];

    for (kind, hyperframe_layer, expected_template, expected_motion) in cases {
        let dir = tempfile::tempdir().expect("temp graphics dir");
        let mut project = sample_project_for_proposal();
        project.render_settings.width = 640;
        project.render_settings.height = 360;
        project.render_settings.fps = 4.0;
        let mut report = sample_codex_report();
        report.proposal.captions = Vec::new();
        report.proposal.overlays = Vec::new();
        report.proposal.hyperframes = vec![hyperframe_layer];

        let graphics =
            render_proposal_graphics(&project, &report, dir.path()).unwrap_or_else(|errors| {
                panic!("{kind} HyperFrame graphics should render: {errors:?}")
            });

        let hyperframe = graphics
            .iter()
            .find(|artifact| {
                artifact
                    .manifest
                    .source_layer_ids
                    .iter()
                    .any(|layer_id| layer_id == "proposal-hyperframe-1")
            })
            .unwrap_or_else(|| panic!("{kind} HyperFrame artifact should be present"));
        assert_eq!(hyperframe.template_id.as_deref(), Some(expected_template));
        assert_eq!(hyperframe.motion_preset_id.as_deref(), expected_motion);
        assert_eq!(hyperframe.visual_qa_status.as_deref(), Some("passed"));
        assert!(
            !hyperframe.sampled_frames.is_empty(),
            "{kind} HyperFrame should include sampled rendered-frame evidence"
        );
        assert!(hyperframe.sampled_frames.iter().all(|sample| dir
            .path()
            .join("proposal-hyperframe-1")
            .join(sample)
            .exists()));
    }
}

#[test]
fn render_proposal_graphics_uses_effective_draft_dimensions() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 1920;
    project.render_settings.height = 1080;
    project.render_settings.fps = 60.0;

    let graphics = render_proposal_graphics(&project, &sample_codex_report(), dir.path())
        .expect("proposal graphics should render");

    let caption = graphics
        .iter()
        .find(|graphics| {
            graphics
                .manifest
                .source_layer_ids
                .iter()
                .any(|layer_id| layer_id == "proposal-caption-1")
        })
        .expect("caption graphics should exist");

    assert_eq!(caption.manifest.dimensions.width, 1280);
    assert_eq!(caption.manifest.dimensions.height, 720);
    assert_eq!(caption.manifest.fps, 24.0);
}

#[test]
fn render_proposal_graphics_writes_gpu_artifacts_or_reports_unavailable() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let project = sample_project_for_proposal();
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();

    let result = render_proposal_graphics(&project, &report, dir.path());

    match result {
        Ok(overlays) => {
            assert_eq!(overlays.len(), 1);
            assert_eq!(overlays[0].artifact_dir, dir.path().join("shader-hook-bg"));
            assert_eq!(overlays[0].manifest.frame_count, 24);
            assert!(dir.path().join("shader-hook-bg/manifest.json").exists());
            assert!(dir.path().join("shader-hook-bg/preview.png").exists());
            assert!(dir
                .path()
                .join("shader-hook-bg/frames/frame-000023.png")
                .exists());
        }
        Err(errors)
            if errors.iter().any(|error| {
                error
                    .details
                    .get("gpuGraphicsCode")
                    .is_some_and(|code| code == "GPU_GRAPHICS_DEVICE_UNAVAILABLE")
            }) =>
        {
            eprintln!(
                "Skipping GPU render proposal artifact assertion because no compatible adapter is available"
            );
        }
        Err(errors) => panic!("unexpected render proposal GPU errors: {errors:?}"),
    }
}

#[test]
fn render_proposal_graphics_uses_software_fallback_for_hq_profile_when_gpu_unavailable() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 160;
    project.render_settings.height = 90;
    project.render_settings.fps = 4.0;

    let overlays =
        render_proposal_graphics_with_gpu_renderer(&project, &report, dir.path(), &|_, _| {
            Err(vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable,
                "adapter",
                "No compatible GPU adapter is available for offscreen graphics rendering.",
                "Use the software fallback for the HQ profile layer.",
            )])
        })
        .expect("hq profile should render with gpu or software fallback");

    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].artifact_dir, dir.path().join("shader-hook-bg"));
    assert_eq!(overlays[0].renderer, "software");
    assert_eq!(
        overlays[0].quality_profile.as_deref(),
        Some("hq-neon-wireframe-shader-v1")
    );
    assert_eq!(overlays[0].visual_qa_status.as_deref(), Some("passed"));
    assert!(dir.path().join("shader-hook-bg/manifest.json").exists());
    assert!(dir.path().join("shader-hook-bg/preview.png").exists());
}

#[test]
fn render_proposal_graphics_gpu_visual_qa_failure_includes_layer_context_after_software_fallback() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 2;
    project.render_settings.height = 2;
    project.render_settings.fps = 1.0;

    let errors =
        render_proposal_graphics_with_gpu_renderer(&project, &report, dir.path(), &|_, _| {
            Err(vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable,
                "adapter",
                "No compatible GPU adapter is available for offscreen graphics rendering.",
                "Use the software fallback for the HQ profile layer.",
            )])
        })
        .expect_err("QA failure after software fallback should reject render graphics");

    let qa_error = errors
        .iter()
        .find(|error| {
            error
                .details
                .get("gpuGraphicsCode")
                .is_some_and(|code| code == "GPU_GRAPHICS_VISUAL_QA_FAILED")
        })
        .expect("visual QA pipeline error");
    assert_eq!(
        qa_error.details.get("layerId"),
        Some(&"shader-hook-bg".to_string())
    );
    assert_eq!(
        qa_error.details.get("artifactDir"),
        Some(&dir.path().join("shader-hook-bg").display().to_string())
    );
}

#[test]
fn render_proposal_graphics_rejects_mixed_errors_without_software_fallback() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 160;
    project.render_settings.height = 90;
    project.render_settings.fps = 4.0;

    let errors =
        render_proposal_graphics_with_gpu_renderer(&project, &report, dir.path(), &|_, _| {
            Err(vec![
                GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable,
                    "adapter",
                    "No compatible GPU adapter is available for offscreen graphics rendering.",
                    "Use the software fallback only when every GPU error is device unavailable.",
                ),
                GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsShaderCompileFailed,
                    "shader.fragmentSource",
                    "GPU shader compilation failed.",
                    "Fix the shader source before rendering this GPU visual.",
                ),
            ])
        })
        .expect_err("mixed GPU errors must not use software fallback");

    assert!(errors.iter().any(|error| {
        error
            .details
            .get("gpuGraphicsCode")
            .is_some_and(|code| code == "GPU_GRAPHICS_DEVICE_UNAVAILABLE")
    }));
    assert!(errors.iter().any(|error| {
        error
            .details
            .get("gpuGraphicsCode")
            .is_some_and(|code| code == "GPU_GRAPHICS_SHADER_COMPILE_FAILED")
    }));
    assert!(!dir.path().join("shader-hook-bg/manifest.json").exists());
}

#[test]
fn render_proposal_graphics_rejects_non_device_error_without_software_fallback() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 160;
    project.render_settings.height = 90;
    project.render_settings.fps = 4.0;

    let errors =
        render_proposal_graphics_with_gpu_renderer(&project, &report, dir.path(), &|_, _| {
            Err(vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
                "readback",
                "Could not read GPU frame data.",
                "Fix the GPU readback failure before rendering this visual.",
            )])
        })
        .expect_err("non-device GPU errors must not use software fallback");

    assert!(errors.iter().any(|error| {
        error
            .details
            .get("gpuGraphicsCode")
            .is_some_and(|code| code == "GPU_GRAPHICS_READBACK_FAILED")
    }));
    assert!(!dir.path().join("shader-hook-bg/manifest.json").exists());
}

#[test]
fn old_render_plan_json_defaults_to_draft_quality() {
    let plan: RenderPlan = serde_json::from_value(serde_json::json!({
        "inputPath": "source.mp4",
        "outputPath": "renders/draft.webm",
        "width": 1280,
        "height": 720,
        "fps": 30.0,
        "clips": [
            {
                "sourceIn": 1.0,
                "sourceOut": 4.0
            }
        ]
    }))
    .expect("old render-plan JSON without quality should deserialize");

    assert_eq!(plan.quality, RenderQuality::Draft);
}

#[test]
fn gstreamer_ges_command_includes_final_webm_quality_metadata() {
    let backend = GstreamerGesRenderBackend::new();
    let mut plan = render_plan();
    plan.quality = RenderQuality::Final;

    let spec = backend
        .build_command(&plan, &[])
        .expect("valid final-quality render plan should build GStreamer/GES metadata");

    assert!(spec.args.contains(&"--quality=finalWebm".to_string()));
}

#[cfg(feature = "ges-render")]
#[test]
fn gstreamer_ges_encoding_profile_preserves_webm_codec_across_quality_settings() {
    let draft = webm_encoding_profile_summary_for_test(&render_plan())
        .expect("draft WebM GES profile should be inspectable");

    assert_eq!(draft.video_format, "video/x-vp8");
    assert_eq!(draft.encoder_factory, "vp8enc");
    assert_eq!(draft.target_bitrate_bps, 2_100_000);
    assert_eq!(draft.deadline, 1);
    assert_eq!(draft.cpu_used, 8);

    let mut final_plan = render_plan();
    final_plan.quality = RenderQuality::Final;
    final_plan.width = 1920;
    final_plan.height = 1080;
    final_plan.fps = 60.0;
    let final_profile = webm_encoding_profile_summary_for_test(&final_plan)
        .expect("final WebM GES profile should be inspectable");

    assert_eq!(final_profile.video_format, "video/x-vp8");
    assert_eq!(final_profile.encoder_factory, "vp8enc");
    assert_eq!(final_profile.target_bitrate_bps, 12_000_000);
    assert_eq!(final_profile.deadline, 1_000_000);
    assert_eq!(final_profile.cpu_used, 2);
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
#[test]
fn mp4_h264_encoding_profile_uses_native_factories() {
    let draft_plan = render_plan();
    let summary = encoding_profile_summary_for_test(&draft_plan, ExportProfile::Mp4H264)
        .expect("mp4 h264 profile summary");

    assert_eq!(summary.container_factory, "mp4mux");
    assert_eq!(summary.video_factory, "vtenc_h264");
    assert_eq!(summary.audio_factory.as_deref(), Some("atenc"));
    assert_eq!(summary.parser_factories, vec!["aacparse"]);
    assert_eq!(summary.video_bitrate_kbps, 2_100);
    assert!(summary.realtime);

    let mut final_plan = draft_plan;
    final_plan.quality = RenderQuality::Final;
    let final_summary = encoding_profile_summary_for_test(&final_plan, ExportProfile::Mp4H264)
        .expect("final mp4 h264 profile summary");

    assert_eq!(final_summary.container_factory, "mp4mux");
    assert_eq!(final_summary.video_factory, "vtenc_h264");
    assert_eq!(final_summary.video_bitrate_kbps, 6_000);
    assert!(!final_summary.realtime);
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
#[test]
fn mp4_h265_encoding_profile_uses_native_factories() {
    let mut plan = render_plan();
    plan.output_path = "renders/final.mp4".to_string();
    plan.output_profile = RenderOutputProfile::Mp4Modern;
    let summary = encoding_profile_summary_for_test(&plan, ExportProfile::Mp4H265)
        .expect("mp4 h265 profile summary");

    assert_eq!(summary.container_factory, "mp4mux");
    assert_eq!(summary.video_factory, "vtenc_h265");
    assert_eq!(summary.audio_factory.as_deref(), Some("atenc"));
    assert_eq!(summary.parser_factories, vec!["aacparse"]);
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
#[test]
fn prores_encoding_profile_uses_quicktime_and_pcm_audio() {
    let summary = encoding_profile_summary_for_test(&render_plan(), ExportProfile::ProResMov)
        .expect("prores profile summary");

    assert_eq!(summary.container_factory, "qtmux");
    assert_eq!(summary.video_factory, "vtenc_prores");
    assert_eq!(summary.audio_factory, None);
}

#[test]
fn gstreamer_ges_backend_builds_command_metadata_without_spawning_process() {
    let backend = GstreamerGesRenderBackend::new();
    let spec = backend
        .build_command(&render_plan(), &[])
        .expect("valid render plan should build GStreamer/GES metadata");

    assert_eq!(spec.program, "gstreamer-ges");
    assert!(spec.args.contains(&"--input=source.mp4".to_string()));
    assert!(spec
        .args
        .contains(&"--output=renders/draft.webm".to_string()));
    assert!(spec.args.contains(&"--quality=draftWebm".to_string()));
    assert!(spec.args.contains(&"--clip-source=source.mp4".to_string()));
    assert!(spec.args.contains(&"--clip-start=0.000".to_string()));
    assert!(spec.args.contains(&"--clip=1.000..4.000".to_string()));
}

#[test]
fn gstreamer_ges_backend_includes_clip_property_metadata() {
    let backend = GstreamerGesRenderBackend::new();
    let mut plan = render_plan();
    plan.clips[0].properties = BTreeMap::from([
        (
            "colorGrade".to_string(),
            serde_json::json!({ "exposure": 0.3 }),
        ),
        (
            "effects".to_string(),
            serde_json::json!([
                {
                    "effectType": "stylize.glow",
                    "enabled": true,
                    "params": { "radius": 12.0 }
                }
            ]),
        ),
        ("opacity".to_string(), serde_json::json!(0.42)),
    ]);

    let spec = backend
        .build_command(&plan, &[])
        .expect("valid render plan should build GStreamer/GES metadata");
    let expected_properties =
        serde_json::to_string(&plan.clips[0].properties).expect("clip properties should serialize");

    assert!(spec
        .args
        .contains(&format!("--clip-properties={expected_properties}")));
}

#[test]
fn gstreamer_ges_backend_accepts_bounded_graphics_frame_sequences() {
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "gpu-sequence.preview".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: Dimensions {
            width: 1280,
            height: 720,
        },
        fps: 30.0,
        duration_seconds: 0.2,
        alpha: true,
        frame_count: 6,
        frames_pattern: "frames/frame-%06d.png".to_string(),
        playback: graphics_playback_manifest(6, 30.0, 0.2, true, "frames/frame-%06d.png"),
        preview_path: PathBuf::from("preview.png"),
        source_layer_ids: vec!["gpu-sequence".to_string()],
        checksums: BTreeMap::new(),
    };
    let backend = GstreamerGesRenderBackend::new();

    let command = backend
        .build_command(
            &render_plan(),
            &[(
                manifest,
                PathBuf::from("/tmp/video-project/gpu-sequence"),
                0.5,
            )],
        )
        .expect("bounded frame sequence should be accepted");

    assert_eq!(command.program, "gstreamer-ges");
    assert!(command.args.iter().any(|arg| {
        arg == "--overlay=/tmp/video-project/gpu-sequence/frames/frame-000000.png@0.500+0.200"
    }));
}

#[test]
fn rendered_frame_extraction_skips_the_bundled_worker_without_sample_times() {
    let temp = tempfile::tempdir().expect("temp project dir");
    let paths = ProjectWebmRenderPaths::new("render-preview-check");
    let runner = RecordingCommandRunner::default();

    let frames = extract_rendered_frame_samples(
        &runner,
        temp.path(),
        &paths.render_dir,
        &paths.output_path,
        &[],
        Duration::from_secs(30),
    )
    .expect("extract rendered frame samples");

    assert!(frames.is_empty());
    assert!(runner.commands().is_empty());
}

#[test]
fn preview_render_comparison_request_uses_project_render_artifacts() {
    let request = build_preview_render_comparison_request(
        Path::new("/tmp/Video Project"),
        "render-preview-check",
        "renders/render-preview-check/report.json",
        "renders/render-preview-check/output.webm",
        4.0,
        &[1.25, 2.5],
        &[
            "renders/render-preview-check/frames/frame-000001.png".to_string(),
            "renders/render-preview-check/frames/frame-000002.png".to_string(),
        ],
    );

    assert_eq!(request.status, "pending");
    assert_eq!(request.project_dir, "/tmp/Video Project");
    assert_eq!(request.project_report_id, "render-preview-check");
    assert_eq!(request.duration_seconds, 4.0);
    assert_eq!(
        request.render_report_path,
        "renders/render-preview-check/report.json"
    );
    assert_eq!(request.frame_time_seconds, 1.25);
    assert!(request.fail_on_mismatch);
    assert_eq!(
        request.rendered_frames,
        vec![
            "renders/render-preview-check/frames/frame-000001.png".to_string(),
            "renders/render-preview-check/frames/frame-000002.png".to_string(),
        ]
    );
}

#[test]
fn rendered_frame_sample_times_are_deterministic_layer_starts() {
    let sample_times = rendered_frame_sample_times_for_graphics_starts(
        &[2.0, 0.3334, -1.0, f64::NAN, 2.0002, 4.0],
        Some(3.0),
    );

    assert_eq!(sample_times, vec![0.333, 2.0]);
}

#[cfg(not(feature = "ges-render"))]
#[test]
fn gstreamer_ges_backend_reports_feature_disabled_when_rendering() {
    let backend = GstreamerGesRenderBackend::new();
    let runner = RecordingRunner {
        output: ProcessOutput {
            status_code: Some(0),
            stdout: "should not be used".to_string(),
            stderr: String::new(),
        },
    };
    let errors = backend
        .render(&runner, &render_plan(), &[], Duration::from_secs(30))
        .expect_err("disabled GES backend should return an actionable error");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendUnavailable);
    assert_eq!(errors[0].path, "gstreamer.ges");
    assert!(
        errors[0].message.contains("not enabled"),
        "message should explain feature state: {}",
        errors[0].message
    );
    assert!(
        errors[0].fix.contains("--features ges-render"),
        "fix should tell developers how to enable the backend: {}",
        errors[0].fix
    );
    assert_eq!(
        errors[0].details.get("backend"),
        Some(&"gstreamer-ges".to_string())
    );
}

#[test]
fn gstreamer_ges_backend_names_required_feature() {
    let backend = GstreamerGesRenderBackend::new();

    assert_eq!(backend.backend_name(), "gstreamer-ges");
    assert_eq!(backend.required_feature(), "ges-render");
}

#[cfg(feature = "ges-render")]
#[test]
fn gstreamer_ges_backend_renders_selected_range_without_process_runner() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let dir = tempfile::tempdir().expect("temp gstreamer render dir");
    let source_path = dir.path().join("source.webm");
    let output_path = dir.path().join("selected.webm");
    generate_tiny_source_video(&source_path);

    let mut plan = render_plan();
    plan.input_path = source_path.to_string_lossy().into_owned();
    plan.output_path = output_path.to_string_lossy().into_owned();
    plan.width = 160;
    plan.height = 90;
    plan.fps = 15.0;
    plan.clips = vec![RenderClip {
        source_path: None,
        timeline_start_seconds: None,
        properties: BTreeMap::from([
            ("opacity".to_string(), serde_json::json!(0.8)),
            (
                "transform".to_string(),
                serde_json::json!({
                    "centerX": 0.5,
                    "centerY": 0.5,
                    "width": 0.8,
                    "height": 0.8
                }),
            ),
            ("cropTop".to_string(), serde_json::json!(0.05)),
            ("cropBottom".to_string(), serde_json::json!(0.05)),
            ("sourceWidth".to_string(), serde_json::json!(320)),
            ("sourceHeight".to_string(), serde_json::json!(180)),
        ]),
        timeline_track_index: 0,
        source_in: 0.25,
        source_out: 1.25,
    }];

    let output = GstreamerGesRenderBackend::new()
        .render(&PanicRunner, &plan, &[], Duration::from_secs(30))
        .expect("GES backend should render selected EDL range without spawning a process");

    assert_eq!(output.status_code, Some(0));
    assert!(
        output_path.exists(),
        "GES render should write the output file"
    );
    assert!(
        std::fs::metadata(&output_path)
            .expect("rendered webm metadata")
            .len()
            > 0,
        "GES render should not produce an empty artifact"
    );
}

#[cfg(feature = "ges-render")]
#[test]
fn gstreamer_ges_backend_renders_short_png_frame_sequence_overlay() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let dir = tempfile::tempdir().expect("temp gstreamer sequence render dir");
    let source_path = dir.path().join("source.webm");
    let output_path = dir.path().join("sequence-overlay.webm");
    generate_tiny_source_video(&source_path);

    let graphics_dir = dir.path().join("graphics/sequence");
    std::fs::create_dir_all(graphics_dir.join("frames")).expect("frames dir");
    for index in 0..4 {
        let mut image = image::RgbaImage::new(160, 90);
        for pixel in image.pixels_mut() {
            *pixel = image::Rgba([20 * index as u8, 180, 120, 180]);
        }
        image
            .save(graphics_dir.join(format!("frames/frame-{index:06}.png")))
            .expect("write overlay frame");
    }
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "sequence.preview".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: Dimensions {
            width: 160,
            height: 90,
        },
        fps: 4.0,
        duration_seconds: 1.0,
        alpha: true,
        frame_count: 4,
        frames_pattern: "frames/frame-%06d.png".to_string(),
        playback: graphics_playback_manifest(4, 4.0, 1.0, true, "frames/frame-%06d.png"),
        preview_path: PathBuf::from("preview.png"),
        source_layer_ids: vec!["sequence".to_string()],
        checksums: BTreeMap::new(),
    };

    let mut plan = render_plan();
    plan.input_path = source_path.to_string_lossy().into_owned();
    plan.output_path = output_path.to_string_lossy().into_owned();
    plan.width = 160;
    plan.height = 90;
    plan.fps = 4.0;
    plan.clips = vec![RenderClip {
        source_path: None,
        timeline_start_seconds: None,
        properties: BTreeMap::new(),
        timeline_track_index: 0,
        source_in: 0.25,
        source_out: 1.25,
    }];

    let output = GstreamerGesRenderBackend::new()
        .render(
            &PanicRunner,
            &plan,
            &[(manifest, graphics_dir, 0.0)],
            Duration::from_secs(30),
        )
        .expect("GES should render a short PNG graphics frame sequence");

    assert_eq!(output.status_code, Some(0));
    assert!(output_path.exists());
    assert!(
        std::fs::metadata(&output_path)
            .expect("sequence render metadata")
            .len()
            > 0
    );
}

#[test]
fn gstreamer_ges_backend_classifies_empty_and_invalid_clip_plans_as_clip_errors() {
    let backend = GstreamerGesRenderBackend::new();
    let mut empty_plan = render_plan();
    empty_plan.clips.clear();
    let empty_errors = backend
        .build_command(&empty_plan, &[])
        .expect_err("empty render plan should be rejected");

    assert_eq!(empty_errors.len(), 1);
    assert_eq!(
        empty_errors[0].code,
        PipelineErrorCode::RenderPlanInvalidClip
    );
    assert_eq!(
        serde_json::to_value(&empty_errors[0]).expect("error serializes")["code"],
        "RENDER_PLAN_INVALID_CLIP"
    );

    let mut invalid_clip_plan = render_plan();
    invalid_clip_plan.clips[0].source_out = invalid_clip_plan.clips[0].source_in;
    let invalid_clip_errors = backend
        .build_command(&invalid_clip_plan, &[])
        .expect_err("invalid clip range should be rejected");

    assert_eq!(invalid_clip_errors.len(), 1);
    assert_eq!(
        invalid_clip_errors[0].code,
        PipelineErrorCode::RenderPlanInvalidClip
    );
    assert_eq!(
        serde_json::to_value(&invalid_clip_errors[0]).expect("error serializes")["code"],
        "RENDER_PLAN_INVALID_CLIP"
    );
}

#[test]
fn gstreamer_plugin_policy_denies_known_non_lgpl_or_libav_factories_with_user_recovery() {
    for factory in [
        GstFactoryInfo::new("x264enc")
            .plugin_name("x264")
            .license("GPL"),
        GstFactoryInfo::new("avenc_aac")
            .plugin_name("libav")
            .license("LGPL"),
        GstFactoryInfo::new("avdec_h264")
            .plugin_name("libav")
            .license("LGPL"),
        GstFactoryInfo::new("fdkaacenc")
            .plugin_name("fdkaac")
            .license("nonfree"),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
        assert!(
            decision.fix.contains("Update Video Creater")
                && decision.fix.contains("unsupported render component"),
            "fix should give an end-user recovery path without developer-only policy instructions: {decision:?}"
        );
        assert!(!decision.fix.contains("allowlist"));
    }
}

#[test]
fn gstreamer_plugin_policy_error_serializes_for_render_reports() {
    let factory = GstFactoryInfo::new("x264enc")
        .plugin_name("x264")
        .license("GPL");

    let error =
        policy_error_for_factory(&factory).expect("denied factory should produce a pipeline error");

    assert_eq!(error.code, PipelineErrorCode::RenderBackendPolicyDenied);
    assert_eq!(error.path, "gstreamer.plugins.x264enc");
    assert!(error.message.contains("x264enc"));
    assert!(
        error.fix.contains("Update Video Creater")
            && error.fix.contains("unsupported render component"),
        "fix should give an end-user recovery path: {}",
        error.fix
    );
    assert!(!error.fix.contains("allowlist"));
    assert_eq!(error.details.get("factory"), Some(&"x264enc".to_string()));
    assert_eq!(error.details.get("verdict"), Some(&"denied".to_string()));

    let json = serde_json::to_value(&error).expect("policy error serializes");
    assert_eq!(json["code"], "RENDER_BACKEND_POLICY_DENIED");
}

#[test]
fn gstreamer_plugin_policy_marks_bad_plugins_for_review() {
    let factory = GstFactoryInfo::new("unreviewedbadenc")
        .plugin_name("unreviewedbad")
        .package("GStreamer Bad Plug-ins")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::ReviewRequired);
    assert!(
        decision.reason.contains("requires review"),
        "reason should name review requirement: {decision:?}"
    );
}

#[test]
fn gstreamer_plugin_policy_marks_bad_plugin_package_spellings_for_review() {
    let factory = GstFactoryInfo::new("unreviewedbadenc")
        .plugin_name("unreviewedbad")
        .package("GStreamer Bad Plugins")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::ReviewRequired);
    assert!(
        decision.reason.contains("requires review"),
        "reason should name review requirement: {decision:?}"
    );
}

#[test]
fn gstreamer_plugin_policy_marks_distro_style_bad_plugins_for_review() {
    let factory = GstFactoryInfo::new("unreviewedbadenc")
        .plugin_name("unreviewedbad")
        .package("gst-plugins-bad")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::ReviewRequired);
    assert!(
        decision.reason.contains("requires review"),
        "reason should name review requirement: {decision:?}"
    );
}

#[test]
fn gstreamer_plugin_policy_allows_reviewed_lgpl_core_base_and_good_factories() {
    for factory in [
        GstFactoryInfo::new("filesrc")
            .plugin_name("coreelements")
            .package("GStreamer")
            .license("LGPL"),
        GstFactoryInfo::new("audioconvert")
            .plugin_name("audioconvert")
            .package("GStreamer Base Plug-ins")
            .license("LGPL"),
        GstFactoryInfo::new("mp4mux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins")
            .license("LGPL"),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(decision.verdict, PluginPolicyVerdict::Allowed);
    }
}

#[test]
fn plugin_policy_allows_reviewed_native_export_factories() {
    let factories = [
        GstFactoryInfo::new("mp4mux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("qtmux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("aacparse")
            .plugin_name("audioparsers")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("atenc")
            .plugin_name("osxaudio")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("atdec")
            .plugin_name("osxaudio")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("vtenc_h264")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("vtenc_h265")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("vtenc_prores")
            .plugin_name("applemedia")
            .package("GStreamer Bad Plug-ins source release")
            .license("LGPL"),
    ];

    for factory in factories {
        assert_eq!(
            evaluate_gstreamer_factory(&factory).verdict,
            PluginPolicyVerdict::Allowed,
            "{factory:?}"
        );
    }
}

#[test]
fn plugin_policy_denies_unreviewed_aac_and_libav_factories() {
    let factories = [
        GstFactoryInfo::new("x264enc")
            .plugin_name("x264")
            .license("GPL"),
        GstFactoryInfo::new("avenc_aac")
            .plugin_name("libav")
            .license("LGPL"),
        GstFactoryInfo::new("avenc_aac_at")
            .plugin_name("libav")
            .license("LGPL"),
        GstFactoryInfo::new("fdkaacenc")
            .plugin_name("fdkaac")
            .license("LGPL"),
        GstFactoryInfo::new("openh264enc")
            .plugin_name("openh264")
            .license("LGPL"),
        GstFactoryInfo::new("faac")
            .plugin_name("faac")
            .license("LGPL"),
        GstFactoryInfo::new("voaacenc")
            .plugin_name("voaacenc")
            .license("LGPL"),
    ];

    for factory in factories {
        let decision = evaluate_gstreamer_factory(&factory);
        assert_eq!(decision.verdict, PluginPolicyVerdict::Denied, "{factory:?}");
        assert!(
            decision
                .reason
                .contains("denied by the render plugin policy"),
            "{factory:?}: {}",
            decision.reason
        );
    }
}

#[test]
fn plugin_policy_denies_unreviewed_libav_plugin_even_for_unknown_factory_name() {
    let factory = GstFactoryInfo::new("customaac")
        .plugin_name("libav")
        .package("GStreamer FFMPEG Plug-ins source release")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
    assert!(decision
        .reason
        .contains("denied by the render plugin policy"));
}

#[test]
fn gstreamer_plugin_policy_allows_every_reviewed_factory_with_expected_provenance() {
    for (factory_name, plugin_name, package) in reviewed_gstreamer_factory_cases() {
        let factory = GstFactoryInfo::new(factory_name)
            .plugin_name(plugin_name)
            .package(package)
            .license("LGPL");

        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(
            decision.verdict,
            PluginPolicyVerdict::Allowed,
            "{factory_name} should allow reviewed provenance: {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_denies_reviewed_factories_with_mutated_provenance_or_license() {
    for (factory_name, plugin_name, package) in reviewed_gstreamer_factory_cases() {
        for (case, factory) in [
            (
                "unexpected plugin",
                GstFactoryInfo::new(factory_name)
                    .plugin_name("unexpectedplugin")
                    .package(package)
                    .license("LGPL"),
            ),
            (
                "unexpected package",
                GstFactoryInfo::new(factory_name)
                    .plugin_name(plugin_name)
                    .package("Third Party GStreamer Plug-ins")
                    .license("LGPL"),
            ),
            (
                "mixed compact GPL license",
                GstFactoryInfo::new(factory_name)
                    .plugin_name(plugin_name)
                    .package(package)
                    .license("LGPLv2+GPLv2"),
            ),
        ] {
            let decision = evaluate_gstreamer_factory(&factory);

            assert_eq!(
                decision.verdict,
                PluginPolicyVerdict::Denied,
                "{factory_name} {case} should not pass reviewed policy checks: {decision:?}"
            );
        }
    }
}

#[test]
fn gstreamer_plugin_policy_denies_unknown_lgpl_factories_from_reviewed_packages() {
    for factory in [
        GstFactoryInfo::new("mysterysrc")
            .plugin_name("coreelements")
            .package("GStreamer")
            .license("LGPL"),
        GstFactoryInfo::new("mysteryfilter")
            .plugin_name("mysterybase")
            .package("GStreamer Base Plug-ins")
            .license("LGPL"),
        GstFactoryInfo::new("mysterymux")
            .plugin_name("mysterygood")
            .package("GStreamer Good Plug-ins")
            .license("LGPL"),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
        assert!(
            decision.reason.contains("not in the reviewed allowlist"),
            "reason should explain explicit factory allowlisting: {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_allows_reviewed_decodebin_but_denies_uri_autoplugging() {
    let decodebin = GstFactoryInfo::new("decodebin")
        .plugin_name("playback")
        .package("GStreamer Base Plug-ins")
        .license("LGPL");
    assert_eq!(
        evaluate_gstreamer_factory(&decodebin).verdict,
        PluginPolicyVerdict::Allowed
    );

    let uri_decodebin = GstFactoryInfo::new("uridecodebin")
        .plugin_name("playback")
        .package("GStreamer Base Plug-ins")
        .license("LGPL");
    let decision = evaluate_gstreamer_factory(&uri_decodebin);
    assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
    assert!(
        decision.reason.contains("not in the reviewed allowlist"),
        "reason should explain URI decode bins need explicit review: {decision:?}"
    );
}

#[test]
fn gstreamer_plugin_policy_treats_gpl_variants_and_nonfree_licenses_as_denied() {
    for license in [
        "GPLv2",
        "GPLv3",
        "LGPL/GPLv2",
        "LGPLv2+GPLv2",
        "LGPLv2|GPLv2",
        "LGPLv2GPLv2",
        "LGPL2GPL2",
        "GNU General Public License v2",
        "GNU Lesser General Public License v2.1 / GNU General Public License v2",
        "nonfree",
        "non-free",
        "proprietary",
    ] {
        let factory = GstFactoryInfo::new("filesrc")
            .plugin_name("coreelements")
            .package("GStreamer")
            .license(license);

        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
        assert!(
            decision
                .reason
                .contains("denied by the render plugin policy"),
            "license should be classified as denied, not default-denied: {license} {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_denies_lgpl_mixed_with_unreviewed_license_families() {
    for license in [
        "LGPL/MIT",
        "LGPL/BSD-2-Clause",
        "LGPL OR MPL-2.0",
        "LGPL OR MIT",
    ] {
        let factory = GstFactoryInfo::new("filesrc")
            .plugin_name("coreelements")
            .package("GStreamer")
            .license(license);

        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(
            decision.verdict,
            PluginPolicyVerdict::Denied,
            "{license} should not satisfy the LGPL-only policy: {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_allows_reviewed_package_source_release_variants() {
    for factory in [
        GstFactoryInfo::new("filesrc")
            .plugin_name("coreelements")
            .package("GStreamer source release")
            .license("LGPL"),
        GstFactoryInfo::new("audioconvert")
            .plugin_name("audioconvert")
            .package("GStreamer Base Plug-ins source release")
            .license("LGPL"),
        GstFactoryInfo::new("mp4mux")
            .plugin_name("isomp4")
            .package("GStreamer Good Plug-ins source release")
            .license("LGPL"),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(
            decision.verdict,
            PluginPolicyVerdict::Allowed,
            "source-release package metadata should match reviewed provenance: {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_allows_pure_lgpl_variants_for_reviewed_factory() {
    for license in [
        "LGPL",
        "LGPLv2",
        "LGPLv2.1",
        "LGPL-2.1-only",
        "LGPL-2.1-or-later",
        "LGPLv2 or later",
        "GNU Lesser General Public License v2.1",
    ] {
        let factory = GstFactoryInfo::new("filesrc")
            .plugin_name("coreelements")
            .package("GStreamer")
            .license(license);

        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(
            decision.verdict,
            PluginPolicyVerdict::Allowed,
            "{license} should remain allowed for reviewed LGPL provenance: {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_allows_full_form_lgpl_for_reviewed_factory() {
    let factory = GstFactoryInfo::new("filesrc")
        .plugin_name("coreelements")
        .package("GStreamer")
        .license("GNU Lesser General Public License v2.1");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::Allowed);
}

#[test]
fn gstreamer_plugin_policy_denies_allowlisted_factory_without_reviewed_provenance() {
    for (case, factory) in [
        (
            "missing plugin_name",
            GstFactoryInfo::new("mp4mux")
                .package("GStreamer Good Plug-ins")
                .license("LGPL"),
        ),
        (
            "missing package",
            GstFactoryInfo::new("mp4mux")
                .plugin_name("isomp4")
                .license("LGPL"),
        ),
        (
            "unexpected plugin_name",
            GstFactoryInfo::new("mp4mux")
                .plugin_name("thirdpartymux")
                .package("GStreamer Good Plug-ins")
                .license("LGPL"),
        ),
        (
            "unexpected package",
            GstFactoryInfo::new("mp4mux")
                .plugin_name("isomp4")
                .package("Third Party GStreamer Plug-ins")
                .license("LGPL"),
        ),
    ] {
        let decision = evaluate_gstreamer_factory(&factory);

        assert_eq!(
            decision.verdict,
            PluginPolicyVerdict::Denied,
            "{case} should not pass reviewed provenance checks: {decision:?}"
        );
        assert!(
            decision.reason.contains("not in the reviewed allowlist"),
            "reason should explain reviewed provenance is required: {case} {decision:?}"
        );
    }
}

#[test]
fn gstreamer_plugin_policy_allows_reviewed_factory_with_full_form_lgpl_provenance() {
    let factory = GstFactoryInfo::new("mp4mux")
        .plugin_name("isomp4")
        .package("GStreamer Good Plug-ins")
        .license("GNU Lesser General Public License v2.1");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::Allowed);
}

#[test]
fn gstreamer_plugin_policy_denies_unknown_factories_by_default() {
    let factory = GstFactoryInfo::new("mysteryenc")
        .plugin_name("mystery")
        .license("LGPL");

    let decision = evaluate_gstreamer_factory(&factory);

    assert_eq!(decision.verdict, PluginPolicyVerdict::Denied);
    assert!(
        decision.reason.contains("not in the reviewed allowlist"),
        "reason should explain the default-deny allowlist: {decision:?}"
    );
}

#[test]
fn gstreamer_plugin_policy_error_code_serializes_policy_denial() {
    let error = PipelineError::new(
        PipelineErrorCode::RenderBackendPolicyDenied,
        "render.backend.plugins[0]",
        "GStreamer factory is denied by policy.",
        "Choose an approved LGPL-compatible GStreamer element.",
    );

    let json = serde_json::to_value(&error).expect("pipeline error should serialize");

    assert_eq!(json["code"], "RENDER_BACKEND_POLICY_DENIED");
}

fn reviewed_gstreamer_factory_cases() -> [(&'static str, &'static str, &'static str); 35] {
    [
        ("filesrc", "coreelements", "GStreamer"),
        ("queue", "coreelements", "GStreamer"),
        ("multifilesrc", "multifile", "GStreamer Good Plug-ins"),
        (
            "videoconvert",
            "videoconvertscale",
            "GStreamer Base Plug-ins",
        ),
        ("videoscale", "videoconvertscale", "GStreamer Base Plug-ins"),
        ("videorate", "videorate", "GStreamer Base Plug-ins"),
        ("pngdec", "png", "GStreamer Good Plug-ins"),
        ("pngenc", "png", "GStreamer Good Plug-ins"),
        ("videoflip", "videofilter", "GStreamer Good Plug-ins"),
        ("videocrop", "videocrop", "GStreamer Good Plug-ins"),
        ("audioconvert", "audioconvert", "GStreamer Base Plug-ins"),
        ("audiomixer", "audiomixer", "GStreamer Base Plug-ins"),
        ("audiorate", "audiorate", "GStreamer Base Plug-ins"),
        ("audioresample", "audioresample", "GStreamer Base Plug-ins"),
        ("compositor", "compositor", "GStreamer Base Plug-ins"),
        ("volume", "volume", "GStreamer Base Plug-ins"),
        ("scaletempo", "audiofx", "GStreamer Good Plug-ins"),
        ("autoaudiosink", "autodetect", "GStreamer Good Plug-ins"),
        ("autovideosink", "autodetect", "GStreamer Good Plug-ins"),
        ("appsrc", "app", "GStreamer Base Plug-ins"),
        ("capsfilter", "coreelements", "GStreamer"),
        ("mp4mux", "isomp4", "GStreamer Good Plug-ins"),
        ("qtmux", "isomp4", "GStreamer Good Plug-ins"),
        ("aacparse", "audioparsers", "GStreamer Good Plug-ins"),
        ("atenc", "osxaudio", "GStreamer Good Plug-ins"),
        ("atdec", "osxaudio", "GStreamer Good Plug-ins"),
        ("vtenc_h264", "applemedia", "GStreamer Bad Plug-ins"),
        ("vtenc_h265", "applemedia", "GStreamer Bad Plug-ins"),
        ("vtenc_prores", "applemedia", "GStreamer Bad Plug-ins"),
        ("matroskamux", "matroska", "GStreamer Good Plug-ins"),
        ("webmmux", "matroska", "GStreamer Good Plug-ins"),
        ("vp8enc", "vpx", "GStreamer Good Plug-ins"),
        ("vp9enc", "vpx", "GStreamer Good Plug-ins"),
        ("opusenc", "opus", "GStreamer Base Plug-ins"),
        ("opusdec", "opus", "GStreamer Base Plug-ins"),
    ]
}

#[cfg(feature = "ges-render")]
#[test]
fn curated_non_macos_input_decode_preflight_uses_common_factories_only() {
    assert_eq!(
        required_input_decode_factories_for_platform_for_test(false),
        vec!["decodebin", "qtdemux", "h264parse", "aacparse"]
    );
}

#[cfg(feature = "ges-render")]
#[test]
fn curated_macos_input_decode_preflight_adds_apple_factories() {
    assert_eq!(
        required_input_decode_factories_for_platform_for_test(true),
        vec![
            "decodebin",
            "qtdemux",
            "h264parse",
            "aacparse",
            "vtdec",
            "atdec",
        ]
    );
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
#[test]
fn curated_input_decode_preflight_selects_macos_factories() {
    assert_eq!(
        required_input_decode_factories_for_test(),
        &[
            "decodebin",
            "qtdemux",
            "h264parse",
            "aacparse",
            "vtdec",
            "atdec",
        ]
    );
}

#[cfg(all(feature = "ges-render", not(target_os = "macos")))]
#[test]
fn curated_input_decode_preflight_selects_non_macos_factories() {
    assert_eq!(
        required_input_decode_factories_for_test(),
        &[
            "decodebin",
            "qtdemux",
            "h264parse",
            "aacparse",
            "avdec_h264",
            "avdec_aac",
        ]
    );
}

#[cfg(all(feature = "ges-render", target_os = "macos"))]
#[test]
fn curated_runtime_discovers_h264_aac_edison_fixture() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/media/edison-speech-1920s-30s.mp4");
    start_render_process_runtime().expect("initialize curated render runtime");

    let (probe, output) =
        probe_media_with_gstreamer(&fixture, Duration::from_secs(30), "edisonFixture")
            .expect("curated runtime should discover H.264 + AAC-LC fixture");

    assert_eq!(
        probe
            .video
            .as_ref()
            .and_then(|video| video.codec_name.as_deref()),
        Some("video/x-h264"),
        "discoverer should expose the fixture's H.264 caps"
    );
    assert_eq!(
        probe
            .audio
            .as_ref()
            .and_then(|audio| audio.codec_name.as_deref()),
        Some("aac"),
        "discoverer should identify AAC from GStreamer's audio/mpeg caps"
    );
    assert_eq!(output.status_code, Some(0));
}

#[test]
fn render_reports_write_json_and_markdown() {
    let dir = tempfile::tempdir().expect("temp report dir");
    let report = render_report();
    let json_path = dir.path().join("render-report.json");
    let markdown_path = dir.path().join("render-report.md");

    write_json_report(&json_path, &report).expect("json report should write");
    write_markdown_report(&markdown_path, &report).expect("markdown report should write");

    let json = std::fs::read_to_string(&json_path).expect("read json report");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("report json parses");
    assert_eq!(parsed["jobId"], "render-job-1");
    assert_eq!(parsed["summary"]["status"], "succeeded");
    assert_eq!(parsed["summary"]["quality"], "draft");
    assert_eq!(parsed["summary"]["requestedWidth"], 1280);
    assert_eq!(parsed["summary"]["requestedHeight"], 720);
    assert_eq!(parsed["summary"]["actualWidth"], 1280);
    assert_eq!(parsed["summary"]["actualHeight"], 720);
    assert_eq!(parsed["command"]["program"], "gstreamer-ges");

    let markdown = std::fs::read_to_string(&markdown_path).expect("read markdown report");
    assert!(markdown.contains("# Render Report: render-job-1"));
    assert!(markdown.contains("- Status: succeeded"));
    assert!(markdown.contains("- Output: renders/draft.webm"));
    assert!(markdown.contains("- Quality: draft"));
    assert!(markdown.contains("- Requested dimensions: 1280x720"));
    assert!(markdown.contains("- Actual dimensions: 1280x720"));
    assert!(markdown.contains("`gstreamer-ges --output=renders/draft.webm`"));
}

#[test]
fn render_report_records_gpu_visual_renderer_modes() {
    let dir = tempfile::tempdir().expect("temp report dir");
    let markdown_path = dir.path().join("render-report.md");
    let report = RenderReport {
        job_id: "render-proposal".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(1.0),
            output_path: Some("renders/final.webm".to_string()),
            ..RenderReportSummary::default()
        },
        command: CommandSpec::new("gstreamer-ges"),
        stdout: String::new(),
        stderr: String::new(),
        streams: None,
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: vec![RenderGraphicsReport {
            layer_id: "shader-hook-bg".to_string(),
            renderer: "software".to_string(),
            quality_profile: Some("hq-neon-wireframe-shader-v1".to_string()),
            template_id: None,
            motion_preset_id: None,
            visual_qa_status: Some("passed".to_string()),
            cache_status: None,
            sampled_frames: Vec::new(),
            qa_metrics: BTreeMap::new(),
        }],
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };

    let json = serde_json::to_string(&report).expect("report json");
    assert!(json.contains("\"renderer\":\"software\""));
    assert!(json.contains("hq-neon-wireframe-shader-v1"));
    assert!(json.contains("\"visualQaStatus\":\"passed\""));

    write_markdown_report(&markdown_path, &report).expect("markdown report should write");
    let markdown = std::fs::read_to_string(&markdown_path).expect("read markdown report");
    assert!(markdown.contains("## Graphics"));
    assert!(markdown.contains("renderer=software"));
    assert!(markdown.contains("profile=hq-neon-wireframe-shader-v1"));
    assert!(markdown.contains("qa=passed"));
}

#[test]
fn render_report_graphics_include_template_preset_and_qa_metadata() {
    let mut report = render_report();
    report.graphics = vec![RenderGraphicsReport {
        layer_id: "proposal-overlay-1".to_string(),
        renderer: "rust".to_string(),
        quality_profile: None,
        template_id: Some("kinetic-lower-third-v1".to_string()),
        motion_preset_id: Some("slide-fade-up-v1".to_string()),
        visual_qa_status: Some("passed".to_string()),
        cache_status: None,
        sampled_frames: vec!["frames/frame-000000.png".to_string()],
        qa_metrics: BTreeMap::from([
            ("visibleAlphaRatio".to_string(), "0.052000".to_string()),
            ("temporalDelta".to_string(), "0.004200".to_string()),
        ]),
    }];

    let json = serde_json::to_value(&report).expect("serialize report");

    assert_eq!(json["graphics"][0]["templateId"], "kinetic-lower-third-v1");
    assert_eq!(json["graphics"][0]["motionPresetId"], "slide-fade-up-v1");
    assert_eq!(json["graphics"][0]["visualQaStatus"], "passed");
    assert_eq!(
        json["graphics"][0]["sampledFrames"][0],
        "frames/frame-000000.png"
    );
    assert_eq!(
        json["graphics"][0]["qaMetrics"]["visibleAlphaRatio"],
        "0.052000"
    );
}

#[test]
fn render_report_serializes_graphics_visual_qa_metrics() {
    let report = RenderReport {
        job_id: "render-proposal".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(1.0),
            output_path: Some("renders/final.webm".to_string()),
            ..RenderReportSummary::default()
        },
        command: CommandSpec::new("gstreamer-ges"),
        stdout: String::new(),
        stderr: String::new(),
        streams: None,
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: vec![RenderGraphicsReport {
            layer_id: "proposal-caption-1".to_string(),
            renderer: "rust".to_string(),
            quality_profile: None,
            template_id: None,
            motion_preset_id: Some("snap-pop-v1".to_string()),
            visual_qa_status: Some("passed".to_string()),
            cache_status: Some("miss:metadata missing".to_string()),
            sampled_frames: vec![
                "frames/frame-000000.png".to_string(),
                "frames/frame-000026.png".to_string(),
                "frames/frame-000052.png".to_string(),
            ],
            qa_metrics: BTreeMap::from([
                ("visibleAlphaRatio".to_string(), "0.073400".to_string()),
                ("temporalDelta".to_string(), "0.002900".to_string()),
            ]),
        }],
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    };

    let json = serde_json::to_value(&report).expect("serialize report");

    assert_eq!(
        json["graphics"][0]["sampledFrames"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        json["graphics"][0]["qaMetrics"]["temporalDelta"],
        "0.002900"
    );
}

#[test]
fn render_report_serializes_preview_render_comparison_evidence() {
    let mut report = render_report();
    report.preview_comparison = Some(RenderPreviewComparison {
        status: "failed".to_string(),
        compared_frames: vec![
            RenderPreviewComparisonFrame {
                timeline_seconds: 1.25,
                preview_frame: "visual-qa/preview-001.png".to_string(),
                rendered_frame: "renders/frames/frame-000030.png".to_string(),
                diff_frame: Some("visual-qa/diff-001.png".to_string()),
                mismatch_ratio: 0.08,
                passed: false,
            },
            RenderPreviewComparisonFrame {
                timeline_seconds: 2.5,
                preview_frame: "visual-qa/preview-002.png".to_string(),
                rendered_frame: "renders/frames/frame-000060.png".to_string(),
                diff_frame: None,
                mismatch_ratio: 0.004,
                passed: true,
            },
        ],
    });

    let json = serde_json::to_value(&report).expect("serialize report");

    assert_eq!(json["previewComparison"]["status"], "failed");
    assert_eq!(
        json["previewComparison"]["comparedFrames"][0]["timelineSeconds"],
        1.25
    );
    assert_eq!(
        json["previewComparison"]["comparedFrames"][0]["previewFrame"],
        "visual-qa/preview-001.png"
    );
    assert_eq!(
        json["previewComparison"]["comparedFrames"][0]["renderedFrame"],
        "renders/frames/frame-000030.png"
    );
    assert_eq!(
        json["previewComparison"]["comparedFrames"][0]["diffFrame"],
        "visual-qa/diff-001.png"
    );
    assert_eq!(
        json["previewComparison"]["comparedFrames"][0]["mismatchRatio"],
        0.08
    );
    assert_eq!(
        json["previewComparison"]["comparedFrames"][1]["passed"],
        true
    );
}

#[test]
fn render_report_serializes_automatic_preview_render_comparison_request() {
    let dir = tempfile::tempdir().expect("temp report dir");
    let markdown_path = dir.path().join("render-report.md");
    let mut report = render_report();
    report.preview_comparison_request = Some(RenderPreviewComparisonRequest {
        status: "pending".to_string(),
        project_dir: "/tmp/project".to_string(),
        project_report_id: "render-job-1".to_string(),
        render_report_path: "renders/render-job-1/report.json".to_string(),
        rendered_video: "renders/render-job-1/output.webm".to_string(),
        duration_seconds: 4.0,
        frame_time_seconds: 1.25,
        rendered_frames: vec!["renders/render-job-1/frames/frame-000001.png".to_string()],
        fail_on_mismatch: true,
    });

    let json = serde_json::to_value(&report).expect("serialize report");

    assert_eq!(json["previewComparisonRequest"]["status"], "pending");
    assert_eq!(
        json["previewComparisonRequest"]["projectReportId"],
        "render-job-1"
    );
    assert_eq!(
        json["previewComparisonRequest"]["renderedFrames"][0],
        "renders/render-job-1/frames/frame-000001.png"
    );
    assert_eq!(json["previewComparisonRequest"]["durationSeconds"], 4.0);
    assert_eq!(json["previewComparisonRequest"]["failOnMismatch"], true);
    assert!(json["previewComparisonRequest"].get("command").is_none());

    write_markdown_report(&markdown_path, &report).expect("markdown report should write");
    let markdown = std::fs::read_to_string(&markdown_path).expect("read markdown report");
    assert!(markdown.contains("## Preview/Render QA Request"));
    assert!(markdown.contains("Status: pending"));
    assert!(markdown.contains("Runner: bundled native preview review"));
}

#[test]
fn project_render_report_fails_visual_timing_without_sampled_rendered_frames() {
    let paths = ProjectWebmRenderPaths::new("render-job-1");
    let mut report = render_report();
    report.summary.output_path = Some(paths.output_path.clone());
    report.artifacts = vec![
        paths.output_path.clone(),
        paths.json_report_path.clone(),
        paths.markdown_report_path.clone(),
        paths.log_path.clone(),
    ];
    report.graphics = vec![RenderGraphicsReport {
        layer_id: "proposal-caption-1".to_string(),
        renderer: "rust".to_string(),
        quality_profile: None,
        template_id: Some("caption-pop-v1".to_string()),
        motion_preset_id: Some("snap-pop-v1".to_string()),
        visual_qa_status: Some("passed".to_string()),
        cache_status: Some("hit".to_string()),
        sampled_frames: Vec::new(),
        qa_metrics: BTreeMap::new(),
    }];

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-07-04T00:00:00Z",
        &ProjectRenderReportEvidence {
            video_stream: true,
            audio_stream: false,
            audio_required: false,
            expected_duration_seconds: Some(3.0),
            duration_tolerance_seconds: Some(0.05),
        },
    )
    .expect("project render report");

    assert_eq!(
        project_report.checks["captionAlignment"],
        RenderReportCheckStatus::Failed
    );
    assert_eq!(
        project_report.checks["overlayTiming"],
        RenderReportCheckStatus::Failed
    );
    assert_eq!(project_report.status, RenderReportStatus::Failed);
}

#[test]
fn project_render_report_fails_visual_timing_when_sampled_frames_are_not_artifacts() {
    let paths = ProjectWebmRenderPaths::new("render-job-1");
    let mut report = render_report();
    report.summary.output_path = Some(paths.output_path.clone());
    report.artifacts = vec![
        paths.output_path.clone(),
        paths.json_report_path.clone(),
        paths.markdown_report_path.clone(),
        paths.log_path.clone(),
    ];
    report.graphics = vec![RenderGraphicsReport {
        layer_id: "proposal-overlay-1".to_string(),
        renderer: "rust".to_string(),
        quality_profile: None,
        template_id: Some("kinetic-lower-third-v1".to_string()),
        motion_preset_id: Some("slide-fade-up-v1".to_string()),
        visual_qa_status: Some("passed".to_string()),
        cache_status: Some("hit".to_string()),
        sampled_frames: vec![
            "renders/render-job-1/graphics/proposal-overlay-1/frames/frame-000000.png".to_string(),
        ],
        qa_metrics: BTreeMap::new(),
    }];

    let project_report = project_render_report_from_pipeline_report(
        &report,
        &paths,
        "2026-07-04T00:00:00Z",
        &ProjectRenderReportEvidence {
            video_stream: true,
            audio_stream: false,
            audio_required: false,
            expected_duration_seconds: Some(3.0),
            duration_tolerance_seconds: Some(0.05),
        },
    )
    .expect("project render report");

    assert_eq!(
        project_report.checks["captionAlignment"],
        RenderReportCheckStatus::Failed
    );
    assert_eq!(
        project_report.checks["overlayTiming"],
        RenderReportCheckStatus::Failed
    );
    assert_eq!(project_report.status, RenderReportStatus::Failed);
}

#[test]
fn render_report_markdown_summarizes_large_artifact_lists() {
    let dir = tempfile::tempdir().expect("temp report dir");
    let mut report = render_report();
    report.artifacts = (0..45)
        .map(|index| format!("renders/graphics/frame-{index:06}.png"))
        .collect();
    let json_path = dir.path().join("render-report.json");
    let markdown_path = dir.path().join("render-report.md");

    write_json_report(&json_path, &report).expect("json report should write");
    write_markdown_report(&markdown_path, &report).expect("markdown report should write");

    let json = std::fs::read_to_string(&json_path).expect("read json report");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("report json parses");
    let markdown = std::fs::read_to_string(&markdown_path).expect("read markdown report");

    assert_eq!(
        parsed["artifacts"]
            .as_array()
            .expect("artifact array")
            .len(),
        45
    );
    assert!(markdown.contains("renders/graphics/frame-000039.png"));
    assert!(!markdown.contains("renders/graphics/frame-000040.png"));
    assert!(markdown.contains(
        "... 5 more artifacts omitted from Markdown; see JSON report for the complete list."
    ));
}

#[test]
fn command_spec_formats_program_and_args_for_reports() {
    let spec = CommandSpec::new("gstreamer-ges")
        .arg("--input=clip.webm")
        .args(["--output=renders/draft.webm", "--clip=0.000..1.000"]);

    assert_eq!(spec.program, "gstreamer-ges");
    assert_eq!(
        spec.args,
        vec![
            "--input=clip.webm",
            "--output=renders/draft.webm",
            "--clip=0.000..1.000"
        ]
    );
    assert_eq!(
        spec.display(),
        "gstreamer-ges --input=clip.webm --output=renders/draft.webm --clip=0.000..1.000"
    );

    let json = serde_json::to_value(&spec).expect("command spec serializes");
    assert_eq!(json["program"], "gstreamer-ges");
    assert_eq!(json["args"][0], "--input=clip.webm");
}

#[test]
fn process_runner_converts_spawn_error_to_actionable_error() {
    let runner = SystemProcessRunner;
    let spec = CommandSpec::new("missing-gstreamer-for-test");

    let errors = runner
        .run(&spec, Duration::from_secs(1))
        .expect_err("missing binary should become a pipeline error");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendUnavailable);
    assert_eq!(errors[0].path, "process.program");
    assert!(
        errors[0].message.contains("missing-gstreamer-for-test"),
        "message should identify missing binary: {}",
        errors[0].message
    );
    assert_eq!(
        errors[0].details.get("program"),
        Some(&"missing-gstreamer-for-test".to_string())
    );
}

#[test]
fn process_runner_bounds_nonzero_stderr_detail() {
    let runner = SystemProcessRunner;
    let spec = CommandSpec::new("sh").args([
        "-c",
        "i=0; while [ $i -lt 80 ]; do printf 'stderr-line-%02d ' \"$i\" >&2; i=$((i + 1)); done; exit 7",
    ]);

    let errors = runner
        .run(&spec, Duration::from_secs(1))
        .expect_err("nonzero command should become a pipeline error");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendFailed);
    let stderr = errors[0]
        .details
        .get("stderr")
        .expect("stderr summary should be present");
    assert!(
        stderr.len() <= 500,
        "stderr detail should be bounded, got {} chars",
        stderr.len()
    );
    assert!(
        stderr.contains("stderr-line-79"),
        "stderr summary should keep the tail: {stderr}"
    );
    assert!(
        !stderr.contains("stderr-line-00"),
        "stderr summary should omit the oldest content: {stderr}"
    );
}

#[test]
fn process_runner_maps_timeout_to_actionable_error() {
    let runner = SystemProcessRunner;
    let spec = CommandSpec::new("/bin/sh").args(["-c", "sleep 2"]);

    let errors = runner
        .run(&spec, Duration::from_millis(25))
        .expect_err("timed out command should become a pipeline error");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendTimeout);
    assert_eq!(errors[0].path, "process.timeout");
    assert_eq!(
        errors[0].details.get("program"),
        Some(&"/bin/sh".to_string())
    );
    assert_eq!(errors[0].details.get("timeoutMs"), Some(&"25".to_string()));
}

#[test]
fn process_runner_timeout_terminates_descendant_pipe_holders() {
    let runner = SystemProcessRunner;
    let spec = CommandSpec::new("/bin/sh").args(["-c", "sleep 2 >&2 & printf 'wrapper ready' >&2"]);

    let started_at = Instant::now();
    let errors = runner
        .run(&spec, Duration::from_millis(50))
        .expect_err("timed out process group should become a pipeline error");
    let elapsed = started_at.elapsed();

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendTimeout);
    assert!(
        elapsed < Duration::from_millis(750),
        "timeout should not wait for descendant pipe holders, elapsed: {elapsed:?}"
    );
    assert_eq!(
        errors[0].details.get("stderr"),
        Some(&"wrapper ready".to_string())
    );
}

#[test]
fn media_validation_reports_dimension_mismatch() {
    let probe = media_probe(1280, 720, 4.0, true);

    let expected = ExpectedMedia {
        width: Some(1920),
        height: Some(1080),
        video_required: true,
        audio_required: true,
        non_empty_required: true,
        expected_duration_seconds: None,
        duration_tolerance_seconds: None,
        container: None,
        video_codec: None,
        audio_codec: None,
    };

    let errors = validate_rendered_media(Path::new("renders/draft.webm"), &probe, &expected)
        .expect_err("dimension mismatch should fail validation");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].code,
        PipelineErrorCode::RenderProbeValidationFailed
    );
    assert_eq!(errors[0].path, "streams.video.dimensions");
    assert_eq!(
        errors[0].details.get("path"),
        Some(&"renders/draft.webm".to_string())
    );
    assert_eq!(
        errors[0].details.get("actual"),
        Some(&"1280x720".to_string())
    );
    assert_eq!(
        errors[0].details.get("expected"),
        Some(&"1920x1080".to_string())
    );
}

#[test]
fn media_validation_rejects_mismatched_container_and_codecs() {
    let probe = MediaProbe {
        container_name: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
        duration_seconds: Some(4.0),
        size_bytes: Some(2048),
        video: Some(VideoProbe {
            codec_name: Some("hevc".to_string()),
            width: Some(1280),
            height: Some(720),
            fps: Some(30.0),
        }),
        audio: Some(AudioProbe {
            codec_name: Some("opus".to_string()),
        }),
    };
    let expected = ExpectedMedia {
        width: Some(1280),
        height: Some(720),
        video_required: true,
        audio_required: true,
        non_empty_required: true,
        expected_duration_seconds: None,
        duration_tolerance_seconds: None,
        container: Some("mp4".to_string()),
        video_codec: Some("h264".to_string()),
        audio_codec: Some("aac".to_string()),
    };

    let errors = validate_rendered_media(Path::new("renders/draft.mp4"), &probe, &expected)
        .expect_err("codec mismatches must fail validation");
    assert_eq!(
        errors
            .iter()
            .map(|error| error.path.as_str())
            .collect::<Vec<_>>(),
        vec!["streams.video.codec", "streams.audio.codec"]
    );
}

#[test]
fn media_validation_reports_duration_mismatch() {
    let probe = media_probe(1280, 720, 4.0, true);

    let expected = ExpectedMedia {
        width: Some(1280),
        height: Some(720),
        video_required: true,
        audio_required: true,
        non_empty_required: true,
        expected_duration_seconds: Some(3.0),
        duration_tolerance_seconds: Some(1.0 / 30.0),
        ..ExpectedMedia::default()
    };

    let errors = validate_rendered_media(Path::new("renders/draft.webm"), &probe, &expected)
        .expect_err("duration mismatch should fail validation");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].code,
        PipelineErrorCode::RenderProbeValidationFailed
    );
    assert_eq!(errors[0].path, "format.duration");
    assert_eq!(errors[0].details.get("actual"), Some(&"4.000".to_string()));
    assert_eq!(
        errors[0].details.get("expected"),
        Some(&"3.000".to_string())
    );
    assert_eq!(
        errors[0].details.get("tolerance"),
        Some(&"0.033".to_string())
    );
}

#[test]
fn media_validation_reports_single_dimension_mismatch() {
    let probe = media_probe(1280, 720, 4.0, false);

    let width_errors = validate_rendered_media(
        Path::new("renders/width-only.mp4"),
        &probe,
        &ExpectedMedia {
            width: Some(1920),
            height: None,
            video_required: true,
            audio_required: false,
            non_empty_required: true,
            expected_duration_seconds: None,
            duration_tolerance_seconds: None,
            ..ExpectedMedia::default()
        },
    )
    .expect_err("width-only mismatch should fail validation");

    assert_eq!(width_errors.len(), 1);
    assert_eq!(width_errors[0].path, "streams.video.width");
    assert_eq!(
        width_errors[0].details.get("actual"),
        Some(&"1280".to_string())
    );
    assert_eq!(
        width_errors[0].details.get("expected"),
        Some(&"1920".to_string())
    );

    let height_errors = validate_rendered_media(
        Path::new("renders/height-only.mp4"),
        &probe,
        &ExpectedMedia {
            width: None,
            height: Some(1080),
            video_required: true,
            audio_required: false,
            non_empty_required: true,
            expected_duration_seconds: None,
            duration_tolerance_seconds: None,
            ..ExpectedMedia::default()
        },
    )
    .expect_err("height-only mismatch should fail validation");

    assert_eq!(height_errors.len(), 1);
    assert_eq!(height_errors[0].path, "streams.video.height");
    assert_eq!(
        height_errors[0].details.get("actual"),
        Some(&"720".to_string())
    );
    assert_eq!(
        height_errors[0].details.get("expected"),
        Some(&"1080".to_string())
    );
}

fn media_probe(width: u32, height: u32, duration_seconds: f64, include_audio: bool) -> MediaProbe {
    MediaProbe {
        container_name: Some("webm".to_string()),
        duration_seconds: Some(duration_seconds),
        size_bytes: Some(2048),
        video: Some(VideoProbe {
            codec_name: Some("vp8".to_string()),
            width: Some(width),
            height: Some(height),
            fps: Some(30.0),
        }),
        audio: include_audio.then_some(AudioProbe {
            codec_name: Some("opus".to_string()),
        }),
    }
}

#[cfg(not(feature = "ges-render"))]
#[derive(Debug, Clone)]
struct RecordingRunner {
    output: ProcessOutput,
}

#[cfg(not(feature = "ges-render"))]
impl ProcessRunner for RecordingRunner {
    fn run(&self, _spec: &CommandSpec, _timeout: Duration) -> PipelineResult<ProcessOutput> {
        Ok(self.output.clone())
    }
}

#[derive(Debug, Default)]
struct RecordingCommandRunner {
    commands: std::sync::Mutex<Vec<CommandSpec>>,
}

impl RecordingCommandRunner {
    fn commands(&self) -> Vec<CommandSpec> {
        self.commands.lock().expect("commands").clone()
    }
}

impl ProcessRunner for RecordingCommandRunner {
    fn run(&self, spec: &CommandSpec, _timeout: Duration) -> PipelineResult<ProcessOutput> {
        self.commands.lock().expect("commands").push(spec.clone());
        Ok(ProcessOutput {
            status_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct PanicRunner;

impl ProcessRunner for PanicRunner {
    fn run(&self, spec: &CommandSpec, _timeout: Duration) -> PipelineResult<ProcessOutput> {
        panic!(
            "GStreamer/GES render should not invoke an external process runner: {}",
            spec.display()
        );
    }
}

fn render_plan() -> RenderPlan {
    RenderPlan {
        input_path: "source.mp4".to_string(),
        output_path: "renders/draft.webm".to_string(),
        width: 1280,
        height: 720,
        fps: 30.0,
        quality: RenderQuality::Draft,
        output_profile: RenderOutputProfile::default(),
        encode_tier: video_creater_lib::edit::render_plan::ExportEncodeTier::Standard,
        clips: vec![RenderClip {
            source_path: None,
            timeline_start_seconds: None,
            properties: BTreeMap::new(),
            timeline_track_index: 0,
            source_in: 1.0,
            source_out: 4.0,
        }],
        audio_clips: Vec::new(),
        transitions: Vec::new(),
        audio_transitions: Vec::new(),
    }
}

fn render_report() -> RenderReport {
    RenderReport {
        job_id: "render-job-1".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(3.0),
            output_path: Some("renders/draft.webm".to_string()),
            quality: Some(RenderQuality::Draft),
            requested_width: Some(1280),
            requested_height: Some(720),
            actual_width: Some(1280),
            actual_height: Some(720),
            container: Some("webm".to_string()),
            video_codec: Some("vp8".to_string()),
            audio_codec: Some("opus".to_string()),
        },
        command: CommandSpec::new("gstreamer-ges").arg("--output=renders/draft.webm"),
        stdout: "rendered 90 frames".to_string(),
        stderr: String::new(),
        streams: None,
        errors: Vec::new(),
        artifacts: vec!["renders/draft.webm".to_string()],
        graphics: Vec::new(),
        performance: None,
        preview_comparison_request: None,
        preview_comparison: None,
    }
}

fn sample_cache_graphics_layer(id: &str) -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: id.to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 320,
            height: 180,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "cache beat".to_string(),
        visual_treatment: "cache test visual".to_string(),
        motion: "static".to_string(),
        safe_zone: "inside frame".to_string(),
        avoid: "none".to_string(),
        nodes: Vec::new(),
    }
}

fn sample_project_for_proposal() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Render Pipeline Proposal Test".to_string(),
        "2026-06-17T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        name: None,
        relative_path: "media/input.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(1920),
        height: Some(1080),
        fps: Some(24.0),
        folder_id: None,
    });
    project
}

fn sample_proposal_request() -> EditJobRequest {
    EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Cut a sharp trailer-style edit with captions and one lower third.".to_string(),
        target_duration_seconds: Some(45.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-17T00:00:00Z".to_string(),
    }
}

fn sample_codex_proposal() -> CodexEditProposal {
    CodexEditProposal {
        media_id: "media-1".to_string(),
        clips: vec![
            CodexProposalClip {
                media_id: "media-1".to_string(),
                source_in: 1.5,
                source_out: 29.0,
                reason: "opening hook and setup".to_string(),
            },
            CodexProposalClip {
                media_id: "media-1".to_string(),
                source_in: 45.25,
                source_out: 62.75,
                reason: "payoff quote and motion".to_string(),
            },
        ],
        captions: vec![serde_json::json!({
            "text": "Build the edit around the hook",
            "startSeconds": 2.0,
            "durationSeconds": 2.2,
            "sourceBeat": "emphasize the opening hook",
            "visualTreatment": "bold caption with compact translucent backing",
            "motion": "quick pop-in with subtle upward drift",
            "safeZone": "keep text inside lower 20% and 10% side margins",
            "avoid": "full-width opaque black caption slabs"
        })],
        overlays: vec![serde_json::json!({
            "kind": "lower_third",
            "templateId": "kinetic-lower-third-v1",
            "startSeconds": 5.0,
            "durationSeconds": 3.0,
            "fields": {
                "headline": "Proposal Pipeline",
                "subline": "Rust render path"
            },
            "brief": "Label the conversion milestone.",
            "sourceBeat": "identify the render pipeline segment",
            "visualTreatment": "compact lower-third chip with accent line and translucent backing",
            "motion": "slide in from left then fade out",
            "safeZone": "keep essential text inside 10% margins",
            "avoid": "centered text on plain boxes"
        })],
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 45.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    }
}

fn sample_codex_report() -> CodexProposalReport {
    CodexProposalReport {
        generated_at: "2026-06-17T00:00:00Z".to_string(),
        proposal: sample_codex_proposal(),
    }
}

fn sample_hyperframe_layer() -> serde_json::Value {
    serde_json::json!({
        "kind": "title_card",
        "role": "title_card",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with a full-frame HyperFrame title scene.",
        "visualTreatment": "kinetic editorial title scene",
        "motion": "fast type-on with camera push",
        "safeZone": "keep title inside 10% margins",
        "avoid": "static text-only cards"
    })
}

fn sample_template_hyperframe_layer() -> serde_json::Value {
    serde_json::json!({
        "kind": "template_overlay",
        "templateId": "chapter-card-v1",
        "startSeconds": 0.0,
        "durationSeconds": 2.0,
        "brief": "Open with a full-frame HyperFrame title scene.",
        "fields": {
            "headline": "Opening",
            "subline": "The setup"
        },
        "visualTreatment": "kinetic editorial title scene",
        "motion": "fast type-on with camera push",
        "safeZone": "keep title inside 10% margins",
        "avoid": "static text-only cards"
    })
}

fn sample_gpu_visual() -> serde_json::Value {
    serde_json::json!({
        "id": "shader-hook-bg",
        "kind": "hybrid_scene",
        "startSeconds": 0.0,
        "durationSeconds": 1.0,
        "qualityProfile": "hq-neon-wireframe-shader-v1",
        "sourceBeat": "open with a generated shader hook",
        "visualTreatment": "procedural gradient field with a rotating cube",
        "motion": "slow shader drift with cube rotation",
        "safeZone": "center stays low contrast",
        "avoid": "strobing and tiny high-frequency noise",
        "shader": {
            "language": "glsl",
            "fragmentSource": "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, progress, 1.0); }"
        },
        "primitives": [
            {
                "id": "main-cube",
                "type": "cube",
                "material": { "color": "#63e6be" },
                "motion": { "orbit": true }
            }
        ]
    })
}

fn tiny_source_codex_report() -> CodexProposalReport {
    let mut proposal = sample_codex_proposal();
    proposal.clips = vec![CodexProposalClip {
        media_id: "media-1".to_string(),
        source_in: 0.4,
        source_out: 2.4,
        reason: "short hook from tiny generated source".to_string(),
    }];
    proposal.captions = Vec::new();
    proposal.overlays = Vec::new();
    proposal.hyperframes = Vec::new();
    proposal.render_review.duration_seconds = 2.0;

    CodexProposalReport {
        generated_at: "2026-06-17T00:00:00Z".to_string(),
        proposal,
    }
}

fn tiny_source_codex_report_with_graphics() -> CodexProposalReport {
    let mut report = tiny_source_codex_report();
    report.proposal.captions = vec![serde_json::json!({
        "text": "Rust render",
        "startSeconds": 0.3,
        "durationSeconds": 1.2,
        "sourceBeat": "show a generated caption over the selected edit",
        "visualTreatment": "compact caption with translucent backing",
        "motion": "quick fade in",
        "safeZone": "keep text inside lower safe zone",
        "avoid": "full-width opaque black caption slabs"
    })];
    report.proposal.overlays = vec![serde_json::json!({
        "kind": "lower_third",
        "startSeconds": 0.8,
        "durationSeconds": 0.9,
        "fields": {
            "headline": "Graphics",
            "subline": "paths"
        },
        "brief": "Identify generated graphics artifacts.",
        "sourceBeat": "prove graphics are rendered and reported",
        "visualTreatment": "small lower-third chip",
        "motion": "slide in and hold",
        "safeZone": "keep inside 10% margins",
        "avoid": "large opaque panels"
    })];
    report
}

fn generate_tiny_source_video(path: &Path) {
    #[cfg(feature = "ges-render")]
    {
        generate_fixture_source_with_gstreamer(path, 160, 90, 15.0, 3.0, Duration::from_secs(30))
            .expect("fixture pipeline should render");
    }

    #[cfg(not(feature = "ges-render"))]
    {
        panic!(
            "GStreamer fixture generation requires the ges-render feature: {}",
            path.display()
        );
    }
}

struct FakeCodexE2eClient {
    requests: RefCell<Vec<serde_json::Value>>,
    assistant_text: String,
    was_invoked: Cell<bool>,
}

impl FakeCodexE2eClient {
    fn with_assistant_text(assistant_text: String) -> Self {
        Self {
            requests: RefCell::new(Vec::new()),
            assistant_text,
            was_invoked: Cell::new(false),
        }
    }
}

impl CodexE2eAppServerClient for FakeCodexE2eClient {
    fn initialize(&mut self, request: serde_json::Value) -> PipelineResult<serde_json::Value> {
        self.was_invoked.set(true);
        self.requests.borrow_mut().push(request);
        Ok(serde_json::json!({}))
    }

    fn start_thread(&mut self, request: serde_json::Value) -> PipelineResult<serde_json::Value> {
        self.was_invoked.set(true);
        self.requests.borrow_mut().push(request);
        Ok(serde_json::json!({ "thread": { "id": "thread-e2e" } }))
    }

    fn start_turn(
        &mut self,
        request: serde_json::Value,
        _thread_id: &str,
    ) -> PipelineResult<CodexE2eTurnOutput> {
        self.was_invoked.set(true);
        self.requests.borrow_mut().push(request);
        Ok(CodexE2eTurnOutput {
            turn_response: serde_json::json!({ "turn": { "id": "turn-e2e" } }),
            assistant_text: self.assistant_text.clone(),
        })
    }
}

fn codex_e2e_config_for_report(report_path: &Path) -> CodexE2eConfig {
    CodexE2eConfig {
        project_root: repo_root(),
        report_path: report_path.to_path_buf(),
        codex_binary: "fake-codex".to_string(),
    }
}

fn codex_tool_acceptance() -> CodexE2eToolAcceptance {
    CodexE2eToolAcceptance {
        project_context: true,
        inspect_timeline: true,
        validate_proposal: true,
        apply_project_actions: true,
        build_codex_edit_start_request: true,
        build_render_start_request: true,
        build_export_start_request: true,
    }
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn render_proposal_bin_path() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_video-creater-render-proposal")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/video-creater-render-proposal")
        })
}

fn valid_codex_e2e_proposal() -> CodexEditProposal {
    CodexEditProposal {
        media_id: "codex-e2e-media".to_string(),
        clips: vec![
            CodexProposalClip {
                media_id: "codex-e2e-media".to_string(),
                source_in: 4.0,
                source_out: 21.0,
                reason: "opening hook with clear setup".to_string(),
            },
            CodexProposalClip {
                media_id: "codex-e2e-media".to_string(),
                source_in: 38.0,
                source_out: 58.0,
                reason: "main payoff and product result".to_string(),
            },
        ],
        captions: vec![serde_json::json!({
            "text": "Cut from hook to payoff",
            "startSeconds": 1.0,
            "durationSeconds": 1.8,
            "visualTreatment": "large phone-readable caption with translucent backing",
            "motion": "quick scale pop with underline wipe",
            "safeZone": "inside lower third and 10% side margins",
            "avoid": "full-width opaque black caption slabs"
        })],
        overlays: Vec::new(),
        hyperframes: Vec::new(),
        gpu_visuals: Vec::new(),
        project_actions: Vec::new(),
        render_review: CodexRenderReview {
            duration_seconds: 37.0,
            stream_check_required: true,
            caption_alignment_required: true,
            overlay_timing_required: true,
            visual_frame_evidence_required: true,
            artifact_paths_required: true,
            log_reference_required: true,
        },
    }
}

#[cfg(target_os = "linux")]
#[test]
fn plugin_policy_accepts_distribution_vendor_suffix_on_reviewed_packages() {
    let factory = GstFactoryInfo::new("videoconvert")
        .plugin_name("videoconvertscale")
        .package("GStreamer Base Plug-ins (Ubuntu)")
        .license("LGPL");

    assert_eq!(
        evaluate_gstreamer_factory(&factory).verdict,
        PluginPolicyVerdict::Allowed
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_plugin_policy_allows_only_the_bundled_lgpl_ffmpeg_codecs() {
    let bundled = |name: &str| {
        GstFactoryInfo::new(name)
            .plugin_name("libav")
            .package("Video Creater LGPL FFmpeg Plug-ins")
            .license("LGPL")
    };
    for name in [
        "avdec_h264",
        "avdec_h265",
        "avdec_aac",
        "avdec_prores",
        "avenc_aac",
        "avenc_prores_ks",
    ] {
        assert_eq!(
            evaluate_gstreamer_factory(&bundled(name)).verdict,
            PluginPolicyVerdict::Allowed,
            "{name}"
        );
    }
    for factory in [
        GstFactoryInfo::new("avdec_h264")
            .plugin_name("libav")
            .package("GStreamer FFMPEG Plug-ins (Ubuntu)")
            .license("LGPL"),
        bundled("avenc_mpeg2video"),
        bundled("avdec_h264").license("GPL"),
        GstFactoryInfo::new("x264enc")
            .plugin_name("x264")
            .package("GStreamer Ugly Plug-ins (Ubuntu)")
            .license("GPL"),
    ] {
        assert_eq!(
            evaluate_gstreamer_factory(&factory).verdict,
            PluginPolicyVerdict::Denied,
            "{factory:?}"
        );
    }
    let openh264 = GstFactoryInfo::new("openh264enc")
        .plugin_name("openh264")
        .package("GStreamer Bad Plug-ins (Ubuntu)")
        .license("BSD");
    assert_eq!(
        evaluate_gstreamer_factory(&openh264).verdict,
        PluginPolicyVerdict::Allowed
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_delivery_profiles_resolve_to_reviewed_linux_codecs() {
    let h264 = gstreamer_output_profile_target(ExportProfile::Mp4H264).expect("mp4 h264 target");
    assert_eq!(
        h264.required_factories(),
        vec![
            "mp4mux",
            "openh264enc",
            "avenc_aac",
            "h264parse",
            "aacparse"
        ]
    );
    let hevc = gstreamer_output_profile_target(ExportProfile::Mp4H265).expect("mp4 h265 target");
    assert_eq!(hevc.video_factory, "vah265enc");
    let prores =
        gstreamer_output_profile_target(ExportProfile::ProResMov).expect("prores mov target");
    assert_eq!(
        prores.required_factories(),
        vec!["qtmux", "avenc_prores_ks"]
    );
    assert_eq!(prores.audio_mode, GstreamerAudioMode::RawPcm);
    for target in [h264, hevc, prores] {
        assert!(!target.macos_only);
        for factory in target.required_factories() {
            assert!(
                !factory.starts_with("vt") && !factory.starts_with("at"),
                "{factory}"
            );
        }
    }
}
