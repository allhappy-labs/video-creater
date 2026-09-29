# Rust Native 3D And Shader Graphics Design

## Summary

Add Rust-native GPU graphics generation to Video Creater so generated edits can include procedural shader backgrounds and simple 3D primitive scenes. The first shippable slice renders one raw GLSL fullscreen background plus one simple primitive scene into the same `rgbaFrameSequence` artifact manifest already used by the Rust graphics pipeline.

The new GPU renderer is an artifact producer, not a new video backend. It validates a bounded GPU graphics IR, renders frames through `wgpu`, writes PNG frame sequences plus previews, and hands the existing `GraphicsArtifactManifest` to the current GStreamer/GES composition path.

The current CPU graphics renderer remains responsible for captions, lower thirds, callouts, image refs, and text-heavy overlays. GPU graphics are for full-frame or mostly full-frame generated visual beats where shaders and 3D motion add value.

## Goals

- Support proposal-authored procedural GLSL backgrounds for video layers.
- Support simple Rust-defined 3D primitives such as cubes, grids, planes, rings, spheres, and particle fields.
- Prove a combined shader-plus-primitive layer behind the existing graphics artifact manifest.
- Keep the EDL-first video pipeline unchanged: selected source clips come before visual layers.
- Keep GStreamer/GES as the compositor and encoder.
- Return concise actionable errors for invalid GLSL, GPU unavailability, invalid primitives, excessive frame budgets, and render failures.
- Allow Mac-first implementation while preserving a portable `wgpu` architecture.
- Allow raising the Rust toolchain requirement to support the selected `wgpu` version.

## Non-Goals

- Replacing the existing `tiny-skia` and `cosmic-text` graphics renderer.
- Changing the video backend contract.
- Building a full game engine or importing Bevy as the render runtime.
- Supporting arbitrary multi-pass shader graphs in the first version.
- Supporting compute shaders, storage buffers, shader includes, filesystem reads, remote textures, or user-provided vertex shaders in the first version.
- Supporting GLTF or full 3D asset import in the first version.
- Promising Linux and Windows GPU coverage before the Mac-first slice is working.

## Current Context

The repository already has a Rust-owned graphics and render pipeline:

- `src-tauri/src/graphics/ir.rs` defines the current CPU graphics IR for text, rectangles, lines, image refs, and animation.
- `src-tauri/src/graphics/renderer.rs` renders CPU graphics into PNG frames and writes `GraphicsArtifactManifest`.
- `src-tauri/src/render_pipeline/gstreamer_backend.rs` adapts graphics manifests into GStreamer/GES overlay layers.
- `docs/superpowers/specs/2026-06-17-rust-native-graphics-frame-generation-design.md` establishes the current artifact boundary.
- `docs/superpowers/plans/2026-06-18-graphics-animation-primitives.md` extends the CPU graphics path with keyframed primitive animation.

The 3D and shader work should reuse that artifact boundary instead of mixing GPU behavior into the CPU primitive renderer.

## Chosen Approach

Use `wgpu` as a new Rust GPU artifact producer.

The GPU renderer receives a validated `GpuGraphicsLayer`, renders one frame per output frame into an offscreen RGBA texture, copies the texture back to CPU memory, writes PNG frames, writes a preview image, and returns the existing `GraphicsArtifactManifest`.

Conceptual flow:

```text
Codex/user proposal
  -> validated video proposal
  -> GraphicsLayer or GpuGraphicsLayer
  -> CPU renderer or GPU renderer
  -> rgbaFrameSequence manifest
  -> existing GStreamer/GES overlay/composition path
```

This keeps the app's render pipeline stable. Existing overlays and captions continue through the CPU path. Shader backgrounds and primitive scenes use the GPU path.

## Alternatives Considered

### Bevy Mini-Renderer

Bevy would provide scenes, cameras, meshes, materials, and custom shaders. It is powerful, but it brings a full ECS, asset runtime, schedule, and render graph into a pipeline that needs deterministic offline frame generation and compact actionable errors. This is too heavy for the first slice.

### CPU Software 3D

A tiny software rasterizer would be easy to sandbox and deterministic, but it misses the core GLSL-background goal and would quickly become a low-value custom graphics engine.

### Raw Vulkan Or Metal

Raw graphics APIs would offer maximum control, but platform plumbing, shader compilation, synchronization, and readback would slow the project without improving the product-facing first slice.

## GPU Graphics IR

Add a separate module under `src-tauri/src/gpu_graphics/` with a typed IR. Do not overload the existing `GraphicNode` enum for GPU concerns.

Top-level layer shape:

