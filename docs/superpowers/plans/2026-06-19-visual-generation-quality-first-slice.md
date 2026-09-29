# Visual Generation Quality First Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing generated overlay templates deterministic, animated, QA-checked visual assets instead of static template metadata.

**Architecture:** Add a small shared motion preset catalog, use it in TypeScript template metadata, and mirror the preset ids in Rust. Extend the existing Rust template adapter in `src-tauri/src/graphics/templates.rs` so template-backed proposal overlays expand into animated `GraphicsLayer` nodes. Add CPU visual QA after CPU graphics rendering and surface template, preset, and QA metadata in render reports.

**Tech Stack:** TypeScript, Vitest, Rust, serde, existing `graphics`, `render_pipeline`, `codex`, and GStreamer/GES modules.

---

## File Structure

- Create `src/lib/motion-presets.ts`: TypeScript motion preset ids and catalog metadata for UI/Codex alignment.
- Modify `src/lib/motion-templates.ts`: add `motionPresetId` to every built-in template and to created template timeline items.
- Modify `src/lib/motion-templates.test.ts`: assert the preset catalog and template/preset metadata.
- Create `src-tauri/src/graphics/motion_presets.rs`: Rust preset ids, parser, and helper constructors for node animations.
- Modify `src-tauri/src/graphics/mod.rs`: expose `motion_presets`.
- Modify `src-tauri/src/graphics/templates.rs`: attach deterministic keyframe animation to each existing template recipe.
- Modify `src-tauri/tests/graphics.rs`: test preset parsing and animated template output.
- Modify `src-tauri/src/render_pipeline/proposal.rs`: accept/validate `motionPresetId`, route template-backed overlays through Rust template recipes, run CPU visual QA, and include metadata in rendered graphics records.
- Modify `src-tauri/src/render_pipeline/report.rs`: add `templateId`, `motionPresetId`, and `visualQaStatus` fields to graphics report output.
- Modify `src-tauri/tests/render_pipeline.rs`: cover template recipe conversion, unknown motion preset rejection, CPU QA behavior, and render report metadata.
- Modify `src-tauri/src/codex/app_server.rs`: add `motionPresetId` to overlay schema and prompt guidance.
- Modify `src-tauri/tests/codex_app_server.rs`: assert schema/prompt exposes motion presets.

## Task 1: Shared TypeScript Motion Presets

**Files:**
- Create: `src/lib/motion-presets.ts`
- Modify: `src/lib/motion-templates.ts`
- Modify: `src/lib/motion-templates.test.ts`

- [ ] **Step 1: Write failing TypeScript catalog tests**

Add this import to `src/lib/motion-templates.test.ts`:

```ts
import { motionPresetCatalog } from "./motion-presets";
```

Add this test inside `describe("motion template catalog", ...)`:

```ts
  it("assigns deterministic motion presets to every built-in template", () => {
    expect(motionPresetCatalog.map((preset) => preset.id)).toEqual([
      "slide-fade-up-v1",
      "snap-pop-v1",
      "underline-wipe-v1",
      "metric-count-pop-v1",
      "vertical-reveal-v1",
      "tracking-draw-v1",
    ]);
    expect(
      motionTemplateCatalog.map((template) => [template.id, template.motionPresetId]),
    ).toEqual([
      ["kinetic-lower-third-v1", "slide-fade-up-v1"],
      ["punchy-caption-v1", "snap-pop-v1"],
      ["metric-callout-v1", "metric-count-pop-v1"],
      ["chapter-card-v1", "vertical-reveal-v1"],
      ["tracking-highlight-v1", "tracking-draw-v1"],
    ]);
  });
```

Update the existing `creates a canonical overlay item...` expectation to include:

```ts
        motionPresetId: kineticLowerThirdTemplate.motionPresetId,
```

Update the `creates canonical overlay items for every built-in template` test to include:

```ts
      expect(item.properties.motionPresetId).toBe(template.motionPresetId);
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
rtk pnpm test src/lib/motion-templates.test.ts
```

Expected: FAIL because `src/lib/motion-presets.ts` and `motionPresetId` do not exist.

- [ ] **Step 3: Add the TypeScript preset catalog and template metadata**

Create `src/lib/motion-presets.ts`:

```ts
export type MotionPresetId =
  | "slide-fade-up-v1"
  | "snap-pop-v1"
  | "underline-wipe-v1"
  | "metric-count-pop-v1"
  | "vertical-reveal-v1"
  | "tracking-draw-v1";

export interface MotionPresetDefinition {
  id: MotionPresetId;
  label: string;
  description: string;
}

export const motionPresetCatalog: MotionPresetDefinition[] = [
  {
    id: "slide-fade-up-v1",
    label: "Slide Fade Up",
    description: "Lower-third entry with upward slide, quick fade, hold, and soft exit.",
  },
  {
    id: "snap-pop-v1",
    label: "Snap Pop",
    description: "Caption scale pop with a small overshoot and quick snap fade.",
  },
  {
    id: "underline-wipe-v1",
    label: "Underline Wipe",
    description: "Accent rule reveal paired with text opacity for caption emphasis.",
  },
  {
    id: "metric-count-pop-v1",
    label: "Metric Count Pop",
    description: "Metric tile pop with directional accent sweep and crisp hold.",
  },
  {
    id: "vertical-reveal-v1",
    label: "Vertical Reveal",
    description: "Chapter marker line wipe with text reveal and mask-like exit.",
  },
  {
    id: "tracking-draw-v1",
    label: "Tracking Draw",
    description: "Highlight ring draw-on with label slide and quick fade.",
  },
];
```

Modify `src/lib/motion-templates.ts`:

```ts
import type { MotionPresetId } from "./motion-presets";
```

Add `motionPresetId: MotionPresetId;` to `MotionTemplateDefinition`, then set each template:

```ts
motionPresetId: "slide-fade-up-v1",
motionPresetId: "snap-pop-v1",
motionPresetId: "metric-count-pop-v1",
motionPresetId: "vertical-reveal-v1",
motionPresetId: "tracking-draw-v1",
```

Add this property in `createTemplateOverlayItem` under `properties`:

```ts
motionPresetId: template.motionPresetId,
```

- [ ] **Step 4: Verify TypeScript tests pass**

Run:

```bash
rtk pnpm test src/lib/motion-templates.test.ts
```

Expected: PASS.

## Task 2: Rust Motion Preset Registry

**Files:**
- Create: `src-tauri/src/graphics/motion_presets.rs`
- Modify: `src-tauri/src/graphics/mod.rs`
- Modify: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing Rust preset tests**

Add imports to `src-tauri/tests/graphics.rs`:

```rust
use video_creater_lib::graphics::motion_presets::{
    motion_preset_animation, parse_motion_preset_id, MotionPresetId,
};
```

Add this test:

```rust
#[test]
fn parses_motion_preset_ids_and_builds_keyframes() {
    assert_eq!(
        parse_motion_preset_id("slide-fade-up-v1"),
        Some(MotionPresetId::SlideFadeUpV1)
    );
    assert_eq!(parse_motion_preset_id("missing-preset"), None);

    let animation = motion_preset_animation(MotionPresetId::SnapPopV1, "headline");
    assert_eq!(animation.ease, Some(Easing::OutBack));
    assert!(animation.keyframes.len() >= 3);
    assert_eq!(animation.keyframes[0].opacity, Some(0.0));
    assert_eq!(animation.keyframes.last().and_then(|frame| frame.opacity), Some(0.0));
}
```

