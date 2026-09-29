# Rendering Animation Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden Rust-native graphics primitives and animations after parallel subagent review so agent-created graphics fail clearly, render deterministically, avoid runaway work, and stay aligned with ffmpeg image-sequence playback while ffmpeg remains the video assembler.

**Architecture:** Keep canonical graphics IR, validation, frame rasterization, and artifacts in Rust. Keep ffmpeg for final composition, but make Rust output directories, animation timing, overlay enable windows, and final media validation explicit enough that a future non-ffmpeg backend can consume the same manifest contract. Preserve `imageRef` as an IR primitive; defer project-wide approved asset catalog plumbing to a follow-up.

**Tech Stack:** Rust, serde JSON schema/proposals, tiny-skia, cosmic-text, image crate, ffmpeg/ffprobe, cargo test.

---

## Subagent Feedback Summary

- Three reviewers independently found stale frame files in deterministic graphics layer directories.
- Two reviewers found animation sample times do not match ffmpeg image timestamps and overlay end windows are inclusive.
- Two reviewers found missing render-budget guardrails for animated layers.
- Two reviewers found final MP4 duration is not validated against the EDL/render plan.
- Two reviewers found `imageRef` proposal plumbing and template motion require broader product work.
- One reviewer found sparse keyframes reset omitted properties, text scale does not scale glyphs, and repeat/delay validation is too loose.
- One reviewer found app-server schema is too broad for agent-created primitives.

## Immediate Scope

- [x] Add failing tests for stale frame cleanup, frame-time sampling, sparse keyframes, animation validation budgets, and final media duration validation.
- [x] Clear/recreate each graphics `frames/` directory before rendering and keep manifest-counted artifacts authoritative.
- [x] Sample animated frames at `frame_index / fps` and make ffmpeg overlay enable windows exclusive at the end.
- [x] Add animation/render-budget validation: max animated frame count, max pixel frame budget, bounded repeat, strictly increasing keyframes, and delay shorter than layer duration.
- [x] Change animation evaluation to interpolate each property from surrounding keyframes that actually define that property.
- [x] Scale text `fontSize` on uniform scale animations so agent-visible scale matches rendered glyphs.
- [x] Add a per-render decoded `imageRef` cache and clip scaled-image drawing loops to the visible pixmap.
- [x] Extend `ExpectedMedia` duration validation and pass render-plan duration with a one-frame tolerance for proposal renders.
- [x] Tighten the app-server primitive schema minimally: non-empty nodes, required `animate.keyframes`, compact translation semantics guidance, and public `sourceBeat` on overlays.
- [x] Run focused tests, full `pnpm verify`, generate/verify an animated sample video, then commit with a Conventional Commit message.

## Deferred Roadmap

- [ ] Add a project-approved `availableImageRefs` catalog and build proposal render asset registries from it.
- [ ] Route `templateId` overlays through template-specific motion adapters and add built-in animation presets for catalog templates.
- [x] Emit explicit manifest playback semantics (`sequence` vs `staticHold`, `startNumber`, frame duration, expected frame list).
- [ ] Write failed render reports/log artifacts for every failed stage.
- [ ] Replace generic render-plan overlay input errors with field-specific actionable errors.
- [ ] Add full discriminated overlay schema variants for template, primitive, and plain overlays.
- [ ] Promote the manual animated-primitives scenario into a repeatable real e2e fixture.
