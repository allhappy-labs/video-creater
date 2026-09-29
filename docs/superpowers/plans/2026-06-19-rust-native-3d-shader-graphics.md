# Rust Native 3D Shader Graphics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Rust-native GPU graphics artifact producer that validates raw GLSL shader visuals plus simple 3D primitives, renders PNG frame sequences, and feeds them into the existing GStreamer/GES render pipeline.

**Architecture:** Add `src-tauri/src/gpu_graphics/` beside the existing CPU `graphics` module. GPU visuals use their own typed IR, validation, shader wrapper, mesh generation, and `wgpu` renderer, but return the existing `GraphicsArtifactManifest` so render proposal and GES composition keep the same artifact boundary. Proposal integration adds a separate `gpuVisuals` array after EDL validation; GES support expands from still PNG overlays to bounded PNG frame sequences by adding one short image clip per rendered frame.

**Tech Stack:** Rust 1.87+ because `wgpu 29.0.3` requires MSRV 1.87, `wgpu` with `glsl` and `wgsl` shader features, `bytemuck` for GPU buffer casts, `glam` for 3D math, `pollster` for blocking async GPU setup, existing `image`/`png` output, existing GStreamer/GES backend, Cargo tests.

---

## Scope

This plan implements the approved design in `docs/superpowers/specs/2026-06-18-rust-native-3d-shader-graphics-design.md`.

In scope:

- GPU graphics IR, error types, validation, shader contract, and mesh generation.
- Raw GLSL fragment function contract: `video_creater_fragment(vec2 uv, float time, float progress)`.
- Offscreen `wgpu` rendering to RGBA PNG frame sequences and `GraphicsArtifactManifest`.
- Cube and grid primitives for the first combined slice.
- GStreamer/GES bounded PNG frame sequence composition.
- Codex proposal schema support for `gpuVisuals`.
- Render proposal conversion, timing validation, artifact rendering, reports, and one tiny e2e fixture.

Out of scope:

- User-provided vertex shaders.
- Compute shaders, storage buffers, multi-pass shader graphs, remote textures, GLTF import, and arbitrary file includes.
- Replacing CPU graphics for captions, lower thirds, overlays, image refs, and text-heavy layers.
- Cross-platform GPU CI beyond Mac-first implementation and clear GPU-unavailable behavior.

## External References Checked

- `wgpu 29.0.3` docs say it is a cross-platform safe Rust graphics API, supports Metal on macOS, supports WGSL/SPIR-V/GLSL shader input with GLSL behind the `glsl` feature, and has MSRV 1.87: https://docs.rs/wgpu/latest/wgpu/
- `ShaderSource::Glsl` in `wgpu 29.0.3` uses `{ shader, stage, defines }` and is stage-specific: https://docs.rs/wgpu/latest/wgpu/enum.ShaderSource.html
- `bytemuck 1.25.0` provides safe casting utilities and derive support for plain data types: https://docs.rs/bytemuck/latest/bytemuck/
- `glam 0.33.1` provides graphics-oriented vectors, matrices, quaternions, and column-major transform conventions: https://docs.rs/glam/latest/glam/
- `gstreamer-editing-services 0.25.0` exposes GES Rust bindings, `UriClipAsset`, and media/timeline APIs already used in this repo: https://docs.rs/gstreamer-editing-services/latest/gstreamer_editing_services/

## File Structure

- Modify `src-tauri/Cargo.toml`
  - Raise `rust-version` to `1.87`.
  - Add `wgpu = { version = "29.0.3", features = ["glsl", "wgsl"] }`.
  - Add `bytemuck = { version = "1.25.0", features = ["derive"] }`.
  - Add `glam = "0.33.1"`.
  - Add `pollster = "0.4.0"`.
- Modify `src-tauri/src/lib.rs`
  - Export `pub mod gpu_graphics;`.
- Create `src-tauri/src/gpu_graphics/mod.rs`
  - Public module boundary and re-exports.
- Create `src-tauri/src/gpu_graphics/error.rs`
  - GPU-specific actionable errors and conversion to `PipelineError`.
- Create `src-tauri/src/gpu_graphics/ir.rs`
  - Typed GPU graphics layer, shader pass, scene, primitive, material, camera, transform, and uniform structs.
- Create `src-tauri/src/gpu_graphics/validation.rs`
  - Budget, timing, metadata, shader, primitive, material, camera, and role validation.
- Create `src-tauri/src/gpu_graphics/shader.rs`
  - GLSL source validation and wrapper generation.
- Create `src-tauri/src/gpu_graphics/mesh.rs`
  - Cube and grid mesh generation with finite vertices and indices.
- Create `src-tauri/src/gpu_graphics/renderer.rs`
  - `wgpu` offscreen renderer that writes PNG frames, preview, and manifest.
- Create `src-tauri/tests/gpu_graphics.rs`
  - Focused GPU IR, validation, shader, mesh, and renderer tests.
- Modify `src-tauri/src/render_pipeline/gstreamer_backend.rs`
  - Accept bounded multi-frame PNG sequence manifests and add frames as short GES image clips.
- Modify `src-tauri/src/codex/proposal.rs`
  - Add `gpu_visuals: Vec<serde_json::Value>` to `CodexEditProposal`.
  - Validate GPU visual timing after EDL validation.
- Modify `src-tauri/src/codex/app_server.rs`
  - Add prompt guidance and JSON schema for `gpuVisuals`.
- Modify `src-tauri/src/render_pipeline/proposal.rs`
  - Convert proposal `gpuVisuals` to `GpuGraphicsLayer`.
  - Render GPU graphics artifacts beside CPU graphics artifacts.
  - Include GPU artifacts in reports and preview counts.
- Modify `src-tauri/tests/codex_app_server.rs`
  - Add schema and validation tests for `gpuVisuals`.
- Modify `src-tauri/tests/render_pipeline.rs`
  - Add proposal conversion, timing, GES sequence, report artifact, and e2e tests.

## Task 1: GPU Graphics IR, Errors, And Validation

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/gpu_graphics/mod.rs`
- Create: `src-tauri/src/gpu_graphics/error.rs`
- Create: `src-tauri/src/gpu_graphics/ir.rs`
- Create: `src-tauri/src/gpu_graphics/validation.rs`
- Test: `src-tauri/tests/gpu_graphics.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/tests/gpu_graphics.rs` with these tests and helpers:

```rust
use video_creater_lib::graphics::ir::Dimensions;
use video_creater_lib::gpu_graphics::error::GpuGraphicsErrorCode;
use video_creater_lib::gpu_graphics::ir::{GpuGraphicsLayer, GpuGraphicRole};
use video_creater_lib::gpu_graphics::validation::validate_gpu_graphics_layer;

#[test]
fn gpu_shader_background_ir_deserializes_and_validates() {
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");

    validate_gpu_graphics_layer(&layer).expect("minimal shader layer should validate");

    assert_eq!(layer.schema_version, 1);
    assert_eq!(layer.role, GpuGraphicRole::ShaderBackground);
    assert_eq!(layer.dimensions, Dimensions { width: 320, height: 180 });
    assert!(layer.background.is_some());
    assert!(layer.scene.is_none());
}

#[test]
fn gpu_hybrid_scene_ir_deserializes_and_validates() {
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_hybrid_scene_json()).expect("hybrid gpu layer json");

    validate_gpu_graphics_layer(&layer).expect("hybrid layer should validate");

    assert_eq!(layer.role, GpuGraphicRole::HybridScene);
    assert_eq!(
        layer
            .scene
            .as_ref()
            .expect("scene")
            .primitives
            .first()
            .expect("primitive")
            .primitive_type
            .as_str(),
        "cube"
    );
}

#[test]
fn gpu_validation_rejects_missing_visual_metadata_actionably() {
    let mut layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");
    layer.motion.clear();

    let errors = validate_gpu_graphics_layer(&layer).expect_err("motion is required");

    assert_eq!(errors[0].code, GpuGraphicsErrorCode::GpuGraphicsTextEmpty);
    assert_eq!(errors[0].path, "motion");
    assert!(errors[0].fix.contains("motion"));
}

#[test]
fn gpu_validation_rejects_oversized_frame_budget() {
    let mut layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");
    layer.duration_seconds = 20.5;
    layer.fps = 60.0;

    let errors = validate_gpu_graphics_layer(&layer).expect_err("frame budget should fail");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsFrameBudgetExceeded
    );
    assert_eq!(errors[0].path, "frames");
    assert_eq!(
        errors[0].details.get("maxFrameCount"),
        Some(&"600".to_string())
    );
}

fn minimal_shader_background_json() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "id": "gpu-shader-bg",
        "role": "shader_background",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 320, "height": 180 },
        "fps": 4.0,
        "alpha": true,
        "sourceBeat": "Open with a procedural generated background.",
        "visualTreatment": "soft procedural gradient field",
        "motion": "slow shader drift",
        "safeZone": "keep center low contrast",
        "avoid": "strobing and tiny high-frequency noise",
        "background": {
            "shaderLanguage": "glsl",
            "fragmentSource": "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, progress, 1.0); }",
            "uniforms": { "speed": 0.75, "enabled": true, "paletteA": "#101820" }
        }
    })
}

