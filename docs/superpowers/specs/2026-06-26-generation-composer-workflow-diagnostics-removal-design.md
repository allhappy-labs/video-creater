# Generation Composer Workflow Diagnostics Removal Design

## Context

Palmier keeps the generation composer focused on creative inputs: media type, references, prompt,
model, output settings, credits, and one generate action. Video Creater's focused generation sheet
still exposes internal workflow route diagnostics inside the composer, including workflow type,
task queue, provider, placement, readiness, and queue target badges. That makes the creative panel
look more like a debug console than an editor-native generation surface.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the generation sheet feel closer to Palmier by removing internal workflow diagnostics from the
visible composer while preserving Temporal-backed request construction, queue gating, and workflow
metadata in job/status surfaces.

## Behavior

- The open `Media generation` composer no longer renders a `Generation workflow route` section.
- The `Queue generation` button is a compact creative submit control with the estimated credit cost
  visible, but no readiness or target diagnostic badges inside the button.
- The button remains disabled when the prompt is empty or when timeline placement has no valid
  unlocked target.
- The existing active recipe and settings summary remain visible so editors can verify model,
  estimate, output, timing, destination, and target context without seeing internal workflow names.
- Temporal workflow type, task queue, provider, and run metadata remain available on generated
  asset/job/status surfaces outside the composer.
- Generation request payloads, placement intent, folder destination, model selection, references,
  and mock worker behavior remain unchanged.

## Non-Goals

- No changes to Temporal Rust workflow registration, fal.ai providers, or generated asset schemas.
- No removal of generated asset workflow status displays.
- No redesign of the reference slot picker or destination picker in this slice.

## Verification

- `MediaBin` tests prove the open generation composer omits the workflow route and queue diagnostic
  badges.
- `MediaBin` tests prove `Queue generation` still carries the estimated credits and remains gated
  by prompt and timeline-target validity.
- Existing generation payload, placement, history, mock completion, and generated asset workflow
  status tests continue to pass.
- Browser QA opens the composer and checks that workflow diagnostics are absent from the sheet with
  no console errors.
