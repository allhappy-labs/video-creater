#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(target_os = "macos")]
use video_creater_lib::media_inspection::{
    derive_media_analysis_moments_with_sampled_signals, inspect_video,
    sample_video_audio_window_metrics, sample_video_frame_metrics,
};
#[cfg(target_os = "macos")]
use video_creater_lib::render_runtime::start_render_process_runtime;

#[cfg(target_os = "macos")]
fn main() {
    if std::env::var_os("VIDEO_CREATER_HEADLESS_RUST_SUITE").is_some() {
        println!("media_inspection_appkit: deferred to dedicated native lane");
        return;
    }
    let exit_code = gstreamer::macos_main(|| match run_fixture() {
        Ok(()) => {
            println!("media_inspection_appkit: passed");
            0
        }
        Err(error) => {
            eprintln!("media_inspection_appkit: {error}");
            1
        }
    });
    std::process::exit(exit_code);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("media_inspection_appkit: skipped outside macOS");
}

#[cfg(target_os = "macos")]
fn run_fixture() -> Result<(), String> {
    // Frame sampling can instantiate GStreamer GL elements on macOS, so this
    // integration fixture intentionally runs in GStreamer's AppKit harness.
    start_render_process_runtime().map_err(|error| error.to_string())?;
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/media/edison-speech-1920s-30s.mp4");
    let report = inspect_video(&fixture_path)
        .map_err(|errors| format!("inspect fixture video: {errors:?}"))?;

    if report.media.video.is_none() {
        return Err("fixture should have video".to_string());
    }
    if report.media.audio.is_none() {
        return Err("fixture should have audio".to_string());
    }

    let sampled_frames = sample_video_frame_metrics(&fixture_path, report.media.duration_seconds);
    let sampled_audio =
        sample_video_audio_window_metrics(&fixture_path, report.media.duration_seconds);
    let moments = derive_media_analysis_moments_with_sampled_signals(
        "edison-fixture",
        &report,
        None,
        &sampled_frames,
        &sampled_audio,
    );

    if sampled_frames.len() < 2 {
        return Err(format!(
            "expected real fixture frame sampling to produce comparable frames, got {sampled_frames:?}"
        ));
    }
    if sampled_audio.len() < 2 {
        return Err(format!(
            "expected real fixture audio sampling to produce comparable windows, got {sampled_audio:?}"
        ));
    }
    if moments.is_empty() {
        return Err("expected fixture to produce EDL-ready moments".to_string());
    }
    if !moments.iter().any(|moment| {
        moment
            .label
            .contains("sampled-frame visual change analysis")
    }) {
        return Err("fixture did not produce a sampled-frame analysis moment".to_string());
    }
    if !moments.iter().any(|moment| {
        moment
            .label
            .contains("sampled-audio loudness/change analysis")
    }) {
        return Err("fixture did not produce a sampled-audio analysis moment".to_string());
    }

    Ok(())
}
