# HQ GPU Visuals Productionization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the approved `hq-neon-wireframe-shader-v1` GPU visual profile a Rust-owned, fallback-capable, QA-checked render behavior.

**Architecture:** Add a GPU visual profile layer between proposal parsing and graphics rendering. The profile layer validates `qualityProfile`, expands the default HQ look into deterministic shader and primitive scene data, renders through GPU when available, falls back to a software renderer when necessary, runs visual QA, and feeds the existing `GraphicsArtifactManifest` boundary into GStreamer/GES.

**Tech Stack:** Rust, serde/serde_json, `image`, `wgpu`, GStreamer/GES, existing Video Creater `graphics`, `gpu_graphics`, and `render_pipeline` modules.

---

## File Structure

- Create `src-tauri/src/gpu_graphics/profile.rs`: profile ids, profile parsing, canonical `hq-neon-wireframe-shader-v1` expansion.
- Create `src-tauri/src/gpu_graphics/software_renderer.rs`: deterministic CPU fallback renderer for built-in GPU visual profiles.
- Create `src-tauri/src/gpu_graphics/visual_qa.rs`: sampled-frame metrics and profile-specific QA checks.
- Create `src-tauri/src/gpu_graphics/primitive_renderer.rs`: GPU primitive buffer generation and draw helpers for cube/grid wireframes.
- Modify `src-tauri/src/gpu_graphics/mod.rs`: expose new modules.
- Modify `src-tauri/src/gpu_graphics/error.rs`: add a visual QA failure error code if needed.
- Modify `src-tauri/src/gpu_graphics/renderer.rs`: render primitive scenes over shader backgrounds and surface renderer mode metadata.
- Modify `src-tauri/src/render_pipeline/proposal.rs`: validate and expand `qualityProfile`, use GPU-or-CPU fallback render path, include QA and renderer mode details.
- Modify `src-tauri/src/render_pipeline/report.rs`: include graphics render details and QA summaries in render reports.
- Modify `src-tauri/src/edit/render_plan.rs`: add render quality profile metadata for draft versus final WebM.
- Modify `src-tauri/src/render_pipeline/gstreamer_backend.rs`: include final-quality WebM command metadata and backend profile selection.
- Modify `src-tauri/src/codex/app_server.rs`: keep schema/profile enum aligned with the Rust registry.
- Modify `src-tauri/tests/gpu_graphics.rs`: profile, fallback, primitive rendering, and QA tests.
- Modify `src-tauri/tests/render_pipeline.rs`: direct proposal validation, fallback integration, report, and final-quality export tests.
- Modify `src-tauri/tests/codex_app_server.rs`: schema alignment tests for allowed profiles.
- Modify `docs/superpowers/specs/2026-06-19-hq-gpu-visuals-productionization-design.md`: implementation status notes after completion.

## Task 1: Profile Registry And Proposal Validation

**Files:**
- Create: `src-tauri/src/gpu_graphics/profile.rs`
- Modify: `src-tauri/src/gpu_graphics/mod.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing tests for missing and unknown `qualityProfile`**

Add these tests near `proposal_gpu_visuals_reject_timing_outside_selected_duration` in `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn proposal_gpu_visuals_reject_missing_quality_profile() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual
        .as_object_mut()
        .expect("gpu visual object")
        .remove("qualityProfile");
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("missing quality profile should fail");

    assert_eq!(errors[0].path, "gpuVisuals[0].qualityProfile");
    assert!(errors[0].message.contains("quality profile"));
}

#[test]
fn proposal_gpu_visuals_reject_unknown_quality_profile() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["qualityProfile"] = serde_json::json!("experimental-profile");
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("unknown quality profile should fail");

    assert_eq!(errors[0].path, "gpuVisuals[0].qualityProfile");
    assert!(errors[0].fix.contains("hq-neon-wireframe-shader-v1"));
}
```

- [ ] **Step 2: Write a failing test for canonical profile expansion**

Add this test in `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn proposal_gpu_visuals_expand_hq_profile_to_canonical_layer() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["shader"]["fragmentSource"] = serde_json::json!(
        "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(1.0, 0.0, 0.0, 1.0); }"
    );
    gpu_visual["visualTreatment"] = serde_json::json!("generic blob field");
    proposal.gpu_visuals = vec![gpu_visual];

    let layers = proposal_gpu_visuals_to_layers_for_duration(&proposal, 1920, 1080, 30.0, 45.0)
        .expect("hq profile should expand");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "shader-hook-bg");
    assert!(layers[0]
        .visual_treatment
        .contains("restrained animated gradient shader background"));
    assert!(layers[0]
        .avoid
        .contains("oversized saturated blobs"));
    assert!(layers[0]
        .background
        .as_ref()
        .expect("background")
        .fragment_source
        .contains("hq_neon_palette"));
    let scene = layers[0].scene.as_ref().expect("scene");
    assert!(scene.primitives.iter().any(|primitive| primitive.primitive_type == "cube"));
    assert!(scene.primitives.iter().any(|primitive| primitive.primitive_type == "grid"));
}
```

- [ ] **Step 3: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline quality_profile
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_gpu_visuals_expand_hq_profile_to_canonical_layer
```

Expected: FAIL because `qualityProfile` is not validated and profile expansion is not implemented.

- [ ] **Step 4: Add the profile module**

Create `src-tauri/src/gpu_graphics/profile.rs` with:

