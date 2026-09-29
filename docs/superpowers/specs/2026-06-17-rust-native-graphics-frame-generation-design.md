# Rust Native Graphics Frame Generation Design

## Summary

Migrate Video Creater graphics and frame generation from the current Node/SVG demo path into a Rust-owned graphics engine while keeping `ffmpeg` as the v1 video encoder and compositor.

The new renderer is built around an agent-friendly declarative graphics IR. Agents, built-in templates, and future user-created templates all author the same validated data model. Rust validates the IR, resolves approved project assets, lays out text and images, rasterizes transparent RGBA frames, writes render manifests, and hands those assets to the existing `ffmpeg` render path.

The selected v1 rendering stack is:

- `cosmic-text` for text shaping, wrapping, measurement, and glyph rasterization.
- `tiny-skia` for 2D rasterization, vector shapes, strokes, gradients, masks, alpha, and compositing.
- `png` for transparent RGBA frame output.
- `ffmpeg` CLI for v1 video composition and encoding.

This keeps the hard part that agents need to control in Rust and leaves full media encoding replacement as a later backend swap.

## Goals

- Replace Node-based graphics/frame generation for the first renderable graphics path.
- Let agents create custom graphics and templates from validated primitives, not only select built-in templates.
- Support `imageRef` from the first version.
- Keep the graphics contract declarative, bounded, deterministic, and easy for AI agents to write.
- Preserve Rust ownership of validation, artifact metadata, timing, safe zones, logs, and actionable errors.
- Keep `ffmpeg` for v1 video encoding and compositing, but design the renderer output so a later backend can consume the same frames and manifests.
- Produce concise structured errors that agents can immediately fix.

## Non-Goals

- Replacing `ffmpeg` in the first implementation.
- Porting browser layout, CSS, HTML canvas, or HyperFrames itself into Rust.
- Allowing arbitrary executable graphics code from agents.
- Allowing remote images, arbitrary filesystem reads, or unvalidated asset paths.
- Building a full public template marketplace.
- Matching every possible SVG, CSS, or Skia feature.

## Current State

The app already has the right high-level boundaries:

- Rust owns one-click EDL generation, caption generation, proposal validation, render-plan construction, and template layer extraction.
- TypeScript owns a built-in motion template catalog and timeline insertion helpers.
- Codex proposals can reference known motion template IDs with fields and required visual metadata.

The missing piece is the production graphics renderer. The current concrete graphics frame generation exists in `scripts/render-codex-funny-draft.mjs`: it builds SVG states, rasterizes them with `rsvg-convert`, creates a transparent qtrle overlay movie with `ffmpeg`, then composites that over the rough cut. That script is useful as a migration target, not as the future architecture.

## Architecture

The Rust renderer introduces a new module boundary:

```text
Timeline items / Codex proposal / built-in templates
  -> graphics IR
  -> validation and asset resolution
  -> layout plan
  -> RGBA frames and manifest
  -> VideoRenderBackend
  -> ffmpeg CLI in v1
```

The graphics engine does not know whether `ffmpeg`, GStreamer, OpenH264, or another backend will consume its output. It produces neutral graphics artifacts:

- transparent PNG frame sequence or state frame sequence
- manifest with dimensions, fps, alpha, timing, checksums, event ids, and source layer ids
- optional preview stills for UI review

The v1 `FfmpegCliBackend` consumes those artifacts and builds the existing overlay/composition command.

## Agent Authoring Model

Agents create graphics by emitting a declarative graphics IR. Built-in templates and agent-created templates use the same IR.

Agents may create:

- one-off graphics layers for a timeline beat
- parameterized templates that can be reused
- template instances with field values
- image-backed composites that reference approved project assets

Agents may not create:

- Rust, JavaScript, shader, shell, or template-execution code
- references to remote URLs
- direct filesystem paths outside approved project asset registries
- unbounded frame counts or dimensions
- graphics without timing, safe-zone, and visual metadata

## Graphics IR

The IR is a versioned data model, serialized as JSON and represented by typed Rust structs.

Top-level shape:

```json
{
  "schemaVersion": 1,
  "id": "agent.metric-pop.v1",
  "role": "overlay",
  "timelineStart": 4.2,
  "durationSeconds": 2.4,
  "dimensions": { "width": 1920, "height": 1080 },
  "fps": 30,
  "alpha": true,
  "sourceBeat": "Introduce the metric without covering the speaker.",
  "visualTreatment": "floating metric tile with translucent backing",
  "motion": "scale pop, accent sweep, short hold, fade",
  "safeZone": "keep essential text inside 10% margins",
  "avoid": "covering faces, opaque caption slabs, tiny text",
  "nodes": [
    {
      "id": "headline",
      "type": "text",
      "text": "42%",
      "box": { "x": 120, "y": 160, "width": 420, "height": 120 },
      "fontSize": 72,
      "fontWeight": 800,
      "align": "center",
      "fill": "#ffffff"
    }
  ]
}
```

