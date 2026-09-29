# Timeline Home End Shortcuts Design

## Context

Palmier-style manual editing depends on direct timeline navigation as much as frame-step precision. Video Creater already supports ruler seeking, canvas seeking, playhead dragging, ArrowLeft/ArrowRight stepping, and clip-editing shortcuts, but there is no keyboard-only way to jump to the start or end of the active timeline.

## Requirements

- `Home` should move the timeline playhead to `0`.
- `End` should move the timeline playhead to the project duration.
- Home/End navigation must not fire while focus is inside editable controls.
- Modified Home/End key combinations should remain available to the browser or host app.
- The visible playhead label/readout should update from the same internal playhead state used by split, trim, and timeline insertion actions.

## Design

- Add Home/End handling to the existing `handleTimelineKeyboardShortcut` path in `TimelineEditor`.
- Apply only unmodified `Home` and `End` events after editable-focus guards.
- Set the playhead to `0` for Home and to the clamped timeline duration for End.
- Keep the behavior local to the editor surface; no timeline schema, project-file, or backend changes are needed.

## Verification

- Add a test that End moves the playhead to the timeline duration and Home returns it to `0`.
- Add a regression test that focused editable controls do not respond to Home/End timeline navigation.
- Run the focused Home/End test first to confirm the missing behavior.
- Run the full timeline-editor test file and TypeScript checks after implementation.
- Browser-smoke the editor timeline and verify End/Home update the playhead label and readout.