```rust
use super::ir::{
    Camera, CameraPreset, GpuGraphicRole, GpuGraphicsLayer, Material, MaterialKind, Primitive,
    PrimitiveAnimation, PrimitiveScene, RotationAnimation, ShaderLanguage, ShaderPass, Transform3,
};
use crate::graphics::ir::Dimensions;
use serde_json::Value;
use std::collections::BTreeMap;

pub const HQ_NEON_WIREFRAME_SHADER_V1: &str = "hq-neon-wireframe-shader-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVisualProfileId {
    HqNeonWireframeShaderV1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuProfileExpansionInput {
    pub id: String,
    pub kind: String,
    pub timeline_start: f64,
    pub duration_seconds: f64,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub source_beat: String,
    pub visual: Value,
}

pub fn parse_profile_id(value: &str) -> Option<GpuVisualProfileId> {
    match value {
        HQ_NEON_WIREFRAME_SHADER_V1 => Some(GpuVisualProfileId::HqNeonWireframeShaderV1),
        _ => None,
    }
}

pub fn expand_profile(
    profile_id: GpuVisualProfileId,
    input: GpuProfileExpansionInput,
) -> GpuGraphicsLayer {
    match profile_id {
        GpuVisualProfileId::HqNeonWireframeShaderV1 => expand_hq_neon_wireframe(input),
    }
}

pub fn supports_cpu_fallback(profile_id: GpuVisualProfileId) -> bool {
    matches!(profile_id, GpuVisualProfileId::HqNeonWireframeShaderV1)
}

fn expand_hq_neon_wireframe(input: GpuProfileExpansionInput) -> GpuGraphicsLayer {
    GpuGraphicsLayer {
        schema_version: 1,
        id: input.id,
        role: GpuGraphicRole::HybridScene,
        timeline_start: input.timeline_start,
        duration_seconds: input.duration_seconds,
        dimensions: input.dimensions,
        fps: input.fps,
        alpha: true,
        source_beat: input.source_beat,
        visual_treatment:
            "restrained animated gradient shader background with crisp neon wireframe 3D primitives"
                .to_string(),
        motion: "slow shader drift, orbiting cube rotation, grid parallax, and subtle motion trails"
            .to_string(),
        safe_zone: "keep essential geometry inside 10% margins; keep center contrast low enough for titles"
            .to_string(),
        avoid: "oversized saturated blobs, low-resolution draft looks, filled abstract shapes that do not read as 3D, strobing, dense noise, and heavy compression artifacts"
            .to_string(),
        background: Some(ShaderPass {
            shader_language: ShaderLanguage::Glsl,
            fragment_source: hq_neon_wireframe_fragment_source(),
            uniforms: BTreeMap::new(),
        }),
        scene: Some(PrimitiveScene {
            camera: Camera {
                preset: CameraPreset::Orbit,
                position: None,
                target: Some([0.0, 0.0, 0.0]),
                distance: Some(4.8),
                fov_degrees: Some(45.0),
            },
            primitives: vec![hq_cube("main-cube", "#35ebff", [-1.15, -0.10, 0.0], [1.12, 1.12, 1.12], 0.80),
                             hq_cube("accent-cube", "#ff4eb3", [0.95, 0.42, -0.35], [0.68, 0.68, 0.68], -0.55),
                             hq_grid()],
        }),
    }
}

fn hq_cube(id: &str, color: &str, position: [f32; 3], scale: [f32; 3], turns: f32) -> Primitive {
    Primitive {
        id: id.to_string(),
        primitive_type: "cube".to_string(),
        transform: Transform3 {
            position,
            rotation: [0.18, 0.36, 0.08],
            scale,
        },
        material: Material {
            kind: MaterialKind::Wire,
            color: color.to_string(),
            opacity: Some(0.94),
        },
        animate: Some(PrimitiveAnimation {
            rotation: Some(RotationAnimation {
                axis: [0.35, 1.0, 0.18],
                turns,
            }),
        }),
    }
}

fn hq_grid() -> Primitive {
    Primitive {
        id: "perspective-grid".to_string(),
        primitive_type: "grid".to_string(),
        transform: Transform3 {
            position: [0.0, -1.18, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [6.0, 1.0, 6.0],
        },
        material: Material {
            kind: MaterialKind::Wire,
            color: "#2bd6ff".to_string(),
            opacity: Some(0.42),
        },
        animate: None,
    }
}

fn hq_neon_wireframe_fragment_source() -> String {
    r#"vec3 hq_neon_palette(float x) {
    vec3 deep = vec3(0.015, 0.045, 0.075);
    vec3 teal = vec3(0.02, 0.42, 0.48);
    vec3 violet = vec3(0.23, 0.10, 0.42);
    return mix(deep, mix(teal, violet, 0.5 + 0.5 * sin(x)), 0.62);
}

vec4 video_creater_fragment(vec2 uv, float time, float progress) {
    vec2 p = uv * 2.0 - 1.0;
    p.x *= 1.7777778;
    float sweep = sin(p.x * 1.25 - p.y * 0.85 + time * 0.42);
    float ring = sin(length(p + vec2(0.15 * sin(time * 0.18), 0.05)) * 4.2 - time * 0.32);
    float vignette = smoothstep(1.45, 0.18, length(p * vec2(0.78, 1.0)));
    vec3 color = hq_neon_palette(sweep + ring + progress);
    color *= 0.42 + 0.58 * vignette;
    color += vec3(0.01, 0.04, 0.06) * (0.5 + 0.5 * ring);
    return vec4(color, 1.0);
}"#
    .to_string()
}
```

- [ ] **Step 5: Expose the module**

Modify `src-tauri/src/gpu_graphics/mod.rs`:

```rust
pub mod error;
pub mod ir;
pub mod mesh;
pub mod profile;
pub mod renderer;
pub mod shader;
pub mod validation;
```

- [ ] **Step 6: Validate and expand `qualityProfile` in proposal conversion**

In `src-tauri/src/render_pipeline/proposal.rs`, add imports:

```rust
use crate::gpu_graphics::profile::{
    expand_profile, parse_profile_id, GpuProfileExpansionInput,
};
```

Inside `gpu_visual_to_layer`, after reading `duration_seconds`, add:

```rust
    let quality_profile = required_string(visual, &base_path, "qualityProfile")?;
    let profile_id = parse_profile_id(quality_profile.as_str()).ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.qualityProfile"),
            "GPU visual quality profile is unsupported.",
            "Use hq-neon-wireframe-shader-v1 or add the profile to the Rust registry before using it.",
        )
        .with_detail("qualityProfile", quality_profile.clone())]
    })?;
```

Then replace the final `Ok(GpuGraphicsLayer { ... })` body with:

```rust
    let expanded = expand_profile(
        profile_id,
        GpuProfileExpansionInput {
            id,
            kind,
            timeline_start,
            duration_seconds,
            dimensions: Dimensions { width, height },
            fps,
            source_beat,
            visual: visual.clone(),
        },
    );
    Ok(expanded)
```

Keep the existing shader and primitive validation code below this function for custom future paths; it can remain unused until a second profile is introduced.

- [ ] **Step 7: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline quality_profile
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_gpu_visuals_expand_hq_profile_to_canonical_layer
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server gpu_visual
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/gpu_graphics/profile.rs src-tauri/src/gpu_graphics/mod.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs src-tauri/tests/codex_app_server.rs
rtk git commit -m "feat: add gpu visual profile registry"
```

## Task 2: CPU Fallback Renderer For HQ Profile

**Files:**
- Create: `src-tauri/src/gpu_graphics/software_renderer.rs`
- Modify: `src-tauri/src/gpu_graphics/mod.rs`
- Modify: `src-tauri/src/gpu_graphics/renderer.rs`
- Modify: `src-tauri/tests/gpu_graphics.rs`

- [ ] **Step 1: Write failing software fallback tests**

Add these tests in `src-tauri/tests/gpu_graphics.rs`:

```rust
use video_creater_lib::gpu_graphics::profile::{
    expand_profile, GpuProfileExpansionInput, GpuVisualProfileId,
};
use video_creater_lib::gpu_graphics::software_renderer::{
    render_hq_profile_software, SoftwareGpuRenderOptions,
};

