# Text Overlay Inspector Design

## Context

Palmier treats generated and manual timeline media as editable objects: a user can select a clip, inspect its source details, then revise the asset or its placement without leaving the editor. Video Creater can already add a manual text overlay from the timeline toolbar, but the selected item currently falls through to the generic source clip inspector. That makes a text overlay look like unlinked source media instead of a first-class timeline object.

## Goal

Add a focused inspector for manual text overlays and a reusable project action for editing any text-backed timeline item. This gives both manual UI workflows and agent proposals a text-file-editable contract for overlay copy.

## Behavior

- Selecting a `kind: "overlay"` timeline item with `source.type: "text"` opens a `Text Overlay Inspector` in the selected item editor region.
- The generic source clip inspector must not claim text overlays as unlinked media.
- The inspector shows the selected overlay timing and an `Overlay text` textarea.
- Applying a non-empty edit dispatches `editTextItem` with `itemId` and `text`.
- The Rust project action trims surrounding whitespace, updates `TimelineSource::Text.text`, synchronizes the visible timeline `label`, synchronizes `properties.text` when present, and marks `properties.textEdited = true`.
- Empty text edits are rejected without mutating the project.
- Non-text timeline items are rejected without mutating the project.

## Out of Scope

- Rich typography controls, animation presets, and safe-zone editing.
- Temporal workflow orchestration. This slice is a synchronous project edit, not a queued render or generation job.
- Automatic caption repair. Caption-specific correction remains on `editCaptionText`.

## Tests

- TypeScript command adapter forwards `editTextItem` to the Rust command unchanged.
- Rust action serde contract uses `type: "editTextItem"` and camelCase fields.
- Rust action updates source text, syncs the visible label and `properties.text`, and records `textEdited`.
- Rust action rejects empty and non-text targets without mutation.
- Workspace UI selects the new inspector after adding a manual text overlay and applies the edit through `editTextItem`.
