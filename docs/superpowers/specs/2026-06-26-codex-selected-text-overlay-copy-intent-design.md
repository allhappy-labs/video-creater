# Codex Selected Text Overlay Copy Intent

## Context

Palmier-style chat lets an editor iterate on timeline media, prompts, and timing without leaving
the project. Video Creater already has a focused manual Text Overlay Inspector and a validated
`updateTextOverlayItems` project action for text overlay copy, timing, visual treatment, motion,
safe zone, and avoid rules. The gap is in the Codex rail: selected manual text overlays are not
passed into the selected timeline clip context, so explicit prompts such as
`Set the selected overlay text to "Launch now"` fall back to a broad edit request.

## Goal

Expose selected manual text overlays to Codex and let explicit selected-overlay copy prompts update
the overlay through `updateTextOverlayItems`.

## Behavior

- `EditorWorkspace` passes selected manual text overlays into `AgentPanel` as selected timeline
  clip context, including the current text.
- The route is only available for selected timeline items with `kind: "overlay"` and text-backed
  source content.
- Prompt recognition requires `text` or `copy`, plus `selected overlay`, `current overlay`,
  `this overlay`, or `overlay`.
- The new copy is parsed from quoted text after `to`, or from the remaining phrase after `to`.
- Blank parsed text is ignored.
- Locked selected tracks block the action.
- Pressing the primary composer action calls a selected-text-overlay callback and records a
  `project_action` tool row.
- `EditorWorkspace` handles the callback by writing a single `updateTextOverlayItems` action that
  preserves the overlay's existing timing and visual metadata while replacing `text`.

## Non-Goals

- No freeform overlay selection by name.
- No caption text editing in this slice.
- No visual-treatment, motion, safe-zone, or avoid prompt parsing yet.
- No new Rust project action shape.

## Tests

- `AgentPanel` proves explicit selected overlay copy prompts route through the primary composer,
  hide `Generate edit`, and record the project-action transcript.
- `EditorWorkspace` proves the prompt route writes `updateTextOverlayItems` for the selected
  manual text overlay with preserved timing and visual metadata.