#[test]
fn software_renderer_writes_hq_profile_manifest_and_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = expand_profile(
        GpuVisualProfileId::HqNeonWireframeShaderV1,
        GpuProfileExpansionInput {
            id: "hq-profile".to_string(),
            kind: "hybrid_scene".to_string(),
            timeline_start: 0.0,
            duration_seconds: 0.25,
            dimensions: video_creater_lib::graphics::ir::Dimensions { width: 160, height: 90 },
            fps: 4.0,
            source_beat: "test hq fallback".to_string(),
            visual: serde_json::json!({}),
        },
    );

    let manifest = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect("software render should succeed");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(manifest.frame_count, 1);
    assert!(dir.path().join("manifest.json").exists());
    assert!(dir.path().join("preview.png").exists());
    assert!(dir.path().join("frames/frame-000000.png").exists());
}

#[test]
fn software_renderer_hq_profile_frames_are_nonblank() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = expand_profile(
        GpuVisualProfileId::HqNeonWireframeShaderV1,
        GpuProfileExpansionInput {
            id: "hq-profile".to_string(),
            kind: "hybrid_scene".to_string(),
            timeline_start: 0.0,
            duration_seconds: 1.0,
            dimensions: video_creater_lib::graphics::ir::Dimensions { width: 160, height: 90 },
            fps: 4.0,
            source_beat: "test hq fallback".to_string(),
            visual: serde_json::json!({}),
        },
    );

    render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect("software render should succeed");

    let first = image::open(dir.path().join("frames/frame-000000.png"))
        .expect("first frame")
        .to_rgba8();
    let last = image::open(dir.path().join("frames/frame-000003.png"))
        .expect("last frame")
        .to_rgba8();
    assert!(first.pixels().any(|pixel| pixel[3] > 0 && (pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)));
    assert_ne!(first.as_raw(), last.as_raw(), "software fallback should animate over time");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics software_renderer
```

Expected: FAIL because `software_renderer` does not exist.

- [ ] **Step 3: Create the software renderer module**

Create `src-tauri/src/gpu_graphics/software_renderer.rs` with:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::GpuGraphicsLayer;
use crate::graphics::manifest::GraphicsArtifactManifest;
use image::{ImageBuffer, Rgba};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const PREVIEW_FILE_NAME: &str = "preview.png";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const FRAMES_DIR_NAME: &str = "frames";
const FRAMES_PATTERN: &str = "frames/frame-%06d.png";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareGpuRenderOptions {
    pub output_dir: PathBuf,
}

pub fn render_hq_profile_software(
    layer: &GpuGraphicsLayer,
    options: SoftwareGpuRenderOptions,
) -> GpuGraphicsResult<GraphicsArtifactManifest> {
    std::fs::create_dir_all(&options.output_dir)
        .map_err(|error| vec![artifact_error("outputDir", "Could not create software GPU output directory.", "Choose a writable output directory.", error)])?;
    let frames_dir = options.output_dir.join(FRAMES_DIR_NAME);
    if frames_dir.exists() {
        std::fs::remove_dir_all(&frames_dir)
            .map_err(|error| vec![artifact_error("frames", "Could not clear stale software GPU frames.", "Choose a fresh output directory.", error)])?;
    }
    std::fs::create_dir_all(&frames_dir)
        .map_err(|error| vec![artifact_error("frames", "Could not create software GPU frames directory.", "Choose a writable output directory.", error)])?;

    let frame_count = (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32;
    for frame_index in 0..frame_count {
        let rgba = render_frame(layer.dimensions.width, layer.dimensions.height, frame_index, frame_count);
        let frame_path = frames_dir.join(format!("frame-{frame_index:06}.png"));
        write_rgba_png(layer.dimensions.width, layer.dimensions.height, rgba, &frame_path)?;
        if frame_index == 0 {
            std::fs::copy(&frame_path, options.output_dir.join(PREVIEW_FILE_NAME))
                .map_err(|error| vec![artifact_error("preview", "Could not write software GPU preview frame.", "Choose a writable output directory.", error)])?;
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
        preview_path: PathBuf::from(PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|error| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
            "manifest",
            "Could not serialize software GPU manifest.",
            "Inspect manifest fields for unsupported values.",
        )
        .with_detail("serdeError", error.to_string())]
    })?;
    std::fs::write(options.output_dir.join(MANIFEST_FILE_NAME), manifest_json)
        .map_err(|error| vec![artifact_error("manifest", "Could not write software GPU manifest.", "Choose a writable output directory.", error)])?;
    Ok(manifest)
}

fn render_frame(width: u32, height: u32, frame_index: u32, frame_count: u32) -> Vec<u8> {
    let progress = if frame_count <= 1 { 0.0 } else { frame_index as f32 / (frame_count - 1) as f32 };
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    draw_background(&mut rgba, width, height, progress);
    draw_grid(&mut rgba, width, height, progress);
    draw_wire_cube(&mut rgba, width, height, progress, -0.55, 0.0, [53, 235, 255, 235]);
    draw_wire_cube(&mut rgba, width, height, progress + 0.35, 0.52, 0.28, [255, 78, 179, 210]);
    rgba
}

fn draw_background(rgba: &mut [u8], width: u32, height: u32, progress: f32) {
    for y in 0..height {
        for x in 0..width {
            let nx = x as f32 / width as f32;
            let ny = y as f32 / height as f32;
            let wave = ((nx * 7.0 + progress * 1.4).sin() + (ny * 5.0 - progress * 0.8).cos()) * 0.5;
            let vignette = (1.0 - ((nx - 0.5).abs() * 1.2 + (ny - 0.5).abs() * 1.35)).clamp(0.2, 1.0);
            let i = ((y * width + x) * 4) as usize;
            rgba[i] = ((12.0 + 24.0 * wave.max(0.0)) * vignette) as u8;
            rgba[i + 1] = ((32.0 + 54.0 * (0.5 + 0.5 * wave)) * vignette) as u8;
            rgba[i + 2] = ((58.0 + 92.0 * (1.0 - wave.abs())) * vignette) as u8;
            rgba[i + 3] = 255;
        }
    }
}

fn draw_grid(rgba: &mut [u8], width: u32, height: u32, progress: f32) {
    let horizon = height as f32 * 0.58;
    for line_index in -10..=10 {
        let sx = width as f32 * 0.5 + line_index as f32 * 56.0;
        draw_line(rgba, width, height, sx, height as f32, width as f32 * 0.5 + line_index as f32 * 4.0, horizon, [40, 210, 255, 70], 1);
    }
    for row in 0..8 {
        let p = row as f32 / 7.0;
        let y = horizon + p.powf(1.7) * (height as f32 - horizon);
        draw_line(rgba, width, height, 0.0, y + progress * 8.0, width as f32, y + progress * 8.0, [40, 210, 255, 60], 1);
    }
}

fn draw_wire_cube(rgba: &mut [u8], width: u32, height: u32, progress: f32, cx: f32, cy: f32, color: [u8; 4]) {
    let size = height as f32 * 0.16;
    let angle = progress * std::f32::consts::TAU;
    let points = [
        project(-1.0, -1.0, -1.0, angle, cx, cy, width, height, size),
        project(1.0, -1.0, -1.0, angle, cx, cy, width, height, size),
        project(1.0, 1.0, -1.0, angle, cx, cy, width, height, size),
        project(-1.0, 1.0, -1.0, angle, cx, cy, width, height, size),
        project(-1.0, -1.0, 1.0, angle, cx, cy, width, height, size),
        project(1.0, -1.0, 1.0, angle, cx, cy, width, height, size),
        project(1.0, 1.0, 1.0, angle, cx, cy, width, height, size),
        project(-1.0, 1.0, 1.0, angle, cx, cy, width, height, size),
    ];
    for (a, b) in [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7),(0,6)] {
        let pa = points[a];
        let pb = points[b];
        draw_line(rgba, width, height, pa.0, pa.1, pb.0, pb.1, [color[0] / 3, color[1] / 3, color[2] / 3, 90], 7);
        draw_line(rgba, width, height, pa.0, pa.1, pb.0, pb.1, color, 2);
    }
}

fn project(x: f32, y: f32, z: f32, angle: f32, cx: f32, cy: f32, width: u32, height: u32, size: f32) -> (f32, f32) {
    let ca = angle.cos();
    let sa = angle.sin();
    let rx = x * ca + z * sa;
    let rz = -x * sa + z * ca + 4.0;
    let ry = y * angle.mul_add(0.7, 0.3).cos() - rz * 0.06;
    let f = size / rz.max(1.0);
    (width as f32 * (0.5 + cx * 0.25) + rx * f, height as f32 * (0.5 - cy * 0.25) - ry * f)
}

fn draw_line(rgba: &mut [u8], width: u32, height: u32, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4], thickness: i32) {
    let mut x0 = x0.round() as i32;
    let mut y0 = y0.round() as i32;
    let x1 = x1.round() as i32;
    let y1 = y1.round() as i32;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        for oy in -thickness..=thickness {
            for ox in -thickness..=thickness {
                blend_pixel(rgba, width, height, x0 + ox, y0 + oy, color);
            }
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = err * 2;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}

fn blend_pixel(rgba: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let i = ((y as u32 * width + x as u32) * 4) as usize;
    let alpha = color[3] as f32 / 255.0;
    rgba[i] = ((rgba[i] as f32 * (1.0 - alpha)) + color[0] as f32 * alpha).min(255.0) as u8;
    rgba[i + 1] = ((rgba[i + 1] as f32 * (1.0 - alpha)) + color[1] as f32 * alpha).min(255.0) as u8;
    rgba[i + 2] = ((rgba[i + 2] as f32 * (1.0 - alpha)) + color[2] as f32 * alpha).min(255.0) as u8;
    rgba[i + 3] = rgba[i + 3].max(color[3]);
}

fn write_rgba_png(width: u32, height: u32, rgba: Vec<u8>, path: &Path) -> GpuGraphicsResult<()> {
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba).ok_or_else(|| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
            "frame",
            "Software GPU frame buffer size did not match output dimensions.",
            "Ensure the renderer returns width * height * 4 bytes.",
        )]
    })?;
    image.save(path).map_err(|error| {
        vec![artifact_error(
            "frame",
            "Could not write software GPU frame PNG.",
            "Choose a writable output directory and retry rendering.",
            error,
        )]
    })
}

fn artifact_error(path: impl Into<String>, message: impl Into<String>, fix: impl Into<String>, error: impl std::fmt::Display) -> GpuGraphicsError {
    GpuGraphicsError::new(GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed, path, message, fix)
        .with_detail("ioError", error.to_string())
}
```

