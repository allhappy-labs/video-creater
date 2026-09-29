# Motion v2 Design

## Summary

Motion v2 upgrades Video Creater's generated visual motion from simple translate, scale, and opacity keyframes into a richer deterministic motion system for captions, overlays, diagrams, callouts, and GPU profile layers.

The goal is not to introduce an unconstrained animation engine. The goal is to give Codex and templates a larger Rust-owned motion vocabulary that stays previewable, renderable, QA-checkable, cacheable, and compatible with the current EDL-first render pipeline.

## Current Context

The current system has a solid v1 foundation:

- `src/lib/motion-presets.ts` and `src-tauri/src/graphics/motion_presets.rs` define shared preset ids.
- `src-tauri/src/graphics/animation.rs` evaluates node-level keyframes for `x`, `y`, `scale`, `scaleX`, `scaleY`, and `opacity`.
- `src-tauri/src/graphics/templates.rs` expands built-in templates into canonical Rust graphics IR.
- `src-tauri/src/render_pipeline/proposal.rs` applies default preset animation to generated captions, overlays, and custom nodes.
- `src-tauri/src/graphics/visual_qa.rs` checks CPU-rendered overlays for coverage, temporal motion, text density, and text safe-zone placement.
- `src-tauri/src/gpu_graphics/profile.rs` owns the canonical HQ GPU visual profile.

The important limitations are:

- No rotation, skew, transform origin, blur, glow, shadow, clipping, masking, or path-trim animation.
- Text animation is whole-node only; there is no word, line, or character reveal contract.
- Draw-on effects are simulated with scale/translation instead of real line/path progress.
- Motion presets cannot express separate enter, emphasis, hold, exit, and loop segments.
- CPU visual QA can detect static frames but cannot validate motion smoothness, velocity spikes, or overshoot bounds.
- Preview metadata does not expose enough motion timing detail for the UI to show useful miniature motion diagnostics.
- GPU and CPU motion concepts are separate; they should share timing vocabulary where practical.

## Goals

- Add a richer typed animation model while keeping Rust as the source of render truth.
- Keep generated visuals deterministic and cacheable.
- Make motion smooth, snappy, and intentional without relying on vague prose.
- Support expressive but bounded effects: rotation, blur/glow, shadow, masks, path draw-on, line trim, and text reveal.
- Preserve existing v1 proposals and rendered projects through schema defaults and migration helpers.
- Let Codex choose from approved motion recipes before authoring raw keyframes.
- Make QA catch objective motion failures: static output, unsafe text, velocity spikes, excessive blur, excessive coverage, unreadable text reveal, and flickering alpha.
- Keep GStreamer/GES composition unchanged for the first Motion v2 slice.

## Non-Goals

- Replacing the Rust CPU renderer with HTML/CSS capture, Remotion, Lottie, Rive, Skia, or a game engine.
- Supporting arbitrary user scripts, arbitrary shader graphs, filesystem imports, or external animation runtimes in v2.
- Making subjective aesthetic scoring.
- Changing the EDL-first contract.
- Enabling MP4/H.264 export as part of Motion v2.
- Supporting full skeletal animation or GLTF scene animation.

## Chosen Approach

Motion v2 should extend the existing graphics IR and preset registry in phases:

```text
Codex proposal
  -> EDL validation
  -> template/profile/motion recipe validation
  -> Rust-owned recipe expansion
  -> Motion v2 keyframe evaluation
  -> CPU/GPU frame sequence rendering
  -> visual and motion QA
  -> GStreamer/GES composition
  -> render report with motion diagnostics
```

The first Motion v2 implementation should focus on CPU overlay motion because that affects captions, lower thirds, callouts, and diagrams immediately. GPU profile motion can adopt the same timing vocabulary later.

## Motion Model

Add a `MotionTransform` concept to the Rust graphics animation layer. The evaluated per-frame state should expand from the current fields to:

- `x`
- `y`
- `rotationDegrees`
- `scaleX`
- `scaleY`
- `opacity`
- `blurRadius`
- `shadowOpacity`
- `glowOpacity`
- `clipProgress`
- `pathProgress`

Add `origin` on animated nodes:

```json
{
  "origin": {
    "x": "left | center | right | number",
    "y": "top | center | bottom | number"
  }
}
```

Defaults:

- Existing v1 nodes default to center origin.
- Text and backing cards default to center origin.
- Lines and polygon draw-on effects default to their geometric path start unless overridden.

## Timing Model

Motion v2 presets should be built from named segments:

- `enter`
- `emphasis`
- `hold`
- `exit`
- `loop`

Each segment should have:

- `start`
- `end`
- `easing`
- optional `delaySeconds`
- optional `staggerSeconds`
- optional `repeat`
- optional `yoyo`

