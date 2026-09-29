# Rust Native Graphics Frame Generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Rust-native graphics/frame-generation slice: validated graphics IR, `imageRef` resolution, PNG preview/manifest output, existing-template expansion, and `ffmpeg` manifest consumption hooks.

**Architecture:** Add a focused `video_creater_lib::graphics` module that owns graphics IR, actionable errors, validation, asset resolution, template expansion, and PNG artifact generation. Keep the existing `edit::render_plan::build_ffmpeg_render_command` stable, and add a new opt-in function that composes graphics manifests over the existing trim/concat output.

**Tech Stack:** Rust 1.77, `serde`, `serde_json`, `thiserror`, `tiny-skia`, `cosmic-text`, `png`, existing `ffmpeg` CLI command construction.

---

## Scope

This plan implements the first vertical slice only. It does not replace `ffmpeg`, does not add UI proposal preview, and does not allow arbitrary executable graphics code. The slice is complete when Rust can validate a graphics layer, resolve an approved image asset, expand one existing motion template into IR, render a PNG preview/manifest, and produce an `ffmpeg` command that consumes the graphics manifest.

## File Structure

- Create `src-tauri/src/graphics/mod.rs`: module exports.
- Create `src-tauri/src/graphics/error.rs`: compact `ActionableError` and `GraphicsErrorCode`.
- Create `src-tauri/src/graphics/ir.rs`: versioned graphics IR structs and primitive enums.
- Create `src-tauri/src/graphics/validation.rs`: deterministic validation for scene, timing, text, safe zone, and primitives.
- Create `src-tauri/src/graphics/assets.rs`: approved `imageRef` asset registry and resolver.
- Create `src-tauri/src/graphics/manifest.rs`: graphics artifact manifest structs and path helpers.
- Create `src-tauri/src/graphics/renderer.rs`: tiny-skia/png based preview and frame writer.
- Create `src-tauri/src/graphics/templates.rs`: adapter from existing `TemplateRenderLayer` to graphics IR.
- Modify `src-tauri/src/lib.rs`: export the `graphics` module.
- Modify `src-tauri/Cargo.toml`: add renderer dependencies.
- Modify `src-tauri/src/edit/render_plan.rs`: add graphics manifest overlay command support without changing existing command API.
- Create `src-tauri/tests/graphics.rs`: unit/integration-style coverage for validation, asset resolution, manifest output, and command generation.

## Task 1: Graphics IR And Actionable Errors

**Files:**
- Create: `src-tauri/src/graphics/mod.rs`
- Create: `src-tauri/src/graphics/error.rs`
- Create: `src-tauri/src/graphics/ir.rs`
- Create: `src-tauri/src/graphics/validation.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing tests for error serialization and minimal IR validation**

Add `src-tauri/tests/graphics.rs`:

```rust
use video_creater_lib::graphics::error::{ActionableError, GraphicsErrorCode};
use video_creater_lib::graphics::ir::{
    Color, Dimensions, GraphicNode, GraphicRole, GraphicsLayer, Rect, TextNode,
};
use video_creater_lib::graphics::validation::validate_graphics_layer;

#[test]
fn actionable_error_serializes_for_agent_repair() {
    let error = ActionableError::new(
        GraphicsErrorCode::GraphicsTextOverflow,
        "layers[0].nodes[0]",
        "Text does not fit inside safe zone.",
        "Reduce fontSize, shorten text, or increase box width.",
    );

    let serialized = serde_json::to_value(error).expect("serialize actionable error");

    assert_eq!(serialized["code"], "GRAPHICS_TEXT_OVERFLOW");
    assert_eq!(serialized["path"], "layers[0].nodes[0]");
    assert_eq!(serialized["message"], "Text does not fit inside safe zone.");
    assert_eq!(
        serialized["fix"],
        "Reduce fontSize, shorten text, or increase box width."
    );
}

