# Real Render Export Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace sample WebM export reports and render-related Temporal placeholders with a real Rust split-project render/report path.

**Architecture:** Add a Rust `project_export` module that can build an EDL-backed WebM render plan, run the existing GStreamer/GES backend, validate/probe the output, write render reports/logs, and apply project actions. Wire it into a Tauri command for the UI and into Temporal render/export-media activity helpers.

**Tech Stack:** Rust/Tauri, GStreamer/GES render backend, existing split-project project actions, React/Vitest frontend tests.

---

### Task 1: Rust Project WebM Render Module

**Files:**
- Create: `src-tauri/src/render_pipeline/project_export.rs`
- Modify: `src-tauri/src/render_pipeline/mod.rs`
- Test: `src-tauri/tests/temporal_workflows.rs`

- [ ] Write failing tests for building a render plan from split-project clips and converting pipeline reports into `ProjectRenderReport`.
- [ ] Implement `ProjectWebmRenderRequest`, `ProjectWebmRenderResult`, render-plan builder, report path helpers, and report conversion.
- [ ] Run targeted Rust tests and verify they pass.

### Task 2: Tauri Command and UI Wiring

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src/lib/project.ts`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] Write failing frontend test proving WebM export calls `render_webm_to_split_project_folder` and does not synthesize sample report actions.
- [ ] Add the Tauri command and TypeScript bridge.
- [ ] Replace `buildSampleRenderReport` usage in the export path with the Rust command result.
- [ ] Run targeted Vitest.

### Task 3: Temporal Activity Wiring

**Files:**
- Modify: `src-tauri/src/workflows/mod.rs`
- Test: `src-tauri/tests/temporal_workflows.rs`

- [ ] Write failing tests for `BuildRenderPlan`, `RenderMedia`, `ValidateRenderedMedia`, and `AttachRenderReport` helper outputs.
- [ ] Implement export-media/render-draft activity helpers that reuse `project_export` and return serializable worker-chain payloads.
- [ ] Replace Temporal worker placeholder branches for real render/export-media inputs.
- [ ] Run targeted Rust tests.

### Task 4: Verification

**Files:**
- Existing Rust and frontend tests only.

- [ ] Run `pnpm test src/components/workspace/editor-workspace.test.tsx src/lib/render.test.ts`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml temporal_workflows -- --test-threads=1`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml render_pipeline -- --test-threads=1` if render module changes compile cleanly.

### Task 5: Temporal Render Workflow Orchestration Follow-Up

**Files:**
- Modify: `src-tauri/src/workflows/mod.rs`
- Test: `src-tauri/tests/temporal_workflows.rs`

- [ ] Write a failing test for a replayable `RenderDraft` start request that carries `RenderQualityProfile` instead of export-policy profiles.
- [ ] Write a failing test for a render workflow activity plan that sequences `BuildRenderPlan`, `RenderMedia`, `ValidateRenderedMedia`, and `AttachRenderReport` without falling back to registered placeholder output.
- [ ] Implement the render draft start request helper and serializable activity-plan helper.
- [ ] Replace the feature-gated `VideoCreaterRenderDraftWorkflow` registered shell with a real Temporal workflow body that mirrors the tested activity sequence.
- [ ] Replace the feature-gated `VideoCreaterExportMediaWorkflow` registered shell with an explicit unsupported workflow response until MP4/MOV encoder policy gates are resolved.
- [ ] Run targeted Rust tests with and without the `temporal-worker` feature.