- [ ] **Step 2: Run the failing Rust test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics parses_motion_preset_ids_and_builds_keyframes
```

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Add the Rust preset module**

Create `src-tauri/src/graphics/motion_presets.rs`:

```rust
use super::ir::{AnimationKeyframe, Easing, NodeAnimation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPresetId {
    SlideFadeUpV1,
    SnapPopV1,
    UnderlineWipeV1,
    MetricCountPopV1,
    VerticalRevealV1,
    TrackingDrawV1,
}

pub const SUPPORTED_MOTION_PRESETS: &[&str] = &[
    "slide-fade-up-v1",
    "snap-pop-v1",
    "underline-wipe-v1",
    "metric-count-pop-v1",
    "vertical-reveal-v1",
    "tracking-draw-v1",
];

pub fn parse_motion_preset_id(value: &str) -> Option<MotionPresetId> {
    match value.trim() {
        "slide-fade-up-v1" => Some(MotionPresetId::SlideFadeUpV1),
        "snap-pop-v1" => Some(MotionPresetId::SnapPopV1),
        "underline-wipe-v1" => Some(MotionPresetId::UnderlineWipeV1),
        "metric-count-pop-v1" => Some(MotionPresetId::MetricCountPopV1),
        "vertical-reveal-v1" => Some(MotionPresetId::VerticalRevealV1),
        "tracking-draw-v1" => Some(MotionPresetId::TrackingDrawV1),
        _ => None,
    }
}

pub fn motion_preset_animation(preset: MotionPresetId, node_id: &str) -> NodeAnimation {
    match preset {
        MotionPresetId::SlideFadeUpV1 => slide_fade_up(node_id),
        MotionPresetId::SnapPopV1 => snap_pop(node_id),
        MotionPresetId::UnderlineWipeV1 => underline_wipe(node_id),
        MotionPresetId::MetricCountPopV1 => metric_count_pop(node_id),
        MotionPresetId::VerticalRevealV1 => vertical_reveal(node_id),
        MotionPresetId::TrackingDrawV1 => tracking_draw(node_id),
    }
}

fn slide_fade_up(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("subline") { 0.08 } else { 0.0 };
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        keyframes: vec![
            keyframe(0.0, Some(-34.0), Some(26.0), Some(0.98), Some(0.0)),
            keyframe(0.16, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.82, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(-10.0), Some(-10.0), Some(1.0), Some(0.0)),
        ],
    }
}

fn snap_pop(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("accent") { 0.06 } else { 0.0 };
    NodeAnimation {
        ease: Some(Easing::OutBack),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        keyframes: vec![
            keyframe(0.0, Some(0.0), Some(18.0), Some(0.72), Some(0.0)),
            keyframe(0.12, Some(0.0), Some(0.0), Some(1.06), Some(1.0)),
            keyframe(0.22, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.84, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(0.0), Some(-8.0), Some(0.98), Some(0.0)),
        ],
    }
}

fn underline_wipe(node_id: &str) -> NodeAnimation {
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: Some(if node_id.contains("accent") { 0.08 } else { 0.0 }),
        repeat: None,
        yoyo: None,
        keyframes: vec![
            keyframe(0.0, Some(-80.0), Some(0.0), Some(0.12), Some(0.0)),
            keyframe(0.18, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.86, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(36.0), Some(0.0), Some(0.92), Some(0.0)),
        ],
    }
}

fn metric_count_pop(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("subline") { 0.10 } else { 0.0 };
    NodeAnimation {
        ease: Some(Easing::OutBack),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        keyframes: vec![
            keyframe(0.0, Some(42.0), Some(-18.0), Some(0.86), Some(0.0)),
            keyframe(0.18, Some(0.0), Some(0.0), Some(1.04), Some(1.0)),
            keyframe(0.30, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.84, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(28.0), Some(8.0), Some(0.98), Some(0.0)),
        ],
    }
}

fn vertical_reveal(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("subline") { 0.12 } else { 0.0 };
    NodeAnimation {
        ease: Some(Easing::InOutCubic),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        keyframes: vec![
            keyframe(0.0, Some(-28.0), Some(18.0), Some(0.96), Some(0.0)),
            keyframe(0.22, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.82, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(0.0), Some(-20.0), Some(1.0), Some(0.0)),
        ],
    }
}