#[test]
fn validates_minimal_text_overlay_ir() {
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "agent.metric-pop.v1".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 4.2,
        duration_seconds: 2.4,
        dimensions: Dimensions {
            width: 1920,
            height: 1080,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Introduce the metric without covering the speaker.".to_string(),
        visual_treatment: "floating metric tile with translucent backing".to_string(),
        motion: "scale pop, accent sweep, short hold, fade".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "covering faces, opaque caption slabs, tiny text".to_string(),
        nodes: vec![GraphicNode::Text(TextNode {
            id: "headline".to_string(),
            text: "42%".to_string(),
            box_rect: Rect {
                x: 120.0,
                y: 160.0,
                width: 420.0,
                height: 120.0,
            },
            font_size: 72.0,
            font_weight: 800,
            align: "center".to_string(),
            fill: Color::Hex("#ffffff".to_string()),
            max_lines: Some(1),
        })],
    };

    validate_graphics_layer(&layer).expect("valid graphics layer");
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml actionable_error_serializes_for_agent_repair validates_minimal_text_overlay_ir
```

Expected: FAIL because `video_creater_lib::graphics` does not exist.

- [ ] **Step 3: Add module exports**

Create `src-tauri/src/graphics/mod.rs`:

```rust
pub mod assets;
pub mod error;
pub mod ir;
pub mod manifest;
pub mod renderer;
pub mod templates;
pub mod validation;
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod codex;
pub mod edit;
pub mod graphics;
pub mod project;
pub mod transcription;
```

- [ ] **Step 4: Add actionable errors**

Create `src-tauri/src/graphics/error.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GraphicsErrorCode {
    GraphicsSchemaUnsupported,
    GraphicsEmptyNodeId,
    GraphicsUnsupportedPrimitive,
    GraphicsInvalidTiming,
    GraphicsInvalidDimensions,
    GraphicsTextEmpty,
    GraphicsTextOverflow,
    GraphicsTextUnreadable,
    GraphicsNodeOutOfSafeZone,
    GraphicsFrameCoverageExceeded,
    GraphicsImageRefMissing,
    GraphicsImageRefUnauthorized,
    GraphicsImageDecodeFailed,
    GraphicsTemplateParamMissing,
    GraphicsTemplateParamInvalid,
    GraphicsRenderFailed,
    RenderOverlayOutOfRange,
    RenderFrameSequenceEmpty,
    RenderBackendUnavailable,
    RenderBackendFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionableError {
    pub code: GraphicsErrorCode,
    pub path: String,
    pub message: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<GraphicsErrorCode>,
}

impl ActionableError {
    pub fn new(
        code: GraphicsErrorCode,
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
            cause: None,
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn with_cause(mut self, cause: GraphicsErrorCode) -> Self {
        self.cause = Some(cause);
        self
    }
}

pub type ActionableResult<T> = Result<T, Vec<ActionableError>>;
```

- [ ] **Step 5: Add IR structs**

Create `src-tauri/src/graphics/ir.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsLayer {
    pub schema_version: u32,
    pub id: String,
    pub role: GraphicRole,
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
    pub nodes: Vec<GraphicNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphicRole {
    Caption,
    Overlay,
    LowerThird,
    TitleCard,
    Diagram,
    Transition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Color {
    Hex(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GraphicNode {
    Text(TextNode),
    RoundedRect(RoundedRectNode),
    Rect(RectNode),
    Line(LineNode),
    ImageRef(ImageRefNode),
}

impl GraphicNode {
    pub fn id(&self) -> &str {
        match self {
            GraphicNode::Text(node) => &node.id,
            GraphicNode::RoundedRect(node) => &node.id,
            GraphicNode::Rect(node) => &node.id,
            GraphicNode::Line(node) => &node.id,
            GraphicNode::ImageRef(node) => &node.id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextNode {
    pub id: String,
    pub text: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub font_size: f64,
    pub font_weight: u16,
    pub align: String,
    pub fill: Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoundedRectNode {
    pub id: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub radius: f64,
    pub fill: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RectNode {
    pub id: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub fill: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineNode {
    pub id: String,
    pub points: Vec<Point>,
    pub stroke: Color,
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageRefNode {
    pub id: String,
    pub asset_id: String,
    #[serde(rename = "box")]
    pub box_rect: Rect,
    pub fit: ImageFit,
    pub opacity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    Contain,
    Cover,
    Stretch,
    None,
}
```

- [ ] **Step 6: Add validation**

Create `src-tauri/src/graphics/validation.rs`:

```rust
use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::{GraphicNode, GraphicsLayer, Rect};
use std::collections::BTreeSet;

const MIN_DIMENSION: u32 = 16;
const MAX_DIMENSION: u32 = 7680;
const MIN_FPS: f64 = 1.0;
const MAX_FPS: f64 = 120.0;
const MAX_DURATION_SECONDS: f64 = 600.0;
const MIN_READABLE_FONT_SIZE: f64 = 14.0;

pub fn validate_graphics_layer(layer: &GraphicsLayer) -> ActionableResult<()> {
    let mut errors = Vec::new();

    if layer.schema_version != 1 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsSchemaUnsupported,
            "schemaVersion",
            "Graphics schema version is not supported.",
            "Set schemaVersion to 1.",
        ));
    }

    if layer.id.trim().is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsEmptyNodeId,
            "id",
            "Layer id is empty.",
            "Set id to a stable non-empty string.",
        ));
    }

    validate_dimensions(layer, &mut errors);
    validate_timing(layer, &mut errors);
    validate_required_text(layer, &mut errors);
    validate_nodes(layer, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_dimensions(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if layer.dimensions.width < MIN_DIMENSION
        || layer.dimensions.height < MIN_DIMENSION
        || layer.dimensions.width > MAX_DIMENSION
        || layer.dimensions.height > MAX_DIMENSION
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            "dimensions",
            "Layer dimensions are outside the supported range.",
            "Use width and height between 16 and 7680 pixels.",
        ));
    }
}

fn validate_timing(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if !layer.timeline_start.is_finite() || layer.timeline_start < 0.0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "timelineStart",
            "Layer start time is invalid.",
            "Use a finite timelineStart greater than or equal to 0.",
        ));
    }
    if !layer.duration_seconds.is_finite()
        || layer.duration_seconds <= 0.0
        || layer.duration_seconds > MAX_DURATION_SECONDS
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "durationSeconds",
            "Layer duration is invalid.",
            "Use a finite durationSeconds between 0 and 600.",
        ));
    }
    if !layer.fps.is_finite() || !(MIN_FPS..=MAX_FPS).contains(&layer.fps) {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "fps",
            "Layer fps is invalid.",
            "Use a finite fps between 1 and 120.",
        ));
    }
}

fn validate_required_text(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    for (field, value) in [
        ("sourceBeat", &layer.source_beat),
        ("visualTreatment", &layer.visual_treatment),
        ("motion", &layer.motion),
        ("safeZone", &layer.safe_zone),
        ("avoid", &layer.avoid),
    ] {
        if value.trim().is_empty() {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextEmpty,
                field,
                "Required visual metadata is empty.",
                format!("Set {field} to a concise non-empty description."),
            ));
        }
    }
}

