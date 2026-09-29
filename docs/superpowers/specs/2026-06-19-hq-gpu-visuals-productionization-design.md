# HQ GPU Visuals Productionization Design

## Summary

Productionize the approved `hq-neon-wireframe-shader-v1` GPU visual look so it becomes reliable product behavior instead of prompt-only guidance.

The program covers six related follow-ups under one contract:

- Rust-owned profile expansion for `hq-neon-wireframe-shader-v1`.
- Deterministic CPU fallback for environments without a compatible GPU adapter.
- Real GPU primitive rendering for cube/grid wireframe scenes.
- Render-pipeline validation for `gpuVisuals[].qualityProfile`.
- Final-quality export profiles that respect the current plugin and license policy.
- Visual QA that catches the low-quality blob-style failure mode before handoff.

The guiding boundary is simple: Codex may request a visual profile, but Rust owns how that profile expands into shader source, primitive scene data, fallback behavior, render settings, and QA expectations.

## Current Context

The GPU graphics stack now has:

- `src-tauri/src/gpu_graphics/ir.rs` for typed GPU graphics layers, shader passes, primitive scenes, cameras, transforms, materials, and animation.
- `src-tauri/src/gpu_graphics/validation.rs` for bounded layer, shader, primitive, timing, and frame budget validation.
- `src-tauri/src/gpu_graphics/renderer.rs` for offscreen `wgpu` fullscreen shader rendering into PNG frame sequences and manifests.
- `src-tauri/src/render_pipeline/proposal.rs` for converting proposal `gpuVisuals` into `GpuGraphicsLayer`s.
- `src-tauri/src/render_pipeline/gstreamer_backend.rs` for composing bounded PNG frame sequences into the current GStreamer/GES WebM backend.
- `src-tauri/src/codex/app_server.rs` schema/prompt guidance requiring `qualityProfile: "hq-neon-wireframe-shader-v1"`.

The important gaps are:

- `qualityProfile` is not yet validated or interpreted by the render-pipeline conversion code.
- The approved HQ look is prompt guidance, not a Rust-owned profile expansion.
- Primitive scene data is validated, and cube/grid meshes exist, but the GPU renderer currently renders only the fullscreen shader pass.
- GPU adapter unavailability currently stops GPU rendering instead of falling back for supported built-in profiles.
- The current GStreamer/GES backend is WebM-only. MP4/H.264 is not part of the approved default backend policy.
- Visual QA is manual; the pipeline does not automatically detect the blob-heavy, low-detail failure mode.

## Goals

- Make `hq-neon-wireframe-shader-v1` a deterministic Rust-owned profile.
- Preserve the EDL-first pipeline: selected source clips must exist before GPU visuals are expanded or rendered.
- Render the HQ profile successfully on systems without a compatible GPU adapter.
- Render real cube/grid wireframe primitive scenes when GPU rendering is available.
- Keep proposal and render errors precise and actionable.
- Add a final-quality export profile without violating the current GStreamer plugin policy.
- Add automated visual QA that checks sampled frames for basic render quality.

## Non-Goals

- Replacing the current CPU graphics renderer for captions, lower thirds, callouts, text, image refs, or template overlays.
- Replacing GStreamer/GES as the default compositor.
- Making MP4/H.264 the default before an explicit plugin, license, and distribution policy is approved.
- Supporting arbitrary GLTF imports, game-engine-style scenes, user vertex shaders, compute shaders, external textures, or filesystem shader includes.
- Building exhaustive computer-vision scoring. Visual QA should catch obvious failures, not judge aesthetics perfectly.

## Chosen Approach

Use one integrated vertical product slice first:

1. Define a Rust `GpuVisualProfile` registry.
2. Validate `gpuVisuals[].qualityProfile` in render proposal conversion.
3. Expand `hq-neon-wireframe-shader-v1` into canonical shader, camera, primitives, metadata, and QA thresholds.
4. Add a CPU fallback renderer for that profile.
5. Add visual QA around generated frame samples.
6. Extend the GPU renderer to draw the primitive scene when an adapter is available.
7. Add a final-quality WebM export profile.

