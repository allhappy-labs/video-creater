//! Rust-owned WebM export for graphics frame sequences.
//!
//! The CPU graphics renderer writes deterministic RGBA PNG frames. This module
//! turns those frames into a video artifact through a GStreamer pipeline owned
//! and validated by Rust, without shelling out to ffmpeg.

use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::Dimensions;
#[cfg(feature = "ges-render")]
use crate::render_pipeline::plugin_policy::{policy_error_for_factory, GstFactoryInfo};
#[cfg(feature = "ges-render")]
use crate::render_runtime::render_runtime_environment;
#[cfg(feature = "ges-render")]
use gst::prelude::*;
#[cfg(feature = "ges-render")]
use gstreamer as gst;
#[cfg(feature = "ges-render")]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(feature = "ges-render")]
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebmEncoder {
    Vp8,
    Vp9,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WebmExportOptions {
    pub frames_pattern: PathBuf,
    pub output_path: PathBuf,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub frame_count: u32,
    pub encoder: WebmEncoder,
    pub target_bitrate_bps: u32,
}

pub fn build_webm_export_pipeline_description(
    options: &WebmExportOptions,
) -> ActionableResult<String> {
    validate_webm_options(options)?;
    let (fps_num, fps_den) = fps_fraction(options.fps)?;
    let encoder = match options.encoder {
        WebmEncoder::Vp8 => format!(
            "vp8enc deadline=1 target-bitrate={} cpu-used=4",
            options.target_bitrate_bps
        ),
        WebmEncoder::Vp9 => format!(
            "vp9enc deadline=1 target-bitrate={} cpu-used=4",
            options.target_bitrate_bps
        ),
    };
    let stop_index = options.frame_count.saturating_sub(1);

    Ok(format!(
        "multifilesrc location={} index=0 stop-index={} caps=\"image/png,framerate=(fraction){}/{}\" \
         ! pngdec \
         ! videoconvert \
         ! videoscale \
         ! video/x-raw,width={},height={},framerate=(fraction){}/{} \
         ! {} \
         ! webmmux \
         ! filesink location={}",
        quote_for_gst_parse_launch(&options.frames_pattern),
        stop_index,
        fps_num,
        fps_den,
        options.dimensions.width,
        options.dimensions.height,
        fps_num,
        fps_den,
        encoder,
        quote_for_gst_parse_launch(&options.output_path)
    ))
}

#[cfg(feature = "ges-render")]
pub fn export_frame_sequence_to_webm(options: &WebmExportOptions) -> ActionableResult<PathBuf> {
    validate_webm_options(options)?;
    require_allowed_factories(options)?;
    if let Some(parent) = options.output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            vec![ActionableError::new(
                GraphicsErrorCode::RenderBackendFailed,
                "webm.outputPath",
                "Could not create WebM export output directory.",
                "Choose a writable output path.",
            )
            .with_detail("ioError", error.to_string())]
        })?;
    }

    render_runtime_environment().map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendUnavailable,
            "gstreamer.runtime",
            "Could not initialize the required render runtime for graphics WebM export.",
            "Restore the bundled render runtime or use a reviewed development runtime.",
        )
        .with_detail("code", error.code)
        .with_detail("detail", error.detail)]
    })?;

    let pipeline_description = build_webm_export_pipeline_description(options)?;
    let element = gst::parse::launch(&pipeline_description).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.pipeline",
            "Could not build the graphics WebM GStreamer pipeline.",
            "Inspect the generated pipeline and approved plugin availability.",
        )
        .with_detail("gstreamerError", error.to_string())
        .with_detail("pipeline", pipeline_description.clone())]
    })?;
    let pipeline = element.dynamic_cast::<gst::Pipeline>().map_err(|_| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.pipeline",
            "GStreamer did not create a pipeline for graphics WebM export.",
            "Use a full pipeline description with source, encoder, muxer, and sink.",
        )
        .with_detail("pipeline", pipeline_description.clone())]
    })?;

    pipeline.set_state(gst::State::Playing).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.pipeline.state",
            "Could not start graphics WebM export pipeline.",
            "Inspect selected plugins, frame paths, and output path.",
        )
        .with_detail("gstreamerError", error.to_string())]
    })?;

    let result = wait_for_gst_pipeline(&pipeline, Duration::from_secs(120));
    let _ = pipeline.set_state(gst::State::Null);
    result?;

    Ok(options.output_path.clone())
}

#[cfg(feature = "ges-render")]
fn require_allowed_factories(options: &WebmExportOptions) -> ActionableResult<()> {
    render_runtime_environment().map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendUnavailable,
            "gstreamer.runtime",
            "Could not initialize the required render runtime for graphics WebM plugin checks.",
            "Restore the bundled render runtime or use a reviewed development runtime.",
        )
        .with_detail("code", error.code)
        .with_detail("detail", error.detail)]
    })?;
    let encoder_name = match options.encoder {
        WebmEncoder::Vp8 => "vp8enc",
        WebmEncoder::Vp9 => "vp9enc",
    };
    let mut errors = Vec::new();
    for factory_name in [
        "multifilesrc",
        "pngdec",
        "videoconvert",
        "videoscale",
        encoder_name,
        "webmmux",
        "filesink",
    ] {
        match gst::ElementFactory::find(factory_name) {
            Some(factory) => {
                let plugin = factory.plugin();
                let info = GstFactoryInfo::new(factory_name)
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
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::RenderBackendFailed,
                        error.path,
                        error.message,
                        error.fix,
                    ));
                }
            }
            None => errors.push(ActionableError::new(
                GraphicsErrorCode::RenderBackendUnavailable,
                format!("gstreamer.plugins.{factory_name}"),
                format!("Required GStreamer factory '{factory_name}' is unavailable."),
                "Update or reinstall Video Creater to restore its bundled render runtime.",
            )),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(feature = "ges-render")]
