# Palmier Toolbar Selection Context Removal Design

## Context

Palmier keeps the timeline tool strip as a compact icon row: undo, redo, pointer, split, trim marks, text, and zoom. Selected clip metadata is visible on the clip body, inline clip action dock, and the inspector, not as a text-heavy pill inside the toolbar. Video Creater still renders a `Timeline selection context` group in the toolbar with label, track, kind, range, and lock state.

## Requirements

- The `Timeline tools` toolbar must not render the selected clip metadata group.
- The toolbar must remain icon-first: undo, redo, select, split, source marks, text overlay, and zoom controls.
- Selected clip metadata remains inspectable through existing surfaces:
  - selected clip body labels and range text,
  - selected clip inline action dock,
  - right rail timeline/source inspector.
- Removing the toolbar metadata group must not change selection, split, source mark, text overlay, zoom, or keyboard behaviors.

## Validation

- Component tests assert `Timeline selection context` is absent even when an item is selected.
- Existing tests for selected clip dock actions, clip labels/ranges, toolbar editing actions, and zoom continue to pass.
- Browser QA confirms the timeline toolbar reads as an icon-only edit strip with zoom on the right.
