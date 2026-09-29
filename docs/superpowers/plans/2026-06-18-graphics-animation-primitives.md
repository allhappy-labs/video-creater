# Graphics Animation Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add agent-friendly declarative animation primitives to the Rust graphics IR and render them as real PNG frame sequences for ffmpeg compositing.

**Architecture:** Keep animations declarative JSON on each primitive node, evaluate them inside Rust before rasterization, and leave static layers on the existing one-frame path. Proposal overlays may pass animated primitive nodes directly through the existing `nodes` escape hatch.

**Tech Stack:** Rust, serde, tiny-skia, cosmic-text, ffmpeg image sequences, existing Video Creater render pipeline.

---

### Task 1: Graphics IR And Renderer Animation Support

**Files:**
- Modify: `src-tauri/src/graphics/ir.rs`
- Create: `src-tauri/src/graphics/animation.rs`
- Modify: `src-tauri/src/graphics/mod.rs`
- Modify: `src-tauri/src/graphics/renderer.rs`
- Modify: `src-tauri/src/graphics/validation.rs`
- Test: `src-tauri/tests/graphics.rs`

- [ ] **Step 1: Write failing tests**

Add tests that deserialize a node with `animate.keyframes`, expect `render_graphics_preview` to produce `ceil(durationSeconds * fps)` frame PNGs, and expect invalid keyframe timing to return an actionable error at `nodes[0].animate.keyframes[0].at`.

- [ ] **Step 2: Verify red**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics renders_animated_nodes_as_frame_sequence rejects_invalid_node_animation_actionably`

Expected: FAIL because `animate` is ignored and only one frame is rendered.

- [ ] **Step 3: Implement minimal animation evaluator**

Add `NodeAnimation`, `AnimationKeyframe`, and `Easing` to the IR; add evaluator support for `x`, `y`, `scale`, and `opacity`; clone nodes into their per-frame animated state before drawing; preserve static one-frame rendering when no node has animation.

- [ ] **Step 4: Verify green**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test graphics renders_animated_nodes_as_frame_sequence rejects_invalid_node_animation_actionably`

Expected: PASS.

### Task 2: Proposal Schema And Agent Surface

**Files:**
- Modify: `src-tauri/src/codex/app_server.rs`
- Test: `src-tauri/tests/codex_app_server.rs`
- Test: `src-tauri/tests/render_pipeline.rs`

- [ ] **Step 1: Write failing schema/proposal tests**

Add tests that the proposal output schema exposes overlay `nodes` with optional `animate`, and that proposal custom nodes preserve `animate` through `proposal_visuals_to_graphics_layers`.

- [ ] **Step 2: Verify red**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --test codex_app_server turn_request_lists_available_animation_primitives` and `rtk cargo test --manifest-path src-tauri/Cargo.toml --test render_pipeline proposal_visuals_preserve_custom_node_animation`

Expected: FAIL until schema text and typed node animation are added.

- [ ] **Step 3: Implement schema/prompt updates**

Update the app-server prompt to document animated primitive nodes and update the JSON schema so agents can return `nodes` and nested `animate` objects without violating `additionalProperties: false`.

- [ ] **Step 4: Verify green**

Run the two targeted tests again. Expected: PASS.

### Task 3: End-To-End Animated Video Verification

**Files:**
- Temporary fixture under `/tmp/video-creater-animated-primitives-20260618`

- [ ] **Step 1: Render a fixture video**

Create a tiny source MP4, write a proposal with animated primitive overlay nodes, render through `video-creater-render-proposal`, and verify the final MP4 with `ffprobe`.

- [ ] **Step 2: Visual QA sampled frames**

Extract first, middle, and late frames and inspect them for readable motion states, safe-zone behavior, and alpha compositing.

- [ ] **Step 3: Full verification and commit**

Run `rtk cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, targeted graphics/render-pipeline tests, and `rtk pnpm verify`; stage only task files and commit with a Conventional Commit message.