This sequence makes the accepted visual direction reliable early, then improves the lower-level GPU renderer and export quality without blocking fallback reliability.

## Alternatives Considered

### Renderer Foundation First

Build the real GPU primitive renderer before profile expansion and fallback. This improves technical completeness first, but it keeps the user-visible default dependent on GPU availability and agent-authored details for longer.

### Export Pipeline First

Start with MP4/H.264 or other final export work. This addresses container and compression quality, but it does not fix the default visual behavior or GPU-unavailable render failures.

### Prompt-Only Defaults

Continue improving the app-server prompt and schema. This is low-cost, but it does not guarantee repeatable visuals and cannot provide fallback rendering when `wgpu` has no compatible adapter.

## Architecture

Add a narrow profile layer between proposal parsing and concrete GPU graphics rendering.

```text
Codex proposal
  -> EDL validation
  -> gpuVisuals qualityProfile validation
  -> Rust profile expansion
  -> GPU renderer or CPU fallback renderer
  -> GraphicsArtifactManifest
  -> GStreamer/GES composition
  -> final-quality export profile
  -> visual QA report
```

The profile layer should be separate from raw shader support. Custom GLSL visuals can continue through the existing shader contract, but the default HQ profile should be deterministic and Rust-owned.

## Data Model

Add an explicit profile model, likely under `src-tauri/src/gpu_graphics/profile.rs`:

```rust
pub enum GpuVisualProfileId {
    HqNeonWireframeShaderV1,
}

pub struct GpuVisualProfile {
    pub id: GpuVisualProfileId,
    pub supports_cpu_fallback: bool,
    pub visual_qa: GpuVisualQaProfile,
}
```

Profile expansion should accept proposal timing, dimensions, fps, and optional agent-provided color or motion hints. It should return a normal `GpuGraphicsLayer` so the rest of the graphics artifact boundary stays unchanged.

For `hq-neon-wireframe-shader-v1`, the canonical expansion should include:

- restrained animated gradient shader background.
- cube and grid primitives with neon wire materials.
- orbit camera preset with stable distance and field of view.
- subtle motion trails encoded either as primitive metadata or fallback renderer behavior.
- visual metadata that names the exact treatment, motion, safe zone, and avoids.

The proposal schema should keep `qualityProfile` as required for `gpuVisuals`. Render-pipeline conversion should reject missing or unknown profiles even if a proposal is loaded directly from disk rather than produced by the app-server schema.

## Profile Expansion

Profile expansion should happen in `proposal_gpu_visuals_to_layers_for_duration` or a helper called from it.

Rules:

- If `qualityProfile` is `hq-neon-wireframe-shader-v1`, Rust expands the canonical profile.
- Agent-authored shader and primitive fields may be used as bounded hints only when they do not degrade the profile.
- If agent-authored details conflict with the profile contract, profile defaults win.
- Unknown profile ids return an actionable error at `gpuVisuals[index].qualityProfile`.
- Custom future profiles must be added to the registry and tests before the schema allows them.

This keeps the default dependable and prevents a vague prompt from producing the old oversized saturated blob look.

## CPU Fallback Renderer

Add a deterministic software renderer for the HQ profile. It should live in the GPU graphics module because it renders the same profile contract and produces the same artifact manifest shape.

Initial scope:

- `hq-neon-wireframe-shader-v1` only.
- 2D rasterization of projected 3D wireframe cube/grid geometry.
- animated gradient background matching the approved treatment.
- additive neon line glow and subtle motion trails.
- PNG frame sequence output, `preview.png`, and `manifest.json`.

The fallback should run when:

- `render_gpu_graphics_layer` returns `GPU_GRAPHICS_DEVICE_UNAVAILABLE`.
- the profile supports CPU fallback.
- the render request has not explicitly disabled fallback.

The render report should record whether a GPU or CPU renderer produced each GPU visual artifact.

## GPU Primitive Renderer

Extend the current fullscreen shader-only `wgpu` renderer so hybrid scenes actually draw primitive geometry.

Initial primitive rendering scope:

- cube wireframes.
- grid lines.
- orbit camera projection.
- primitive rotation animation.
- alpha blending.
- depth ordering sufficient for readable wireframes.
- glow pass or line-thickness approximation that preserves crisp edges.
- frame-to-frame motion trails for the HQ profile.

Implementation should reuse `build_cube_mesh` and `build_grid_mesh` where practical, but wireframe rendering may use line topology or generated edge buffers rather than filled triangle faces. Lighting is optional for the first primitive slice; readable neon wireframe geometry is required.

The shader background should render first. Primitive geometry should render over it into the same offscreen RGBA texture before readback.

## Final-Quality Export Profiles

Do not make MP4/H.264 the default in this program. The current GStreamer/GES backend validates WebM output and the plugin policy intentionally denies non-reviewed H.264 encoders.

Add a final-quality WebM profile first:

- keep the existing WebM container.
- use reviewed `vp8enc` or `vp9enc` only if plugin policy and availability allow it.
- prefer higher bitrate or quality settings than draft renders.
- expose the profile through render config or proposal render args without changing the default draft behavior unexpectedly.

MP4/H.264 can be a later policy-gated profile. That work requires explicit approval for encoder plugin provenance, license compatibility, distribution expectations, and CI availability.

## Visual QA

Add a render artifact QA step that can run after GPU visual artifact generation and after final composition.

For each GPU visual profile, sample:

- first frame.
- middle frame.
- last frame.

Initial checks:

- dimensions match expected output.
- frame is nonblank and has nonzero alpha where expected.
- no giant saturated blob dominance: reject frames where a small number of highly saturated connected regions dominate most of the frame.
- edge/detail presence: require enough high-contrast line pixels for wireframe profiles.
- temporal difference: sampled frames should differ enough to prove motion.

The QA report should include:

- sampled frame paths.
- metrics.
- pass/fail state.
- actionable failure messages.

Visual QA should be a quality gate for built-in profiles and a warning/reporting mechanism for custom raw shader visuals until thresholds mature.

## Error Handling

Use the existing actionable error style.

Examples:

```json
{
  "code": "PIPELINE_INPUT_INVALID",
  "path": "gpuVisuals[0].qualityProfile",
  "message": "GPU visual quality profile is unsupported.",
  "fix": "Use hq-neon-wireframe-shader-v1 or add the profile to the Rust registry before using it."
}
```

```json
{
  "code": "GPU_GRAPHICS_DEVICE_UNAVAILABLE",
  "path": "gpuVisuals[0]",
  "message": "No compatible GPU adapter is available for offscreen graphics rendering.",
  "fix": "The hq-neon-wireframe-shader-v1 profile supports CPU fallback; render with fallback enabled or run on a machine with a compatible GPU adapter."
}
```

```json
{
  "code": "GRAPHICS_VISUAL_QA_FAILED",
  "path": "gpuVisuals[0].visualQa",
  "message": "GPU visual QA detected blob-dominant frames without readable wireframe detail.",
  "fix": "Use the built-in hq-neon-wireframe-shader-v1 profile expansion or revise the shader and primitive treatment."
}
```

Add a new graphics or pipeline error code for visual QA failures if the existing error enums do not have a precise fit.

## Testing Strategy

Use test-driven development for each implementation slice.

### Profile Contract Tests

- App-server schema lists allowed `qualityProfile` values.
- Direct render proposal conversion rejects missing `qualityProfile`.
- Direct render proposal conversion rejects unknown `qualityProfile`.
- `hq-neon-wireframe-shader-v1` expands into a valid `GpuGraphicsLayer`.
- Profile expansion keeps dimensions, fps, duration, and timeline start from the render context.

### CPU Fallback Tests

- CPU fallback writes `preview.png`, `manifest.json`, and a bounded frame sequence.
- CPU fallback frames are nonblank.
- CPU fallback sampled frames differ over time.
- render proposal uses CPU fallback when GPU adapter is unavailable and the profile supports fallback.
- render report records fallback usage.

### GPU Primitive Renderer Tests