fn validate_nodes(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if layer.nodes.is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsUnsupportedPrimitive,
            "nodes",
            "Layer has no drawable nodes.",
            "Add at least one supported primitive node.",
        ));
    }

    let mut ids = BTreeSet::new();
    for (index, node) in layer.nodes.iter().enumerate() {
        let path = format!("nodes[{index}]");
        let id = node.id().trim();
        if id.is_empty() || !ids.insert(id.to_string()) {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsEmptyNodeId,
                format!("{path}.id"),
                "Node id is empty or duplicated.",
                "Use a unique non-empty id for every node.",
            ));
        }

        match node {
            GraphicNode::Text(text) => {
                validate_rect(&text.box_rect, &format!("{path}.box"), errors);
                if text.text.trim().is_empty() {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTextEmpty,
                        format!("{path}.text"),
                        "Text node content is empty.",
                        "Set text to a short readable string.",
                    ));
                }
                if !text.font_size.is_finite() || text.font_size < MIN_READABLE_FONT_SIZE {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTextUnreadable,
                        format!("{path}.fontSize"),
                        "Text font size is too small.",
                        "Use fontSize greater than or equal to 14.",
                    ));
                }
                if text.box_rect.width < text.font_size * 0.8 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTextOverflow,
                        format!("{path}.box"),
                        "Text box is too narrow for the configured font size.",
                        "Reduce fontSize, shorten text, or increase box width.",
                    ));
                }
            }
            GraphicNode::RoundedRect(rect) => {
                validate_rect(&rect.box_rect, &format!("{path}.box"), errors);
            }
            GraphicNode::Rect(rect) => {
                validate_rect(&rect.box_rect, &format!("{path}.box"), errors);
            }
            GraphicNode::Line(line) => {
                if line.points.len() < 2 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsUnsupportedPrimitive,
                        format!("{path}.points"),
                        "Line node needs at least two points.",
                        "Add two or more finite points.",
                    ));
                }
            }
            GraphicNode::ImageRef(image) => {
                validate_rect(&image.box_rect, &format!("{path}.box"), errors);
                if image.asset_id.trim().is_empty() {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsImageRefMissing,
                        format!("{path}.assetId"),
                        "Image asset reference is empty.",
                        "Set assetId to an approved project or bundled image asset.",
                    ));
                }
                if !image.opacity.is_finite() || image.opacity < 0.0 || image.opacity > 1.0 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTemplateParamInvalid,
                        format!("{path}.opacity"),
                        "Image opacity is invalid.",
                        "Use opacity between 0 and 1.",
                    ));
                }
            }
        }
    }
}

fn validate_rect(rect: &Rect, path: &str, errors: &mut Vec<ActionableError>) {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            path,
            "Node bounds are invalid.",
            "Use finite x, y, width, and height with positive width and height.",
        ));
    }
}
```

- [ ] **Step 7: Add scaffold modules needed by `mod.rs`**

Create these files with module-level comments so the crate compiles before subsequent tasks fill them in:

`src-tauri/src/graphics/assets.rs`

```rust
//! Approved image asset resolution for graphics rendering.
```

`src-tauri/src/graphics/manifest.rs`

```rust
//! Graphics artifact manifests shared with video render backends.
```

`src-tauri/src/graphics/renderer.rs`

```rust
//! Rust-native graphics rasterization.
```

`src-tauri/src/graphics/templates.rs`

```rust
//! Motion template to graphics IR adapters.
```

- [ ] **Step 8: Run tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml actionable_error_serializes_for_agent_repair validates_minimal_text_overlay_ir
```

Expected: PASS.

- [ ] **Step 9: Commit**

Run:

```bash
rtk git add src-tauri/src/lib.rs src-tauri/src/graphics src-tauri/tests/graphics.rs
rtk git commit -m "feat: add graphics ir validation"
```

## Task 2: `imageRef` Asset Resolution

**Files:**
- Modify: `src-tauri/src/graphics/assets.rs`
- Modify: `src-tauri/src/graphics/validation.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Add failing tests for approved and missing image refs**

Append to `src-tauri/tests/graphics.rs`:

```rust
use std::path::PathBuf;
use video_creater_lib::graphics::assets::{AssetRegistry, ImageAsset};
use video_creater_lib::graphics::ir::{ImageFit, ImageRefNode};
use video_creater_lib::graphics::validation::validate_graphics_layer_with_assets;

#[test]
fn resolves_registered_image_ref_assets() {
    let mut registry = AssetRegistry::new(PathBuf::from("/project"));
    registry.register(ImageAsset {
        asset_id: "image-logo".to_string(),
        relative_path: "generated/graphics/logo.png".to_string(),
        width: 128,
        height: 64,
    });

    let resolved = registry
        .resolve("image-logo")
        .expect("registered image asset should resolve");

    assert_eq!(resolved.asset_id, "image-logo");
    assert_eq!(resolved.absolute_path, PathBuf::from("/project/generated/graphics/logo.png"));
}

#[test]
fn image_ref_validation_reports_missing_asset_with_fix() {
    let layer = layer_with_image_ref("missing-image");
    let registry = AssetRegistry::new(PathBuf::from("/project"));

    let errors = validate_graphics_layer_with_assets(&layer, &registry)
        .expect_err("missing imageRef should fail");

    assert_eq!(errors[0].code, GraphicsErrorCode::GraphicsImageRefMissing);
    assert_eq!(errors[0].path, "nodes[0].assetId");
    assert_eq!(
        errors[0].fix,
        "Register the image asset or replace assetId with an approved imageRef."
    );
}

