use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FRAME_RELATIVE_DIR: &str = "frames";
const ALPHA_VISIBLE_THRESHOLD: u8 = 8;
const VISIBLE_CHANNEL_THRESHOLD: u8 = 10;
const SATURATED_CHANNEL_THRESHOLD: u8 = 245;
const SATURATED_CHROMA_THRESHOLD: u8 = 160;
const EDGE_DELTA_THRESHOLD: u16 = 48;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualQaOptions {
    pub min_opaque_ratio: f64,
    pub min_visible_ratio: f64,
    pub max_saturated_ratio: f64,
    pub min_edge_ratio: f64,
    pub min_temporal_delta: f64,
}

impl Default for VisualQaOptions {
    fn default() -> Self {
        Self {
            min_opaque_ratio: 0.05,
            min_visible_ratio: 0.05,
            max_saturated_ratio: 0.85,
            min_edge_ratio: 0.002,
            min_temporal_delta: 0.001,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VisualQaReport {
    pub sampled_frames: Vec<String>,
    pub opaque_ratio: f64,
    pub saturated_ratio: f64,
    pub edge_ratio: f64,
    pub temporal_delta: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct FrameMetrics {
    pixels: u64,
    opaque_pixels: u64,
    visible_pixels: u64,
    saturated_pixels: u64,
    edge_pairs: u64,
    edge_pairs_over_threshold: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct AggregateMetrics {
    opaque_ratio: f64,
    visible_ratio: f64,
    saturated_ratio: f64,
    edge_ratio: f64,
    temporal_delta: f64,
}

#[derive(Debug)]
struct FrameSample {
    relative_path: String,
    frame: RgbaImage,
    metrics: FrameMetrics,
}

pub fn run_hq_visual_qa(
    artifact_dir: impl AsRef<Path>,
    width: u32,
    height: u32,
    frame_count: u32,
    options: VisualQaOptions,
) -> GpuGraphicsResult<VisualQaReport> {
    if width == 0 || height == 0 {
        return Err(vec![qa_error(
            "visualQa.dimensions",
            "Visual QA expected nonzero frame dimensions.",
            "Render GPU visual artifacts with positive width and height before QA.",
        )
        .with_detail("width", width.to_string())
        .with_detail("height", height.to_string())]);
    }

    let artifact_dir = artifact_dir.as_ref();
    let sample_indices = sample_frame_indices(frame_count)?;
    let sampled_frame_paths = sample_indices
        .iter()
        .map(|frame_index| frame_relative_path(*frame_index))
        .collect::<Vec<_>>();
    let sampled_frames_detail = sampled_frame_paths.join(",");
    let mut samples = Vec::with_capacity(sample_indices.len());

    for relative_path in sampled_frame_paths {
        let frame_path = artifact_dir.join(&relative_path);
        let frame = image::open(&frame_path)
            .map_err(|error| {
                vec![qa_error(
                    "visualQa.samples",
                    "Visual QA could not read a sampled HQ GPU frame.",
                    "Ensure the GPU visual renderer wrote frames/frame-%06d.png artifacts before QA.",
                )
                .with_detail("sampleFrame", relative_path.clone())
                .with_detail("sampledFrames", sampled_frames_detail.clone())
                .with_detail("ioError", error.to_string())]
            })?
            .to_rgba8();

        if frame.width() != width || frame.height() != height {
            return Err(vec![qa_error(
                "visualQa.dimensions",
                "Visual QA sampled frame dimensions do not match the artifact manifest.",
                "Regenerate the GPU visual artifact so every sampled frame matches manifest dimensions.",
            )
            .with_detail("sampleFrame", relative_path)
            .with_detail("sampledFrames", sampled_frames_detail.clone())
            .with_detail("expectedWidth", width.to_string())
            .with_detail("expectedHeight", height.to_string())
            .with_detail("actualWidth", frame.width().to_string())
            .with_detail("actualHeight", frame.height().to_string())]);
        }

        let metrics = frame_metrics(&frame);
        samples.push(FrameSample {
            relative_path,
            frame,
            metrics,
        });
    }

    let frames = samples
        .iter()
        .map(|sample| sample.frame.clone())
        .collect::<Vec<_>>();
    let metrics = aggregate_metrics(&frames);
    let report = VisualQaReport {
        sampled_frames: samples
            .iter()
            .map(|sample| sample.relative_path.clone())
            .collect(),
        opaque_ratio: metrics.opaque_ratio,
        saturated_ratio: metrics.saturated_ratio,
        edge_ratio: metrics.edge_ratio,
        temporal_delta: metrics.temporal_delta,
    };

    for sample in &samples {
        let sample_metrics = frame_metric_ratios(&sample.metrics);
        if sample_metrics.opaque_ratio < options.min_opaque_ratio
            || sample_metrics.visible_ratio < options.min_visible_ratio
        {
            return Err(vec![qa_error(
                "visualQa.samples",
                "Visual QA rejected a blank HQ GPU frame sample.",
                "Regenerate the visual so sampled first, middle, and last frames contain visible content.",
            )
            .with_detail("sampleFrame", sample.relative_path.clone())
            .with_detail("sampledFrames", sampled_frames_detail.clone())
            .with_detail("opaqueRatio", format_metric(sample_metrics.opaque_ratio))
            .with_detail("visibleRatio", format_metric(sample_metrics.visible_ratio))]);
        }

        if sample_metrics.saturated_ratio > options.max_saturated_ratio
            || sample_metrics.edge_ratio < options.min_edge_ratio
        {
            return Err(vec![qa_error(
                "visualQa.blobDominance",
                "Visual QA rejected an HQ GPU frame sample dominated by flat color or saturated blobs.",
                "Use the built-in HQ profile output with visible edges, gradients, and scene structure.",
            )
            .with_detail("sampleFrame", sample.relative_path.clone())
            .with_detail("sampledFrames", sampled_frames_detail.clone())
            .with_detail("saturatedRatio", format_metric(sample_metrics.saturated_ratio))
            .with_detail("edgeRatio", format_metric(sample_metrics.edge_ratio))]);
        }
    }

    if frames.len() >= 2 && metrics.temporal_delta < options.min_temporal_delta {
        return Err(vec![qa_error(
            "visualQa.motion",
            "Visual QA rejected static HQ GPU frames.",
            "Render the HQ GPU profile with visible motion across first, middle, and last samples.",
        )
        .with_detail("sampledFrames", sampled_frames_detail)
        .with_detail("temporalDelta", format_metric(metrics.temporal_delta))]);
    }

    Ok(report)
}

fn sample_frame_indices(frame_count: u32) -> GpuGraphicsResult<Vec<u32>> {
    if frame_count == 0 {
        return Err(vec![qa_error(
            "visualQa.samples",
            "Visual QA requires at least one HQ GPU frame.",
            "Render a nonempty GPU frame sequence before QA.",
        )]);
    }

    let mut indices = Vec::with_capacity(3);
    for frame_index in [0, frame_count / 2, frame_count - 1] {
        if indices.last().copied() != Some(frame_index) {
            indices.push(frame_index);
        }
    }
    Ok(indices)
}

fn frame_relative_path(frame_index: u32) -> String {
    format!("{FRAME_RELATIVE_DIR}/frame-{frame_index:06}.png")
}

fn aggregate_metrics(frames: &[RgbaImage]) -> AggregateMetrics {
    let mut totals = FrameMetrics::default();
    for frame in frames {
        let metrics = frame_metrics(frame);
        totals.pixels += metrics.pixels;
        totals.opaque_pixels += metrics.opaque_pixels;
        totals.visible_pixels += metrics.visible_pixels;
        totals.saturated_pixels += metrics.saturated_pixels;
        totals.edge_pairs += metrics.edge_pairs;
        totals.edge_pairs_over_threshold += metrics.edge_pairs_over_threshold;
    }

    AggregateMetrics {
        temporal_delta: temporal_delta(frames),
        ..frame_metric_ratios(&totals)
    }
}

fn frame_metric_ratios(metrics: &FrameMetrics) -> AggregateMetrics {
    AggregateMetrics {
        opaque_ratio: ratio(metrics.opaque_pixels, metrics.pixels),
        visible_ratio: ratio(metrics.visible_pixels, metrics.pixels),
        saturated_ratio: ratio(metrics.saturated_pixels, metrics.pixels),
        edge_ratio: ratio(metrics.edge_pairs_over_threshold, metrics.edge_pairs),
        temporal_delta: 0.0,
    }
}

fn frame_metrics(frame: &RgbaImage) -> FrameMetrics {
    let mut metrics = FrameMetrics {
        pixels: u64::from(frame.width()) * u64::from(frame.height()),
        ..FrameMetrics::default()
    };

    for pixel in frame.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha > ALPHA_VISIBLE_THRESHOLD {
            metrics.opaque_pixels += 1;
            let max_channel = red.max(green).max(blue);
            let min_channel = red.min(green).min(blue);
            if max_channel > VISIBLE_CHANNEL_THRESHOLD {
                metrics.visible_pixels += 1;
            }
            if max_channel >= SATURATED_CHANNEL_THRESHOLD
                && max_channel - min_channel >= SATURATED_CHROMA_THRESHOLD
            {
                metrics.saturated_pixels += 1;
            }
        }
    }

    for y in 0..frame.height() {
        for x in 0..frame.width() {
            if x + 1 < frame.width() {
                metrics.edge_pairs += 1;
                if pixel_delta(frame.get_pixel(x, y).0, frame.get_pixel(x + 1, y).0)
                    > EDGE_DELTA_THRESHOLD
                {
                    metrics.edge_pairs_over_threshold += 1;
                }
            }
            if y + 1 < frame.height() {
                metrics.edge_pairs += 1;
                if pixel_delta(frame.get_pixel(x, y).0, frame.get_pixel(x, y + 1).0)
                    > EDGE_DELTA_THRESHOLD
                {
                    metrics.edge_pairs_over_threshold += 1;
                }
            }
        }
    }

    metrics
}

fn temporal_delta(frames: &[RgbaImage]) -> f64 {
    if frames.len() < 2 {
        return 0.0;
    }

    let mut total_delta = 0u64;
    let mut channel_count = 0u64;
    for pair in frames.windows(2) {
        for (left, right) in pair[0].pixels().zip(pair[1].pixels()) {
            total_delta += u64::from(left[0].abs_diff(right[0]));
            total_delta += u64::from(left[1].abs_diff(right[1]));
            total_delta += u64::from(left[2].abs_diff(right[2]));
            channel_count += 3;
        }
    }

    if channel_count == 0 {
        0.0
    } else {
        total_delta as f64 / (channel_count as f64 * 255.0)
    }
}

fn pixel_delta(left: [u8; 4], right: [u8; 4]) -> u16 {
    u16::from(left[0].abs_diff(right[0]))
        + u16::from(left[1].abs_diff(right[1]))
        + u16::from(left[2].abs_diff(right[2]))
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn qa_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> GpuGraphicsError {
    GpuGraphicsError::new(
        GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed,
        path,
        message,
        fix,
    )
}

fn format_metric(value: f64) -> String {
    format!("{value:.6}")
}