fn minimal_hybrid_scene_json() -> serde_json::Value {
    let mut value = minimal_shader_background_json();
    value["id"] = serde_json::json!("gpu-hybrid-scene");
    value["role"] = serde_json::json!("hybrid_scene");
    value["scene"] = serde_json::json!({
        "camera": {
            "preset": "orbit",
            "distance": 4.0,
            "fovDegrees": 45.0
        },
        "primitives": [
            {
                "id": "main-cube",
                "type": "cube",
                "transform": {
                    "position": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0],
                    "scale": [1.0, 1.0, 1.0]
                },
                "material": {
                    "kind": "flat",
                    "color": "#63e6be",
                    "opacity": 1.0
                },
                "animate": {
                    "rotation": { "axis": [0.0, 1.0, 0.0], "turns": 1.0 }
                }
            }
        ]
    });
    value
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics gpu_
```

Expected: FAIL because `gpu_graphics` does not exist.

- [ ] **Step 3: Add dependencies, module export, and IR**

Modify `src-tauri/Cargo.toml`:

```toml
rust-version = "1.87"
```

Add dependencies in `[dependencies]`:

```toml
bytemuck = { version = "1.25.0", features = ["derive"] }
glam = "0.33.1"
pollster = "0.4.0"
wgpu = { version = "29.0.3", features = ["glsl", "wgsl"] }
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod codex;
pub mod edit;
pub mod gpu_graphics;
pub mod graphics;
pub mod project;
pub mod render_pipeline;
pub mod transcription;
```

Create `src-tauri/src/gpu_graphics/mod.rs`:

```rust
pub mod error;
pub mod ir;
pub mod mesh;
pub mod renderer;
pub mod shader;
pub mod validation;
```

Create `src-tauri/src/gpu_graphics/error.rs`:

```rust
use crate::render_pipeline::error::{PipelineError, PipelineErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GpuGraphicsErrorCode {
    GpuGraphicsSchemaUnsupported,
    GpuGraphicsInvalidTiming,
    GpuGraphicsInvalidDimensions,
    GpuGraphicsFrameBudgetExceeded,
    GpuGraphicsTextEmpty,
    GpuGraphicsShaderSourceTooLarge,
    GpuGraphicsShaderEntryMissing,
    GpuGraphicsShaderUnsupportedFeature,
    GpuGraphicsShaderCompileFailed,
    GpuGraphicsUniformInvalid,
    GpuGraphicsPrimitiveUnsupported,
    GpuGraphicsPrimitiveInvalid,
    GpuGraphicsCameraInvalid,
    GpuGraphicsDeviceUnavailable,
    GpuGraphicsRenderFailed,
    GpuGraphicsReadbackFailed,
    GpuGraphicsArtifactWriteFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuGraphicsError {
    pub code: GpuGraphicsErrorCode,
    pub path: String,
    pub message: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl GpuGraphicsError {
    pub fn new(
        code: GpuGraphicsErrorCode,
        path: impl Into<String>,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
            fix: fix.into(),
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

pub type GpuGraphicsResult<T> = Result<T, Vec<GpuGraphicsError>>;

pub fn gpu_errors_to_pipeline_errors(errors: Vec<GpuGraphicsError>) -> Vec<PipelineError> {
    errors
        .into_iter()
        .map(|error| {
            let mut details = error.details;
            details.insert("gpuGraphicsCode".to_string(), gpu_error_code_label(&error.code));
            PipelineError {
                code: PipelineErrorCode::PipelineInputInvalid,
                path: error.path,
                message: error.message,
                fix: error.fix,
                details,
            }
        })
        .collect()
}

pub fn gpu_error_code_label(code: &GpuGraphicsErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "GPU_GRAPHICS_RENDER_FAILED".to_string())
}
```

Create `src-tauri/src/gpu_graphics/ir.rs` with the approved IR:

```rust
use crate::graphics::ir::Dimensions;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuGraphicsLayer {
    pub schema_version: u32,
    pub id: String,
    pub role: GpuGraphicRole,
    pub timeline_start: f64,
    pub duration_seconds: f64,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub alpha: bool,
    pub source_beat: String,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ShaderPass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<PrimitiveScene>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GpuGraphicRole {
    ShaderBackground,
    PrimitiveScene,
    HybridScene,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShaderPass {
    pub shader_language: ShaderLanguage,
    pub fragment_source: String,
    #[serde(default)]
    pub uniforms: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShaderLanguage {
    Glsl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveScene {
    pub camera: Camera,
    #[serde(default)]
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    pub preset: CameraPreset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fov_degrees: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CameraPreset {
    Fixed,
    Orbit,
    Dolly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Primitive {
    pub id: String,
    #[serde(rename = "type")]
    pub primitive_type: String,
    pub transform: Transform3,
    pub material: Material,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animate: Option<PrimitiveAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transform3 {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub kind: MaterialKind,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    Flat,
    Emissive,
    Wire,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveAnimation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<RotationAnimation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RotationAnimation {
    pub axis: [f32; 3],
    pub turns: f32,
}
```

- [ ] **Step 4: Add validation implementation**

Create `src-tauri/src/gpu_graphics/validation.rs`:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::{GpuGraphicRole, GpuGraphicsLayer, PrimitiveScene, ShaderPass};

const SUPPORTED_SCHEMA_VERSION: u32 = 1;
pub const MAX_SHADER_SOURCE_BYTES: usize = 20 * 1024;
pub const MAX_WIDTH: u32 = 1920;
pub const MAX_HEIGHT: u32 = 1080;
pub const MAX_FPS: f64 = 60.0;
pub const MAX_DURATION_SECONDS: f64 = 20.0;
pub const MAX_FRAME_COUNT: u32 = 600;
pub const MAX_PRIMITIVE_COUNT: usize = 256;
pub const MAX_PARTICLE_COUNT: u32 = 10_000;

pub fn validate_gpu_graphics_layer(layer: &GpuGraphicsLayer) -> GpuGraphicsResult<()> {
    let mut errors = Vec::new();

    if layer.schema_version != SUPPORTED_SCHEMA_VERSION {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsSchemaUnsupported,
            "schemaVersion",
            "GPU graphics schema version is not supported.",
            "Set schemaVersion to 1.",
        ));
    }

    if layer.id.trim().is_empty() {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsTextEmpty,
            "id",
            "GPU graphics layer id is empty.",
            "Set id to a stable non-empty string.",
        ));
    }

    validate_required_text(layer, &mut errors);
    validate_timing_and_dimensions(layer, &mut errors);
    validate_role_shape(layer, &mut errors);

    if let Some(background) = &layer.background {
        validate_shader_pass(background, "background", &mut errors);
    }
    if let Some(scene) = &layer.scene {
        validate_scene(scene, "scene", &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_required_text(layer: &GpuGraphicsLayer, errors: &mut Vec<GpuGraphicsError>) {
    for (field, value) in [
        ("sourceBeat", &layer.source_beat),
        ("visualTreatment", &layer.visual_treatment),
        ("motion", &layer.motion),
        ("safeZone", &layer.safe_zone),
        ("avoid", &layer.avoid),
    ] {
        if value.trim().is_empty() {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsTextEmpty,
                field,
                "Required GPU visual metadata is empty.",
                format!("Set {field} to a concise non-empty visual direction."),
            ));
        }
    }
}

fn validate_timing_and_dimensions(layer: &GpuGraphicsLayer, errors: &mut Vec<GpuGraphicsError>) {
    if layer.dimensions.width == 0
        || layer.dimensions.height == 0
        || layer.dimensions.width > MAX_WIDTH
        || layer.dimensions.height > MAX_HEIGHT
    {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidDimensions,
            "dimensions",
            "GPU layer dimensions are outside the v1 budget.",
            "Use dimensions no larger than 1920x1080 for GPU-generated layers.",
        ));
    }

    if !layer.timeline_start.is_finite() || layer.timeline_start < 0.0 {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidTiming,
            "timelineStart",
            "GPU layer start time is invalid.",
            "Use a finite timelineStart greater than or equal to 0.",
        ));
    }

    if !layer.duration_seconds.is_finite()
        || layer.duration_seconds <= 0.0
        || layer.duration_seconds > MAX_DURATION_SECONDS
    {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidTiming,
            "durationSeconds",
            "GPU layer duration is outside the v1 budget.",
            "Use a finite durationSeconds greater than 0 and no more than 20.",
        ));
    }

    if !layer.fps.is_finite() || layer.fps <= 0.0 || layer.fps > MAX_FPS {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidTiming,
            "fps",
            "GPU layer FPS is outside the v1 budget.",
            "Use a finite fps greater than 0 and no more than 60.",
        ));
    }

    if layer.duration_seconds.is_finite() && layer.fps.is_finite() {
        let frame_count = (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32;
        if frame_count > MAX_FRAME_COUNT {
            errors.push(
                GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsFrameBudgetExceeded,
                    "frames",
                    "GPU layer frame budget is too large.",
                    "Reduce durationSeconds or fps so the layer renders no more than 600 frames.",
                )
                .with_detail("frameCount", frame_count.to_string())
                .with_detail("maxFrameCount", MAX_FRAME_COUNT.to_string()),
            );
        }
    }
}

fn validate_role_shape(layer: &GpuGraphicsLayer, errors: &mut Vec<GpuGraphicsError>) {
    match layer.role {
        GpuGraphicRole::ShaderBackground => {
            if layer.background.is_none() || layer.scene.is_some() {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    "role",
                    "shader_background layers require background and no scene.",
                    "Use role hybrid_scene when combining a shader with primitives.",
                ));
            }
        }
        GpuGraphicRole::PrimitiveScene => {
            if layer.scene.is_none() {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    "scene",
                    "primitive_scene layers require a scene.",
                    "Add scene.camera and at least one primitive.",
                ));
            }
        }
        GpuGraphicRole::HybridScene => {
            if layer.background.is_none() || layer.scene.is_none() {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    "role",
                    "hybrid_scene layers require both background and scene.",
                    "Add a background shader and scene primitives.",
                ));
            }
        }
    }
}

fn validate_shader_pass(pass: &ShaderPass, path: &str, errors: &mut Vec<GpuGraphicsError>) {
    if pass.fragment_source.len() > MAX_SHADER_SOURCE_BYTES {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderSourceTooLarge,
            format!("{path}.fragmentSource"),
            "GLSL fragment source exceeds the v1 size budget.",
            "Keep fragmentSource at or below 20 KiB.",
        ));
    }
    if !pass.fragment_source.contains("video_creater_fragment") {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderEntryMissing,
            format!("{path}.fragmentSource"),
            "GLSL fragment source is missing video_creater_fragment.",
            "Return a vec4 video_creater_fragment(vec2 uv, float time, float progress) function.",
        ));
    }
}

fn validate_scene(scene: &PrimitiveScene, path: &str, errors: &mut Vec<GpuGraphicsError>) {
    if scene.primitives.is_empty() {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            format!("{path}.primitives"),
            "Primitive scene has no primitives.",
            "Add at least one cube or grid primitive.",
        ));
    }
    if scene.primitives.len() > MAX_PRIMITIVE_COUNT {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            format!("{path}.primitives"),
            "Primitive count exceeds the v1 budget.",
            "Use no more than 256 primitives in one GPU layer.",
        ));
    }

    for (index, primitive) in scene.primitives.iter().enumerate() {
        let primitive_path = format!("{path}.primitives[{index}]");
        if primitive.id.trim().is_empty() {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                format!("{primitive_path}.id"),
                "Primitive id is empty.",
                "Set id to a stable non-empty string.",
            ));
        }
        if primitive.primitive_type != "cube" && primitive.primitive_type != "grid" {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveUnsupported,
                format!("{primitive_path}.type"),
                "Primitive type is not supported in the first GPU slice.",
                "Use cube or grid.",
            ));
        }
        for (field, values) in [
            ("position", primitive.transform.position),
            ("rotation", primitive.transform.rotation),
            ("scale", primitive.transform.scale),
        ] {
            if values.iter().any(|value| !value.is_finite()) {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    format!("{primitive_path}.transform.{field}"),
                    "Primitive transform contains a non-finite value.",
                    "Use finite transform values.",
                ));
            }
        }
        if primitive.transform.scale.iter().any(|value| *value <= 0.0) {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                format!("{primitive_path}.transform.scale"),
                "Primitive scale must be positive.",
                "Use scale values greater than 0.",
            ));
        }
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics gpu_
```

Expected: PASS for the new IR and validation tests.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/gpu_graphics src-tauri/tests/gpu_graphics.rs
rtk git commit -m "feat: add gpu graphics ir"
```

## Task 2: GLSL Contract Validation And Wrapper

**Files:**
- Modify: `src-tauri/src/gpu_graphics/shader.rs`
- Modify: `src-tauri/src/gpu_graphics/validation.rs`
- Test: `src-tauri/tests/gpu_graphics.rs`

- [ ] **Step 1: Write failing shader tests**

Append these tests to `src-tauri/tests/gpu_graphics.rs`:

```rust
use video_creater_lib::gpu_graphics::shader::{
    validate_fragment_source, wrap_glsl_fragment_source,
};

#[test]
fn shader_wrapper_injects_fixed_uniforms_and_entrypoint() {
    let source = "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, time, 1.0); }";

    let wrapped = wrap_glsl_fragment_source(source).expect("valid source should wrap");

    assert!(wrapped.contains("#version 450"));
    assert!(wrapped.contains("layout(location = 0) in vec2 v_uv;"));
    assert!(wrapped.contains("layout(location = 0) out vec4 o_color;"));
    assert!(wrapped.contains("uniform VideoCreaterUniforms"));
    assert!(wrapped.contains("o_color = video_creater_fragment(v_uv, u_time, u_progress);"));
}

#[test]
fn shader_validation_rejects_missing_contract_function() {
    let errors = validate_fragment_source("void main() {}").expect_err("missing contract");

    assert_eq!(errors[0].code, GpuGraphicsErrorCode::GpuGraphicsShaderEntryMissing);
    assert_eq!(errors[0].path, "fragmentSource");
}

#[test]
fn shader_validation_rejects_disallowed_preprocessor_and_external_resources() {
    for source in [
        "#include \"private.glsl\"\nvec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, time, 1.0); }",
        "#pragma optimize(on)\nvec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, time, 1.0); }",
        "uniform sampler2D remoteTexture;\nvec4 video_creater_fragment(vec2 uv, float time, float progress) { return texture(remoteTexture, uv); }",
        "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(0.0); }\n// https://example.com/texture.png",
    ] {
        let errors = validate_fragment_source(source).expect_err("source should be rejected");

        assert_eq!(
            errors[0].code,
            GpuGraphicsErrorCode::GpuGraphicsShaderUnsupportedFeature
        );
        assert!(errors[0].fix.contains("bounded GLSL fragment function"));
    }
}
```

- [ ] **Step 2: Run shader tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics shader_
```

Expected: FAIL because `validate_fragment_source` and `wrap_glsl_fragment_source` are not implemented.

- [ ] **Step 3: Implement shader validation and wrapping**

Create `src-tauri/src/gpu_graphics/shader.rs`:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::validation::MAX_SHADER_SOURCE_BYTES;

pub fn validate_fragment_source(source: &str) -> GpuGraphicsResult<()> {
    let mut errors = Vec::new();

    if source.len() > MAX_SHADER_SOURCE_BYTES {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderSourceTooLarge,
            "fragmentSource",
            "GLSL fragment source exceeds the v1 size budget.",
            "Keep fragmentSource at or below 20 KiB.",
        ));
    }

    if !source.contains("video_creater_fragment") {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderEntryMissing,
            "fragmentSource",
            "GLSL fragment source is missing video_creater_fragment.",
            "Return a vec4 video_creater_fragment(vec2 uv, float time, float progress) function.",
        ));
    }

    for blocked in [
        "#include",
        "#pragma",
        "sampler",
        "image2D",
        "buffer ",
        "layout(binding",
        "http://",
        "https://",
        "file://",
    ] {
        if source.contains(blocked) {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsShaderUnsupportedFeature,
                "fragmentSource",
                "GLSL fragment source uses an unsupported feature.",
                "Use one bounded GLSL fragment function without includes, external textures, storage buffers, URLs, or custom bindings.",
            )
            .with_detail("blockedPattern", blocked));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn wrap_glsl_fragment_source(source: &str) -> GpuGraphicsResult<String> {
    validate_fragment_source(source)?;
    Ok(format!(
        r#"#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform VideoCreaterUniforms {{
    float u_time;
    float u_duration;
    vec2 u_resolution;
    float u_frame;
    float u_progress;
}};

{source}

void main() {{
    o_color = video_creater_fragment(v_uv, u_time, u_progress);
}}
"#
    ))
}

pub fn fullscreen_vertex_wgsl() -> &'static str {
    r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0)
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 0.0)
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}
"#
}
```

Update `validate_shader_pass` in `validation.rs` so it delegates source checks:

```rust
fn validate_shader_pass(pass: &ShaderPass, path: &str, errors: &mut Vec<GpuGraphicsError>) {
    if let Err(shader_errors) = crate::gpu_graphics::shader::validate_fragment_source(
        &pass.fragment_source,
    ) {
        errors.extend(shader_errors.into_iter().map(|mut error| {
            error.path = format!("{path}.{}", error.path);
            error
        }));
    }
}
```

- [ ] **Step 4: Run shader tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics shader_
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/gpu_graphics/shader.rs src-tauri/src/gpu_graphics/validation.rs src-tauri/tests/gpu_graphics.rs
rtk git commit -m "feat: add gpu shader contract"
```

## Task 3: Cube And Grid Mesh Generation

**Files:**
- Modify: `src-tauri/src/gpu_graphics/mesh.rs`
- Test: `src-tauri/tests/gpu_graphics.rs`

- [ ] **Step 1: Write failing mesh tests**

Append these tests:

```rust
use video_creater_lib::gpu_graphics::mesh::{build_cube_mesh, build_grid_mesh};

#[test]
fn cube_mesh_has_finite_vertices_and_indices() {
    let mesh = build_cube_mesh(1.0).expect("cube mesh");

    assert_eq!(mesh.vertices.len(), 24);
    assert_eq!(mesh.indices.len(), 36);
    assert!(mesh
        .vertices
        .iter()
        .flat_map(|vertex| vertex.position)
        .all(f32::is_finite));
    assert!(mesh.indices.iter().all(|index| (*index as usize) < mesh.vertices.len()));
}

#[test]
fn grid_mesh_has_expected_line_vertices() {
    let mesh = build_grid_mesh(4, 2.0).expect("grid mesh");

    assert_eq!(mesh.indices.len() % 2, 0);
    assert!(mesh.vertices.len() >= 20);
    assert!(mesh
        .vertices
        .iter()
        .flat_map(|vertex| vertex.position)
        .all(f32::is_finite));
}

#[test]
fn mesh_generation_rejects_invalid_inputs_actionably() {
    let cube_errors = build_cube_mesh(0.0).expect_err("zero cube size should fail");
    assert_eq!(
        cube_errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );

    let grid_errors = build_grid_mesh(0, 2.0).expect_err("zero grid divisions should fail");
    assert_eq!(
        grid_errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );
}
```

- [ ] **Step 2: Run mesh tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics mesh_
```

Expected: FAIL because mesh generation is not implemented.

- [ ] **Step 3: Implement mesh generation**

Create `src-tauri/src/gpu_graphics/mesh.rs`:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub topology: MeshTopology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshTopology {
    Triangles,
    Lines,
}

pub fn build_cube_mesh(size: f32) -> GpuGraphicsResult<Mesh> {
    if !size.is_finite() || size <= 0.0 {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            "primitive.size",
            "Cube size is invalid.",
            "Use a finite cube size greater than 0.",
        )]);
    }
    let h = size / 2.0;
    let faces = [
        ([0.0, 0.0, 1.0], [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]]),
        ([0.0, 0.0, -1.0], [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]]),
        ([1.0, 0.0, 0.0], [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]]),
        ([-1.0, 0.0, 0.0], [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]]),
        ([0.0, 1.0, 0.0], [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]]),
        ([0.0, -1.0, 0.0], [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]]),
    ];

    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (face_index, (normal, positions)) in faces.iter().enumerate() {
        let base = (face_index * 4) as u32;
        for (uv, position) in [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
            .into_iter()
            .zip(*positions)
        {
            vertices.push(Vertex {
                position,
                normal: *normal,
                uv,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Ok(Mesh {
        vertices,
        indices,
        topology: MeshTopology::Triangles,
    })
}

pub fn build_grid_mesh(divisions: u32, size: f32) -> GpuGraphicsResult<Mesh> {
    if divisions == 0 || !size.is_finite() || size <= 0.0 {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            "primitive.grid",
            "Grid divisions and size are invalid.",
            "Use divisions greater than 0 and finite size greater than 0.",
        )]);
    }

    let half = size / 2.0;
    let step = size / divisions as f32;
    let mut vertices = Vec::with_capacity(((divisions + 1) * 4) as usize);
    let mut indices = Vec::with_capacity(((divisions + 1) * 4) as usize);

    for index in 0..=divisions {
        let value = -half + step * index as f32;
        let base = vertices.len() as u32;
        vertices.push(Vertex {
            position: [-half, 0.0, value],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        });
        vertices.push(Vertex {
            position: [half, 0.0, value],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
        });
        vertices.push(Vertex {
            position: [value, 0.0, -half],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
        });
        vertices.push(Vertex {
            position: [value, 0.0, half],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
        });
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 3]);
    }

    Ok(Mesh {
        vertices,
        indices,
        topology: MeshTopology::Lines,
    })
}
```

- [ ] **Step 4: Run mesh tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics mesh_
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/gpu_graphics/mesh.rs src-tauri/tests/gpu_graphics.rs
rtk git commit -m "feat: add gpu primitive meshes"
```

## Task 4: Offscreen GPU Renderer To Manifest

**Files:**
- Modify: `src-tauri/src/gpu_graphics/renderer.rs`
- Modify: `src-tauri/src/gpu_graphics/error.rs`
- Test: `src-tauri/tests/gpu_graphics.rs`

- [ ] **Step 1: Write failing renderer tests**

Append these tests:

```rust
use std::path::PathBuf;
use video_creater_lib::gpu_graphics::renderer::{
    render_gpu_graphics_layer, GpuRenderOptions,
};

#[test]
fn gpu_renderer_writes_shader_frame_sequence_or_reports_unavailable() {
    let dir = tempfile::tempdir().expect("temp gpu render dir");
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");

    let result = render_gpu_graphics_layer(
        &layer,
        GpuRenderOptions {
            output_dir: dir.path().join("gpu-shader"),
        },
    );

    match result {
        Ok(manifest) => {
            assert_eq!(manifest.kind, "rgbaFrameSequence");
            assert_eq!(manifest.frame_count, 4);
            assert_eq!(manifest.duration_seconds, 1.0);
            assert_eq!(manifest.fps, 4.0);
            assert_eq!(manifest.frames_pattern, "frames/frame-%06d.png");
            assert!(dir.path().join("gpu-shader/preview.png").exists());
            assert!(dir.path().join("gpu-shader/manifest.json").exists());
            assert!(dir.path().join("gpu-shader/frames/frame-000000.png").exists());
            assert!(dir.path().join("gpu-shader/frames/frame-000003.png").exists());
        }
        Err(errors)
            if errors
                .iter()
                .any(|error| error.code == GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable) =>
        {
            eprintln!("Skipping GPU render assertion because no compatible adapter is available");
        }
        Err(errors) => panic!("unexpected GPU renderer errors: {errors:?}"),
    }
}

#[test]
fn gpu_renderer_writes_hybrid_scene_manifest_or_reports_unavailable() {
    let dir = tempfile::tempdir().expect("temp gpu render dir");
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_hybrid_scene_json()).expect("hybrid layer json");

    let result = render_gpu_graphics_layer(
        &layer,
        GpuRenderOptions {
            output_dir: dir.path().join("gpu-hybrid"),
        },
    );

    match result {
        Ok(manifest) => {
            assert_eq!(manifest.source_layer_ids, vec!["gpu-hybrid-scene"]);
            assert_eq!(manifest.frame_count, 4);
            assert!(dir.path().join("gpu-hybrid/frames/frame-000002.png").exists());
        }
        Err(errors)
            if errors
                .iter()
                .any(|error| error.code == GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable) =>
        {
            eprintln!("Skipping hybrid GPU render assertion because no compatible adapter is available");
        }
        Err(errors) => panic!("unexpected GPU renderer errors: {errors:?}"),
    }
}
```

- [ ] **Step 2: Run renderer tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics gpu_renderer_
```

Expected: FAIL because renderer API is not implemented.

- [ ] **Step 3: Implement renderer public API and artifact writer**

Create `src-tauri/src/gpu_graphics/renderer.rs` with this public surface and artifact layout:

```rust
use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::GpuGraphicsLayer;
use super::validation::validate_gpu_graphics_layer;
use crate::graphics::manifest::GraphicsArtifactManifest;
use image::{ImageBuffer, Rgba};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const PREVIEW_FILE_NAME: &str = "preview.png";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const FRAMES_DIR_NAME: &str = "frames";
const FRAMES_PATTERN: &str = "frames/frame-%06d.png";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuRenderOptions {
    pub output_dir: PathBuf,
}

pub fn render_gpu_graphics_layer(
    layer: &GpuGraphicsLayer,
    options: GpuRenderOptions,
) -> GpuGraphicsResult<GraphicsArtifactManifest> {
    validate_gpu_graphics_layer(layer)?;
    std::fs::create_dir_all(&options.output_dir).map_err(|error| {
        vec![artifact_error(
            "outputDir",
            "Could not create GPU graphics output directory.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    let frames_dir = options.output_dir.join(FRAMES_DIR_NAME);
    if frames_dir.exists() {
        std::fs::remove_dir_all(&frames_dir).map_err(|error| {
            vec![artifact_error(
                "frames",
                "Could not clear stale GPU graphics frames.",
                "Remove stale graphics artifacts or choose a fresh outputDir.",
                error,
            )]
        })?;
    }
    std::fs::create_dir_all(&frames_dir).map_err(|error| {
        vec![artifact_error(
            "frames",
            "Could not create GPU graphics frames directory.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    let frame_count = (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32;
    let mut renderer = BlockingGpuRenderer::new(layer)?;
    for frame_index in 0..frame_count {
        let rgba = renderer.render_frame(layer, frame_index, frame_count)?;
        let frame_path = frames_dir.join(format!("frame-{frame_index:06}.png"));
        write_rgba_png(layer.dimensions.width, layer.dimensions.height, rgba, &frame_path)?;
        if frame_index == 0 {
            let preview_path = options.output_dir.join(PREVIEW_FILE_NAME);
            let preview_bytes = std::fs::read(&frame_path).map_err(|error| {
                artifact_error(
                    "preview",
                    "Could not read first GPU frame for preview.",
                    "Regenerate the GPU graphics artifact.",
                    error,
                )
            })?;
            std::fs::write(preview_path, preview_bytes).map_err(|error| {
                artifact_error(
                    "preview",
                    "Could not write GPU preview frame.",
                    "Choose a writable outputDir for GPU graphics artifacts.",
                    error,
                )
            })?;
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
        preview_path: PathBuf::from(PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|error| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed,
            "manifest",
            "Could not serialize GPU graphics manifest.",
            "Inspect manifest fields for unsupported values.",
        )
        .with_detail("serdeError", error.to_string())]
    })?;
    std::fs::write(options.output_dir.join(MANIFEST_FILE_NAME), manifest_json).map_err(|error| {
        vec![artifact_error(
            "manifest",
            "Could not write GPU graphics manifest.",
            "Choose a writable outputDir for GPU graphics artifacts.",
            error,
        )]
    })?;

    Ok(manifest)
}

fn write_rgba_png(width: u32, height: u32, rgba: Vec<u8>, path: &Path) -> GpuGraphicsResult<()> {
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba).ok_or_else(|| {
        vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsReadbackFailed,
            "frame",
            "GPU readback buffer size did not match output dimensions.",
            "Ensure the renderer returns width * height * 4 bytes.",
        )]
    })?;
    image.save(path).map_err(|error| {
        vec![artifact_error(
            "frame",
            "Could not write GPU frame PNG.",
            "Choose a writable outputDir and retry GPU rendering.",
            error,
        )]
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
```

- [ ] **Step 4: Implement the `wgpu` renderer internals**

In the same file, add `BlockingGpuRenderer` and render internals. The implementation should:

- create `wgpu::Instance::new(&wgpu::InstanceDescriptor::default())`;
- request a high-performance adapter with `compatible_surface: None`;
- request a device with default features and limits;
- use `wgpu::TextureFormat::Rgba8UnormSrgb`;
- create an offscreen texture with `TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC`;
- compile the fullscreen vertex shader from `shader::fullscreen_vertex_wgsl()`;
- compile raw GLSL via:

```rust
wgpu::ShaderSource::Glsl {
    shader: std::borrow::Cow::Owned(wrapped_glsl),
    stage: wgpu::naga::ShaderStage::Fragment,
    defines: &[],
}
```

- push a `GpuGraphicsDeviceUnavailable` error when adapter or device creation fails;
- push a `GpuGraphicsShaderCompileFailed` error when shader module or pipeline creation fails;
- copy the rendered texture into a `MAP_READ` buffer with padded `bytes_per_row`;
- strip row padding before returning `Vec<u8>`.

Use this concrete uniform layout:

```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniforms {
    time: f32,
    duration: f32,
    resolution: [f32; 2],
    frame: f32,
    progress: f32,
    padding: [f32; 2],
}
```

The primitive scene may render after the shader background with a simple color pipeline. For this task, it is acceptable for the cube/grid material pass to use the generated mesh buffers and a flat color; lighting and depth polish are not required for acceptance. The renderer must still write non-empty frames for hybrid scenes.

- [ ] **Step 5: Run renderer tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics gpu_renderer_
```

Expected: PASS, either by rendering manifests or by skipping assertions through a `GpuGraphicsDeviceUnavailable` error when no compatible GPU adapter is available.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/gpu_graphics/renderer.rs src-tauri/src/gpu_graphics/error.rs src-tauri/tests/gpu_graphics.rs
rtk git commit -m "feat: render gpu graphics artifacts"
```

## Task 5: GStreamer/GES PNG Frame Sequence Composition

**Files:**
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing GES sequence tests**

Add this import near existing imports:

```rust
use video_creater_lib::graphics::manifest::GraphicsArtifactManifest;
```

Add this test near the other GStreamer backend command tests:

```rust
#[test]
fn gstreamer_ges_backend_accepts_bounded_graphics_frame_sequences() {
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "gpu-sequence.preview".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: Dimensions {
            width: 1280,
            height: 720,
        },
        fps: 30.0,
        duration_seconds: 0.2,
        alpha: true,
        frame_count: 6,
        frames_pattern: "frames/frame-%06d.png".to_string(),
        preview_path: PathBuf::from("preview.png"),
        source_layer_ids: vec!["gpu-sequence".to_string()],
        checksums: BTreeMap::new(),
    };
    let backend = GstreamerGesRenderBackend::new();

    let command = backend
        .build_command(
            &render_plan(),
            &[(manifest, PathBuf::from("/tmp/video-project/gpu-sequence"), 0.5)],
        )
        .expect("bounded frame sequence should be accepted");

    assert_eq!(command.program, "gstreamer-ges");
    assert!(command.args.iter().any(|arg| {
        arg == "--overlay=/tmp/video-project/gpu-sequence/frames/frame-000000.png@0.500+0.200"
    }));
}
```

Add this cfg-gated e2e test near `gstreamer_ges_backend_renders_selected_range_without_process_runner`:

```rust
#[cfg(feature = "ges-render")]
#[test]
fn gstreamer_ges_backend_renders_short_png_frame_sequence_overlay() {
    let dir = tempfile::tempdir().expect("temp gstreamer sequence render dir");
    let source_path = dir.path().join("source.webm");
    let output_path = dir.path().join("sequence-overlay.webm");
    generate_tiny_source_video(&source_path);

    let graphics_dir = dir.path().join("graphics/sequence");
    std::fs::create_dir_all(graphics_dir.join("frames")).expect("frames dir");
    for index in 0..4 {
        let mut image = image::RgbaImage::new(160, 90);
        for pixel in image.pixels_mut() {
            *pixel = image::Rgba([20 * index as u8, 180, 120, 180]);
        }
        image
            .save(graphics_dir.join(format!("frames/frame-{index:06}.png")))
            .expect("write overlay frame");
    }
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "sequence.preview".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: Dimensions {
            width: 160,
            height: 90,
        },
        fps: 4.0,
        duration_seconds: 1.0,
        alpha: true,
        frame_count: 4,
        frames_pattern: "frames/frame-%06d.png".to_string(),
        preview_path: PathBuf::from("preview.png"),
        source_layer_ids: vec!["sequence".to_string()],
        checksums: BTreeMap::new(),
    };

    let mut plan = render_plan();
    plan.input_path = source_path.to_string_lossy().into_owned();
    plan.output_path = output_path.to_string_lossy().into_owned();
    plan.width = 160;
    plan.height = 90;
    plan.fps = 4.0;
    plan.clips = vec![RenderClip {
        source_in: 0.25,
        source_out: 1.25,
    }];

    let output = GstreamerGesRenderBackend::new()
        .render(
            &PanicRunner,
            &plan,
            &[(manifest, graphics_dir, 0.0)],
            Duration::from_secs(30),
        )
        .expect("GES should render a short PNG graphics frame sequence");

    assert_eq!(output.status_code, Some(0));
    assert!(output_path.exists());
    assert!(
        std::fs::metadata(&output_path)
            .expect("sequence render metadata")
            .len()
            > 0
    );
}
```

- [ ] **Step 2: Run GES sequence tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_accepts_bounded_graphics_frame_sequences
```

Expected: FAIL because `validate_graphics_inputs` rejects `frame_count > 1`.

- [ ] **Step 3: Accept bounded frame sequences in validation**

In `src-tauri/src/render_pipeline/gstreamer_backend.rs`, remove the `manifest.frame_count > 1` rejection and replace it with a bounded validation:

```rust
if manifest.frame_count > 600 {
    errors.push(PipelineError::new(
        PipelineErrorCode::RenderPlanInvalidOverlay,
        format!("{path}.frameCount"),
        "Graphics overlay frame sequence exceeds the GStreamer/GES v1 budget.",
        "Render no more than 600 graphics frames for one overlay.",
    ));
}
```

- [ ] **Step 4: Add per-frame GES image clips**

In `render_with_ges`, replace the single `UriClipAsset` addition inside the graphics loop with a nested frame loop:

```rust
for frame_index in 0..manifest.frame_count {
    let frame_path = graphics_frame_path(manifest, artifact_dir, frame_index);
    let frame_uri = filename_to_uri(&frame_path, "renderPlan.graphics.frame")?;
    let asset = ges::UriClipAsset::request_sync(frame_uri.as_str()).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderBackendFailed,
            format!("gstreamer.ges.graphics[{index}].frames[{frame_index}]"),
            "GStreamer/GES could not load a graphics overlay PNG frame.",
            "Regenerate graphics artifacts and ensure every frame exists.",
        )
        .with_detail("framePath", frame_path.display().to_string())
        .with_detail("gstreamerError", error.to_string())]
    })?;
    let frame_duration_seconds = manifest.duration_seconds / manifest.frame_count as f64;
    let frame_start_seconds = *timeline_start_seconds + frame_duration_seconds * frame_index as f64;
    let start = seconds_to_clock_time(frame_start_seconds, "renderPlan.graphics.frameStart")?;
    let duration = seconds_to_clock_time(frame_duration_seconds, "renderPlan.graphics.frameDuration")?;
    asset.set_duration(duration.nseconds());
    overlay_layer
        .add_asset(
            &asset,
            Some(start),
            Some(gst::ClockTime::ZERO),
            Some(duration),
            ges::TrackType::VIDEO,
        )
        .map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                format!("gstreamer.ges.graphics[{index}].frames[{frame_index}]"),
                "GStreamer/GES could not add a graphics overlay frame to the timeline.",
                "Inspect overlay timing, dimensions, and PNG frame compatibility.",
            )
            .with_detail("framePath", frame_path.display().to_string())
            .with_detail("gstreamerError", error.to_string())]
        })?;
}
```

Add this helper beside `first_graphics_frame_path`:

```rust
fn graphics_frame_path(
    manifest: &GraphicsArtifactManifest,
    artifact_dir: &Path,
    frame_index: u32,
) -> PathBuf {
    let six = format!("{frame_index:06}");
    let five = format!("{frame_index:05}");
    let four = format!("{frame_index:04}");
    let raw = frame_index.to_string();
    let frame = manifest
        .frames_pattern
        .replace("%06d", &six)
        .replace("%05d", &five)
        .replace("%04d", &four)
        .replace("%d", &raw);
    artifact_dir.join(frame)
}
```

Keep `first_graphics_frame_path` as:

```rust
fn first_graphics_frame_path(manifest: &GraphicsArtifactManifest, artifact_dir: &Path) -> PathBuf {
    graphics_frame_path(manifest, artifact_dir, 0)
}
```

- [ ] **Step 5: Run GES sequence tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_accepts_bounded_graphics_frame_sequences
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline gstreamer_ges_backend_renders_short_png_frame_sequence_overlay
```

