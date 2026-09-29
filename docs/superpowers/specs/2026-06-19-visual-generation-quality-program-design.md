# Visual Generation Quality Program Design

## Summary

Improve generated visual quality by replacing vague visual prose with deterministic visual contracts.

The program covers five phases:

1. Deterministic motion presets and template recipes.
2. Visual QA for CPU-rendered overlays and captions.
3. Additional Rust-owned GPU visual profiles.
4. Final WebM export polish under the existing plugin policy.
5. Review UI that exposes template, preset, renderer, QA, and artifact details.

The first implementation slice should ship Phase 1 plus the first CPU-overlay QA gates. That slice should make generated captions, lower thirds, callouts, chapter cards, and tracking highlights smoother, snappier, and more consistent without changing the render backend.

## Current Context

The app already has a strong graphics foundation:

- `src/lib/motion-templates.ts` defines five motion templates with visual metadata.
- `src-tauri/src/codex/app_server.rs` exposes those templates, animated primitive nodes, and GPU profile guidance to Codex.
- `src-tauri/src/graphics/animation.rs` evaluates per-node keyframes for translation, scale, opacity, delay, repeat, and yoyo.
- `src-tauri/src/render_pipeline/proposal.rs` converts captions, overlays, and GPU visuals into Rust graphics layers.
- `src-tauri/src/gpu_graphics/profile.rs` expands `hq-neon-wireframe-shader-v1` into a deterministic Rust-owned shader and primitive scene.
- `src-tauri/src/gpu_graphics/visual_qa.rs` samples GPU frames for blank output, blob dominance, edge/detail presence, and temporal motion.
- `src-tauri/src/render_pipeline/gstreamer_backend.rs` composes graphics frame sequences into WebM output through GStreamer/GES.

The important gaps are:

- Template metadata describes motion, but templates do not own canonical animated node recipes.
- Codex can still return raw overlay nodes before choosing a reusable template or preset.
- Motion is expressed as prose instead of a stable `motionPresetId` that expands to exact keyframes.
- CPU overlays do not have the same QA gate as GPU visuals.
- Text quality relies mostly on fixed boxes and font sizes; there is no auto-fit or safe-zone QA.
- The GPU profile registry has one visual look.
- Final WebM quality profiles are named, but encoder quality settings are not yet a meaningful visual quality lever.
- The review UI shows render reports, but does not make visual quality failures or sampled artifacts easy enough to inspect.

## Goals

- Make generated overlay motion deterministic, smooth, and reusable.
- Give Codex a small set of high-quality visual choices instead of open-ended raw primitives by default.
- Preserve the EDL-first contract: visuals are added only after selected source ranges exist.
- Keep Rust responsible for canonical validation, rendering, QA, and artifact metadata.
- Keep all visual layers legible over real video and phone-readable.
- Reject generic visual failures before final handoff: static slabs, oversized opaque blocks, unsafe holds, blank frames, and low-motion generated layers.
- Add enough review metadata for users and developers to understand why a visual passed or failed.

## Non-Goals

- Replacing the current Rust `tiny-skia` and `cosmic-text` CPU renderer.
- Replacing GStreamer/GES as the compositor.
- Making MP4/H.264 the default export path.
- Supporting arbitrary external animation engines, CSS/HTML capture, GLTF imports, or unconstrained shader graphs.
- Building subjective aesthetic scoring. QA should catch objective failures and obvious regressions, not decide taste.

## Chosen Approach

Use deterministic visual contracts layered on top of the existing renderer:

```text
Codex proposal
  -> EDL validation
  -> templateId / motionPresetId validation
  -> Rust-owned template recipe expansion
  -> CPU graphics render
  -> CPU visual QA
  -> GStreamer/GES composition
  -> render report with visual QA details
```

Codex should choose from approved templates and motion presets first. Raw `nodes` remain available for custom graphics, but proposal conversion should prefer and validate reusable visual contracts.

## Phase 1: Motion Presets And Template Recipes

Add a shared motion preset catalog. A preset is a named choreography contract that can be expanded into node keyframes for a layer duration and FPS.

Initial presets:

- `slide-fade-up-v1`: compact lower-third slide, fade, hold, soft exit.
- `snap-pop-v1`: fast caption scale pop with opacity and slight overshoot.
- `underline-wipe-v1`: accent rule wipe paired with text opacity.
- `metric-count-pop-v1`: metric tile count-feel pop and directional accent sweep.
- `vertical-reveal-v1`: chapter marker line wipe with text type-on feel.
- `tracking-draw-v1`: ring/line draw-on with label slide and fade.

Add template recipes for the existing five templates:

- `kinetic-lower-third-v1`
- `punchy-caption-v1`
- `metric-callout-v1`
- `chapter-card-v1`
- `tracking-highlight-v1`

Each recipe should produce a canonical `GraphicsLayer` node tree with:

- stable node ids.
- exact text fields.
- template-specific placement.
- alpha-safe colors.
- motion preset metadata.
- generated `animate.keyframes` for every moving node.

Template recipes should live in Rust for render determinism. The TypeScript template catalog can keep preview metadata and should add `motionPresetId` so UI and Codex guidance stay aligned.

## Phase 2: CPU Overlay Visual QA

Add QA for CPU-rendered overlays and captions after `render_graphics_preview`.

Initial checks:

