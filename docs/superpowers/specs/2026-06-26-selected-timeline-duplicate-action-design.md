# Selected Timeline Duplicate Action Design

## Context

Palmier-style editing keeps common clip operations close to the selected timeline item. Video
Creater can select, split, trim, delete, move, and inspect clips, but duplicating a selected clip
still requires reconstructing the item through lower-level project actions or agent output.

Palmier reference: https://www.palmier.io/docs

## Goal

Let editors duplicate the selected timeline item from the timeline toolbar and selected clip action
dock while preserving the existing text-file-editable project action path.

## Behavior

- The timeline toolbar exposes a `Duplicate selected timeline item` icon button.
- The selected clip action dock exposes `Duplicate <label>`.
- Duplicate is enabled only when a selected item exists, the source track is unlocked, and the
  workspace provides a duplicate handler.
- The duplicate is inserted on the same track immediately after the selected item.
- The duplicate preserves `kind`, `source`, `durationSeconds`, and `properties`.
- The duplicate receives a deterministic unique id derived from the original item id and a label
  suffix of `copy`.
- The duplicate is applied through the existing `addItems` project action, so split-project writes,
  browser fallback, validation, undo, and redo use the same path as other manual edits.
- Existing split, delete, trim, drag, source, generation, and track-lock behavior remains unchanged.

## Non-Goals

- No clipboard, pasteboard, multi-select duplicate, ripple edit, or cross-track duplicate.
- No new Rust project action type.
- No schema, render, Temporal, or fal.ai provider change.

## Verification

- `TimelineEditor` tests prove the toolbar and selected clip dock duplicate actions call the new
  handler only for unlocked selected items.
- `EditorWorkspace` test proves duplicate dispatches an `addItems` action with the expected copied
  timeline item.
- Existing timeline editor and editor workspace tests for split, delete, trim, and action docks
  continue passing.