fn layer_with_image_ref(asset_id: &str) -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: "image-overlay".to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 1920,
            height: 1080,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Show approved project image.".to_string(),
        visual_treatment: "small image bug in safe zone".to_string(),
        motion: "fade in and fade out".to_string(),
        safe_zone: "inside 10% margins".to_string(),
        avoid: "remote or arbitrary filesystem images".to_string(),
        nodes: vec![GraphicNode::ImageRef(ImageRefNode {
            id: "image".to_string(),
            asset_id: asset_id.to_string(),
            box_rect: Rect {
                x: 100.0,
                y: 100.0,
                width: 240.0,
                height: 120.0,
            },
            fit: ImageFit::Contain,
            opacity: 1.0,
        })],
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml resolves_registered_image_ref_assets image_ref_validation_reports_missing_asset_with_fix
```

Expected: FAIL because `AssetRegistry`, `ImageAsset`, and `validate_graphics_layer_with_assets` do not exist.

- [ ] **Step 3: Implement asset registry**

Replace `src-tauri/src/graphics/assets.rs` with:

```rust
use super::error::{ActionableError, GraphicsErrorCode};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAsset {
    pub asset_id: String,
    pub relative_path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImageAsset {
    pub asset_id: String,
    pub absolute_path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct AssetRegistry {
    project_root: PathBuf,
    images: BTreeMap<String, ImageAsset>,
}

impl AssetRegistry {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            images: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, asset: ImageAsset) {
        self.images.insert(asset.asset_id.clone(), asset);
    }

    pub fn resolve(&self, asset_id: &str) -> Result<ResolvedImageAsset, ActionableError> {
        let Some(asset) = self.images.get(asset_id) else {
            return Err(ActionableError::new(
                GraphicsErrorCode::GraphicsImageRefMissing,
                "assetId",
                "Image asset is not registered.",
                "Register the image asset or replace assetId with an approved imageRef.",
            ));
        };

        if !is_safe_relative_path(&asset.relative_path) {
            return Err(ActionableError::new(
                GraphicsErrorCode::GraphicsImageRefUnauthorized,
                "assetId",
                "Image asset path is not allowed.",
                "Use a project-relative image path without parent directory segments.",
            ));
        }

        Ok(ResolvedImageAsset {
            asset_id: asset.asset_id.clone(),
            absolute_path: self.project_root.join(&asset.relative_path),
            width: asset.width,
            height: asset.height,
        })
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    let candidate = Path::new(path);
    !candidate.is_absolute()
        && candidate
            .components()
            .all(|component| !matches!(component, std::path::Component::ParentDir))
}
```

- [ ] **Step 4: Add asset-aware validation**

Append to `src-tauri/src/graphics/validation.rs`:

```rust
use super::assets::AssetRegistry;

pub fn validate_graphics_layer_with_assets(
    layer: &GraphicsLayer,
    assets: &AssetRegistry,
) -> ActionableResult<()> {
    let mut errors = validate_graphics_layer(layer).err().unwrap_or_default();

    for (index, node) in layer.nodes.iter().enumerate() {
        if let GraphicNode::ImageRef(image) = node {
            if let Err(error) = assets.resolve(&image.asset_id) {
                let mut error = error;
                error.path = format!("nodes[{index}].assetId");
                errors.push(error);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

- [ ] **Step 5: Run tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml resolves_registered_image_ref_assets image_ref_validation_reports_missing_asset_with_fix
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/graphics/assets.rs src-tauri/src/graphics/validation.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: validate graphics image refs"
```

## Task 3: Renderer Dependencies, Manifest, And PNG Preview Output

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/graphics/manifest.rs`
- Modify: `src-tauri/src/graphics/renderer.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Add failing render/manifest test**

Append to `src-tauri/tests/graphics.rs`:

```rust
use tempfile::tempdir;
use video_creater_lib::graphics::renderer::{render_graphics_preview, GraphicsRenderOptions};

#[test]
fn renders_preview_png_and_manifest_for_valid_layer() {
    let dir = tempdir().expect("tempdir");
    let layer = GraphicsLayer {
        schema_version: 1,
        id: "lower-third-preview".to_string(),
        role: GraphicRole::LowerThird,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 640,
            height: 360,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "Label the speaker.".to_string(),
        visual_treatment: "compact lower third with translucent backing".to_string(),
        motion: "slide in and fade out".to_string(),
        safe_zone: "inside 10% margins".to_string(),
        avoid: "full-width opaque slabs".to_string(),
        nodes: vec![
            GraphicNode::RoundedRect(video_creater_lib::graphics::ir::RoundedRectNode {
                id: "backing".to_string(),
                box_rect: Rect {
                    x: 48.0,
                    y: 240.0,
                    width: 360.0,
                    height: 72.0,
                },
                radius: 16.0,
                fill: Color::Hex("#101820cc".to_string()),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: "Olha API".to_string(),
                box_rect: Rect {
                    x: 72.0,
                    y: 256.0,
                    width: 312.0,
                    height: 40.0,
                },
                font_size: 28.0,
                font_weight: 800,
                align: "start".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(1),
            }),
        ],
    };

    let manifest = render_graphics_preview(
        &layer,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: dir.path().join("graphics/lower-third-preview"),
        },
    )
    .expect("render preview");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(manifest.frame_count, 1);
    assert!(dir.path().join("graphics/lower-third-preview/preview.png").exists());
    assert!(dir.path().join("graphics/lower-third-preview/manifest.json").exists());
}
```

- [ ] **Step 2: Run the failing render test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml renders_preview_png_and_manifest_for_valid_layer
```

Expected: FAIL because renderer and manifest APIs do not exist.

- [ ] **Step 3: Add dependencies**

Modify `src-tauri/Cargo.toml` dependencies:

```toml
cosmic-text = "0.19.0"
image = { version = "0.25.8", default-features = false, features = ["png"] }
png = "0.18.1"
tiny-skia = "0.12.0"
```

Keep existing dependencies unchanged.

- [ ] **Step 4: Implement manifest structs**

Replace `src-tauri/src/graphics/manifest.rs` with:

```rust
use super::ir::Dimensions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsArtifactManifest {
    pub schema_version: u32,
    pub artifact_id: String,
    pub kind: String,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub duration_seconds: f64,
    pub alpha: bool,
    pub frame_count: u32,
    pub frames_pattern: String,
    pub preview_path: String,
    pub source_layer_ids: Vec<String>,
    #[serde(default)]
    pub checksums: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenGraphicsArtifact {
    pub manifest: GraphicsArtifactManifest,
    pub manifest_path: PathBuf,
    pub preview_path: PathBuf,
}
```

- [ ] **Step 5: Implement PNG preview renderer**

Replace `src-tauri/src/graphics/renderer.rs` with:

```rust
use super::assets::AssetRegistry;
use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::{Color, GraphicNode, GraphicsLayer, Rect, TextNode};
use super::manifest::GraphicsArtifactManifest;
use super::validation::validate_graphics_layer_with_assets;
use cosmic_text::{
    Attrs, Buffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, SwashCache, Weight,
};
use std::fs::{create_dir_all, File};
use std::io::BufWriter;
use std::path::PathBuf;
use tiny_skia::{Color as SkiaColor, Paint, PathBuilder, Pixmap, Rect as SkiaRect, Transform};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsRenderOptions {
    pub output_dir: PathBuf,
}

pub fn render_graphics_preview(
    layer: &GraphicsLayer,
    assets: &AssetRegistry,
    options: GraphicsRenderOptions,
) -> ActionableResult<GraphicsArtifactManifest> {
    validate_graphics_layer_with_assets(layer, assets)?;
    create_dir_all(&options.output_dir).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "outputDir",
            "Graphics output directory could not be created.",
            "Choose a writable generated graphics directory.",
        )
        .with_detail("io", error.to_string())]
    })?;

    let mut pixmap =
        Pixmap::new(layer.dimensions.width, layer.dimensions.height).ok_or_else(|| {
            vec![ActionableError::new(
                GraphicsErrorCode::GraphicsRenderFailed,
                "dimensions",
                "Renderer could not allocate the output pixmap.",
                "Use smaller dimensions or render fewer layers at once.",
            )]
        })?;

    for node in &layer.nodes {
        draw_node(&mut pixmap, node);
    }

    let preview_path = options.output_dir.join("preview.png");
    write_pixmap_png(&pixmap, &preview_path)?;

    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: format!("graphics:{}", layer.id),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count: 1,
        frames_pattern: "preview.png".to_string(),
        preview_path: "preview.png".to_string(),
        source_layer_ids: vec![layer.id.clone()],
        checksums: Default::default(),
    };

    let manifest_path = options.output_dir.join("manifest.json");
    let manifest_json = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "manifest",
            "Graphics manifest could not be serialized.",
            "Check manifest fields for unsupported values.",
        )
        .with_detail("serde", error.to_string())]
    })?;
    std::fs::write(&manifest_path, manifest_json).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "manifest",
            "Graphics manifest could not be written.",
            "Choose a writable generated graphics directory.",
        )
        .with_detail("io", error.to_string())]
    })?;

    Ok(manifest)
}

fn draw_node(pixmap: &mut Pixmap, node: &GraphicNode) {
    match node {
        GraphicNode::RoundedRect(node) => draw_rect(pixmap, &node.box_rect, &node.fill),
        GraphicNode::Rect(node) => draw_rect(pixmap, &node.box_rect, &node.fill),
        GraphicNode::Text(node) => draw_text_node(pixmap, node),
        GraphicNode::Line(node) => {
            if node.points.len() >= 2 {
                let mut builder = PathBuilder::new();
                builder.move_to(node.points[0].x as f32, node.points[0].y as f32);
                for point in node.points.iter().skip(1) {
                    builder.line_to(point.x as f32, point.y as f32);
                }
                if let Some(path) = builder.finish() {
                    let mut paint = Paint::default();
                    paint.set_color(parse_color(&node.stroke));
                    let mut stroke = tiny_skia::Stroke::default();
                    stroke.width = node.stroke_width as f32;
                    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                }
            }
        }
        GraphicNode::ImageRef(_) => {}
    }
}

fn draw_rect(pixmap: &mut Pixmap, rect: &Rect, color: &Color) {
    let Some(skia_rect) = SkiaRect::from_xywh(
        rect.x as f32,
        rect.y as f32,
        rect.width as f32,
        rect.height as f32,
    ) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(parse_color(color));
    pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
}

fn draw_text_node(pixmap: &mut Pixmap, node: &TextNode) {
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    let line_height = (node.font_size * 1.15) as f32;
    let metrics = Metrics::new(node.font_size as f32, line_height);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    {
        let mut borrowed = buffer.borrow_with(&mut font_system);
        borrowed.set_size(
            Some(node.box_rect.width as f32),
            Some(node.box_rect.height as f32),
        );
        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(Weight(node.font_weight));
        borrowed.set_text(&node.text, &attrs, Shaping::Advanced, None);
        borrowed.shape_until_scroll(true);
        let color = text_color(&node.fill);
        let origin_x = node.box_rect.x.round() as i32;
        let origin_y = node.box_rect.y.round() as i32;
        borrowed.draw(&mut swash_cache, color, |x, y, width, height, color| {
            blend_solid_rect(
                pixmap,
                origin_x + x,
                origin_y + y,
                width,
                height,
                color,
            );
        });
    }
}

fn parse_color(color: &Color) -> SkiaColor {
    match color {
        Color::Hex(value) => parse_hex_color(value).unwrap_or_else(|| SkiaColor::from_rgba8(255, 255, 255, 255)),
    }
}

fn parse_hex_color(value: &str) -> Option<SkiaColor> {
    let hex = value.trim().trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        6 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        ),
        8 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(SkiaColor::from_rgba8(r, g, b, a))
}

fn text_color(color: &Color) -> TextColor {
    match color {
        Color::Hex(value) => {
            let hex = value.trim().trim_start_matches('#');
            let (r, g, b, a) = match hex.len() {
                6 => (
                    u8::from_str_radix(&hex[0..2], 16).unwrap_or(255),
                    u8::from_str_radix(&hex[2..4], 16).unwrap_or(255),
                    u8::from_str_radix(&hex[4..6], 16).unwrap_or(255),
                    255,
                ),
                8 => (
                    u8::from_str_radix(&hex[0..2], 16).unwrap_or(255),
                    u8::from_str_radix(&hex[2..4], 16).unwrap_or(255),
                    u8::from_str_radix(&hex[4..6], 16).unwrap_or(255),
                    u8::from_str_radix(&hex[6..8], 16).unwrap_or(255),
                ),
                _ => (255, 255, 255, 255),
            };
            TextColor::rgba(r, g, b, a)
        }
    }
}

fn blend_solid_rect(
    pixmap: &mut Pixmap,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: TextColor,
) {
    let data = pixmap.data_mut();
    let pixmap_width = pixmap.width() as i32;
    let pixmap_height = pixmap.height() as i32;
    let rgba = color.as_rgba_tuple();
    for row in 0..height as i32 {
        let py = y + row;
        if py < 0 || py >= pixmap_height {
            continue;
        }
        for col in 0..width as i32 {
            let px = x + col;
            if px < 0 || px >= pixmap_width {
                continue;
            }
            let offset = ((py * pixmap_width + px) * 4) as usize;
            let alpha = rgba.3 as f32 / 255.0;
            data[offset] = ((rgba.0 as f32 * alpha) + (data[offset] as f32 * (1.0 - alpha))) as u8;
            data[offset + 1] =
                ((rgba.1 as f32 * alpha) + (data[offset + 1] as f32 * (1.0 - alpha))) as u8;
            data[offset + 2] =
                ((rgba.2 as f32 * alpha) + (data[offset + 2] as f32 * (1.0 - alpha))) as u8;
            data[offset + 3] = rgba.3.max(data[offset + 3]);
        }
    }
}

fn write_pixmap_png(pixmap: &Pixmap, path: &PathBuf) -> ActionableResult<()> {
    let file = File::create(path).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "previewPath",
            "Preview PNG could not be created.",
            "Choose a writable generated graphics directory.",
        )
        .with_detail("io", error.to_string())]
    })?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header().map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "previewPath",
            "Preview PNG header could not be written.",
            "Retry with valid RGBA frame dimensions.",
        )
        .with_detail("png", error.to_string())]
    })?;
    png_writer.write_image_data(pixmap.data()).map_err(|error| {
        vec![ActionableError::new(
            GraphicsErrorCode::GraphicsRenderFailed,
            "previewPath",
            "Preview PNG data could not be written.",
            "Retry with valid RGBA frame data.",
        )
        .with_detail("png", error.to_string())]
    })?;
    Ok(())
}
```

This first renderer uses `cosmic-text` for glyph layout and rasterization, then writes the resulting pixels into the same transparent `tiny-skia` pixmap used for vector primitives.

- [ ] **Step 6: Run the render test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml renders_preview_png_and_manifest_for_valid_layer
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
rtk git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/graphics/manifest.rs src-tauri/src/graphics/renderer.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: render graphics preview manifests"
```

## Task 4: Existing Template To Graphics IR Adapter

**Files:**
- Modify: `src-tauri/src/graphics/templates.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Add failing template expansion test**

Append to `src-tauri/tests/graphics.rs`:

```rust
use std::collections::BTreeMap;
use video_creater_lib::edit::render_plan::TemplateRenderLayer;
use video_creater_lib::graphics::templates::template_layer_to_graphics_ir;

#[test]
fn expands_existing_template_layer_to_graphics_ir() {
    let mut fields = BTreeMap::new();
    fields.insert("headline".to_string(), "Olha API".to_string());
    fields.insert("subline".to_string(), "Founder".to_string());
    let layer = TemplateRenderLayer {
        item_id: "template-item-1".to_string(),
        template_id: "kinetic-lower-third-v1".to_string(),
        label: "Kinetic Lower Third".to_string(),
        timeline_start_seconds: 1.2,
        duration_seconds: 2.4,
        fields,
        visual_treatment: "compact lower-third block with translucent backing".to_string(),
        motion: "slide-and-fade in".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
        preview_variant: "lower-third".to_string(),
    };

    let graphics = template_layer_to_graphics_ir(&layer, Dimensions { width: 1920, height: 1080 }, 30.0)
        .expect("template to graphics ir");

    assert_eq!(graphics.id, "template-item-1");
    assert_eq!(graphics.timeline_start, 1.2);
    assert_eq!(graphics.duration_seconds, 2.4);
    assert!(graphics.nodes.iter().any(|node| node.id() == "headline"));
    assert!(graphics.nodes.iter().any(|node| node.id() == "subline"));
}
```

- [ ] **Step 2: Run the failing template test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml expands_existing_template_layer_to_graphics_ir
```

Expected: FAIL because `template_layer_to_graphics_ir` does not exist.

- [ ] **Step 3: Implement template adapter**

Replace `src-tauri/src/graphics/templates.rs` with:

```rust
use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::{
    Color, Dimensions, GraphicNode, GraphicRole, GraphicsLayer, Rect, RoundedRectNode, TextNode,
};
use crate::edit::render_plan::TemplateRenderLayer;

pub fn template_layer_to_graphics_ir(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    match layer.template_id.as_str() {
        "kinetic-lower-third-v1" => lower_third(layer, dimensions, fps),
        "punchy-caption-v1" => punchy_caption(layer, dimensions, fps),
        _ => Err(vec![ActionableError::new(
            GraphicsErrorCode::GraphicsUnsupportedPrimitive,
            "templateId",
            "Template is not supported by the Rust graphics renderer.",
            "Use a supported templateId or expand the graphic as custom IR.",
        )]),
    }
}

fn lower_third(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;
    let subline = required_field(layer, "subline")?;
    Ok(base_layer(
        layer,
        dimensions,
        fps,
        GraphicRole::LowerThird,
        vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: Rect {
                    x: 96.0,
                    y: 760.0,
                    width: 720.0,
                    height: 150.0,
                },
                radius: 24.0,
                fill: Color::Hex("#071013cc".to_string()),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline,
                box_rect: Rect {
                    x: 132.0,
                    y: 790.0,
                    width: 648.0,
                    height: 54.0,
                },
                font_size: 42.0,
                font_weight: 800,
                align: "start".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(1),
            }),
            GraphicNode::Text(TextNode {
                id: "subline".to_string(),
                text: subline,
                box_rect: Rect {
                    x: 132.0,
                    y: 846.0,
                    width: 648.0,
                    height: 36.0,
                },
                font_size: 24.0,
                font_weight: 600,
                align: "start".to_string(),
                fill: Color::Hex("#d8f3dc".to_string()),
                max_lines: Some(1),
            }),
        ],
    ))
}

fn punchy_caption(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
) -> ActionableResult<GraphicsLayer> {
    let headline = required_field(layer, "headline")?;
    Ok(base_layer(
        layer,
        dimensions,
        fps,
        GraphicRole::Overlay,
        vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "caption-backing".to_string(),
                box_rect: Rect {
                    x: 360.0,
                    y: 720.0,
                    width: 1200.0,
                    height: 150.0,
                },
                radius: 28.0,
                fill: Color::Hex("#050505b8".to_string()),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: headline,
                box_rect: Rect {
                    x: 420.0,
                    y: 758.0,
                    width: 1080.0,
                    height: 78.0,
                },
                font_size: 58.0,
                font_weight: 900,
                align: "center".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(1),
            }),
        ],
    ))
}

