# Timeline Ruler Edit Point Markers Design

## Context

Palmier-style editing makes cut structure visible while reviewing the timeline. Video Creater now supports edit-point navigation with PageUp/PageDown and toolbar buttons, but those navigation targets are not visible on the ruler. Editors can jump between clip boundaries without seeing where those boundaries are.

## Requirements

- The timeline ruler renders compact edit-point markers for internal clip boundaries.
- Edit-point markers use the same edit-point set as previous/next navigation.
- Markers exclude `0` and project duration to avoid duplicating the timeline ends.
- Integer edit points still render even if they overlap a normal second tick.
- Markers are non-interactive so existing ruler seek buttons keep their click behavior.
- Each marker has an accessible label with its exact timecode.
- Existing ruler seeking, edit-point navigation, zoom, playhead dragging, and clip interactions remain unchanged.

## Design

- Derive `rulerEditPointSeconds` from the existing `editPoints` array inside `TimelineEditor`.
- Render each marker as an absolutely positioned, pointer-events-none ruler tick overlay.
- Use `formatTimecode` for labels such as `Timeline edit point at 00:00:00.650`.
- Position markers with the same `pixelsPerSecond` scale used by ruler seek ticks.
- Keep this local to `TimelineEditor`; no project-file, backend, Temporal, render, or schema changes are needed.

## Verification

- Add a `TimelineEditor` test that internal edit points render on the ruler at the expected positions.
- Confirm start and end do not render duplicate edit-point markers.
- Run the focused ruler test, then the full timeline-editor test file and TypeScript checks.
- Browser-smoke the editor ruler and verify marker labels exist without console errors.
