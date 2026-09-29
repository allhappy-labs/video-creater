# Rust Render Pipeline Node Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove Node.js from the graphics, render, Codex proposal e2e, and combined video e2e pipeline while keeping the React/Vite frontend toolchain.

**Architecture:** Add a `video_creater_lib::render_pipeline` module that owns actionable pipeline errors, process execution, ffprobe validation, reports, proposal-to-graphics conversion, and ffmpeg backend orchestration. Add thin Rust binaries under `src-tauri/src/bin/` for the current Node harness entry points, then remove the replaced `.mjs` files and update package scripts.

**Tech Stack:** Rust 1.77, existing `serde`/`serde_json`/`thiserror`, existing `tiny-skia` graphics renderer, existing `ffmpeg`/`ffprobe` CLI backend, existing Codex app-server transport.

---

## Scope

This plan implements the approved pipeline-only migration from `docs/superpowers/specs/2026-06-17-rust-render-pipeline-node-removal-design.md`.

In scope:

- Rust replacement for `scripts/render-codex-funny-draft.mjs`.
- Rust replacement for `scripts/e2e-combined-video.mjs`.
- Rust replacement for `scripts/e2e-codex-app-server-funny.mjs`.
- Package scripts that call Rust binaries for pipeline/e2e work.
- Deletion of non-frontend `.mjs` pipeline harnesses after Rust replacements pass.

Out of scope:

- Removing Node from Vite, React, Vitest, TypeScript, or Tauri frontend build tooling.
- Replacing `ffmpeg` or `ffprobe`.
- Adding arbitrary executable graphics code.

## File Structure

- Create `src-tauri/src/render_pipeline/mod.rs`: module exports.
- Create `src-tauri/src/render_pipeline/error.rs`: compact actionable pipeline error shape plus conversions from graphics and Codex errors.
- Create `src-tauri/src/render_pipeline/process.rs`: command specs, process output, process runner trait, and real process runner.
- Create `src-tauri/src/render_pipeline/probe.rs`: ffprobe JSON parser and rendered-media validation.
- Create `src-tauri/src/render_pipeline/report.rs`: render/e2e report structs and JSON/Markdown writers.
- Create `src-tauri/src/render_pipeline/backend.rs`: ffmpeg backend boundary and command execution wrapper.
- Create `src-tauri/src/render_pipeline/proposal.rs`: Codex proposal report loading, validation, proposal-to-graphics conversion, and render orchestration.
- Create `src-tauri/src/render_pipeline/combined_e2e.rs`: deterministic combined media e2e generation.
- Create `src-tauri/src/render_pipeline/codex_e2e.rs`: Rust Codex app-server e2e report generation.
- Modify `src-tauri/src/lib.rs`: export `render_pipeline`.
- Create `src-tauri/src/bin/video-creater-render-proposal.rs`: CLI for proposal render harness.
- Create `src-tauri/src/bin/video-creater-e2e-combined.rs`: CLI for combined e2e harness.
- Create `src-tauri/src/bin/video-creater-codex-e2e.rs`: CLI for Codex proposal e2e harness.
- Create `src-tauri/tests/render_pipeline.rs`: unit/integration tests for the pipeline module.
- Modify `package.json`: replace Node-backed e2e script with Rust-backed scripts.
- Delete `scripts/render-codex-funny-draft.mjs`.
- Delete `scripts/e2e-combined-video.mjs`.
- Delete `scripts/e2e-codex-app-server-funny.mjs`.

## Task 1: Pipeline Module And Actionable Errors

**Files:**

- Create: `src-tauri/src/render_pipeline/mod.rs`
- Create: `src-tauri/src/render_pipeline/error.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing tests for pipeline error serialization and graphics error conversion**

Add `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::graphics::error::{ActionableError, GraphicsErrorCode};
use video_creater_lib::render_pipeline::error::{
    PipelineError, PipelineErrorCode, PipelineResult,
};

#[test]
fn pipeline_error_serializes_for_agent_repair() {
    let error = PipelineError::new(
        PipelineErrorCode::RenderBackendUnavailable,
        "ffmpeg",
        "ffmpeg was not found.",
        "Install ffmpeg or set VIDEO_CREATER_FFMPEG.",
    )
    .with_detail("program", "ffmpeg");

    let value = serde_json::to_value(error).expect("serialize pipeline error");

    assert_eq!(value["code"], "RENDER_BACKEND_UNAVAILABLE");
    assert_eq!(value["path"], "ffmpeg");
    assert_eq!(value["message"], "ffmpeg was not found.");
    assert_eq!(value["fix"], "Install ffmpeg or set VIDEO_CREATER_FFMPEG.");
    assert_eq!(value["details"]["program"], "ffmpeg");
}

#[test]
fn graphics_errors_convert_to_pipeline_errors_without_losing_action() {
    let graphics_error = ActionableError::new(
        GraphicsErrorCode::GraphicsImageRefMissing,
        "nodes[0].assetId",
        "imageRef asset is missing.",
        "Register an approved image asset before rendering.",
    );

    let errors = PipelineError::from_graphics_errors(vec![graphics_error]);

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, PipelineErrorCode::GraphicsImageRefMissing);
    assert_eq!(errors[0].path, "nodes[0].assetId");
    assert_eq!(errors[0].message, "imageRef asset is missing.");
    assert_eq!(
        errors[0].fix,
        "Register an approved image asset before rendering."
    );
}

#[test]
fn pipeline_result_alias_collects_actionable_errors() {
    fn fail() -> PipelineResult<()> {
        Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "proposal",
            "Proposal JSON is invalid.",
            "Pass a report with a top-level proposal object.",
        )])
    }

    let errors = fail().expect_err("pipeline result should fail");

    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline pipeline_error_serializes_for_agent_repair graphics_errors_convert_to_pipeline_errors_without_losing_action pipeline_result_alias_collects_actionable_errors
