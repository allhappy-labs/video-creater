# Timeline Ruler Minor Ticks Design

## Intent

Make the timeline ruler read like a dense manual editor ruler instead of a sparse project-duration label strip. Palmier-style editing depends on judging clip starts, generated segment boundaries, captions, hover previews, and playhead positions against a visible time scale.

## Requirements

- Render non-interactive minor ruler ticks at the timeline snap interval (`0.25s`) between whole-second major ticks.
- Position minor ticks with the same `pixelsPerSecond` scale used by clips, ruler major ticks, edit guides, hover preview, and the playhead.
- Exclude whole-second positions from minor ticks so major seek buttons remain the only whole-second ruler controls.
- Expose accessible labels in the form `Timeline minor tick at <timecode>`.
- Keep minor ticks visually quieter than whole-second ruler ticks and edit-point markers.
- Do not change seeking, snapping, playhead movement, drag, resize, or timeline schema behavior.

## Non-Goals

- No frame-level ruler subdivisions in this slice.
- No new timeline snapping behavior.
- No schema, project-file, render, Temporal, or asset-generation changes.