The renderer can still store keyframes, but templates and Codex should work with higher-level recipes first. Rust expands recipes into exact keyframes.

## New Motion Presets

Add v2 presets without removing v1 ids:

- `spring-pop-v2`: fast scale and opacity entry with bounded overshoot.
- `slide-rotate-settle-v2`: side entry with 2-4 degree rotation settling to zero.
- `mask-wipe-v2`: rectangular clip reveal with fade-safe edges.
- `line-draw-v2`: line/path progress from 0 to 1 with optional label delay.
- `word-pop-stagger-v2`: word-level caption reveal with short stagger and no long holds.
- `soft-depth-card-v2`: translucent backing with shadow/glow ramp and subtle parallax.
- `pulse-emphasis-v2`: short accent emphasis loop capped to one or two cycles.
- `exit-snap-v2`: quick exit with opacity and small directional movement.

Every preset must define:

- supported node kinds.
- default duration range.
- properties it animates.
- overshoot bounds.
- QA expectations.
- v1 fallback if the renderer cannot evaluate the v2 properties.

## Text Reveal

Do not make generated text arbitrary HTML. Add structured text reveal modes to `TextNode`:

```json
{
  "textReveal": {
    "mode": "whole | line | word | character",
    "staggerSeconds": 0.025,
    "order": "forward | reverse | centerOut",
    "maxRevealDurationSeconds": 0.45
  }
}
```

Rules:

- Captions default to `word` reveal only for short emphasis captions.
- Standard subtitle-style cues default to whole-node motion to preserve readability.
- Character reveal is allowed for title cards and chapter cards only.
- Text reveal must finish early enough that the viewer can read the final text before exit.

## Masks And Path Draw-On

Add bounded mask support:

- rectangular reveal masks.
- rounded-rect masks.
- line/path trim for `LineNode`.
- polygon clip progress for simple left-to-right or bottom-to-top reveals.

Do not add arbitrary vector boolean operations in v2. The renderer should support simple masks that can be validated and rasterized predictably.

## Blur, Glow, And Shadow

Add visual polish properties cautiously:

- `blurRadius` for entry/exit only, capped by dimensions.
- `shadowOpacity` and `shadowOffset` for text/card separation.
- `glowOpacity` for accent lines, callouts, and GPU-style overlays.

QA should reject:

- blur that makes text unreadable during the hold segment.
- glow that causes saturated blob dominance.
- shadow/glow coverage that pushes overlay alpha coverage over the existing limits.

## Data Model

Add v2 fields to `NodeAnimation` with serde defaults so old JSON continues to deserialize:

```rust
pub struct NodeAnimation {
    pub ease: Option<Easing>,
    pub delay_seconds: Option<f64>,
    pub repeat: Option<u32>,
    pub yoyo: Option<bool>,
    pub origin: Option<TransformOrigin>,
    pub keyframes: Vec<AnimationKeyframe>,
}

pub struct AnimationKeyframe {
    pub at: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation_degrees: Option<f64>,
    pub scale: Option<f64>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub opacity: Option<f64>,
    pub blur_radius: Option<f64>,
    pub shadow_opacity: Option<f64>,
    pub glow_opacity: Option<f64>,
    pub clip_progress: Option<f64>,
    pub path_progress: Option<f64>,
}
```

Add `MotionRecipe` separately from raw node animation:

```rust
pub struct MotionRecipe {
    pub preset_id: String,
    pub version: u32,
    pub segments: Vec<MotionSegment>,
}
```

Templates should reference recipe ids. Rust should expand recipes into node animations during template or proposal conversion.

## Renderer Changes

CPU renderer:

- Apply rotation around transform origin for rects, rounded rects, text boxes, polygons, lines, and image refs.
- Apply opacity as today.
- Apply simple blur/glow/shadow through bounded raster passes after node render.
- Apply masks before compositing the node into the layer pixmap.
- Apply path progress for line nodes by trimming polyline length.

GPU renderer:

- Keep existing profile expansion.
- Reuse segment timing names for GPU profile uniforms.
- Defer arbitrary GPU node animation until CPU v2 is stable.

## Validation

Extend `validate_graphics_layer`:

- unknown v2 fields fail only when schema version opts into strict v2 validation.
- numeric animation values must be finite.
- `rotationDegrees` should be bounded, initially -45 to 45 for overlays.
- `blurRadius` should be non-negative and capped by frame size.
- `clipProgress` and `pathProgress` must be 0 to 1.
- text reveal duration must leave readable hold time.
- v2 animation frame budget must include blur/mask cost.

Proposal validation:

- unknown Motion v2 preset ids fail actionably.
- v2 presets must be compatible with node kinds.
- Codex cannot request `character` reveal for ordinary captions.
- custom raw v2 keyframes are allowed only when no template or recipe fits the beat.

