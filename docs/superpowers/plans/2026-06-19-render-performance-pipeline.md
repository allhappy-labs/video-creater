# Render Performance Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add render performance observability, conservative graphics artifact caching, and explicit draft-vs-final quality metadata to the Rust proposal render pipeline.

**Architecture:** Add small backend-neutral modules for performance timing, cache validation, and quality settings. Integrate them into the existing proposal render path without changing the Codex proposal schema or canonical project mutation rules. Keep report fields optional where needed so existing frontend and sample reports remain compatible.

**Tech Stack:** Rust, serde, GStreamer/GES backend boundary, TypeScript render model, Vitest, Cargo tests.

---

## File Structure

- Modify `src-tauri/src/render_pipeline/report.rs`: add `RenderPerformanceSummary`, `RenderStageReport`, optional `performance` on `RenderReport`, and Markdown performance rendering.
- Create `src-tauri/src/render_pipeline/performance.rs`: timed stage helper and details utilities.
- Create `src-tauri/src/render_pipeline/graphics_cache.rs`: cache fingerprinting, metadata read/write, manifest/frame validation, cache hit/miss result types.
- Create `src-tauri/src/render_pipeline/quality.rs`: effective render quality settings for draft and final WebM.
- Modify `src-tauri/src/render_pipeline/mod.rs`: export new modules.
- Modify `src-tauri/src/render_pipeline/proposal.rs`: measure proposal render stages, use graphics cache, include performance and quality details in render reports.
- Modify `src/lib/render.ts`: add optional frontend performance report types.
- Modify `src/lib/render.test.ts`: verify optional performance data shape and backward compatibility.
- Modify `src-tauri/tests/render_pipeline.rs`: add Rust unit/integration tests for report serialization, Markdown, performance helper, cache validation, and quality settings.

---

### Task 1: Report Performance Model

**Files:**
- Modify: `src-tauri/src/render_pipeline/report.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [x] **Step 1: Write failing Rust report serialization test**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn render_report_serializes_optional_performance_summary() {
    let report = RenderReport {
        job_id: "perf-report".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: Some(2.5),
            output_path: Some("renders/out.webm".to_string()),
        },
        command: CommandSpec::new("gstreamer-ges").arg("--quality=draftWebm"),
        stdout: String::new(),
        stderr: String::new(),
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: Vec::new(),
        performance: Some(RenderPerformanceSummary {
            total_duration_ms: 42,
            stages: vec![RenderStageReport {
                name: "sourceProbe".to_string(),
                status: "succeeded".to_string(),
                duration_ms: 7,
                details: BTreeMap::from([("path".to_string(), "source.webm".to_string())]),
            }],
        }),
    };

    let json = serde_json::to_value(&report).expect("report serializes");

    assert_eq!(json["performance"]["totalDurationMs"], 42);
    assert_eq!(json["performance"]["stages"][0]["name"], "sourceProbe");
    assert_eq!(json["performance"]["stages"][0]["details"]["path"], "source.webm");
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_report_serializes_optional_performance_summary --test render_pipeline -- --test-threads=1
```

Expected: compile failure because `RenderPerformanceSummary`, `RenderStageReport`, and `RenderReport.performance` do not exist.

- [x] **Step 3: Add report model types**

In `src-tauri/src/render_pipeline/report.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderPerformanceSummary {
    pub total_duration_ms: u128,
    pub stages: Vec<RenderStageReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderStageReport {
    pub name: String,
    pub status: String,
    pub duration_ms: u128,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}
```

Import `BTreeMap` at the top of `report.rs`:

```rust
use std::collections::BTreeMap;
```

