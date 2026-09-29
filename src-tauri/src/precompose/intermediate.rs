use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
#[cfg(feature = "ges-render")]
use crate::render_runtime::render_runtime_environment;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PngSequenceSpec {
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
}

#[cfg(feature = "ges-render")]
use gstreamer as gst;
#[cfg(feature = "ges-render")]
use gstreamer::prelude::*;
#[cfg(feature = "ges-render")]
use gstreamer_app as gst_app;

#[cfg(feature = "ges-render")]
pub fn package_png_frames_as_mov(
    frames_dir: &Path,
    frame_count: u32,
    sequence: PngSequenceSpec,
    output_path: &Path,
    timeout: Duration,
) -> PipelineResult<()> {
    let PngSequenceSpec {
        width,
        height,
        fps_numerator,
        fps_denominator,
    } = sequence;
    render_runtime_environment().map_err(|error| gst_error("runtime", error))?;
    let caps = gst::Caps::builder("image/png")
        .field("parsed", true)
        .field("width", width as i32)
        .field("height", height as i32)
        .field(
            "framerate",
            gst::Fraction::new(fps_numerator as i32, fps_denominator as i32),
        )
        .build();
    let appsrc = gst_app::AppSrc::builder()
        .name("precompose-png-source")
        .caps(&caps)
        .format(gst::Format::Time)
        .is_live(false)
        .block(true)
        .build();
    let muxer = gst::ElementFactory::make("qtmux")
        .name("precompose-qtmux")
        .build()
        .map_err(|error| gst_error("qtmux", error))?;
    let sink = gst::ElementFactory::make("filesink")
        .name("precompose-file-sink")
        .property("location", output_path.to_string_lossy().as_ref())
        .build()
        .map_err(|error| gst_error("filesink", error))?;
    let pipeline = gst::Pipeline::with_name("video-creater-precompose-intermediate");
    pipeline
        .add_many([appsrc.upcast_ref(), &muxer, &sink])
        .map_err(|error| gst_error("pipeline.add", error))?;
    let source_pad = appsrc
        .static_pad("src")
        .ok_or_else(|| gst_missing("appsrc.srcPad"))?;
    let video_pad = muxer
        .request_pad_simple("video_%u")
        .ok_or_else(|| gst_missing("qtmux.videoPad"))?;
    source_pad
        .link(&video_pad)
        .map_err(|error| gst_error("appsrc.link", error))?;
    muxer
        .link(&sink)
        .map_err(|error| gst_error("qtmux.link", error))?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| gst_error("pipeline.play", error))?;

    let frame_duration_ns =
        (1_000_000_000_u128 * u128::from(fps_denominator) / u128::from(fps_numerator)) as u64;
    for frame_index in 0..frame_count {
        let path = frames_dir.join(format!("frame-{frame_index:06}.png"));
        let bytes = std::fs::read(&path).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "precompose.intermediate.frame",
                "A precomposed PNG frame could not be read.",
                "Rebuild the precompose cache entry and retry.",
            )
            .with_detail("framePath", path.display().to_string())
            .with_detail("error", error.to_string())]
        })?;
        let mut buffer = gst::Buffer::from_mut_slice(bytes);
        {
            let buffer = buffer.get_mut().expect("new GStreamer buffer is writable");
            buffer.set_pts(gst::ClockTime::from_nseconds(
                frame_duration_ns.saturating_mul(u64::from(frame_index)),
            ));
            buffer.set_duration(gst::ClockTime::from_nseconds(frame_duration_ns));
        }
        appsrc
            .push_buffer(buffer)
            .map_err(|error| gst_error("appsrc.push", error))?;
    }
    appsrc
        .end_of_stream()
        .map_err(|error| gst_error("appsrc.eos", error))?;

    let bus = pipeline.bus().ok_or_else(|| gst_missing("pipeline.bus"))?;
    let deadline = std::time::Instant::now() + timeout;
    let result = loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "precompose.intermediate.timeout",
                "PNG-in-QuickTime packaging timed out.",
                "Inspect qtmux availability and the generated PNG frames.",
            )]);
        }
        let timeout =
            gst::ClockTime::from_nseconds(remaining.as_nanos().min(u64::MAX as u128) as u64);
        let Some(message) = bus.timed_pop(timeout) else {
            continue;
        };
        match message.view() {
            gst::MessageView::Eos(..) => break Ok(()),
            gst::MessageView::Error(error) => {
                break Err(vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendFailed,
                    "precompose.intermediate.gstreamer",
                    "GStreamer could not package PNG frames as a QuickTime intermediate.",
                    "Use the reviewed appsrc, qtmux, and filesink runtime factories.",
                )
                .with_detail("error", error.error().to_string())
                .with_detail("debug", error.debug().unwrap_or_default().to_string())]);
            }
            _ => {}
        }
    };
    let _ = pipeline.set_state(gst::State::Null);
    result?;
    if std::fs::metadata(output_path).map_or(true, |metadata| metadata.len() == 0) {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            "precompose.intermediate.output",
            "PNG-in-QuickTime packaging produced no media.",
            "Inspect qtmux output and rebuild the cache entry.",
        )]);
    }
    Ok(())
}