Required top-level fields:

- `schemaVersion`
- `id`
- `role`
- `timelineStart`
- `durationSeconds`
- `dimensions`
- `fps`
- `alpha`
- `sourceBeat`
- `visualTreatment`
- `motion`
- `safeZone`
- `avoid`
- `nodes`

Allowed `role` values:

- `caption`
- `overlay`
- `lower_third`
- `title_card`
- `diagram`
- `transition`

## Primitive Vocabulary

The v1 primitive set should be small, expressive, and stable.

### `group`

Groups child nodes and applies shared transform, opacity, clipping, and blend settings.

Fields:

- `id`
- `type: "group"`
- `children`
- `transform`
- `opacity`
- `clip`
- `blendMode`

### `text`

Draws shaped, wrapped, fitted text.

Fields:

- `id`
- `type: "text"`
- `text` or `param`
- `box`
- `fontSize`
- `fontWeight`
- `align`
- `verticalAlign`
- `fill`
- `stroke`
- `shadow`
- `maxLines`
- `fit`

Rules:

- Text must be non-empty after trimming.
- Text must fit its box and safe zone unless `fit` allows deterministic font-size reduction.
- The renderer reports overflow as an actionable validation error.
- Minimum readable size is enforced per output dimensions.

### `rect` and `roundedRect`

Draws solid, translucent, or gradient rectangles.

Fields:

- `id`
- `type`
- `box`
- `radius`
- `fill`
- `stroke`
- `shadow`

### `line`

Draws accent rules, pointer lines, and connector lines.

Fields:

- `id`
- `type: "line"`
- `points`
- `stroke`
- `cap`
- `dash`

### `path`

Draws a restricted path for custom shapes.

Fields:

- `id`
- `type: "path"`
- `commands`
- `fill`
- `stroke`

Rules:

- Commands are limited to `moveTo`, `lineTo`, `quadTo`, `cubicTo`, and `close`.
- Path bounds must be finite.

### `polygon` and `circle`

Draws common non-rectangular shapes without requiring raw path commands.

Fields:

- `id`
- `type`
- `points` or `center` and `radius`
- `fill`
- `stroke`

### `imageRef`

Draws an approved project or bundled image asset.

Fields:

- `id`
- `type: "imageRef"`
- `assetId`
- `box`
- `fit`
- `opacity`
- `clip`
- `tint`

Allowed `fit` values:

- `contain`
- `cover`
- `stretch`
- `none`

Allowed asset sources in v1:

- imported project media stills registered as image assets
- generated project artifacts registered by Rust
- bundled renderer/template assets
- renderer-produced preview or intermediate assets

Disallowed asset sources in v1:

- remote URLs
- absolute filesystem paths from agent output
- parent-directory relative paths
- files not present in the project asset registry

Validation rules:

- `assetId` must resolve before rendering.
- Image dimensions must be known after decode.
- The image must fit inside finite bounds.
- Large images may be downscaled by Rust before compositing.
- Missing, unsupported, or unreadable images return structured errors.

## Animation Model

The v1 animation model is timeline-driven and deterministic. Each node can define optional `enter`, `hold`, and `exit` animation blocks.

Allowed animation properties:

- opacity
- translate
- scale
- rotate
- stroke reveal
- clip reveal

Allowed curves:

- linear
- easeOut
- easeInOut
- step

Rules:

- Animation times are relative to the layer duration.
- The renderer samples frames at the layer fps.
- Frame counts are bounded by duration and fps limits.
- Static state rendering is allowed for v1 when a node has no animation.

## Templates

A template is a parameterized IR document. Built-ins and agent-authored templates share the same schema. A template stores reusable scene structure and parameter rules; a template instance supplies timing, dimensions, fps, and required visual metadata so expansion always produces a complete graphics IR.

Example:

```json
{
  "schemaVersion": 1,
  "kind": "template",
  "id": "agent.metric-pop.v1",
  "params": [
    { "name": "headline", "required": true },
    { "name": "subline", "required": true },
    { "name": "badgeImage", "required": false, "type": "imageRef" }
  ],
  "scene": {
    "role": "overlay",
    "nodes": [
      { "type": "roundedRect", "id": "backing" },
      { "type": "text", "id": "headline", "param": "headline" },
      { "type": "text", "id": "subline", "param": "subline" },
      { "type": "imageRef", "id": "badge", "param": "badgeImage" }
    ]
  }
}
```

