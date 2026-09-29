# Generation Queue Readiness Badge Design

## Context

Palmier keeps generation controls close to the timeline and makes the final queue action feel
immediate. Video Creater now has a compact sticky queue bar, but when the prompt is empty the
disabled `Queue generation` button does not say why it is unavailable.

Palmier reference: https://www.palmier.io/docs

## Goal

Show a compact readiness state inside the sticky queue bar so editors can tell whether the
generation is ready to queue without expanding the footer or scanning the prompt field.

## Behavior

- The `Generation submit footer` keeps its compact sticky layout.
- The `Queue generation` button shows a small `Generation queue readiness` badge.
- When the prompt is empty, the badge reads `Prompt required` and the button remains disabled.
- When the prompt has non-whitespace text, the badge reads `Ready` and the button is enabled.
- Existing destination and estimate badges stay visible in the queue button.
- Generation request payloads, placement, destination, references, model settings, Temporal route,
  and mock provider behavior remain unchanged.

## Non-Goals

- No new validation rule beyond the existing prompt-required gate.
- No schema, Temporal, fal.ai, Rust workflow, or provider changes.
- No modal or focus behavior changes.

## Verification

- Add a `MediaBin` test proving the sticky queue bar shows `Prompt required`, then switches to
  `Ready` after the prompt is filled.
- Existing media generation payload and sticky-footer tests continue to pass.
- Browser-smoke the media generation composer and confirm the readiness badge fits inside the
  compact queue button.
