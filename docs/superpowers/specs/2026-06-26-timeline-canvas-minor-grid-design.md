# Timeline Canvas Minor Grid Design

## Intent

Carry the ruler's quarter-second scale into the editable canvas so clip edges, captions, audio beats, generated media boundaries, hover previews, and playhead movement can be judged against the same visual time grid.

## Requirements

- Render non-interactive vertical canvas grid lines at the same minor tick positions as the ruler.
- Position grid lines with the same `pixelsPerSecond` scale used by clips, ruler ticks, edit guides, hover preview, and the playhead.
- Exclude whole-second positions so major grid/tick emphasis remains clear.
- Size each grid line to the full timeline canvas height.
- Expose accessible labels in the form `Timeline minor grid at <timecode>`.
- Keep minor grid lines quieter than edit-point guides, hover preview, and the red playhead.
- Do not block clip selection, dragging, resizing, track drops, canvas seeking, or playhead scrubbing.

## Non-Goals

- No snapping behavior changes.
- No frame-level subdivisions.
- No schema, project-file, render, Temporal, or asset-generation changes.