- rendered frames exist and match the manifest dimensions.
- animated layers have first/middle/last temporal difference above a small threshold.
- visible alpha coverage is not too low.
- visible alpha coverage is not too high for non-title overlays.
- text boxes stay inside a 10% safe zone unless the template explicitly allows edge anchoring.
- caption text density is bounded by duration and box size.
- overlays do not cover more than 30% of the frame for more than 1.5 seconds unless their role is `title_card` or `transition`.

The QA report should include sampled frame paths and numeric metrics. Failures should use the existing actionable error style and include layer id, role, template id when present, motion preset id when present, and artifact directory.

## Phase 3: Additional GPU Profiles

Add Rust-owned GPU profiles only after Phase 1 and Phase 2 are stable.

Candidate profiles:

- `editorial-glass-panel-v1`: restrained glass panels, depth, crisp highlights, low center contrast.
- `kinetic-gradient-map-v1`: animated gradient map with accent linework and subtle parallax.
- `product-callout-grid-v1`: structured grid, pointer lines, and product/detail callout energy.

Each profile must have:

- a registry id.
- canonical shader/scene expansion.
- CPU fallback if practical.
- profile-specific QA thresholds.
- app-server schema alignment tests.

## Phase 4: Final Export Polish

Improve final WebM quality while respecting the existing plugin policy.

The first export improvement should stay WebM-only:

- keep draft renders fast.
- make `finalWebm` materially higher quality than `draftWebm`.
- prefer reviewed VP9 settings if plugin policy and runtime availability allow it.
- otherwise use approved VP8 settings with less aggressive speed/quality tradeoffs.
- expose chosen encoder/profile metadata in the render report.

MP4/H.264 remains policy-gated future work.

## Phase 5: Review UI

Expose visual quality details in the editor render report UI:

- layer id.
- template id.
- motion preset id.
- renderer.
- quality profile.
- QA status.
- sampled frame paths.
- actionable failure details.

This UI should remain compact and operational. It should help users inspect failed visuals and help developers debug generation regressions.

## Data Model

Add motion preset ids to the shared TypeScript template metadata:

```ts
export type MotionPresetId =
  | "slide-fade-up-v1"
  | "snap-pop-v1"
  | "underline-wipe-v1"
  | "metric-count-pop-v1"
  | "vertical-reveal-v1"
  | "tracking-draw-v1";
```

Add `motionPresetId` to `MotionTemplateDefinition` and to template-backed timeline item properties.

For Rust proposal conversion, accept `motionPresetId` on overlays. If an overlay has `templateId`, conversion should route to a Rust template recipe. If an overlay has raw `nodes`, conversion should preserve them but still validate visual metadata and timing.

## Validation Rules

- Unknown `templateId` remains invalid.
- Unknown `motionPresetId` is invalid.
- Template overlays must fit inside selected render duration.
- Template fields required by the recipe must be non-empty.
- Template-generated nodes must validate through `validate_graphics_layer`.
- Raw custom nodes remain supported but should be treated as custom graphics in reporting.
- CPU visual QA runs for all CPU graphics artifacts.
- GPU visual QA remains required for built-in GPU profiles.

## Prompt And Schema Rules

Update the Codex app-server prompt so it instructs agents to:

- choose a motion template before inventing custom nodes.
- include `templateId`, `fields`, and `motionPresetId` for template-backed overlays.
- use raw `nodes` only when no template can express the beat.
- keep custom nodes sparse and animated with concrete keyframes.
- keep GPU visuals profile-based.

The schema should allow `motionPresetId` on overlays and keep `additionalProperties: false`.

## Error Handling

Use existing actionable errors. Example:

```json
{
  "code": "PIPELINE_INPUT_INVALID",
  "path": "overlays[0].motionPresetId",
  "message": "Motion preset is unsupported.",
  "fix": "Use one of the built-in motion presets: slide-fade-up-v1, snap-pop-v1, underline-wipe-v1, metric-count-pop-v1, vertical-reveal-v1, tracking-draw-v1."
}
```

Example CPU QA failure:

```json
{
  "code": "GRAPHICS_VISUAL_QA_FAILED",
  "path": "graphics[proposal-overlay-1].visualQa",
  "message": "Overlay covers too much of the frame for too long.",
  "fix": "Use a smaller template, shorten the duration, or change the role to title_card only when the beat is intentionally full-frame."
}
```

## Testing Strategy

Use TDD for each slice:

- TypeScript tests for motion template catalog metadata and timeline item creation.
- Rust tests for motion preset parsing and generated keyframes.
- Rust tests for template recipe conversion into `GraphicsLayer`.
- Rust tests for proposal conversion using `templateId` and `motionPresetId`.
- Rust tests for unknown motion preset errors.
- Rust tests for CPU visual QA pass/fail conditions.
- App-server schema tests for overlay `motionPresetId`.
- Render-pipeline tests proving render reports include template/preset/QA details.

Use tiny fixture layers and existing render-pipeline fixture helpers. Avoid large media fixtures for the first slice.

## Rollout

The first implementation plan should build Phase 1 and the first CPU-overlay QA gates:

1. Shared TypeScript motion preset metadata.
2. Rust motion preset registry.
3. Rust template recipe expansion for the five existing templates.
4. Proposal conversion from template overlays to recipe-generated animated nodes.
5. App-server schema and prompt updates.
6. CPU visual QA for coverage and temporal motion.
7. Render report metadata for template id, motion preset id, and CPU QA status.

Later plans should handle the additional GPU profiles, final export polish, and review UI expansion.