fn base_layer(
    layer: &TemplateRenderLayer,
    dimensions: Dimensions,
    fps: f64,
    role: GraphicRole,
    nodes: Vec<GraphicNode>,
) -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: layer.item_id.clone(),
        role,
        timeline_start: layer.timeline_start_seconds,
        duration_seconds: layer.duration_seconds,
        dimensions,
        fps,
        alpha: true,
        source_beat: layer.label.clone(),
        visual_treatment: layer.visual_treatment.clone(),
        motion: layer.motion.clone(),
        safe_zone: layer.safe_zone.clone(),
        avoid: layer.avoid.clone(),
        nodes,
    }
}

fn required_field(layer: &TemplateRenderLayer, key: &str) -> Result<String, Vec<ActionableError>> {
    layer
        .fields
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            vec![ActionableError::new(
                GraphicsErrorCode::GraphicsTemplateParamMissing,
                format!("fields.{key}"),
                "Template field is missing.",
                format!("Set fields.{key} to a non-empty string."),
            )]
        })
}
```

- [ ] **Step 4: Run template test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml expands_existing_template_layer_to_graphics_ir
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/graphics/templates.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: expand templates to graphics ir"
```

## Task 5: `ffmpeg` Command Support For Graphics Manifests

**Files:**
- Modify: `src-tauri/src/edit/render_plan.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Add failing ffmpeg manifest command test**

Append to `src-tauri/tests/graphics.rs`:

```rust
use video_creater_lib::edit::render_plan::{
    build_ffmpeg_render_command_with_graphics, GraphicsOverlayInput, RenderClip, RenderPlan,
};

