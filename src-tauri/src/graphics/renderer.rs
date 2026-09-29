//! Rust-native graphics rasterization.

use super::animation::{
    animate_node, evaluate_animation, frame_count_for_duration, layer_has_animation, AnimationState,
};
use super::assets::AssetRegistry;
use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::{
    Color, Easing, GraphicNode, GraphicsLayer, HolographicLogoNode, ImageFit, ImageRefNode,
    LineNode, Point, PolygonNode, Rect, RectNode, RoundedRectNode, TextNode, TransformOrigin,
    TransformOriginCoordinate,
};
use super::manifest::{graphics_playback_manifest, GraphicsArtifactManifest};
use super::shader_noise::{
    domain_warp_2d, fbm_2d, fbm_3d, fract, rotate_2d, smoothstep, stable_pixel_noise,
    turbulence_3d, value_noise_2d, value_noise_3d,
};
use super::validation::validate_graphics_layer_with_assets;
use cosmic_text::{
    Align, Attrs, Buffer, Color as TextColor, Ellipsize, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight, Wrap,
};
use image::{ImageFormat, RgbaImage};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect as SkRect, Stroke, Transform};

const PREVIEW_FILE_NAME: &str = "preview.png";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const FRAMES_DIR_NAME: &str = "frames";
const FRAMES_PATTERN: &str = "frames/frame-%06d.png";
const ROUND_RECT_KAPPA: f32 = 0.552_284_8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsRenderOptions {
    pub output_dir: PathBuf,
}

pub fn render_graphics_preview(
    layer: &GraphicsLayer,
    assets: &AssetRegistry,
    options: GraphicsRenderOptions,
) -> ActionableResult<GraphicsArtifactManifest> {
    render_graphics_preview_cancellable(layer, assets, options, || false)
}

pub fn render_graphics_preview_cancellable(
    layer: &GraphicsLayer,
    assets: &AssetRegistry,
    options: GraphicsRenderOptions,
    is_cancelled: impl Fn() -> bool,
) -> ActionableResult<GraphicsArtifactManifest> {
    if is_cancelled() {
        return Err(cancelled_graphics_errors(&options.output_dir));
    }

    validate_graphics_layer_with_assets(layer, assets)?;

    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        render_failed(
            "outputDir",
            "Could not create graphics preview output directory.",
            "Choose a writable outputDir for the graphics artifact.",
            error,
        )
    })?;

    let frames_dir = options.output_dir.join(FRAMES_DIR_NAME);
    if frames_dir.exists() {
        std::fs::remove_dir_all(&frames_dir).map_err(|error| {
            render_failed(
                "frames",
                "Could not clear stale graphics frame output directory.",
                "Remove stale graphics artifacts or choose a fresh writable outputDir.",
                error,
            )
        })?;
    }
    std::fs::create_dir_all(&frames_dir).map_err(|error| {
        render_failed(
            "frames",
            "Could not create graphics frame output directory.",
            "Choose a writable outputDir for the graphics artifact.",
            error,
        )
    })?;

    let animated = layer_has_animation(&layer.nodes);
    let frame_count = frame_count_for_duration(layer.duration_seconds, layer.fps, animated);
    let mut preview_pixmap = None;
    let mut render_context = RenderContext::new();
    for frame_index in 0..frame_count {
        if is_cancelled() {
            return Err(cancelled_graphics_errors(&options.output_dir));
        }
        let time_seconds = frame_time_seconds(frame_index, layer.fps, layer.duration_seconds);
        let pixmap = render_layer_pixmap(layer, assets, &mut render_context, time_seconds)?;
        let frame_path = frames_dir.join(frame_file_name(frame_index));
        write_png(&pixmap, &frame_path)?;
        if frame_index == 0 {
            preview_pixmap = Some(pixmap);
        }
    }

    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("{}.preview", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count,
        frames_pattern: FRAMES_PATTERN.to_string(),
        playback: graphics_playback_manifest(
            frame_count,
            layer.fps,
            layer.duration_seconds,
            animated,
            FRAMES_PATTERN,
        ),
        preview_path: PathBuf::from(PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };

    if is_cancelled() {
        return Err(cancelled_graphics_errors(&options.output_dir));
    }

    let preview_path = options.output_dir.join(PREVIEW_FILE_NAME);
    let preview_pixmap = preview_pixmap.ok_or_else(|| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "frames",
            "Could not render a preview frame.",
            "Use a layer duration and fps that produce at least one graphics frame.",
        )]
    })?;
    write_png(&preview_pixmap, &preview_path)?;

    let manifest_path = options.output_dir.join(MANIFEST_FILE_NAME);
    write_manifest(&manifest, &manifest_path)?;

    Ok(manifest)
}

fn cancelled_graphics_errors(output_dir: &std::path::Path) -> Vec<ActionableError> {
    let mut errors = vec![ActionableError::new(
        GraphicsErrorCode::GraphicsRenderFailed,
        "cancelled",
        "Graphics rendering was cancelled.",
        "Start a new render attempt when you are ready to retry.",
    )];
    if let Err(error) = std::fs::remove_dir_all(output_dir) {
        if error.kind() != std::io::ErrorKind::NotFound {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsRenderFailed,
                "cancelled.cleanup",
                format!("Cancelled graphics artifacts could not be removed: {error}"),
                "Remove the partial graphics directory before retrying the render.",
            ));
        }
    }
    errors
}

fn render_layer_pixmap(
    layer: &GraphicsLayer,
    assets: &AssetRegistry,
    render_context: &mut RenderContext,
    time_seconds: f64,
) -> ActionableResult<Pixmap> {
    let mut pixmap =
        Pixmap::new(layer.dimensions.width, layer.dimensions.height).ok_or_else(|| {
            vec![ActionableError::new(
                GraphicsErrorCode::GraphicsRenderFailed,
                "dimensions",
                "Could not allocate preview pixmap.",
                "Use supported non-zero graphics dimensions.",
            )]
        })?;

    for (index, source_node) in layer.nodes.iter().enumerate() {
        let animation = source_node.animation();
        let state = animation
            .map(|animation| evaluate_animation(animation, time_seconds, layer.duration_seconds))
            .unwrap_or_default();
        let mut node = animate_node(source_node, time_seconds, layer.duration_seconds);
        if let GraphicNode::Line(line) = &mut node {
            line.points = trim_line_points(&line.points, state.path_progress);
        }
        draw_composited_node(
            &mut pixmap,
            &node,
            animation.and_then(|animation| animation.origin.as_ref()),
            state,
            NodeRenderContext {
                assets,
                render_context,
                index,
                time_seconds,
                duration_seconds: layer.duration_seconds,
            },
        )?;
    }

    Ok(pixmap)
}

struct NodeRenderContext<'a> {
    assets: &'a AssetRegistry,
    render_context: &'a mut RenderContext,
    index: usize,
    time_seconds: f64,
    duration_seconds: f64,
}

fn draw_composited_node(
    target: &mut Pixmap,
    node: &GraphicNode,
    origin: Option<&TransformOrigin>,
    state: AnimationState,
    context: NodeRenderContext<'_>,
) -> ActionableResult<()> {
    let mut node_pixmap = Pixmap::new(target.width(), target.height()).ok_or_else(|| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "dimensions",
            "Could not allocate node pixmap.",
            "Use supported non-zero graphics dimensions.",
        )]
    })?;

    draw_node(
        &mut node_pixmap,
        node,
        context.assets,
        context.render_context,
        context.index,
        context.time_seconds,
        context.duration_seconds,
    )?;

    if state.blur_radius > 0.0 {
        blur_pixmap(&mut node_pixmap, state.blur_radius);
    }

    let bounds = node_bounds(node);
    let origin = bounds.map(|bounds| resolve_transform_origin(origin, &bounds));

    if state.shadow_opacity > 0.0 {
        let shadow = shadow_pixmap(&node_pixmap, state.shadow_opacity);
        composite_pixmap(target, &shadow, None, origin, state.rotation_degrees);
    }

    if state.glow_opacity > 0.0 {
        let glow = glow_pixmap(&node_pixmap, state.glow_opacity);
        composite_pixmap(target, &glow, None, origin, state.rotation_degrees);
    }

    let clip = bounds.map(|bounds| Rect {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width * state.clip_progress,
        height: bounds.height,
    });
    composite_pixmap(target, &node_pixmap, clip, origin, state.rotation_degrees);

    Ok(())
}

fn draw_node(
    pixmap: &mut Pixmap,
    node: &GraphicNode,
    assets: &AssetRegistry,
    render_context: &mut RenderContext,
    index: usize,
    time_seconds: f64,
    duration_seconds: f64,
) -> ActionableResult<()> {
    match node {
        GraphicNode::RoundedRect(node) => draw_rounded_rect(pixmap, node, index)?,
        GraphicNode::Rect(node) => draw_rect(pixmap, node, index, time_seconds, duration_seconds)?,
        GraphicNode::Polygon(node) => draw_polygon(pixmap, node, index)?,
        GraphicNode::Line(node) => draw_line(pixmap, node, index)?,
        GraphicNode::Text(node) => {
            render_context
                .text_renderer
                .draw_text(pixmap, node, index, time_seconds)?
        }
        GraphicNode::ImageRef(node) => draw_image_ref(pixmap, node, assets, render_context, index)?,
        GraphicNode::HolographicLogo(node) => draw_holographic_logo(
            pixmap,
            node,
            render_context,
            index,
            time_seconds,
            duration_seconds,
        )?,
    }
    Ok(())
}

fn node_bounds(node: &GraphicNode) -> Option<Rect> {
    match node {
        GraphicNode::Text(node) => Some(node.box_rect),
        GraphicNode::RoundedRect(node) => Some(node.box_rect),
        GraphicNode::Rect(node) => Some(node.box_rect),
        GraphicNode::ImageRef(node) => Some(node.box_rect),
        GraphicNode::HolographicLogo(node) => Some(node.box_rect),
        GraphicNode::Polygon(node) => bounds_for_points(&node.points),
        GraphicNode::Line(node) => bounds_for_points(&node.points),
    }
}

fn bounds_for_points(points: &[Point]) -> Option<Rect> {
    if points.is_empty() {
        return None;
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }

    Some(Rect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(1.0),
        height: (max_y - min_y).max(1.0),
    })
}

fn resolve_transform_origin(origin: Option<&TransformOrigin>, bounds: &Rect) -> Point {
    let Some(origin) = origin else {
        return Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        };
    };

    Point {
        x: resolve_origin_coordinate(&origin.x, bounds.x, bounds.width, "center"),
        y: resolve_origin_coordinate(&origin.y, bounds.y, bounds.height, "center"),
    }
}

fn resolve_origin_coordinate(
    coordinate: &TransformOriginCoordinate,
    start: f64,
    size: f64,
    default_keyword: &str,
) -> f64 {
    match coordinate {
        TransformOriginCoordinate::Pixels(value) => *value,
        TransformOriginCoordinate::Keyword(keyword) => match keyword.as_str() {
            "left" | "top" => start,
            "right" | "bottom" => start + size,
            "center" => start + size / 2.0,
            _ if default_keyword == "center" => start + size / 2.0,
            _ => start,
        },
    }
}

fn trim_line_points(points: &[Point], progress: f64) -> Vec<Point> {
    let progress = progress.clamp(0.0, 1.0);
    if points.len() < 2 || progress <= 0.0 {
        return Vec::new();
    }
    if progress >= 1.0 {
        return points.to_vec();
    }

    let total = polyline_length(points);
    if total <= f64::EPSILON {
        return points.to_vec();
    }

    let mut remaining = total * progress;
    let mut trimmed = vec![points[0]];
    for pair in points.windows(2) {
        let start = &pair[0];
        let end = &pair[1];
        let length = point_distance(start, end);
        if remaining >= length {
            trimmed.push(*end);
            remaining -= length;
            continue;
        }

        let t = (remaining / length.max(f64::EPSILON)).clamp(0.0, 1.0);
        trimmed.push(Point {
            x: start.x + (end.x - start.x) * t,
            y: start.y + (end.y - start.y) * t,
        });
        break;
    }
    trimmed
}

fn polyline_length(points: &[Point]) -> f64 {
    points
        .windows(2)
        .map(|pair| point_distance(&pair[0], &pair[1]))
        .sum()
}

fn point_distance(a: &Point, b: &Point) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

fn blur_pixmap(pixmap: &mut Pixmap, radius: f64) {
    let passes = (radius.round() as u32).clamp(1, 8);
    for _ in 0..passes {
        box_blur_once(pixmap);
    }
}

fn box_blur_once(pixmap: &mut Pixmap) {
    let width = pixmap.width();
    let height = pixmap.height();
    let source = pixmap.data().to_vec();
    let target = pixmap.data_mut();
    for y in 0..height {
        for x in 0..width {
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let sx = x as i32 + ox;
                    let sy = y as i32 + oy;
                    if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                        continue;
                    }
                    let index = ((sy as u32 * width + sx as u32) * 4) as usize;
                    sums[0] += source[index] as u32;
                    sums[1] += source[index + 1] as u32;
                    sums[2] += source[index + 2] as u32;
                    sums[3] += source[index + 3] as u32;
                    count += 1;
                }
            }
            let index = ((y * width + x) * 4) as usize;
            target[index] = (sums[0] / count) as u8;
            target[index + 1] = (sums[1] / count) as u8;
            target[index + 2] = (sums[2] / count) as u8;
            target[index + 3] = (sums[3] / count) as u8;
        }
    }
}