Template validation rules:

- Template IDs are stable and versioned.
- Required params must be supplied by instances.
- Param values are validated after substitution.
- Template expansion produces ordinary graphics IR.
- Expanded IR is validated before render.

## Error Contract

Every graphics and video generation failure must return concise actionable errors. This applies to agent-authored graphics, built-in templates, timeline conversion, asset resolution, frame rendering, and video render planning.

Error shape:

```json
{
  "code": "GRAPHICS_TEXT_OVERFLOW",
  "path": "layers[2].nodes[4]",
  "message": "Text does not fit inside safe zone.",
  "fix": "Reduce fontSize, shorten text, or increase box width."
}
```

Required fields:

- `code`: stable machine-readable enum string
- `path`: precise path to the bad object or render stage
- `message`: concise human-readable problem
- `fix`: direct repair instruction for an agent or user

Optional fields:

- `details`: compact structured values, such as actual width, max width, or duration
- `cause`: lower-level error code when wrapping IO, decode, or backend failures

Error message rules:

- Prefer one sentence.
- Do not include long stack traces in user-facing messages.
- Include exact numeric constraints when helpful.
- Avoid vague failures such as "render failed" without a code and fix.
- Use the same error shape for validation and runtime failures.

Initial error codes:

- `GRAPHICS_SCHEMA_UNSUPPORTED`
- `GRAPHICS_EMPTY_NODE_ID`
- `GRAPHICS_UNSUPPORTED_PRIMITIVE`
- `GRAPHICS_INVALID_TIMING`
- `GRAPHICS_INVALID_DIMENSIONS`
- `GRAPHICS_TEXT_EMPTY`
- `GRAPHICS_TEXT_OVERFLOW`
- `GRAPHICS_TEXT_UNREADABLE`
- `GRAPHICS_NODE_OUT_OF_SAFE_ZONE`
- `GRAPHICS_FRAME_COVERAGE_EXCEEDED`
- `GRAPHICS_IMAGE_REF_MISSING`
- `GRAPHICS_IMAGE_REF_UNAUTHORIZED`
- `GRAPHICS_IMAGE_DECODE_FAILED`
- `GRAPHICS_TEMPLATE_PARAM_MISSING`
- `GRAPHICS_TEMPLATE_PARAM_INVALID`
- `GRAPHICS_RENDER_FAILED`
- `RENDER_OVERLAY_OUT_OF_RANGE`
- `RENDER_FRAME_SEQUENCE_EMPTY`
- `RENDER_BACKEND_UNAVAILABLE`
- `RENDER_BACKEND_FAILED`

## Validation

Validation runs before frame generation and again before handing artifacts to the video backend.

Graphics validation checks:

- schema version is supported
- all ids are non-empty and unique within a scene
- dimensions, fps, start, and duration are finite and positive
- timeline start is non-negative
- node bounds are finite
- top-level visual metadata is non-empty
- node primitives are supported
- text is non-empty and readable
- text layout fits within safe zone or deterministic fitting succeeds
- `imageRef` resolves to approved assets only
- node coverage does not violate quality gates
- animation times fit within the layer duration
- frame count is bounded

Video/render validation checks:

- overlay event timing fits inside timeline duration
- generated frame sequence is non-empty
- manifest dimensions and fps match render settings
- alpha is present for overlay assets
- artifact paths stay inside project generated directories
- downstream backend receives complete inputs

## Rendering Artifacts

The renderer writes an artifact directory per graphics layer or flattened overlay stack.

Example:

```text
generated/graphics/
  layer-template-1/
    manifest.json
    preview.png
    frames/
      frame-000000.png
      frame-000001.png
```

Manifest shape:

```json
{
  "schemaVersion": 1,
  "artifactId": "graphics:layer-template-1",
  "kind": "rgbaFrameSequence",
  "dimensions": { "width": 1920, "height": 1080 },
  "fps": 30,
  "durationSeconds": 2.4,
  "alpha": true,
  "frameCount": 72,
  "framesPattern": "frames/frame-%06d.png",
  "previewPath": "preview.png",
  "sourceLayerIds": ["template-1"],
  "checksums": {}
}
```

The manifest is the contract between graphics generation and video composition.

## Video Backend Boundary

Introduce a narrow backend boundary so `ffmpeg` can be replaced later without changing the graphics IR.

Conceptual trait:

```rust
pub trait VideoRenderBackend {
    fn build_render_command(&self, plan: &VideoRenderPlan) -> Result<BackendCommand, RenderError>;
    fn validate_inputs(&self, plan: &VideoRenderPlan) -> Result<(), Vec<ActionableError>>;
}
```

