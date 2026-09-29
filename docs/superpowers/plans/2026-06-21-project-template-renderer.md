# Project Template Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable `projects/<slug>/` folder workflow that renders a configured motion template without creating one-off Rust examples.

**Architecture:** Introduce a `render_pipeline::template_project` module that loads `video-creater.project.json` plus `render-template.json`, expands the requested template through the existing Rust graphics template renderer, writes frames to `generated/graphics/<outputName>/`, exports video to `renders/`, and writes `renders/render-report.json`. Add a thin `video-creater-render-template` binary that invokes this module.

**Tech Stack:** Rust, serde JSON, existing `VideoProject` storage, existing graphics IR/template renderer, existing WebM GStreamer exporter, `ffprobe` for external verification.

---

### Task 1: Project Folder Config And Argument Parsing

**Files:**
- Create: `src-tauri/src/render_pipeline/template_project.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/render_template_project.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn render_template_args_default_to_project_render_template_config() {
    let config = parse_render_template_args([
        "video-creater-render-template",
        "--project-root",
        "/tmp/video-creater/projects/gradient-background-loop",
    ])
    .expect("parse args");

    assert_eq!(config.project_root, Path::new("/tmp/video-creater/projects/gradient-background-loop"));
    assert_eq!(
        config.template_config_path,
        Path::new("/tmp/video-creater/projects/gradient-background-loop/render-template.json")
    );
}

#[test]
fn template_render_config_deserializes_project_manifest_shape() {
    let config: TemplateRenderConfig = serde_json::from_str(r#"{
      "schemaVersion": 1,
      "templateId": "gradient-background-loop-v1",
      "outputName": "gradient-background-loop-full-hd",
      "fields": { "headline": "Love\nwins." },
      "renderSettings": {
        "width": 1920,
        "height": 1080,
        "fps": 30,
        "durationSeconds": 4,
        "format": "webm"
      }
    }"#).expect("config json");

    assert_eq!(config.template_id, "gradient-background-loop-v1");
    assert_eq!(config.render_settings.width, 1920);
    assert_eq!(config.render_settings.format, TemplateRenderFormat::Webm);
}
```

- [ ] **Step 2: Verify red**

Run: `rtk bash -lc 'cargo test --manifest-path src-tauri/Cargo.toml --test render_template_project render_template_args -- --nocapture'`

Expected: compile failure because `render_template_project` test target and `template_project` module do not exist.

- [ ] **Step 3: Implement minimal parser/config types**

Add `TemplateRenderConfig`, `TemplateRenderSettings`, `TemplateRenderFormat`, `RenderTemplateProjectConfig`, and `parse_render_template_args`.

- [ ] **Step 4: Verify green**

Run: `rtk bash -lc 'cargo test --manifest-path src-tauri/Cargo.toml --test render_template_project render_template_args -- --nocapture'`

Expected: parser tests pass.

### Task 2: Render Project Folder To Reproducible Artifacts

**Files:**
- Modify: `src-tauri/src/render_pipeline/template_project.rs`
- Test: `src-tauri/tests/render_template_project.rs`

- [ ] **Step 1: Write failing render-layout test**

```rust
#[test]
fn render_template_project_writes_frames_video_and_report() {
    let dir = tempfile::tempdir().expect("project dir");
    create_gradient_template_project(dir.path());

    let config = RenderTemplateProjectConfig {
        project_root: dir.path().to_path_buf(),
        template_config_path: dir.path().join("render-template.json"),
    };

    let result = run_render_template_project(&config).expect("render template project");

    assert_eq!(result.video_path, dir.path().join("renders/gradient-background-loop-test.webm"));
    assert!(result.video_path.exists());
    assert!(dir.path().join("generated/graphics/gradient-background-loop-test/preview.png").exists());
    assert!(dir.path().join("renders/render-report.json").exists());
}
```

- [ ] **Step 2: Verify red**

Run: `rtk bash -lc 'cargo test --manifest-path src-tauri/Cargo.toml --test render_template_project render_template_project_writes_frames_video_and_report -- --nocapture'`

Expected: failure because `run_render_template_project` is not implemented.

- [ ] **Step 3: Implement render pipeline**

Load project/config, build `TemplateRenderLayer`, call `template_layer_to_graphics_ir`, render with `render_graphics_preview`, export WebM with `export_frame_sequence_to_webm`, and write a JSON report with artifact paths, dimensions, fps, frame count, duration, and validation expectations.

- [ ] **Step 4: Verify green**

Run: `rtk bash -lc 'cargo test --manifest-path src-tauri/Cargo.toml --test render_template_project render_template_project_writes_frames_video_and_report -- --nocapture'`

Expected: render test passes and artifact paths are under the temp project folder.

### Task 3: CLI Entrypoint And Sample Project

**Files:**
- Create: `src-tauri/src/bin/video-creater-render-template.rs`
- Create: `projects/gradient-background-loop/video-creater.project.json`
- Create: `projects/gradient-background-loop/render-template.json`

- [ ] **Step 1: Add CLI binary**

`main` calls `parse_render_template_args(std::env::args()).and_then(|config| run_render_template_project(&config))`, prints `{"ok":true,"videoPath":"...","reportPath":"..."}` on success, and prints serialized pipeline errors on failure.

- [ ] **Step 2: Add sample project folder**

Create `projects/gradient-background-loop/` with the approved `Gradient Background Loop` template config and no media requirement.

- [ ] **Step 3: Run real project render**

Run:

```bash
rtk bash -lc 'cargo run --manifest-path src-tauri/Cargo.toml --bin video-creater-render-template -- --project-root projects/gradient-background-loop'
```

Expected: writes `projects/gradient-background-loop/renders/gradient-background-loop-full-hd.webm` and `projects/gradient-background-loop/renders/render-report.json`.

### Task 4: Verification

**Files:**
- No new files.

- [ ] **Step 1: Run focused tests**

```bash
rtk bash -lc 'cargo test --manifest-path src-tauri/Cargo.toml --test render_template_project -- --nocapture'
```

- [ ] **Step 2: Run touched-file formatting**

```bash
rtk rustfmt --edition 2021 --check src-tauri/src/render_pipeline/template_project.rs src-tauri/src/bin/video-creater-render-template.rs src-tauri/tests/render_template_project.rs
```

- [ ] **Step 3: Validate sample output**

```bash
rtk ffprobe -v error -show_entries format=duration:stream=codec_type,width,height,r_frame_rate -of json projects/gradient-background-loop/renders/gradient-background-loop-full-hd.webm
```

Expected: one 1920x1080 video stream at 30 fps and roughly 4 seconds.

---

Self-review: The plan covers config parsing, project-folder rendering, CLI execution, sample project layout, and verification. It intentionally limits the first slice to WebM because the existing Rust-owned graphics exporter supports WebM; MP4 can be layered on later through the same project-folder contract.
