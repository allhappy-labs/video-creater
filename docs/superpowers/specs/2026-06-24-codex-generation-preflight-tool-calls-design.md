# Codex Generation Preflight Tool Calls

## Problem

Palmier-style chat shows model and tool activity around generation actions, not only final queued jobs. Video Creater now records `list_models` for EDL edit requests, but Codex-originated generation actions still jump straight to `generate_media` or `generate_variation` in the transcript. That hides the model-readiness step for the exact actions that create or rerun assets.

## Goals

- Add a loaded `list_models` transcript row before every queued Codex generation or variation action.
- Preserve the existing queued `generate_media` and `generate_variation` rows.
- Keep the transcript compact and consistent with existing tool-call cards.
- Avoid backend, Temporal, and fal.ai payload changes.

## Non-goals

- No new model registry.
- No live provider call.
- No credential handling.
- No change to generated asset creation behavior.

## UX Requirements

- Queueing a selected generated clip rerun or prompt variation should show `list_models` before `generate_variation`.
- Queueing a selected generated source variation set should show `list_models` before `generate_media`.
- The preflight row should say `3 model families ready`.
- Existing user and Codex chat messages should remain unchanged.

## Verification

- Extend AgentPanel queue transcript tests for generated clip variation and selected source generation actions.
- Run focused AgentPanel tests, full tests, lint, and diff checks.
- Smoke-check the visible Codex rail in the browser.