- [ ] **Step 4: Expose the module**

Modify `src-tauri/src/gpu_graphics/mod.rs`:

```rust
pub mod error;
pub mod ir;
pub mod mesh;
pub mod profile;
pub mod renderer;
pub mod shader;
pub mod software_renderer;
pub mod validation;
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics software_renderer
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/src/gpu_graphics/software_renderer.rs src-tauri/src/gpu_graphics/mod.rs src-tauri/tests/gpu_graphics.rs
rtk git commit -m "feat: add hq gpu visual software fallback"
```

## Task 3: Render Proposal Fallback Integration And Renderer Mode Reporting

**Files:**
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/src/render_pipeline/report.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing fallback integration test**

Add this test near `render_proposal_graphics_writes_gpu_artifacts_or_reports_unavailable` in `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn render_proposal_graphics_uses_software_fallback_for_hq_profile_when_gpu_unavailable() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    let project = sample_project_with_render_settings(160, 90, 4.0);

    let overlays = render_proposal_graphics(&project, &report, dir.path())
        .expect("hq profile should render with gpu or software fallback");

    assert_eq!(overlays.len(), 1);
    assert_eq!(overlays[0].artifact_dir, dir.path().join("shader-hook-bg"));
    assert!(dir.path().join("shader-hook-bg/manifest.json").exists());
    assert!(dir.path().join("shader-hook-bg/preview.png").exists());
}
```

- [ ] **Step 2: Write failing render report metadata test**

Add this test near `render_reports_write_json_and_markdown`:

```rust
#[test]
fn render_report_records_gpu_visual_renderer_modes() {
    let report = RenderReport {
        job_id: "render-proposal".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(1.0),
            output_path: Some("renders/final.webm".to_string()),
        },
        command: CommandSpec::new("gstreamer-ges"),
        stdout: String::new(),
        stderr: String::new(),
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: vec![RenderGraphicsReport {
            layer_id: "shader-hook-bg".to_string(),
            renderer: "software".to_string(),
            quality_profile: Some("hq-neon-wireframe-shader-v1".to_string()),
            visual_qa_status: None,
        }],
    };

    let json = serde_json::to_string(&report).expect("report json");
    assert!(json.contains("\"renderer\":\"software\""));
    assert!(json.contains("hq-neon-wireframe-shader-v1"));
}
```

- [ ] **Step 3: Run tests to verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline software_fallback
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_report_records_gpu_visual_renderer_modes
```

Expected: FAIL because fallback and report graphics metadata are not wired.

- [ ] **Step 4: Add report graphics metadata**

In `src-tauri/src/render_pipeline/report.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderGraphicsReport {
    pub layer_id: String,
    pub renderer: String,
    pub quality_profile: Option<String>,
    pub visual_qa_status: Option<String>,
}
```

Update `RenderReport`:

```rust
pub struct RenderReport {
    pub job_id: String,
    pub summary: RenderReportSummary,
    pub command: CommandSpec,
    pub stdout: String,
    pub stderr: String,
    pub errors: Vec<PipelineError>,
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub graphics: Vec<RenderGraphicsReport>,
}
```

Update `markdown_report` after artifacts:

```rust
    if !report.graphics.is_empty() {
        markdown.push_str("\n## Graphics\n\n");
        for graphics in &report.graphics {
            markdown.push_str(&format!(
                "- {}: renderer={} profile={} qa={}\n",
                graphics.layer_id,
                graphics.renderer,
                graphics.quality_profile.as_deref().unwrap_or("none"),
                graphics.visual_qa_status.as_deref().unwrap_or("not-run")
            ));
        }
    }
