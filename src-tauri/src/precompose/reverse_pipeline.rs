//! The GStreamer side of reversed video intermediates: the clip's source span
//! is decoded forwards through the reviewed `decode_video_frames_rgba`, and
//! each RGBA frame is encoded by `appsrc ! pngenc ! appsink` and written under
//! its reversed output index (`frame-{first_output_index - j:06}.png`).
//!
//! `pngenc` is 1:1 and keeps buffer order, so the `j`-th encoded buffer is the
//! `j`-th decoded frame. Decoding and encoding run on separate streaming
//! threads; `appsrc` blocks when a few frames are queued.

use super::frame_source::{decode_video_frames_rgba, FrameDecodeSpec};
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::render_pipeline::gstreamer_backend::require_allowed_factories;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// zlib level passed to `pngenc compression-level` (its default on 1.24.2).
pub(super) const PNG_COMPRESSION_LEVEL: u32 = 6;
const QUEUED_FRAMES: u64 = 4;

/// SHA-256 digests of the frames written, in decode order.
pub(super) struct EncodedFrames {
    pub(super) rgba_sha256: Vec<String>,
    pub(super) png_sha256: Vec<String>,
}

#[derive(Default)]
struct WriterState {
    written: u32,
    png_sha256: Vec<String>,
    error: Option<String>,
}

/// Decodes `spec` from `source` and writes decoded frame `j` as
/// `frames_dir/frame-{first_output_index - j:06}.png`.
pub(super) fn encode_reversed_frames(
    source: &Path,
    spec: FrameDecodeSpec,
    frames_dir: &Path,
    first_output_index: u32,
    timeout: Duration,
    is_cancelled: &dyn Fn() -> bool,
) -> PipelineResult<EncodedFrames> {
    if spec.frame_count == 0 || spec.frame_count > first_output_index.saturating_add(1) {
        return Err(writer_error(
            "The reversed frame range does not fit the intermediate.",
        ));
    }
    require_allowed_factories(&["appsrc", "pngenc", "appsink"])?;
    let writer = PngWriter::start(
        spec.width,
        spec.height,
        (spec.fps_numerator, spec.fps_denominator),
        frames_dir.to_path_buf(),
        first_output_index,
    )?;
    let deadline = Instant::now() + timeout;
    let mut rgba_sha256 = Vec::with_capacity(spec.frame_count as usize);
    decode_video_frames_rgba(source, spec, timeout, |index, rgba| {
        if is_cancelled() {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                "precompose.cancelled",
                "Render preparation was cancelled.",
                "Restart the render when preparation should continue.",
            )]);
        }
        rgba_sha256.push(format!("{:x}", Sha256::digest(rgba)));
        writer.push(index, rgba)
    })?;
    let png_sha256 = writer.finish(spec.frame_count, deadline)?;
    Ok(EncodedFrames {
        rgba_sha256,
        png_sha256,
    })
}

struct PngWriter {
    pipeline: gst::Pipeline,
    appsrc: gst_app::AppSrc,
    state: Arc<Mutex<WriterState>>,
    frame_duration_ns: u64,
}