Expected: first command PASS. Second command PASS when `ges-render` is available; it is cfg-gated and should not run when the feature is absent.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/gstreamer_backend.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: render graphics frame sequences in ges"
```

## Task 6: Proposal Schema And GPU Visual Conversion

**Files:**
- Modify: `src-tauri/src/codex/proposal.rs`
- Modify: `src-tauri/src/codex/app_server.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Test: `src-tauri/tests/codex_app_server.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing app-server schema tests**

Add this test to `src-tauri/tests/codex_app_server.rs`:

```rust
#[test]
fn turn_request_lists_gpu_visual_shader_and_primitive_schema() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    let schema = &turn["params"]["outputSchema"];
    let gpu_visuals = &schema["properties"]["gpuVisuals"]["items"];

    assert!(text.contains("GPU visuals"));
    assert!(text.contains("video_creater_fragment"));
    assert_eq!(
        schema["required"],
        json!([
            "mediaId",
            "clips",
            "captions",
            "overlays",
            "hyperframes",
            "gpuVisuals",
            "renderReview"
        ])
    );
    assert_eq!(gpu_visuals["additionalProperties"], json!(false));
    assert_eq!(
        gpu_visuals["required"],
        json!([
            "id",
            "kind",
            "startSeconds",
            "durationSeconds",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid",
            "shader"
        ])
    );
    assert_eq!(
        gpu_visuals["properties"]["shader"]["properties"]["fragmentSource"]["type"],
        json!("string")
    );
    assert_eq!(
        gpu_visuals["properties"]["primitives"]["items"]["properties"]["type"]["enum"],
        json!(["cube", "grid"])
    );
}
```

- [ ] **Step 2: Write failing proposal conversion tests**

Add these tests to `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::render_pipeline::proposal::{
    proposal_gpu_visuals_to_layers_for_duration,
};

