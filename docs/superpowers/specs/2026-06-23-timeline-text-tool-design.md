# Timeline Text Tool

## Context

Palmier exposes text insertion as a primary timeline toolbar action next to split, trim, and selection tools. Video Creater already rendered a `T` toolbar button, but it was disabled and did not create a project-editable timeline item.

## Goal

Enable the timeline text tool to add a manual text overlay at the current playhead through the same validated project action path used by templates and generated media.

## Behavior

- `TimelineEditor` enables `Add text overlay` when the workspace provides a handler.
- Clicking the button passes the current playhead seconds to the workspace.
- `EditorWorkspace` creates a text-backed overlay item on the overlay track using an `addItems` project action.
- The new item includes visual guidance metadata: `visualTreatment`, `motion`, `safeZone`, and `avoid`.
- The new item becomes selected after the project action succeeds.

## Non-Goals

- No rich text editor or style picker in this slice.
- No drag-to-place text tool mode.
- No Temporal workflow; this is an immediate manual edit, not a queued generation or render job.

## Tests

- `TimelineEditor` calls the text-overlay handler with the current playhead.
- `EditorWorkspace` emits an `addItems` project action for an overlay track item and renders the inserted text overlay.