impl PngWriter {
    fn start(
        width: u32,
        height: u32,
        (fps_numerator, fps_denominator): (u32, u32),
        frames_dir: PathBuf,
        first_output_index: u32,
    ) -> PipelineResult<Self> {
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .field("width", width as i32)
            .field("height", height as i32)
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .field(
                "framerate",
                gst::Fraction::new(fps_numerator as i32, fps_denominator as i32),
            )
            .build();
        let frame_bytes = u64::from(width) * u64::from(height) * 4;
        let appsrc = gst_app::AppSrc::builder()
            .name("reverse-rgba-source")
            .caps(&caps)
            .format(gst::Format::Time)
            .is_live(false)
            .block(true)
            .max_bytes(frame_bytes * QUEUED_FRAMES)
            .build();
        let encoder = gst::ElementFactory::make("pngenc")
            .name("reverse-pngenc")
            .property("compression-level", PNG_COMPRESSION_LEVEL)
            .property("snapshot", false)
            .build()
            .map_err(|error| gst_error("pngenc", error))?;
        let state = Arc::new(Mutex::new(WriterState::default()));
        let callback_state = Arc::clone(&state);
        let sink = gst_app::AppSink::builder()
            .name("reverse-png-sink")
            .sync(false)
            .callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let mut state = callback_state.lock().expect("png writer state");
                        match write_sample(sink, &frames_dir, first_output_index, &mut state) {
                            Ok(()) => Ok(gst::FlowSuccess::Ok),
                            Err(message) => {
                                state.error.get_or_insert(message);
                                Err(gst::FlowError::Error)
                            }
                        }
                    })
                    .build(),
            )
            .build();
        let pipeline = gst::Pipeline::with_name("video-creater-reverse-png-writer");
        pipeline
            .add_many([appsrc.upcast_ref(), &encoder, sink.upcast_ref()])
            .map_err(|error| gst_error("pipeline.add", error))?;
        gst::Element::link_many([appsrc.upcast_ref(), &encoder, sink.upcast_ref()])
            .map_err(|error| gst_error("pipeline.link", error))?;
        let writer = Self {
            pipeline,
            appsrc,
            state,
            frame_duration_ns: (1_000_000_000_u128 * u128::from(fps_denominator)
                / u128::from(fps_numerator)) as u64,
        };
        writer
            .pipeline
            .set_state(gst::State::Playing)
            .map_err(|error| gst_error("pipeline.play", error))?;
        Ok(writer)
    }

    fn push(&self, index: u32, rgba: &[u8]) -> PipelineResult<()> {
        let mut buffer = gst::Buffer::from_mut_slice(rgba.to_vec());
        {
            let buffer = buffer.get_mut().expect("new GStreamer buffer is writable");
            buffer.set_pts(gst::ClockTime::from_nseconds(
                self.frame_duration_ns.saturating_mul(u64::from(index)),
            ));
            buffer.set_duration(gst::ClockTime::from_nseconds(self.frame_duration_ns));
        }
        self.appsrc.push_buffer(buffer).map_err(|error| {
            self.failure()
                .unwrap_or_else(|| gst_error("appsrc.push", error))
        })?;
        Ok(())
    }

    /// Ends the stream, waits for every frame to be written, and returns the
    /// PNG digests in decode order.
    fn finish(self, expected: u32, deadline: Instant) -> PipelineResult<Vec<String>> {
        self.appsrc
            .end_of_stream()
            .map_err(|error| gst_error("appsrc.eos", error))?;
        let bus = self
            .pipeline
            .bus()
            .ok_or_else(|| writer_error("The PNG writer has no bus."))?;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(writer_error("Reversed frame encoding timed out."));
            }
            let Some(message) = bus.timed_pop_filtered(
                gst::ClockTime::from_nseconds(remaining.as_nanos().min(u64::MAX as u128) as u64),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            ) else {
                continue;
            };
            if let gst::MessageView::Error(error) = message.view() {
                return Err(self
                    .failure()
                    .unwrap_or_else(|| gst_error("pngenc", error.error())));
            }
            break;
        }
        if let Some(error) = self.failure() {
            return Err(error);
        }
        let mut state = self.state.lock().expect("png writer state");
        if state.written != expected {
            return Err(writer_error(&format!(
                "pngenc wrote {} of {expected} reversed frames.",
                state.written
            )));
        }
        Ok(std::mem::take(&mut state.png_sha256))
    }

    fn failure(&self) -> Option<Vec<PipelineError>> {
        let message = self.state.lock().expect("png writer state").error.clone()?;
        Some(writer_error(&message))
    }
}

impl Drop for PngWriter {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn write_sample(
    sink: &gst_app::AppSink,
    frames_dir: &Path,
    first_output_index: u32,
    state: &mut WriterState,
) -> Result<(), String> {
    let sample = sink
        .pull_sample()
        .map_err(|_| "pngenc produced no sample".to_string())?;
    let buffer = sample
        .buffer()
        .ok_or_else(|| "pngenc sample has no buffer".to_string())?;
    let map = buffer
        .map_readable()
        .map_err(|error| format!("PNG buffer could not be read: {error}"))?;
    let output_index = first_output_index
        .checked_sub(state.written)
        .ok_or_else(|| "pngenc produced more frames than were decoded".to_string())?;
    let path = frames_dir.join(format!("frame-{output_index:06}.png"));
    std::fs::write(&path, map.as_slice())
        .map_err(|error| format!("Reversed PNG frame could not be written: {error}"))?;
    state
        .png_sha256
        .push(format!("{:x}", Sha256::digest(map.as_slice())));
    state.written += 1;
    Ok(())
}

fn writer_error(message: &str) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "precompose.reverse.pngWriter",
        message,
        "Inspect the reviewed appsrc, pngenc and appsink runtime factories and the source media.",
    )]
}

fn gst_error(path: &str, error: impl std::fmt::Display) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        format!("precompose.reverse.pngWriter.{path}"),
        "GStreamer could not encode reversed PNG frames.",
        "Inspect the reviewed appsrc, pngenc and appsink runtime factories.",
    )
    .with_detail("error", error.to_string())]
}