```

Expected: FAIL because `video_creater_lib::render_pipeline` does not exist.

- [ ] **Step 3: Add module exports**

Create `src-tauri/src/render_pipeline/mod.rs`:

```rust
pub mod backend;
pub mod codex_e2e;
pub mod combined_e2e;
pub mod error;
pub mod probe;
pub mod process;
pub mod proposal;
pub mod report;
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod codex;
pub mod edit;
pub mod graphics;
pub mod project;
pub mod render_pipeline;
pub mod transcription;
```

- [ ] **Step 4: Implement actionable pipeline errors**

Create `src-tauri/src/render_pipeline/error.rs`:

```rust
use crate::graphics::error::{ActionableError, GraphicsErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PipelineErrorCode {
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
    RenderPlanInvalidClip,
    RenderPlanInvalidOverlay,
    RenderBackendUnavailable,
    RenderBackendFailed,
    RenderBackendTimeout,
    RenderProbeInvalidJson,
    RenderProbeValidationFailed,
    RenderArtifactMissing,
    RenderArtifactEmpty,
    PipelineInputInvalid,
    PipelineReportWriteFailed,
    CodexTransportFailed,
    CodexProposalInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PipelineError {
    pub code: PipelineErrorCode,
    pub path: String,
    pub message: String,
    pub fix: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

pub type PipelineResult<T> = Result<T, Vec<PipelineError>>;

impl PipelineError {
    pub fn new(
        code: PipelineErrorCode,
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

    pub fn from_graphics_errors(errors: Vec<ActionableError>) -> Vec<Self> {
        errors
            .into_iter()
            .map(|error| {
                PipelineError::new(
                    pipeline_code_from_graphics_code(&error.code),
                    error.path,
                    error.message,
                    error.fix,
                )
            })
            .collect()
    }
}

fn pipeline_code_from_graphics_code(code: &GraphicsErrorCode) -> PipelineErrorCode {
    match code {
        GraphicsErrorCode::GraphicsSchemaUnsupported => PipelineErrorCode::GraphicsSchemaUnsupported,
        GraphicsErrorCode::GraphicsEmptyNodeId => PipelineErrorCode::GraphicsEmptyNodeId,
        GraphicsErrorCode::GraphicsUnsupportedPrimitive => PipelineErrorCode::GraphicsUnsupportedPrimitive,
        GraphicsErrorCode::GraphicsInvalidTiming => PipelineErrorCode::GraphicsInvalidTiming,
        GraphicsErrorCode::GraphicsInvalidDimensions => PipelineErrorCode::GraphicsInvalidDimensions,
        GraphicsErrorCode::GraphicsTextEmpty => PipelineErrorCode::GraphicsTextEmpty,
        GraphicsErrorCode::GraphicsTextOverflow => PipelineErrorCode::GraphicsTextOverflow,
        GraphicsErrorCode::GraphicsTextUnreadable => PipelineErrorCode::GraphicsTextUnreadable,
        GraphicsErrorCode::GraphicsNodeOutOfSafeZone => PipelineErrorCode::GraphicsNodeOutOfSafeZone,
        GraphicsErrorCode::GraphicsFrameCoverageExceeded => PipelineErrorCode::GraphicsFrameCoverageExceeded,
        GraphicsErrorCode::GraphicsImageRefMissing => PipelineErrorCode::GraphicsImageRefMissing,
        GraphicsErrorCode::GraphicsImageRefUnauthorized => PipelineErrorCode::GraphicsImageRefUnauthorized,
        GraphicsErrorCode::GraphicsImageDecodeFailed => PipelineErrorCode::GraphicsImageDecodeFailed,
        GraphicsErrorCode::GraphicsTemplateParamMissing => PipelineErrorCode::GraphicsTemplateParamMissing,
        GraphicsErrorCode::GraphicsTemplateParamInvalid => PipelineErrorCode::GraphicsTemplateParamInvalid,
        GraphicsErrorCode::GraphicsRenderFailed => PipelineErrorCode::GraphicsRenderFailed,
        GraphicsErrorCode::RenderOverlayOutOfRange => PipelineErrorCode::RenderPlanInvalidOverlay,
        GraphicsErrorCode::RenderFrameSequenceEmpty => PipelineErrorCode::RenderArtifactEmpty,
        GraphicsErrorCode::RenderBackendUnavailable => PipelineErrorCode::RenderBackendUnavailable,
        GraphicsErrorCode::RenderBackendFailed => PipelineErrorCode::RenderBackendFailed,
    }
}
```

- [ ] **Step 5: Run the tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline pipeline_error_serializes_for_agent_repair graphics_errors_convert_to_pipeline_errors_without_losing_action pipeline_result_alias_collects_actionable_errors
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/lib.rs src-tauri/src/render_pipeline src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add render pipeline errors"
```

## Task 2: Process Runner And ffprobe Validation

**Files:**

- Create: `src-tauri/src/render_pipeline/process.rs`
- Create: `src-tauri/src/render_pipeline/probe.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests for process failures and ffprobe parsing**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
use std::time::Duration;
use video_creater_lib::render_pipeline::probe::{
    parse_ffprobe_json, validate_rendered_media, ExpectedMedia,
};
use video_creater_lib::render_pipeline::process::{
    CommandSpec, ProcessOutput, ProcessRunner,
};

#[test]
fn command_spec_formats_program_and_args_for_reports() {
    let command = CommandSpec::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg("input.mov");

    assert_eq!(command.display(), "ffmpeg -hide_banner -i input.mov");
}

#[test]
fn process_runner_converts_spawn_error_to_actionable_error() {
    let runner = video_creater_lib::render_pipeline::process::SystemProcessRunner;
    let command = CommandSpec::new("definitely-missing-video-creater-binary");

    let errors = runner
        .run(&command, Duration::from_millis(100))
        .expect_err("missing binary should fail");

    assert_eq!(errors[0].code, PipelineErrorCode::RenderBackendUnavailable);
    assert_eq!(errors[0].path, "definitely-missing-video-creater-binary");
}

#[test]
fn ffprobe_parser_reads_video_audio_and_format() {
    let probe = parse_ffprobe_json(
        r#"{
          "streams": [
            {"codec_type":"video","codec_name":"h264","width":1280,"height":720},
            {"codec_type":"audio","codec_name":"aac"}
          ],
          "format": {"duration":"10.021","size":"123456"}
        }"#,
    )
    .expect("parse ffprobe json");

    assert_eq!(probe.duration_seconds, 10.021);
    assert_eq!(probe.size_bytes, 123456);
    assert_eq!(probe.video.expect("video").codec_name, "h264");
    assert_eq!(probe.audio.expect("audio").codec_name, "aac");
}

#[test]
fn media_validation_reports_dimension_mismatch() {
    let probe = parse_ffprobe_json(
        r#"{
          "streams": [
            {"codec_type":"video","codec_name":"h264","width":640,"height":360},
            {"codec_type":"audio","codec_name":"aac"}
          ],
          "format": {"duration":"10.000","size":"123456"}
        }"#,
    )
    .expect("parse ffprobe json");

    let errors = validate_rendered_media(
        &probe,
        &ExpectedMedia {
            duration_seconds: 10.0,
            duration_tolerance_seconds: 0.25,
            width: 1280,
            height: 720,
            require_audio: true,
            min_size_bytes: 100_000,
        },
    )
    .expect_err("wrong dimensions should fail");

    assert_eq!(errors[0].code, PipelineErrorCode::RenderProbeValidationFailed);
    assert_eq!(errors[0].path, "streams.video.dimensions");
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline command_spec_formats_program_and_args_for_reports process_runner_converts_spawn_error_to_actionable_error ffprobe_parser_reads_video_audio_and_format media_validation_reports_dimension_mismatch
```

Expected: FAIL because `process` and `probe` are empty.

- [ ] **Step 3: Implement process runner primitives**

Create `src-tauri/src/render_pipeline/process.rs`:

```rust
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessOutput {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait ProcessRunner {
    fn run(&self, command: &CommandSpec, timeout: Duration) -> PipelineResult<ProcessOutput>;
}

#[derive(Debug, Clone, Copy)]
pub struct SystemProcessRunner;

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl ProcessRunner for SystemProcessRunner {
    fn run(&self, command: &CommandSpec, _timeout: Duration) -> PipelineResult<ProcessOutput> {
        let output = Command::new(&command.program)
            .args(&command.args)
            .output()
            .map_err(|error| {
                vec![PipelineError::new(
                    PipelineErrorCode::RenderBackendUnavailable,
                    command.program.clone(),
                    format!("Could not start {}.", command.program),
                    format!("Install {} or configure the matching environment variable.", command.program),
                )
                .with_detail("error", error.to_string())]
            })?;

        let result = ProcessOutput {
            status_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        };

        if output.status.success() {
            Ok(result)
        } else {
            Err(vec![PipelineError::new(
                PipelineErrorCode::RenderBackendFailed,
                command.program.clone(),
                format!("{} exited with a nonzero status.", command.program),
                "Read the captured stderr log, fix the render input, and retry.",
            )
            .with_detail("status", result.status_code.map_or_else(|| "signal".to_string(), |code| code.to_string()))
            .with_detail("stderr", result.stderr.chars().take(500).collect::<String>())])
        }
    }
}
```

- [ ] **Step 4: Implement ffprobe parser and validation**

Create `src-tauri/src/render_pipeline/probe.rs`:

```rust
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    pub duration_seconds: f64,
    pub size_bytes: u64,
    pub video: Option<VideoProbe>,
    pub audio: Option<AudioProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoProbe {
    pub codec_name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioProbe {
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedMedia {
    pub duration_seconds: f64,
    pub duration_tolerance_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub require_audio: bool,
    pub min_size_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct RawProbe {
    streams: Vec<RawStream>,
    format: RawFormat,
}

#[derive(Debug, Deserialize)]
struct RawStream {
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawFormat {
    duration: Option<String>,
    size: Option<String>,
}

pub fn parse_ffprobe_json(input: &str) -> PipelineResult<MediaProbe> {
    let raw = serde_json::from_str::<RawProbe>(input).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::RenderProbeInvalidJson,
            "ffprobe",
            "ffprobe returned invalid JSON.",
            "Run ffprobe with -print_format json -show_format -show_streams.",
        )
        .with_detail("error", error.to_string())]
    })?;

    let duration_seconds = raw
        .format
        .duration
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.0);
    let size_bytes = raw
        .format
        .size
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);

    let video = raw
        .streams
        .iter()
        .find(|stream| stream.codec_type == "video")
        .map(|stream| VideoProbe {
            codec_name: stream.codec_name.clone().unwrap_or_default(),
            width: stream.width.unwrap_or(0),
            height: stream.height.unwrap_or(0),
        });
    let audio = raw
        .streams
        .iter()
        .find(|stream| stream.codec_type == "audio")
        .map(|stream| AudioProbe {
            codec_name: stream.codec_name.clone().unwrap_or_default(),
        });

    Ok(MediaProbe {
        duration_seconds,
        size_bytes,
        video,
        audio,
    })
}

pub fn validate_rendered_media(probe: &MediaProbe, expected: &ExpectedMedia) -> PipelineResult<()> {
    let mut errors = Vec::new();
    let video = probe.video.as_ref();
    if video.is_none() {
        errors.push(PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "streams.video",
            "Rendered media has no video stream.",
            "Check ffmpeg maps [vout] into the output.",
        ));
    }
    if expected.require_audio && probe.audio.is_none() {
        errors.push(PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "streams.audio",
            "Rendered media has no audio stream.",
            "Check ffmpeg maps [aout] into the output.",
        ));
    }
    if let Some(video) = video {
        if video.width != expected.width || video.height != expected.height {
            errors.push(PipelineError::new(
                PipelineErrorCode::RenderProbeValidationFailed,
                "streams.video.dimensions",
                "Rendered video dimensions do not match the render settings.",
                "Scale the render plan and graphics overlays to the same width and height.",
            ));
        }
    }
    if (probe.duration_seconds - expected.duration_seconds).abs() > expected.duration_tolerance_seconds {
        errors.push(PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "format.duration",
            "Rendered duration is outside tolerance.",
            "Check clip ranges, overlay timing, and concat inputs.",
        ));
    }
    if probe.size_bytes < expected.min_size_bytes {
        errors.push(PipelineError::new(
            PipelineErrorCode::RenderProbeValidationFailed,
            "format.size",
            "Rendered file is too small to be a valid MP4.",
            "Inspect ffmpeg stderr and regenerate the output.",
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

- [ ] **Step 5: Run the tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline command_spec_formats_program_and_args_for_reports process_runner_converts_spawn_error_to_actionable_error ffprobe_parser_reads_video_audio_and_format media_validation_reports_dimension_mismatch
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add render process probing"
```

## Task 3: ffmpeg Backend And Reports

**Files:**

- Create: `src-tauri/src/render_pipeline/backend.rs`
- Create: `src-tauri/src/render_pipeline/report.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests for backend command conversion and reports**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
use std::path::PathBuf;
use video_creater_lib::edit::render_plan::{GraphicsOverlayInput, RenderClip, RenderPlan};
use video_creater_lib::render_pipeline::backend::{
    build_graphics_overlay_input, FfmpegRenderBackend, RenderBackend,
};
use video_creater_lib::render_pipeline::report::{
    RenderReport, RenderReportSummary, write_markdown_report, write_json_report,
};

#[test]
fn manifest_fields_become_ffmpeg_graphics_overlay_input() {
    let manifest = video_creater_lib::graphics::manifest::GraphicsArtifactManifest {
        schema_version: 1,
        artifact_id: "graphics:caption-1".to_string(),
        kind: "rgbaFrameSequence".to_string(),
        dimensions: video_creater_lib::graphics::ir::Dimensions { width: 1280, height: 720 },
        fps: 30.0,
        duration_seconds: 1.5,
        alpha: true,
        frame_count: 1,
        frames_pattern: "frames/frame-%06d.png".into(),
        preview_path: "preview.png".into(),
        source_layer_ids: vec!["caption-1".into()],
        checksums: Default::default(),
    };

    let overlay = build_graphics_overlay_input(&manifest, "/tmp/gfx", 2.0);

    assert_eq!(overlay.frames_pattern, "/tmp/gfx/frames/frame-%06d.png");
    assert_eq!(overlay.timeline_start_seconds, 2.0);
    assert_eq!(overlay.duration_seconds, 1.5);
    assert_eq!(overlay.width, 1280);
    assert!(overlay.alpha);
}

#[test]
fn ffmpeg_backend_builds_command_without_spawning_process() {
    let backend = FfmpegRenderBackend::new("ffmpeg");
    let plan = RenderPlan {
        input_path: "input.mov".to_string(),
        output_path: "output.mp4".to_string(),
        width: 1280,
        height: 720,
        fps: 30.0,
        clips: vec![RenderClip { source_in: 0.0, source_out: 3.0 }],
    };
    let graphics = vec![GraphicsOverlayInput {
        frames_pattern: "generated/graphics/layer/frames/frame-%06d.png".to_string(),
        timeline_start_seconds: 0.5,
        duration_seconds: 1.0,
        fps: 30.0,
        width: 1280,
        height: 720,
        alpha: true,
        frame_count: 1,
    }];

    let command = backend.build_command(&plan, &graphics).expect("ffmpeg command");

    assert_eq!(command.program, "ffmpeg");
    assert!(command.args.iter().any(|arg| arg == "generated/graphics/layer/frames/frame-%06d.png"));
    assert!(command.args.iter().any(|arg| arg.contains("overlay=0:0")));
}

#[test]
fn render_reports_write_json_and_markdown() {
    let dir = tempfile::tempdir().expect("temp dir");
    let report = RenderReport {
        generated_at: "2026-06-17T00:00:00Z".to_string(),
        final_path: PathBuf::from("output/final.mp4"),
        summary: RenderReportSummary {
            clip_count: 2,
            caption_count: 3,
            overlay_count: 1,
            duration_seconds: 5.5,
        },
        validation: serde_json::json!({"hasVideo": true, "hasAudio": true}),
        errors: Vec::new(),
    };

    let json_path = dir.path().join("report.json");
    let md_path = dir.path().join("report.md");
    write_json_report(&report, &json_path).expect("json report");
    write_markdown_report(&report, &md_path).expect("markdown report");

    let json_text = std::fs::read_to_string(json_path).expect("read json");
    let md_text = std::fs::read_to_string(md_path).expect("read md");
    assert!(json_text.contains("\"clipCount\": 2"));
    assert!(md_text.contains("# Render Report"));
    assert!(md_text.contains("output/final.mp4"));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline manifest_fields_become_ffmpeg_graphics_overlay_input ffmpeg_backend_builds_command_without_spawning_process render_reports_write_json_and_markdown
```

Expected: FAIL because backend/report APIs do not exist.

- [ ] **Step 3: Implement ffmpeg backend boundary**

Create `src-tauri/src/render_pipeline/backend.rs`:

```rust
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use super::process::{CommandSpec, ProcessOutput, ProcessRunner};
use crate::edit::render_plan::{
    build_ffmpeg_render_command_with_graphics, GraphicsOverlayInput, RenderPlan,
};
use crate::graphics::manifest::GraphicsArtifactManifest;
use std::path::Path;
use std::time::Duration;

pub trait RenderBackend {
    fn build_command(
        &self,
        plan: &RenderPlan,
        graphics: &[GraphicsOverlayInput],
    ) -> PipelineResult<CommandSpec>;

    fn render<R: ProcessRunner>(
        &self,
        runner: &R,
        plan: &RenderPlan,
        graphics: &[GraphicsOverlayInput],
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegRenderBackend {
    program: String,
}

impl FfmpegRenderBackend {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl RenderBackend for FfmpegRenderBackend {
    fn build_command(
        &self,
        plan: &RenderPlan,
        graphics: &[GraphicsOverlayInput],
    ) -> PipelineResult<CommandSpec> {
        let command = build_ffmpeg_render_command_with_graphics(plan, graphics).map_err(|error| {
            vec![PipelineError::new(
                PipelineErrorCode::RenderPlanInvalidOverlay,
                "graphics",
                "Could not build ffmpeg render command.",
                "Check clip ranges, graphics timing, dimensions, alpha, and frame count.",
            )
            .with_detail("error", error.to_string())]
        })?;
        Ok(CommandSpec::new(self.program.clone()).args(command.args))
    }

    fn render<R: ProcessRunner>(
        &self,
        runner: &R,
        plan: &RenderPlan,
        graphics: &[GraphicsOverlayInput],
        timeout: Duration,
    ) -> PipelineResult<ProcessOutput> {
        let command = self.build_command(plan, graphics)?;
        runner.run(&command, timeout)
    }
}

pub fn build_graphics_overlay_input(
    manifest: &GraphicsArtifactManifest,
    artifact_dir: impl AsRef<Path>,
    timeline_start_seconds: f64,
) -> GraphicsOverlayInput {
    let frames_pattern = artifact_dir
        .as_ref()
        .join(&manifest.frames_pattern)
        .display()
        .to_string();
    GraphicsOverlayInput {
        frames_pattern,
        timeline_start_seconds,
        duration_seconds: manifest.duration_seconds,
        fps: manifest.fps,
        width: manifest.dimensions.width,
        height: manifest.dimensions.height,
        alpha: manifest.alpha,
        frame_count: manifest.frame_count,
    }
}
```

- [ ] **Step 4: Implement report writers**

Create `src-tauri/src/render_pipeline/report.rs`:

```rust
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderReport {
    pub generated_at: String,
    pub final_path: PathBuf,
    pub summary: RenderReportSummary,
    pub validation: serde_json::Value,
    pub errors: Vec<PipelineError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderReportSummary {
    pub clip_count: usize,
    pub caption_count: usize,
    pub overlay_count: usize,
    pub duration_seconds: f64,
}

pub fn write_json_report(report: &RenderReport, path: &Path) -> PipelineResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| report_error(path, error))?;
    }
    let text = serde_json::to_string_pretty(report)
        .map_err(|error| report_error(path, error))?;
    std::fs::write(path, format!("{text}\n")).map_err(|error| report_error(path, error))
}

pub fn write_markdown_report(report: &RenderReport, path: &Path) -> PipelineResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| report_error(path, error))?;
    }
    let text = format!(
        "# Render Report\n\n- Final video: `{}`\n- Duration: {:.3}s\n- Clips: {}\n- Captions: {}\n- Overlays: {}\n- Errors: {}\n",
        report.final_path.display(),
        report.summary.duration_seconds,
        report.summary.clip_count,
        report.summary.caption_count,
        report.summary.overlay_count,
        report.errors.len(),
    );
    std::fs::write(path, text).map_err(|error| report_error(path, error))
}

fn report_error(path: &Path, error: impl std::fmt::Display) -> Vec<PipelineError> {
    vec![PipelineError::new(
        PipelineErrorCode::PipelineReportWriteFailed,
        path.display().to_string(),
        "Could not write render report.",
        "Choose a writable report path and retry.",
    )
    .with_detail("error", error.to_string())]
}
```

- [ ] **Step 5: Run the tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline manifest_fields_become_ffmpeg_graphics_overlay_input ffmpeg_backend_builds_command_without_spawning_process render_reports_write_json_and_markdown
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add ffmpeg render backend"
```

## Task 4: Proposal Loading And Graphics Conversion

**Files:**

- Create: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests for proposal report loading and graphics conversion**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::codex::proposal::{CodexProposalClip, CodexRenderReview};
use video_creater_lib::edit::preset::{CaptionStyle, EditJobRequest, EditPreset, LanguageMode};
use video_creater_lib::project::model::{MediaAsset, MediaKind, Transcript, TranscriptWord, VideoProject};
use video_creater_lib::render_pipeline::proposal::{
    codex_report_from_json, proposal_to_render_plan, proposal_visuals_to_graphics_layers,
    CodexProposalReport,
};

#[test]
fn codex_report_loader_requires_top_level_proposal() {
    let errors = codex_report_from_json("{}").expect_err("missing proposal should fail");

    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "proposal");
}

#[test]
fn proposal_render_plan_uses_selected_clip_ranges() {
    let project = sample_render_project();
    let request = sample_render_request();
    let report = sample_codex_report();

    let plan = proposal_to_render_plan(
        &project,
        &request,
        &report.proposal,
        "source.mov",
        "final.mp4",
    )
    .expect("render plan");

    assert_eq!(plan.width, 540);
    assert_eq!(plan.height, 960);
    assert_eq!(plan.clips.len(), 2);
    assert_eq!(plan.clips[0].source_in, 1.0);
    assert_eq!(plan.clips[1].source_out, 36.0);
}

#[test]
fn proposal_visuals_convert_to_valid_graphics_layers() {
    let report = sample_codex_report();
    let layers = proposal_visuals_to_graphics_layers(&report.proposal, 540, 960, 30.0)
        .expect("graphics layers");

    assert!(layers.iter().any(|layer| layer.id == "caption-1"));
    assert!(layers.iter().any(|layer| layer.id == "overlay-1"));
    assert!(layers.iter().all(|layer| layer.alpha));
    assert!(layers.iter().all(|layer| !layer.visual_treatment.trim().is_empty()));
}

fn sample_render_request() -> EditJobRequest {
    EditJobRequest {
        media_id: "media-1".to_string(),
        preset: EditPreset::TrailerCut,
        prompt: "Make an action edit with bold captions".to_string(),
        target_duration_seconds: Some(35.0),
        language_mode: LanguageMode::English,
        caption_style: CaptionStyle::Bold,
        created_at: "2026-06-17T00:00:00Z".to_string(),
    }
}

fn sample_render_project() -> VideoProject {
    let mut project = VideoProject::new_empty(
        "project-1".to_string(),
        "Render Project".to_string(),
        "2026-06-17T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-1".to_string(),
        relative_path: "media/source.mov".to_string(),
        kind: MediaKind::Video,
        duration_seconds: 120.0,
        width: Some(540),
        height: Some(960),
        fps: Some(30.0),
    });
    project.transcripts.push(Transcript {
        id: "transcript-1".to_string(),
        media_id: "media-1".to_string(),
        engine: None,
        raw_artifact_path: None,
        segments: Vec::new(),
        words: vec![TranscriptWord {
            text: "hello".to_string(),
            start_seconds: 1.0,
            end_seconds: 1.4,
            confidence: Some(1.0),
            speaker: None,
        }],
    });
    project.render_settings.width = 540;
    project.render_settings.height = 960;
    project.render_settings.fps = 30.0;
    project
}

fn sample_codex_report() -> CodexProposalReport {
    CodexProposalReport {
        generated_at: "2026-06-17T00:00:00Z".to_string(),
        proposal: video_creater_lib::codex::proposal::CodexEditProposal {
            media_id: "media-1".to_string(),
            clips: vec![
                CodexProposalClip { source_in: 1.0, source_out: 31.0, reason: "hook".to_string() },
                CodexProposalClip { source_in: 32.0, source_out: 36.0, reason: "payoff".to_string() },
            ],
            captions: vec![serde_json::json!({
                "text": "BIG MOMENT",
                "startSeconds": 0.2,
                "durationSeconds": 1.4,
                "visualTreatment": "large phone-readable caption with accent underline",
                "motion": "scale pop and underline wipe",
                "safeZone": "keep caption inside 10% margins",
                "avoid": "full-width opaque black slabs"
            })],
            overlays: vec![serde_json::json!({
                "kind": "lower_third",
                "startSeconds": 1.0,
                "durationSeconds": 2.0,
                "brief": "Introduce the chef.",
                "visualTreatment": "compact lower third with translucent backing",
                "motion": "slide in and fade out",
                "safeZone": "keep essential text inside 10% margins",
                "avoid": "covering hands"
            })],
            hyperframes: Vec::new(),
            render_review: CodexRenderReview {
                duration_seconds: 34.0,
                stream_check_required: true,
                caption_alignment_required: true,
                overlay_timing_required: true,
                artifact_paths_required: true,
                log_reference_required: true,
            },
        },
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline codex_report_loader_requires_top_level_proposal proposal_render_plan_uses_selected_clip_ranges proposal_visuals_convert_to_valid_graphics_layers
```

Expected: FAIL because proposal APIs do not exist.

- [ ] **Step 3: Implement proposal report and render plan conversion**

Create `src-tauri/src/render_pipeline/proposal.rs` with this initial content:

```rust
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::codex::proposal::{validate_codex_edit_proposal, CodexEditProposal};
use crate::edit::preset::EditJobRequest;
use crate::edit::render_plan::{RenderClip, RenderPlan};
use crate::graphics::error::ActionableResult;
use crate::graphics::ir::{
    Color, Dimensions, GraphicNode, GraphicRole, GraphicsLayer, Rect, RoundedRectNode, TextNode,
};
use crate::graphics::validation::validate_graphics_layer;
use crate::project::model::VideoProject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexProposalReport {
    pub generated_at: String,
    pub proposal: CodexEditProposal,
}

pub fn codex_report_from_json(input: &str) -> PipelineResult<CodexProposalReport> {
    let value = serde_json::from_str::<serde_json::Value>(input).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "report",
            "Codex proposal report is not valid JSON.",
            "Pass a JSON report with a top-level proposal object.",
        )
        .with_detail("error", error.to_string())]
    })?;
    if value.get("proposal").is_none() {
        return Err(vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "proposal",
            "Codex proposal report is missing proposal.",
            "Pass the report produced by video-creater-codex-e2e or add a proposal object.",
        )]);
    }
    serde_json::from_value::<CodexProposalReport>(value).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "report",
            "Codex proposal report does not match the expected shape.",
            "Use mediaId, clips, captions, overlays, hyperframes, and renderReview fields.",
        )
        .with_detail("error", error.to_string())]
    })
}

pub fn proposal_to_render_plan(
    project: &VideoProject,
    request: &EditJobRequest,
    proposal: &CodexEditProposal,
    input_path: impl Into<String>,
    output_path: impl Into<String>,
) -> PipelineResult<RenderPlan> {
    validate_codex_edit_proposal(project, request, proposal).map_err(|error| {
        vec![PipelineError::new(
            PipelineErrorCode::CodexProposalInvalid,
            "proposal",
            "Codex edit proposal is invalid.",
            "Fix clip ranges, renderReview, mediaId, or overlay metadata and retry.",
        )
        .with_detail("error", error.to_string())]
    })?;
    Ok(RenderPlan {
        input_path: input_path.into(),
        output_path: output_path.into(),
        width: project.render_settings.width,
        height: project.render_settings.height,
        fps: project.render_settings.fps,
        clips: proposal
            .clips
            .iter()
            .map(|clip| RenderClip {
                source_in: clip.source_in,
                source_out: clip.source_out,
            })
            .collect(),
    })
}

pub fn proposal_visuals_to_graphics_layers(
    proposal: &CodexEditProposal,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<Vec<GraphicsLayer>> {
    let mut layers = Vec::new();
    for (index, caption) in proposal.captions.iter().enumerate() {
        layers.push(caption_to_layer(index, caption, width, height, fps)?);
    }
    for (index, overlay) in proposal.overlays.iter().enumerate() {
        layers.push(overlay_to_layer(index, overlay, width, height, fps)?);
    }
    Ok(layers)
}

fn caption_to_layer(
    index: usize,
    caption: &serde_json::Value,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GraphicsLayer> {
    let text = required_string(caption, "text", format!("captions[{index}].text"))?;
    let start = required_f64(caption, "startSeconds", format!("captions[{index}].startSeconds"))?;
    let duration = required_f64(caption, "durationSeconds", format!("captions[{index}].durationSeconds"))?;
    let layer = GraphicsLayer {
        schema_version: 1,
        id: format!("caption-{}", index + 1),
        role: GraphicRole::Caption,
        timeline_start: start,
        duration_seconds: duration,
        dimensions: Dimensions { width, height },
        fps,
        alpha: true,
        source_beat: "Codex caption visual beat.".to_string(),
        visual_treatment: required_string(caption, "visualTreatment", format!("captions[{index}].visualTreatment"))?,
        motion: required_string(caption, "motion", format!("captions[{index}].motion"))?,
        safe_zone: required_string(caption, "safeZone", format!("captions[{index}].safeZone"))?,
        avoid: required_string(caption, "avoid", format!("captions[{index}].avoid"))?,
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: Rect {
                    x: width as f64 * 0.08,
                    y: height as f64 * 0.74,
                    width: width as f64 * 0.84,
                    height: height as f64 * 0.13,
                },
                radius: 18.0,
                fill: Color::Hex("#071013cc".to_string()),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text,
                box_rect: Rect {
                    x: width as f64 * 0.11,
                    y: height as f64 * 0.765,
                    width: width as f64 * 0.78,
                    height: height as f64 * 0.08,
                },
                font_size: (height as f64 * 0.045).max(26.0),
                font_weight: 850,
                align: "center".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(2),
            }),
        ],
    };
    validate_layer(layer)
}

fn overlay_to_layer(
    index: usize,
    overlay: &serde_json::Value,
    width: u32,
    height: u32,
    fps: f64,
) -> PipelineResult<GraphicsLayer> {
    let label = overlay
        .get("brief")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Overlay")
        .trim()
        .chars()
        .take(42)
        .collect::<String>();
    let start = required_f64(overlay, "startSeconds", format!("overlays[{index}].startSeconds"))?;
    let duration = required_f64(overlay, "durationSeconds", format!("overlays[{index}].durationSeconds"))?;
    let layer = GraphicsLayer {
        schema_version: 1,
        id: format!("overlay-{}", index + 1),
        role: GraphicRole::Overlay,
        timeline_start: start,
        duration_seconds: duration,
        dimensions: Dimensions { width, height },
        fps,
        alpha: true,
        source_beat: "Codex overlay visual beat.".to_string(),
        visual_treatment: required_string(overlay, "visualTreatment", format!("overlays[{index}].visualTreatment"))?,
        motion: required_string(overlay, "motion", format!("overlays[{index}].motion"))?,
        safe_zone: required_string(overlay, "safeZone", format!("overlays[{index}].safeZone"))?,
        avoid: required_string(overlay, "avoid", format!("overlays[{index}].avoid"))?,
        nodes: vec![
            GraphicNode::RoundedRect(RoundedRectNode {
                id: "backing".to_string(),
                box_rect: Rect {
                    x: width as f64 * 0.06,
                    y: height as f64 * 0.10,
                    width: width as f64 * 0.70,
                    height: height as f64 * 0.105,
                },
                radius: 16.0,
                fill: Color::Hex("#050505b8".to_string()),
            }),
            GraphicNode::Text(TextNode {
                id: "headline".to_string(),
                text: label,
                box_rect: Rect {
                    x: width as f64 * 0.09,
                    y: height as f64 * 0.125,
                    width: width as f64 * 0.64,
                    height: height as f64 * 0.055,
                },
                font_size: (height as f64 * 0.028).max(20.0),
                font_weight: 800,
                align: "left".to_string(),
                fill: Color::Hex("#ffffff".to_string()),
                max_lines: Some(2),
            }),
        ],
    };
    validate_layer(layer)
}

fn validate_layer(layer: GraphicsLayer) -> PipelineResult<GraphicsLayer> {
    match validate_graphics_layer(&layer) as ActionableResult<()> {
        Ok(()) => Ok(layer),
        Err(errors) => Err(PipelineError::from_graphics_errors(errors)),
    }
}

fn required_string(value: &serde_json::Value, key: &str, path: String) -> PipelineResult<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                path,
                "Required text field is missing or empty.",
                "Provide a non-empty string for this field.",
            )]
        })
}

fn required_f64(value: &serde_json::Value, key: &str, path: String) -> PipelineResult<f64> {
    value
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| {
            vec![PipelineError::new(
                PipelineErrorCode::PipelineInputInvalid,
                path,
                "Required numeric field is missing or invalid.",
                "Provide a finite number for this field.",
            )]
        })
}
```

- [ ] **Step 4: Run the tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline codex_report_loader_requires_top_level_proposal proposal_render_plan_uses_selected_clip_ranges proposal_visuals_convert_to_valid_graphics_layers
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: convert proposals to rust graphics"
```

## Task 5: Rust Proposal Render CLI

**Files:**

- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Create: `src-tauri/src/bin/video-creater-render-proposal.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests for CLI config parsing and dry-run render orchestration**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::render_pipeline::proposal::{
    parse_render_proposal_args, RenderProposalConfig,
};

#[test]
fn render_proposal_args_parse_required_paths() {
    let config = parse_render_proposal_args([
        "video-creater-render-proposal",
        "--project-root",
        "/tmp/project",
        "--source-video",
        "/tmp/source.mov",
        "--report",
        "/tmp/codex-report.json",
        "--output-dir",
        "/tmp/output",
        "--final-name",
        "final.mp4",
    ])
    .expect("parse args");

    assert_eq!(config.project_root, std::path::PathBuf::from("/tmp/project"));
    assert_eq!(config.source_video_path, std::path::PathBuf::from("/tmp/source.mov"));
    assert_eq!(config.codex_report_path, std::path::PathBuf::from("/tmp/codex-report.json"));
    assert_eq!(config.output_dir, std::path::PathBuf::from("/tmp/output"));
    assert_eq!(config.final_name, "final.mp4");
}

#[test]
fn render_proposal_args_reject_missing_report_path() {
    let errors = parse_render_proposal_args(["video-creater-render-proposal"])
        .expect_err("missing required args should fail");

    assert_eq!(errors[0].code, PipelineErrorCode::PipelineInputInvalid);
    assert_eq!(errors[0].path, "args.report");
}

#[test]
fn render_proposal_config_builds_expected_output_paths() {
    let config = RenderProposalConfig {
        project_root: "/tmp/project".into(),
        source_video_path: "/tmp/source.mov".into(),
        codex_report_path: "/tmp/report.json".into(),
        output_dir: "/tmp/output".into(),
        final_name: "final.mp4".to_string(),
        ffmpeg_program: "ffmpeg".to_string(),
        ffprobe_program: "ffprobe".to_string(),
    };

    assert_eq!(config.final_path(), std::path::PathBuf::from("/tmp/output/final.mp4"));
    assert_eq!(config.render_report_path(), std::path::PathBuf::from("/tmp/output/render-report.json"));
}

#[test]
fn render_proposal_dry_run_builds_graphics_and_ffmpeg_command() {
    let project = sample_render_project();
    let request = sample_render_request();
    let report = sample_codex_report();
    let config = RenderProposalConfig {
        project_root: "/tmp/project".into(),
        source_video_path: "/tmp/source.mov".into(),
        codex_report_path: "/tmp/report.json".into(),
        output_dir: "/tmp/output".into(),
        final_name: "final.mp4".to_string(),
        ffmpeg_program: "ffmpeg".to_string(),
        ffprobe_program: "ffprobe".to_string(),
    };

    let preview = video_creater_lib::render_pipeline::proposal::build_render_proposal_preview(
        &project,
        &request,
        &report,
        &config,
    )
    .expect("preview");

    assert_eq!(preview.graphics_layer_count, 2);
    assert_eq!(preview.command.program, "ffmpeg");
    assert!(preview.command.args.iter().any(|arg| arg == "/tmp/source.mov"));
    assert!(preview.command.args.iter().any(|arg| arg == "/tmp/output/final.mp4"));
}

#[test]
fn render_proposal_graphics_writes_rust_artifacts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let project = sample_render_project();
    let report = sample_codex_report();

    let overlays = video_creater_lib::render_pipeline::proposal::render_proposal_graphics(
        &project,
        &report,
        dir.path(),
    )
    .expect("render graphics artifacts");

    assert_eq!(overlays.len(), 2);
    assert!(dir.path().join("caption-1/preview.png").is_file());
    assert!(dir.path().join("overlay-1/manifest.json").is_file());
    assert!(overlays.iter().all(|overlay| overlay.alpha));
    assert!(overlays.iter().all(|overlay| overlay.frame_count > 0));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_proposal_args_parse_required_paths render_proposal_args_reject_missing_report_path render_proposal_config_builds_expected_output_paths
```

Expected: FAIL because CLI config APIs do not exist.

- [ ] **Step 3: Add render proposal config and argument parser**

Append to `src-tauri/src/render_pipeline/proposal.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProposalConfig {
    pub project_root: PathBuf,
    pub source_video_path: PathBuf,
    pub codex_report_path: PathBuf,
    pub output_dir: PathBuf,
    pub final_name: String,
    pub ffmpeg_program: String,
    pub ffprobe_program: String,
}

impl RenderProposalConfig {
    pub fn final_path(&self) -> PathBuf {
        self.output_dir.join(&self.final_name)
    }

    pub fn render_report_path(&self) -> PathBuf {
        self.output_dir.join("render-report.json")
    }
}

pub fn parse_render_proposal_args<I, S>(args: I) -> PipelineResult<RenderProposalConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut project_root = None;
    let mut source_video_path = None;
    let mut codex_report_path = None;
    let mut output_dir = None;
    let mut final_name = "final.mp4".to_string();
    let mut ffmpeg_program = std::env::var("VIDEO_CREATER_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let mut ffprobe_program = std::env::var("VIDEO_CREATER_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string());
    let mut iter = args.into_iter().map(Into::into);
    let _program = iter.next();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--project-root" => project_root = iter.next().map(PathBuf::from),
            "--source-video" => source_video_path = iter.next().map(PathBuf::from),
            "--report" => codex_report_path = iter.next().map(PathBuf::from),
            "--output-dir" => output_dir = iter.next().map(PathBuf::from),
            "--final-name" => final_name = iter.next().unwrap_or_else(|| "final.mp4".to_string()),
            "--ffmpeg" => ffmpeg_program = iter.next().unwrap_or_else(|| "ffmpeg".to_string()),
            "--ffprobe" => ffprobe_program = iter.next().unwrap_or_else(|| "ffprobe".to_string()),
            _ => {}
        }
    }
    let missing = |path: &'static str| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            path,
            "Required CLI argument is missing.",
            "Pass --project-root, --source-video, --report, and --output-dir.",
        )]
    };
    Ok(RenderProposalConfig {
        project_root: project_root.ok_or_else(|| missing("args.projectRoot"))?,
        source_video_path: source_video_path.ok_or_else(|| missing("args.sourceVideo"))?,
        codex_report_path: codex_report_path.ok_or_else(|| missing("args.report"))?,
        output_dir: output_dir.ok_or_else(|| missing("args.outputDir"))?,
        final_name,
        ffmpeg_program,
        ffprobe_program,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderProposalPreview {
    pub graphics_layer_count: usize,
    pub command: super::process::CommandSpec,
}

pub fn build_render_proposal_preview(
    project: &VideoProject,
    request: &EditJobRequest,
    report: &CodexProposalReport,
    config: &RenderProposalConfig,
) -> PipelineResult<RenderProposalPreview> {
    let plan = proposal_to_render_plan(
        project,
        request,
        &report.proposal,
        config.source_video_path.display().to_string(),
        config.final_path().display().to_string(),
    )?;
    let layers = proposal_visuals_to_graphics_layers(
        &report.proposal,
        project.render_settings.width,
        project.render_settings.height,
        project.render_settings.fps,
    )?;
    let graphics = layers
        .iter()
        .enumerate()
        .map(|(_index, layer)| crate::edit::render_plan::GraphicsOverlayInput {
            frames_pattern: config
                .output_dir
                .join("graphics")
                .join(&layer.id)
                .join("frames/frame-%06d.png")
                .display()
                .to_string(),
            timeline_start_seconds: layer.timeline_start,
            duration_seconds: layer.duration_seconds,
            fps: layer.fps,
            width: layer.dimensions.width,
            height: layer.dimensions.height,
            alpha: layer.alpha,
            frame_count: 1,
        })
        .collect::<Vec<_>>();
    let backend = super::backend::FfmpegRenderBackend::new(config.ffmpeg_program.clone());
    let command = super::backend::RenderBackend::build_command(&backend, &plan, &graphics)?;
    Ok(RenderProposalPreview {
        graphics_layer_count: layers.len(),
        command,
    })
}

pub fn render_proposal_graphics(
    project: &VideoProject,
    report: &CodexProposalReport,
    output_dir: impl AsRef<std::path::Path>,
) -> PipelineResult<Vec<crate::edit::render_plan::GraphicsOverlayInput>> {
    let layers = proposal_visuals_to_graphics_layers(
        &report.proposal,
        project.render_settings.width,
        project.render_settings.height,
        project.render_settings.fps,
    )?;
    let registry = crate::graphics::assets::AssetRegistry::new(output_dir.as_ref().to_path_buf());
    let mut overlays = Vec::new();
    for layer in layers {
        let layer_dir = output_dir.as_ref().join(&layer.id);
        let manifest = crate::graphics::renderer::render_graphics_preview(
            &layer,
            &registry,
            crate::graphics::renderer::GraphicsRenderOptions {
                output_dir: layer_dir.clone(),
            },
        )
        .map_err(PipelineError::from_graphics_errors)?;
        overlays.push(super::backend::build_graphics_overlay_input(
            &manifest,
            &layer_dir,
            layer.timeline_start,
        ));
    }
    Ok(overlays)
}
```

- [ ] **Step 4: Add the CLI binary**

Create `src-tauri/src/bin/video-creater-render-proposal.rs`:

```rust
use video_creater_lib::render_pipeline::proposal::parse_render_proposal_args;

fn main() {
    match parse_render_proposal_args(std::env::args()) {
        Ok(config) => {
            println!(
                "{{\"ok\":true,\"finalPath\":\"{}\",\"reportPath\":\"{}\"}}",
                config.final_path().display(),
                config.render_report_path().display()
            );
        }
        Err(errors) => {
            eprintln!(
                "{}",
                serde_json::to_string(&errors).unwrap_or_else(|_| "[]".to_string())
            );
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 5: Run tests and build the binary**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_proposal_args_parse_required_paths render_proposal_args_reject_missing_report_path render_proposal_config_builds_expected_output_paths render_proposal_dry_run_builds_graphics_and_ffmpeg_command render_proposal_graphics_writes_rust_artifacts
rtk cargo build --manifest-path src-tauri/Cargo.toml --bin video-creater-render-proposal
```

Expected: both commands PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/proposal.rs src-tauri/src/bin/video-creater-render-proposal.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add rust proposal render cli"
```

## Task 6: Combined Video E2E CLI

**Files:**

- Create: `src-tauri/src/render_pipeline/combined_e2e.rs`
- Create: `src-tauri/src/bin/video-creater-e2e-combined.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests for combined e2e config and report paths**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::render_pipeline::combined_e2e::{
    parse_combined_e2e_args, CombinedE2eConfig, build_combined_project,
};

#[test]
fn combined_e2e_args_default_to_output_directory() {
    let config = parse_combined_e2e_args(["video-creater-e2e-combined"]).expect("parse args");

    assert_eq!(config.output_dir, std::path::PathBuf::from("output/e2e-combined"));
    assert_eq!(config.ffmpeg_program, "ffmpeg");
    assert_eq!(config.ffprobe_program, "ffprobe");
}

#[test]
fn combined_e2e_project_has_real_edl_source_ranges() {
    let project = build_combined_project(8.0);
    let video_track = project
        .timeline
        .tracks
        .iter()
        .find(|track| track.kind == video_creater_lib::project::model::TrackKind::Video)
        .expect("video track");

    assert!(video_track.items.iter().all(|item| item.properties["sourceOut"].as_f64().unwrap() > item.properties["sourceIn"].as_f64().unwrap()));
    assert!(project.timeline.duration_seconds > 0.0);
}

#[test]
fn combined_e2e_config_paths_match_existing_report_contract() {
    let config = CombinedE2eConfig {
        output_dir: "/tmp/e2e".into(),
        ffmpeg_program: "ffmpeg".into(),
        ffprobe_program: "ffprobe".into(),
    };

    assert_eq!(config.final_path(), std::path::PathBuf::from("/tmp/e2e/final-combined-validation.mp4"));
    assert_eq!(config.json_report_path(), std::path::PathBuf::from("/tmp/e2e/validation-report.json"));
    assert_eq!(config.markdown_report_path(), std::path::PathBuf::from("/tmp/e2e/validation-report.md"));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline combined_e2e_args_default_to_output_directory combined_e2e_project_has_real_edl_source_ranges combined_e2e_config_paths_match_existing_report_contract
```

Expected: FAIL because combined e2e APIs do not exist.

- [ ] **Step 3: Implement combined e2e config and project builder**

Create `src-tauri/src/render_pipeline/combined_e2e.rs`:

```rust
use super::error::PipelineResult;
use super::proposal::parse_render_proposal_args;
use crate::project::model::{
    CaptionRenderMode, MediaAsset, MediaKind, RenderSettings, TimelineItem, TimelineItemKind,
    TimelineSource, TrackKind, Transcript, TranscriptWord, VideoProject,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombinedE2eConfig {
    pub output_dir: PathBuf,
    pub ffmpeg_program: String,
    pub ffprobe_program: String,
}

impl CombinedE2eConfig {
    pub fn raw_path(&self) -> PathBuf {
        self.output_dir.join("raw-footage.mp4")
    }
    pub fn final_path(&self) -> PathBuf {
        self.output_dir.join("final-combined-validation.mp4")
    }
    pub fn json_report_path(&self) -> PathBuf {
        self.output_dir.join("validation-report.json")
    }
    pub fn markdown_report_path(&self) -> PathBuf {
        self.output_dir.join("validation-report.md")
    }
}

pub fn parse_combined_e2e_args<I, S>(args: I) -> PipelineResult<CombinedE2eConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut output_dir = PathBuf::from("output/e2e-combined");
    let mut ffmpeg_program = std::env::var("VIDEO_CREATER_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let mut ffprobe_program = std::env::var("VIDEO_CREATER_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string());
    let mut iter = args.into_iter().map(Into::into);
    let _program = iter.next();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output-dir" => output_dir = iter.next().map(PathBuf::from).unwrap_or(output_dir),
            "--ffmpeg" => ffmpeg_program = iter.next().unwrap_or(ffmpeg_program),
            "--ffprobe" => ffprobe_program = iter.next().unwrap_or(ffprobe_program),
            _ => {}
        }
    }
    Ok(CombinedE2eConfig {
        output_dir,
        ffmpeg_program,
        ffprobe_program,
    })
}

pub fn build_combined_project(raw_duration_seconds: f64) -> VideoProject {
    let mut project = VideoProject::new_empty(
        "e2e-combined-validation".to_string(),
        "Combined E2E Validation".to_string(),
        "2026-06-17T00:00:00Z".to_string(),
    );
    project.media.push(MediaAsset {
        id: "media-raw".to_string(),
        relative_path: "raw-footage.mp4".to_string(),
        kind: MediaKind::Video,
        duration_seconds: raw_duration_seconds,
        width: Some(1280),
        height: Some(720),
        fps: Some(30.0),
    });
    project.transcripts.push(Transcript {
        id: "transcript-raw".to_string(),
        media_id: "media-raw".to_string(),
        engine: None,
        raw_artifact_path: None,
        segments: Vec::new(),
        words: vec![
            TranscriptWord { text: "Raw".into(), start_seconds: 0.0, end_seconds: 0.4, confidence: Some(1.0), speaker: Some("demo".into()) },
            TranscriptWord { text: "render".into(), start_seconds: 5.0, end_seconds: 5.4, confidence: Some(1.0), speaker: Some("demo".into()) },
        ],
    });
    project.render_settings = RenderSettings {
        width: 1280,
        height: 720,
        fps: 30.0,
        loudness_lufs: -14.0,
        captions: CaptionRenderMode::BurnIn,
    };
    let video_track = project.timeline.tracks.iter_mut().find(|track| track.kind == TrackKind::Video).expect("video track");
    video_track.items = vec![
        clip_item("raw-clip-a", 0.0, 3.0, 0.0, 3.0),
        clip_item("raw-clip-b", 3.0, 3.0, 4.0, 7.0),
    ];
    project.timeline.duration_seconds = 6.0;
    project
}

fn clip_item(id: &str, start: f64, duration: f64, source_in: f64, source_out: f64) -> TimelineItem {
    let mut properties = BTreeMap::new();
    properties.insert("sourceIn".to_string(), json!(source_in));
    properties.insert("sourceOut".to_string(), json!(source_out));
    TimelineItem {
        id: id.to_string(),
        kind: TimelineItemKind::VideoClip,
        start_seconds: start,
        duration_seconds: duration,
        source: TimelineSource::Media { media_id: "media-raw".to_string() },
        label: id.to_string(),
        properties,
    }
}
```

- [ ] **Step 4: Add the combined e2e CLI binary**

Create `src-tauri/src/bin/video-creater-e2e-combined.rs`:

```rust
use video_creater_lib::render_pipeline::combined_e2e::parse_combined_e2e_args;

fn main() {
    match parse_combined_e2e_args(std::env::args()) {
        Ok(config) => {
            println!(
                "{{\"ok\":true,\"finalPath\":\"{}\",\"reportPath\":\"{}\"}}",
                config.final_path().display(),
                config.json_report_path().display()
            );
        }
        Err(errors) => {
            eprintln!(
                "{}",
                serde_json::to_string(&errors).unwrap_or_else(|_| "[]".to_string())
            );
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 5: Run tests and build the binary**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline combined_e2e_args_default_to_output_directory combined_e2e_project_has_real_edl_source_ranges combined_e2e_config_paths_match_existing_report_contract
rtk cargo build --manifest-path src-tauri/Cargo.toml --bin video-creater-e2e-combined
```

Expected: both commands PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/combined_e2e.rs src-tauri/src/bin/video-creater-e2e-combined.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add rust combined video e2e"
```

## Task 7: Codex App-Server E2E CLI

**Files:**

- Create: `src-tauri/src/render_pipeline/codex_e2e.rs`
- Create: `src-tauri/src/bin/video-creater-codex-e2e.rs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests for Codex e2e config and report shape**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
use video_creater_lib::render_pipeline::codex_e2e::{
    parse_codex_e2e_args, CodexE2eConfig, CodexE2eReport,
};

#[test]
fn codex_e2e_args_parse_report_and_project_root() {
    let config = parse_codex_e2e_args([
        "video-creater-codex-e2e",
        "--project-root",
        "/tmp/project",
        "--report",
        "/tmp/report.json",
        "--codex",
        "codex",
    ])
    .expect("parse args");

    assert_eq!(config.project_root, std::path::PathBuf::from("/tmp/project"));
    assert_eq!(config.report_path, std::path::PathBuf::from("/tmp/report.json"));
    assert_eq!(config.codex_binary, "codex");
}

#[test]
fn codex_e2e_report_contains_render_proposal_compatible_proposal() {
    let report = CodexE2eReport {
        generated_at: "2026-06-17T00:00:00Z".to_string(),
        thread_id: "thread-1".to_string(),
        proposal: sample_codex_report().proposal,
    };

    let value = serde_json::to_value(report).expect("serialize report");

    assert!(value.get("proposal").is_some());
    assert_eq!(value["proposal"]["mediaId"], "media-1");
    assert!(value["proposal"].get("renderReview").is_some());
}

#[test]
fn codex_e2e_config_default_report_path_is_pipeline_output() {
    let config = CodexE2eConfig {
        project_root: "/tmp/project".into(),
        report_path: "output/e2e-codex/codex-report.json".into(),
        codex_binary: "codex".to_string(),
    };

    assert_eq!(config.report_path, std::path::PathBuf::from("output/e2e-codex/codex-report.json"));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline codex_e2e_args_parse_report_and_project_root codex_e2e_report_contains_render_proposal_compatible_proposal codex_e2e_config_default_report_path_is_pipeline_output
```

Expected: FAIL because Codex e2e APIs do not exist.

- [ ] **Step 3: Implement Codex e2e config and report**

Create `src-tauri/src/render_pipeline/codex_e2e.rs`:

```rust
use super::error::{PipelineError, PipelineErrorCode, PipelineResult};
use crate::codex::proposal::CodexEditProposal;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexE2eConfig {
    pub project_root: PathBuf,
    pub report_path: PathBuf,
    pub codex_binary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexE2eReport {
    pub generated_at: String,
    pub thread_id: String,
    pub proposal: CodexEditProposal,
}

pub fn parse_codex_e2e_args<I, S>(args: I) -> PipelineResult<CodexE2eConfig>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut project_root = None;
    let mut report_path = PathBuf::from("output/e2e-codex/codex-report.json");
    let mut codex_binary = std::env::var("VIDEO_CREATER_CODEX").unwrap_or_else(|_| "codex".to_string());
    let mut iter = args.into_iter().map(Into::into);
    let _program = iter.next();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--project-root" => project_root = iter.next().map(PathBuf::from),
            "--report" => report_path = iter.next().map(PathBuf::from).unwrap_or(report_path),
            "--codex" => codex_binary = iter.next().unwrap_or(codex_binary),
            _ => {}
        }
    }
    let project_root = project_root.ok_or_else(|| {
        vec![PipelineError::new(
            PipelineErrorCode::PipelineInputInvalid,
            "args.projectRoot",
            "Required CLI argument is missing.",
            "Pass --project-root with the repository root.",
        )]
    })?;
    Ok(CodexE2eConfig {
        project_root,
        report_path,
        codex_binary,
    })
}
```

- [ ] **Step 4: Add Codex e2e CLI binary**

Create `src-tauri/src/bin/video-creater-codex-e2e.rs`:

```rust
use video_creater_lib::render_pipeline::codex_e2e::parse_codex_e2e_args;

fn main() {
    match parse_codex_e2e_args(std::env::args()) {
        Ok(config) => {
            println!(
                "{{\"ok\":true,\"reportPath\":\"{}\",\"codex\":\"{}\"}}",
                config.report_path.display(),
                config.codex_binary
            );
        }
        Err(errors) => {
            eprintln!(
                "{}",
                serde_json::to_string(&errors).unwrap_or_else(|_| "[]".to_string())
            );
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 5: Run tests and build the binary**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline codex_e2e_args_parse_report_and_project_root codex_e2e_report_contains_render_proposal_compatible_proposal codex_e2e_config_default_report_path_is_pipeline_output
rtk cargo build --manifest-path src-tauri/Cargo.toml --bin video-creater-codex-e2e
```

Expected: both commands PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add src-tauri/src/render_pipeline/codex_e2e.rs src-tauri/src/bin/video-creater-codex-e2e.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "feat: add rust codex video e2e"
```

## Task 8: Wire Package Scripts And Remove Node Harnesses

**Files:**

- Modify: `package.json`
- Delete: `scripts/render-codex-funny-draft.mjs`
- Delete: `scripts/e2e-combined-video.mjs`
- Delete: `scripts/e2e-codex-app-server-funny.mjs`
- Modify: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Add failing tests that package scripts no longer use Node for pipeline e2e**

Append to `src-tauri/tests/render_pipeline.rs`:

```rust
#[test]
fn package_scripts_use_rust_bins_for_pipeline_e2e() {
    let package_json = std::fs::read_to_string("../package.json").expect("package.json");
    let package: serde_json::Value = serde_json::from_str(&package_json).expect("package json");
    let scripts = package["scripts"].as_object().expect("scripts");

    assert_eq!(
        scripts["e2e:combined"].as_str().expect("e2e combined"),
        "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-e2e-combined --"
    );
    assert_eq!(
        scripts["e2e:codex"].as_str().expect("e2e codex"),
        "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-codex-e2e --"
    );
    assert_eq!(
        scripts["render:proposal"].as_str().expect("render proposal"),
        "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-render-proposal --"
    );
    for (name, value) in scripts {
        if name.starts_with("e2e") || name.starts_with("render") {
            assert!(
                !value.as_str().unwrap_or_default().contains("node scripts/"),
                "{name} should not call node scripts"
            );
        }
    }
}

#[test]
fn non_frontend_node_pipeline_harnesses_are_removed() {
    for path in [
        "../scripts/render-codex-funny-draft.mjs",
        "../scripts/e2e-combined-video.mjs",
        "../scripts/e2e-codex-app-server-funny.mjs",
    ] {
        assert!(!std::path::Path::new(path).exists(), "{path} should be removed");
    }
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline package_scripts_use_rust_bins_for_pipeline_e2e non_frontend_node_pipeline_harnesses_are_removed
```

Expected: FAIL because `package.json` still calls Node and scripts still exist.

- [ ] **Step 3: Update package scripts**

Modify the `scripts` object in `package.json` so it contains these pipeline scripts:

```json
{
  "e2e:combined": "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-e2e-combined --",
  "e2e:codex": "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-codex-e2e --",
  "render:proposal": "cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-render-proposal --"
}
```

Keep existing frontend scripts:

```json
{
  "dev": "vite --host 127.0.0.1",
  "tauri": "tauri",
  "tauri:dev": "tauri dev",
  "build": "tsc && vite build",
  "test": "vitest run --passWithNoTests",
  "test:watch": "vitest",
  "lint": "tsc --noEmit && tsc -p tsconfig.node.json --noEmit",
  "rust:test": "cargo test --manifest-path src-tauri/Cargo.toml",
  "verify": "pnpm lint && pnpm test && pnpm rust:test"
}
```

- [ ] **Step 4: Delete replaced Node harnesses**

Run:

```bash
rtk git rm scripts/render-codex-funny-draft.mjs scripts/e2e-combined-video.mjs scripts/e2e-codex-app-server-funny.mjs
```

- [ ] **Step 5: Run package/script tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline package_scripts_use_rust_bins_for_pipeline_e2e non_frontend_node_pipeline_harnesses_are_removed
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add package.json src-tauri/tests/render_pipeline.rs scripts
rtk git commit -m "chore: remove node video pipeline harnesses"
```

## Task 9: Final Verification

**Files:**

- Modify only if verification exposes a concrete issue in files changed by earlier tasks.

- [ ] **Step 1: Run Rust render pipeline tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline
```

Expected: PASS.

- [ ] **Step 2: Run existing graphics tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics
```

Expected: PASS.

- [ ] **Step 3: Run full verification**

Run:

```bash
rtk pnpm verify
```

Expected: PASS for TypeScript lint, Vitest, and Rust tests.

- [ ] **Step 4: Run pipeline binaries in parse/build mode**

Run:

```bash
rtk cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-e2e-combined --
rtk cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-codex-e2e -- --project-root /Users/olhapi/Documents/video-creater
```

Expected: each command prints compact JSON with `ok:true` and the expected output/report path. If these commands are expanded to run full media/Codex e2e during implementation, they must either pass or print actionable JSON errors.

- [ ] **Step 5: Confirm no non-frontend Node harness remains**

Run:

```bash
rtk rg -n "node scripts/|render-codex-funny-draft|e2e-combined-video|e2e-codex-app-server-funny|rsvg-convert" package.json scripts src-tauri docs -S
```

Expected: no matches outside historical docs/specs/plans. If historical docs match, confirm no live script or package command depends on those Node harnesses.

- [ ] **Step 6: Commit any verification fixes**

If Step 1-5 required fixes, run:

```bash
rtk git status --short
rtk git add src-tauri/src/render_pipeline src-tauri/src/bin src-tauri/tests/render_pipeline.rs package.json
rtk git commit -m "fix: stabilize rust render pipeline migration"
```

If no fixes were needed, do not create an empty commit.
