//! CPU-side wireframe primitive compositing for GPU graphics readbacks.

use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::{Camera, CameraPreset, Primitive, PrimitiveScene};

const CUBE_VERTICES: [[f32; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
const CUBE_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];
const GRID_DIVISIONS: u32 = 10;
const DEFAULT_CAMERA_DISTANCE: f32 = 4.0;
const DEFAULT_FOV_DEGREES: f32 = 46.0;
const MIN_PRECISE_LINE_AREA: u64 = 16_384;
const MAX_PRECISE_LINE_AREA: u64 = 96_000;
const MAX_SAMPLED_LINE_STEPS: u32 = 4_096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireEdge3 {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireEdge2 {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub color: [f32; 4],
}

pub fn build_wireframe_edges_for_primitive(
    primitive: &Primitive,
) -> GpuGraphicsResult<Vec<WireEdge3>> {
    build_wireframe_edges_for_primitive_at_progress(primitive, 0.0)
}

pub fn project_wireframe_edges(
    edges: &[WireEdge3],
    width: u32,
    height: u32,
    progress: f32,
) -> Vec<WireEdge2> {
    project_wireframe_edges_with_camera(
        edges,
        width,
        height,
        &ProjectionCamera::default_for_progress(progress),
        progress,
    )
}

pub fn draw_projected_wireframes_into_rgba(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    scene: &PrimitiveScene,
    progress: f32,
) -> GpuGraphicsResult<()> {
    validate_rgba_buffer(rgba, width, height)?;
    if width == 0 || height == 0 {
        return Ok(());
    }

    let camera = ProjectionCamera::from_scene_camera(&scene.camera, progress);
    for (index, primitive) in scene.primitives.iter().enumerate() {
        let edges = build_wireframe_edges_for_primitive_at_progress(primitive, progress).map_err(
            |mut errors| {
                for error in &mut errors {
                    let path = error
                        .path
                        .strip_prefix("primitive.")
                        .unwrap_or(error.path.as_str())
                        .to_string();
                    error.path = format!("scene.primitives[{index}].{path}");
                }
                errors
            },
        )?;
        let projected =
            project_wireframe_edges_with_camera(&edges, width, height, &camera, progress);
        for edge in projected {
            draw_projected_edge(rgba, width, height, edge);
        }
    }

    Ok(())
}

fn build_wireframe_edges_for_primitive_at_progress(
    primitive: &Primitive,
    progress: f32,
) -> GpuGraphicsResult<Vec<WireEdge3>> {
    validate_primitive_transform(primitive)?;
    match primitive.primitive_type.as_str() {
        "cube" => Ok(build_cube_wireframe_edges(primitive, progress)),
        "grid" => Ok(build_grid_wireframe_edges(primitive, progress)),
        _ => Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveUnsupported,
            "primitive.type",
            "Primitive type is not supported by the wireframe renderer.",
            "Use cube or grid, or add primitive_renderer support for this primitive before rendering GPU wireframes.",
        )
        .with_detail("primitiveId", primitive.id.clone())
        .with_detail("primitiveType", primitive.primitive_type.clone())]),
    }
}

fn build_cube_wireframe_edges(primitive: &Primitive, progress: f32) -> Vec<WireEdge3> {
    let color = material_color(primitive);
    CUBE_EDGES
        .iter()
        .map(|(start, end)| WireEdge3 {
            start: transform_point(CUBE_VERTICES[*start], primitive, progress),
            end: transform_point(CUBE_VERTICES[*end], primitive, progress),
            color,
        })
        .collect()
}

fn build_grid_wireframe_edges(primitive: &Primitive, progress: f32) -> Vec<WireEdge3> {
    let color = material_color(primitive);
    let mut edges = Vec::with_capacity(((GRID_DIVISIONS + 1) * 2) as usize);
    let step = 2.0 / GRID_DIVISIONS as f32;

    for index in 0..=GRID_DIVISIONS {
        let value = -1.0 + step * index as f32;
        edges.push(WireEdge3 {
            start: transform_point([-1.0, value, 0.0], primitive, progress),
            end: transform_point([1.0, value, 0.0], primitive, progress),
            color,
        });
        edges.push(WireEdge3 {
            start: transform_point([value, -1.0, 0.0], primitive, progress),
            end: transform_point([value, 1.0, 0.0], primitive, progress),
            color,
        });
    }

    edges
}