```json
{
  "schemaVersion": 1,
  "id": "gpu-hook-visual",
  "role": "hybrid_scene",
  "timelineStart": 0,
  "durationSeconds": 3,
  "dimensions": { "width": 1920, "height": 1080 },
  "fps": 30,
  "alpha": false,
  "sourceBeat": "Open the edit with an energetic generated visual beat.",
  "visualTreatment": "procedural gradient field with a rotating wire cube",
  "motion": "slow shader drift with orbiting cube rotation",
  "safeZone": "keep center low contrast for titles and essential detail inside 10% margins",
  "avoid": "strobing, high-frequency noise, tiny patterns, text-like artifacts",
  "background": {
    "shaderLanguage": "glsl",
    "fragmentSource": "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, 0.2 + 0.8 * progress, 1.0); }",
    "uniforms": {
      "intensity": 0.7,
      "speed": 0.85,
      "paletteA": "#101820",
      "paletteB": "#63e6be"
    }
  },
  "scene": {
    "camera": { "preset": "orbit", "distance": 4.5 },
    "primitives": [
      {
        "id": "main-cube",
        "type": "cube",
        "transform": {
          "position": [0, 0, 0],
          "rotation": [0, 0, 0],
          "scale": [1, 1, 1]
        },
        "material": {
          "kind": "flat",
          "color": "#63e6be"
        },
        "animate": {
          "rotation": { "axis": [0, 1, 0], "turns": 1.0 }
        }
      }
    ]
  }
}
```

Allowed roles:

- `shader_background`
- `primitive_scene`
- `hybrid_scene`

`shader_background` requires `background` and no `scene`.

`primitive_scene` requires `scene` and may use a transparent or solid generated background.

`hybrid_scene` requires both `background` and `scene`.

## Raw GLSL Contract

Raw GLSL is allowed in proposals, but only as a constrained fragment function.

Proposal authors provide this function:

```glsl
vec4 video_creater_fragment(vec2 uv, float time, float progress) {
  return vec4(uv, 0.5 + 0.5 * sin(time), 1.0);
}
```

Rust wraps that function into the actual shader entry point and supplies a fixed fullscreen vertex shader. This avoids proposal-owned vertex shaders, file includes, custom entry points, storage buffers, and arbitrary pipeline layout.

Fixed uniforms supplied by Rust:

- `u_time`: seconds from the start of the GPU layer.
- `u_duration`: layer duration in seconds.
- `u_resolution`: output width and height.
- `u_frame`: zero-based frame index.
- `u_progress`: normalized progress from 0 to 1.

User uniforms are passed through a bounded uniform object. Initial uniform value types:

- finite number
- boolean
- color hex string
- number vector with 2, 3, or 4 finite values

Shader validation should reject:

- source longer than the configured maximum.
- missing `video_creater_fragment`.
- preprocessor directives except `#define` for simple constants.
- references to filesystem paths, URLs, external textures, storage buffers, compute stages, or unsupported entry points.
- compile or translation failures from the selected shader path.

Validation cannot prove shader runtime cost in all cases. The renderer must also enforce frame budgets and process-level timeouts in integration paths.

## Primitive Scene V1

Supported primitive types:

- `plane`
- `cube`
- `sphere`
- `grid`
- `ring`
- `particleField`

The first implementation only needs to ship `cube` and `grid` if that is enough to prove the vertical slice. Other primitive structs can be defined after the renderer and proposal path are stable.

Primitive fields:

- `id`
- `type`
- `transform`
- `material`
- optional `animate`

Transform fields:

- `position: [x, y, z]`
- `rotation: [x, y, z]` in radians
- `scale: [x, y, z]`

Material fields:

- `kind: "flat" | "emissive" | "wire"`
- `color`
- optional `opacity`

Camera fields:

- `preset: "fixed" | "orbit" | "dolly"`
- `position`
- `target`
- `distance`
- `fovDegrees`

Animation fields should stay declarative. V1 should support rotation over normalized progress and simple orbit/dolly camera motion. More complex timelines can reuse or mirror the existing CPU animation evaluator later.

## Validation

Validation must happen before GPU initialization when possible.

Layer validation:

- schema version is supported.
- id is non-empty.
- dimensions are finite and within renderer bounds.
- fps and duration are finite and within renderer bounds.
- frame count and pixel-frame budget are within limits.
- required visual metadata fields are non-empty.
- role-specific `background` and `scene` requirements are met.
- alpha is explicit.

Shader validation:

- raw GLSL uses the `video_creater_fragment` function contract.
- source length is bounded.
- disallowed preprocessor and external resource patterns are rejected.
- user uniforms are finite, typed, and bounded.
- shader compile or translation failures return actionable errors with a concise source location when available.

