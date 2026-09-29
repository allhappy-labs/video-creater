# Draft Render Quality Application Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make proposal draft renders actually use effective draft dimensions and fps, with graphics generated at the same output size.

**Architecture:** Apply `effective_quality_settings` before constructing `RenderPlan`, then treat `RenderPlan.width`, `height`, and `fps` as authoritative for command construction and graphics layer generation. Keep public graphics helper APIs stable by deriving draft settings internally when no render plan is passed.

**Tech Stack:** Rust, serde render plan model, GStreamer/GES backend boundary, Cargo render-pipeline tests.

---

## File Structure

- Modify `src-tauri/src/render_pipeline/proposal.rs`: apply draft settings in `proposal_to_render_plan`, use plan dimensions in preview graphics, and pass a render-dimension struct into actual graphics rendering.
- Modify `src-tauri/tests/render_pipeline.rs`: add/adjust tests for render plan dimensions, preview command metadata, and graphics manifest dimensions.

---

### Task 1: Render Plan Draft Settings

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`

- [x] **Step 1: Write the failing render-plan expectation**

In `proposal_render_plan_uses_selected_clip_ranges`, change:

```rust
assert_eq!(plan.width, 1920);
assert_eq!(plan.height, 1080);
assert_eq!(plan.fps, 24.0);
```

to:

```rust
assert_eq!(plan.width, 960);
assert_eq!(plan.height, 540);
assert_eq!(plan.fps, 24.0);
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml proposal_render_plan_uses_selected_clip_ranges --test render_pipeline -- --test-threads=1
```

Expected: FAIL because `proposal_to_render_plan` still emits `1920x1080`.

- [x] **Step 3: Apply effective draft settings in `proposal_to_render_plan`**

In `src-tauri/src/render_pipeline/proposal.rs`, inside `proposal_to_render_plan`, insert:

```rust
let quality_profile = RenderQualityProfile::DraftWebm;
let quality_settings = effective_quality_settings(
    quality_profile.clone(),
    project.render_settings.width,
    project.render_settings.height,
    project.render_settings.fps,
);
```

Then construct `RenderPlan` with:

```rust
width: quality_settings.output_width,
height: quality_settings.output_height,
fps: quality_settings.output_fps,
quality_profile,
```

- [x] **Step 4: Run test to verify it passes**

Run the same cargo command. Expected: PASS.

---

### Task 2: Preview Graphics and Command Dimensions

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`

- [x] **Step 1: Write failing preview command assertions**

In `render_proposal_dry_run_builds_graphics_and_gstreamer_command`, add:

```rust
assert!(preview.command.args.contains(&"--size=960x540".to_string()));
assert!(preview.command.args.contains(&"--fps=24.000".to_string()));
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_proposal_dry_run_builds_graphics_and_gstreamer_command --test render_pipeline -- --test-threads=1
```

Expected: FAIL before Task 1 implementation, or PASS after Task 1 if command construction already consumes the corrected plan.

- [x] **Step 3: Use plan dimensions for preview graphics**

In `build_render_proposal_preview`, change both preview graphics calls from project settings:

```rust
project.render_settings.width,
project.render_settings.height,
project.render_settings.fps,
```

to:

```rust
plan.width,
plan.height,
plan.fps,
```

- [x] **Step 4: Run preview test**

Run the same cargo command. Expected: PASS.

---

### Task 3: Actual Graphics Artifact Dimensions

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`

- [x] **Step 1: Write failing graphics manifest test**

Append this test near the existing proposal graphics tests:

```rust
#[test]
fn render_proposal_graphics_uses_effective_draft_dimensions() {
    let temp = tempfile::tempdir().expect("temp graphics dir");
    let mut project = sample_project_for_proposal();
    project.render_settings.width = 1920;
    project.render_settings.height = 1080;
    project.render_settings.fps = 60.0;

    let graphics = render_proposal_graphics(&project, &sample_codex_report(), temp.path())
        .expect("proposal graphics should render");

    let caption = graphics
        .iter()
        .find(|graphics| graphics.manifest.source_layer_ids.contains(&"proposal-caption-1".to_string()))
        .expect("caption graphics should exist");

    assert_eq!(caption.manifest.dimensions.width, 960);
    assert_eq!(caption.manifest.dimensions.height, 540);
    assert_eq!(caption.manifest.fps, 24.0);
}
```

- [x] **Step 2: Run test to verify it fails**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml render_proposal_graphics_uses_effective_draft_dimensions --test render_pipeline -- --test-threads=1
```

Expected: FAIL because graphics manifests still use `1920x1080` and `60.0`.

- [x] **Step 3: Add a focused render-dimensions helper**

In `proposal.rs`, add near the rendered artifact structs:

```rust
#[derive(Debug, Clone, Copy)]
struct RenderOutputSettings {
    width: u32,
    height: u32,
    fps: f64,
}
```

Add helper functions:

```rust
fn render_output_settings_from_plan(plan: &RenderPlan) -> RenderOutputSettings {
    RenderOutputSettings {
        width: plan.width,
        height: plan.height,
        fps: plan.fps,
    }
}

fn draft_render_output_settings(project: &VideoProject) -> RenderOutputSettings {
    let settings = effective_quality_settings(
        RenderQualityProfile::DraftWebm,
        project.render_settings.width,
        project.render_settings.height,
        project.render_settings.fps,
    );
    RenderOutputSettings {
        width: settings.output_width,
        height: settings.output_height,
        fps: settings.output_fps,
    }
}
```

- [x] **Step 4: Thread output settings through graphics generation**

Change `render_proposal_graphics_artifacts` to accept `settings: RenderOutputSettings`, and call `render_proposal_graphics_artifacts_with_gpu_renderer(report, output_dir, settings, ...)`.

In `run_render_proposal_with_runner`, call:

```rust
render_proposal_graphics_artifacts(
    &project,
    &report,
    &config.output_dir.join(GRAPHICS_DIR_NAME),
    render_output_settings_from_plan(&plan),
)
```

In public graphics helper paths, call:

```rust
let settings = draft_render_output_settings(project);
render_proposal_graphics_artifacts_with_gpu_renderer(report, output_dir, settings, gpu_renderer)
```

Inside `render_proposal_graphics_artifacts_with_gpu_renderer`, replace project render settings arguments with:

```rust
settings.width,
settings.height,
settings.fps,
```

- [x] **Step 5: Run graphics manifest test**

Run the same cargo command. Expected: PASS.

---

### Task 4: Verification and Commit

**Files:**
- Modify: `docs/superpowers/plans/2026-06-19-draft-render-quality-application.md`

- [x] **Step 1: Run focused tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml proposal_render_plan_uses_selected_clip_ranges --test render_pipeline -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml render_proposal_dry_run_builds_graphics_and_gstreamer_command --test render_pipeline -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml render_proposal_graphics_uses_effective_draft_dimensions --test render_pipeline -- --test-threads=1
```

Expected: all PASS.

- [x] **Step 2: Run full render-pipeline suite**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline -- --test-threads=1
```

Expected: all render-pipeline tests PASS.

- [x] **Step 3: Review diff and commit**

Run:

```bash
rtk git diff --check
rtk git diff --stat
rtk git status --short
```

Commit:

```bash
rtk git add docs/superpowers/specs/2026-06-19-draft-render-quality-application-design.md docs/superpowers/plans/2026-06-19-draft-render-quality-application.md src-tauri/src/render_pipeline/proposal.rs src-tauri/tests/render_pipeline.rs
rtk git commit -m "perf: apply draft render quality settings"
```