fn transform_point(point: [f32; 3], primitive: &Primitive, progress: f32) -> [f32; 3] {
    let transform = &primitive.transform;
    let mut transformed = [
        point[0] * transform.scale[0],
        point[1] * transform.scale[1],
        point[2] * transform.scale[2],
    ];
    transformed = rotate_xyz(transformed, transform.rotation);

    if let Some(rotation) = primitive
        .animate
        .as_ref()
        .and_then(|animation| animation.rotation.as_ref())
    {
        let turns = if rotation.turns.is_finite() {
            rotation.turns
        } else {
            0.0
        };
        transformed = rotate_axis_angle(
            transformed,
            rotation.axis,
            turns * std::f32::consts::TAU * normalized_progress(progress),
        );
    }

    [
        transformed[0] + transform.position[0],
        transformed[1] + transform.position[1],
        transformed[2] + transform.position[2],
    ]
}

fn project_wireframe_edges_with_camera(
    edges: &[WireEdge3],
    width: u32,
    height: u32,
    camera: &ProjectionCamera,
    progress: f32,
) -> Vec<WireEdge2> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    edges
        .iter()
        .filter_map(|edge| {
            let start = project_point(edge.start, width, height, camera, progress)?;
            let end = project_point(edge.end, width, height, camera, progress)?;
            Some(WireEdge2 {
                start,
                end,
                color: edge.color,
            })
        })
        .collect()
}

fn project_point(
    point: [f32; 3],
    width: u32,
    height: u32,
    camera: &ProjectionCamera,
    progress: f32,
) -> Option<[f32; 2]> {
    if point.iter().any(|coord| !coord.is_finite()) {
        return None;
    }

    let (view_x, view_y, camera_z) = if let Some(eye) = camera.eye {
        camera_space_from_eye(point, eye, camera.target)?
    } else {
        let mut view = [
            point[0] - camera.target[0],
            point[1] - camera.target[1],
            point[2] - camera.target[2],
        ];
        match camera.preset {
            ProjectionCameraPreset::Fixed | ProjectionCameraPreset::Dolly => {}
            ProjectionCameraPreset::Orbit => {
                let orbit = normalized_progress(progress) * std::f32::consts::TAU * 0.12;
                let lift = 0.08 * (normalized_progress(progress) * std::f32::consts::TAU).sin();
                view = rotate_xyz(view, [lift, orbit, 0.0]);
            }
        }
        (view[0], view[1], view[2] + camera.distance.max(0.25))
    };
    if camera_z <= 0.05 || !camera_z.is_finite() {
        return None;
    }

    let width_f = width.max(1) as f32;
    let height_f = height.max(1) as f32;
    let aspect = (width_f / height_f).max(0.01);
    let fov_radians = camera.fov_degrees.clamp(20.0, 120.0).to_radians();
    let focal = 1.0 / (fov_radians * 0.5).tan();
    let screen = [
        width_f * 0.5 + (view_x * focal / camera_z / aspect) * width_f * 0.5,
        height_f * 0.53 - (view_y * focal / camera_z) * height_f * 0.5,
    ];

    if screen.iter().all(|coord| coord.is_finite()) {
        Some(screen)
    } else {
        None
    }
}

fn draw_projected_edge(rgba: &mut [u8], width: u32, height: u32, edge: WireEdge2) {
    if edge
        .start
        .iter()
        .chain(edge.end.iter())
        .chain(edge.color.iter())
        .any(|value| !value.is_finite())
    {
        return;
    }

    draw_line(rgba, width, height, edge, edge.color[3] * 0.22, 6.5);
    draw_line(rgba, width, height, edge, edge.color[3], 1.35);
}

fn draw_line(rgba: &mut [u8], width: u32, height: u32, edge: WireEdge2, opacity: f32, radius: f32) {
    if opacity <= 0.0 || width == 0 || height == 0 {
        return;
    }

    let Some(bounds) = line_bounds(width, height, edge, radius) else {
        return;
    };
    if bounds.area() > precise_line_area_limit(width, height) {
        draw_line_sampled(rgba, width, height, edge, opacity, radius);
        return;
    }

    draw_line_precise(rgba, width, edge, opacity, radius, bounds);
}