```

- [ ] **Step 5: Add renderer mode to rendered graphics**

In `src-tauri/src/render_pipeline/proposal.rs`, update `RenderedProposalGraphics`:

```rust
pub struct RenderedProposalGraphics {
    pub artifact_dir: PathBuf,
    pub manifest: GraphicsArtifactManifest,
    pub timeline_start_seconds: f64,
    pub renderer: String,
    pub quality_profile: Option<String>,
}
```

Introduce an internal struct:

```rust
#[derive(Debug, Clone)]
struct RenderedGraphicsArtifact {
    manifest: GraphicsArtifactManifest,
    artifact_dir: PathBuf,
    timeline_start_seconds: f64,
    renderer: String,
    quality_profile: Option<String>,
}
```

Change internal vectors from `Vec<(GraphicsArtifactManifest, PathBuf, f64)>` to `Vec<RenderedGraphicsArtifact>`, and convert to backend tuples with:

```rust
fn graphics_backend_inputs(
    graphics: &[RenderedGraphicsArtifact],
) -> Vec<(GraphicsArtifactManifest, PathBuf, f64)> {
    graphics
        .iter()
        .map(|graphics| {
            (
                graphics.manifest.clone(),
                graphics.artifact_dir.clone(),
                graphics.timeline_start_seconds,
            )
        })
        .collect()
}
```

- [ ] **Step 6: Render GPU layers with software fallback on adapter unavailable**

In the GPU loop in `render_proposal_graphics_artifacts`, replace direct GPU rendering with:

```rust
        let gpu_result = crate::gpu_graphics::renderer::render_gpu_graphics_layer(
            layer,
            crate::gpu_graphics::renderer::GpuRenderOptions {
                output_dir: layer_dir.clone(),
            },
        );
        let (manifest, renderer) = match gpu_result {
            Ok(manifest) => (manifest, "gpu".to_string()),
            Err(errors)
                if errors.iter().any(|error| {
                    error.code
                        == crate::gpu_graphics::error::GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable
                }) =>
            {
                let manifest = crate::gpu_graphics::software_renderer::render_hq_profile_software(
                    layer,
                    crate::gpu_graphics::software_renderer::SoftwareGpuRenderOptions {
                        output_dir: layer_dir.clone(),
                    },
                )
                .map_err(crate::gpu_graphics::error::gpu_errors_to_pipeline_errors)?;
                (manifest, "software".to_string())
            }
            Err(errors) => {
                return Err(crate::gpu_graphics::error::gpu_errors_to_pipeline_errors(errors));
            }
        };
```

Push `RenderedGraphicsArtifact` with `quality_profile: Some("hq-neon-wireframe-shader-v1".to_string())`.

- [ ] **Step 7: Include graphics metadata in success report**

When constructing `RenderReport` in `run_render_proposal_with_runner`, add:

```rust
        graphics: graphics
            .iter()
            .filter_map(|graphics| {
                graphics.quality_profile.as_ref().map(|quality_profile| {
                    crate::render_pipeline::report::RenderGraphicsReport {
                        layer_id: graphics
                            .manifest
                            .source_layer_ids
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string()),
                        renderer: graphics.renderer.clone(),
                        quality_profile: Some(quality_profile.clone()),
                        visual_qa_status: None,
                    }
                })
            })
            .collect(),
```

- [ ] **Step 8: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline software_fallback
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_report_records_gpu_visual_renderer_modes
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
rtk git add src-tauri/src/render_pipeline/proposal.rs src-tauri/src/render_pipeline/report.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: fall back gpu visuals to software renderer"
```

## Task 4: Visual QA For Built-In GPU Profiles

**Files:**
- Create: `src-tauri/src/gpu_graphics/visual_qa.rs`
- Modify: `src-tauri/src/gpu_graphics/mod.rs`
- Modify: `src-tauri/src/gpu_graphics/error.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/src/render_pipeline/report.rs`
- Modify: `src-tauri/tests/gpu_graphics.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing QA tests**

Add tests in `src-tauri/tests/gpu_graphics.rs`:

```rust
use video_creater_lib::gpu_graphics::visual_qa::{
    run_hq_visual_qa, VisualQaOptions,
};

#[test]
fn visual_qa_rejects_blank_hq_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("frames")).expect("frames dir");
    for index in 0..3 {
        image::RgbaImage::from_pixel(120, 68, image::Rgba([0, 0, 0, 0]))
            .save(dir.path().join(format!("frames/frame-{index:06}.png")))
            .expect("write blank frame");
    }

    let errors = run_hq_visual_qa(
        dir.path(),
        120,
        68,
        3,
        VisualQaOptions::default(),
    )
    .expect_err("blank frames should fail QA");

    assert_eq!(errors[0].path, "visualQa.samples");
    assert!(errors[0].message.contains("blank"));
}