Primitive validation:

- primitive count is bounded.
- generated vertex and index counts are bounded.
- transforms contain finite values.
- camera values are finite and non-degenerate.
- material colors and opacity are valid.
- particle counts are bounded.

Output validation:

- output frame sequence is non-empty.
- manifest dimensions, fps, duration, alpha, and frame count match the requested layer.
- artifact paths stay inside the configured output directory.
- first, middle, and last sampled frames can be inspected in the e2e workflow.

Initial v1 budgets:

- max shader source length: 20 KiB.
- max dimensions: 1920x1080 unless a later render setting explicitly opts into higher GPU budgets.
- max fps: 60 for GPU-generated layers.
- max GPU layer duration: 20 seconds.
- max rendered GPU frames per layer: 600.
- max primitive count: 256.
- max generated vertices per layer: 100,000.
- max generated indices per layer: 300,000.
- max particle count: 10,000.

These budgets can be raised after the first end-to-end renderer is stable and measured.

## Renderer Module Shape

Add:

```text
src-tauri/src/gpu_graphics/
  mod.rs
  ir.rs
  validation.rs
  shader.rs
  mesh.rs
  renderer.rs
  error.rs
```

Public API:

```rust
pub fn render_gpu_graphics_layer(
    layer: &GpuGraphicsLayer,
    options: GpuRenderOptions,
) -> GpuGraphicsResult<GraphicsArtifactManifest>;
```

`GpuRenderOptions` should include:

- output directory
- optional adapter preference
- optional force software adapter flag if supported by selected `wgpu`
- render timeout or per-frame budget hooks where practical

Renderer flow:

1. Validate the layer.
2. Create or receive a `wgpu` device and queue.
3. Build the shader background pipeline when `background` is present.
4. Generate primitive meshes and build primitive pipelines when `scene` is present.
5. For each output frame:
   - update uniform buffer with time, duration, frame, progress, resolution, and user uniforms.
   - render the shader background into an offscreen RGBA texture.
   - render primitive scene on top.
   - copy texture to a CPU-readable buffer.
   - write `frames/frame-%06d.png`.
6. Write `preview.png` from the first frame.
7. Write and return `manifest.json` using the existing `GraphicsArtifactManifest` shape.

The first implementation can write every frame as PNG. Later optimizations can pipe raw frames directly to the active video backend or deduplicate static intervals.

## Proposal Integration

Add a separate `gpuVisuals` array to structured Codex video proposals. Do not mix GPU scenes into `captions` or `overlays`.

Example:

```json
{
  "gpuVisuals": [
    {
      "id": "shader-hook-bg",
      "kind": "hybrid_scene",
      "startSeconds": 0,
      "durationSeconds": 3,
      "qualityProfile": "hq-neon-wireframe-shader-v1",
      "visualTreatment": "restrained animated gradient shader background with crisp neon wireframe 3D primitives",
      "motion": "slow shader drift, orbiting cube rotation, and subtle motion trails",
      "safeZone": "no essential detail inside outer 10%; center stays low contrast for titles",
      "avoid": "oversized saturated blobs, low-resolution draft looks, strobing, dense noise, tiny high-frequency patterns",
      "shader": {
        "language": "glsl",
        "fragmentSource": "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, 0.3, 1.0); }"
      },
      "primitives": [
        {
          "type": "cube",
          "material": { "color": "#63e6be" },
          "motion": { "orbit": true }
        }
      ]
    }
  ]
}
```

Pipeline order remains:

```text
EDL clips -> render duration -> captions/overlays -> gpuVisuals -> graphics artifacts -> GStreamer/GES render
```

GPU visuals must be validated against the final render duration so they cannot start or end outside the edited sequence.

## Error Contract

GPU errors should use the same actionable style as existing graphics and render pipeline errors:

```json
{
  "code": "GPU_GRAPHICS_SHADER_COMPILE_FAILED",
  "path": "gpuVisuals[0].shader.fragmentSource",
  "message": "GLSL shader did not compile.",
  "fix": "Return a valid video_creater_fragment function using supported uniforms only."
}
```

Initial error codes:

- `GPU_GRAPHICS_SCHEMA_UNSUPPORTED`
- `GPU_GRAPHICS_INVALID_TIMING`
- `GPU_GRAPHICS_INVALID_DIMENSIONS`
- `GPU_GRAPHICS_FRAME_BUDGET_EXCEEDED`
- `GPU_GRAPHICS_SHADER_SOURCE_TOO_LARGE`
- `GPU_GRAPHICS_SHADER_ENTRY_MISSING`
- `GPU_GRAPHICS_SHADER_UNSUPPORTED_FEATURE`
- `GPU_GRAPHICS_SHADER_COMPILE_FAILED`
- `GPU_GRAPHICS_UNIFORM_INVALID`
- `GPU_GRAPHICS_PRIMITIVE_UNSUPPORTED`
- `GPU_GRAPHICS_PRIMITIVE_INVALID`
- `GPU_GRAPHICS_CAMERA_INVALID`
- `GPU_GRAPHICS_DEVICE_UNAVAILABLE`
- `GPU_GRAPHICS_RENDER_FAILED`
- `GPU_GRAPHICS_READBACK_FAILED`
- `GPU_GRAPHICS_ARTIFACT_WRITE_FAILED`

Errors should include precise `path` values under `gpuVisuals[index]` when they originate from proposals.

## Testing

Unit tests:

- IR deserializes a minimal valid `shader_background`.
- IR deserializes a minimal valid `hybrid_scene`.
- validation rejects missing visual metadata.
- validation rejects invalid timing, fps, dimensions, and frame budgets.
- validation rejects missing `video_creater_fragment`.
- validation rejects disallowed shader include/preprocessor patterns.
- mesh generation produces finite vertices and indices for cube and grid.
- invalid primitive transforms return actionable errors.

Renderer tests:

- render a tiny one-second shader layer when a compatible GPU adapter is available.
- render a tiny hybrid layer with shader background and cube/grid primitive.
- skip with a clear reason when no compatible GPU adapter is available.
- generated manifest has expected dimensions, fps, frame count, alpha, preview path, and frames pattern.

Render pipeline tests:

- GPU manifests convert into existing GStreamer/GES overlay inputs.
- proposal `gpuVisuals` convert into `GpuGraphicsLayer` after EDL/render duration validation.
- proposal timing outside render duration is rejected.

End-to-end fixture:

- create a tiny source WebM.
- render a one-second hybrid GPU visual.
- composite it with GStreamer/GES through the current render backend.
- validate final WebM duration and stream presence with GStreamer Discoverer.
- extract first, middle, and last frames for visual QA.

## Rollout

1. Add `gpu_graphics` IR, validation, error types, and tests.
2. Add shader wrapper and source validation tests.
3. Add cube and grid mesh generation tests.
4. Add `wgpu` renderer behind the new module boundary.
5. Render one tiny shader background to PNG frames and manifest.
6. Render one tiny hybrid shader plus primitive layer.
7. Wire proposal `gpuVisuals` into the render proposal path.
8. Add the GStreamer/GES composition e2e fixture.
9. Raise the Rust toolchain requirement to the selected `wgpu` requirement.
10. Document Mac-first support and GPU-unavailable behavior.

## Acceptance Criteria

- A structured proposal can include one raw GLSL fullscreen background and one primitive cube or grid scene.
- Rust validates the GPU visual before rendering.
- Rust renders the GPU visual into PNG frames, `preview.png`, and `manifest.json`.
- The manifest uses the existing `GraphicsArtifactManifest` contract.
- The existing GStreamer/GES backend composites the generated GPU artifact without a new video backend concept.
- Bad GLSL returns a concise actionable error.
- Oversized frame budgets return a concise actionable error before rendering.
- Missing or unsupported GPU adapters return a concise actionable error or a test skip where appropriate.
- The current CPU graphics renderer remains the path for captions, overlays, text, image refs, and lower thirds.

## Implementation Notes

- Keep shader and primitive rendering separate inside the renderer so future shader-only backgrounds do not pay scene setup costs.
- Prefer a device/context abstraction that can be reused across multiple GPU layers in a render job.
- Keep raw GLSL support narrow. If it becomes unreliable, fall back to a wrapped WGSL internal representation while preserving proposal-facing GLSL where feasible.
- Keep proposal output bounded. Raw shader source should be short enough for review and repair in Codex loops.
- Store generated GPU artifacts next to existing graphics artifacts so render reports can link to them consistently.

## Implementation Status

- First implementation uses `wgpu 29.0.3` with GLSL and WGSL features enabled.
- Raw proposal GLSL is wrapped into a fixed fragment entrypoint and paired with a Rust-owned fullscreen vertex shader.
- GStreamer/GES composes bounded PNG frame sequences by adding one short image clip per rendered frame.
- GPU-unavailable environments return actionable `GPU_GRAPHICS_DEVICE_UNAVAILABLE` errors; tests treat that condition as a skip where GPU hardware is not required.
- The productionized HQ profile is Rust-owned through `hq-neon-wireframe-shader-v1`.
- GPU-unavailable environments can render the HQ profile with a deterministic software fallback.
- Visual QA samples first, middle, and last frames for built-in GPU profiles.