#[test]
fn proposal_gpu_visuals_convert_to_gpu_layers() {
    let mut proposal = sample_codex_proposal();
    proposal.gpu_visuals = vec![sample_gpu_visual()];

    let layers = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect("gpu visuals should convert");

    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].id, "shader-hook-bg");
    assert_eq!(layers[0].duration_seconds, 1.0);
    assert!(layers[0].background.is_some());
    assert!(layers[0].scene.is_some());
}

#[test]
fn proposal_gpu_visuals_reject_timing_outside_selected_duration() {
    let mut proposal = sample_codex_proposal();
    let mut gpu_visual = sample_gpu_visual();
    gpu_visual["startSeconds"] = serde_json::json!(44.5);
    gpu_visual["durationSeconds"] = serde_json::json!(2.0);
    proposal.gpu_visuals = vec![gpu_visual];

    let errors = proposal_gpu_visuals_to_layers_for_duration(&proposal, 320, 180, 4.0, 45.0)
        .expect_err("gpu visual should be inside selected duration");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "gpuVisuals[0].durationSeconds");
}

fn sample_gpu_visual() -> serde_json::Value {
    serde_json::json!({
        "id": "shader-hook-bg",
        "kind": "hybrid_scene",
        "startSeconds": 0.0,
        "durationSeconds": 1.0,
        "sourceBeat": "open with a generated shader hook",
        "visualTreatment": "procedural gradient field with a rotating cube",
        "motion": "slow shader drift with cube rotation",
        "safeZone": "center stays low contrast",
        "avoid": "strobing and tiny high-frequency noise",
        "shader": {
            "language": "glsl",
            "fragmentSource": "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, progress, 1.0); }"
        },
        "primitives": [
            {
                "id": "main-cube",
                "type": "cube",
                "material": { "color": "#63e6be" },
                "motion": { "orbit": true }
            }
        ]
    })
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_gpu_visual_shader_and_primitive_schema
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_gpu_visuals_
```

Expected: FAIL because `gpuVisuals` is not in the schema or proposal model.

- [ ] **Step 4: Add `gpu_visuals` to proposal model and validation**

Modify `CodexEditProposal` in `src-tauri/src/codex/proposal.rs`:

```rust
#[serde(default)]
pub gpu_visuals: Vec<serde_json::Value>,
```

Add `gpu_visuals: Vec::new()` to every existing `CodexEditProposal` literal in tests and fixtures. Update `validate_codex_edit_proposal` so the EDL validation still runs before visual validation. GPU visual timing can be handled in `render_pipeline::proposal` to avoid coupling `codex::proposal` to `gpu_graphics`.

- [ ] **Step 5: Add app-server prompt and schema**

In `build_video_edit_prompt`, add this section after Animated primitive nodes:

```text
GPU visuals:
- Use gpuVisuals only for generated full-frame or mostly full-frame shader/3D beats after the EDL is real.
- Raw GLSL must define vec4 video_creater_fragment(vec2 uv, float time, float progress).
- Use kind shader_background for shader-only visuals and hybrid_scene for shader plus primitives.
- Supported first-slice primitives: cube and grid.
- Every GPU visual must include visualTreatment, motion, safeZone, and avoid.
- Avoid strobing, unsafe high-frequency noise, unreadable patterns, and generic static slides.
```

Update `codex_edit_proposal_output_schema`:

```rust
"required": ["mediaId", "clips", "captions", "overlays", "hyperframes", "gpuVisuals", "renderReview"],
```

Add a `gpuVisuals` property:

```rust
"gpuVisuals": {
    "type": "array",
    "items": gpu_visual_schema()
},
```

Add helper functions:

```rust
fn gpu_visual_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "id",
            "kind",
            "startSeconds",
            "durationSeconds",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid",
            "shader"
        ],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "kind": { "type": "string", "enum": ["shader_background", "hybrid_scene"] },
            "startSeconds": { "type": "number", "minimum": 0 },
            "durationSeconds": { "type": "number", "minimum": 0.001 },
            "sourceBeat": { "type": "string" },
            "visualTreatment": { "type": "string", "minLength": 1 },
            "motion": { "type": "string", "minLength": 1 },
            "safeZone": { "type": "string", "minLength": 1 },
            "avoid": { "type": "string", "minLength": 1 },
            "shader": {
                "type": "object",
                "required": ["language", "fragmentSource"],
                "additionalProperties": false,
                "properties": {
                    "language": { "type": "string", "enum": ["glsl"] },
                    "fragmentSource": { "type": "string", "minLength": 1, "maxLength": 20480 },
                    "uniforms": { "type": "object" }
                }
            },
            "primitives": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "type", "material"],
                    "additionalProperties": false,
                    "properties": {
                        "id": { "type": "string", "minLength": 1 },
                        "type": { "type": "string", "enum": ["cube", "grid"] },
                        "material": {
                            "type": "object",
                            "required": ["color"],
                            "additionalProperties": false,
                            "properties": {
                                "color": { "type": "string", "minLength": 1 }
                            }
                        },
                        "motion": { "type": "object" }
                    }
                }
            }
        }
    })
}
```

- [ ] **Step 6: Add render proposal GPU visual conversion**

In `src-tauri/src/render_pipeline/proposal.rs`, import GPU types:

```rust
use crate::gpu_graphics::error::gpu_errors_to_pipeline_errors;
use crate::gpu_graphics::ir::{
    Camera, CameraPreset, GpuGraphicRole, GpuGraphicsLayer, Material, MaterialKind,
    Primitive, PrimitiveAnimation, PrimitiveScene, RotationAnimation, ShaderLanguage,
    ShaderPass, Transform3,
};
use crate::gpu_graphics::validation::validate_gpu_graphics_layer;
```

Add public conversion:

```rust
pub fn proposal_gpu_visuals_to_layers_for_duration(
    proposal: &CodexEditProposal,
    width: u32,
    height: u32,
    fps: f64,
    render_duration_seconds: f64,
) -> PipelineResult<Vec<GpuGraphicsLayer>> {
    validate_gpu_visual_timing(proposal, render_duration_seconds)?;
    let mut layers = Vec::with_capacity(proposal.gpu_visuals.len());
    for (index, visual) in proposal.gpu_visuals.iter().enumerate() {
        let layer = gpu_visual_to_layer(visual, index, width, height, fps)?;
        validate_gpu_graphics_layer(&layer).map_err(gpu_errors_to_pipeline_errors)?;
        layers.push(layer);
    }
    Ok(layers)
}
```

Add timing validation:

```rust
fn validate_gpu_visual_timing(
    proposal: &CodexEditProposal,
    render_duration_seconds: f64,
) -> PipelineResult<()> {
    for (index, visual) in proposal.gpu_visuals.iter().enumerate() {
        validate_visual_timing(
            visual,
            &format!("gpuVisuals[{index}]"),
            render_duration_seconds,
        )?;
    }
    Ok(())
}
```

Add conversion helper:

```rust
fn gpu_visual_to_layer(
    visual: &Value,
    index: usize,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GpuGraphicsLayer> {
    let base_path = format!("gpuVisuals[{index}]");
    let id = required_string(visual, &base_path, "id")?;
    let kind = required_string(visual, &base_path, "kind")?;
    let timeline_start = required_non_negative_number(visual, &base_path, "startSeconds")?;
    let duration_seconds = required_positive_number(visual, &base_path, "durationSeconds")?;
    let visual_treatment = required_visual_metadata(visual, &base_path, "visualTreatment")?;
    let motion = required_visual_metadata(visual, &base_path, "motion")?;
    let safe_zone = required_visual_metadata(visual, &base_path, "safeZone")?;
    let avoid = required_visual_metadata(visual, &base_path, "avoid")?;
    let source_beat = optional_non_empty_string(visual, "sourceBeat")
        .unwrap_or_else(|| "GPU generated visual beat".to_string());
    let shader = visual.get("shader").ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.shader"),
            "GPU visual is missing shader.",
            "Add shader.language and shader.fragmentSource.",
        )]
    })?;
    let language = required_string(shader, &format!("{base_path}.shader"), "language")?;
    if language != "glsl" {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            format!("{base_path}.shader.language"),
            "GPU visual shader language is unsupported.",
            "Use glsl.",
        )]);
    }
    let fragment_source =
        required_string(shader, &format!("{base_path}.shader"), "fragmentSource")?;
    let background = Some(ShaderPass {
        shader_language: ShaderLanguage::Glsl,
        fragment_source,
        uniforms: shader
            .get("uniforms")
            .and_then(Value::as_object)
            .map(|object| {
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default(),
    });

    let role = match kind.as_str() {
        "shader_background" => GpuGraphicRole::ShaderBackground,
        "hybrid_scene" => GpuGraphicRole::HybridScene,
        _ => {
            return Err(vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.kind"),
                "GPU visual kind is unsupported.",
                "Use shader_background or hybrid_scene.",
            )])
        }
    };
    let scene = if role == GpuGraphicRole::HybridScene {
        Some(gpu_scene_from_visual(visual, &base_path)?)
    } else {
        None
    };

    Ok(GpuGraphicsLayer {
        schema_version: 1,
        id,
        role,
        timeline_start,
        duration_seconds,
        dimensions: Dimensions { width, height },
        fps,
        alpha: true,
        source_beat,
        visual_treatment,
        motion,
        safe_zone,
        avoid,
        background,
        scene,
    })
}
```

Add primitive conversion:

```rust
fn gpu_scene_from_visual(visual: &Value, base_path: &str) -> PipelineResult<PrimitiveScene> {
    let primitive_values = visual
        .get("primitives")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{base_path}.primitives"),
                "Hybrid GPU visual is missing primitives.",
                "Add at least one cube or grid primitive.",
            )]
        })?;
    let primitives = primitive_values
        .iter()
        .enumerate()
        .map(|(index, primitive)| gpu_primitive_from_value(primitive, base_path, index))
        .collect::<PipelineResult<Vec<_>>>()?;
    Ok(PrimitiveScene {
        camera: Camera {
            preset: CameraPreset::Orbit,
            position: None,
            target: Some([0.0, 0.0, 0.0]),
            distance: Some(4.0),
            fov_degrees: Some(45.0),
        },
        primitives,
    })
}

