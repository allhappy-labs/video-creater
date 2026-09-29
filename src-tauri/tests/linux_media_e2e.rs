//! Real-media Linux export and probe checks against the staged render runtime
//! (`pnpm build:linux-media-runtime`) and the compatibility worker binary.
#![cfg(all(target_os = "linux", feature = "ges-render"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;
use video_creater_lib::edit::render_plan::RenderQuality;
use video_creater_lib::project::export_options::ExportRenderOptions;
use video_creater_lib::project::export_profiles::{export_profile_availability, ExportProfile};
use video_creater_lib::project::fixtures::sample_project;
use video_creater_lib::project::model::{JobStatus, TrackKind};
use video_creater_lib::project::source_probe::{probe_source_metadata, SourceMediaType};
use video_creater_lib::project::split::{load_split_project, save_split_project};
use video_creater_lib::render_pipeline::gstreamer_backend::probe_media_with_gstreamer;
use video_creater_lib::render_pipeline::project_export::{
    render_media_to_split_project_folder, ProjectMediaRenderResult,
};
use video_creater_lib::render_runtime::start_render_process_runtime;
use video_creater_lib::workflows::{
    temporal_export_media_start_request_with_options,
    temporal_export_media_workflow_activity_plan_value, temporal_job_summary,
    temporal_validate_rendered_media_activity_input_value,
    temporal_validate_rendered_media_activity_value, TemporalWorkflowKind,
};

const FIXTURE: &str = "tests/fixtures/media/edison-speech-1920s-30s.mp4";

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn prepare_runtime() {
    static START: Once = Once::new();
    START.call_once(|| {
        let worker = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/debug/video-creater-compatibility-decoder");
        assert!(
            worker.is_file(),
            "build the worker first: cargo build -p video-creater-compatibility-decoder"
        );
        std::env::set_var("VIDEO_CREATER_COMPATIBILITY_DECODER", worker);
        start_render_process_runtime().expect("staged Linux render runtime starts");
    });
}

#[test]
fn linux_probe_reads_h264_aac_fixture_with_bundled_codecs() {
    prepare_runtime();
    let metadata = probe_source_metadata(&manifest_path(FIXTURE)).expect("probe H.264/AAC MP4");

    assert_eq!(metadata.media_type, SourceMediaType::Video);
    assert!(metadata.width.is_some_and(|width| width > 0));
    let duration = metadata.duration_seconds.expect("duration");
    assert!((duration - 30.0).abs() < 1.0, "duration {duration}");
    assert!(metadata.fps.is_some_and(|fps| fps > 0.0));
}

fn export(profile: ExportProfile, quality: RenderQuality, extension: &str) -> PathBuf {
    let (dir, result) = export_with_result(profile, quality, extension);
    dir.join(result.output_path)
}

/// Renders the audio-free sample timeline and returns the project folder with
/// the full render result, so tests can inspect the report the export wrote.
fn export_with_result(
    profile: ExportProfile,
    quality: RenderQuality,
    extension: &str,
) -> (PathBuf, ProjectMediaRenderResult) {
    prepare_runtime();
    let availability = export_profile_availability(profile);
    assert!(
        availability.available,
        "{profile:?} unavailable: {:?}",
        availability.unavailable_reason
    );
    let dir = tempfile::tempdir().expect("project dir").keep();
    fs::create_dir_all(dir.join("media")).expect("media dir");
    fs::copy(manifest_path(FIXTURE), dir.join("media/input.mp4")).expect("copy fixture");
    let mut project = sample_project();
    project.media[0].relative_path = "media/input.mp4".to_string();
    project.media[0].duration_seconds = 30.0;
    project.media[0].width = Some(320);
    project.media[0].height = Some(180);
    project.media[0].fps = Some(24.0);
    project.render_settings.width = 320;
    project.render_settings.height = 180;
    project.render_settings.fps = 24.0;
    project.timeline.duration_seconds = 3.0;
    let item = &mut project.timeline.tracks[0].items[0];
    item.duration_seconds = 3.0;
    item.properties
        .insert("sourceIn".to_string(), serde_json::json!(2.0));
    item.properties
        .insert("sourceOut".to_string(), serde_json::json!(5.0));
    save_split_project(&dir, &project).expect("save project");
    let job_id = format!("linux-e2e-{extension}-{quality:?}").to_ascii_lowercase();

    let result = render_media_to_split_project_folder(
        &dir,
        &project.id,
        ExportRenderOptions::new(profile, quality, 320, 180).expect("export options"),
        temporal_job_summary(
            TemporalWorkflowKind::ExportMedia,
            &project.id,
            &job_id,
            JobStatus::Queued,
            "2026-09-13T00:00:00Z",
        ),
        "2026-09-13T00:00:00Z",
        None,
        None,
    )
    .unwrap_or_else(|errors| panic!("{profile:?} export failed: {errors:#?}"));

    assert_eq!(
        result.output_path,
        format!("renders/{job_id}/output.{extension}")
    );
    (dir, result)
}

fn assert_output_streams(path: &Path, codec_fragment: &str, audio_fragment: &str) {
    let (probe, _) = probe_media_with_gstreamer(path, Duration::from_secs(30), "linuxE2e")
        .expect("independent discoverer probe of export");
    let video = probe.video.expect("video stream");
    assert_eq!((video.width, video.height), (Some(320), Some(180)));
    assert!(
        video
            .codec_name
            .as_deref()
            .is_some_and(|codec| codec.to_ascii_lowercase().contains(codec_fragment)),
        "codec {:?}",
        video.codec_name
    );
    let audio_codec = probe
        .audio
        .and_then(|audio| audio.codec_name)
        .expect("audio stream");
    let audio_caps = discovered_audio_caps(path);
    assert!(
        audio_caps.contains(audio_fragment),
        "audio caps {audio_caps}"
    );
    if audio_fragment.contains("mpegversion=(int)4") {
        assert_eq!(audio_codec, "aac");
    }
    let duration = probe.duration_seconds.expect("duration");
    assert!((duration - 3.0).abs() < 0.25, "duration {duration}");
}

