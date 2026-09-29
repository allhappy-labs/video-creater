# Timeline Canvas Major Grid Design

## Intent

Make whole-second timeline structure explicit in the editable canvas so the major time scale is stable, accessible, and aligned with the ruler. This gives editors a clear visual hierarchy: major seconds, quarter-second subdivisions, edit points, hover preview, and the committed playhead.

## Requirements

- Render non-interactive vertical major grid lines through the timeline canvas at whole-second positions.
- Exclude the project start (`0`) so the red playhead at zero and canvas edge are not visually duplicated.
- Include the project duration when it lands on a whole second, giving the canvas a clear end boundary.
- Position major grid lines with the same `pixelsPerSecond` scale used by ruler buttons, clips, minor grid lines, edit guides, hover preview, and the playhead.
- Size each major grid line to the full timeline canvas height.
- Expose accessible labels in the form `Timeline major grid at <timecode>`.
- Keep major grid lines more visible than minor grid lines but quieter than edit-point guides, hover preview, and the red playhead.
- Do not block clip selection, dragging, resizing, track drops, canvas seeking, or playhead scrubbing.

## Non-Goals

- No snapping behavior changes.
- No timeline schema or project-file changes.
- No render, Temporal, or asset-generation changes.
