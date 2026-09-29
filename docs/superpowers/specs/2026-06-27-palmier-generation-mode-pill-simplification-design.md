# Palmier Generation Mode Pill Simplification Design

## Context

Palmier's generation sheet presents `Image`, `Video`, and `Audio` as compact mode pills. Model,
duration, aspect ratio, credits, history, and submit state remain visible elsewhere in the sheet,
but the mode control itself stays short and scannable. Video Creater currently repeats model and
credit metadata inside every mode button, making the focused sheet taller and busier than the
Palmier reference.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the media generation mode selector read as three compact media-type pills while preserving
the existing request payloads, settings summary, credit estimate, and queue behavior.

## Behavior

- The `Generation mode selector` still exposes `Image`, `Video`, and `Audio` buttons.
- Each mode button shows only its icon and media-type label.
- The selected mode remains visually selected with `aria-pressed="true"`.
- Model labels and credit estimates are not duplicated inside mode buttons.
- Model, estimate, output, timing, and placement context remain visible in the active recipe,
  settings summary, header estimate badge, and submit footer.
- Clicking a mode still changes only the active generation mode and does not mutate prompt,
  references, placement, or request shape.

## Non-Goals

- No pricing, billing, Temporal, fal.ai provider, or generated asset schema changes.
- No changes to model options or output settings.
- No redesign of reference slots, destination, placement, history, or queue behavior.

## Verification

- Update `MediaBin` coverage to prove the mode selector buttons do not contain model or credit
  metadata.
- Keep coverage proving generation metadata remains visible outside the mode selector.
- Run the focused `MediaBin` suite, typecheck/lint, and browser QA for the open generation sheet.