#[test]
fn ffmpeg_command_can_overlay_graphics_manifest_sequence() {
    let plan = RenderPlan {
        input_path: "input.mp4".to_string(),
        output_path: "output.mp4".to_string(),
        width: 640,
        height: 360,
        fps: 30.0,
        clips: vec![RenderClip {
            source_in: 0.0,
            source_out: 3.0,
        }],
    };
    let graphics = vec![GraphicsOverlayInput {
        frames_pattern: "generated/graphics/layer/preview.png".to_string(),
        timeline_start_seconds: 0.5,
        duration_seconds: 1.0,
        fps: 30.0,
    }];

    let command = build_ffmpeg_render_command_with_graphics(&plan, &graphics)
        .expect("ffmpeg command with graphics");

    assert!(command.args.iter().any(|arg| arg == "generated/graphics/layer/preview.png"));
    assert!(command.args.iter().any(|arg| arg.contains("overlay=0:0")));
    assert!(command.args.iter().any(|arg| arg.contains("between(t,0.500,1.500)")));
}
```

- [ ] **Step 2: Run the failing ffmpeg command test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml ffmpeg_command_can_overlay_graphics_manifest_sequence
```

Expected: FAIL because `GraphicsOverlayInput` and `build_ffmpeg_render_command_with_graphics` do not exist.

