# Timeline Canvas Edit-Point Guides Design

## Intent

Make timeline edit points visible across the full canvas so manual editors and agent-generated sequences can align cuts, captions, overlays, and generated media boundaries across tracks.

## Requirements

- Render non-interactive vertical guide lines through the timeline canvas at the same internal edit points shown on the ruler.
- Exclude the project start (`0`) and the project duration from guide rendering.
- Position guides with the same `pixelsPerSecond` scale used by clips, ruler markers, and the playhead.
- Size guides to the full timeline canvas height so they span every track row.
- Expose accessible labels in the form `Timeline edit guide at <timecode>`.
- Keep guides visually secondary to the playhead and selected clips.
- Do not intercept seeking, dragging, resizing, track drops, or clip selection.

## Non-Goals

- No timeline schema changes.
- No project file format changes.
- No render pipeline, Temporal, or asset-generation changes.
- No snapping behavior changes.