fn gpu_primitive_from_value(
    primitive: &Value,
    base_path: &str,
    index: usize,
) -> PipelineResult<Primitive> {
    let primitive_path = format!("{base_path}.primitives[{index}]");
    let id = optional_non_empty_string(primitive, "id")
        .unwrap_or_else(|| format!("gpu-primitive-{}", index + 1));
    let primitive_type = required_string(primitive, &primitive_path, "type")?;
    let color = primitive
        .get("material")
        .and_then(|material| material.get("color"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                format!("{primitive_path}.material.color"),
                "GPU primitive material color is missing.",
                "Set material.color to a hex color.",
            )]
        })?;
    Ok(Primitive {
        id,
        primitive_type,
        transform: Transform3 {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        },
        material: Material {
            kind: MaterialKind::Flat,
            color,
            opacity: Some(1.0),
        },
        animate: Some(PrimitiveAnimation {
            rotation: Some(RotationAnimation {
                axis: [0.0, 1.0, 0.0],
                turns: 1.0,
            }),
        }),
    })
}
```

- [ ] **Step 7: Run schema and conversion tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_gpu_visual_shader_and_primitive_schema
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_gpu_visuals_
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
rtk git add src-tauri/src/codex/proposal.rs src-tauri/src/codex/app_server.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/codex_app_server.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: accept gpu visuals in proposals"
```