#[test]
fn visual_qa_rejects_saturated_blob_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("frames")).expect("frames dir");
    for index in 0..3 {
        image::RgbaImage::from_pixel(120, 68, image::Rgba([255, 0, 220, 255]))
            .save(dir.path().join(format!("frames/frame-{index:06}.png")))
            .expect("write blob frame");
    }

    let errors = run_hq_visual_qa(
        dir.path(),
        120,
        68,
        3,
        VisualQaOptions::default(),
    )
    .expect_err("blob frames should fail QA");

    assert_eq!(errors[0].path, "visualQa.blobDominance");
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics visual_qa
```

Expected: FAIL because `visual_qa` does not exist.

- [ ] **Step 3: Add QA error code**

In `src-tauri/src/gpu_graphics/error.rs`, add enum variant:

```rust
GpuGraphicsVisualQaFailed,
```

- [ ] **Step 4: Create visual QA module**

Create `src-tauri/src/gpu_graphics/visual_qa.rs` with:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use image::RgbaImage;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct VisualQaOptions {
    pub min_opaque_ratio: f64,
    pub max_saturated_ratio: f64,
    pub min_edge_ratio: f64,
    pub min_temporal_delta: f64,
}

impl Default for VisualQaOptions {
    fn default() -> Self {
        Self {
            min_opaque_ratio: 0.20,
            max_saturated_ratio: 0.72,
            min_edge_ratio: 0.006,
            min_temporal_delta: 0.002,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualQaReport {
    pub sampled_frames: Vec<PathBuf>,
    pub opaque_ratio: f64,
    pub saturated_ratio: f64,
    pub edge_ratio: f64,
    pub temporal_delta: f64,
}

pub fn run_hq_visual_qa(
    artifact_dir: &Path,
    width: u32,
    height: u32,
    frame_count: u32,
    options: VisualQaOptions,
) -> GpuGraphicsResult<VisualQaReport> {
    let sample_indices = sample_indices(frame_count);
    let mut frames = Vec::new();
    let mut paths = Vec::new();
    for index in sample_indices {
        let path = artifact_dir.join("frames").join(format!("frame-{index:06}.png"));
        let frame = image::open(&path)
            .map_err(|error| {
                vec![GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed,
                    "visualQa.samples",
                    "Visual QA could not read a sampled frame.",
                    "Regenerate graphics artifacts before running visual QA.",
                )
                .with_detail("framePath", path.display().to_string())
                .with_detail("imageError", error.to_string())]
            })?
            .to_rgba8();
        if frame.width() != width || frame.height() != height {
            return Err(vec![GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed,
                "visualQa.dimensions",
                "Visual QA sampled frame dimensions do not match the manifest.",
                "Regenerate the graphics artifact at the render target dimensions.",
            )
            .with_detail("framePath", path.display().to_string())]);
        }
        paths.push(path);
        frames.push(frame);
    }

    let opaque_ratio = frames.iter().map(opaque_ratio).sum::<f64>() / frames.len() as f64;
    if opaque_ratio < options.min_opaque_ratio {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed,
            "visualQa.samples",
            "Visual QA detected blank or nearly transparent sampled frames.",
            "Render nonblank HQ GPU visual frames before composition.",
        )
        .with_detail("opaqueRatio", format!("{opaque_ratio:.6}"))]);
    }

    let saturated_ratio = frames.iter().map(saturated_ratio).sum::<f64>() / frames.len() as f64;
    let edge_ratio = frames.iter().map(edge_ratio).sum::<f64>() / frames.len() as f64;
    if saturated_ratio > options.max_saturated_ratio && edge_ratio < options.min_edge_ratio {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed,
            "visualQa.blobDominance",
            "Visual QA detected blob-dominant frames without readable wireframe detail.",
            "Use the built-in HQ profile expansion or revise the shader and primitive treatment.",
        )
        .with_detail("saturatedRatio", format!("{saturated_ratio:.6}"))
        .with_detail("edgeRatio", format!("{edge_ratio:.6}"))]);
    }

    let temporal_delta = temporal_delta(&frames);
    if temporal_delta < options.min_temporal_delta {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed,
            "visualQa.motion",
            "Visual QA sampled frames are too similar for an animated HQ profile.",
            "Animate the shader, camera, or wireframe primitives over the layer duration.",
        )
        .with_detail("temporalDelta", format!("{temporal_delta:.6}"))]);
    }

    Ok(VisualQaReport {
        sampled_frames: paths,
        opaque_ratio,
        saturated_ratio,
        edge_ratio,
        temporal_delta,
    })
}

fn sample_indices(frame_count: u32) -> Vec<u32> {
    let last = frame_count.saturating_sub(1);
    vec![0, last / 2, last].into_iter().collect()
}

fn opaque_ratio(frame: &RgbaImage) -> f64 {
    frame.pixels().filter(|pixel| pixel[3] > 12).count() as f64
        / frame.pixels().len() as f64
}

fn saturated_ratio(frame: &RgbaImage) -> f64 {
    frame
        .pixels()
        .filter(|pixel| {
            let max = pixel[0].max(pixel[1]).max(pixel[2]) as i16;
            let min = pixel[0].min(pixel[1]).min(pixel[2]) as i16;
            pixel[3] > 12 && max > 180 && max - min > 90
        })
        .count() as f64
        / frame.pixels().len() as f64
}

fn edge_ratio(frame: &RgbaImage) -> f64 {
    let width = frame.width();
    let height = frame.height();
    let mut edges = 0usize;
    let mut total = 0usize;
    for y in 0..height.saturating_sub(1) {
        for x in 0..width.saturating_sub(1) {
            let a = luminance(frame.get_pixel(x, y));
            let b = luminance(frame.get_pixel(x + 1, y));
            let c = luminance(frame.get_pixel(x, y + 1));
            if (a - b).abs() > 32.0 || (a - c).abs() > 32.0 {
                edges += 1;
            }
            total += 1;
        }
    }
    if total == 0 { 0.0 } else { edges as f64 / total as f64 }
}

fn temporal_delta(frames: &[RgbaImage]) -> f64 {
    if frames.len() < 2 {
        return 0.0;
    }
    let first = frames.first().expect("first frame").as_raw();
    let last = frames.last().expect("last frame").as_raw();
    first
        .iter()
        .zip(last)
        .map(|(a, b)| (*a as f64 - *b as f64).abs() / 255.0)
        .sum::<f64>()
        / first.len().max(1) as f64
}

fn luminance(pixel: &image::Rgba<u8>) -> f64 {
    0.2126 * pixel[0] as f64 + 0.7152 * pixel[1] as f64 + 0.0722 * pixel[2] as f64
}
```

- [ ] **Step 5: Expose QA module**

Modify `src-tauri/src/gpu_graphics/mod.rs`:

```rust
pub mod visual_qa;
```

- [ ] **Step 6: Run QA after GPU visual artifact generation**

In the GPU artifact branch in `src-tauri/src/render_pipeline/proposal.rs`, after manifest generation, add:

```rust
        let qa_report = crate::gpu_graphics::visual_qa::run_hq_visual_qa(
            &layer_dir,
            manifest.dimensions.width,
            manifest.dimensions.height,
            manifest.frame_count,
            crate::gpu_graphics::visual_qa::VisualQaOptions::default(),
        )
        .map_err(crate::gpu_graphics::error::gpu_errors_to_pipeline_errors)?;
```

Store `visual_qa_status: Some("passed".to_string())` in `RenderedGraphicsArtifact` and include sampled frame paths in artifacts:

```rust
        for sampled_frame in qa_report.sampled_frames {
            extra_artifacts.push(sampled_frame.display().to_string());
        }
```

- [ ] **Step 7: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics visual_qa
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gpu_visual
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/gpu_graphics/visual_qa.rs src-tauri/src/gpu_graphics/mod.rs src-tauri/src/gpu_graphics/error.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/src/render_pipeline/report.rs src-tauri/tests/gpu_graphics.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add visual qa for gpu profiles"
```

## Task 5: GPU Wireframe Primitive Rendering

**Files:**
- Create: `src-tauri/src/gpu_graphics/primitive_renderer.rs`
- Modify: `src-tauri/src/gpu_graphics/mod.rs`
- Modify: `src-tauri/src/gpu_graphics/renderer.rs`
- Modify: `src-tauri/tests/gpu_graphics.rs`

- [ ] **Step 1: Write failing primitive buffer tests**

Add tests in `src-tauri/tests/gpu_graphics.rs`:

```rust
use video_creater_lib::gpu_graphics::primitive_renderer::{
    build_wireframe_edges_for_primitive, project_wireframe_edges,
};