- cube wireframe buffer generation is finite and bounded.
- grid wireframe buffer generation is finite and bounded.
- hybrid scenes render through GPU when an adapter is available.
- adapter-unavailable environments skip GPU-specific assertions but still exercise CPU fallback.
- generated frames include primitive detail above the QA threshold.

### Final Export Tests

- final-quality WebM profile builds a GStreamer command/profile using only reviewed plugin factories.
- unsupported MP4/H.264 profile returns an actionable policy error unless explicitly enabled by future policy work.
- rendered media validation checks streams, duration, dimensions, and non-empty output.

### Visual QA Tests

- QA accepts known-good HQ fixture frames.
- QA rejects a synthetic oversized saturated blob fixture.
- QA rejects blank frames.
- QA rejects static sampled frames for animated profiles.
- QA emits sampled frame paths and metric details.

Full verification remains:

```bash
rtk pnpm verify
```

GPU-dependent tests should be written so no-compatible-adapter environments report explicit skips or use CPU fallback instead of failing unrelated CI.

## Rollout Plan

### Phase 1: Contract And Profile Registry

- Add `gpu_graphics::profile`.
- Validate missing and unknown `qualityProfile` in render proposal conversion.
- Expand `hq-neon-wireframe-shader-v1` into canonical layer data.
- Update docs and fixtures.

### Phase 2: CPU Fallback Reliability

- Add software fallback renderer for `hq-neon-wireframe-shader-v1`.
- Wire fallback into render proposal artifact generation when GPU is unavailable.
- Record renderer mode in render reports.

### Phase 3: Visual QA Gate

- Add sampled-frame extraction helpers for graphics artifacts.
- Add QA metrics and profile thresholds.
- Fail built-in profile renders when QA detects blank, blob-dominant, or static output.

### Phase 4: GPU Primitive Rendering

- Add primitive pipeline, buffers, camera transforms, and wireframe drawing.
- Render cube/grid over shader backgrounds.
- Add glow/trail polish.
- Keep CPU fallback available and tested.

### Phase 5: Final-Quality WebM Export

- Add final-quality WebM profile under current plugin policy.
- Add render args/config for final-quality export.
- Validate output with existing media probe checks.

### Phase 6: MP4/H.264 Policy Decision

- Write a separate policy/design addendum only if MP4 is required.
- Review plugin provenance, licensing, CI availability, and distribution impact.
- Do not silently enable non-reviewed encoders.

## Acceptance Criteria

- Proposals with `gpuVisuals` must include a supported `qualityProfile`.
- `hq-neon-wireframe-shader-v1` expands deterministically in Rust.
- The HQ profile renders even when no compatible GPU adapter is available.
- GPU-capable environments render visible cube/grid wireframe primitives over shader backgrounds.
- Render reports identify GPU versus CPU fallback rendering.
- Visual QA catches blank, static, and blob-dominant failure modes.
- Final-quality WebM export is available without violating plugin policy.
- Existing CPU graphics, caption, overlay, EDL, and GStreamer/GES tests continue to pass.

## Implementation Status

- `hq-neon-wireframe-shader-v1` is represented by a Rust profile registry and direct proposal validation.
- The HQ profile expands into canonical shader, camera, cube, and grid scene data.
- The HQ profile has a deterministic software fallback renderer for GPU-unavailable environments.
- Render reports identify whether GPU visuals were rendered by GPU or software fallback.
- Built-in HQ profile renders run sampled-frame visual QA before final handoff.
- Hybrid GPU scenes include visible wireframe primitive detail.
- Final-quality WebM metadata is available under the existing GStreamer/GES policy.
- MP4/H.264 remains policy-gated and is not enabled by this implementation.

## Open Policy Note

MP4/H.264 is intentionally outside the default implementation path. The repository already marks `mp4mux` as reviewed, but H.264 encoders such as `x264enc` and `openh264enc` have policy concerns in tests. Enabling MP4/H.264 requires explicit product and licensing approval before implementation.

## Spec Self-Review

- No placeholders or unresolved items.
- The architecture matches the approved one-spec direction.
- Scope is broad but decomposed into phases under a single product contract.
- Ambiguous export behavior is resolved: final-quality WebM first, MP4/H.264 requires later policy approval.