## Task 7: Render Proposal GPU Artifacts And Reports

**Files:**
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing render proposal tests**

Add this test near `render_proposal_graphics_writes_rust_artifacts`:

```rust
#[test]
fn render_proposal_graphics_writes_gpu_artifacts_or_reports_unavailable() {
    let dir = tempfile::tempdir().expect("temp graphics dir");
    let project = sample_project_for_proposal();
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    report.proposal.captions = Vec::new();
    report.proposal.overlays = Vec::new();

    let result = render_proposal_graphics(&project, &report, dir.path());

    match result {
        Ok(overlays) => {
            assert_eq!(overlays.len(), 1);
            assert_eq!(overlays[0].artifact_dir, dir.path().join("shader-hook-bg"));
            assert_eq!(overlays[0].manifest.frame_count, 4);
            assert!(dir.path().join("shader-hook-bg/manifest.json").exists());
            assert!(dir.path().join("shader-hook-bg/preview.png").exists());
            assert!(dir
                .path()
                .join("shader-hook-bg/frames/frame-000003.png")
                .exists());
        }
        Err(errors)
            if errors.iter().any(|error| {
                error
                    .details
                    .get("gpuGraphicsCode")
                    .is_some_and(|code| code == "GPU_GRAPHICS_DEVICE_UNAVAILABLE")
            }) =>
        {
            eprintln!("Skipping GPU render proposal artifact assertion because no compatible adapter is available");
        }
        Err(errors) => panic!("unexpected render proposal GPU errors: {errors:?}"),
    }
}
```