fn line_bounds(width: u32, height: u32, edge: WireEdge2, radius: f32) -> Option<LineBounds> {
    let spread = radius * 3.0;
    let min_x_f = edge.start[0].min(edge.end[0]) - spread;
    let max_x_f = edge.start[0].max(edge.end[0]) + spread;
    let min_y_f = edge.start[1].min(edge.end[1]) - spread;
    let max_y_f = edge.start[1].max(edge.end[1]) + spread;
    let max_pixel_x = width.saturating_sub(1) as f32;
    let max_pixel_y = height.saturating_sub(1) as f32;

    if max_x_f < 0.0 || max_y_f < 0.0 || min_x_f > max_pixel_x || min_y_f > max_pixel_y {
        return None;
    }

    let min_x = min_x_f.floor().max(0.0).min(max_pixel_x) as u32;
    let max_x = max_x_f.ceil().max(0.0).min(max_pixel_x) as u32;
    let min_y = min_y_f.floor().max(0.0).min(max_pixel_y) as u32;
    let max_y = max_y_f.ceil().max(0.0).min(max_pixel_y) as u32;
    if min_x > max_x || min_y > max_y {
        return None;
    }

    Some(LineBounds {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn draw_line_precise(
    rgba: &mut [u8],
    width: u32,
    edge: WireEdge2,
    opacity: f32,
    radius: f32,
    bounds: LineBounds,
) {
    for y in bounds.min_y..=bounds.max_y {
        for x in bounds.min_x..=bounds.max_x {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let distance = distance_to_segment(point, edge.start, edge.end);
            let core = 1.0 - smoothstep(0.0, radius, distance);
            let glow = 1.0 - smoothstep(radius, radius * 3.0, distance);
            let intensity = (core + glow * 0.28).clamp(0.0, 1.0) * opacity;
            if intensity > 0.0 {
                blend_additive(rgba, width, x, y, edge.color, intensity);
            }
        }
    }
}

fn draw_line_sampled(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    edge: WireEdge2,
    opacity: f32,
    radius: f32,
) {
    let spread = radius * 3.0;
    let clip_rect = ClipRect {
        min_x: -spread,
        max_x: width.saturating_sub(1) as f32 + spread,
        min_y: -spread,
        max_y: height.saturating_sub(1) as f32 + spread,
    };
    let Some((start, end)) = clip_segment_to_rect(edge.start, edge.end, clip_rect) else {
        return;
    };
    let length = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
    if !length.is_finite() {
        return;
    }

    let sample_step = (radius * 0.55).clamp(0.75, 4.0);
    let steps = ((length / sample_step).ceil() as u32).clamp(1, MAX_SAMPLED_LINE_STEPS);
    let sample_spacing = length / steps as f32;
    let opacity_scale = (sample_spacing / (radius * 1.4).max(1.0)).clamp(0.16, 0.70);
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let sample = [
            start[0] + (end[0] - start[0]) * t,
            start[1] + (end[1] - start[1]) * t,
        ];
        stamp_line_sample(
            rgba,
            width,
            height,
            LineStamp {
                sample,
                start,
                end,
                color: edge.color,
                opacity: opacity * opacity_scale,
                radius,
            },
        );
    }
}

fn stamp_line_sample(rgba: &mut [u8], width: u32, height: u32, stamp: LineStamp) {
    let spread = stamp.radius * 3.0;
    let max_pixel_x = width.saturating_sub(1) as f32;
    let max_pixel_y = height.saturating_sub(1) as f32;
    let min_x_f = stamp.sample[0] - spread;
    let max_x_f = stamp.sample[0] + spread;
    let min_y_f = stamp.sample[1] - spread;
    let max_y_f = stamp.sample[1] + spread;

    if max_x_f < 0.0 || max_y_f < 0.0 || min_x_f > max_pixel_x || min_y_f > max_pixel_y {
        return;
    }

    let min_x = min_x_f.floor().max(0.0).min(max_pixel_x) as u32;
    let max_x = max_x_f.ceil().max(0.0).min(max_pixel_x) as u32;
    let min_y = min_y_f.floor().max(0.0).min(max_pixel_y) as u32;
    let max_y = max_y_f.ceil().max(0.0).min(max_pixel_y) as u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let distance = distance_to_segment(point, stamp.start, stamp.end);
            let core = 1.0 - smoothstep(0.0, stamp.radius, distance);
            let glow = 1.0 - smoothstep(stamp.radius, stamp.radius * 3.0, distance);
            let intensity = (core + glow * 0.28).clamp(0.0, 1.0) * stamp.opacity;
            if intensity > 0.0 {
                blend_additive(rgba, width, x, y, stamp.color, intensity);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LineStamp {
    sample: [f32; 2],
    start: [f32; 2],
    end: [f32; 2],
    color: [f32; 4],
    opacity: f32,
    radius: f32,
}

fn precise_line_area_limit(width: u32, height: u32) -> u64 {
    let frame_area = width as u64 * height as u64;
    (frame_area / 3).clamp(MIN_PRECISE_LINE_AREA, MAX_PRECISE_LINE_AREA)
}

#[derive(Debug, Clone, Copy)]
struct LineBounds {
    min_x: u32,
    max_x: u32,
    min_y: u32,
    max_y: u32,
}

impl LineBounds {
    fn area(self) -> u64 {
        (self.max_x - self.min_x + 1) as u64 * (self.max_y - self.min_y + 1) as u64
    }
}

#[derive(Debug, Clone, Copy)]
struct ClipRect {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

fn clip_segment_to_rect(
    start: [f32; 2],
    end: [f32; 2],
    rect: ClipRect,
) -> Option<([f32; 2], [f32; 2])> {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let mut start_t = 0.0;
    let mut end_t = 1.0;

    for (p, q) in [
        (-delta[0], start[0] - rect.min_x),
        (delta[0], rect.max_x - start[0]),
        (-delta[1], start[1] - rect.min_y),
        (delta[1], rect.max_y - start[1]),
    ] {
        if p.abs() <= f32::EPSILON {
            if q < 0.0 {
                return None;
            }
            continue;
        }

        let t = q / p;
        if p < 0.0 {
            start_t = f32::max(start_t, t);
        } else {
            end_t = f32::min(end_t, t);
        }
        if start_t > end_t {
            return None;
        }
    }

    Some((
        [start[0] + delta[0] * start_t, start[1] + delta[1] * start_t],
        [start[0] + delta[0] * end_t, start[1] + delta[1] * end_t],
    ))
}

fn blend_additive(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [f32; 4], intensity: f32) {
    let offset = ((y as usize * width as usize) + x as usize) * 4;
    rgba[offset] = to_byte(rgba[offset] as f32 / 255.0 + color[0] * intensity);
    rgba[offset + 1] = to_byte(rgba[offset + 1] as f32 / 255.0 + color[1] * intensity);
    rgba[offset + 2] = to_byte(rgba[offset + 2] as f32 / 255.0 + color[2] * intensity);
    rgba[offset + 3] = to_byte(rgba[offset + 3] as f32 / 255.0 + color[3] * intensity);
}

fn validate_rgba_buffer(rgba: &[u8], width: u32, height: u32) -> GpuGraphicsResult<()> {
    let expected = frame_byte_len(width, height)?;
    if rgba.len() == expected {
        return Ok(());
    }

    Err(vec![
        GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
            "frame",
            "GPU readback buffer size did not match primitive compositing dimensions.",
            "Ensure the renderer returns exactly width * height * 4 RGBA bytes before compositing primitives.",
        )
        .with_detail("expectedBytes", expected.to_string())
        .with_detail("actualBytes", rgba.len().to_string()),
    ])
}

fn frame_byte_len(width: u32, height: u32) -> GpuGraphicsResult<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsRenderFailed,
                "frame.dimensions",
                "Primitive compositing dimensions overflowed the RGBA buffer budget.",
                "Use bounded GPU layer dimensions before compositing primitives.",
            )]
        })
}