fn shadow_pixmap(source: &Pixmap, opacity: f64) -> Pixmap {
    let mut shadow = Pixmap::new(source.width(), source.height()).expect("matching pixmap");
    let alpha_scale = (opacity.clamp(0.0, 1.0) * 0.45).clamp(0.0, 1.0);
    for y in 0..source.height() {
        for x in 0..source.width() {
            let index = ((y * source.width() + x) * 4) as usize;
            let alpha = source.data()[index + 3];
            if alpha == 0 {
                continue;
            }
            let dx = x as i32 + 6;
            let dy = y as i32 + 8;
            if dx < 0 || dy < 0 || dx >= source.width() as i32 || dy >= source.height() as i32 {
                continue;
            }
            let shadow_alpha = ((alpha as f64 * alpha_scale).round()).clamp(0.0, 255.0) as u8;
            set_premultiplied_pixel(&mut shadow, dx as u32, dy as u32, [0, 0, 0, shadow_alpha]);
        }
    }
    shadow
}

fn glow_pixmap(source: &Pixmap, opacity: f64) -> Pixmap {
    let mut glow = Pixmap::new(source.width(), source.height()).expect("matching pixmap");
    let radius = 6i32;
    let opacity = opacity.clamp(0.0, 1.0) * 0.35;
    for y in 0..source.height() {
        for x in 0..source.width() {
            let index = ((y * source.width() + x) * 4) as usize;
            let alpha = source.data()[index + 3];
            if alpha == 0 {
                continue;
            }
            let color = [
                source.data()[index],
                source.data()[index + 1],
                source.data()[index + 2],
            ];
            for oy in -radius..=radius {
                for ox in -radius..=radius {
                    let distance = ((ox * ox + oy * oy) as f64).sqrt();
                    if distance > radius as f64 {
                        continue;
                    }
                    let dx = x as i32 + ox;
                    let dy = y as i32 + oy;
                    if dx < 0
                        || dy < 0
                        || dx >= source.width() as i32
                        || dy >= source.height() as i32
                    {
                        continue;
                    }
                    let falloff = 1.0 - distance / radius as f64;
                    let glow_alpha =
                        ((alpha as f64 * opacity * falloff).round()).clamp(0.0, 255.0) as u8;
                    if glow_alpha == 0 {
                        continue;
                    }
                    blend_premultiplied_pixel(
                        &mut glow,
                        dx as u32,
                        dy as u32,
                        premultiply_color([color[0], color[1], color[2], glow_alpha]),
                    );
                }
            }
        }
    }
    glow
}

fn composite_pixmap(
    target: &mut Pixmap,
    source: &Pixmap,
    clip: Option<Rect>,
    origin: Option<Point>,
    rotation_degrees: f64,
) {
    let rotation = if rotation_degrees.abs() > f64::EPSILON {
        Some(rotation_degrees.to_radians())
    } else {
        None
    };
    let origin = origin.unwrap_or(Point { x: 0.0, y: 0.0 });

    for y in 0..target.height() {
        for x in 0..target.width() {
            let (source_x, source_y) = if let Some(rotation) = rotation {
                inverse_rotate_point(x as f64, y as f64, &origin, rotation)
            } else {
                (x as f64, y as f64)
            };

            if let Some(clip) = &clip {
                if source_x < clip.x
                    || source_y < clip.y
                    || source_x >= clip.x + clip.width
                    || source_y >= clip.y + clip.height
                {
                    continue;
                }
            }

            let sx = source_x.round() as i32;
            let sy = source_y.round() as i32;
            if sx < 0 || sy < 0 || sx >= source.width() as i32 || sy >= source.height() as i32 {
                continue;
            }
            let source_index = ((sy as u32 * source.width() + sx as u32) * 4) as usize;
            let rgba = [
                source.data()[source_index],
                source.data()[source_index + 1],
                source.data()[source_index + 2],
                source.data()[source_index + 3],
            ];
            if rgba[3] == 0 {
                continue;
            }
            blend_premultiplied_pixel(target, x, y, rgba);
        }
    }
}

fn inverse_rotate_point(x: f64, y: f64, origin: &Point, rotation_radians: f64) -> (f64, f64) {
    let inverse = -rotation_radians;
    let (sin, cos) = inverse.sin_cos();
    let dx = x - origin.x;
    let dy = y - origin.y;
    (
        origin.x + dx * cos - dy * sin,
        origin.y + dx * sin + dy * cos,
    )
}

fn premultiply_color(rgba: [u8; 4]) -> [u8; 4] {
    let alpha = rgba[3] as u16;
    [
        ((rgba[0] as u16 * alpha + 127) / 255) as u8,
        ((rgba[1] as u16 * alpha + 127) / 255) as u8,
        ((rgba[2] as u16 * alpha + 127) / 255) as u8,
        rgba[3],
    ]
}

fn set_premultiplied_pixel(pixmap: &mut Pixmap, x: u32, y: u32, rgba: [u8; 4]) {
    let index = ((y * pixmap.width() + x) * 4) as usize;
    let target = &mut pixmap.data_mut()[index..index + 4];
    target.copy_from_slice(&rgba);
}

fn blend_premultiplied_pixel(pixmap: &mut Pixmap, x: u32, y: u32, rgba: [u8; 4]) {
    if rgba[3] == 0 || x >= pixmap.width() || y >= pixmap.height() {
        return;
    }
    let index = ((y * pixmap.width() + x) * 4) as usize;
    let dest = &mut pixmap.data_mut()[index..index + 4];
    let source_alpha = rgba[3] as u32;
    let inv_alpha = 255 - source_alpha;
    dest[0] = (rgba[0] as u32 + dest[0] as u32 * inv_alpha / 255).min(255) as u8;
    dest[1] = (rgba[1] as u32 + dest[1] as u32 * inv_alpha / 255).min(255) as u8;
    dest[2] = (rgba[2] as u32 + dest[2] as u32 * inv_alpha / 255).min(255) as u8;
    dest[3] = (source_alpha + dest[3] as u32 * inv_alpha / 255).min(255) as u8;
}

fn frame_time_seconds(frame_index: u32, fps: f64, duration_seconds: f64) -> f64 {
    if frame_index == 0 || !fps.is_finite() || fps <= 0.0 {
        return 0.0;
    }
    (frame_index as f64 / fps).min(duration_seconds)
}

fn frame_file_name(frame_index: u32) -> String {
    format!("frame-{frame_index:06}.png")
}

struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

struct RenderContext {
    text_renderer: TextRenderer,
    image_cache: BTreeMap<String, RgbaImage>,
    logo_mask_cache: BTreeMap<String, Pixmap>,
}

impl RenderContext {
    fn new() -> Self {
        Self {
            text_renderer: TextRenderer::new(),
            image_cache: BTreeMap::new(),
            logo_mask_cache: BTreeMap::new(),
        }
    }
}

impl TextRenderer {
    fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    fn draw_text(
        &mut self,
        pixmap: &mut Pixmap,
        node: &TextNode,
        index: usize,
        time_seconds: f64,
    ) -> ActionableResult<()> {
        let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
        let metrics = Metrics::new(node.font_size as f32, (node.font_size * 1.2) as f32);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_wrap(match node.max_lines {
            Some(1) => Wrap::None,
            _ => Wrap::WordOrGlyph,
        });
        buffer.set_ellipsize(Ellipsize::None);
        buffer.set_size(
            Some(node.box_rect.width as f32),
            Some(node.box_rect.height as f32),
        );
        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(Weight(node.font_weight));
        if node.emphasis.is_empty() {
            buffer.set_text(
                &node.text,
                &attrs,
                Shaping::Advanced,
                align_from_node(&node.align),
            );
        } else {
            let mut spans = Vec::new();
            let mut cursor = 0;
            for emphasis in &node.emphasis {
                if cursor < emphasis.start_byte {
                    spans.push((&node.text[cursor..emphasis.start_byte], attrs.clone()));
                }
                let emphasis_color =
                    parse_color(&emphasis.fill, &format!("nodes[{index}].emphasis.fill"))?;
                let is_active = match (emphasis.active_start_seconds, emphasis.active_end_seconds) {
                    (Some(start), Some(end)) => time_seconds >= start && time_seconds <= end,
                    _ => true,
                };
                let motion_progress = word_emphasis_progress(emphasis, time_seconds);
                let emphasis_attrs = if is_active && motion_progress > 0.0 {
                    let eased = apply_word_easing(
                        motion_progress,
                        emphasis.easing.unwrap_or(Easing::OutQuad),
                    );
                    let scale = 1.0 + (emphasis.emphasis_scale.unwrap_or(1.0) - 1.0) * eased;
                    let opacity = emphasis.emphasis_opacity.unwrap_or(1.0) * motion_progress;
                    attrs
                        .clone()
                        .weight(Weight(emphasis.font_weight))
                        .metrics(Metrics::new(
                            (node.font_size * scale) as f32,
                            (node.font_size * 1.2 * scale) as f32,
                        ))
                        .color(TextColor::rgba(
                            emphasis_color[0],
                            emphasis_color[1],
                            emphasis_color[2],
                            (f64::from(emphasis_color[3]) * opacity.clamp(0.0, 1.0)).round() as u8,
                        ))
                } else {
                    attrs.clone()
                };
                spans.push((
                    &node.text[emphasis.start_byte..emphasis.end_byte],
                    emphasis_attrs,
                ));
                cursor = emphasis.end_byte;
            }
            if cursor < node.text.len() {
                spans.push((&node.text[cursor..], attrs.clone()));
            }
            buffer.set_rich_text(
                spans,
                &attrs,
                Shaping::Advanced,
                align_from_node(&node.align),
            );
        }

        let x_offset = node.box_rect.x.round() as i32;
        let y_offset = node.box_rect.y.round() as i32;
        let clip = IntClip::from_rect(&node.box_rect, pixmap.width(), pixmap.height());
        let text_color = TextColor::rgba(rgba[0], rgba[1], rgba[2], rgba[3]);

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            text_color,
            |x, y, width, height, color| {
                let alpha = multiply_alpha(color.a(), rgba[3]);
                blend_solid_rect(
                    pixmap,
                    x_offset + x,
                    y_offset + y,
                    width,
                    height,
                    [color.r(), color.g(), color.b(), alpha],
                    clip,
                );
            },
        );

        Ok(())
    }
}

fn word_emphasis_progress(emphasis: &crate::graphics::ir::TextEmphasisRange, time: f64) -> f64 {
    let (Some(enter_start), Some(enter_end), Some(hold_end), Some(exit_end)) = (
        emphasis.enter_start_seconds,
        emphasis.enter_end_seconds,
        emphasis.hold_end_seconds,
        emphasis.exit_end_seconds,
    ) else {
        return 1.0;
    };
    if time < enter_start || time > exit_end {
        0.0
    } else if time < enter_end {
        ((time - enter_start) / (enter_end - enter_start).max(0.001)).clamp(0.0, 1.0)
    } else if time <= hold_end {
        1.0
    } else {
        (1.0 - (time - hold_end) / (exit_end - hold_end).max(0.001)).clamp(0.0, 1.0)
    }
}

fn apply_word_easing(progress: f64, easing: Easing) -> f64 {
    match easing {
        Easing::Linear => progress,
        Easing::OutBack => {
            1.0 + 2.70158 * (progress - 1.0).powi(3) + 1.70158 * (progress - 1.0).powi(2)
        }
        _ => 1.0 - (1.0 - progress).powi(2),
    }
}

fn draw_rounded_rect(
    pixmap: &mut Pixmap,
    node: &RoundedRectNode,
    index: usize,
) -> ActionableResult<()> {
    let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let Some(path) = rounded_rect_path(&node.box_rect, node.radius) else {
        return Ok(());
    };
    let paint = paint_from_rgba(rgba);
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    Ok(())
}

fn draw_rect(
    pixmap: &mut Pixmap,
    node: &RectNode,
    index: usize,
    time_seconds: f64,
    duration_seconds: f64,
) -> ActionableResult<()> {
    if node.id == "dark-gradient" {
        return draw_dark_gradient_rect(pixmap, node, index);
    }
    if node.id.starts_with("gradient-loop-panel-") {
        return draw_gradient_loop_panel_rect(pixmap, node, index, time_seconds, duration_seconds);
    }
    if node.id == "stylize-grain" {
        return draw_stylize_grain_rect(pixmap, node, index, time_seconds);
    }
    if node.id == "stylize-vignette" {
        return draw_stylize_vignette_rect(pixmap, node, index);
    }

    let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let Some(rect) = sk_rect(&node.box_rect) else {
        return Ok(());
    };
    let paint = paint_from_rgba(rgba);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    Ok(())
}

fn draw_stylize_grain_rect(
    pixmap: &mut Pixmap,
    node: &RectNode,
    index: usize,
    time_seconds: f64,
) -> ActionableResult<()> {
    let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let Some(bounds) = IntClip::from_rect(&node.box_rect, pixmap.width(), pixmap.height()) else {
        return Ok(());
    };
    let frame = (time_seconds * 24.0).round().max(0.0) as u32;
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let noise = stable_pixel_noise(x as u32, y as u32, frame);
            let alpha = (f64::from(rgba[3]) * (0.15 + noise * 0.85)).round() as u8;
            blend_pixel(pixmap, x, y, [rgba[0], rgba[1], rgba[2], alpha]);
        }
    }
    Ok(())
}