Add a dry-run count test:

```rust
#[test]
fn render_proposal_dry_run_counts_gpu_visual_layers() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let mut report = sample_codex_report();
    report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    let config = RenderProposalConfig {
        project_root: "/tmp/video-project".into(),
        source_video_path: "/tmp/video-project/media/input.mp4".into(),
        codex_report_path: "/tmp/video-project/proposal.json".into(),
        output_dir: "/tmp/video-project/renders/proposal".into(),
        final_name: "final.webm".to_string(),
    };

    let preview = build_render_proposal_preview(&project, &request, &report, &config)
        .expect("valid proposal should build a dry-run command");

    assert_eq!(preview.graphics_layer_count, 3);
    assert!(preview.command.args.iter().any(|arg| {
        arg == "--overlay=/tmp/video-project/renders/proposal/graphics/shader-hook-bg/frames/frame-000000.png@0.000+1.000"
    }));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_proposal_graphics_writes_gpu_artifacts_or_reports_unavailable render_proposal_dry_run_counts_gpu_visual_layers
```

Expected: FAIL because render proposal does not include GPU layers yet.

- [ ] **Step 3: Render GPU artifacts beside CPU artifacts**

In `render_proposal_graphics_artifacts`, compute both CPU and GPU layers:

```rust
let cpu_layers = proposal_visuals_to_graphics_layers_for_duration(
    &report.proposal,
    project.render_settings.width,
    project.render_settings.height,
    project.render_settings.fps,
    render_duration_seconds,
)?;
let gpu_layers = proposal_gpu_visuals_to_layers_for_duration(
    &report.proposal,
    project.render_settings.width,
    project.render_settings.height,
    project.render_settings.fps,
    render_duration_seconds,
)?;
let assets = AssetRegistry::new(PathBuf::new());
let mut graphics = Vec::with_capacity(cpu_layers.len() + gpu_layers.len());
```

