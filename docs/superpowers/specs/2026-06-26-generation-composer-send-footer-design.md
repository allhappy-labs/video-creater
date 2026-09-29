# Generation Composer Send Footer Design

## Context

Palmier's generation panel keeps the final action compact: cost remains visible, and the submit
control reads as a send action rather than a full-width form button. Video Creater's current
generation composer has already removed workflow diagnostics, but the footer still ends with a wide
rectangular `Queue generation` button. That shape feels heavier than the Palmier reference and
competes with the prompt, reference, and model controls.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the generation sheet footer feel like an editor-native send bar: visible cost on the left,
one circular queue action on the right, and the same request behavior underneath.

## Behavior

- `Generation submit footer` renders as a compact horizontal control row.
- The estimated generation cost is visible as a `Generation submit estimate` badge.
- The submit action remains accessible as `Queue generation`, but it is an icon-only circular
  button with an upward arrow, matching Palmier's compact send affordance.
- The button remains disabled when the prompt is empty or when timeline placement has no compatible
  unlocked target.
- Existing generation request payloads, destination, placement, references, model settings, mock
  worker behavior, and Temporal workflow/job surfaces remain unchanged.
- The composer still omits workflow route, queue readiness, and queue target diagnostics.

## Non-Goals

- No changes to Temporal Rust workflow registration or fal.ai provider calls.
- No schema changes.
- No prompt-box relocation in this slice.

## Verification

- `MediaBin` tests prove the submit footer contains a cost badge and icon-only circular
  `Queue generation` button.
- Existing generation gating and payload tests continue to pass.
- Browser QA opens the generation composer and verifies the footer has no workflow diagnostics and
  the queue button text is no longer visually rendered inside the button.