#[test]
fn linux_exports_mp4_h264_with_openh264_and_ffmpeg_aac() {
    let output = export(ExportProfile::Mp4H264, RenderQuality::Final, "mp4");
    assert_output_streams(&output, "h264", "mpegversion=(int)4");
}

#[test]
fn linux_exports_prores_mov_with_ffmpeg_prores() {
    let output = export(ExportProfile::ProResMov, RenderQuality::Final, "mov");
    assert_output_streams(&output, "prores", "audio/x-raw");
}

#[test]
fn linux_exports_webm() {
    let output = export(ExportProfile::Webm, RenderQuality::Draft, "webm");
    assert_output_streams(&output, "vp8", "audio/x-opus");
}

fn discovered_audio_caps(path: &Path) -> String {
    use gstreamer_pbutils::prelude::*;
    let uri = gstreamer::glib::filename_to_uri(path, None).expect("output uri");
    let discoverer = gstreamer_pbutils::Discoverer::new(gstreamer::ClockTime::from_seconds(30))
        .expect("discoverer");
    let info = discoverer.discover_uri(&uri).expect("discover output");
    info.audio_streams()
        .into_iter()
        .next()
        .and_then(|stream| stream.caps())
        .map(|caps| caps.to_string())
        .unwrap_or_default()
}

/// The sample timeline has no audio track, but GES renders an audio/video
/// timeline and the MP4 target always muxes an AAC stream. The render report
/// must describe the streams it wrote, and spell their codecs the way the
/// export profile's validation payload does, otherwise the Temporal
/// `ValidateRenderedMedia` activity rejects a perfectly good export.
#[test]
fn linux_mp4_export_without_timeline_audio_reports_the_streams_it_wrote() {
    let (dir, result) = export_with_result(ExportProfile::Mp4H264, RenderQuality::Final, "mp4");
    assert!(
        !project_timeline_has_audio_track(&dir),
        "the sample timeline must stay audio free for this test"
    );

    let (probe, _) = probe_media_with_gstreamer(
        &dir.join(&result.output_path),
        Duration::from_secs(30),
        "linuxE2e",
    )
    .expect("independent discoverer probe of export");
    let probed_audio_codec = probe
        .audio
        .as_ref()
        .and_then(|audio| audio.codec_name.clone())
        .expect("MP4 export writes an audio stream even without timeline audio");
    assert_eq!(probed_audio_codec, "aac");

    assert_eq!(
        result
            .render_report
            .streams
            .as_ref()
            .map(|streams| streams.audio),
        Some(true),
        "render report records the muxed audio stream"
    );
    assert_eq!(
        result.render_report.summary.audio_codec.as_deref(),
        Some("aac"),
        "render report summary must name the audio codec it actually wrote"
    );
    let probed_video_codec = probe
        .video
        .as_ref()
        .and_then(|video| video.codec_name.clone())
        .expect("MP4 export writes a video stream");
    // The discoverer names video streams by their caps, so the summary has to
    // carry the normalised codec name the validation payload uses.
    assert_eq!(probed_video_codec, "video/x-h264");
    assert_eq!(
        result.render_report.summary.video_codec.as_deref(),
        Some("h264"),
        "render report summary must name the video codec it actually wrote"
    );

    let start = temporal_export_media_start_request_with_options(
        "project-test",
        &dir.to_string_lossy(),
        "linux-e2e-mp4-final",
        ExportRenderOptions::new(ExportProfile::Mp4H264, RenderQuality::Final, 320, 180)
            .expect("export options"),
        "exports/video.mp4",
    );
    let validation = start
        .input
        .get("validation")
        .expect("export start request carries the validation payload");
    assert_eq!(
        validation["container"].as_str(),
        result.render_report.summary.container.as_deref(),
        "validation payload and render report must spell the container alike"
    );
    assert_eq!(
        validation["videoCodec"].as_str(),
        result.render_report.summary.video_codec.as_deref(),
        "validation payload and render report must spell the video codec alike"
    );
    assert_eq!(
        validation["audioCodec"].as_str(),
        result.render_report.summary.audio_codec.as_deref(),
        "validation payload and render report must spell the audio codec alike"
    );
    let plan = temporal_export_media_workflow_activity_plan_value(
        start.input.clone(),
        "2026-09-13T00:00:00Z",
        None,
    )
    .expect("export activity plan");
    let validate_input = plan
        .get("validateRenderedMediaInput")
        .cloned()
        .expect("validateRenderedMediaInput");
    let render_output = serde_json::json!({
        "status": "rendered",
        "renderReport": serde_json::to_value(&result.render_report).expect("render report json"),
        "projectRenderReport": serde_json::to_value(&result.project_render_report)
            .expect("project render report json"),
        "outputPath": result.output_path,
    });
    let input =
        temporal_validate_rendered_media_activity_input_value(render_output, Some(&validate_input))
            .expect("validation activity input");

    temporal_validate_rendered_media_activity_value(input)
        .expect("Temporal validation accepts an MP4 export of a timeline without audio");
}

fn project_timeline_has_audio_track(dir: &Path) -> bool {
    let project = load_split_project(dir).expect("load project");
    project
        .timeline
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Audio && track.enabled && !track.items.is_empty())
}
