# Timeline Keyboard Playhead Step Design

## Context

Palmier-style timeline editing depends on fast, precise playhead movement: after clicking or opening a clip, the editor should be able to nudge the playhead and immediately split, trim, add overlays, or generate at that point. Video Creater already supports ruler seeking and canvas click seeking, but keyboard playhead stepping is limited to source-preview playback and does not work on the timeline itself.

## Requirements

- `ArrowRight` should move the timeline playhead forward by the timeline snapping interval.
- `ArrowLeft` should move the timeline playhead backward by the timeline snapping interval.
- Stepping must clamp at `0` and the project duration.
- Keyboard stepping must not fire while focus is inside editable fields.
- Existing `S`, `I`, `O`, `Delete`, and `Backspace` shortcuts must keep their current behavior.
- The visible playhead label/readout should update from the same internal playhead state used by split, trim, and add-text actions.

## Design

- Reuse the existing `timelineSnapSeconds` constant for keyboard step size.
- Add ArrowLeft/ArrowRight handling to `handleTimelineKeyboardShortcut`.
- Ignore modified shortcuts with Alt, Ctrl, or Meta so browser/app conventions remain available.
- Use `clampPlayheadSeconds` when applying the next position.
- Keep this local to `TimelineEditor`; no project-file or backend changes are needed.

## Verification

- Add tests for ArrowRight and ArrowLeft playhead stepping.
- Add tests for clamping at the start/end of the timeline.
- Add a regression test that arrow keys inside an input do not move the playhead.
- Run the full timeline-editor test file.
- Run `pnpm lint`.
- Browser-smoke the editor timeline and verify ArrowRight moves the playhead label.