fn tracking_draw(node_id: &str) -> NodeAnimation {
    let delay = if node_id.contains("label") || node_id.contains("headline") {
        0.10
    } else {
        0.0
    };
    NodeAnimation {
        ease: Some(Easing::OutCubic),
        delay_seconds: Some(delay),
        repeat: None,
        yoyo: None,
        keyframes: vec![
            keyframe(0.0, Some(-16.0), Some(10.0), Some(0.88), Some(0.0)),
            keyframe(0.20, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(0.78, Some(0.0), Some(0.0), Some(1.0), Some(1.0)),
            keyframe(1.0, Some(8.0), Some(-8.0), Some(0.98), Some(0.0)),
        ],
    }
}

fn keyframe(
    at: f64,
    x: Option<f64>,
    y: Option<f64>,
    scale: Option<f64>,
    opacity: Option<f64>,
) -> AnimationKeyframe {
    AnimationKeyframe {
        at,
        x,
        y,
        scale,
        scale_x: None,
        scale_y: None,
        opacity,
    }
}
```

Add to `src-tauri/src/graphics/mod.rs`:

```rust
pub mod motion_presets;
```

- [ ] **Step 4: Verify Rust preset test passes**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics parses_motion_preset_ids_and_builds_keyframes
```

Expected: PASS.

## Task 3: Animated Rust Template Recipes

**Files:**
- Modify: `src-tauri/src/graphics/templates.rs`
- Modify: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing animated template tests**

Add this test to `src-tauri/tests/graphics.rs`:

```rust
#[test]
fn template_recipes_attach_motion_preset_animations() {
    for (template_id, expected_animated_node) in [
        ("kinetic-lower-third-v1", "backing"),
        ("punchy-caption-v1", "headline"),
        ("metric-callout-v1", "headline"),
        ("chapter-card-v1", "headline"),
        ("tracking-highlight-v1", "highlight-frame"),
    ] {
        let layer = template_render_layer(template_id);

        let graphics = template_layer_to_graphics_ir(
            &layer,
            Dimensions {
                width: 1920,
                height: 1080,
            },
            30.0,
        )
        .expect("template should expand");

        let node = graphics
            .nodes
            .iter()
            .find(|node| node.id() == expected_animated_node)
            .expect("expected animated node");
        let animation = node.animation().expect("template node should animate");
        assert!(animation.keyframes.len() >= 3);
        assert_eq!(animation.keyframes[0].opacity, Some(0.0));
    }
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics template_recipes_attach_motion_preset_animations
```

Expected: FAIL because template nodes are static.

- [ ] **Step 3: Attach preset animations in `templates.rs`**

Import presets:

```rust
use crate::graphics::motion_presets::{motion_preset_animation, MotionPresetId};
```

For every node currently created with `animate: None`, set a template-specific animation:

```rust
animate: Some(motion_preset_animation(MotionPresetId::SlideFadeUpV1, "backing")),
animate: Some(motion_preset_animation(MotionPresetId::SnapPopV1, "headline")),
animate: Some(motion_preset_animation(MotionPresetId::MetricCountPopV1, "headline")),
animate: Some(motion_preset_animation(MotionPresetId::VerticalRevealV1, "headline")),
animate: Some(motion_preset_animation(MotionPresetId::TrackingDrawV1, "highlight-frame")),
```

Use the same preset for all nodes in a template unless a node has a better paired preset, for example the `punchy-caption-v1` accent can use `UnderlineWipeV1`.

- [ ] **Step 4: Verify animated template tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics template_recipes_attach_motion_preset_animations expands_every_renderer_template_catalog_entry
```

Expected: PASS.

## Task 4: Proposal Schema And Motion Preset Validation

**Files:**
- Modify: `src-tauri/src/codex/app_server.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/tests/codex_app_server.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing schema and proposal tests**

Add to `src-tauri/tests/codex_app_server.rs`:

```rust
#[test]
fn turn_request_lists_motion_presets_and_overlay_schema_allows_motion_preset_id() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");

    assert!(text.contains("Available motion presets"));
    assert!(text.contains("slide-fade-up-v1"));
    assert!(text.contains("tracking-draw-v1"));
    assert_eq!(
        turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
            ["motionPresetId"],
        json!({ "type": "string", "enum": [
            "slide-fade-up-v1",
            "snap-pop-v1",
            "underline-wipe-v1",
            "metric-count-pop-v1",
            "vertical-reveal-v1",
            "tracking-draw-v1"
        ]})
    );
}
```

Add to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn proposal_visuals_reject_unknown_motion_preset_id() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays[0]["motionPresetId"] = serde_json::json!("bad-preset");

    let errors = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect_err("unknown motion preset should fail");

    assert_eq!(errors[0].path, "overlays[0].motionPresetId");
    assert!(errors[0].fix.contains("slide-fade-up-v1"));
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server motion_presets
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_visuals_reject_unknown_motion_preset_id
```

Expected: FAIL because schema and validation do not know `motionPresetId`.

- [ ] **Step 3: Add prompt/schema support**

In `src-tauri/src/codex/app_server.rs`, add a new prompt section after `Available motion templates`:

```text
Available motion presets:
- slide-fade-up-v1: lower-third slide/fade entry with soft exit.
- snap-pop-v1: caption scale pop with overshoot and snap fade.
- underline-wipe-v1: accent underline wipe paired with text reveal.
- metric-count-pop-v1: metric tile pop with directional accent sweep.
- vertical-reveal-v1: chapter marker reveal with text type-on feel.
- tracking-draw-v1: highlight ring draw-on with label slide.

Template guidance:
- Prefer templateId + fields + motionPresetId for overlays before custom nodes.
- Use raw nodes only when no listed template can express the visual beat.
```

Add `motionPresetId` to overlay schema properties:

```rust
"motionPresetId": {
    "type": "string",
    "enum": [
        "slide-fade-up-v1",
        "snap-pop-v1",
        "underline-wipe-v1",
        "metric-count-pop-v1",
        "vertical-reveal-v1",
        "tracking-draw-v1"
    ]
},
```

In `src-tauri/src/render_pipeline/proposal.rs`, validate optional overlay `motionPresetId` with `parse_motion_preset_id`; return `PipelineInputInvalid` at `overlays[index].motionPresetId` on unknown ids.

- [ ] **Step 4: Verify schema and proposal tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server motion_presets
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_visuals_reject_unknown_motion_preset_id
```

Expected: PASS.

## Task 5: CPU Graphics Visual QA

**Files:**
- Create: `src-tauri/src/graphics/visual_qa.rs`
- Modify: `src-tauri/src/graphics/mod.rs`
- Modify: `src-tauri/src/graphics/error.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/tests/graphics.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing CPU QA tests**

Add to `src-tauri/tests/graphics.rs`:

```rust
use video_creater_lib::graphics::visual_qa::{run_cpu_visual_qa, CpuVisualQaOptions};
```

Add this test:

```rust
#[test]
fn cpu_visual_qa_rejects_static_animated_layer_frames() {
    let temp = temp_dir("cpu-visual-qa-static");
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "static-layer".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions { width: 320, height: 180 },
        fps: 3.0,
        alpha: true,
        source_beat: "static test".to_string(),
        visual_treatment: "static block".to_string(),
        motion: "claims motion".to_string(),
        safe_zone: "center".to_string(),
        avoid: "static".to_string(),
        nodes: vec![GraphicNode::Rect(RectNode {
            id: "block".to_string(),
            box_rect: Rect { x: 40.0, y: 40.0, width: 120.0, height: 60.0 },
            fill: Color::Hex("#FFFFFFFF".to_string()),
            animate: Some(NodeAnimation {
                ease: Some(Easing::Linear),
                delay_seconds: None,
                repeat: None,
                yoyo: None,
                keyframes: vec![
                    AnimationKeyframe { at: 0.0, x: Some(0.0), y: Some(0.0), scale: None, scale_x: None, scale_y: None, opacity: Some(1.0) },
                    AnimationKeyframe { at: 1.0, x: Some(0.0), y: Some(0.0), scale: None, scale_x: None, scale_y: None, opacity: Some(1.0) },
                ],
            }),
        })],
    };

    let assets = AssetRegistry::new(PathBuf::new());
    let manifest = render_graphics_preview(
        &layer,
        &assets,
        GraphicsRenderOptions { output_dir: temp.clone() },
    )
    .expect("static animated layer should render");

    let errors = run_cpu_visual_qa(&layer, &temp, &manifest, CpuVisualQaOptions::default())
        .expect_err("static animated layer should fail QA");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsVisualQaFailed);
    assert!(errors[0].message.contains("static"));
}
```

- [ ] **Step 2: Run failing CPU QA test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics cpu_visual_qa_rejects_static_animated_layer_frames
```