fn draw_stylize_vignette_rect(
    pixmap: &mut Pixmap,
    node: &RectNode,
    index: usize,
) -> ActionableResult<()> {
    let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let Some(bounds) = IntClip::from_rect(&node.box_rect, pixmap.width(), pixmap.height()) else {
        return Ok(());
    };
    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let u =
                ((x as f64 + 0.5 - node.box_rect.x) / node.box_rect.width.max(1.0)).clamp(0.0, 1.0);
            let v = ((y as f64 + 0.5 - node.box_rect.y) / node.box_rect.height.max(1.0))
                .clamp(0.0, 1.0);
            let distance = (((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt()
                / std::f64::consts::FRAC_1_SQRT_2)
                .clamp(0.0, 1.0);
            let edge = smoothstep(0.32, 1.0, distance).powf(1.4);
            let alpha = (f64::from(rgba[3]) * edge).round() as u8;
            blend_pixel(pixmap, x, y, [rgba[0], rgba[1], rgba[2], alpha]);
        }
    }
    Ok(())
}

fn draw_gradient_loop_panel_rect(
    pixmap: &mut Pixmap,
    node: &RectNode,
    index: usize,
    time_seconds: f64,
    duration_seconds: f64,
) -> ActionableResult<()> {
    let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let Some(bounds) = IntClip::from_rect(&node.box_rect, pixmap.width(), pixmap.height()) else {
        return Ok(());
    };
    let panel_index = gradient_loop_panel_index(&node.id);
    let stops = gradient_loop_panel_stops(panel_index);
    let phase = gradient_loop_panel_phase(panel_index, time_seconds, duration_seconds);

    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let u =
                ((x as f64 + 0.5 - node.box_rect.x) / node.box_rect.width.max(1.0)).clamp(0.0, 1.0);
            let v = ((y as f64 + 0.5 - node.box_rect.y) / node.box_rect.height.max(1.0))
                .clamp(0.0, 1.0);
            let animated_v = (v + phase).rem_euclid(1.0);
            let [mut r, mut g, mut b] = sample_gradient_loop_stops(stops, animated_v);
            let side_shadow = ((u - 0.5).abs() * 2.0).powf(1.7) * 0.12;
            let center_lift = (1.0 - smoothstep(0.0, 0.95, (u - 0.5).abs() * 2.0)) * 0.035;
            let grain = stable_pixel_noise(x as u32, y as u32, panel_index as u32) * 0.008;
            r = (r - side_shadow + center_lift + grain).clamp(0.0, 1.0);
            g = (g - side_shadow + center_lift + grain).clamp(0.0, 1.0);
            b = (b - side_shadow * 0.75 + center_lift + grain).clamp(0.0, 1.0);

            blend_pixel(
                pixmap,
                x,
                y,
                [
                    to_color_byte(r),
                    to_color_byte(g),
                    to_color_byte(b),
                    rgba[3],
                ],
            );
        }
    }

    Ok(())
}

fn draw_dark_gradient_rect(
    pixmap: &mut Pixmap,
    node: &RectNode,
    index: usize,
) -> ActionableResult<()> {
    let rgba = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let Some(bounds) = IntClip::from_rect(&node.box_rect, pixmap.width(), pixmap.height()) else {
        return Ok(());
    };

    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let u =
                ((x as f64 + 0.5 - node.box_rect.x) / node.box_rect.width.max(1.0)).clamp(0.0, 1.0);
            let v = ((y as f64 + 0.5 - node.box_rect.y) / node.box_rect.height.max(1.0))
                .clamp(0.0, 1.0);
            let center_glow =
                1.0 - smoothstep(0.0, 0.88, ((u - 0.54).powi(2) + (v - 0.45).powi(2)).sqrt());
            let diagonal = 1.0 - smoothstep(0.0, 1.0, (u * 0.42 + v * 0.58).abs());
            let edge_falloff = smoothstep(0.18, 0.88, ((u - 0.5).abs() + (v - 0.5).abs()) * 0.9);
            let grain = stable_pixel_noise(x as u32, y as u32, 0) * 0.006;

            let r = 0.002 + center_glow * 0.010 + diagonal * 0.006 - edge_falloff * 0.002 + grain;
            let g = 0.006 + center_glow * 0.016 + diagonal * 0.004 + grain;
            let b = 0.018 + center_glow * 0.038 + (1.0 - v) * 0.010 + grain;

            blend_pixel(
                pixmap,
                x,
                y,
                [
                    to_color_byte(r),
                    to_color_byte(g),
                    to_color_byte(b),
                    rgba[3],
                ],
            );
        }
    }

    Ok(())
}

fn gradient_loop_panel_index(id: &str) -> usize {
    id.strip_prefix("gradient-loop-panel-")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

fn gradient_loop_panel_phase(panel_index: usize, time_seconds: f64, duration_seconds: f64) -> f64 {
    if !time_seconds.is_finite() || !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return 0.0;
    }
    let loop_progress = (time_seconds / duration_seconds).rem_euclid(1.0);
    let carousel_step = loop_progress * 4.0;
    let step_index = carousel_step.floor();
    let step_progress = carousel_step.fract();
    let spring_progress = gentle_ease_in_out_spring_progress(step_progress);
    let offset = panel_index as f64 * 0.173;
    let vertical_shimmer = (std::f64::consts::TAU * (loop_progress * 8.0 + offset)).sin() * 0.055
        + (std::f64::consts::TAU * (loop_progress * 12.0 + offset * 0.61)).sin() * 0.022;
    let direction = if panel_index.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    };
    direction * (((step_index + spring_progress) * 0.5) + vertical_shimmer)
}

fn gentle_ease_in_out_spring_progress(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    let eased = if progress < 0.5 {
        4.0 * progress * progress * progress
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    };
    let spring =
        (std::f64::consts::TAU * progress * 2.0).sin() * (1.0 - progress).powi(2) * progress * 0.12;
    (eased + spring).clamp(0.0, 1.04)
}

fn gradient_loop_panel_stops(panel_index: usize) -> &'static [(f64, [f64; 3])] {
    match panel_index % 5 {
        0 => &[
            (0.00, [0.08, 0.39, 0.58]),
            (0.25, [0.04, 0.23, 0.34]),
            (0.31, [0.83, 0.88, 0.88]),
            (0.50, [0.84, 0.89, 0.88]),
            (0.62, [0.13, 0.57, 0.84]),
            (1.00, [0.11, 0.53, 0.82]),
        ],
        1 => &[
            (0.00, [0.05, 0.26, 0.39]),
            (0.22, [0.01, 0.08, 0.10]),
            (0.46, [0.04, 0.25, 0.38]),
            (0.58, [0.74, 0.88, 0.92]),
            (0.78, [0.46, 0.73, 0.84]),
            (1.00, [0.12, 0.55, 0.82]),
        ],
        2 => &[
            (0.00, [0.25, 0.67, 0.91]),
            (0.05, [0.24, 0.64, 0.86]),
            (0.06, [0.00, 0.06, 0.07]),
            (0.36, [0.01, 0.12, 0.17]),
            (0.56, [0.06, 0.30, 0.45]),
            (0.70, [0.00, 0.08, 0.10]),
            (0.84, [0.02, 0.12, 0.17]),
            (1.00, [0.13, 0.58, 0.85]),
        ],
        3 => &[
            (0.00, [0.12, 0.55, 0.82]),
            (0.22, [0.09, 0.42, 0.62]),
            (0.54, [0.06, 0.29, 0.42]),
            (0.68, [0.12, 0.54, 0.81]),
            (1.00, [0.13, 0.58, 0.86]),
        ],
        _ => &[
            (0.00, [0.67, 0.84, 0.89]),
            (0.16, [0.42, 0.70, 0.84]),
            (0.48, [0.14, 0.58, 0.84]),
            (0.74, [0.17, 0.63, 0.89]),
            (0.80, [0.84, 0.89, 0.88]),
            (1.00, [0.84, 0.88, 0.87]),
        ],
    }
}

fn sample_gradient_loop_stops(stops: &[(f64, [f64; 3])], v: f64) -> [f64; 3] {
    for window in stops.windows(2) {
        let (from_at, from_color) = window[0];
        let (to_at, to_color) = window[1];
        if v <= to_at {
            let local = ((v - from_at) / (to_at - from_at).max(f64::EPSILON)).clamp(0.0, 1.0);
            let t = smoothstep(0.0, 1.0, local);
            return [
                from_color[0] + (to_color[0] - from_color[0]) * t,
                from_color[1] + (to_color[1] - from_color[1]) * t,
                from_color[2] + (to_color[2] - from_color[2]) * t,
            ];
        }
    }

    stops
        .last()
        .map(|(_, color)| *color)
        .unwrap_or([0.0, 0.0, 0.0])
}