Add this field to `RenderReport`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub performance: Option<RenderPerformanceSummary>,
```

- [x] **Step 4: Run test to verify it passes**

Run the same cargo test command. Expected: PASS.

- [x] **Step 5: Write failing Markdown performance test**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn markdown_report_includes_performance_section() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("render-report.md");
    let report = RenderReport {
        job_id: "perf-markdown".to_string(),
        summary: RenderReportSummary {
            status: "succeeded".to_string(),
            duration_seconds: None,
            output_path: None,
        },
        command: CommandSpec::new("gstreamer-ges"),
        stdout: String::new(),
        stderr: String::new(),
        errors: Vec::new(),
        artifacts: Vec::new(),
        graphics: Vec::new(),
        performance: Some(RenderPerformanceSummary {
            total_duration_ms: 125,
            stages: vec![RenderStageReport {
                name: "graphics".to_string(),
                status: "succeeded".to_string(),
                duration_ms: 90,
                details: BTreeMap::from([("cache".to_string(), "miss".to_string())]),
            }],
        }),
    };

    write_markdown_report(&path, &report).expect("write markdown report");
    let markdown = std::fs::read_to_string(path).expect("read markdown");

    assert!(markdown.contains("## Performance"));
    assert!(markdown.contains("- Total: 125ms"));
    assert!(markdown.contains("| graphics | succeeded | 90ms | cache=miss |"));
}
```

- [x] **Step 6: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml markdown_report_includes_performance_section --test render_pipeline -- --test-threads=1
```

Expected: FAIL because Markdown does not include performance.

- [x] **Step 7: Add Markdown rendering**

In `markdown_report`, after the command section and before artifacts, add:

```rust
if let Some(performance) = &report.performance {
    markdown.push_str("\n## Performance\n\n");
    markdown.push_str(&format!("- Total: {}ms\n\n", performance.total_duration_ms));
    markdown.push_str("| Stage | Status | Duration | Details |\n");
    markdown.push_str("| --- | --- | ---: | --- |\n");
    for stage in &performance.stages {
        let details = stage
            .details
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        markdown.push_str(&format!(
            "| {} | {} | {}ms | {} |\n",
            stage.name, stage.status, stage.duration_ms, details
        ));
    }
}
```

- [x] **Step 8: Run Markdown test to verify it passes**

Run the same cargo test command. Expected: PASS.

---

### Task 2: Performance Timing Helper

**Files:**
- Create: `src-tauri/src/render_pipeline/performance.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [x] **Step 1: Write failing success/failure timer tests**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn performance_recorder_records_successful_stage() {
    let mut recorder = RenderPerformanceRecorder::start();

    let value = recorder
        .measure_stage("sourceProbe", BTreeMap::from([("path".to_string(), "in.webm".to_string())]), || {
            Ok::<_, Vec<PipelineError>>(7)
        })
        .expect("stage should succeed");

    let summary = recorder.finish();

    assert_eq!(value, 7);
    assert_eq!(summary.stages.len(), 1);
    assert_eq!(summary.stages[0].name, "sourceProbe");
    assert_eq!(summary.stages[0].status, "succeeded");
    assert_eq!(summary.stages[0].details["path"], "in.webm");
}