fn validate_primitive_transform(primitive: &Primitive) -> GpuGraphicsResult<()> {
    let transform = &primitive.transform;
    for (field, values) in [
        ("position", transform.position),
        ("rotation", transform.rotation),
        ("scale", transform.scale),
    ] {
        if values.iter().any(|value| !value.is_finite()) {
            return Err(vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                format!("primitive.transform.{field}"),
                "Primitive transform contains a non-finite value.",
                "Use finite primitive transform values before rendering wireframes.",
            )
            .with_detail("primitiveId", primitive.id.clone())]);
        }
    }
    if transform.scale.iter().any(|value| *value <= 0.0) {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            "primitive.transform.scale",
            "Primitive scale must be positive.",
            "Use scale values greater than 0 before rendering wireframes.",
        )
        .with_detail("primitiveId", primitive.id.clone())]);
    }
    if let Some(rotation) = primitive
        .animate
        .as_ref()
        .and_then(|animation| animation.rotation.as_ref())
    {
        if !rotation.turns.is_finite() || rotation.axis.iter().any(|value| !value.is_finite()) {
            return Err(vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                "primitive.animate.rotation",
                "Primitive rotation animation contains a non-finite value.",
                "Use finite axis and turns values before rendering animated wireframes.",
            )
            .with_detail("primitiveId", primitive.id.clone())]);
        }
    }

    Ok(())
}