fn draw_polygon(pixmap: &mut Pixmap, node: &PolygonNode, index: usize) -> ActionableResult<()> {
    if node.points.len() < 3 {
        return Ok(());
    }

    let Some(path) = polygon_path(&node.points) else {
        return Ok(());
    };
    let fill = parse_color(&node.fill, &format!("nodes[{index}].fill"))?;
    let fill_paint = paint_from_rgba(fill);
    pixmap.fill_path(
        &path,
        &fill_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    if let Some(stroke) = &node.stroke {
        let stroke_rgba = parse_color(stroke, &format!("nodes[{index}].stroke"))?;
        let stroke_paint = paint_from_rgba(stroke_rgba);
        let stroke = Stroke {
            width: node.stroke_width.unwrap_or(1.0) as f32,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
    }

    Ok(())
}

fn draw_line(pixmap: &mut Pixmap, node: &LineNode, index: usize) -> ActionableResult<()> {
    if node.points.len() < 2 {
        return Ok(());
    }

    let rgba = parse_color(&node.stroke, &format!("nodes[{index}].stroke"))?;
    let mut builder = PathBuilder::new();
    builder.move_to(node.points[0].x as f32, node.points[0].y as f32);
    for point in &node.points[1..] {
        builder.line_to(point.x as f32, point.y as f32);
    }

    let Some(path) = builder.finish() else {
        return Ok(());
    };
    let paint = paint_from_rgba(rgba);
    let stroke = Stroke {
        width: node.stroke_width as f32,
        ..Stroke::default()
    };
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    Ok(())
}

fn draw_holographic_logo(
    pixmap: &mut Pixmap,
    node: &HolographicLogoNode,
    render_context: &mut RenderContext,
    index: usize,
    time_seconds: f64,
    duration_seconds: f64,
) -> ActionableResult<()> {
    if node.opacity <= 0.0 {
        return Ok(());
    }

    let cache_key = holographic_logo_mask_cache_key(node, pixmap.width(), pixmap.height());
    if !render_context.logo_mask_cache.contains_key(&cache_key) {
        let mask = build_holographic_logo_mask(pixmap.width(), pixmap.height(), node, index)?;
        render_context
            .logo_mask_cache
            .insert(cache_key.clone(), mask);
    }
    let mask = render_context
        .logo_mask_cache
        .get(&cache_key)
        .expect("holographic logo mask cache populated");
    let Some(bounds) = IntClip::from_rect(&node.box_rect, pixmap.width(), pixmap.height()) else {
        return Ok(());
    };

    for y in bounds.min_y..bounds.max_y {
        for x in bounds.min_x..bounds.max_x {
            let mask_index = ((y as u32 * mask.width() + x as u32) * 4) as usize;
            let coverage = mask.data()[mask_index + 3] as f64 / 255.0;
            if coverage <= 0.0 {
                continue;
            }
            let mask_lighting = holographic_logo_mask_lighting(mask, x, y);
            let rgba = holographic_metal_logo_pixel(
                x as u32,
                y as u32,
                &node.box_rect,
                time_seconds,
                duration_seconds,
                LogoPixelStyle {
                    opacity: coverage * node.opacity,
                    edge_rim: mask_lighting.rim,
                    edge_shadow: mask_lighting.shadow,
                },
            );
            blend_pixel(pixmap, x, y, rgba);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct LogoMaskLighting {
    rim: f64,
    shadow: f64,
}

fn holographic_logo_mask_lighting(mask: &Pixmap, x: i32, y: i32) -> LogoMaskLighting {
    let center = mask_alpha_at(mask, x, y);
    if center <= 0.0 {
        return LogoMaskLighting {
            rim: 0.0,
            shadow: 0.0,
        };
    }

    let near_samples = [
        mask_alpha_at(mask, x - 2, y),
        mask_alpha_at(mask, x + 2, y),
        mask_alpha_at(mask, x, y - 2),
        mask_alpha_at(mask, x, y + 2),
        mask_alpha_at(mask, x - 2, y - 2),
        mask_alpha_at(mask, x + 2, y - 2),
        mask_alpha_at(mask, x - 2, y + 2),
        mask_alpha_at(mask, x + 2, y + 2),
    ];
    let wide_samples = [
        mask_alpha_at(mask, x - 5, y),
        mask_alpha_at(mask, x + 5, y),
        mask_alpha_at(mask, x, y - 5),
        mask_alpha_at(mask, x, y + 5),
        mask_alpha_at(mask, x - 4, y - 4),
        mask_alpha_at(mask, x + 4, y - 4),
        mask_alpha_at(mask, x - 4, y + 4),
        mask_alpha_at(mask, x + 4, y + 4),
    ];
    let near_min = near_samples
        .iter()
        .fold(center, |min, value| min.min(*value));
    let wide_min = wide_samples
        .iter()
        .fold(center, |min, value| min.min(*value));
    let edge = ((1.0 - near_min) * 0.62 + (1.0 - wide_min) * 0.38).clamp(0.0, 1.0);

    let gx = (mask_alpha_at(mask, x + 2, y) - mask_alpha_at(mask, x - 2, y)) * 0.72
        + (mask_alpha_at(mask, x + 5, y) - mask_alpha_at(mask, x - 5, y)) * 0.28;
    let gy = (mask_alpha_at(mask, x, y + 2) - mask_alpha_at(mask, x, y - 2)) * 0.72
        + (mask_alpha_at(mask, x, y + 5) - mask_alpha_at(mask, x, y - 5)) * 0.28;
    let gradient_len = (gx * gx + gy * gy).sqrt();
    if gradient_len <= f64::EPSILON {
        return LogoMaskLighting {
            rim: edge * 0.18,
            shadow: edge * 0.10,
        };
    }

    let nx = -gx / gradient_len;
    let ny = -gy / gradient_len;
    let light_dot = (nx * -0.74 + ny * -0.43).clamp(-1.0, 1.0);
    let rim = edge * (0.28 + light_dot.max(0.0) * 0.72) * gradient_len.clamp(0.0, 1.0);
    let shadow = edge * ((-light_dot).max(0.0) * 0.70 + 0.16) * gradient_len.clamp(0.0, 1.0);

    LogoMaskLighting {
        rim: rim.clamp(0.0, 1.0),
        shadow: shadow.clamp(0.0, 1.0),
    }
}

fn mask_alpha_at(mask: &Pixmap, x: i32, y: i32) -> f64 {
    if x < 0 || y < 0 || x >= mask.width() as i32 || y >= mask.height() as i32 {
        return 0.0;
    }
    let index = ((y as u32 * mask.width() + x as u32) * 4 + 3) as usize;
    mask.data()[index] as f64 / 255.0
}

fn build_holographic_logo_mask(
    width: u32,
    height: u32,
    node: &HolographicLogoNode,
    index: usize,
) -> ActionableResult<Pixmap> {
    let mut mask = Pixmap::new(width, height).ok_or_else(|| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "dimensions",
            "Could not allocate holographic logo mask.",
            "Use supported non-zero graphics dimensions.",
        )]
    })?;
    let paint = paint_from_rgba([255, 255, 255, 255]);
    for (path_index, path_data) in node.path_data.iter().enumerate() {
        let path =
            build_svg_path(path_data, &node.view_box, &node.box_rect).map_err(|message| {
                vec![ActionableError::new(
                    GraphicsErrorCode::GraphicsTemplateParamInvalid,
                    format!("nodes[{index}].pathData[{path_index}]"),
                    message,
                    "Use bounded SVG path data with M, L, H, V, C, and Z commands.",
                )]
            })?;
        mask.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    Ok(mask)
}

fn holographic_logo_mask_cache_key(
    node: &HolographicLogoNode,
    pixmap_width: u32,
    pixmap_height: u32,
) -> String {
    format!(
        "{}:{}x{}:{:.3},{:.3},{:.3},{:.3}:{:.3},{:.3},{:.3},{:.3}:{}",
        node.asset_id,
        pixmap_width,
        pixmap_height,
        node.view_box.x,
        node.view_box.y,
        node.view_box.width,
        node.view_box.height,
        node.box_rect.x,
        node.box_rect.y,
        node.box_rect.width,
        node.box_rect.height,
        node.path_data.join("|")
    )
}

#[derive(Debug, Clone, Copy)]
struct HolographicMetalShaderParams {
    iridescence: f64,
    shimmer_speed: f64,
    glossiness: f64,
    sparkle: f64,
    band_scale: f64,
    edge_threshold: f64,
    edge_noise_amount: f64,
    edge_noise_scale: f64,
    edge_noise_speed: f64,
}

#[derive(Debug, Clone, Copy)]
struct LogoPixelStyle {
    opacity: f64,
    edge_rim: f64,
    edge_shadow: f64,
}

impl HolographicMetalShaderParams {
    const REFERENCE: Self = Self {
        iridescence: 0.68,
        shimmer_speed: 2.45,
        glossiness: 0.65,
        sparkle: 0.08,
        band_scale: 6.2,
        edge_threshold: 0.25,
        edge_noise_amount: 0.70,
        edge_noise_scale: 1.20,
        edge_noise_speed: 1.0,
    };
}

fn holographic_metal_logo_pixel(
    x: u32,
    y: u32,
    rect: &Rect,
    time_seconds: f64,
    duration_seconds: f64,
    style: LogoPixelStyle,
) -> [u8; 4] {
    let LogoPixelStyle {
        opacity,
        edge_rim,
        edge_shadow,
    } = style;
    let params = HolographicMetalShaderParams::REFERENCE;
    let local_u = ((x as f64 + 0.5 - rect.x) / rect.width.max(1.0)).clamp(0.0, 1.0);
    let local_v = ((y as f64 + 0.5 - rect.y) / rect.height.max(1.0)).clamp(0.0, 1.0);
    let time = time_seconds.max(0.0);
    let progress = if duration_seconds > 0.0 {
        (time_seconds / duration_seconds).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let aspect = rect.width.max(1.0) / rect.height.max(1.0);
    let centered_x = (local_u - 0.5) * aspect;
    let centered_y = local_v - 0.5;
    let organic_time =
        time + (time * 0.43).sin() * 0.090 + (time * 0.79 + 1.7).sin() * 0.052 + progress * 0.120;
    let slow_time = organic_time * params.shimmer_speed;

    let warp = domain_warp_2d(
        local_u * 1.10 + slow_time * 0.040,
        local_v * 1.00 - slow_time * 0.032,
        slow_time * 0.115,
        901,
        0.44,
    );
    let micro_warp = domain_warp_2d(
        local_u * 2.35 - slow_time * 0.030,
        local_v * 2.10 + slow_time * 0.026,
        slow_time * 0.165 + 3.2,
        907,
        0.14,
    );
    let warped_u = warp.x + micro_warp.x * 0.055;
    let warped_v = warp.y + micro_warp.y * 0.055;

    let flow_a = fbm_3d(warped_u * 1.25, warped_v * 1.12, slow_time * 0.140, 911, 5);
    let flow_b = fbm_3d(
        warped_u * 2.10 + 4.1,
        warped_v * 1.85 - 2.6,
        slow_time * 0.135 + 1.8,
        919,
        5,
    );
    let turbulence = turbulence_3d(warped_u * 1.70, warped_v * 1.45, slow_time * 0.135, 923, 5);
    let organic_field = (flow_a * 0.62 + flow_b * 0.26 + turbulence * 0.12).clamp(0.0, 1.0);

    let angle = 0.56 + (flow_a - 0.5) * 0.30 + (slow_time * 0.09).sin() * 0.035;
    let (band_sin, band_cos) = angle.sin_cos();
    let coord = centered_x * band_cos + centered_y * band_sin;
    let curved_coord = coord
        + (flow_a - 0.5) * 0.68
        + (flow_b - 0.5) * 0.42
        + (turbulence - 0.5) * 0.18
        + (centered_x * 1.9 + centered_y * 0.7 + slow_time * 0.12).sin() * 0.030;

    let band_phase = curved_coord * params.band_scale
        + organic_field * 2.20
        + (turbulence - 0.5) * 1.10
        + warp.strength * 0.70
        + slow_time * 0.35
        + edge_rim * 0.22;
    let irid1 = holographic_spectrum(band_phase * 0.50);
    let irid2 = holographic_spectrum(band_phase * 0.50 + 0.18 + (flow_b - 0.5) * 0.04);
    let raw_irid = [
        (irid1[0] + irid2[0]) * 0.5,
        (irid1[1] + irid2[1]) * 0.5,
        (irid1[2] + irid2[2]) * 0.5,
    ];
    let irid = [
        0.74 + (raw_irid[0] - 0.5) * 0.72,
        0.78 + (raw_irid[1] - 0.5) * 0.66,
        0.88 + (raw_irid[2] - 0.5) * 0.74,
    ];

    let grad_top = 1.0 - local_v;
    let grad = smoothstep(0.0, 1.0, grad_top);
    let brushed_field = fbm_2d(warped_u * 1.10 + slow_time * 0.025, warped_v * 1.05, 929, 4);
    let metal_value = (0.50
        + grad * 0.20
        + (organic_field - 0.5) * 0.14
        + (brushed_field - 0.5) * 0.08
        + edge_rim * 0.08
        - edge_shadow * 0.08)
        .clamp(0.08, 1.0);
    let mut metal_base = [
        0.16 + metal_value * 0.78,
        0.18 + metal_value * 0.79,
        0.22 + metal_value * 0.82,
    ];
    let luma_tint = [metal_value * 1.02, metal_value, metal_value * 1.05];
    for channel in 0..3 {
        metal_base[channel] = metal_base[channel] * 0.72 + luma_tint[channel] * 0.28;
    }

    let mut holo = [
        metal_base[0] * (1.0 - params.iridescence) + irid[0] * params.iridescence,
        metal_base[1] * (1.0 - params.iridescence) + irid[1] * params.iridescence,
        metal_base[2] * (1.0 - params.iridescence) + irid[2] * params.iridescence,
    ];

    let spec_phase =
        curved_coord * params.band_scale * 0.50 + organic_field * 1.05 + slow_time * 0.20;
    let spec_distance = spec_phase - (spec_phase + 0.5).floor();
    let broad_spec = (-(spec_distance * spec_distance) * 22.0).exp();
    let narrow_spec = (-(spec_distance * spec_distance) * 96.0).exp();
    let fresnel = (1.0
        - ((centered_x * centered_x + centered_y * centered_y).sqrt() * 1.1).clamp(0.0, 1.0))
    .powf(2.5);
    let gloss_breakup = (0.80 + organic_field * 0.24 - turbulence * 0.08).clamp(0.62, 1.05);
    let gloss = (broad_spec * 0.58 + narrow_spec * 0.50 + fresnel * 0.34).clamp(0.0, 1.0)
        * params.glossiness
        * gloss_breakup;
    holo[0] += gloss * 1.00;
    holo[1] += gloss * 0.98;
    holo[2] += gloss * 0.95;

    let sparkle_noise = stable_pixel_noise(
        x.wrapping_add((time * 14.0).floor() as u32 * 13),
        y.wrapping_add((time * 11.0).floor() as u32 * 17),
        919,
    );
    let sparkle_gate = smoothstep(0.62, 0.94, organic_field);
    let sparkle = smoothstep(0.992, 1.0, sparkle_noise) * params.sparkle * 0.10 * sparkle_gate;
    holo[0] += sparkle;
    holo[1] += sparkle;
    holo[2] += sparkle;

    let edge_distance = local_u.min(1.0 - local_u).min(local_v.min(1.0 - local_v));
    let edge_fade = smoothstep(0.0, 0.18, edge_distance);
    for channel in &mut holo {
        *channel *= 0.85 * (1.0 - edge_fade) + edge_fade;
    }

    let edge_noise = fbm_3d(
        local_u * params.edge_noise_scale + slow_time * params.edge_noise_speed * 0.10,
        local_v * params.edge_noise_scale + slow_time * params.edge_noise_speed * 0.07,
        slow_time * 0.12,
        929,
        4,
    );
    let noise_mask = (1.0 - params.edge_noise_amount)
        + params.edge_noise_amount
            * smoothstep(
                1.0 - params.edge_noise_amount,
                1.0,
                edge_noise + (1.0 - params.edge_noise_amount),
            );
    let edge_blink = smoothstep(
        params.edge_threshold,
        params.edge_threshold + 0.15,
        edge_rim,
    ) * noise_mask;
    holo[0] += edge_blink * 0.18;
    holo[1] += edge_blink * 0.17;
    holo[2] += edge_blink * 0.16;

    let roughness = metal_imperfection_texture(warped_u, warped_v, slow_time);
    let brushed = anisotropic_scratch_texture(warped_u, warped_v, slow_time) * 0.009;
    let smooth_imperfection = (roughness - 0.34) * 0.014 + (turbulence - 0.5) * 0.010;
    holo[0] += brushed + smooth_imperfection;
    holo[1] += brushed * 0.92 + smooth_imperfection * 0.96;
    holo[2] += brushed * 0.85 + smooth_imperfection * 0.92;

    let neutral = (holo[0] + holo[1] + holo[2]) / 3.0;
    let highlight = smoothstep(0.76, 1.12, neutral);
    let shadow = 1.0 - smoothstep(0.18, 0.58, neutral);
    let saturation_trim = 0.06 + shadow * 0.18 - highlight * 0.10;
    for channel in &mut holo {
        *channel = *channel * (1.0 - saturation_trim) + neutral * saturation_trim;
    }

    let contrast = 1.16 + narrow_spec * 0.09 + roughness * 0.018;
    for channel in &mut holo {
        *channel = 0.56 + (*channel - 0.56) * contrast;
        *channel = channel.clamp(0.0, 1.20);
    }

    [
        to_holographic_color_byte(holo[0]),
        to_holographic_color_byte(holo[1]),
        to_holographic_color_byte(holo[2]),
        to_color_byte(opacity),
    ]
}

#[allow(dead_code)]
fn holographic_logo_pixel(
    x: u32,
    y: u32,
    rect: &Rect,
    time_seconds: f64,
    duration_seconds: f64,
    style: LogoPixelStyle,
) -> [u8; 4] {
    let LogoPixelStyle {
        opacity,
        edge_rim,
        edge_shadow,
    } = style;
    let progress = if duration_seconds > 0.0 {
        (time_seconds / duration_seconds).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let u = ((x as f64 + 0.5 - rect.x) / rect.width.max(1.0)).clamp(0.0, 1.0);
    let v = ((y as f64 + 0.5 - rect.y) / rect.height.max(1.0)).clamp(0.0, 1.0);
    let time = time_seconds.max(0.0);
    let organic_time = time + 0.18 * (time * 0.72).sin() + 0.08 * (time * 1.37 + 0.6).sin();
    let drift = organic_time * 0.24 + progress * 0.08;
    let flow_a = value_noise_2d(u * 2.1 + organic_time * 0.12, v * 1.8 - time * 0.08, 101);
    let flow_b = value_noise_2d(u * 1.7 - time * 0.10, v * 2.3 + organic_time * 0.11, 103);
    let flow_c = value_noise_2d(u * 3.7 + time * 0.07, v * 3.0 - organic_time * 0.09, 107);
    let flow_u = u
        + (flow_a - 0.5) * 0.16
        + 0.028 * ((v * 1.45 + organic_time * 0.10) * std::f64::consts::TAU).sin();
    let flow_v = v
        + (flow_b - 0.5) * 0.14
        + 0.024 * ((u * 1.18 - organic_time * 0.08) * std::f64::consts::TAU).cos();
    let low_noise = value_noise_2d(
        flow_u * 3.0 + organic_time * 0.14,
        flow_v * 2.5 - time * 0.09,
        11,
    );
    let mid_noise = value_noise_2d(
        flow_u * 5.2 - time * 0.17,
        flow_v * 4.2 + organic_time * 0.13,
        23,
    );
    let organic_warp = 0.088
        * ((flow_v * 1.48 + organic_time * 0.095) * std::f64::consts::TAU).sin()
        + 0.064 * ((flow_u * 1.10 - organic_time * 0.082) * std::f64::consts::TAU).cos()
        + 0.042 * (((flow_u + flow_v) * 1.04 + time * 0.060) * std::f64::consts::TAU).sin()
        + (low_noise - 0.5) * 0.26
        + (mid_noise - 0.5) * 0.13
        + (flow_c - 0.5) * 0.09;
    let wave_phase = flow_u * 0.98 - flow_v * 0.72 + drift + organic_warp;
    let primary_wave = 0.5 + 0.5 * (wave_phase * std::f64::consts::TAU).sin();
    let secondary_wave = 0.5
        + 0.5
            * ((flow_u * 0.46 + flow_v * 0.88 - organic_time * 0.16 + organic_warp * 0.55)
                * std::f64::consts::TAU)
                .sin();
    let tertiary_wave = 0.5
        + 0.5
            * ((flow_u * 1.24 - flow_v * 0.35 + time * 0.09 + (flow_c - 0.5) * 0.38)
                * std::f64::consts::TAU)
                .sin();
    let broad_band = 0.5
        + 0.5
            * ((flow_u * 1.12 - flow_v * 0.74 + organic_time * 0.055 + low_noise * 0.18)
                * std::f64::consts::TAU)
                .sin();
    let broad_shadow = (1.0 - smoothstep(0.24, 0.62, broad_band)).powf(0.86);
    let broad_highlight = smoothstep(0.55, 0.88, broad_band).powf(0.76);
    let wave = (primary_wave * 0.66 + secondary_wave * 0.24 + tertiary_wave * 0.10).clamp(0.0, 1.0);
    let shadow_wave = (1.0 - smoothstep(0.18, 0.70, wave)).powf(0.74);
    let highlight_wave = smoothstep(0.44, 0.90, wave).powf(0.74);
    let specular_ridge = 1.0
        - smoothstep(
            0.022,
            0.110,
            (fract(wave_phase + secondary_wave * 0.08 + (flow_c - 0.5) * 0.05) - 0.54).abs(),
        );
    let cyan_lobe = (gaussian_2d(
        u,
        v,
        0.24 + (organic_time * 0.72).sin() * 0.040,
        0.34 + (time * 0.46).cos() * 0.026,
        0.24,
        0.30,
    ) + gaussian_2d(u, v, 0.65 + (time * 0.30).sin() * 0.024, 0.66, 0.22, 0.24)
        * 0.55)
        .clamp(0.0, 1.0);
    let lavender_lobe =
        (gaussian_2d(
            u,
            v,
            0.45 + (organic_time * 0.35).cos() * 0.030,
            0.55 + (time * 0.70).cos() * 0.038,
            0.22,
            0.24,
        ) + gaussian_2d(u, v, 0.78, 0.38 + (time * 0.42).sin() * 0.020, 0.20, 0.26) * 0.50)
            .clamp(0.0, 1.0);
    let rose_lobe = gaussian_2d(
        u,
        v,
        0.18 + (time * 0.40).sin() * 0.024,
        0.78 + (organic_time * 0.32).cos() * 0.022,
        0.24,
        0.22,
    )
    .clamp(0.0, 1.0);
    let frost_zone = (gaussian_2d(u, v, 0.20, 0.70, 0.30, 0.26)
        + gaussian_2d(u, v, 0.38, 0.58, 0.28, 0.22) * 0.70)
        .clamp(0.0, 1.0);
    let mint_lobe = (gaussian_2d(
        u,
        v,
        0.20 + (organic_time * 0.28).sin() * 0.030,
        0.52 + (time * 0.34).cos() * 0.026,
        0.28,
        0.30,
    ) + gaussian_2d(u, v, 0.55, 0.42, 0.30, 0.24) * 0.38)
        .clamp(0.0, 1.0);
    let pearl_shear = 0.5
        + 0.5
            * ((flow_u * 1.62 + flow_v * 0.42 + low_noise * 0.34 - organic_time * 0.10)
                * std::f64::consts::TAU)
                .sin();
    let oil_slick = 0.5
        + 0.5
            * ((flow_u * 0.50 - flow_v * 1.06 + mid_noise * 0.42 + organic_time * 0.12)
                * std::f64::consts::TAU)
                .cos();
    let pearl_roll = 0.5
        + 0.5
            * ((flow_u * 0.84 + flow_v * 0.46 + organic_time * 0.14) * std::f64::consts::TAU).cos();
    let fine_wave = value_noise_2d(flow_u * 9.2 + time * 0.46, flow_v * 7.6 - time * 0.34, 37);
    let caustic = value_noise_2d(
        flow_u * 6.4 - time * 0.28,
        flow_v * 5.0 + organic_time * 0.24,
        43,
    );
    let metal_roughness = metal_imperfection_texture(flow_u, flow_v, organic_time);
    let scratch_roughness = anisotropic_scratch_texture(flow_u, flow_v, organic_time);
    let surface_roughness = (metal_roughness * 0.94 + scratch_roughness * 0.82).clamp(0.0, 1.0);
    let thin_film = 0.5
        + 0.5
            * ((flow_u * 1.18 + flow_v * 0.74 + organic_time * 0.16 + surface_roughness * 0.44)
                * std::f64::consts::TAU)
                .sin();
    let pearl_cloud = (value_noise_3d(
        flow_u * 3.8 - organic_time * 0.08,
        flow_v * 3.2 + time * 0.06,
        organic_time * 0.42 + low_noise * 0.35,
        353,
    ) * 0.62
        + value_noise_3d(
            flow_u * 8.4 + time * 0.12,
            flow_v * 6.8 - organic_time * 0.10,
            organic_time * 0.68 + mid_noise * 0.45,
            359,
        ) * 0.38)
        .clamp(0.0, 1.0);
    let nacre_bloom = smoothstep(0.34, 0.88, pearl_cloud)
        * (0.36 + highlight_wave * 0.46 + (1.0 - shadow_wave) * 0.18);
    let cloudy_occlusion =
        smoothstep(0.22, 0.62, 1.0 - pearl_cloud) * (0.006 + surface_roughness * 0.018);
    let contour = 1.0
        - smoothstep(
            0.12,
            0.82,
            ((u - 0.46).powi(2) * 0.75 + (v - 0.48).powi(2) * 1.35).sqrt(),
        );
    let sparkle_frame = (time_seconds * 8.0).floor() as u32;
    let pixel_noise = stable_pixel_noise(x, y, 0);
    let sparkle_seed = stable_pixel_noise(
        x.wrapping_mul(7).wrapping_add(13),
        y.wrapping_mul(11).wrapping_add(29),
        sparkle_frame.wrapping_add(17),
    );
    let noise = value_noise_2d(
        flow_u * 42.0 + organic_time * 0.18,
        flow_v * 36.0 - time * 0.13,
        613,
    );
    let fine_noise = value_noise_2d(
        flow_u * 76.0 - time * 0.10,
        flow_v * 62.0 + organic_time * 0.12,
        617,
    );
    let pore_noise = value_noise_2d(
        flow_u * 108.0 + organic_time * 0.08,
        flow_v * 86.0 - time * 0.07,
        619,
    );
    let light_pore = if pore_noise > 0.82 {
        (pore_noise - 0.82) / 0.18
    } else {
        0.0
    };
    let dark_pore = if fine_noise < 0.16 {
        (0.16 - fine_noise) / 0.16
    } else {
        0.0
    };
    let micro_grain =
        (pixel_noise - 0.5) * 0.006 + (noise - 0.5) * 0.012 + (fine_noise - 0.5) * 0.016;
    let material_grit = ((fine_noise - 0.5) * 0.018 + (pore_noise - 0.5) * 0.012)
        * (0.20 + highlight_wave * 0.30 + frost_zone * 0.18)
        + (surface_roughness - 0.45) * (0.018 + highlight_wave * 0.026);
    let frost_grain = frost_zone
        * ((fine_noise - 0.5) * 0.018 + (noise - 0.5) * 0.016 + (mid_noise - 0.5) * 0.024);
    let frosted_pores =
        frost_zone * (shadow_wave * 0.48 + 0.22) * (light_pore * 0.020 - dark_pore * 0.012)
            + (0.18 + highlight_wave * 0.38) * (light_pore * 0.014 - dark_pore * 0.016);
    let sparkle = if sparkle_seed > 0.992 && pore_noise > 0.78 {
        (sparkle_seed - 0.992) * 2.6
    } else {
        0.0
    };
    let aspect = rect.width.max(1.0) / rect.height.max(1.0);
    let centered_x = (u - 0.5) * aspect;
    let centered_y = v - 0.5;
    let (band_sin, band_cos) = 0.6_f64.sin_cos();
    let diagonal_coord = centered_x * band_cos + centered_y * band_sin;
    let shader_warp = ((low_noise - 0.5) * 0.76
        + (mid_noise - 0.5) * 0.38
        + (pearl_cloud - 0.5) * 0.52
        + (surface_roughness - 0.5) * 0.24)
        .clamp(-1.0, 1.0);
    let band_scale = 7.0;
    let shimmer_speed = 1.2;
    let shader_luma =
        (0.28 + highlight_wave * 0.46 + broad_highlight * 0.36 + specular_ridge * 0.28
            - shadow_wave * 0.18
            - broad_shadow * 0.34)
            .clamp(0.0, 1.0);
    let band_phase = diagonal_coord * band_scale
        + shader_warp * 0.60
        + time * shimmer_speed * 0.35
        + shader_luma * 1.20;
    let irid1 = holographic_spectrum(band_phase * 0.5);
    let irid2 = holographic_spectrum(band_phase * 0.5 + 0.18);
    let pearlescent_irid = [
        0.74 + (((irid1[0] + irid2[0]) * 0.5) - 0.5) * 0.36,
        0.77 + (((irid1[1] + irid2[1]) * 0.5) - 0.5) * 0.32,
        0.83 + (((irid1[2] + irid2[2]) * 0.5) - 0.5) * 0.40,
    ];
    let spec_phase =
        diagonal_coord * band_scale * 0.5 + shader_warp * 0.40 + time * shimmer_speed * 0.20;
    let spec_distance = spec_phase - (spec_phase + 0.5).floor();
    let spectral_spec = (-(spec_distance * spec_distance) * 28.0).exp();
    let fresnel = (1.0
        - ((centered_x * centered_x + centered_y * centered_y).sqrt() * 1.1).clamp(0.0, 1.0))
    .powf(2.5);
    let shader_gloss = (spectral_spec * 1.12 + fresnel * 0.66).clamp(0.0, 1.0) * 0.78;
    let edge_noise = value_noise_2d(
        u * 1.2 + organic_time * 0.10,
        v * 1.2 + organic_time * 0.07,
        503,
    );
    let noise_edge_mask = smoothstep(0.42, 0.96, edge_noise + surface_roughness * 0.22);
    let masked_edge = edge_rim * (0.34 + noise_edge_mask * 0.66);
    let metal_grad = smoothstep(0.0, 1.0, 1.0 - v);
    let source_luma =
        (0.42 + highlight_wave * 0.26 + broad_highlight * 0.32 + specular_ridge * 0.20
            - shadow_wave * 0.14
            - broad_shadow * 0.34
            + nacre_bloom * 0.10)
            .clamp(0.08, 1.0);
    let mut metal_base = [
        0.06 + (0.99 - 0.06) * metal_grad,
        0.08 + (1.01 - 0.08) * metal_grad,
        0.13 + (1.04 - 0.13) * metal_grad,
    ];
    let luma_tint = [source_luma * 1.02, source_luma, source_luma * 1.05];
    for channel in 0..3 {
        metal_base[channel] = metal_base[channel] * 0.72 + luma_tint[channel] * 0.28;
    }
    let iridescence =
        (0.36 + (1.0 - highlight_wave) * 0.08 + surface_roughness * 0.03).clamp(0.32, 0.52);
    let mut shader_metal = [
        metal_base[0] * (1.0 - iridescence) + pearlescent_irid[0] * iridescence,
        metal_base[1] * (1.0 - iridescence) + pearlescent_irid[1] * iridescence,
        metal_base[2] * (1.0 - iridescence) + pearlescent_irid[2] * iridescence,
    ];
    let pitted_shadow = smoothstep(0.18, 0.86, surface_roughness)
        * (0.035 + shadow_wave * 0.030 + (1.0 - highlight_wave) * 0.020);
    let brushed_glint = smoothstep(0.50, 0.98, surface_roughness)
        * (0.060 + spectral_spec * 0.120 + specular_ridge * 0.070);
    let sparkle_noise = smoothstep(0.92, 1.0, fine_noise) * 0.025;
    let micro_etch = ((fine_noise - 0.5) * 0.014
        + (pore_noise - 0.5) * 0.012
        + (surface_roughness - 0.38) * 0.046)
        * (0.42 + frost_zone * 0.22 + shadow_wave * 0.08);
    let organic_contrast = highlight_wave * 0.082 + spectral_spec * 0.070 - shadow_wave * 0.048
        + (pearl_cloud - 0.5) * 0.040;
    shader_metal[0] += shader_gloss * 1.00 + brushed_glint + sparkle_noise + masked_edge * 0.190;
    shader_metal[1] +=
        shader_gloss * 0.98 + brushed_glint * 0.92 + sparkle_noise + masked_edge * 0.170;
    shader_metal[2] +=
        shader_gloss * 0.95 + brushed_glint * 0.86 + sparkle_noise + masked_edge * 0.145;
    shader_metal[0] += organic_contrast + micro_etch * 0.92;
    shader_metal[1] += organic_contrast + micro_etch * 0.98;
    shader_metal[2] += organic_contrast + micro_etch * 1.04;
    shader_metal[0] -= pitted_shadow * 0.55 + edge_shadow * 0.032;
    shader_metal[1] -= pitted_shadow * 0.50 + edge_shadow * 0.036;
    shader_metal[2] -= pitted_shadow * 0.44 + edge_shadow * 0.040;
    let shader_contrast = 1.20 + surface_roughness * 0.050 + spectral_spec * 0.080;
    for channel in &mut shader_metal {
        *channel = 0.56 + (*channel - 0.56) * shader_contrast;
    }

    let silk = (fine_wave * 0.020
        + caustic * 0.022
        + pearl_roll * 0.040
        + pearl_shear * 0.052
        + sparkle * 0.045)
        .clamp(0.0, 0.28);
    let roughness_trench =
        surface_roughness * (0.022 + highlight_wave * 0.036 + (1.0 - shadow_wave) * 0.012);
    let roughness_glint = smoothstep(0.46, 0.96, surface_roughness)
        * (0.050 + specular_ridge * 0.044 + highlight_wave * 0.028);
    let luminance = 0.49
        + highlight_wave * 0.24
        + broad_highlight * 0.35
        + specular_ridge * 0.180
        + pearl_roll * 0.032
        - shadow_wave * 0.15
        - broad_shadow * 0.34
        + contour * 0.028
        + silk
        + frost_grain * 0.36
        + material_grit
        - roughness_trench
        + roughness_glint
        + nacre_bloom * 0.060
        - cloudy_occlusion
        + edge_rim * (0.038 + specular_ridge * 0.030)
        - edge_shadow * 0.050;
    let luminance = luminance + frosted_pores;
    let chroma_boost =
        (0.86 + shadow_wave * 0.16 + (1.0 - highlight_wave) * 0.12 + oil_slick * 0.10)
            .clamp(0.0, 1.28);

    let mut r = luminance - cyan_lobe * 0.115 * chroma_boost
        + lavender_lobe * 0.145 * chroma_boost
        + rose_lobe * 0.105 * chroma_boost
        + (oil_slick - 0.5) * 0.085
        + (thin_film - 0.5) * 0.070
        + nacre_bloom * 0.058
        + roughness_glint * 0.080
        + edge_rim * 0.052
        - shadow_wave * 0.018
        - surface_roughness * 0.010
        - edge_shadow * 0.026
        + micro_grain;
    let mut g = luminance + cyan_lobe * 0.142 * chroma_boost + mint_lobe * 0.130 * chroma_boost
        - lavender_lobe * 0.052 * chroma_boost
        - rose_lobe * 0.032 * chroma_boost
        + (pearl_shear - 0.5) * 0.070
        + (0.5 - thin_film).abs() * 0.036
        + nacre_bloom * 0.046
        + roughness_glint * 0.040
        + edge_rim * 0.036
        + (1.0 - v) * 0.030
        - edge_shadow * 0.024
        + micro_grain;
    let mut b = luminance
        + cyan_lobe * 0.215 * chroma_boost
        + lavender_lobe * 0.205 * chroma_boost
        + rose_lobe * 0.025 * chroma_boost
        + (1.0 - oil_slick) * 0.074
        + (0.5 - thin_film) * 0.095
        + nacre_bloom * 0.024
        + shadow_wave * 0.030
        + edge_rim * 0.020
        - surface_roughness * 0.018
        - edge_shadow * 0.034
        + u * 0.025
        + micro_grain;

    let lift = specular_ridge * 0.046 + sparkle * 0.032;
    r += lift;
    g += lift;
    b += lift;

    let film_neutral = (pearlescent_irid[0] + pearlescent_irid[1] + pearlescent_irid[2]) / 3.0;
    let reflected_shadow = (gaussian_2d(
        u,
        v,
        0.31 + (organic_time * 0.18).sin() * 0.025,
        0.55 + (time * 0.15).cos() * 0.022,
        0.24,
        0.32,
    ) * 0.68
        + gaussian_2d(
            u,
            v,
            0.70 + (time * 0.12).sin() * 0.020,
            0.30 + (organic_time * 0.16).cos() * 0.018,
            0.26,
            0.26,
        ) * 0.42
        + (1.0 - pearl_cloud) * 0.18)
        .clamp(0.0, 1.0);
    let reflected_light = (gaussian_2d(
        u,
        v,
        0.22 + (time * 0.16).sin() * 0.026,
        0.30 + (organic_time * 0.13).cos() * 0.018,
        0.22,
        0.24,
    ) * 0.72
        + gaussian_2d(
            u,
            v,
            0.58 + (organic_time * 0.14).cos() * 0.024,
            0.62 + (time * 0.12).sin() * 0.020,
            0.24,
            0.24,
        ) * 0.58
        + gaussian_2d(u, v, 0.86, 0.34, 0.20, 0.22) * 0.36)
        .clamp(0.0, 1.0);
    let soft_shadow = smoothstep(
        0.18,
        0.82,
        reflected_shadow * 0.78 + (1.0 - broad_band) * 0.22,
    )
    .powf(1.10);
    let soft_light = smoothstep(0.16, 0.92, reflected_light * 0.78 + broad_band * 0.22).powf(0.82);
    let ribbon_phase = diagonal_coord * 2.65 + shader_warp * 0.08 + organic_time * 0.035;
    let spec_distance = (fract(ribbon_phase + 0.20) - 0.50).abs();
    let satin_spec = (-(spec_distance * spec_distance) * 42.0).exp()
        * (0.11 + soft_light * 0.12 + (1.0 - soft_shadow) * 0.05);
    let soft_fresnel = fresnel * 0.12 + edge_rim * 0.045;
    let pearl_lift =
        smoothstep(0.22, 0.86, pearl_shear) * 0.070 + smoothstep(0.30, 0.92, oil_slick) * 0.050;
    let polished_luma =
        (0.62 + soft_light * 0.36 + satin_spec + specular_ridge * 0.075 + soft_fresnel
            - soft_shadow * 0.20
            + pearl_lift
            + (pearl_cloud - 0.5) * 0.030
            + (surface_roughness - 0.5) * 0.018)
            .clamp(0.40, 1.12);

    let shadow_cool = soft_shadow * 0.028;
    let highlight_warm = soft_light * 0.018;
    let film_strength =
        (0.125 + soft_light * 0.145 + satin_spec * 0.090 + (1.0 - soft_shadow) * 0.030)
            .clamp(0.090, 0.320);
    let color_gate = (1.0 - soft_shadow * 0.28).clamp(0.70, 1.0);
    let mut target_r = polished_luma * (0.965 + highlight_warm - shadow_cool * 0.45);
    let mut target_g = polished_luma * (0.975 - shadow_cool * 0.20);
    let mut target_b = polished_luma * (1.000 + shadow_cool * 0.25);
    target_r += ((pearlescent_irid[0] - film_neutral) * film_strength
        + lavender_lobe * 0.070
        + rose_lobe * 0.046
        + (thin_film - 0.5) * 0.040
        - cyan_lobe * 0.020)
        * color_gate;
    target_g += ((pearlescent_irid[1] - film_neutral) * film_strength
        + cyan_lobe * 0.064
        + mint_lobe * 0.054
        + (pearl_shear - 0.5) * 0.036
        - rose_lobe * 0.012)
        * color_gate;
    target_b += ((pearlescent_irid[2] - film_neutral) * film_strength
        + lavender_lobe * 0.084
        + cyan_lobe * 0.076
        + (0.5 - thin_film) * 0.052)
        * color_gate;

    let source_texture_weight = (0.10 + surface_roughness * 0.02).clamp(0.08, 0.15);
    r = target_r * (1.0 - source_texture_weight) + r * source_texture_weight;
    g = target_g * (1.0 - source_texture_weight) + g * source_texture_weight;
    b = target_b * (1.0 - source_texture_weight) + b * source_texture_weight;

    let final_neutral = (r + g + b) / 3.0;
    let final_desaturate = (0.025 + soft_shadow * 0.120).clamp(0.02, 0.18);
    r = r * (1.0 - final_desaturate) + final_neutral * final_desaturate;
    g = g * (1.0 - final_desaturate) + final_neutral * final_desaturate;
    b = b * (1.0 - final_desaturate) + final_neutral * final_desaturate;

    let final_contrast = 1.16 + satin_spec * 0.08;
    r = 0.62 + (r - 0.62) * final_contrast;
    g = 0.62 + (g - 0.62) * final_contrast;
    b = 0.62 + (b - 0.62) * final_contrast;

    [
        to_holographic_color_byte(r),
        to_holographic_color_byte(g),
        to_holographic_color_byte(b),
        to_color_byte(opacity),
    ]
}

fn build_svg_path(
    path_data: &str,
    view_box: &Rect,
    box_rect: &Rect,
) -> Result<tiny_skia::Path, String> {
    if view_box.width <= 0.0
        || view_box.height <= 0.0
        || box_rect.width <= 0.0
        || box_rect.height <= 0.0
    {
        return Err("Holographic logo viewBox and box must have positive dimensions.".to_string());
    }

    let tokens = tokenize_svg_path(path_data)?;
    let mut cursor = SvgPathCursor::new(tokens);
    let mut builder = PathBuilder::new();
    let mut command = None;
    let mut current = (0.0, 0.0);
    let mut subpath_start = (0.0, 0.0);
    let mut open_subpath = false;

    while !cursor.is_done() {
        if let Some(next_command) = cursor.take_command() {
            command = Some(next_command);
        }
        let command_char = command
            .ok_or_else(|| "SVG path data must start with a supported path command.".to_string())?;
        let relative = command_char.is_ascii_lowercase();

        match command_char.to_ascii_uppercase() {
            'M' => {
                let mut first_pair = true;
                while cursor.next_is_number() {
                    let (mut x, mut y) = cursor.take_pair()?;
                    if relative {
                        x += current.0;
                        y += current.1;
                    }
                    let mapped = map_svg_point(x, y, view_box, box_rect);
                    if first_pair {
                        if open_subpath {
                            builder.close();
                        }
                        builder.move_to(mapped.0, mapped.1);
                        subpath_start = (x, y);
                        open_subpath = true;
                        first_pair = false;
                    } else {
                        builder.line_to(mapped.0, mapped.1);
                    }
                    current = (x, y);
                }
                command = Some(if relative { 'l' } else { 'L' });
            }
            'L' => {
                while cursor.next_is_number() {
                    let (mut x, mut y) = cursor.take_pair()?;
                    if relative {
                        x += current.0;
                        y += current.1;
                    }
                    let mapped = map_svg_point(x, y, view_box, box_rect);
                    builder.line_to(mapped.0, mapped.1);
                    current = (x, y);
                    open_subpath = true;
                }
            }
            'H' => {
                while cursor.next_is_number() {
                    let mut x = cursor.take_number()?;
                    if relative {
                        x += current.0;
                    }
                    current.0 = x;
                    let mapped = map_svg_point(current.0, current.1, view_box, box_rect);
                    builder.line_to(mapped.0, mapped.1);
                    open_subpath = true;
                }
            }
            'V' => {
                while cursor.next_is_number() {
                    let mut y = cursor.take_number()?;
                    if relative {
                        y += current.1;
                    }
                    current.1 = y;
                    let mapped = map_svg_point(current.0, current.1, view_box, box_rect);
                    builder.line_to(mapped.0, mapped.1);
                    open_subpath = true;
                }
            }
            'C' => {
                while cursor.next_is_number() {
                    let (mut x1, mut y1) = cursor.take_pair()?;
                    let (mut x2, mut y2) = cursor.take_pair()?;
                    let (mut x, mut y) = cursor.take_pair()?;
                    if relative {
                        x1 += current.0;
                        y1 += current.1;
                        x2 += current.0;
                        y2 += current.1;
                        x += current.0;
                        y += current.1;
                    }
                    let p1 = map_svg_point(x1, y1, view_box, box_rect);
                    let p2 = map_svg_point(x2, y2, view_box, box_rect);
                    let p = map_svg_point(x, y, view_box, box_rect);
                    builder.cubic_to(p1.0, p1.1, p2.0, p2.1, p.0, p.1);
                    current = (x, y);
                    open_subpath = true;
                }
            }
            'Z' => {
                if open_subpath {
                    builder.close();
                    current = subpath_start;
                    open_subpath = false;
                }
                command = None;
            }
            unsupported => {
                return Err(format!(
                    "SVG path command '{unsupported}' is not supported for holographic logos."
                ));
            }
        }
    }

    if open_subpath {
        builder.close();
    }

    builder
        .finish()
        .ok_or_else(|| "SVG path data did not produce a drawable path.".to_string())
}

fn map_svg_point(x: f64, y: f64, view_box: &Rect, box_rect: &Rect) -> (f32, f32) {
    let mapped_x = box_rect.x + ((x - view_box.x) / view_box.width) * box_rect.width;
    let mapped_y = box_rect.y + ((y - view_box.y) / view_box.height) * box_rect.height;
    (mapped_x as f32, mapped_y as f32)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SvgPathToken {
    Command(char),
    Number(f64),
}

struct SvgPathCursor {
    tokens: Vec<SvgPathToken>,
    index: usize,
}

impl SvgPathCursor {
    fn new(tokens: Vec<SvgPathToken>) -> Self {
        Self { tokens, index: 0 }
    }

    fn is_done(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn take_command(&mut self) -> Option<char> {
        match self.tokens.get(self.index) {
            Some(SvgPathToken::Command(command)) => {
                self.index += 1;
                Some(*command)
            }
            _ => None,
        }
    }

    fn next_is_number(&self) -> bool {
        matches!(self.tokens.get(self.index), Some(SvgPathToken::Number(_)))
    }

    fn take_number(&mut self) -> Result<f64, String> {
        match self.tokens.get(self.index).copied() {
            Some(SvgPathToken::Number(value)) => {
                self.index += 1;
                Ok(value)
            }
            _ => Err("SVG path command is missing a numeric parameter.".to_string()),
        }
    }

    fn take_pair(&mut self) -> Result<(f64, f64), String> {
        Ok((self.take_number()?, self.take_number()?))
    }
}

fn tokenize_svg_path(path_data: &str) -> Result<Vec<SvgPathToken>, String> {
    let chars = path_data.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];
        if character.is_ascii_whitespace() || character == ',' {
            index += 1;
            continue;
        }
        if character.is_ascii_alphabetic() {
            tokens.push(SvgPathToken::Command(character));
            index += 1;
            continue;
        }

        let start = index;
        let mut saw_digit = false;
        let mut saw_decimal = false;
        let mut saw_exponent = false;
        if matches!(chars[index], '+' | '-') {
            index += 1;
        }
        while index < chars.len() {
            let current = chars[index];
            if current.is_ascii_digit() {
                saw_digit = true;
                index += 1;
                continue;
            }
            if current == '.' && !saw_decimal && !saw_exponent {
                saw_decimal = true;
                index += 1;
                continue;
            }
            if matches!(current, 'e' | 'E') && !saw_exponent {
                saw_exponent = true;
                index += 1;
                if index < chars.len() && matches!(chars[index], '+' | '-') {
                    index += 1;
                }
                continue;
            }
            break;
        }

        if !saw_digit {
            return Err("SVG path data contains an invalid numeric parameter.".to_string());
        }
        let raw = chars[start..index].iter().collect::<String>();
        let value = raw
            .parse::<f64>()
            .map_err(|error| format!("SVG path numeric parameter '{raw}' is invalid: {error}"))?;
        tokens.push(SvgPathToken::Number(value));
    }

    Ok(tokens)
}

fn polygon_path(points: &[Point]) -> Option<tiny_skia::Path> {
    if points.len() < 3 {
        return None;
    }

    let mut builder = PathBuilder::new();
    builder.move_to(points[0].x as f32, points[0].y as f32);
    for point in &points[1..] {
        builder.line_to(point.x as f32, point.y as f32);
    }
    builder.close();
    builder.finish()
}

fn draw_image_ref(
    pixmap: &mut Pixmap,
    node: &ImageRefNode,
    assets: &AssetRegistry,
    render_context: &mut RenderContext,
    index: usize,
) -> ActionableResult<()> {
    if !render_context.image_cache.contains_key(&node.asset_id) {
        let image = load_image_ref_asset(node, assets, index)?;
        render_context
            .image_cache
            .insert(node.asset_id.clone(), image);
    }
    let image = render_context
        .image_cache
        .get(&node.asset_id)
        .expect("imageRef cache populated");

    draw_scaled_image(pixmap, image, &node.box_rect, &node.fit, node.opacity);
    Ok(())
}

fn load_image_ref_asset(
    node: &ImageRefNode,
    assets: &AssetRegistry,
    index: usize,
) -> ActionableResult<RgbaImage> {
    let resolved = assets.resolve(&node.asset_id)?;
    let bytes = std::fs::read(&resolved.absolute_path).map_err(|error| {
        render_failed(
            format!("nodes[{index}].assetId"),
            "Could not read imageRef PNG asset.",
            "Ensure assetId resolves to a readable project PNG file.",
            error,
        )
    })?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|error| {
            image_failed(
                format!("nodes[{index}].assetId"),
                "Could not decode imageRef PNG asset.",
                "Replace the asset with a valid 8-bit PNG or remove the imageRef.",
                error,
            )
        })?
        .to_rgba8();
    Ok(image)
}

fn draw_scaled_image(
    pixmap: &mut Pixmap,
    image: &image::RgbaImage,
    rect: &Rect,
    fit: &ImageFit,
    opacity: f64,
) {
    let Some(placement) = image_placement(rect, fit, image.width(), image.height()) else {
        return;
    };
    let opacity = opacity.clamp(0.0, 1.0);
    let min_x = placement.target.x.max(0);
    let min_y = placement.target.y.max(0);
    let max_x = (placement.target.x + placement.target.width)
        .min(pixmap.width() as i32)
        .max(min_x);
    let max_y = (placement.target.y + placement.target.height)
        .min(pixmap.height() as i32)
        .max(min_y);

    for dest_y in min_y..max_y {
        for dest_x in min_x..max_x {
            let source_x = placement.sample_x((dest_x - placement.target.x) as u32, image.width());
            let source_y = placement.sample_y((dest_y - placement.target.y) as u32, image.height());
            let source = image.get_pixel(source_x, source_y).0;
            let alpha = ((source[3] as f64 * opacity).round()).clamp(0.0, 255.0) as u8;
            blend_pixel(
                pixmap,
                dest_x,
                dest_y,
                [source[0], source[1], source[2], alpha],
            );
        }
    }
}

fn image_placement(
    rect: &Rect,
    fit: &ImageFit,
    image_width: u32,
    image_height: u32,
) -> Option<ImagePlacement> {
    if image_width == 0 || image_height == 0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }

    let mut x = rect.x;
    let mut y = rect.y;
    let mut width = rect.width;
    let mut height = rect.height;
    let mut source_x = 0.0;
    let mut source_y = 0.0;
    let mut source_step_x = image_width as f64 / rect.width;
    let mut source_step_y = image_height as f64 / rect.height;

    match fit {
        ImageFit::Contain => {
            let scale = (rect.width / image_width as f64).min(rect.height / image_height as f64);
            width = image_width as f64 * scale;
            height = image_height as f64 * scale;
            x += (rect.width - width) / 2.0;
            y += (rect.height - height) / 2.0;
            source_step_x = 1.0 / scale;
            source_step_y = 1.0 / scale;
        }
        ImageFit::Cover => {
            let scale = (rect.width / image_width as f64).max(rect.height / image_height as f64);
            let scaled_width = image_width as f64 * scale;
            let scaled_height = image_height as f64 * scale;
            source_x = ((scaled_width - rect.width) / 2.0) / scale;
            source_y = ((scaled_height - rect.height) / 2.0) / scale;
            source_step_x = 1.0 / scale;
            source_step_y = 1.0 / scale;
        }
        ImageFit::None => {
            width = (image_width as f64).min(rect.width);
            height = (image_height as f64).min(rect.height);
            source_step_x = 1.0;
            source_step_y = 1.0;
        }
        ImageFit::Stretch => {}
    }

    let target = IntRect {
        x: x.round() as i32,
        y: y.round() as i32,
        width: width.round().max(1.0) as i32,
        height: height.round().max(1.0) as i32,
    };

    Some(ImagePlacement {
        target,
        source_x,
        source_y,
        source_step_x,
        source_step_y,
    })
}

fn sk_rect(rect: &Rect) -> Option<SkRect> {
    SkRect::from_xywh(
        rect.x as f32,
        rect.y as f32,
        rect.width as f32,
        rect.height as f32,
    )
}

fn rounded_rect_path(rect: &Rect, radius: f64) -> Option<tiny_skia::Path> {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }

    let x = rect.x as f32;
    let y = rect.y as f32;
    let width = rect.width as f32;
    let height = rect.height as f32;
    let radius = (radius as f32).min(width / 2.0).min(height / 2.0);

    if radius <= 0.0 {
        return sk_rect(rect).map(PathBuilder::from_rect);
    }

    let right = x + width;
    let bottom = y + height;
    let control = radius * ROUND_RECT_KAPPA;
    let mut builder = PathBuilder::new();
    builder.move_to(x + radius, y);
    builder.line_to(right - radius, y);
    builder.cubic_to(
        right - radius + control,
        y,
        right,
        y + radius - control,
        right,
        y + radius,
    );
    builder.line_to(right, bottom - radius);
    builder.cubic_to(
        right,
        bottom - radius + control,
        right - radius + control,
        bottom,
        right - radius,
        bottom,
    );
    builder.line_to(x + radius, bottom);
    builder.cubic_to(
        x + radius - control,
        bottom,
        x,
        bottom - radius + control,
        x,
        bottom - radius,
    );
    builder.line_to(x, y + radius);
    builder.cubic_to(
        x,
        y + radius - control,
        x + radius - control,
        y,
        x + radius,
        y,
    );
    builder.close();
    builder.finish()
}

fn paint_from_rgba(rgba: [u8; 4]) -> Paint<'static> {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
    paint
}

fn parse_color(color: &Color, path: &str) -> ActionableResult<[u8; 4]> {
    let Color::Hex(raw) = color;
    let Some(hex) = raw.strip_prefix('#') else {
        return Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidColor,
            path,
            "Color must use #RRGGBB or #RRGGBBAA syntax.",
            "Use a valid hex color string.",
        )]);
    };
    if hex.len() != 6 && hex.len() != 8 {
        return Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidColor,
            path,
            "Color must use #RRGGBB or #RRGGBBAA syntax.",
            "Use six or eight hex digits after #.",
        )]);
    }

    let r = parse_hex_byte(hex, 0, path)?;
    let g = parse_hex_byte(hex, 2, path)?;
    let b = parse_hex_byte(hex, 4, path)?;
    let a = if hex.len() == 8 {
        parse_hex_byte(hex, 6, path)?
    } else {
        255
    };

    Ok([r, g, b, a])
}

