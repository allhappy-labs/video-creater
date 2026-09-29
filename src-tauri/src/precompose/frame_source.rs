use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
#[cfg(feature = "ges-render")]
use crate::render_pipeline::gstreamer_backend::require_allowed_factories;
#[cfg(feature = "ges-render")]
use crate::render_pipeline::plugin_policy::{policy_error_for_factory, GstFactoryInfo};
#[cfg(feature = "ges-render")]
use crate::render_runtime::render_runtime_environment;
use std::path::Path;
#[cfg(feature = "ges-render")]
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(feature = "ges-render")]
use std::time::Instant;

#[cfg(feature = "ges-render")]
use gst_video::VideoFrameExt;
#[cfg(feature = "ges-render")]
use gstreamer as gst;
#[cfg(feature = "ges-render")]
use gstreamer::prelude::*;
#[cfg(feature = "ges-render")]
use gstreamer_app as gst_app;
#[cfg(feature = "ges-render")]
use gstreamer_video as gst_video;

#[derive(Debug, Clone, Copy)]
pub struct FrameDecodeSpec {
    pub width: u32,
    pub height: u32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
    pub source_start_micros: u64,
    pub source_stop_micros: u64,
    pub playback_rate_micros: u32,
    pub frame_count: u32,
}

#[cfg(feature = "ges-render")]
pub fn decode_video_frames_rgba(
    source: &Path,
    spec: FrameDecodeSpec,
    timeout: Duration,
    mut receive: impl FnMut(u32, &[u8]) -> PipelineResult<()>,
) -> PipelineResult<()> {
    validate_spec(spec)?;
    require_allowed_factories(&[
        "filesrc",
        "decodebin",
        "videoconvert",
        "videoscale",
        "videorate",
        "capsfilter",
        "appsink",
    ])?;
    #[cfg(target_os = "macos")]
    require_allowed_factories(&["gldownload"])?;
    render_runtime_environment().map_err(|error| decode_error("runtime", error))?;
    prefer_reviewed_native_decoders();

    let source_element = gst::ElementFactory::make("filesrc")
        .property("location", source.to_string_lossy().as_ref())
        .build()
        .map_err(|error| decode_error("filesrc", error))?;
    let decodebin = gst::ElementFactory::make("decodebin")
        .build()
        .map_err(|error| decode_error("decodebin", error))?;
    let convert = gst::ElementFactory::make("videoconvert")
        .build()
        .map_err(|error| decode_error("videoconvert", error))?;
    #[cfg(target_os = "macos")]
    let gl_download = Some(
        gst::ElementFactory::make("gldownload")
            .build()
            .map_err(|error| decode_error("gldownload", error))?,
    );
    #[cfg(not(target_os = "macos"))]
    let gl_download: Option<gst::Element> = None;
    let system_memory_caps = gst::Caps::builder("video/x-raw").build();
    let system_memory_filter = gst::ElementFactory::make("capsfilter")
        .property("caps", &system_memory_caps)
        .build()
        .map_err(|error| decode_error("systemMemoryCapsfilter", error))?;
    let scale = gst::ElementFactory::make("videoscale")
        .build()
        .map_err(|error| decode_error("videoscale", error))?;
    let rate = gst::ElementFactory::make("videorate")
        .build()
        .map_err(|error| decode_error("videorate", error))?;
    let source_fps_numerator = u64::from(spec.fps_numerator) * 1_000_000;
    let source_fps_denominator =
        u64::from(spec.fps_denominator) * u64::from(spec.playback_rate_micros);
    let divisor = gcd_u64(source_fps_numerator, source_fps_denominator);
    let source_fps_numerator = source_fps_numerator / divisor;
    let source_fps_denominator = source_fps_denominator / divisor;
    if source_fps_numerator > i32::MAX as u64 || source_fps_denominator > i32::MAX as u64 {
        return Err(invalid_spec(
            "Decoded source frame rate is outside the GStreamer rational range.",
        ));
    }
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("colorimetry", "sRGB")
        .field("width", spec.width as i32)
        .field("height", spec.height as i32)
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .field(
            "framerate",
            gst::Fraction::new(source_fps_numerator as i32, source_fps_denominator as i32),
        )
        .build();
    let caps_filter = gst::ElementFactory::make("capsfilter")
        .property("caps", &caps)
        .build()
        .map_err(|error| decode_error("capsfilter", error))?;
    let app_sink = gst::ElementFactory::make("appsink")
        .property("sync", false)
        .property("max-buffers", 2u32)
        .property("drop", false)
        .build()
        .map_err(|error| decode_error("appsink", error))?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| invalid_spec("GStreamer appsink could not be constructed."))?;

    let pipeline = gst::Pipeline::with_name("video-creater-precompose-frame-source");
    let dynamic_policy_errors = Arc::new(Mutex::new(Vec::new()));
    let pad_link_errors = Arc::new(Mutex::new(Vec::new()));
    let dynamic_policy_errors_for_signal = Arc::clone(&dynamic_policy_errors);
    pipeline.connect_deep_element_added(move |_, _, element| {
        let Some(factory) = element.factory() else {
            return;
        };
        let class = factory.metadata("klass").unwrap_or_default();
        if !class.contains("Decoder") && !class.contains("Demuxer") {
            return;
        }
        let plugin = factory.plugin();
        let info = GstFactoryInfo::new(factory.name().to_string())
            .plugin_name(
                factory
                    .plugin_name()
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
            )
            .package(
                plugin
                    .as_ref()
                    .map(|plugin| plugin.package().to_string())
                    .unwrap_or_default(),
            )
            .license(
                plugin
                    .as_ref()
                    .map(|plugin| plugin.license().to_string())
                    .unwrap_or_default(),
            );
        if let Some(error) = policy_error_for_factory(&info) {
            dynamic_policy_errors_for_signal
                .lock()
                .expect("dynamic decoder policy lock")
                .push(error);
        }
    });
    pipeline
        .add_many([
            &source_element,
            &decodebin,
            &convert,
            &system_memory_filter,
            &scale,
            &rate,
            &caps_filter,
            app_sink.upcast_ref(),
        ])
        .map_err(|error| decode_error("pipeline.add", error))?;
    if let Some(gl_download) = gl_download.as_ref() {
        pipeline
            .add(gl_download)
            .map_err(|error| decode_error("pipeline.addGlDownload", error))?;
    }
    source_element
        .link(&decodebin)
        .map_err(|error| decode_error("source.link", error))?;
    if let Some(gl_download) = gl_download.as_ref() {
        gst::Element::link_many([
            gl_download,
            &system_memory_filter,
            &convert,
            &scale,
            &rate,
            &caps_filter,
            app_sink.upcast_ref(),
        ])
        .map_err(|error| decode_error("video.link", error))?;
    } else {
        gst::Element::link_many([
            &convert,
            &system_memory_filter,
            &scale,
            &rate,
            &caps_filter,
            app_sink.upcast_ref(),
        ])
        .map_err(|error| decode_error("video.link", error))?;
    }
    let decode_sink = gl_download
        .as_ref()
        .unwrap_or(&convert)
        .static_pad("sink")
        .ok_or_else(|| invalid_spec("GStreamer decode target has no sink pad."))?;
    let pad_link_errors_for_signal = Arc::clone(&pad_link_errors);
    decodebin.connect_pad_added(move |_, pad| {
        if decode_sink.is_linked() {
            return;
        }
        let is_video = pad
            .current_caps()
            .or_else(|| Some(pad.query_caps(None)))
            .and_then(|caps| {
                caps.structure(0)
                    .map(|structure| structure.name().to_string())
            })
            .is_some_and(|name| name.starts_with("video/"));
        if is_video {
            if let Err(error) = pad.link(&decode_sink) {
                pad_link_errors_for_signal
                    .lock()
                    .expect("frame source pad-link error lock")
                    .push(format!("{error:?} for {:?}", pad.current_caps()));
            }
        }
    });

    pipeline
        .set_state(gst::State::Paused)
        .map_err(|error| decode_error("pipeline.pause", error))?;
    let gst_timeout =
        gst::ClockTime::from_nseconds(timeout.as_nanos().min(u64::MAX as u128) as u64);
    if let Err(error) = pipeline.state(gst_timeout).0 {
        let link_errors = pad_link_errors
            .lock()
            .expect("frame source pad-link error lock")
            .join("; ");
        return Err(decode_error(
            "pipeline.preroll",
            if link_errors.is_empty() {
                error.to_string()
            } else {
                format!("{error}; dynamic pad link failed: {link_errors}")
            },
        ));
    }
    let policy_errors = dynamic_policy_errors
        .lock()
        .expect("dynamic decoder policy lock")
        .clone();
    if !policy_errors.is_empty() {
        let _ = pipeline.set_state(gst::State::Null);
        return Err(policy_errors);
    }
    pipeline
        .seek(
            1.0,
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::SeekType::Set,
            gst::ClockTime::from_useconds(spec.source_start_micros),
            gst::SeekType::Set,
            gst::ClockTime::from_useconds(spec.source_stop_micros),
        )
        .map_err(|error| decode_error("pipeline.seek", error))?;
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|error| decode_error("pipeline.play", error))?;

    let deadline = Instant::now() + timeout;
    let expected_bytes = usize::try_from(spec.width)
        .ok()
        .and_then(|width| {
            usize::try_from(spec.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| invalid_spec("Decoded RGBA frame dimensions overflowed."))?;
    let result = (|| {
        for frame_index in 0..spec.frame_count {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(decode_timeout(frame_index));
            }
            let sample = app_sink
                .try_pull_sample(gst::ClockTime::from_nseconds(
                    remaining.as_nanos().min(u64::MAX as u128) as u64,
                ))
                .ok_or_else(|| {
                    if app_sink.is_eos() {
                        invalid_spec(&format!(
                            "Decoded source ended after {frame_index} of {} required frames.",
                            spec.frame_count
                        ))
                    } else {
                        decode_timeout(frame_index)
                    }
                })?;
            let buffer = sample
                .buffer()
                .ok_or_else(|| invalid_spec("Decoded GStreamer sample has no buffer."))?;
            let caps = sample
                .caps()
                .ok_or_else(|| invalid_spec("Decoded GStreamer sample has no video caps."))?;
            let info = gst_video::VideoInfo::from_caps(caps)
                .map_err(|error| decode_error("videoInfo", error))?;
            let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
                .map_err(|error| decode_error("videoFrame.map", error))?;
            let source = frame
                .plane_data(0)
                .map_err(|error| decode_error("videoFrame.plane", error))?;
            let stride = frame.plane_stride()[0];
            let row_bytes = usize::try_from(spec.width)
                .ok()
                .and_then(|width| width.checked_mul(4))
                .ok_or_else(|| invalid_spec("Decoded RGBA row dimensions overflowed."))?;
            if stride < 0 || (stride as usize) < row_bytes {
                return Err(invalid_spec(&format!(
                    "Decoded RGBA frame has unsupported row stride {stride}; expected at least {row_bytes}."
                )));
            }
            let stride = stride as usize;
            let mut packed = vec![0_u8; expected_bytes];
            for row in 0..spec.height as usize {
                let source_start = row * stride;
                let output_start = row * row_bytes;
                let source_end = source_start + row_bytes;
                if source_end > source.len() {
                    return Err(invalid_spec(
                        "Decoded RGBA plane is shorter than its declared stride.",
                    ));
                }
                packed[output_start..output_start + row_bytes]
                    .copy_from_slice(&source[source_start..source_end]);
            }
            receive(frame_index, &packed)?;
        }
        Ok(())
    })();
    let _ = pipeline.set_state(gst::State::Null);
    result
}