Expected: FAIL because CPU visual QA does not exist.

- [ ] **Step 3: Implement CPU visual QA**

Create `src-tauri/src/graphics/visual_qa.rs` with:

- first/middle/last frame sampling.
- alpha coverage ratio.
- temporal delta across sampled frames.
- max coverage threshold of `0.30` for `Overlay`, `Caption`, `LowerThird`, and `Diagram`.
- temporal delta minimum of `0.001` when the layer has animation and `manifest.frame_count > 1`.

Add `GraphicsVisualQaFailed` to `GraphicsErrorCode` with serialized code `GRAPHICS_VISUAL_QA_FAILED`.

Add `pub mod visual_qa;` to `src-tauri/src/graphics/mod.rs`.

- [ ] **Step 4: Wire CPU QA into render proposal graphics**

In `src-tauri/src/render_pipeline/proposal.rs`, after each CPU `render_graphics_preview`, call:

```rust
run_cpu_visual_qa(layer, &layer_dir, &manifest, CpuVisualQaOptions::default())
```

Convert errors through `PipelineError::from_graphics_errors`. Store `visual_qa_status: Some("passed".to_string())` for CPU graphics too.

- [ ] **Step 5: Verify CPU QA tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics cpu_visual_qa
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_proposal_graphics
```

Expected: PASS.

## Task 6: Render Report Metadata

**Files:**
- Modify: `src-tauri/src/render_pipeline/report.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src/lib/render.ts`
- Modify: `src/lib/render.test.ts`

- [ ] **Step 1: Write failing metadata tests**

Add to `src-tauri/tests/render_pipeline.rs` near render report tests:

```rust
#[test]
fn render_report_graphics_include_template_preset_and_qa_metadata() {
    let mut report = render_report();
    report.graphics = vec![RenderGraphicsReport {
        layer_id: "proposal-overlay-1".to_string(),
        renderer: "rust".to_string(),
        quality_profile: None,
        template_id: Some("kinetic-lower-third-v1".to_string()),
        motion_preset_id: Some("slide-fade-up-v1".to_string()),
        visual_qa_status: Some("passed".to_string()),
    }];

    let json = serde_json::to_value(&report).expect("serialize report");

    assert_eq!(json["graphics"][0]["templateId"], "kinetic-lower-third-v1");
    assert_eq!(json["graphics"][0]["motionPresetId"], "slide-fade-up-v1");
    assert_eq!(json["graphics"][0]["visualQaStatus"], "passed");
}
```

Update TypeScript render model tests to expect optional `templateId` and `motionPresetId` in graphics entries.

- [ ] **Step 2: Run failing metadata tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_report_graphics_include_template_preset_and_qa_metadata
rtk pnpm test src/lib/render.test.ts
```

