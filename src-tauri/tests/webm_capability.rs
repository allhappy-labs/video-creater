use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::ExportProfile;
use video_creater_lib::render_pipeline::output_profile::{
    gstreamer_output_profile_target, GstreamerAudioMode,
};
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::workflows::{
    temporal_export_media_start_request, temporal_export_media_workflow_activity_plan_value,
};

#[test]
fn webm_profile_resolves_to_the_factories_used_by_the_renderer() {
    let target = gstreamer_output_profile_target(ExportProfile::Webm)
        .expect("WebM should expose its GStreamer capability target");

    assert_eq!(target.extension, "webm");
    assert_eq!(target.mime_type, "video/webm");
    assert_eq!(target.container_factory, "webmmux");
    assert_eq!(target.container_caps, "video/webm");
    assert_eq!(target.video_factory, "vp8enc");
    assert_eq!(target.video_caps, "video/x-vp8");
    assert_eq!(target.audio_factory, Some("opusenc"));
    assert_eq!(target.audio_caps, Some("audio/x-opus"));
    assert_eq!(target.audio_mode, GstreamerAudioMode::Opus);
    assert_eq!(
        target.required_factories(),
        vec!["webmmux", "vp8enc", "opusenc"]
    );
}

#[test]
fn explicit_temporal_webm_export_passes_the_capability_guard() {
    start_render_process_runtime().expect("initialize curated render runtime");
    let request = temporal_export_media_start_request(
        "Project A",
        "/tmp/video-creater/Project A",
        "webm-export-1",
        ExportRenderOptions::new(ExportProfile::Webm, RenderQuality::Draft, 1280, 720)
            .expect("explicit WebM export options"),
        "exports/project-a.webm",
    );

    let plan = temporal_export_media_workflow_activity_plan_value(
        request.input,
        "2026-07-14T12:00:00Z",
        Some("temporal-run-webm-1"),
    )
    .expect("WebM export workflow activity plan");

    assert_eq!(plan["profile"], serde_json::json!("webm"));
    assert_eq!(
        plan["validateExportProfileInput"]["validation"],
        serde_json::json!({
            "container": "webm",
            "extension": "webm",
            "mimeType": "video/webm",
            "videoCodec": "vp8",
            "audioCodec": "opus",
            "requireVideoStream": true,
            "requireAudioStreamWhenTimelineHasAudio": true
        })
    );
}