#[cfg(not(feature = "ges-render"))]
pub fn package_png_frames_as_mov(
    _frames_dir: &Path,
    _frame_count: u32,
    _sequence: PngSequenceSpec,
    _output_path: &Path,
    _timeout: Duration,
) -> PipelineResult<()> {
    Err(vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.intermediate",
        "PNG-in-QuickTime packaging requires the GES render feature.",
        "Build the app with ges-render enabled.",
    )])
}

#[cfg(feature = "ges-render")]
fn gst_error(path: &str, error: impl std::fmt::Display) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.intermediate.{path}"),
        "GStreamer intermediate packaging failed.",
        "Inspect the reviewed GStreamer runtime and retry.",
    )
    .with_detail("error", error.to_string())]
}

#[cfg(feature = "ges-render")]
fn gst_missing(path: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.intermediate.{path}"),
        "GStreamer intermediate packaging object is unavailable.",
        "Inspect the reviewed GStreamer runtime and retry.",
    )]
}

#[cfg(all(test, feature = "ges-render"))]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    #[cfg_attr(
        target_os = "macos",
        ignore = "covered by precompose_alpha_ges; PNG-in-QuickTime packaging and alpha decode require the AppKit-hosted GStreamer path on macOS"
    )]
    fn packages_timestamped_png_frames_without_png_parser() {
        crate::render_runtime::start_render_process_runtime()
            .expect("start shared render runtime before precompose packaging");
        let temporary = tempfile::tempdir().expect("temporary directory");
        let frames = temporary.path().join("frames");
        std::fs::create_dir(&frames).expect("frames directory");
        for index in 0..3 {
            let mut image = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 0]));
            image.put_pixel(8 + index * 4, 16, Rgba([255, 32, 16, 128]));
            image
                .save(frames.join(format!("frame-{index:06}.png")))
                .expect("write PNG frame");
        }
        let output = temporary.path().join("intermediate.mov");

        package_png_frames_as_mov(
            &frames,
            3,
            PngSequenceSpec {
                width: 32,
                height: 32,
                fps_numerator: 2,
                fps_denominator: 1,
            },
            &output,
            Duration::from_secs(10),
        )
        .expect("package PNG-in-QuickTime");

        assert!(std::fs::metadata(output).expect("MOV metadata").len() > 0);
        let mut decoded = Vec::new();
        crate::precompose::decode_video_frames_rgba(
            &temporary.path().join("intermediate.mov"),
            crate::precompose::FrameDecodeSpec {
                width: 32,
                height: 32,
                fps_numerator: 2,
                fps_denominator: 1,
                source_start_micros: 0,
                source_stop_micros: 500_000,
                playback_rate_micros: 1_000_000,
                frame_count: 1,
            },
            Duration::from_secs(10),
            |_, frame| {
                decoded.extend_from_slice(frame);
                Ok(())
            },
        )
        .expect("decode intermediate with bundled GStreamer");
        let alpha_index = ((16 * 32 + 8) * 4 + 3) as usize;
        assert_eq!(decoded[alpha_index], 128);
    }
}