- [ ] **Step 3: Add graphics overlay input and command builder**

Append these types and function near `RenderPlan` in `src-tauri/src/edit/render_plan.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsOverlayInput {
    pub frames_pattern: String,
    pub timeline_start_seconds: f64,
    pub duration_seconds: f64,
    pub fps: f64,
}
```

Append this function after `build_ffmpeg_render_command`:

```rust
pub fn build_ffmpeg_render_command_with_graphics(
    plan: &RenderPlan,
    graphics: &[GraphicsOverlayInput],
) -> Result<FfmpegCommand, RenderPlanError> {
    let base_command = build_ffmpeg_render_command(plan)?;
    if graphics.is_empty() {
        return Ok(base_command);
    }

    for overlay in graphics {
        if overlay.frames_pattern.trim().is_empty()
            || !overlay.timeline_start_seconds.is_finite()
            || overlay.timeline_start_seconds < 0.0
            || !overlay.duration_seconds.is_finite()
            || overlay.duration_seconds <= 0.0
            || !overlay.fps.is_finite()
            || overlay.fps <= 0.0
        {
            return Err(RenderPlanError::InvalidTemplateLayer(
                "graphics overlay input is invalid",
            ));
        }
    }

    let mut args = vec![
        "-hide_banner".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        plan.input_path.clone(),
    ];
    for overlay in graphics {
        args.push("-loop".to_string());
        args.push("1".to_string());
        args.push("-framerate".to_string());
        args.push(format!("{:.3}", overlay.fps));
        args.push("-i".to_string());
        args.push(overlay.frames_pattern.clone());
    }

    let mut filter_parts = Vec::new();
    let mut concat_inputs = String::new();
    for (index, clip) in plan.clips.iter().enumerate() {
        if !clip.source_in.is_finite()
            || !clip.source_out.is_finite()
            || clip.source_in < 0.0
            || clip.source_out <= clip.source_in
        {
            return Err(RenderPlanError::InvalidClipRange);
        }
        filter_parts.push(format!(
            "[0:v]trim=start={:.3}:end={:.3},setpts=PTS-STARTPTS,scale={}:{},fps={:.3}[v{}]",
            clip.source_in, clip.source_out, plan.width, plan.height, plan.fps, index
        ));
        filter_parts.push(format!(
            "[0:a]atrim=start={:.3}:end={:.3},asetpts=PTS-STARTPTS[a{}]",
            clip.source_in, clip.source_out, index
        ));
        concat_inputs.push_str(&format!("[v{index}][a{index}]"));
    }
    filter_parts.push(format!(
        "{}concat=n={}:v=1:a=1[basev][aout]",
        concat_inputs,
        plan.clips.len()
    ));

    let mut current_video = "basev".to_string();
    for (index, overlay) in graphics.iter().enumerate() {
        let input_index = index + 1;
        let output_label = format!("gv{index}");
        let end = overlay.timeline_start_seconds + overlay.duration_seconds;
        filter_parts.push(format!(
            "[{}][{}:v]overlay=0:0:format=auto:enable='between(t,{:.3},{:.3})'[{}]",
            current_video, input_index, overlay.timeline_start_seconds, end, output_label
        ));
        current_video = output_label;
    }
    filter_parts.push(format!("[{}]format=yuv420p[vout]", current_video));

    args.extend([
        "-filter_complex".to_string(),
        filter_parts.join(";"),
        "-map".to_string(),
        "[vout]".to_string(),
        "-map".to_string(),
        "[aout]".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        plan.output_path.clone(),
    ]);

    Ok(FfmpegCommand {
        program: "ffmpeg".to_string(),
        args,
    })
}
```