#[test]
fn primitive_renderer_builds_cube_wireframe_edges() {
    let primitive = minimal_hybrid_scene_layer()
        .scene
        .expect("scene")
        .primitives
        .into_iter()
        .find(|primitive| primitive.primitive_type == "cube")
        .expect("cube");

    let edges = build_wireframe_edges_for_primitive(&primitive).expect("cube edges");

    assert!(edges.len() >= 12);
    assert!(edges.iter().all(|edge| edge.start.iter().all(|value| value.is_finite())));
    assert!(edges.iter().all(|edge| edge.end.iter().all(|value| value.is_finite())));
}

#[test]
fn primitive_renderer_projects_edges_into_screen_space() {
    let primitive = minimal_hybrid_scene_layer()
        .scene
        .expect("scene")
        .primitives
        .into_iter()
        .find(|primitive| primitive.primitive_type == "cube")
        .expect("cube");
    let edges = build_wireframe_edges_for_primitive(&primitive).expect("cube edges");

    let projected = project_wireframe_edges(&edges, 320, 180, 0.25);

    assert!(!projected.is_empty());
    assert!(projected.iter().all(|edge| edge.start[0].is_finite()));
    assert!(projected.iter().all(|edge| edge.end[1].is_finite()));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics primitive_renderer
```

Expected: FAIL because `primitive_renderer` does not exist.

- [ ] **Step 3: Create primitive renderer helper module**

Create `src-tauri/src/gpu_graphics/primitive_renderer.rs` with:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::Primitive;

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

pub fn build_wireframe_edges_for_primitive(primitive: &Primitive) -> GpuGraphicsResult<Vec<WireEdge3>> {
    match primitive.primitive_type.as_str() {
        "cube" => Ok(cube_edges(primitive)),
        "grid" => Ok(grid_edges(primitive)),
        _ => Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveUnsupported,
            "scene.primitives.type",
            "Primitive type is not supported by the wireframe renderer.",
            "Use cube or grid for the HQ GPU profile.",
        )]),
    }
}

pub fn project_wireframe_edges(edges: &[WireEdge3], width: u32, height: u32, progress: f32) -> Vec<WireEdge2> {
    edges
        .iter()
        .map(|edge| WireEdge2 {
            start: project(edge.start, width, height, progress),
            end: project(edge.end, width, height, progress),
            color: edge.color,
        })
        .collect()
}

fn cube_edges(primitive: &Primitive) -> Vec<WireEdge3> {
    let vertices = [
        [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0],
    ];
    let color = color_rgba(&primitive.material.color, primitive.material.opacity.unwrap_or(1.0));
    [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7),(0,6)]
        .into_iter()
        .map(|(a, b)| WireEdge3 {
            start: transform(vertices[a], primitive),
            end: transform(vertices[b], primitive),
            color,
        })
        .collect()
}

fn grid_edges(primitive: &Primitive) -> Vec<WireEdge3> {
    let color = color_rgba(&primitive.material.color, primitive.material.opacity.unwrap_or(0.45));
    let mut edges = Vec::new();
    for index in -10..=10 {
        let p = index as f32 / 10.0;
        edges.push(WireEdge3 {
            start: transform([-1.0, 0.0, p], primitive),
            end: transform([1.0, 0.0, p], primitive),
            color,
        });
        edges.push(WireEdge3 {
            start: transform([p, 0.0, -1.0], primitive),
            end: transform([p, 0.0, 1.0], primitive),
            color,
        });
    }
    edges
}

fn transform(point: [f32; 3], primitive: &Primitive) -> [f32; 3] {
    [
        point[0] * primitive.transform.scale[0] + primitive.transform.position[0],
        point[1] * primitive.transform.scale[1] + primitive.transform.position[1],
        point[2] * primitive.transform.scale[2] + primitive.transform.position[2],
    ]
}

fn project(point: [f32; 3], width: u32, height: u32, progress: f32) -> [f32; 2] {
    let angle = progress * std::f32::consts::TAU;
    let ca = angle.cos();
    let sa = angle.sin();
    let x = point[0] * ca + point[2] * sa;
    let z = -point[0] * sa + point[2] * ca + 6.0;
    let y = point[1];
    let focal = height as f32 * 0.72;
    [
        width as f32 * 0.5 + x * focal / z.max(1.0),
        height as f32 * 0.52 - y * focal / z.max(1.0),
    ]
}

fn color_rgba(hex: &str, opacity: f32) -> [f32; 4] {
    let hex = hex.trim_start_matches('#');
    let parse = |range: std::ops::Range<usize>| {
        u8::from_str_radix(hex.get(range).unwrap_or("ff"), 16).unwrap_or(255) as f32 / 255.0
    };
    [parse(0..2), parse(2..4), parse(4..6), opacity.clamp(0.0, 1.0)]
}
```

- [ ] **Step 4: Expose primitive renderer module**

Modify `src-tauri/src/gpu_graphics/mod.rs`:

```rust
pub mod primitive_renderer;
```

- [ ] **Step 5: Render primitive detail in `renderer.rs`**

In `src-tauri/src/gpu_graphics/renderer.rs`, after the shader render pass and before readback, add a second CPU-composited pass as the first implementation bridge:

```rust
        let mut rgba = self.readback_rgba()?;
        if let Some(scene) = &layer.scene {
            let progress = if total_frames <= 1 {
                0.0
            } else {
                frame_index as f32 / (total_frames - 1) as f32
            };
            crate::gpu_graphics::primitive_renderer::draw_projected_wireframes_into_rgba(
                &mut rgba,
                self.width,
                self.height,
                scene,
                progress,
            )?;
        }
        Ok(rgba)
```

Add `draw_projected_wireframes_into_rgba` to `primitive_renderer.rs` using the same line drawing and blending helpers from `software_renderer.rs`. This keeps the first primitive-rendering slice deterministic while leaving full GPU line pipelines as a later optimization.

- [ ] **Step 6: Add render-or-skip integration check**

Update `gpu_renderer_writes_hybrid_scene_manifest_or_reports_unavailable` in `src-tauri/tests/gpu_graphics.rs` to inspect the first rendered frame when GPU is available:

```rust
let frame = image::open(dir.path().join("gpu-hybrid/frames/frame-000000.png"))
    .expect("hybrid frame")
    .to_rgba8();
let bright_pixels = frame
    .pixels()
    .filter(|pixel| pixel[3] > 0 && (pixel[0] > 160 || pixel[1] > 160 || pixel[2] > 160))
    .count();
assert!(bright_pixels > 12, "hybrid render should include visible wireframe primitives");
```

- [ ] **Step 7: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics primitive_renderer
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics gpu_renderer_writes_hybrid_scene_manifest_or_reports_unavailable
```

Expected: PASS, with adapter-unavailable environments following the existing skip path for GPU-specific assertions.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/gpu_graphics/primitive_renderer.rs src-tauri/src/gpu_graphics/mod.rs src-tauri/src/gpu_graphics/renderer.rs src-tauri/tests/gpu_graphics.rs
rtk git commit -m "feat: draw gpu wireframe primitives"
```

## Task 6: Final-Quality WebM Export Profile

**Files:**
- Modify: `src-tauri/src/edit/render_plan.rs`
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing render quality metadata tests**

Add tests in `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn render_plan_defaults_to_draft_webm_quality() {
    let plan = RenderPlan {
        input_path: "source.webm".to_string(),
        output_path: "renders/draft.webm".to_string(),
        width: 320,
        height: 180,
        fps: 30.0,
        clips: vec![RenderClip {
            source_in: 0.0,
            source_out: 1.0,
        }],
        quality_profile: RenderQualityProfile::default(),
    };

    assert_eq!(plan.quality_profile, RenderQualityProfile::DraftWebm);
}

#[test]
fn gstreamer_ges_command_includes_final_webm_quality_metadata() {
    let plan = RenderPlan {
        input_path: "source.webm".to_string(),
        output_path: "renders/final.webm".to_string(),
        width: 320,
        height: 180,
        fps: 30.0,
        clips: vec![RenderClip {
            source_in: 0.0,
            source_out: 1.0,
        }],
        quality_profile: RenderQualityProfile::FinalWebm,
    };

    let command = GstreamerGesRenderBackend::new()
        .build_command(&plan, &[])
        .expect("command");

    assert!(command.args.contains(&"--quality=finalWebm".to_string()));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline final_webm_quality
```

Expected: FAIL because `RenderQualityProfile` does not exist.

- [ ] **Step 3: Add render quality profile to render plans**

In `src-tauri/src/edit/render_plan.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RenderQualityProfile {
    DraftWebm,
    FinalWebm,
}

impl Default for RenderQualityProfile {
    fn default() -> Self {
        Self::DraftWebm
    }
}
```

Update `RenderPlan`:

```rust
pub struct RenderPlan {
    pub input_path: String,
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub clips: Vec<RenderClip>,
    #[serde(default)]
    pub quality_profile: RenderQualityProfile,
}
```

- [ ] **Step 4: Add command metadata in GStreamer backend**

In `src-tauri/src/render_pipeline/gstreamer_backend.rs`, import `RenderQualityProfile`:

```rust
use crate::edit::render_plan::{RenderPlan, RenderQualityProfile};
```

Add this in `build_command` after FPS:

```rust
            .arg(format!(
                "--quality={}",
                match plan.quality_profile {
                    RenderQualityProfile::DraftWebm => "draftWebm",
                    RenderQualityProfile::FinalWebm => "finalWebm",
                }
            ));
```

Update `webm_encoding_profile` name:

```rust
    let profile_name = match plan.quality_profile {
        RenderQualityProfile::DraftWebm => "video-creater-webm-vp8-opus-draft",
        RenderQualityProfile::FinalWebm => "video-creater-webm-vp8-opus-final",
    };
```

Use `.name(profile_name)` when building the container profile.

- [ ] **Step 5: Set proposal renders to final WebM only when explicitly requested**

In `src-tauri/src/render_pipeline/proposal.rs`, leave default proposal renders as `DraftWebm`. Add a follow-up CLI flag in a later task if product UX needs users to select final quality from the command line. This task only adds the backend-supported model and metadata.

- [ ] **Step 6: Update existing `RenderPlan` literals**

Search:

```bash
rtk rg -n "RenderPlan \\{" src-tauri/tests src-tauri/src
```

For each literal, add:

```rust
quality_profile: RenderQualityProfile::DraftWebm,
```

Also import `RenderQualityProfile` in affected test modules.

- [ ] **Step 7: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline final_webm_quality
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_builds_command_metadata_without_spawning_process
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
rtk git add src-tauri/src/edit/render_plan.rs src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add final webm render quality profile"
```

## Task 7: Documentation, Status, And Full Verification

**Files:**
- Modify: `docs/superpowers/specs/2026-06-19-hq-gpu-visuals-productionization-design.md`
- Modify: `docs/superpowers/specs/2026-06-18-rust-native-3d-shader-graphics-design.md`

- [ ] **Step 1: Update implementation status in the productionization spec**

Append this section to `docs/superpowers/specs/2026-06-19-hq-gpu-visuals-productionization-design.md`:

```markdown
## Implementation Status

- `hq-neon-wireframe-shader-v1` is represented by a Rust profile registry and direct proposal validation.
- The HQ profile expands into canonical shader, camera, cube, and grid scene data.
- The HQ profile has a deterministic software fallback renderer for GPU-unavailable environments.
- Render reports identify whether GPU visuals were rendered by GPU or software fallback.
- Built-in HQ profile renders run sampled-frame visual QA before final handoff.
- Hybrid GPU scenes include visible wireframe primitive detail.
- Final-quality WebM metadata is available under the existing GStreamer/GES policy.
- MP4/H.264 remains policy-gated and is not enabled by this implementation.
```

- [ ] **Step 2: Update the older GPU graphics design status**

In `docs/superpowers/specs/2026-06-18-rust-native-3d-shader-graphics-design.md`, update the `Implementation Status` section so it includes:

```markdown
- The productionized HQ profile is Rust-owned through `hq-neon-wireframe-shader-v1`.
- GPU-unavailable environments can render the HQ profile with a deterministic software fallback.
- Visual QA samples first, middle, and last frames for built-in GPU profiles.
```

- [ ] **Step 3: Run formatting**

Run:

```bash
rtk cargo fmt --manifest-path src-tauri/Cargo.toml
```

Expected: exits 0.

- [ ] **Step 4: Run focused GPU and render tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gpu_visual
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline final_webm_quality
```

Expected: PASS.

- [ ] **Step 5: Run full verification**

Run:

```bash
rtk pnpm verify
```

Expected: TypeScript lint passes, Vitest passes, Rust tests pass.

- [ ] **Step 6: Commit docs and final cleanup**

```bash
rtk git add docs/superpowers/specs/2026-06-19-hq-gpu-visuals-productionization-design.md docs/superpowers/specs/2026-06-18-rust-native-3d-shader-graphics-design.md
rtk git commit -m "docs: record hq gpu visuals productionization status"
```

## Self-Review

Spec coverage:

- Profile registry and proposal validation: Task 1.
- CPU fallback: Task 2 and Task 3.
- Renderer mode reporting: Task 3.
- Visual QA: Task 4.
- GPU primitive rendering: Task 5.
- Final-quality WebM export: Task 6.
- Documentation and full verification: Task 7.

Placeholder scan:

- No unresolved placeholder words are present.
- Every task has explicit files, tests, commands, and expected outcomes.

Type consistency:

- `qualityProfile` remains the JSON field.
- `GpuVisualProfileId::HqNeonWireframeShaderV1` maps to `hq-neon-wireframe-shader-v1`.
- `RenderQualityProfile::DraftWebm` and `RenderQualityProfile::FinalWebm` map to `draftWebm` and `finalWebm` command metadata.