fn parse_hex_byte(hex: &str, start: usize, path: &str) -> ActionableResult<u8> {
    u8::from_str_radix(&hex[start..start + 2], 16).map_err(|error| {
        render_failed(
            path,
            "Color contains invalid hex digits.",
            "Use only hexadecimal digits in color values.",
            error,
        )
    })
}

fn align_from_node(align: &str) -> Option<Align> {
    match align.trim().to_ascii_lowercase().as_str() {
        "center" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        "left" | "start" => Some(Align::Left),
        _ => Some(Align::Left),
    }
}

fn multiply_alpha(coverage_alpha: u8, fill_alpha: u8) -> u8 {
    ((coverage_alpha as u16 * fill_alpha as u16 + 127) / 255) as u8
}

fn blend_solid_rect(
    pixmap: &mut Pixmap,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    rgba: [u8; 4],
    clip: Option<IntClip>,
) {
    if rgba[3] == 0 || width == 0 || height == 0 {
        return;
    }

    let clip = clip.unwrap_or_else(|| IntClip {
        min_x: 0,
        min_y: 0,
        max_x: pixmap.width() as i32,
        max_y: pixmap.height() as i32,
    });
    let min_x = x.max(clip.min_x).max(0);
    let min_y = y.max(clip.min_y).max(0);
    let max_x = (x + width as i32)
        .min(clip.max_x)
        .min(pixmap.width() as i32);
    let max_y = (y + height as i32)
        .min(clip.max_y)
        .min(pixmap.height() as i32);

    for dest_y in min_y..max_y {
        for dest_x in min_x..max_x {
            blend_pixel(pixmap, dest_x, dest_y, rgba);
        }
    }
}