Expected: FAIL until report types are extended.

- [ ] **Step 3: Extend report structures**

Add to `RenderGraphicsReport` in `src-tauri/src/render_pipeline/report.rs`:

```rust
pub template_id: Option<String>,
pub motion_preset_id: Option<String>,
```

Add matching fields to the frontend `RenderGraphicReport` type in `src/lib/render.ts`:

```ts
templateId: string | null;
motionPresetId: string | null;
```

In `proposal.rs`, populate those fields from template-backed overlay metadata where available.

- [ ] **Step 4: Verify metadata tests pass**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_report_graphics_include_template_preset_and_qa_metadata
rtk pnpm test src/lib/render.test.ts
```

Expected: PASS.

## Task 7: Verification

**Files:**
- All changed files.

- [ ] **Step 1: Run targeted Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics motion_preset template_recipes cpu_visual_qa
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server motion_presets
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline motion_preset render_report_graphics render_proposal_graphics
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
rtk pnpm test src/lib/motion-templates.test.ts src/lib/render.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run full verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS.

- [ ] **Step 4: Commit**

Run:

```bash
rtk git status --short
rtk git add src/lib/motion-presets.ts src/lib/motion-templates.ts src/lib/motion-templates.test.ts src/lib/render.ts src/lib/render.test.ts src-tauri/src/graphics/motion_presets.rs src-tauri/src/graphics/visual_qa.rs src-tauri/src/graphics/mod.rs src-tauri/src/graphics/error.rs src-tauri/src/graphics/templates.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/src/render_pipeline/report.rs src-tauri/src/codex/app_server.rs src-tauri/tests/graphics.rs src-tauri/tests/render_pipeline.rs src-tauri/tests/codex_app_server.rs docs/superpowers/plans/2026-06-19-visual-generation-quality-first-slice.md
rtk git commit -m "feat: add deterministic visual motion recipes"
```

Expected: commit succeeds with only relevant files staged.

## Self-Review

- Spec coverage: This plan implements the approved first slice: shared preset metadata, Rust preset registry, animated template recipes, Codex prompt/schema updates, CPU overlay QA, and render report metadata.
- Deferred scope: Additional GPU profiles, final export polish, and review UI expansion remain later phases by design.
- Red-flag scan: No task uses deferred markers or unspecified edge handling.
- Type consistency: `motionPresetId` is the TypeScript field, `motion_preset_id` is the Rust report field, and serialized JSON uses camelCase.
