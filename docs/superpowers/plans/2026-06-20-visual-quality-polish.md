# Visual Quality Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve generated visual quality for the remaining high-impact gaps: default captions/overlays should animate, visual QA metrics should be reviewable, and final WebM should carry meaningful quality settings.

**Architecture:** Reuse the existing Rust motion preset, graphics QA, render report, and quality profile modules. Keep EDL-first proposal validation unchanged. Add small helpers in the render proposal conversion path so template and non-template visuals share deterministic motion behavior, then thread QA reports into render metadata and apply WebM quality intent through typed settings plus GES VP8/VP9 encoder element properties.

**Tech Stack:** Rust, TypeScript, Vitest, Cargo tests, existing `graphics`, `render_pipeline`, and frontend render model modules.

---

## File Structure

- Modify `src-tauri/src/render_pipeline/proposal.rs`: add preset-backed animations for default captions/overlays, capture CPU/GPU visual QA reports, and pass QA summaries to render reports.
- Modify `src-tauri/src/render_pipeline/report.rs`: extend `RenderGraphicsReport` with sampled frames and metric details.
- Modify `src-tauri/src/render_pipeline/quality.rs`: make `FinalWebm` materially different by assigning a bitrate and explicit speed hint.
- Modify `src-tauri/src/render_pipeline/gstreamer_backend.rs`: apply quality settings to the GES WebM encoding profile with VP8 draft and VP9 final encoder properties.
- Modify `src-tauri/Cargo.toml`: enable the GStreamer PBUtils binding feature needed for encoding-profile element properties.
- Modify `src/lib/render.ts`: mirror render graphics QA report fields in the frontend model.
- Modify `src/lib/render.test.ts`: lock the frontend report shape.
- Modify `src-tauri/tests/render_pipeline.rs`: add red/green tests for animation, QA metadata, and final quality settings.

## Task 1: Animate Default Generated Captions And Overlays

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`

- [ ] **Step 1: Write failing tests**

Add tests asserting default captions and non-template overlays from `proposal_visuals_to_graphics_layers` contain animated nodes.

- [ ] **Step 2: Run focused tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_default_visuals_animate`

Expected: FAIL because current default caption and non-template overlay nodes use `animate: None`.

- [ ] **Step 3: Implement minimal preset application**

Use `MotionPresetId::SnapPopV1` for default captions and `MotionPresetId::SlideFadeUpV1` for default overlays. Apply preset animations to the backing/accent/text nodes generated in `caption_to_graphics_layer` and `overlay_to_graphics_layer`.

- [ ] **Step 4: Verify**

Run the same focused test. Expected: PASS.

## Task 2: Report Visual QA Metrics

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/src/render_pipeline/proposal.rs`
- Modify: `src-tauri/src/render_pipeline/report.rs`
- Modify: `src/lib/render.ts`
- Modify: `src/lib/render.test.ts`

- [ ] **Step 1: Write failing tests**

Add Rust report serialization tests that expect `sampledFrames` and `qaMetrics` for graphics entries. Add TypeScript render model tests that accept the same shape.

- [ ] **Step 2: Run focused tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline render_report_serializes_graphics_visual_qa_metrics`

Run: `rtk pnpm test -- --run src/lib/render.test.ts`

Expected: FAIL because `RenderGraphicsReport` only exposes pass/fail status.

- [ ] **Step 3: Implement report fields**

Add `sampled_frames: Vec<String>` and `qa_metrics: BTreeMap<String, String>` to Rust reports. Store CPU QA `visibleAlphaRatio` and `temporalDelta`; store GPU QA `opaqueRatio`, `saturatedRatio`, `edgeRatio`, and `temporalDelta`.

- [ ] **Step 4: Verify**

Run the same Rust and TypeScript tests. Expected: PASS.

## Task 3: Make Final WebM Quality Meaningful

**Files:**
- Modify: `src-tauri/tests/render_pipeline.rs`
- Modify: `src-tauri/src/render_pipeline/quality.rs`
- Modify: `src-tauri/src/render_pipeline/gstreamer_backend.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Write failing test**

Update `final_quality_settings_preserve_source_dimensions_and_fps` to assert that `FinalWebm` has a non-empty bitrate and a distinct high-quality speed hint. Add a GES profile test that inspects the WebM video `EncodingProfile` and asserts VP8 draft / VP9 final element properties.

- [ ] **Step 2: Run focused test**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline final_quality_settings_preserve_source_dimensions_and_fps`

Expected: FAIL because `FinalWebm` currently has `video_bitrate_kbps: None`.

- [ ] **Step 3: Implement final bitrate**

Set final WebM bitrate based on output resolution with a minimum high-quality floor. Keep draft settings fast. Enable PBUtils `v1_20`, choose VP8 for draft and VP9 for final, and attach `target-bitrate`, `deadline`, `cpu-used`, and `threads` through GES encoder element properties.

- [ ] **Step 4: Verify**

Run the same focused test. Expected: PASS.

## Task 4: Final Verification

**Files:**
- All modified files above.

- [ ] **Step 1: Run focused Rust tests**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_default_visuals_animate render_report_serializes_graphics_visual_qa_metrics final_quality_settings_preserve_source_dimensions_and_fps`

- [ ] **Step 2: Run focused TypeScript tests**

Run: `rtk pnpm test -- --run src/lib/render.test.ts`

- [ ] **Step 3: Inspect diff**

Run: `rtk git diff --stat`

Confirm only the planned files changed, apart from pre-existing untracked render artifacts.