fn material_color(primitive: &Primitive) -> [f32; 4] {
    let [red, green, blue] = parse_hex_rgb(&primitive.material.color).unwrap_or([1.0, 1.0, 1.0]);
    let opacity = primitive
        .material
        .opacity
        .filter(|value| value.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    [red, green, blue, opacity]
}

fn parse_hex_rgb(value: &str) -> Option<[f32; 3]> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
    ])
}

#[derive(Debug, Clone, Copy)]
struct ProjectionCamera {
    preset: ProjectionCameraPreset,
    target: [f32; 3],
    eye: Option<[f32; 3]>,
    distance: f32,
    fov_degrees: f32,
}

impl ProjectionCamera {
    fn default_for_progress(_progress: f32) -> Self {
        Self {
            preset: ProjectionCameraPreset::Orbit,
            target: [0.0, 0.0, 0.0],
            eye: None,
            distance: DEFAULT_CAMERA_DISTANCE,
            fov_degrees: DEFAULT_FOV_DEGREES,
        }
    }

    fn from_scene_camera(camera: &Camera, progress: f32) -> Self {
        let target = finite_point(camera.target).unwrap_or([0.0, 0.0, 0.0]);
        let position = finite_point(camera.position);
        let preset = match &camera.preset {
            CameraPreset::Fixed => ProjectionCameraPreset::Fixed,
            CameraPreset::Orbit => ProjectionCameraPreset::Orbit,
            CameraPreset::Dolly => ProjectionCameraPreset::Dolly,
        };
        let position_distance = position
            .map(|position| length(subtract(target, position)))
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(DEFAULT_CAMERA_DISTANCE);
        let distance = camera
            .distance
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(position_distance);
        let dolly_factor = match preset {
            ProjectionCameraPreset::Dolly => {
                0.98 + 0.04 * (normalized_progress(progress) * std::f32::consts::TAU).sin()
            }
            ProjectionCameraPreset::Fixed | ProjectionCameraPreset::Orbit => 1.0,
        };
        let eye = position.map(|position| match preset {
            ProjectionCameraPreset::Fixed => position,
            ProjectionCameraPreset::Orbit => {
                let offset = subtract(position, target);
                let orbit = normalized_progress(progress) * std::f32::consts::TAU * 0.12;
                let lift = 0.08 * (normalized_progress(progress) * std::f32::consts::TAU).sin();
                add(target, rotate_xyz(offset, [lift, orbit, 0.0]))
            }
            ProjectionCameraPreset::Dolly => {
                let forward = normalize(subtract(target, position)).unwrap_or([0.0, 0.0, -1.0]);
                subtract(target, scale(forward, position_distance * dolly_factor))
            }
        });
        let fov_degrees = camera
            .fov_degrees
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(DEFAULT_FOV_DEGREES);

        Self {
            preset,
            target,
            eye,
            distance: distance * dolly_factor,
            fov_degrees,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ProjectionCameraPreset {
    Fixed,
    Orbit,
    Dolly,
}

fn camera_space_from_eye(
    point: [f32; 3],
    eye: [f32; 3],
    target: [f32; 3],
) -> Option<(f32, f32, f32)> {
    if point
        .iter()
        .chain(eye.iter())
        .chain(target.iter())
        .any(|value| !value.is_finite())
    {
        return None;
    }

    let forward = normalize(subtract(target, eye)).unwrap_or([0.0, 0.0, -1.0]);
    let world_up = if forward[1].abs() > 0.96 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let right = normalize(cross(forward, world_up))?;
    let up = cross(right, forward);
    let offset = subtract(point, eye);

    Some((dot(offset, right), dot(offset, up), dot(offset, forward)))
}

fn finite_point(point: Option<[f32; 3]>) -> Option<[f32; 3]> {
    point.filter(|point| point.iter().all(|value| value.is_finite()))
}

fn add(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] + rhs[0], lhs[1] + rhs[1], lhs[2] + rhs[2]]
}

fn subtract(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] - rhs[0], lhs[1] - rhs[1], lhs[2] - rhs[2]]
}

