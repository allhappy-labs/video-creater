# Timeline Hover Time Preview Design

## Intent

Make timeline inspection feel closer to a dense manual editor by showing the exact time under the pointer before the editor commits a seek or scrub. Palmier-style timeline work depends on quick visual alignment between the ruler, playhead, clip boundaries, and generated media segments.

## Requirements

- Show a non-interactive vertical hover guide when the pointer moves over the timeline canvas.
- Align a compact timecode badge to the hover guide in the timeline ruler band using the existing `formatTimecode` display.
- Position the guide with the same `pixelsPerSecond` scale used by clips, ruler ticks, edit guides, and the playhead.
- Keep the hover guide visually secondary to the committed red playhead.
- Do not change the committed playhead time until the editor clicks or scrubs.
- Hide the hover guide when the pointer leaves the timeline canvas.
- Do not block clip selection, dragging, resizing, track drops, canvas seeking, or playhead scrubbing.

## Non-Goals

- No schema, project-file, render, Temporal, or asset-generation changes.
- No hover preview thumbnails in this slice.
- No snapping behavior changes.
