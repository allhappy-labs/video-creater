//! Deterministic CPU fallback for the HQ GPU visual profile.

use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::GpuGraphicsLayer;
use super::profile::is_canonical_hq_neon_wireframe_layer;
use super::validation::validate_gpu_graphics_layer;
use crate::graphics::manifest::{graphics_playback_manifest, GraphicsArtifactManifest};
use image::{ImageBuffer, Rgba};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const PREVIEW_FILE_NAME: &str = "preview.png";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const MANIFEST_TEMP_FILE_NAME: &str = "manifest.json.tmp";
const FRAMES_DIR_NAME: &str = "frames";
const FRAMES_PATTERN: &str = "frames/frame-%06d.png";
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

type SoftwareFrame = ImageBuffer<Rgba<u8>, Vec<u8>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareGpuRenderOptions {
    pub output_dir: PathBuf,
}

pub fn render_hq_profile_software(
    layer: &GpuGraphicsLayer,
    options: SoftwareGpuRenderOptions,
) -> GpuGraphicsResult<GraphicsArtifactManifest> {
    validate_gpu_graphics_layer(layer)?;
    validate_hq_profile_shape(layer)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        vec![artifact_error(
            "outputDir",
            "Could not create software GPU fallback output directory.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;
    remove_stale_file(&options.output_dir.join(PREVIEW_FILE_NAME), "preview")?;
    remove_stale_file(&options.output_dir.join(MANIFEST_FILE_NAME), "manifest")?;
    remove_stale_file(
        &options.output_dir.join(MANIFEST_TEMP_FILE_NAME),
        "manifestTemp",
    )?;

    let frames_dir = options.output_dir.join(FRAMES_DIR_NAME);
    if frames_dir.exists() {
        std::fs::remove_dir_all(&frames_dir).map_err(|error| {
            vec![artifact_error(
                "frames",
                "Could not clear stale software GPU fallback frames.",
                "Remove stale graphics artifacts or choose a fresh outputDir.",
                error,
            )]
        })?;
    }
    std::fs::create_dir_all(&frames_dir).map_err(|error| {
        vec![artifact_error(
            "frames",
            "Could not create software GPU fallback frames directory.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    let frame_count = frame_count(layer);
    for frame_index in 0..frame_count {
        let frame = render_frame(layer, frame_index, frame_count);
        let frame_path = frames_dir.join(frame_file_name(frame_index));
        write_png(&frame, &frame_path)?;
        if frame_index == 0 {
            std::fs::copy(&frame_path, options.output_dir.join(PREVIEW_FILE_NAME)).map_err(
                |error| {
                    vec![artifact_error(
                        "preview",
                        "Could not write software GPU fallback preview frame.",
                        "Choose a writable outputDir for GPU graphics artifacts.",
                        error,
                    )]
                },
            )?;
        }
    }

    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("{}.preview", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: true,
        frame_count,
        frames_pattern: FRAMES_PATTERN.to_string(),
        playback: graphics_playback_manifest(
            frame_count,
            layer.fps,
            layer.duration_seconds,
            true,
            FRAMES_PATTERN,
        ),
        preview_path: PathBuf::from(PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };
    write_manifest(&manifest, &options.output_dir)?;

    Ok(manifest)
}

fn validate_hq_profile_shape(layer: &GpuGraphicsLayer) -> GpuGraphicsResult<()> {
    if is_canonical_hq_neon_wireframe_layer(layer) {
        return Ok(());
    }

    Err(vec![GpuGraphicsError::new(
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
        "profile",
        "Software GPU fallback only supports the HQ neon wireframe shader profile.",
        "Expand hq-neon-wireframe-shader-v1 before using the software fallback.",
    )])
}

fn write_manifest(manifest: &GraphicsArtifactManifest, output_dir: &Path) -> GpuGraphicsResult<()> {
    let manifest_json = serde_json::to_string_pretty(manifest).map_err(|error| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
            "manifest",
            "Could not serialize software GPU fallback manifest.",
            "Inspect manifest fields for unsupported values.",
        )
        .with_detail("serdeError", error.to_string())]
    })?;
    let manifest_path = output_dir.join(MANIFEST_FILE_NAME);
    let temp_path = output_dir.join(MANIFEST_TEMP_FILE_NAME);
    std::fs::write(&temp_path, manifest_json).map_err(|error| {
        vec![artifact_error(
            "manifest",
            "Could not write temporary software GPU fallback manifest.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    std::fs::rename(&temp_path, manifest_path).map_err(|error| {
        let _ = std::fs::remove_file(&temp_path);
        vec![artifact_error(
            "manifest",
            "Could not finalize software GPU fallback manifest.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })
}

fn remove_stale_file(path: &Path, artifact_path: &'static str) -> GpuGraphicsResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(vec![artifact_error(
            artifact_path,
            "Could not remove stale software GPU fallback artifact.",
            "Remove stale graphics artifacts or choose a fresh outputDir.",
            error,
        )]),
    }
}

fn render_frame(layer: &GpuGraphicsLayer, frame_index: u32, total_frames: u32) -> SoftwareFrame {
    let width = layer.dimensions.width;
    let height = layer.dimensions.height;
    let time = frame_time_seconds(frame_index, layer.fps, layer.duration_seconds);
    let progress = frame_progress(frame_index, total_frames);
    let mut frame = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba(background_pixel(x, y, width, height, time, progress))
    });

    draw_cube(
        &mut frame,
        CubeSpec {
            position: [-0.78, 0.06, 0.0],
            scale: 0.48,
            rotation: [0.18, 0.44 + progress * 3.45, progress * 0.42],
            color: [99, 230, 190],
            opacity: 0.82,
        },
        4.1,
    );
    draw_cube(
        &mut frame,
        CubeSpec {
            position: [0.92, -0.14, -0.54],
            scale: 0.28,
            rotation: [-0.15, 0.92 - progress * 2.2, 0.2 + progress * 1.6],
            color: [255, 79, 216],
            opacity: 0.72,
        },
        4.1,
    );
    frame
}

fn background_pixel(x: u32, y: u32, width: u32, height: u32, time: f32, progress: f32) -> [u8; 4] {
    let uv_x = (x as f32 + 0.5) / width as f32;
    let uv_y = (y as f32 + 0.5) / height as f32;
    let aspect = width as f32 / height.max(1) as f32;
    let px = (uv_x * 2.0 - 1.0) * aspect;
    let py = uv_y * 2.0 - 1.0;
    let drift = time * 0.035 + progress * 0.42;
    let wave = [
        0.5 + 0.5 * ((drift + px * 0.035) * std::f32::consts::TAU).cos(),
        0.5 + 0.5 * ((drift + py * 0.052 + 0.33) * std::f32::consts::TAU).cos(),
        0.5 + 0.5 * ((drift + px * 0.028 + py * 0.025 + 0.67) * std::f32::consts::TAU).cos(),
    ];
    let vignette = 1.0 - smoothstep(0.18, 1.46, (px * px + py * py).sqrt());
    let diagonal = neon_line((px * 0.32 + py * 0.74 + drift) * 7.0, 0.035);
    let grid = perspective_grid(uv_x, uv_y, progress, time);
    let vertical_glow = 1.0 - smoothstep(0.12, 0.96, (px + (time * 0.18).sin() * 0.22).abs());

    let mut color = [
        0.035 + wave[0] * 0.10 + wave[1] * 0.035,
        0.055 + wave[0] * 0.30 + grid * 0.34,
        0.105 + wave[1] * 0.22 + wave[2] * 0.08 + diagonal * 0.24,
    ];
    color[0] += diagonal * 0.10 + vertical_glow * 0.055 + grid * 0.055;
    color[1] += diagonal * 0.12 + vertical_glow * 0.03;
    color[2] += vertical_glow * 0.09;

    for channel in &mut color {
        *channel *= 0.52 + vignette * 0.62;
    }

    [to_byte(color[0]), to_byte(color[1]), to_byte(color[2]), 255]
}

fn perspective_grid(uv_x: f32, uv_y: f32, progress: f32, time: f32) -> f32 {
    let floor = smoothstep(0.48, 1.0, uv_y);
    if floor <= 0.0 {
        return 0.0;
    }

    let depth = 1.0 / ((uv_y - 0.45).max(0.035) * 6.2);
    let lateral = (uv_x - 0.5) * depth * 8.0 + (time * 0.12).sin() * 0.18;
    let forward = depth * 1.35 + progress * 3.2;
    let x_line = neon_line(lateral, 0.030);
    let z_line = neon_line(forward, 0.025);
    (x_line.max(z_line) * floor * (0.26 + depth.min(2.0) * 0.12)).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
struct CubeSpec {
    position: [f32; 3],
    scale: f32,
    rotation: [f32; 3],
    color: [u8; 3],
    opacity: f32,
}

fn draw_cube(frame: &mut SoftwareFrame, cube: CubeSpec, camera_distance: f32) {
    let mut projected = Vec::with_capacity(CUBE_VERTICES.len());
    for vertex in CUBE_VERTICES {
        let mut point = [
            vertex[0] * cube.scale,
            vertex[1] * cube.scale,
            vertex[2] * cube.scale,
        ];
        point = rotate_xyz(point, cube.rotation);
        point[0] += cube.position[0];
        point[1] += cube.position[1];
        point[2] += cube.position[2];
        projected.push(project_point(
            point,
            frame.width(),
            frame.height(),
            camera_distance,
        ));
    }

    for (start_index, end_index) in CUBE_EDGES {
        if let (Some(start), Some(end)) = (projected[start_index], projected[end_index]) {
            draw_line(frame, start, end, cube.color, cube.opacity * 0.22, 6.5);
            draw_line(frame, start, end, cube.color, cube.opacity, 1.35);
        }
    }
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

fn project_point(
    point: [f32; 3],
    width: u32,
    height: u32,
    camera_distance: f32,
) -> Option<[f32; 2]> {
    let camera_z = point[2] + camera_distance;
    if camera_z <= 0.05 {
        return None;
    }

    let focal = 1.18 / camera_z;
    Some([
        width as f32 * 0.5 + point[0] * focal * width as f32 * 0.5,
        height as f32 * 0.53 - point[1] * focal * height as f32 * 0.82,
    ])
}

fn draw_line(
    frame: &mut SoftwareFrame,
    start: [f32; 2],
    end: [f32; 2],
    color: [u8; 3],
    opacity: f32,
    radius: f32,
) {
    let min_x = (start[0].min(end[0]) - radius * 3.0).floor().max(0.0) as u32;
    let min_y = (start[1].min(end[1]) - radius * 3.0).floor().max(0.0) as u32;
    let max_x = (start[0].max(end[0]) + radius * 3.0)
        .ceil()
        .min(frame.width().saturating_sub(1) as f32) as u32;
    let max_y = (start[1].max(end[1]) + radius * 3.0)
        .ceil()
        .min(frame.height().saturating_sub(1) as f32) as u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let distance = distance_to_segment(point, start, end);
            let core = 1.0 - smoothstep(0.0, radius, distance);
            let glow = 1.0 - smoothstep(radius, radius * 3.0, distance);
            let intensity = (core + glow * 0.28).clamp(0.0, 1.0) * opacity;
            if intensity > 0.0 {
                blend_additive(frame.get_pixel_mut(x, y), color, intensity);
            }
        }
    }
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

fn blend_additive(pixel: &mut Rgba<u8>, color: [u8; 3], intensity: f32) {
    pixel[0] = to_byte(pixel[0] as f32 / 255.0 + color[0] as f32 / 255.0 * intensity);
    pixel[1] = to_byte(pixel[1] as f32 / 255.0 + color[1] as f32 / 255.0 * intensity);
    pixel[2] = to_byte(pixel[2] as f32 / 255.0 + color[2] as f32 / 255.0 * intensity);
    pixel[3] = 255;
}

fn write_png(frame: &SoftwareFrame, path: &Path) -> GpuGraphicsResult<()> {
    frame.save(path).map_err(|error| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
            "frame",
            "Could not write software GPU fallback frame PNG.",
            "Choose a writable outputDir and retry software GPU fallback rendering.",
        )
        .with_detail("imageError", error.to_string())]
    })
}

fn artifact_error(
    path: impl Into<String>,
    message: impl Into<String>,
    fix: impl Into<String>,
    error: impl std::fmt::Display,
) -> GpuGraphicsError {
    GpuGraphicsError::new(
        GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
        path,
        message,
        fix,
    )
    .with_detail("ioError", error.to_string())
}

fn frame_count(layer: &GpuGraphicsLayer) -> u32 {
    (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32
}

fn frame_progress(frame_index: u32, total_frames: u32) -> f32 {
    if total_frames <= 1 {
        0.0
    } else {
        frame_index as f32 / (total_frames - 1) as f32
    }
}

fn frame_time_seconds(frame_index: u32, fps: f64, duration_seconds: f64) -> f32 {
    ((frame_index as f64 / fps).min(duration_seconds)) as f32
}

fn frame_file_name(frame_index: u32) -> String {
    format!("frame-{frame_index:06}.png")
}

fn neon_line(value: f32, width: f32) -> f32 {
    let distance_to_line = (value.fract() - 0.5).abs();
    1.0 - smoothstep(0.0, width, distance_to_line)
}

fn smoothstep(edge_0: f32, edge_1: f32, value: f32) -> f32 {
    let t = ((value - edge_0) / (edge_1 - edge_0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
