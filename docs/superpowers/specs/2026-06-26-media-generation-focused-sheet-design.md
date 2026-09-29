# Media Generation Focused Sheet Design

## Context

Palmier's generation flow opens a compact, editor-native creation panel with Image, Video, and Audio
modes, first/last/reference controls, prompt editing, model settings, credit metadata, and a clear
queue action. Video Creater already has those controls, but at the current desktop four-pane layout
the media rail is about 208 px wide. The composer becomes a very tall column, reference controls
overlap visually, and `Queue generation` is pushed far below the first viewport.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the existing media generation composer usable as a focused desktop sheet while preserving the
same request payloads, callbacks, and media-panel launch point.

## Behavior

- `Generate media` still opens and closes the existing `Media generation` region from the Media
  panel.
- On desktop layouts, the open composer is rendered as a viewport-bounded focused sheet with enough
  width for mode tabs, first/last/reference slots, prompt, settings, workflow route, and queue
  action to scan without whole-page scrolling.
- The sheet has its own vertical scrolling when content exceeds the available viewport height.
- On stacked and narrow layouts, the composer remains an in-flow panel so it does not cover the
  editor.
- Request construction remains unchanged for image, video, and audio generation.
- Existing generation history, destination, placement, selected-reference seeding, mock completion,
  insertion, and replacement behavior remain unchanged.

## Non-Goals

- No schema, Temporal, fal.ai, or Rust workflow changes.
- No drag/drop reference assignment.
- No modal focus trap in this slice.
- No resizable pane system.

## Verification

- Add a `MediaBin` test proving the open composer carries desktop focused-sheet sizing, viewport
  bounds, and in-panel fallback classes.
- Existing media generation request tests continue to pass.
- Browser-smoke the open composer at desktop width and confirm the sheet is wide enough, has no
  console errors, and the queue action is reachable within the sheet.
