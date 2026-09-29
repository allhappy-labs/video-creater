# Palmier Preview Panel Shell Flattening

## Context

Palmier keeps the viewer, timeline, and media rails in one direct editor surface. Video Creater has already flattened the source rail, timeline shell, preview transport, and inspector shells, but the central `PreviewPanel` still renders as a shadcn `Card` with `CardHeader` and `CardContent`. That makes the main viewer read as a dashboard card instead of the primary editing canvas.

## Goal

Make the preview panel a native editor panel with direct tabs, viewport, transport, and render review content while preserving the bounded first-viewport layout.

## Requirements

- Replace the outer `Card`, `CardHeader`, and `CardContent` wrapper with semantic editor chrome.
- Expose the preview panel as `role="region"` with `aria-label="Preview panel"` so tests and assistive technology can address the whole viewer.
- Preserve the existing `Viewer tabs` tablist, source tab close behavior, and previous/next viewer navigation.
- Preserve the existing bounded `Preview viewport` sizing so the timeline remains visible on desktop.
- Do not add extra toggles, switches, or secondary view modes.

## Non-Goals

- No changes to timeline editing behavior.
- No changes to source playback, preview transport semantics, or render report content.
- No new generated media controls.

## Acceptance Tests

- `PreviewPanel` proves the top-level viewer shell is a semantic `Preview panel` region.
- `PreviewPanel` proves the shell keeps natural height and does not use card styling such as `bg-card`, `text-card-foreground`, or `shadow-sm`.
- Existing preview viewport, viewer tab, source playback, and render report tests continue to pass.
