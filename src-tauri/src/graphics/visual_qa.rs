use super::animation::layer_has_animation;
use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::{GraphicNode, GraphicRole, GraphicsLayer, TextNode};
use super::manifest::GraphicsArtifactManifest;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FRAME_DIR: &str = "frames";
const ALPHA_VISIBLE_THRESHOLD: u8 = 8;
const MIN_TEMPORAL_DELTA: f64 = 0.001;
const MIN_VISIBLE_ALPHA_RATIO: f64 = 0.001;
const MAX_OVERLAY_ALPHA_RATIO: f64 = 0.30;
const MIN_TEXT_SAFE_ZONE_RATIO: f64 = 0.08;
const TEXT_DENSITY_CHAR_WIDTH_EM: f64 = 0.56;
const TEXT_DENSITY_ALLOWANCE: f64 = 1.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuVisualQaOptions {
    pub min_visible_alpha_ratio: f64,
    pub max_overlay_alpha_ratio: f64,
    pub min_temporal_delta: f64,
}

impl Default for CpuVisualQaOptions {
    fn default() -> Self {
        Self {
            min_visible_alpha_ratio: MIN_VISIBLE_ALPHA_RATIO,
            max_overlay_alpha_ratio: MAX_OVERLAY_ALPHA_RATIO,
            min_temporal_delta: MIN_TEMPORAL_DELTA,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CpuVisualQaReport {
    pub sampled_frames: Vec<String>,
    pub visible_alpha_ratio: f64,
    pub temporal_delta: f64,
}

pub fn run_cpu_visual_qa(
    layer: &GraphicsLayer,
    artifact_dir: impl AsRef<Path>,
    manifest: &GraphicsArtifactManifest,
    options: CpuVisualQaOptions,
) -> ActionableResult<CpuVisualQaReport> {
    if manifest.frame_count == 0 {
        return Err(vec![qa_error(
            layer,
            "frames",
            "CPU visual QA expected at least one rendered frame.",
            "Render graphics before running visual QA.",
        )]);
    }

    let artifact_dir = artifact_dir.as_ref();
    let sample_indices = sample_frame_indices(manifest.frame_count);
    let sampled_frames = sample_indices
        .iter()
        .map(|index| frame_relative_path(*index))
        .collect::<Vec<_>>();
    let mut frames = Vec::with_capacity(sampled_frames.len());
    for relative_path in &sampled_frames {
        let frame_path = artifact_dir.join(relative_path);
        let frame = image::open(&frame_path)
            .map_err(|error| {
                vec![qa_error(
                    layer,
                    "samples",
                    "CPU visual QA could not read a sampled graphics frame.",
                    "Ensure the CPU graphics renderer wrote frames/frame-%06d.png artifacts before QA.",
                )
                .with_detail("sampleFrame", relative_path.clone())
                .with_detail("ioError", error.to_string())]
            })?
            .to_rgba8();
        if frame.width() != manifest.dimensions.width
            || frame.height() != manifest.dimensions.height
        {
            return Err(vec![qa_error(
                layer,
                "dimensions",
                "CPU visual QA sampled frame dimensions do not match the artifact manifest.",
                "Regenerate the graphics artifact so every sampled frame matches manifest dimensions.",
            )
            .with_detail("sampleFrame", relative_path.clone())
            .with_detail("expectedWidth", manifest.dimensions.width.to_string())
            .with_detail("expectedHeight", manifest.dimensions.height.to_string())
            .with_detail("actualWidth", frame.width().to_string())
            .with_detail("actualHeight", frame.height().to_string())]);
        }
        frames.push(frame);
    }

    let visible_alpha_ratio = average_visible_alpha_ratio(&frames);
    let temporal_delta = temporal_delta(&frames);
    let report = CpuVisualQaReport {
        sampled_frames,
        visible_alpha_ratio,
        temporal_delta,
    };

    if visible_alpha_ratio < options.min_visible_alpha_ratio {
        return Err(vec![qa_error(
            layer,
            "coverage",
            "CPU visual QA rejected a blank or nearly invisible graphics layer.",
            "Increase visible alpha coverage or remove the visual layer.",
        )
        .with_detail("visibleAlphaRatio", format_metric(visible_alpha_ratio))]);
    }

    if role_has_overlay_coverage_limit(&layer.role)
        && visible_alpha_ratio > options.max_overlay_alpha_ratio
    {
        return Err(vec![qa_error(
            layer,
            "coverage",
            "CPU visual QA rejected an overlay that covers too much of the frame.",
            "Use a smaller overlay, shorten the duration, or use title_card only for intentional full-frame beats.",
        )
        .with_detail("visibleAlphaRatio", format_metric(visible_alpha_ratio))
        .with_detail("maxOverlayAlphaRatio", format_metric(options.max_overlay_alpha_ratio))]);
    }

    validate_text_readability(layer)?;

    if layer_has_animation(&layer.nodes)
        && manifest.frame_count > 1
        && temporal_delta < options.min_temporal_delta
    {
        return Err(vec![qa_error(
            layer,
            "motion",
            "CPU visual QA rejected static animated graphics frames.",
            "Use motion preset keyframes that visibly change first, middle, and last sampled frames.",
        )
        .with_detail("temporalDelta", format_metric(temporal_delta))
        .with_detail("minTemporalDelta", format_metric(options.min_temporal_delta))]);
    }

    Ok(report)
}

fn validate_text_readability(layer: &GraphicsLayer) -> ActionableResult<()> {
    for node in &layer.nodes {
        let GraphicNode::Text(text) = node else {
            continue;
        };
        validate_text_safe_zone(layer, text)?;
        validate_text_density(layer, text)?;
    }

    Ok(())
}

fn validate_text_safe_zone(layer: &GraphicsLayer, text: &TextNode) -> ActionableResult<()> {
    let min_x = layer.dimensions.width as f64 * MIN_TEXT_SAFE_ZONE_RATIO;
    let min_y = layer.dimensions.height as f64 * MIN_TEXT_SAFE_ZONE_RATIO;
    let max_x = layer.dimensions.width as f64 * (1.0 - MIN_TEXT_SAFE_ZONE_RATIO);
    let max_y = layer.dimensions.height as f64 * (1.0 - MIN_TEXT_SAFE_ZONE_RATIO);
    let rect = &text.box_rect;

    if rect.x >= min_x
        && rect.y >= min_y
        && rect.x + rect.width <= max_x
        && rect.y + rect.height <= max_y
    {
        return Ok(());
    }

    Err(vec![qa_error(
        layer,
        "safeZone",
        "CPU visual QA rejected text outside the safe zone.",
        "Move essential text farther from the frame edge, shrink the text box, or use a non-text pointer element near the edge.",
    )
    .with_detail("nodeId", text.id.clone())
    .with_detail("minSafeZoneRatio", format_metric(MIN_TEXT_SAFE_ZONE_RATIO))
    .with_detail("textBoxX", format_metric(rect.x))
    .with_detail("textBoxY", format_metric(rect.y))
    .with_detail("textBoxWidth", format_metric(rect.width))
    .with_detail("textBoxHeight", format_metric(rect.height))])
}

fn validate_text_density(layer: &GraphicsLayer, text: &TextNode) -> ActionableResult<()> {
    let normalized_text = text.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let character_count = normalized_text.chars().count();
    if character_count == 0 {
        return Ok(());
    }

    let max_lines = text.max_lines.unwrap_or(2).max(1) as usize;
    let chars_per_line = (text.box_rect.width / (text.font_size * TEXT_DENSITY_CHAR_WIDTH_EM))
        .floor()
        .max(1.0) as usize;
    let allowed_chars = ((chars_per_line * max_lines) as f64 * TEXT_DENSITY_ALLOWANCE)
        .ceil()
        .max(1.0) as usize;

    if character_count <= allowed_chars {
        return Ok(());
    }

    Err(vec![qa_error(
        layer,
        "textDensity",
        "CPU visual QA rejected text that is too dense for its box and line count.",
        "Shorten the text, increase the text box width, reduce fontSize, or allow more caption lines.",
    )
    .with_detail("nodeId", text.id.clone())
    .with_detail("characterCount", character_count.to_string())
    .with_detail("allowedCharacters", allowed_chars.to_string())
    .with_detail("maxLines", max_lines.to_string())
    .with_detail("fontSize", format_metric(text.font_size))])
}

fn sample_frame_indices(frame_count: u32) -> Vec<u32> {
    let mut indices = Vec::with_capacity(3);
    for index in [0, frame_count / 2, frame_count - 1] {
        if indices.last().copied() != Some(index) {
            indices.push(index);
        }
    }
    indices
}

fn frame_relative_path(frame_index: u32) -> String {
    format!("{FRAME_DIR}/frame-{frame_index:06}.png")
}

fn average_visible_alpha_ratio(frames: &[RgbaImage]) -> f64 {
    if frames.is_empty() {
        return 0.0;
    }
    frames.iter().map(visible_alpha_ratio).sum::<f64>() / frames.len() as f64
}

fn visible_alpha_ratio(frame: &RgbaImage) -> f64 {
    let pixels = u64::from(frame.width()) * u64::from(frame.height());
    if pixels == 0 {
        return 0.0;
    }
    let visible = frame
        .pixels()
        .filter(|pixel| pixel[3] > ALPHA_VISIBLE_THRESHOLD)
        .count() as u64;
    visible as f64 / pixels as f64
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
            total_delta += u64::from(left[3].abs_diff(right[3]));
            channel_count += 4;
        }
    }

    if channel_count == 0 {
        0.0
    } else {
        total_delta as f64 / (channel_count as f64 * 255.0)
    }
}

fn role_has_overlay_coverage_limit(role: &GraphicRole) -> bool {
    matches!(
        role,
        GraphicRole::Overlay | GraphicRole::LowerThird | GraphicRole::Diagram
    )
}

fn qa_error(
    layer: &GraphicsLayer,
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
) -> ActionableError {
    ActionableError::new(
        GraphicsErrorCode::GraphicsVisualQaFailed,
        format!("graphics[{}].visualQa.{}", layer.id, path.into()),
        message,
        fix,
    )
    .with_detail("layerId", layer.id.clone())
    .with_detail("role", format!("{:?}", layer.role))
}

fn format_metric(value: f64) -> String {
    format!("{value:.6}")
}