#[test]
fn performance_recorder_records_failed_stage_without_rewriting_error() {
    let mut recorder = RenderPerformanceRecorder::start();
    let error = PipelineError::new(
        PipelineErrorCode::RenderBackendFailed,
        "stage",
        "stage failed",
        "fix stage",
    );

    let errors = recorder
        .measure_stage("gesRender", BTreeMap::new(), || Err::<(), _>(vec![error.clone()]))
        .expect_err("stage should fail");
    let summary = recorder.finish();

    assert_eq!(errors, vec![error]);
    assert_eq!(summary.stages.len(), 1);
    assert_eq!(summary.stages[0].name, "gesRender");
    assert_eq!(summary.stages[0].status, "failed");
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml performance_recorder --test render_pipeline -- --test-threads=1
```

Expected: compile failure because `RenderPerformanceRecorder` does not exist.

- [x] **Step 3: Implement performance module**

Create `src-tauri/src/render_pipeline/performance.rs`:

```rust
use std::collections::BTreeMap;
use std::time::Instant;

use super::error::PipelineResult;
use super::report::{RenderPerformanceSummary, RenderStageReport};

#[derive(Debug)]
pub struct RenderPerformanceRecorder {
    started_at: Instant,
    stages: Vec<RenderStageReport>,
}

impl RenderPerformanceRecorder {
    pub fn start() -> Self {
        Self {
            started_at: Instant::now(),
            stages: Vec::new(),
        }
    }

    pub fn measure_stage<T, F>(
        &mut self,
        name: impl Into<String>,
        details: BTreeMap<String, String>,
        stage: F,
    ) -> PipelineResult<T>
    where
        F: FnOnce() -> PipelineResult<T>,
    {
        let name = name.into();
        let started_at = Instant::now();
        match stage() {
            Ok(value) => {
                self.stages.push(RenderStageReport {
                    name,
                    status: "succeeded".to_string(),
                    duration_ms: started_at.elapsed().as_millis(),
                    details,
                });
                Ok(value)
            }
            Err(errors) => {
                self.stages.push(RenderStageReport {
                    name,
                    status: "failed".to_string(),
                    duration_ms: started_at.elapsed().as_millis(),
                    details,
                });
                Err(errors)
            }
        }
    }

    pub fn push_stage(&mut self, stage: RenderStageReport) {
        self.stages.push(stage);
    }

    pub fn finish(self) -> RenderPerformanceSummary {
        RenderPerformanceSummary {
            total_duration_ms: self.started_at.elapsed().as_millis(),
            stages: self.stages,
        }
    }
}
```

Modify `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod performance;
```

Import in `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::render_pipeline::performance::RenderPerformanceRecorder;
use video_creater_lib::render_pipeline::report::{RenderPerformanceSummary, RenderStageReport};
```

- [x] **Step 4: Run tests to verify they pass**

Run the same cargo test command. Expected: PASS.

---

### Task 3: Graphics Cache Validation

**Files:**
- Create: `src-tauri/src/render_pipeline/graphics_cache.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [x] **Step 1: Write failing cache tests**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn graphics_cache_misses_when_manifest_is_missing() {
    let temp = tempfile::tempdir().expect("temp dir");
    let layer = sample_cache_graphics_layer("cache-layer");
    let fingerprint = graphics_layer_fingerprint(&layer, "rust", None).expect("fingerprint");

    let result = validate_graphics_cache_hit(temp.path(), &layer, "rust", None, &fingerprint)
        .expect("cache miss is not an error");

    assert_eq!(result, GraphicsCacheLookup::Miss("metadata missing".to_string()));
}

#[test]
fn graphics_cache_hits_complete_matching_artifact() {
    let temp = tempfile::tempdir().expect("temp dir");
    let layer = sample_cache_graphics_layer("cache-layer");
    let fingerprint = graphics_layer_fingerprint(&layer, "rust", None).expect("fingerprint");
    let manifest = GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "cache-layer.preview".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: layer.dimensions.clone(),
        fps: layer.fps,
        duration_seconds: layer.duration_seconds,
        alpha: layer.alpha,
        frame_count: 1,
        frames_pattern: "frames/frame-%06d.png".to_string(),
        preview_path: PathBuf::from("preview.png"),
        source_layer_ids: vec![layer.id.clone()],
        checksums: BTreeMap::new(),
    };
    let frames_dir = temp.path().join("frames");
    std::fs::create_dir_all(&frames_dir).expect("frames dir");
    std::fs::write(temp.path().join("preview.png"), b"preview").expect("preview");
    std::fs::write(frames_dir.join("frame-000000.png"), b"frame").expect("frame");
    write_graphics_cache_metadata(temp.path(), &fingerprint, "rust", None).expect("metadata");
    std::fs::write(
        temp.path().join("manifest.json"),
        serde_json::to_string_pretty(&manifest).expect("manifest json"),
    )
    .expect("manifest");

    let result = validate_graphics_cache_hit(temp.path(), &layer, "rust", None, &fingerprint)
        .expect("cache lookup");

    assert_eq!(result, GraphicsCacheLookup::Hit(manifest));
}