V1 backend:

- `FfmpegCliBackend`
- consumes timeline clips and graphics manifests
- builds the current trim/concat/composite command shape
- writes logs and backend errors using the shared actionable error contract

Future backend options:

- GStreamer for a full multimedia pipeline through Rust bindings and native plugins.
- OpenH264 for Rust-owned H.264 encode/decode paths.
- rav1e for AV1 export.
- x264 bindings only if native x264 packaging is acceptable.

The graphics engine should not depend on any of these directly.

## Data Flow

### Built-In Template Render

1. Timeline overlay item references a known template ID.
2. Rust extracts `TemplateRenderLayer`.
3. Rust expands the template into graphics IR.
4. Rust validates IR and resolves image refs.
5. Rust renders transparent frames and manifest.
6. `FfmpegCliBackend` composites the manifest over the primary cut.
7. Render review validates output duration, streams, overlay timing, artifacts, and logs.

### Agent-Created Custom Graphic

1. Codex returns a structured graphics IR or template definition in a proposal.
2. Rust validates the proposal schema, EDL, and timing first.
3. Rust validates the graphics IR.
4. The UI can preview the proposed graphic and show actionable errors.
5. Accepted graphics become generated timeline items.
6. Render uses the same frame generation and backend flow.

### `imageRef` Resolution

1. IR references `assetId`.
2. Rust checks the project asset registry or bundled asset table.
3. Rust resolves to an allowed relative path.
4. Rust decodes and normalizes image dimensions.
5. The renderer composites the image according to `fit`, `clip`, and opacity.

## Migration Plan Shape

The first implementation plan should target the smallest useful vertical slice:

1. Add Rust graphics module with IR structs, actionable error type, and validation.
2. Add asset resolver with `imageRef` support for project/bundled assets.
3. Add template-to-IR adapter for existing motion templates.
4. Add renderer that can draw text, rounded rectangles, lines, and image refs into PNG frames.
5. Add manifest writer.
6. Add tests for valid built-in template render planning, invalid timing, missing image refs, text overflow, and concise errors.
7. Replace the Node demo/e2e frame generation path with the Rust graphics artifact path.
8. Keep production `ffmpeg` command construction but extend it to consume graphics manifests.

## Testing Strategy

Rust unit tests:

- IR schema validation accepts a minimal valid overlay.
- unsupported primitives return `GRAPHICS_UNSUPPORTED_PRIMITIVE`.
- missing `imageRef` returns `GRAPHICS_IMAGE_REF_MISSING`.
- unauthorized image path returns `GRAPHICS_IMAGE_REF_UNAUTHORIZED`.
- text overflow returns `GRAPHICS_TEXT_OVERFLOW` with a concise fix.
- template expansion validates required params.
- timeline overlay timing beyond duration returns `RENDER_OVERLAY_OUT_OF_RANGE`.

Rust render tests:

- rendering a valid lower-third IR writes a non-empty transparent PNG preview.
- rendering with `imageRef` composites the approved image into the output frame.
- generated manifest has dimensions, fps, frame count, alpha, and relative frame paths.
- repeated render of the same IR produces stable manifest metadata.

Integration tests:

- Node demo render script path is replaced or bypassed by Rust-generated graphics artifacts.
- `ffmpeg` can composite a Rust-generated transparent graphics sequence over a tiny video fixture.
- final MP4 has expected duration, video stream, audio stream when expected, and logged artifact paths.

Error tests:

- every graphics and render error serializes to `code`, `path`, `message`, and `fix`.
- messages stay concise enough for agent loops.
- lower-level IO/decode/backend errors are wrapped with actionable codes.

## Rollout

1. Land the Rust graphics IR and validation without changing production render behavior.
2. Add renderer output and manifest generation behind tests.
3. Wire existing template render layers into the graphics renderer.
4. Move the Node demo/e2e graphics generation to Rust output.
5. Extend the real render plan to consume graphics manifests.
6. Add UI preview and proposal error surfacing for agent-authored graphics.
7. Later, evaluate a non-ffmpeg `VideoRenderBackend` once the graphics artifact boundary is stable.

## Open Implementation Decisions

- Exact location of the project asset registry for `imageRef` lookup.
- Whether v1 writes every frame or writes only state frames plus concat durations for static intervals.
- Whether text fitting first shrinks font size, wraps, or rejects based on per-template policy.
- Which image formats are accepted in v1.
- Whether generated graphics are stored per layer or flattened into one overlay stack before `ffmpeg`.

These are implementation details, not blockers for the approved architecture.