## Visual QA

Extend CPU QA with motion diagnostics:

- sample more than first/middle/last for v2 layers: at least enter peak, hold, and exit.
- compute temporal delta per segment.
- reject velocity spikes from bad keyframes.
- reject excessive overshoot beyond preset bounds.
- reject text unreadability during hold.
- reject alpha flicker where opacity alternates unintentionally.
- record `motionPresetId`, `motionRecipeVersion`, sampled frames, temporal deltas, and max coverage.

Render reports should include:

- `motionPresetId`
- `motionRecipeVersion`
- `animatedPropertyCount`
- `sampledFrames`
- `qaMetrics`
- `motionQaStatus`

## Prompt And Schema Rules

Codex prompt guidance should change from "include motion prose" to "choose a motion recipe":

- Prefer `templateId` plus `motionPresetId`.
- Use `motionRecipeId` when a template supports multiple v2 motions.
- Use raw `animate.keyframes` only for custom graphics that cannot be represented by approved recipes.
- Always include `visualTreatment`, `motion`, `safeZone`, and `avoid` for human review, even when a recipe owns exact motion.

The schema should expose:

- v1 preset ids.
- v2 preset ids.
- optional `textReveal`.
- optional `origin`.
- optional v2 keyframe properties.

## Migration And Compatibility

- Existing v1 proposals remain valid.
- Existing v1 presets continue to expand to current keyframes.
- v2 fields use serde defaults and optional TypeScript fields.
- Cache fingerprints must include new animation fields and recipe ids.
- Render reports should show whether an artifact used `motionVersion: 1` or `motionVersion: 2`.

## Implementation Phases

### Phase 1: Typed V2 Animation Fields

- Extend Rust IR and TypeScript schema.
- Add validation for bounded rotation, blur, clip, path, and glow fields.
- Add unit tests proving old v1 JSON still deserializes.

### Phase 2: Renderer Support

- Implement rotation origin.
- Implement line path progress.
- Implement rectangular clip progress.
- Implement bounded text/card shadow and glow.
- Add render tests for first, middle, and last frames.

### Phase 3: Recipe Registry

- Add Rust `MotionRecipe` registry.
- Add TypeScript catalog metadata.
- Add v2 presets listed above.
- Convert selected templates to v2 recipes.

### Phase 4: Text Reveal

- Add structured text reveal.
- Add word-level and line-level rendering expansion.
- Add QA for readable hold time.

### Phase 5: Motion QA And Report UI

- Add motion diagnostics to CPU visual QA.
- Surface motion QA metrics in render reports and the report panel.
- Add sampled frame review for enter, hold, and exit moments.

### Phase 6: GPU Profile Alignment

- Reuse segment timing names in GPU profile expansion.
- Add at least one GPU profile variant that uses the v2 timing vocabulary.

## Testing Strategy

- Rust IR serialization tests for v1 and v2 animation JSON.
- Rust validation tests for each new property bound.
- Rust animation evaluator tests for rotation, blur, clip, path progress, and origin behavior.
- Rust renderer tests that compare sampled pixel changes for each new primitive.
- Proposal conversion tests for v2 preset ids and incompatible node kinds.
- CPU visual QA tests for overshoot, flicker, unreadable hold, and velocity spikes.
- TypeScript catalog tests for v2 preset metadata.
- Report UI tests for motion QA metrics and sampled frames.
- Render-pipeline tests proving cache fingerprints change when v2 recipe fields change.

## Risks

- Blur/glow can be expensive if implemented naively. Keep caps strict and include frame-budget validation.
- Text reveal can reduce comprehension if overused. Gate it by role and duration.
- Rotation and masks can create bounding-box surprises. Transform-origin defaults and QA must be explicit.
- Too many presets can make Codex choice worse. Keep the first v2 catalog small and opinionated.
- Tiny render fixtures can fail real QA thresholds. Fixtures should use copy and dimensions that represent the intended visual contract.

## Acceptance Criteria

- Existing v1 visual proposals continue to render.
- At least three v2 presets render through the CPU graphics path.
- V2 animation fields are validated and included in cache fingerprints.
- Text reveal supports at least whole, line, and word modes.
- QA rejects static, unsafe, overdense, flickering, and excessive-overshoot v2 visuals.
- Render reports expose motion version, preset id, sampled frames, and QA metrics.
- App-server schema and prompt list v2 presets and forbid unconstrained custom animation by default.

## Spec Self-Review

- No placeholders or unresolved TODOs remain.
- The spec keeps Rust as canonical renderer and validator.
- The scope is phased so Motion v2 can ship incrementally.
- The design preserves EDL-first generation and existing v1 proposal compatibility.