Keep the existing CPU render loop and add this GPU loop:

```rust
for layer in &gpu_layers {
    let layer_dir = output_dir.join(&layer.id);
    let manifest = crate::gpu_graphics::renderer::render_gpu_graphics_layer(
        layer,
        crate::gpu_graphics::renderer::GpuRenderOptions {
            output_dir: layer_dir.clone(),
        },
    )
    .map_err(crate::gpu_graphics::error::gpu_errors_to_pipeline_errors)?;
    graphics.push((manifest, layer_dir, layer.timeline_start));
}
```

Update `build_render_proposal_preview` so it appends synthetic GPU manifests:

```rust
let gpu_layers = proposal_gpu_visuals_to_layers_for_duration(
    &report.proposal,
    project.render_settings.width,
    project.render_settings.height,
    project.render_settings.fps,
    render_duration_seconds,
)?;
```

Add helper:

```rust
fn manifest_for_gpu_graphics_layer(layer: &GpuGraphicsLayer) -> GraphicsArtifactManifest {
    GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("{}.preview", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count: (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32,
        frames_pattern: GRAPHICS_FRAMES_PATTERN.to_string(),
        preview_path: PathBuf::from(GRAPHICS_PREVIEW_FILE_NAME),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    }
}
```

Append GPU manifests to the preview command graphics vector:

```rust
graphics.extend(gpu_layers.iter().map(|layer| {
    (
        manifest_for_gpu_graphics_layer(layer),
        config.output_dir.join(GRAPHICS_DIR_NAME).join(layer.id.as_str()),
        layer.timeline_start,
    )
}));
```

- [ ] **Step 4: Run render proposal tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_proposal_graphics_writes_gpu_artifacts_or_reports_unavailable render_proposal_dry_run_counts_gpu_visual_layers
```

Expected: PASS, with GPU renderer unavailable handled as a clear skip branch in the artifact test.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: render proposal gpu artifacts"
```

## Task 8: Tiny End-To-End GPU Visual Fixture

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write e2e fixture test**

Add this cfg-gated test near `render_proposal_runner_writes_final_webm_and_report_before_success`:

```rust
#[cfg(feature = "ges-render")]
#[test]
fn render_proposal_runner_composites_gpu_visual_when_gpu_available() {
    let tempdir = tempfile::tempdir().expect("temp render proposal dir");
    let source_path = tempdir.path().join("source.webm");
    generate_tiny_source_video(&source_path);

    let report_path = tempdir.path().join("codex-report.json");
    let mut codex_report = tiny_source_codex_report();
    codex_report.proposal.captions = Vec::new();
    codex_report.proposal.overlays = Vec::new();
    codex_report.proposal.gpu_visuals = vec![sample_gpu_visual()];
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&codex_report).expect("codex report json"),
    )
    .expect("write codex report");

    let output_dir = tempdir.path().join("proposal-render");
    let config = RenderProposalConfig {
        project_root: tempdir.path().to_path_buf(),
        source_video_path: source_path,
        codex_report_path: report_path,
        output_dir,
        final_name: "accepted.webm".to_string(),
    };

    let result = run_render_proposal(&config);

    match result {
        Ok(result) => {
            assert!(result.final_path.exists(), "final WebM should exist");
            assert!(config
                .output_dir
                .join("graphics/shader-hook-bg/frames/frame-000000.png")
                .exists());
            assert!(config
                .output_dir
                .join("graphics/shader-hook-bg/frames/frame-000003.png")
                .exists());
            let report_json = std::fs::read_to_string(&result.render_report_path)
                .expect("render report should read");
            let report: RenderReport =
                serde_json::from_str(&report_json).expect("render report should deserialize");
            assert!(report.artifacts.iter().any(|artifact| {
                artifact.ends_with("graphics/shader-hook-bg/manifest.json")
            }));
        }
        Err(errors)
            if errors.iter().any(|error| {
                error
                    .details
                    .get("gpuGraphicsCode")
                    .is_some_and(|code| code == "GPU_GRAPHICS_DEVICE_UNAVAILABLE")
            }) =>
        {
            eprintln!("Skipping GPU visual e2e because no compatible adapter is available");
        }
        Err(errors) => panic!("unexpected GPU visual e2e errors: {errors:?}"),
    }
}
```

- [ ] **Step 2: Run the e2e fixture**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_proposal_runner_composites_gpu_visual_when_gpu_available
```

Expected: PASS when GES and GPU are available, or clear skip branch when no compatible GPU adapter is available.

- [ ] **Step 3: Commit**

Run:

```bash
rtk git add src-tauri/tests/render_pipeline.rs
rtk git commit -m "test: cover gpu visual render proposal"
```

## Task 9: Full Verification And Documentation Touches

**Files:**
- Modify: `docs/superpowers/specs/2026-06-18-rust-native-3d-shader-graphics-design.md`
- Optional modify: `AGENTS.md` only if implementation discovers a project-local rule that must be recorded for future agents.

- [ ] **Step 1: Update spec implementation notes with actual backend behavior**

Append this section to the design spec:

```markdown
## Implementation Status

- First implementation uses `wgpu 29.0.3` with GLSL and WGSL features enabled.
- Raw proposal GLSL is wrapped into a fixed fragment entrypoint and paired with a Rust-owned fullscreen vertex shader.
- GStreamer/GES composes bounded PNG frame sequences by adding one short image clip per rendered frame.
- GPU-unavailable environments return actionable `GPU_GRAPHICS_DEVICE_UNAVAILABLE` errors; tests treat that condition as a skip where GPU hardware is not required.
```

- [ ] **Step 2: Run targeted verification**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test gpu_graphics
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server gpu_visual
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_gpu_visuals_ render_proposal_dry_run_counts_gpu_visual_layers gstreamer_ges_backend_accepts_bounded_graphics_frame_sequences
```

Expected: PASS.

- [ ] **Step 3: Run broader project verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS. If this fails because the environment lacks GStreamer framework paths or a GPU adapter, record the exact failure and run the targeted non-GPU tests above before handing off.

- [ ] **Step 4: Final commit**

Run:

```bash
rtk git add docs/superpowers/specs/2026-06-18-rust-native-3d-shader-graphics-design.md
rtk git commit -m "docs: record gpu graphics implementation status"
```

## Self-Review Notes

Spec coverage:

- Raw GLSL proposal support is covered by Tasks 2 and 6.
- Primitive cube/grid support is covered by Task 3 and Task 4.
- GPU artifact manifests are covered by Task 4 and Task 7.
- GStreamer/GES composition is covered by Task 5 and Task 8.
- EDL-first proposal timing validation is covered by Task 6.
- Actionable GPU errors are covered by Tasks 1, 2, 4, and 7.
- Mac-first GPU-unavailable behavior is covered by Task 4 and Task 8.
- CPU graphics preservation is maintained by keeping existing `graphics` code paths and adding GPU visuals beside them.

Incomplete-work scan:

- No incomplete sections are required for execution.
- Every task has exact files, concrete tests, commands, expected results, and commit commands.

Type consistency:

- Proposal field is `gpu_visuals` in Rust and `gpuVisuals` in JSON.
- GPU layer conversion function is `proposal_gpu_visuals_to_layers_for_duration`.
- Renderer entrypoint is `render_gpu_graphics_layer`.
- Artifact contract remains `GraphicsArtifactManifest`.
