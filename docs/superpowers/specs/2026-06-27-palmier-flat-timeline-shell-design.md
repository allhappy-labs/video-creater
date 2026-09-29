# Palmier Flat Timeline Shell Design

## Context

Palmier presents the timeline as a flat editor band attached to the preview area. Video Creater still wrapped `TimelineEditor` in a generic shadcn card with card padding and rounded outer chrome, which made the manual editing surface feel like a nested panel instead of the main editing bed.

## Decision

Render `TimelineEditor` as a named flat region instead of a card. Keep the existing toolbar, ruler, track rows, zoom controls, drag/drop, resize handles, and clip interactions unchanged.

## Requirements

- The timeline root is an accessible `Timeline editor` region.
- The root uses flat editor chrome with `border-y`, not card padding or rounded card chrome.
- The `Timeline tools` toolbar remains inside the timeline region.
- Existing timeline editing behavior is unchanged.
- The preview/timeline workspace keeps the timeline directly below the preview separator.

## Testing

- Add a focused `TimelineEditor` test for the flat editor region.
- Run the full `TimelineEditor` test file.
- Run the focused `EditorWorkspace` layout test that covers the preview and timeline editor surface.
- Run lint, full tests, and desktop/narrow visual QA.