- [ ] **Step 4: Run ffmpeg command tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_plan_builds_ffmpeg_trim_concat_shape ffmpeg_command_can_overlay_graphics_manifest_sequence
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/edit/render_plan.rs src-tauri/tests/graphics.rs
rtk git commit -m "feat: add graphics overlay render command"
```

## Task 6: End-To-End Rust Graphics Slice Verification

**Files:**
- Modify: `src-tauri/tests/graphics.rs`
- No production code changes expected unless the test exposes a defect.

- [ ] **Step 1: Add end-to-end slice test**

Append to `src-tauri/tests/graphics.rs`:

```rust
#[test]
fn template_layer_renders_manifest_that_ffmpeg_backend_can_consume() {
    let dir = tempdir().expect("tempdir");
    let mut fields = BTreeMap::new();
    fields.insert("headline".to_string(), "Olha API".to_string());
    fields.insert("subline".to_string(), "Founder".to_string());
    let template = TemplateRenderLayer {
        item_id: "template-item-1".to_string(),
        template_id: "kinetic-lower-third-v1".to_string(),
        label: "Kinetic Lower Third".to_string(),
        timeline_start_seconds: 0.5,
        duration_seconds: 1.0,
        fields,
        visual_treatment: "compact lower-third block with translucent backing".to_string(),
        motion: "slide-and-fade in".to_string(),
        safe_zone: "keep essential text inside 10% margins".to_string(),
        avoid: "full-width opaque black slabs".to_string(),
        preview_variant: "lower-third".to_string(),
    };
    let graphics = template_layer_to_graphics_ir(
        &template,
        Dimensions {
            width: 640,
            height: 360,
        },
        30.0,
    )
    .expect("template ir");
    let manifest = render_graphics_preview(
        &graphics,
        &AssetRegistry::new(dir.path().to_path_buf()),
        GraphicsRenderOptions {
            output_dir: dir.path().join("graphics/template-item-1"),
        },
    )
    .expect("render graphics");

    let plan = RenderPlan {
        input_path: "input.mp4".to_string(),
        output_path: "output.mp4".to_string(),
        width: 640,
        height: 360,
        fps: 30.0,
        clips: vec![RenderClip {
            source_in: 0.0,
            source_out: 3.0,
        }],
    };
    let overlays = vec![GraphicsOverlayInput {
        frames_pattern: dir
            .path()
            .join("graphics/template-item-1")
            .join(&manifest.preview_path)
            .to_string_lossy()
            .to_string(),
        timeline_start_seconds: graphics.timeline_start,
        duration_seconds: graphics.duration_seconds,
        fps: graphics.fps,
    }];

    let command = build_ffmpeg_render_command_with_graphics(&plan, &overlays)
        .expect("ffmpeg command");

    assert_eq!(manifest.source_layer_ids, vec!["template-item-1"]);
    assert!(command.args.iter().any(|arg| arg.contains("preview.png")));
    assert!(command.args.iter().any(|arg| arg.contains("between(t,0.500,1.500)")));
}
```

- [ ] **Step 2: Run the end-to-end slice test**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml template_layer_renders_manifest_that_ffmpeg_backend_can_consume
```

Expected: PASS.

- [ ] **Step 3: Run full Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 4: Run full project verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS for TypeScript lint, Vitest, and Rust tests.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/tests/graphics.rs
rtk git commit -m "test: cover rust graphics render slice"
```

## Task 7: Plan Completion Review

**Files:**
- Inspect changed files only.

- [ ] **Step 1: Check git status**

Run:

```bash
rtk git status --short
```

Expected: no unstaged changes after the final commit.

- [ ] **Step 2: Inspect recent commits**

Run:

```bash
rtk git log --oneline -6
```

Expected: recent commits include:

```text
test: cover rust graphics render slice
feat: add graphics overlay render command
feat: expand templates to graphics ir
feat: render graphics preview manifests
feat: validate graphics image refs
feat: add graphics ir validation
```

- [ ] **Step 3: Record follow-up notes**

If all verification passed, report these follow-ups without implementing them in this slice:

```text
- Generate full frame sequences for animated enter/hold/exit phases.
- Move the Node demo render harness to the Rust graphics path.
- Add UI preview and concise error display for agent-authored graphics proposals.
- Add text stroke, shadow, and advanced font fallback controls.
```

Do not add these follow-ups to code as comments.

## Self-Review Against Spec

Spec coverage:

- Rust-owned IR: Task 1.
- Agent-friendly declarative data model: Task 1 and Task 4.
- `imageRef` from v1: Task 2.
- Concise actionable errors: Task 1 and Task 2.
- PNG preview/manifest output: Task 3.
- Existing template compatibility: Task 4.
- `ffmpeg` retained behind backend-style boundary: Task 5.
- End-to-end first slice verification: Task 6.

Known deliberate limits in this first slice:

- Text uses basic `cosmic-text` sans-serif layout without stroke, shadow, or font selection controls.
- Full animation frame sequences are not generated in this slice.
- UI proposal preview is not changed in this slice.
- `ffmpeg` remains the only implemented video backend.