fn scale(point: [f32; 3], value: f32) -> [f32; 3] {
    [point[0] * value, point[1] * value, point[2] * value]
}

fn dot(lhs: [f32; 3], rhs: [f32; 3]) -> f32 {
    lhs[0] * rhs[0] + lhs[1] * rhs[1] + lhs[2] * rhs[2]
}

fn cross(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [
        lhs[1] * rhs[2] - lhs[2] * rhs[1],
        lhs[2] * rhs[0] - lhs[0] * rhs[2],
        lhs[0] * rhs[1] - lhs[1] * rhs[0],
    ]
}

fn normalize(point: [f32; 3]) -> Option<[f32; 3]> {
    let length = length(point);
    if length <= f32::EPSILON || !length.is_finite() {
        return None;
    }
    Some([point[0] / length, point[1] / length, point[2] / length])
}

fn length(point: [f32; 3]) -> f32 {
    (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt()
}

fn rotate_xyz(point: [f32; 3], rotation: [f32; 3]) -> [f32; 3] {
    let (sin_x, cos_x) = rotation[0].sin_cos();
    let (sin_y, cos_y) = rotation[1].sin_cos();
    let (sin_z, cos_z) = rotation[2].sin_cos();

    let x_rotated = [
        point[0],
        point[1] * cos_x - point[2] * sin_x,
        point[1] * sin_x + point[2] * cos_x,
    ];
    let y_rotated = [
        x_rotated[0] * cos_y + x_rotated[2] * sin_y,
        x_rotated[1],
        -x_rotated[0] * sin_y + x_rotated[2] * cos_y,
    ];
    [
        y_rotated[0] * cos_z - y_rotated[1] * sin_z,
        y_rotated[0] * sin_z + y_rotated[1] * cos_z,
        y_rotated[2],
    ]
}

fn rotate_axis_angle(point: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if length <= f32::EPSILON || !angle.is_finite() {
        return point;
    }

    let axis = [axis[0] / length, axis[1] / length, axis[2] / length];
    let (sin_angle, cos_angle) = angle.sin_cos();
    let dot = point[0] * axis[0] + point[1] * axis[1] + point[2] * axis[2];
    [
        point[0] * cos_angle
            + (axis[1] * point[2] - axis[2] * point[1]) * sin_angle
            + axis[0] * dot * (1.0 - cos_angle),
        point[1] * cos_angle
            + (axis[2] * point[0] - axis[0] * point[2]) * sin_angle
            + axis[1] * dot * (1.0 - cos_angle),
        point[2] * cos_angle
            + (axis[0] * point[1] - axis[1] * point[0]) * sin_angle
            + axis[2] * dot * (1.0 - cos_angle),
    ]
}

fn distance_to_segment(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let segment = [end[0] - start[0], end[1] - start[1]];
    let length_squared = segment[0] * segment[0] + segment[1] * segment[1];
    if length_squared <= f32::EPSILON {
        return ((point[0] - start[0]).powi(2) + (point[1] - start[1]).powi(2)).sqrt();
    }

    let offset = [point[0] - start[0], point[1] - start[1]];
    let t = ((offset[0] * segment[0] + offset[1] * segment[1]) / length_squared).clamp(0.0, 1.0);
    let closest = [start[0] + segment[0] * t, start[1] + segment[1] * t];
    ((point[0] - closest[0]).powi(2) + (point[1] - closest[1]).powi(2)).sqrt()
}

fn smoothstep(edge_0: f32, edge_1: f32, value: f32) -> f32 {
    let t = ((value - edge_0) / (edge_1 - edge_0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn normalized_progress(progress: f32) -> f32 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