#[cfg(feature = "ges-render")]
fn prefer_reviewed_native_decoders() {
    if let Some(factory) = gst::ElementFactory::find("openh264dec") {
        // OpenH264 decodes only the constrained baseline profile; the bundled LGPL FFmpeg
        // decoder handles every H.264 profile on Linux.
        factory.set_rank(gst::Rank::NONE);
    }
    if let Some(factory) = gst::ElementFactory::find("vtdec") {
        factory.set_rank(gst::Rank::PRIMARY + 100);
    }
    if let Some(factory) = gst::ElementFactory::find("vtdec_hw") {
        // Frame extraction terminates in a CPU RGBA appsink. The hardware-only
        // decoder prefers GLMemory once another AppKit pipeline has established
        // a GL context, which cannot link to the explicit CPU videoconvert.
        factory.set_rank(gst::Rank::NONE);
    }
}

#[cfg(not(feature = "ges-render"))]
pub fn decode_video_frames_rgba(
    _source: &Path,
    _spec: FrameDecodeSpec,
    _timeout: Duration,
    _receive: impl FnMut(u32, &[u8]) -> PipelineResult<()>,
) -> PipelineResult<()> {
    Err(invalid_spec(
        "Frame decoding requires the GES render feature.",
    ))
}

#[cfg(feature = "ges-render")]
fn validate_spec(spec: FrameDecodeSpec) -> PipelineResult<()> {
    if spec.width == 0
        || spec.height == 0
        || spec.fps_numerator == 0
        || spec.fps_denominator == 0
        || spec.source_stop_micros <= spec.source_start_micros
        || spec.playback_rate_micros < 100_000
        || spec.playback_rate_micros > 8_000_000
        || spec.frame_count == 0
    {
        return Err(invalid_spec("Frame decode specification is invalid."));
    }
    Ok(())
}

#[cfg(feature = "ges-render")]
fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.max(1)
}

fn invalid_spec(message: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.frameSource",
        message,
        "Use a readable visual source and a finite deterministic frame range.",
    )]
}

#[cfg(feature = "ges-render")]
fn decode_error(path: &str, error: impl std::fmt::Display) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.frameSource.{path}"),
        "GStreamer could not decode the prepared-source frame stream.",
        "Inspect the approved GStreamer decode/base/good plugin set and source media.",
    )
    .with_detail("error", error.to_string())]
}

#[cfg(feature = "ges-render")]
fn decode_timeout(frame_index: u32) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.frameSource.timeout",
        "Frame decoding timed out.",
        "Inspect the source media and precompose time budget.",
    )
    .with_detail("frameIndex", frame_index.to_string())]
}