fn blend_pixel(pixmap: &mut Pixmap, x: i32, y: i32, rgba: [u8; 4]) {
    if rgba[3] == 0 || x < 0 || y < 0 || x >= pixmap.width() as i32 || y >= pixmap.height() as i32 {
        return;
    }

    let index = ((y as u32 * pixmap.width() + x as u32) * 4) as usize;
    let dest = &mut pixmap.data_mut()[index..index + 4];
    let source_alpha = rgba[3] as u32;
    let inv_alpha = 255 - source_alpha;
    let source_r = rgba[0] as u32 * source_alpha / 255;
    let source_g = rgba[1] as u32 * source_alpha / 255;
    let source_b = rgba[2] as u32 * source_alpha / 255;

    dest[0] = (source_r + dest[0] as u32 * inv_alpha / 255).min(255) as u8;
    dest[1] = (source_g + dest[1] as u32 * inv_alpha / 255).min(255) as u8;
    dest[2] = (source_b + dest[2] as u32 * inv_alpha / 255).min(255) as u8;
    dest[3] = (source_alpha + dest[3] as u32 * inv_alpha / 255).min(255) as u8;
}

fn to_color_byte(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn to_holographic_color_byte(value: f64) -> u8 {
    let value = if value > 0.92 {
        0.92 + (value - 0.92) * 0.55
    } else {
        value
    };
    to_color_byte(value)
}

fn holographic_spectrum(t: f64) -> [f64; 3] {
    let phase = fract(t);
    [
        0.5 + 0.5 * (std::f64::consts::TAU * phase).cos(),
        0.5 + 0.5 * (std::f64::consts::TAU * (phase + 0.33)).cos(),
        0.5 + 0.5 * (std::f64::consts::TAU * (phase + 0.66)).cos(),
    ]
}

#[allow(dead_code)]
fn metal_imperfection_texture(u: f64, v: f64, time: f64) -> f64 {
    let mut p = [
        u * 2.0 + time * 0.035,
        v * 2.0 - time * 0.025,
        time * 0.18 + (u - v) * 0.16,
    ];
    let base = value_noise_3d(p[0] * 4.0, p[1] * 4.0, p[2] * 4.0, 211)
        + value_noise_3d(p[0] * 8.0, p[1] * 8.0, p[2] * 8.0, 223) * 0.5
        + value_noise_3d(p[0] * 16.0, p[1] * 16.0, p[2] * 16.0, 227) * 0.25;
    let mut roughness = (base * 0.70 - 0.20).clamp(0.0, 1.0).powf(4.0);
    let mut scale = [15.0, 3.0, 3.0];

    for index in 0..5 {
        let seed = 241 + index * 19;
        scale[0] *= 1.45;
        scale[1] *= 1.45;
        scale[2] *= 1.45;
        p[0] += 0.20;
        p[1] += 0.13;
        p[2] += 0.17;

        let n0 = value_noise_3d(p[0] * scale[0], p[1] * scale[1], p[2] * scale[2], seed);
        roughness = roughness.max(n0.powf(22.0) * 0.66);
        p = metal_matrix_two(p);

        let n1 = value_noise_3d(p[0] * scale[0], p[1] * scale[1], p[2] * scale[2], seed + 5);
        roughness = roughness.max(n1.powf(24.0) * 0.52);
        p = metal_matrix_one(p);

        let n2 = value_noise_3d(p[0] * scale[0], p[1] * scale[1], p[2] * scale[2], seed + 11);
        roughness = roughness.max(n2.powf(24.0) * 0.60);
    }

    roughness.clamp(0.0, 1.0)
}

fn anisotropic_scratch_texture(u: f64, v: f64, time: f64) -> f64 {
    let (brush_u, brush_v) = rotate_2d(u - 0.5, v - 0.5, -0.62 + (time * 0.16).sin() * 0.08);
    let brush_noise = value_noise_2d(brush_u * 5.4 + time * 0.035, brush_v * 18.0, 307);
    let long_line = (fract((brush_u + brush_noise * 0.030) * 74.0) - 0.5).abs();
    let long_scratch = 1.0 - smoothstep(0.006, 0.052, long_line);
    let long_gate = smoothstep(
        0.58,
        0.94,
        value_noise_2d(brush_u * 10.0, brush_v * 3.2, 311),
    );

    let (cross_u, cross_v) = rotate_2d(u - 0.5, v - 0.5, 0.42 + (time * 0.10).cos() * 0.06);
    let cross_noise = value_noise_2d(cross_u * 8.0 - time * 0.025, cross_v * 24.0, 317);
    let cross_line = (fract((cross_u + cross_noise * 0.018) * 118.0) - 0.5).abs();
    let cross_scratch = 1.0 - smoothstep(0.004, 0.034, cross_line);
    let cross_gate = smoothstep(
        0.70,
        0.97,
        value_noise_2d(cross_u * 7.0, cross_v * 6.0, 331),
    );

    (long_scratch * long_gate * 0.68 + cross_scratch * cross_gate * 0.36).clamp(0.0, 1.0)
}

#[allow(dead_code)]
fn metal_matrix_one(p: [f64; 3]) -> [f64; 3] {
    [
        p[0] * 0.61 + p[1] * 0.12 + p[2] * 0.78,
        p[0] * 0.04 + p[1] * 0.98 - p[2] * 0.18,
        -p[0] * 0.79 + p[1] * 0.14 + p[2] * 0.59,
    ]
}

#[allow(dead_code)]
fn metal_matrix_two(p: [f64; 3]) -> [f64; 3] {
    [
        p[0] * 0.44 + p[1] * 0.87 + p[2] * 0.20,
        -p[0] * 0.74 + p[1] * 0.23 + p[2] * 0.63,
        p[0] * 0.50 - p[1] * 0.43 + p[2] * 0.75,
    ]
}

#[allow(dead_code)]
fn gaussian_2d(u: f64, v: f64, center_u: f64, center_v: f64, sigma_u: f64, sigma_v: f64) -> f64 {
    let du = (u - center_u) / sigma_u.max(f64::EPSILON);
    let dv = (v - center_v) / sigma_v.max(f64::EPSILON);
    (-0.5 * (du * du + dv * dv)).exp().clamp(0.0, 1.0)
}

fn write_png(pixmap: &Pixmap, path: &Path) -> ActionableResult<()> {
    let file = File::create(path).map_err(|error| {
        render_failed(
            path.to_string_lossy(),
            "Could not create graphics PNG artifact.",
            "Choose a writable graphics preview output directory.",
            error,
        )
    })?;
    let mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(&mut writer, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header().map_err(|error| {
        render_failed(
            path.to_string_lossy(),
            "Could not write graphics PNG header.",
            "Retry with a writable output path and valid RGBA dimensions.",
            error,
        )
    })?;
    let rgba = pixmap.clone().take_demultiplied();
    png_writer.write_image_data(&rgba).map_err(|error| {
        render_failed(
            path.to_string_lossy(),
            "Could not encode graphics PNG artifact.",
            "Retry with a writable output path and valid RGBA dimensions.",
            error,
        )
    })?;
    Ok(())
}

fn write_manifest(manifest: &GraphicsArtifactManifest, path: &Path) -> ActionableResult<()> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|error| {
        render_failed(
            path.to_string_lossy(),
            "Could not serialize graphics artifact manifest.",
            "Check manifest fields for serializable values.",
            error,
        )
    })?;
    std::fs::write(path, json).map_err(|error| {
        render_failed(
            path.to_string_lossy(),
            "Could not write graphics artifact manifest.",
            "Choose a writable graphics preview output directory.",
            error,
        )
    })?;
    Ok(())
}