fn wait_for_gst_pipeline(pipeline: &gst::Pipeline, timeout: Duration) -> ActionableResult<()> {
    let bus = pipeline.bus().ok_or_else(|| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.pipeline.bus",
            "Graphics WebM export pipeline did not expose a bus.",
            "Inspect GStreamer pipeline setup before rendering.",
        )]
    })?;
    let timeout = gst::ClockTime::try_from_seconds_f64(timeout.as_secs_f64()).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.pipeline.timeout",
            "Graphics WebM export timeout could not be represented as GStreamer clock time.",
            "Use a finite positive render timeout.",
        )
        .with_detail("clockError", error.to_string())]
    })?;

    match bus.timed_pop_filtered(timeout, &[gst::MessageType::Eos, gst::MessageType::Error]) {
        Some(message) => {
            match message.view() {
                gst::MessageView::Eos(_) => Ok(()),
                gst::MessageView::Error(error) => Err(vec![ActionableError::new(
                GraphicsErrorCode::RenderBackendFailed,
                "webm.pipeline.error",
                "Graphics WebM export pipeline failed.",
                "Inspect the GStreamer error, selected plugins, frame sequence, and output path.",
            )
            .with_detail("gstreamerError", error.error().to_string())
            .with_detail(
                "debug",
                error.debug().map(|debug| debug.to_string()).unwrap_or_default(),
            )]),
                _ => Err(vec![ActionableError::new(
                    GraphicsErrorCode::RenderBackendFailed,
                    "webm.pipeline.message",
                    "Graphics WebM export returned an unexpected bus message.",
                    "Inspect backend logs and retry the render.",
                )
                .with_detail("messageType", format!("{:?}", message.type_()))]),
            }
        }
        None => Err(vec![ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.pipeline.timeout",
            "Graphics WebM export timed out.",
            "Shorten the render or inspect the GStreamer pipeline for a stalled element.",
        )
        .with_detail("timeoutSeconds", timeout.seconds().to_string())]),
    }
}

#[cfg(not(feature = "ges-render"))]
pub fn export_frame_sequence_to_webm(_options: &WebmExportOptions) -> ActionableResult<PathBuf> {
    Err(vec![ActionableError::new(
        GraphicsErrorCode::RenderBackendUnavailable,
        "features.ges-render",
        "Graphics WebM export requires the ges-render feature.",
        "Build with the default ges-render feature or render the PNG frame sequence only.",
    )])
}

fn validate_webm_options(options: &WebmExportOptions) -> ActionableResult<()> {
    let mut errors = Vec::new();
    if options.frame_count == 0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::RenderFrameSequenceEmpty,
            "webm.frameCount",
            "Cannot export WebM from an empty frame sequence.",
            "Render at least one graphics frame before exporting WebM.",
        ));
    }
    if !options.fps.is_finite() || options.fps <= 0.0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "webm.fps",
            "Graphics WebM export requires a finite positive frame rate.",
            "Set fps to a positive finite value such as 24 or 30.",
        ));
    }
    if options.dimensions.width == 0 || options.dimensions.height == 0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            "webm.dimensions",
            "Graphics WebM export requires non-zero dimensions.",
            "Set width and height to positive pixel dimensions.",
        ));
    }
    if options.frames_pattern.as_os_str().is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::RenderFrameSequenceEmpty,
            "webm.framesPattern",
            "Graphics WebM export requires a frame pattern.",
            "Pass the frame sequence pattern such as frames/frame-%06d.png.",
        ));
    }
    if options.output_path.as_os_str().is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::RenderBackendFailed,
            "webm.outputPath",
            "Graphics WebM export requires an output path.",
            "Choose a writable .webm output path.",
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn fps_fraction(fps: f64) -> ActionableResult<(u32, u32)> {
    if !fps.is_finite() || fps <= 0.0 {
        return Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "webm.fps",
            "Graphics WebM export requires a finite positive frame rate.",
            "Set fps to a positive finite value such as 24 or 30.",
        )]);
    }
    let common = [
        (24.0, 24, 1),
        (25.0, 25, 1),
        (30.0, 30, 1),
        (60.0, 60, 1),
        (23.976, 24000, 1001),
        (29.97, 30000, 1001),
        (59.94, 60000, 1001),
    ];
    for (value, numerator, denominator) in common {
        if (fps - value).abs() < 0.01 {
            return Ok((numerator, denominator));
        }
    }
    Ok(((fps * 1000.0).round().max(1.0) as u32, 1000))
}

fn quote_for_gst_parse_launch(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}
