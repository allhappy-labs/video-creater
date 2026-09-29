# Motion System V2 First Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first production Motion v2 slice for Rust CPU overlay graphics: richer typed animation fields, transform origin, rotation, blur/glow/shadow, path and clip progress, text reveal, v2 presets, and preset fallback for custom overlay nodes.

**Architecture:** Extend the existing Rust graphics IR and evaluator instead of introducing a separate animation engine. Keep Codex proposals declarative, validate all new motion fields in Rust, render CPU overlay effects deterministically, and let existing cache fingerprints pick up new serde fields through full-layer serialization.

**Tech Stack:** Rust, serde, tiny-skia, cosmic-text, image PNG fixtures, Vitest/TypeScript catalog tests, existing Video Creater render-pipeline tests.

---

## File Structure

- Modify `src-tauri/src/graphics/ir.rs`: add Motion v2 serde fields and typed text reveal/origin structures.
- Modify `src-tauri/src/graphics/animation.rs`: evaluate new numeric keyframe fields, resolve transform origins, trim line paths, and apply text reveal timing.
- Modify `src-tauri/src/graphics/validation.rs`: validate bounds and role-specific reveal rules.
- Modify `src-tauri/src/graphics/renderer.rs`: render rotated/transformed primitives, line draw-on, clip progress, and shadow/glow/blur polish.
- Modify `src-tauri/src/graphics/motion_presets.rs`: add v2 preset ids and keyframes.
- Modify `src-tauri/src/codex/app_server.rs`: expose v2 motion fields and preset ids in prompt/schema.
- Modify `src-tauri/src/render_pipeline/proposal.rs`: keep automatic `motionPresetId` expansion for custom nodes and ensure v2 preset ids are accepted.
- Modify `src/lib/motion-presets.ts`: add v2 preset catalog entries.
- Test `src-tauri/tests/graphics.rs`: Rust IR, evaluator, validation, and renderer behavior.
- Test `src-tauri/tests/codex_app_server.rs`: app-server prompt/schema behavior.
- Test `src-tauri/tests/render_pipeline.rs`: proposal conversion, preset fallback, and cache fingerprint behavior.
- Test `src/lib/motion-templates.test.ts`: TypeScript preset catalog compatibility.

---

### Task 1: Motion V2 IR And Evaluator Fields

**Files:**
- Modify: `src-tauri/src/graphics/ir.rs`
- Modify: `src-tauri/src/graphics/animation.rs`
- Modify: `src-tauri/src/graphics/motion_presets.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing IR/evaluator tests**

Add these tests to `src-tauri/tests/graphics.rs` near the existing animation evaluator tests:

```rust
#[test]
fn motion_v2_keyframes_deserialize_and_evaluate_new_properties() {
    let animation: NodeAnimation = serde_json::from_value(serde_json::json!({
        "ease": "linear",
        "origin": { "x": "left", "y": "bottom" },
        "keyframes": [
            {
                "at": 0.0,
                "rotationDegrees": -6.0,
                "blurRadius": 4.0,
                "shadowOpacity": 0.0,
                "glowOpacity": 0.0,
                "clipProgress": 0.0,
                "pathProgress": 0.0
            },
            {
                "at": 1.0,
                "rotationDegrees": 0.0,
                "blurRadius": 0.0,
                "shadowOpacity": 0.55,
                "glowOpacity": 0.4,
                "clipProgress": 1.0,
                "pathProgress": 1.0
            }
        ]
    }))
    .expect("motion v2 animation JSON should deserialize");

    let state = evaluate_animation(&animation, 0.5, 1.0);

    assert!((state.rotation_degrees + 3.0).abs() < f64::EPSILON);
    assert!((state.blur_radius - 2.0).abs() < f64::EPSILON);
    assert!((state.shadow_opacity - 0.275).abs() < f64::EPSILON);
    assert!((state.glow_opacity - 0.2).abs() < f64::EPSILON);
    assert!((state.clip_progress - 0.5).abs() < f64::EPSILON);
    assert!((state.path_progress - 0.5).abs() < f64::EPSILON);
}