fn render_failed(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
    error: impl std::fmt::Display,
) -> Vec<ActionableError> {
    vec![
        ActionableError::new(GraphicsErrorCode::GraphicsRenderFailed, path, message, fix)
            .with_detail("cause", error.to_string()),
    ]
}

fn image_failed(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
    error: impl std::fmt::Display,
) -> Vec<ActionableError> {
    vec![ActionableError::new(
        GraphicsErrorCode::GraphicsImageDecodeFailed,
        path,
        message,
        fix,
    )
    .with_detail("cause", error.to_string())]
}

#[derive(Debug, Clone, Copy)]
struct IntRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, Copy)]
struct ImagePlacement {
    target: IntRect,
    source_x: f64,
    source_y: f64,
    source_step_x: f64,
    source_step_y: f64,
}

impl ImagePlacement {
    fn sample_x(&self, x: u32, image_width: u32) -> u32 {
        self.sample_axis(self.source_x, self.source_step_x, x, image_width)
    }

    fn sample_y(&self, y: u32, image_height: u32) -> u32 {
        self.sample_axis(self.source_y, self.source_step_y, y, image_height)
    }

    fn sample_axis(&self, source_start: f64, source_step: f64, offset: u32, limit: u32) -> u32 {
        (source_start + offset as f64 * source_step)
            .floor()
            .clamp(0.0, (limit - 1) as f64) as u32
    }
}

