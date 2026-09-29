# Viewer Source Tabs Design

## Context

Palmier's viewer header shows open timeline and source tabs directly above the preview. The screenshots show `Timeline` plus media/source tabs with close affordances and an active underline, while the right rail shows source details. Video Creater already supports selecting media, double-clicking timeline clips to reveal backing media, and inspecting source clips in the right rail. The missing UI cue is the viewer-level tab strip that tells the editor which timeline or source is open in the preview area.

## Goals

- Add a compact viewer tab strip above the preview viewport.
- Always show a `Timeline` tab.
- Show a selected source tab when `selectedMediaId` resolves to project media.
- Keep the source tab label short and filename-based.
- Show source metadata near the tab strip: kind, duration, resolution, frame rate where available.
- Let the user switch back to `Timeline` without clearing selected media or timeline selection.
- Let the user return to the source tab from the viewer strip.

## Non-Goals

- No multi-source tab persistence in this slice.
- No actual media decoding or frame preview changes.
- No canonical project schema changes.
- No new Temporal job path.

## Behavior

`EditorWorkspace` owns a local `viewerMode` state:

- `timeline`: preview represents the timeline.
- `source`: preview represents the selected media source.

Opening source media from the timeline, source inspector, or media bin switches `viewerMode` to `source`. Clicking the `Timeline` viewer tab switches it back to `timeline`. Clicking the source tab switches back to `source`.

`PreviewPanel` receives a nullable `selectedSource` object and renders:

- a tablist labeled `Viewer tabs`;
- a `Timeline` tab with `aria-selected`;
- a source tab labeled by filename when selected media exists;
- source summary rows when source mode is active.

## Acceptance Criteria

- The default workspace shows `Viewer tabs` with `Timeline` selected and a selected source tab for the default media.
- Double-clicking a generated timeline clip switches the viewer to the generated output source tab.
- Clicking the `Timeline` viewer tab switches the active tab back to timeline.
- Source metadata is visible when the source tab is active.
- Existing preview, media selection, source inspector, and render report behavior keeps working.
