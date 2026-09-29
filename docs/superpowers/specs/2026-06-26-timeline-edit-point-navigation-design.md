# Timeline Edit Point Navigation Design

## Context

Palmier-style manual editing keeps timeline review close to clip structure: editors need to move between cuts, generated inserts, captions, and audio boundaries without scrubbing pixel by pixel. Video Creater now supports playhead stepping and start/end jumps, but it still lacks edit-point navigation across timeline item starts and ends.

## Requirements

- The timeline exposes previous and next edit-point navigation in the toolbar.
- `PageUp` jumps to the nearest previous edit point.
- `PageDown` jumps to the nearest next edit point.
- Edit points come from timeline item start and end times, plus `0` and project duration.
- Duplicate edit points are collapsed and rounded to timeline precision.
- Navigation is clamped to the timeline duration.
- Keyboard shortcuts must not fire while focus is inside editable controls.
- Existing start/end, step, zoom, split, trim, and clip movement behavior remains unchanged.

## Design

- Compute sorted edit points inside `TimelineEditor` from current timeline items.
- Derive previous and next edit points relative to `clampedPlayheadSeconds`.
- Add two compact icon buttons to the transport cluster with accessible labels:
  - `Jump timeline playhead to previous edit point`
  - `Jump timeline playhead to next edit point`
- Add unmodified `PageUp` and `PageDown` handling to the existing keyboard shortcut path after editable-focus guards.
- Keep this local to the React timeline; no schema, backend, Temporal, render, or project-file changes are needed.

## Verification

- Add tests for PageUp/PageDown edit-point navigation.
- Add a regression that editable focus ignores PageUp/PageDown.
- Add toolbar button tests for previous/next edit-point navigation and boundary disabled states.
- Run the focused transport tests, then the full timeline-editor test file and TypeScript checks.
- Browser-smoke the editor toolbar and verify previous/next edit-point buttons update the playhead label/readout without console errors.