#[derive(Debug, Clone, Copy)]
struct IntClip {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl IntClip {
    fn from_rect(rect: &Rect, pixmap_width: u32, pixmap_height: u32) -> Option<Self> {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return None;
        }

        Some(Self {
            min_x: rect.x.floor().max(0.0) as i32,
            min_y: rect.y.floor().max(0.0) as i32,
            max_x: (rect.x + rect.width).ceil().min(pixmap_width as f64) as i32,
            max_y: (rect.y + rect.height).ceil().min(pixmap_height as f64) as i32,
        })
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use crate::graphics::ir::{Dimensions, GraphicRole};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cancellable_renderer_stops_before_first_frame_publication() {
        let temp = tempfile::tempdir().expect("temp");
        let layer = GraphicsLayer {
            schema_version: 1,
            id: "cancelled-layer".to_string(),
            role: GraphicRole::Overlay,
            timeline_start: 0.0,
            duration_seconds: 1.0,
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
            fps: 30.0,
            alpha: true,
            source_beat: "test cancellation".to_string(),
            visual_treatment: "test".to_string(),
            motion: "test".to_string(),
            safe_zone: "test".to_string(),
            avoid: "test".to_string(),
            nodes: Vec::new(),
        };
        let output_dir = temp.path().join("graphics");
        let error = render_graphics_preview_cancellable(
            &layer,
            &AssetRegistry::new(temp.path().to_path_buf()),
            GraphicsRenderOptions {
                output_dir: output_dir.clone(),
            },
            || true,
        )
        .expect_err("cancelled render must stop");

        assert_eq!(error[0].path, "cancelled");
        assert!(!output_dir.join(PREVIEW_FILE_NAME).exists());
        assert!(!output_dir.join(MANIFEST_FILE_NAME).exists());
    }

    #[test]
    fn cancellable_renderer_removes_frames_written_before_late_cancellation() {
        let temp = tempfile::tempdir().expect("temp");
        let mut layer = GraphicsLayer {
            schema_version: 1,
            id: "late-cancelled-layer".to_string(),
            role: GraphicRole::Overlay,
            timeline_start: 0.0,
            duration_seconds: 1.0,
            dimensions: Dimensions {
                width: 64,
                height: 64,
            },
            fps: 30.0,
            alpha: true,
            source_beat: "test cancellation".to_string(),
            visual_treatment: "test".to_string(),
            motion: "test".to_string(),
            safe_zone: "test".to_string(),
            avoid: "test".to_string(),
            nodes: Vec::new(),
        };
        layer.nodes.push(GraphicNode::Rect(RectNode {
            id: "background".into(),
            box_rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 64.0,
                height: 64.0,
            },
            fill: Color::Hex("#ff0000".into()),
            animate: None,
        }));
        let output_dir = temp.path().join("graphics");
        let checks = AtomicUsize::new(0);
        let error = render_graphics_preview_cancellable(
            &layer,
            &AssetRegistry::new(temp.path().to_path_buf()),
            GraphicsRenderOptions {
                output_dir: output_dir.clone(),
            },
            || checks.fetch_add(1, Ordering::AcqRel) >= 2,
        )
        .expect_err("late cancellation must stop");

        assert_eq!(error[0].path, "cancelled");
        assert!(!output_dir.exists());
    }
}