#[test]
fn graphics_cache_misses_on_stale_fingerprint() {
    let temp = tempfile::tempdir().expect("temp dir");
    let layer = sample_cache_graphics_layer("cache-layer");
    write_graphics_cache_metadata(temp.path(), "stale", "rust", None).expect("metadata");

    let result = validate_graphics_cache_hit(temp.path(), &layer, "rust", None, "fresh")
        .expect("cache lookup");

    assert_eq!(result, GraphicsCacheLookup::Miss("fingerprint mismatch".to_string()));
}
```

Also add helper in the test file:

```rust
fn sample_cache_graphics_layer(id: &str) -> GraphicsLayer {
    GraphicsLayer {
        schema_version: 1,
        id: id.to_string(),
        role: GraphicRole::Overlay,
        timeline_start: 0.0,
        duration_seconds: 1.0,
        dimensions: Dimensions {
            width: 320,
            height: 180,
        },
        fps: 30.0,
        alpha: true,
        source_beat: "cache beat".to_string(),
        visual_treatment: "cache test visual".to_string(),
        motion: "static".to_string(),
        safe_zone: "inside frame".to_string(),
        avoid: "none".to_string(),
        nodes: Vec::new(),
    }
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml graphics_cache --test render_pipeline -- --test-threads=1
```

Expected: compile failure because cache module and imports do not exist.

- [x] **Step 3: Implement graphics cache module**

Create `src-tauri/src/render_pipeline/graphics_cache.rs` with:

```rust
use crate::graphics::error::{ActionableError, GraphicsErrorCode};
use crate::graphics::ir::GraphicsLayer;
use crate::graphics::manifest::GraphicsArtifactManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const CACHE_METADATA_FILE_NAME: &str = "cache-metadata.json";
const GRAPHICS_MANIFEST_FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsCacheMetadata {
    pub schema_version: u32,
    pub fingerprint: String,
    pub renderer: String,
    pub quality_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsCacheLookup {
    Hit(GraphicsArtifactManifest),
    Miss(String),
}

pub fn graphics_layer_fingerprint(
    layer: &GraphicsLayer,
    renderer: &str,
    quality_profile: Option<&str>,
) -> Result<String, serde_json::Error> {
    let payload = serde_json::json!({
        "layer": layer,
        "renderer": renderer,
        "qualityProfile": quality_profile,
    });
    let bytes = serde_json::to_vec(&payload)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

pub fn write_graphics_cache_metadata(
    artifact_dir: &Path,
    fingerprint: &str,
    renderer: &str,
    quality_profile: Option<&str>,
) -> Result<(), std::io::Error> {
    let metadata = GraphicsCacheMetadata {
        schema_version: 1,
        fingerprint: fingerprint.to_string(),
        renderer: renderer.to_string(),
        quality_profile: quality_profile.map(str::to_string),
    };
    let json = serde_json::to_string_pretty(&metadata)
        .map_err(std::io::Error::other)?;
    std::fs::write(artifact_dir.join(CACHE_METADATA_FILE_NAME), json)
}

pub fn validate_graphics_cache_hit(
    artifact_dir: &Path,
    layer: &GraphicsLayer,
    renderer: &str,
    quality_profile: Option<&str>,
    expected_fingerprint: &str,
) -> Result<GraphicsCacheLookup, std::io::Error> {
    let metadata_path = artifact_dir.join(CACHE_METADATA_FILE_NAME);
    if !metadata_path.is_file() {
        return Ok(GraphicsCacheLookup::Miss("metadata missing".to_string()));
    }
    let metadata_json = std::fs::read_to_string(&metadata_path)?;
    let metadata: GraphicsCacheMetadata = serde_json::from_str(&metadata_json)
        .map_err(std::io::Error::other)?;
    if metadata.schema_version != 1 {
        return Ok(GraphicsCacheLookup::Miss("metadata schema mismatch".to_string()));
    }
    if metadata.fingerprint != expected_fingerprint {
        return Ok(GraphicsCacheLookup::Miss("fingerprint mismatch".to_string()));
    }
    if metadata.renderer != renderer || metadata.quality_profile.as_deref() != quality_profile {
        return Ok(GraphicsCacheLookup::Miss("renderer mismatch".to_string()));
    }

    let manifest_path = artifact_dir.join(GRAPHICS_MANIFEST_FILE_NAME);
    if !manifest_path.is_file() {
        return Ok(GraphicsCacheLookup::Miss("manifest missing".to_string()));
    }
    let manifest_json = std::fs::read_to_string(&manifest_path)?;
    let manifest: GraphicsArtifactManifest = serde_json::from_str(&manifest_json)
        .map_err(std::io::Error::other)?;

    if manifest.dimensions != layer.dimensions
        || (manifest.fps - layer.fps).abs() > 0.001
        || (manifest.duration_seconds - layer.duration_seconds).abs() > 0.001
        || manifest.alpha != layer.alpha
        || manifest.frame_count == 0
    {
        return Ok(GraphicsCacheLookup::Miss("manifest contract mismatch".to_string()));
    }

    if !nonempty_file(&artifact_dir.join(&manifest.preview_path)) {
        return Ok(GraphicsCacheLookup::Miss("preview missing".to_string()));
    }
    for frame_index in 0..manifest.frame_count {
        if !nonempty_file(&graphics_frame_path(&manifest, artifact_dir, frame_index)) {
            return Ok(GraphicsCacheLookup::Miss("frame missing".to_string()));
        }
    }

    Ok(GraphicsCacheLookup::Hit(manifest))
}

fn nonempty_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

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

Add dependency to `src-tauri/Cargo.toml`:

```toml
sha2 = "0.10.9"
```

Modify `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod graphics_cache;
```

Add test imports:

```rust
use video_creater_lib::graphics::ir::{Dimensions, GraphicNode, GraphicRole, GraphicsLayer};
use video_creater_lib::render_pipeline::graphics_cache::{
    graphics_layer_fingerprint, validate_graphics_cache_hit, write_graphics_cache_metadata,
    GraphicsCacheLookup,
};
```

- [x] **Step 4: Run tests to verify they pass**

Run the same cargo test command. Expected: PASS.

---

### Task 4: Draft Quality Settings

**Files:**
- Create: `src-tauri/src/render_pipeline/quality.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [x] **Step 1: Write failing quality tests**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn draft_quality_settings_cap_width_and_fps() {
    let settings = effective_quality_settings(RenderQualityProfile::DraftWebm, 1920, 1080, 60.0);

    assert_eq!(settings.output_width, 960);
    assert_eq!(settings.output_height, 540);
    assert_eq!(settings.output_fps, 24.0);
    assert_eq!(settings.speed_hint, "draft-fast");
}

#[test]
fn final_quality_settings_preserve_source_dimensions_and_fps() {
    let settings = effective_quality_settings(RenderQualityProfile::FinalWebm, 1920, 1080, 60.0);

    assert_eq!(settings.output_width, 1920);
    assert_eq!(settings.output_height, 1080);
    assert_eq!(settings.output_fps, 60.0);
    assert_eq!(settings.speed_hint, "final-quality");
}
```

- [x] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml quality_settings --test render_pipeline -- --test-threads=1
```

Expected: compile failure because `effective_quality_settings` does not exist.

- [x] **Step 3: Implement quality module**

Create `src-tauri/src/render_pipeline/quality.rs`:

```rust
use crate::edit::render_plan::RenderQualityProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderQualitySettings {
    pub output_width: u32,
    pub output_height: u32,
    pub output_fps: f64,
    pub video_bitrate_kbps: Option<u32>,
    pub speed_hint: String,
}

pub fn effective_quality_settings(
    profile: RenderQualityProfile,
    width: u32,
    height: u32,
    fps: f64,
) -> RenderQualitySettings {
    match profile {
        RenderQualityProfile::DraftWebm => draft_settings(width, height, fps),
        RenderQualityProfile::FinalWebm => RenderQualitySettings {
            output_width: width,
            output_height: height,
            output_fps: fps,
            video_bitrate_kbps: None,
            speed_hint: "final-quality".to_string(),
        },
    }
}

fn draft_settings(width: u32, height: u32, fps: f64) -> RenderQualitySettings {
    let capped_width = width.min(960).max(1);
    let scale = capped_width as f64 / width.max(1) as f64;
    let capped_height = ((height.max(1) as f64 * scale).round() as u32).max(1);
    RenderQualitySettings {
        output_width: capped_width,
        output_height: capped_height,
        output_fps: if fps.is_finite() && fps > 24.0 { 24.0 } else { fps },
        video_bitrate_kbps: Some(1_200),
        speed_hint: "draft-fast".to_string(),
    }
}
```

Modify `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod quality;
```

Add test import:

```rust
use video_creater_lib::render_pipeline::quality::effective_quality_settings;
```

- [x] **Step 4: Run tests to verify they pass**

Run the same cargo test command. Expected: PASS.

---

### Task 5: Integrate Observability, Cache, And Quality Into Proposal Render

**Files:**
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [x] **Step 1: Write failing proposal preview/report tests**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn render_proposal_preview_reports_effective_draft_quality_details() {
    let project = sample_project_for_proposal();
    let request = sample_proposal_request();
    let report = CodexProposalReport {
        generated_at: "2026-06-19T00:00:00Z".to_string(),
        proposal: sample_codex_proposal(),
    };
    let config = RenderProposalConfig {
        project_root: PathBuf::from("."),
        source_video_path: PathBuf::from("source.webm"),
        codex_report_path: PathBuf::from("codex-report.json"),
        output_dir: PathBuf::from("renders"),
        final_name: "draft.webm".to_string(),
    };

    let preview = build_render_proposal_preview(&project, &request, &report, &config)
        .expect("preview should build");

    assert_eq!(preview.quality_profile, "draftWebm");
    assert_eq!(preview.effective_width, 960);
    assert_eq!(preview.effective_height, 540);
    assert_eq!(preview.effective_fps, 24.0);
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_proposal_preview_reports_effective_draft_quality_details --test render_pipeline -- --test-threads=1
```

Expected: compile failure because preview quality fields do not exist.

- [x] **Step 3: Add preview quality fields**

In `RenderProposalPreview`, add:

```rust
pub quality_profile: String,
pub effective_width: u32,
pub effective_height: u32,
pub effective_fps: f64,
```

In `build_render_proposal_preview`, compute:

```rust
let quality_settings = effective_quality_settings(
    plan.quality_profile.clone(),
    plan.width,
    plan.height,
    plan.fps,
);
```

Return the new fields:

```rust
quality_profile: quality_profile_command_value(&plan.quality_profile).to_string(),
effective_width: quality_settings.output_width,
effective_height: quality_settings.output_height,
effective_fps: quality_settings.output_fps,
```

If `quality_profile_command_value` is private to `gstreamer_backend.rs`, add a local helper in `proposal.rs`:

```rust
fn render_quality_profile_value(profile: &RenderQualityProfile) -> &'static str {
    match profile {
        RenderQualityProfile::DraftWebm => "draftWebm",
        RenderQualityProfile::FinalWebm => "finalWebm",
    }
}
```

- [x] **Step 4: Run preview test to verify it passes**

Run the same cargo test command. Expected: PASS.

- [x] **Step 5: Integrate recorder and cache into `run_render_proposal_with_runner`**

Use `RenderPerformanceRecorder::start()` at the start of `run_render_proposal_with_runner`.

Wrap these stages:

```rust
let report_json = recorder.measure_stage("readReport", BTreeMap::new(), || read_codex_report(config))?;
let (source_probe, source_probe_output) = recorder.measure_stage("sourceProbe", details, || {
    probe_media_with_gstreamer(&config.source_video_path, PROCESS_TIMEOUT, "sourceVideo")
})?;
let plan = recorder.measure_stage("buildRenderPlan", details, || proposal_to_render_plan(...))?;
let graphics = recorder.measure_stage("graphics", details, || render_proposal_graphics_artifacts(...))?;
let render_output = recorder.measure_stage("gesRender", details, || backend.render(...))?;
let (final_probe, final_probe_output) = recorder.measure_stage("finalProbe", details, || {
    probe_media_with_gstreamer(&final_path, PROCESS_TIMEOUT, "final")
})?;
```

Finish before report construction:

```rust
let performance = Some(recorder.finish());
```

Set `RenderReport.performance` to that value.

In `render_proposal_graphics_artifacts_with_gpu_renderer`, check cache before rendering CPU layers. On hit, push `RenderedGraphicsArtifact` with the cached manifest. On miss, render and then write cache metadata. Use details strings in `RenderedGraphicsArtifact`:

```rust
cache_status: Option<String>,
```

Record `cache_status` in `RenderGraphicsReport` or in performance stage details.

- [x] **Step 6: Run focused proposal tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_proposal_preview_reports_effective_draft_quality_details --test render_pipeline -- --test-threads=1
```

Expected: PASS.

---

### Task 6: Frontend Optional Performance Types

**Files:**
- Modify: `src/lib/render.ts`
- Modify: `src/lib/render.test.ts`

- [x] **Step 1: Write failing TypeScript model test**

In `src/lib/render.test.ts`, add:

```ts
it("accepts optional render performance metadata", () => {
  const report: RenderReport = {
    jobId: "render-proposal",
    summary: {
      status: "succeeded",
      durationSeconds: 5.6,
      outputPath: "renders/draft.webm",
    },
    command: {
      program: "gstreamer-ges",
      args: ["--quality=draftWebm"],
    },
    stdout: "",
    stderr: "",
    errors: [],
    artifacts: [],
    graphics: [],
    performance: {
      totalDurationMs: 125,
      stages: [
        {
          name: "graphics",
          status: "succeeded",
          durationMs: 40,
          details: { cache: "hit" },
        },
      ],
    },
  };

  expect(report.performance?.stages[0]?.details.cache).toBe("hit");
});
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
rtk pnpm test -- --run src/lib/render.test.ts
```

Expected: TypeScript compile failure because `performance` is not in `RenderReport`.

- [x] **Step 3: Add frontend optional performance types**

In `src/lib/render.ts`, add:

```ts
export interface RenderStageReport {
  name: string;
  status: string;
  durationMs: number;
  details: Record<string, string>;
}

export interface RenderPerformanceSummary {
  totalDurationMs: number;
  stages: RenderStageReport[];
}
```

Add to `RenderReport`:

```ts
performance?: RenderPerformanceSummary | null;
```

- [x] **Step 4: Run test to verify it passes**

Run the same pnpm test command. Expected: PASS.

---

### Task 7: Verification

**Files:**
- All modified files.

- [x] **Step 1: Run focused Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_pipeline --test render_pipeline -- --test-threads=1
```

Expected: PASS.

- [x] **Step 2: Run frontend render tests**

Run:

```bash
rtk pnpm test -- --run src/lib/render.test.ts
```

Expected: PASS.

- [x] **Step 3: Run lint**

Run:

```bash
rtk pnpm lint
```

Expected: PASS.

- [x] **Step 4: Review diff**

Run:

```bash
rtk git diff --stat
```

Expected: changes limited to spec, plan, render pipeline modules, proposal integration, Cargo files, and render model tests.

- [x] **Step 5: Commit implementation**

Run:

```bash
rtk git add docs/superpowers/plans/2026-06-19-render-performance-pipeline.md src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/render_pipeline src-tauri/tests/render_pipeline.rs src/lib/render.ts src/lib/render.test.ts
rtk git commit -m "feat: add render performance pipeline metadata"
```

Expected: commit succeeds with Conventional Commit message.

---

## Plan Self-Review

- Spec coverage: Task 1 and 2 cover observability; Task 3 covers cache validation; Task 4 and 5 cover quality metadata; Task 6 covers frontend compatibility; Task 7 covers verification.
- Placeholder scan: no TBD, TODO, or deferred implementation instructions remain.
- Type consistency: `RenderPerformanceSummary`, `RenderStageReport`, `GraphicsCacheLookup`, and `RenderQualitySettings` are defined before downstream integration steps reference them.
