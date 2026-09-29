# Codex Selected Clip Reorder Design

## Context

Palmier's docs describe a connected agent that can trim, split, reorder, and adjust clips while
seeing the full project context. Video Creater now lets Codex show the selected timeline clip,
mention its source, split it, and trim its range. Manual clip reordering already exists in Source
Inspector through a validated `reorderItems` project action. Codex still cannot reorder the
selected clip from its context block.

## Goal

Add compact `Move earlier` and `Move later` controls to the Codex selected timeline clip block,
using the same sibling sequence rules and `reorderItems` action path as Source Inspector.

## Behavior

- Show a `Sequence` control group in the Codex selected timeline clip block when:
  - a selected timeline clip has a reorder context;
  - the track has at least two ordered items;
  - a reorder callback is available.
- Disable `Earlier` when the selected clip is already first in its ordered track.
- Disable `Later` when the selected clip is already last in its ordered track.
- Clicking `Earlier` swaps the selected clip with the previous sibling and emits:
  - `targetTrackId` from the reorder context;
  - the reordered `itemIds`;
  - the context's `startSeconds` and `gapSeconds`.
- Clicking `Later` swaps the selected clip with the next sibling with the same payload shape.
- Keep the existing `Mention source`, `Trim range`, and `Split clip` controls available.

## Non-Goals

- No multi-select reorder.
- No cross-track reorder.
- No drag-and-drop change in this slice.
- No new Rust action, project schema, or reducer behavior.
- No autonomous natural-language planner in this slice.
- No changes to Temporal, fal.ai, render, or generation workflows.

## Validation

- `AgentPanel` renders sequence controls from reorder context, disables boundary buttons, and calls
  the reorder callback with swapped item ids.
- `EditorWorkspace` passes selected source clip reorder context into Codex and emits the existing
  split-project `reorderItems` action when a Codex sequence button is clicked.
- Focused tests pass before full test, lint, browser QA, diff checks, and secret scan.