#[test]
fn transform_origin_round_trips_with_keyword_and_numeric_coordinates() {
    let animation: NodeAnimation = serde_json::from_value(serde_json::json!({
        "origin": { "x": 120.0, "y": "center" },
        "keyframes": [
            { "at": 0.0, "rotationDegrees": 0.0 },
            { "at": 1.0, "rotationDegrees": 12.0 }
        ]
    }))
    .expect("origin should deserialize");

    let json = serde_json::to_value(&animation).expect("serialize origin");

    assert_eq!(json["origin"]["x"], serde_json::json!(120.0));
    assert_eq!(json["origin"]["y"], serde_json::json!("center"));
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics motion_v2_keyframes_deserialize_and_evaluate_new_properties transform_origin_round_trips_with_keyword_and_numeric_coordinates
```

Expected: FAIL because `origin`, `rotationDegrees`, `blurRadius`, `shadowOpacity`, `glowOpacity`, `clipProgress`, and `pathProgress` are not in the Rust IR.

- [ ] **Step 3: Add IR types and evaluator fields**

In `src-tauri/src/graphics/ir.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformOrigin {
    pub x: TransformOriginCoordinate,
    pub y: TransformOriginCoordinate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TransformOriginCoordinate {
    Keyword(String),
    Pixels(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextReveal {
    pub mode: TextRevealMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stagger_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<TextRevealOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_reveal_duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TextRevealMode {
    Whole,
    Line,
    Word,
    Character,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TextRevealOrder {
    Forward,
    Reverse,
    CenterOut,
}
```

Add `origin: Option<TransformOrigin>` to `NodeAnimation`, add optional v2 numeric fields to `AnimationKeyframe`, and add `text_reveal: Option<TextReveal>` to `TextNode` with `#[serde(rename = "textReveal", default, skip_serializing_if = "Option::is_none")]`.

In `src-tauri/src/graphics/animation.rs`, extend `AnimationState` with:

```rust
pub rotation_degrees: f64,
pub blur_radius: f64,
pub shadow_opacity: f64,
pub glow_opacity: f64,
pub clip_progress: f64,
pub path_progress: f64,
```

Interpolate the new fields with defaults `0.0` except `clip_progress` and `path_progress`, which default to `1.0` when no keyframe defines the property.

Update `src-tauri/src/graphics/motion_presets.rs::keyframe` so it initializes all new fields to `None` and sets `origin: None` on existing v1 `NodeAnimation` values.

- [ ] **Step 4: Run tests to verify green**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics motion_v2_keyframes_deserialize_and_evaluate_new_properties transform_origin_round_trips_with_keyword_and_numeric_coordinates
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
rtk git add src-tauri/src/graphics/ir.rs src-tauri/src/graphics/animation.rs src-tauri/src/graphics/motion_presets.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: add motion v2 animation fields"
```

### Task 2: Validation And App-Server Schema

**Files:**
- Modify: `src-tauri/src/graphics/validation.rs`
- Modify: `src-tauri/src/codex/app_server.rs`
- Test: `src-tauri/tests/graphics.rs`
- Test: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing validation tests**

Add a test in `src-tauri/tests/graphics.rs`:

```rust
#[test]
fn rejects_invalid_motion_v2_animation_fields_actionably() {
    let mut layer = valid_text_layer();
    if let GraphicNode::Text(text) = &mut layer.nodes[0] {
        text.text_reveal = Some(serde_json::from_value(serde_json::json!({
            "mode": "character",
            "staggerSeconds": -0.1,
            "order": "forward",
            "maxRevealDurationSeconds": 3.0
        }))
        .expect("text reveal"));
        text.animate = Some(serde_json::from_value(serde_json::json!({
            "origin": { "x": "bad-x", "y": "center" },
            "keyframes": [
                {
                    "at": 0.0,
                    "rotationDegrees": 90.0,
                    "blurRadius": -1.0,
                    "shadowOpacity": 2.0,
                    "glowOpacity": -0.2,
                    "clipProgress": 1.2,
                    "pathProgress": -0.1
                }
            ]
        }))
        .expect("bad v2 animation JSON should still deserialize"));
    }

    let errors = expect_validation_errors(layer);

    assert_error(&errors, GraphicsErrorCode::GraphicsTemplateParamInvalid, "nodes[0].animate.origin.x");
    assert_error(&errors, GraphicsErrorCode::GraphicsTemplateParamInvalid, "nodes[0].animate.keyframes[0].rotationDegrees");
    assert_error(&errors, GraphicsErrorCode::GraphicsTemplateParamInvalid, "nodes[0].animate.keyframes[0].blurRadius");
    assert_error(&errors, GraphicsErrorCode::GraphicsTemplateParamInvalid, "nodes[0].animate.keyframes[0].clipProgress");
    assert_error(&errors, GraphicsErrorCode::GraphicsTemplateParamInvalid, "nodes[0].textReveal.staggerSeconds");
}
```

Add a schema test in `src-tauri/tests/codex_app_server.rs`:

```rust
#[test]
fn turn_request_lists_motion_v2_schema_fields() {
    let project = sample_project();
    let request = sample_edit_request();
    let context = build_video_edit_context(&project, &request).expect("context");

    let turn = build_video_edit_turn_request(9, "thread-123", &context);
    let text = turn["params"]["input"][0]["text"].as_str().expect("text");
    let animate = &turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
        ["nodes"]["items"]["properties"]["animate"]["properties"];
    let text_reveal = &turn["params"]["outputSchema"]["properties"]["overlays"]["items"]["properties"]
        ["nodes"]["items"]["properties"]["textReveal"];

    assert!(text.contains("rotationDegrees"));
    assert!(text.contains("pathProgress"));
    assert!(text.contains("textReveal"));
    assert_eq!(animate["origin"]["properties"]["x"]["oneOf"][0]["type"], "string");
    assert_eq!(animate["keyframes"]["items"]["properties"]["rotationDegrees"]["type"], "number");
    assert_eq!(animate["keyframes"]["items"]["properties"]["clipProgress"]["maximum"], 1);
    assert_eq!(text_reveal["properties"]["mode"]["enum"], serde_json::json!(["whole", "line", "word", "character"]));
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics rejects_invalid_motion_v2_animation_fields_actionably
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_motion_v2_schema_fields
```

Expected: FAIL because validation and schema do not know Motion v2 fields.

- [ ] **Step 3: Implement validation**

In `src-tauri/src/graphics/validation.rs`:

- Pass `layer.role` into text-node validation so `character` reveal can be rejected for `GraphicRole::Caption`.
- Validate origin keywords by axis: x allows `left`, `center`, `right`; y allows `top`, `center`, `bottom`; numeric coordinates must be finite.
- Validate `rotationDegrees` is finite and between `-45.0` and `45.0`.
- Validate `blurRadius >= 0.0` and no more than `64.0`.
- Validate `shadowOpacity` and `glowOpacity` are between `0.0` and `1.0`.
- Validate `clipProgress` and `pathProgress` are between `0.0` and `1.0`.
- Validate text reveal stagger is finite and `>= 0.0`, and max reveal duration is finite, positive, and no more than half the layer duration.

- [ ] **Step 4: Implement app-server prompt/schema**

In `src-tauri/src/codex/app_server.rs`:

- Add the v2 fields to `animation_schema()`.
- Add `textReveal` to the `text` node schema only.
- Update the prompt under `Animated primitive nodes` to name rotation, origin, blur, glow, shadow, clip/path progress, and text reveal.
- Keep `additionalProperties: false`.

- [ ] **Step 5: Run tests to verify green**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics rejects_invalid_motion_v2_animation_fields_actionably
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_motion_v2_schema_fields
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

```bash
rtk git add src-tauri/src/graphics/validation.rs src-tauri/src/codex/app_server.rs src-tauri/tests/graphics.rs src-tauri/tests/codex_app_server.rs
rtk git commit -m "feat: validate motion v2 proposal fields"
```

### Task 3: Renderer Support For Rotation, Origin, Path Progress, Clip, Shadow, Glow, And Blur

**Files:**
- Modify: `src-tauri/src/graphics/animation.rs`
- Modify: `src-tauri/src/graphics/renderer.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing renderer tests**

Add focused pixel tests to `src-tauri/tests/graphics.rs`:

```rust
#[test]
fn renders_rotated_rect_around_configured_origin() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "rotation-v2",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 120, "height": 120 },
        "fps": 1.0,
        "alpha": true,
        "sourceBeat": "Rotate a marker around its center.",
        "visualTreatment": "simple rotated rectangle",
        "motion": "settled rotation",
        "safeZone": "inside frame",
        "avoid": "axis-only transform",
        "nodes": [{
            "type": "rect",
            "id": "marker",
            "box": { "x": 45.0, "y": 35.0, "width": 30.0, "height": 50.0 },
            "fill": "#FFFFFFFF",
            "animate": {
                "origin": { "x": "center", "y": "center" },
                "keyframes": [{ "at": 0.0, "rotationDegrees": 45.0 }]
            }
        }]
    }))
    .expect("rotation layer");

    let output_dir = dir.path().join("graphics/rotation-v2");
    render_graphics_preview(&layer, &AssetRegistry::new(dir.path().to_path_buf()), GraphicsRenderOptions { output_dir: output_dir.clone() })
        .expect("render rotated rect");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert!(preview.get_pixel(60, 60).0[3] > 0);
    assert_eq!(preview.get_pixel(45, 35).0[3], 0, "corner should move after rotation");
}

#[test]
fn renders_line_path_progress_as_draw_on() {
    let dir = tempfile::tempdir().expect("create temp output root");
    let layer: GraphicsLayer = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1,
        "id": "line-draw-v2",
        "role": "overlay",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 120, "height": 40 },
        "fps": 1.0,
        "alpha": true,
        "sourceBeat": "Draw a callout line halfway.",
        "visualTreatment": "thin line draw-on",
        "motion": "path progress half",
        "safeZone": "inside frame",
        "avoid": "full line at first frame",
        "nodes": [{
            "type": "line",
            "id": "callout",
            "points": [{ "x": 10.0, "y": 20.0 }, { "x": 110.0, "y": 20.0 }],
            "stroke": "#FFFFFFFF",
            "strokeWidth": 4.0,
            "animate": { "keyframes": [{ "at": 0.0, "pathProgress": 0.5 }] }
        }]
    }))
    .expect("line layer");

    let output_dir = dir.path().join("graphics/line-draw-v2");
    render_graphics_preview(&layer, &AssetRegistry::new(dir.path().to_path_buf()), GraphicsRenderOptions { output_dir: output_dir.clone() })
        .expect("render line draw");
    let preview = decode_png(&output_dir.join("preview.png"));

    assert!(preview.get_pixel(35, 20).0[3] > 0);
    assert_eq!(preview.get_pixel(90, 20).0[3], 0);
}
```

Add similar tests for `clipProgress` revealing half a rect and `glowOpacity`/`shadowOpacity` creating alpha outside the base box.

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics renders_rotated_rect_around_configured_origin renders_line_path_progress_as_draw_on
```

Expected: FAIL because the renderer ignores rotation/path progress.

- [ ] **Step 3: Implement transform and path progress**

In `src-tauri/src/graphics/animation.rs`:

- Resolve transform origin for rect-like nodes from `TransformOrigin`; default to rect center.
- Resolve transform origin for line/polygon nodes from bounds center unless origin is supplied.
- Rotate and scale `LineNode` and `PolygonNode` points directly.
- Add a `trim_line_points(points: &[Point], progress: f64) -> Vec<Point>` helper that returns points from path start through the requested progress.

In `src-tauri/src/graphics/renderer.rs`:

- Apply `Transform` when drawing `RectNode` and `RoundedRectNode`.
- Keep text and image rotation in this slice by rendering into a temporary pixmap and compositing it through a rotation transform only when rotation is non-zero.
- Apply `clipProgress` by clipping each node pixmap to a left-to-right reveal rectangle before compositing.
- Apply `pathProgress` before drawing `LineNode`.

- [ ] **Step 4: Implement shadow, glow, and blur**

Use bounded raster passes:

- Shadow: render the node alpha offset by `(6, 8)` pixels with black alpha multiplied by `shadowOpacity * 0.45`.
- Glow: expand around nonzero-alpha pixels within a radius of `6` pixels and tint with the node color multiplied by `glowOpacity * 0.35`.
- Blur: for this first slice, soften alpha by a bounded 3x3 box pass repeated `blurRadius.round().min(8.0)` times.

Do not add new crate dependencies.

- [ ] **Step 5: Run tests to verify green**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics renders_rotated_rect_around_configured_origin renders_line_path_progress_as_draw_on
```

Expected: PASS. Also run any added clip/glow/shadow tests.

- [ ] **Step 6: Commit Task 3**

```bash
rtk git add src-tauri/src/graphics/animation.rs src-tauri/src/graphics/renderer.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: render motion v2 overlay effects"
```

### Task 4: Structured Text Reveal

**Files:**
- Modify: `src-tauri/src/graphics/animation.rs`
- Modify: `src-tauri/src/graphics/renderer.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing text reveal tests**

Add:

```rust
#[test]
fn text_reveal_character_mode_progressively_reveals_text() {
    let mut node = valid_text_node("reveal");
    node.text = "ABCD".to_string();
    node.text_reveal = Some(serde_json::from_value(serde_json::json!({
        "mode": "character",
        "staggerSeconds": 0.1,
        "order": "forward",
        "maxRevealDurationSeconds": 0.4
    }))
    .expect("text reveal"));

    let early = animate_node(&GraphicNode::Text(node.clone()), 0.11, 1.0);
    let late = animate_node(&GraphicNode::Text(node), 0.45, 1.0);
    let GraphicNode::Text(early_text) = early else { panic!("expected text"); };
    let GraphicNode::Text(late_text) = late else { panic!("expected text"); };

    assert_eq!(early_text.text, "AB");
    assert_eq!(late_text.text, "ABCD");
}
```

- [ ] **Step 2: Run test to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics text_reveal_character_mode_progressively_reveals_text
```

Expected: FAIL because `TextNode` does not apply text reveal.

- [ ] **Step 3: Implement reveal modes**

In `src-tauri/src/graphics/animation.rs`:

- Treat `textReveal` as temporal animation in `layer_has_animation`.
- For `whole`, keep existing text once progress is above zero.
- For `word`, split on whitespace and preserve spaces between revealed words.
- For `line`, split on `\n`.
- For `character`, reveal Unicode scalar values with `text.chars()`.
- Implement `forward`, `reverse`, and `centerOut` ordering. Preserve original ordering in the displayed string by hiding unrevealed units rather than reordering visible units.

- [ ] **Step 4: Run test to verify green**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics text_reveal_character_mode_progressively_reveals_text
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
rtk git add src-tauri/src/graphics/animation.rs src-tauri/src/graphics/renderer.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: add structured text reveal"
```

### Task 5: Motion V2 Presets And Custom Overlay Preset Expansion

**Files:**
- Modify: `src-tauri/src/graphics/motion_presets.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/src/codex/app_server.rs`
- Test: `src-tauri/tests/graphics.rs`
- Test: `src-tauri/tests/render_pipeline.rs`
- Test: `src-tauri/tests/codex_app_server.rs`

- [ ] **Step 1: Write failing preset tests**

Add to `src-tauri/tests/graphics.rs`:

```rust
#[test]
fn parses_motion_v2_preset_ids_and_builds_v2_keyframes() {
    assert_eq!(
        parse_motion_preset_id("slide-rotate-settle-v2"),
        Some(MotionPresetId::SlideRotateSettleV2)
    );

    let animation = motion_preset_animation(MotionPresetId::LineDrawV2, "accent-line");
    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.path_progress == Some(0.0)));
    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.path_progress == Some(1.0)));
}
```

Add to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn proposal_visuals_apply_motion_v2_preset_to_custom_nodes_without_animation() {
    let mut proposal = sample_codex_proposal();
    proposal.overlays = vec![serde_json::json!({
        "kind": "overlay",
        "startSeconds": 1.0,
        "durationSeconds": 2.0,
        "sourceBeat": "draw a metric badge with v2 motion",
        "visualTreatment": "primitive badge with v2 depth motion",
        "motion": "soft depth card v2 preset",
        "motionPresetId": "soft-depth-card-v2",
        "safeZone": "keep badge inside 10% margins",
        "avoid": "static marker",
        "nodes": [{
            "type": "rect",
            "id": "metric-backing",
            "box": { "x": 220.0, "y": 180.0, "width": 360.0, "height": 160.0 },
            "fill": "#101820CC"
        }]
    })];

    let layers = proposal_visuals_to_graphics_layers(&proposal, 1280, 720, 30.0)
        .expect("custom primitive overlay should convert");
    let animation = layers[1].nodes[0].animation().expect("v2 preset animation");

    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.shadow_opacity.is_some()));
    assert!(animation
        .keyframes
        .iter()
        .any(|keyframe| keyframe.rotation_degrees.is_some()));
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics parses_motion_v2_preset_ids_and_builds_v2_keyframes
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_visuals_apply_motion_v2_preset_to_custom_nodes_without_animation
```

Expected: FAIL because v2 preset ids are unknown.

- [ ] **Step 3: Implement preset ids and animations**

Add `MotionPresetId` variants and `SUPPORTED_MOTION_PRESETS` values:

- `spring-pop-v2`
- `slide-rotate-settle-v2`
- `mask-wipe-v2`
- `line-draw-v2`
- `word-pop-stagger-v2`
- `soft-depth-card-v2`
- `pulse-emphasis-v2`
- `exit-snap-v2`

Use helper constructors that set only bounded fields:

- `slide-rotate-settle-v2`: `x`, `opacity`, `rotationDegrees`, `blurRadius`.
- `mask-wipe-v2`: `clipProgress`, `opacity`.
- `line-draw-v2`: `pathProgress`, `opacity`, optional `glowOpacity`.
- `soft-depth-card-v2`: `shadowOpacity`, `glowOpacity`, `rotationDegrees`, `opacity`.
- `word-pop-stagger-v2`: `scale`, `opacity`, plus default `TextReveal` added in proposal/template conversion only for text nodes.

Keep the existing automatic custom-node behavior in `custom_overlay_nodes`: nodes with explicit `animate` keep it; nodes without `animate` receive `motion_preset_animation(preset, node_id)`.

- [ ] **Step 4: Update prompt/schema preset enums**

Ensure `SUPPORTED_MOTION_PRESETS` drives `motionPresetId` schema and prompt text so v2 ids are accepted.

- [ ] **Step 5: Run tests to verify green**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics parses_motion_v2_preset_ids_and_builds_v2_keyframes
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_visuals_apply_motion_v2_preset_to_custom_nodes_without_animation
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_motion_presets_and_overlay_schema_allows_motion_preset_id
```

Expected: PASS.

- [ ] **Step 6: Commit Task 5**

```bash
rtk git add src-tauri/src/graphics/motion_presets.rs src-tauri/src/render_pipeline/proposal.rs src-tauri/src/codex/app_server.rs src-tauri/tests/graphics.rs src-tauri/tests/render_pipeline.rs src-tauri/tests/codex_app_server.rs
rtk git commit -m "feat: add motion v2 presets"
```

### Task 6: TypeScript Catalog, Cache Fingerprint, And Verification

**Files:**
- Modify: `src/lib/motion-presets.ts`
- Modify: `src/lib/motion-templates.test.ts`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing TypeScript and fingerprint tests**

In `src/lib/motion-templates.test.ts`, update the expected `motionPresetCatalog.map((preset) => preset.id)` list to include all v2 ids after the existing v1 ids.

In `src-tauri/tests/render_pipeline.rs`, add:

```rust
#[test]
fn graphics_cache_fingerprint_changes_for_motion_v2_fields() {
    let mut base = sample_cache_graphics_layer("motion-v2-cache");
    let mut changed = base.clone();

    if let GraphicNode::Text(text) = &mut base.nodes[0] {
        text.animate = Some(serde_json::from_value(serde_json::json!({
            "keyframes": [
                { "at": 0.0, "rotationDegrees": 0.0 },
                { "at": 1.0, "rotationDegrees": 0.0 }
            ]
        }))
        .expect("base animation"));
    }
    if let GraphicNode::Text(text) = &mut changed.nodes[0] {
        text.animate = Some(serde_json::from_value(serde_json::json!({
            "keyframes": [
                { "at": 0.0, "rotationDegrees": 0.0 },
                { "at": 1.0, "rotationDegrees": 6.0 }
            ]
        }))
        .expect("changed animation"));
    }

    let base_fingerprint = graphics_layer_fingerprint(&base, "rust", None).expect("base fingerprint");
    let changed_fingerprint = graphics_layer_fingerprint(&changed, "rust", None).expect("changed fingerprint");

    assert_ne!(base_fingerprint, changed_fingerprint);
}
```

- [ ] **Step 2: Run tests to verify red**

Run:

```bash
rtk pnpm test -- --run src/lib/motion-templates.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline graphics_cache_fingerprint_changes_for_motion_v2_fields
```

Expected: TypeScript test fails until catalog ids are added. Fingerprint test should fail until IR fields serialize.

- [ ] **Step 3: Update TypeScript catalog**

In `src/lib/motion-presets.ts`, extend `MotionPresetId` and `motionPresetCatalog` with the eight v2 ids. Labels should be short editor-facing names, for example `Spring Pop V2`, `Line Draw V2`, and `Soft Depth Card V2`.

- [ ] **Step 4: Run targeted verification**

Run:

```bash
rtk pnpm test -- --run src/lib/motion-templates.test.ts
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_visuals_apply_motion_v2_preset_to_custom_nodes_without_animation graphics_cache_fingerprint_changes_for_motion_v2_fields
rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_motion_v2_schema_fields turn_request_lists_motion_presets_and_overlay_schema_allows_motion_preset_id
```

Expected: PASS.

- [ ] **Step 5: Run final project verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS. If this project-level command is too broad or unavailable, run the narrow Rust and TypeScript commands above and record the limitation.

- [ ] **Step 6: Commit Task 6**

```bash
rtk git add src/lib/motion-presets.ts src/lib/motion-templates.test.ts src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: expose motion v2 preset catalog"
```

---

## Self-Review

- Spec coverage: This plan covers the first production CPU overlay slice for Motion v2 fields, rotation/origin, blur/glow/shadow, path trim, clip progress, per-character text reveal, v2 presets, app-server schema, automatic custom-node preset application, and cache fingerprint behavior.
- Out of scope for this slice: full motion QA report UI, GPU profile alignment, arbitrary masks, external animation runtimes, and subjective visual scoring.
- Placeholder scan: no TBD/TODO/implement-later placeholders remain.
- Type consistency: Rust uses serde camelCase fields (`rotationDegrees`, `clipProgress`, `textReveal`) and snake_case Rust fields (`rotation_degrees`, `clip_progress`, `text_reveal`) consistently.
