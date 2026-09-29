# Compact Timeline Format Summary Design

## Context

When a source inspector is open, Video Creater shows a compact `Timeline context` card above the
source details. Palmier keeps project format metadata scannable in the right rail, but this compact
card still collapses format into `1920x1080 @ 24 fps` and omits aspect ratio. Editors and agents
lose the quick `16:9` check while reviewing a selected source.

## Goal

Expose the same core project format metadata in the compact timeline context that appears in the
full project timeline inspector.

## Behavior

- The compact `Timeline context` card keeps its duration, contents, workflow, and viewer metrics.
- Its format metric includes resolution, frame rate, and reduced aspect ratio.
- The value stays compact enough for the two-column metric grid.
- Invalid or missing width/height still falls back to `unknown` for aspect ratio through the shared
  helper.

## Non-Goals

- No project schema change.
- No right-rail tab behavior changes.
- No render settings editing UI.

## Verification

- Add a `ProjectTimelineContext` test for the compact format metric.
- Run the focused project timeline inspector tests.
- Run `pnpm lint`.
- Browser-smoke the source-inspector state and confirm the compact card shows the updated format
  without overlap.
