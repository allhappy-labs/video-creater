# Timeline Keyboard Clip Nudge Design

## Context

Palmier-style editing keeps manual timeline adjustment close to agent-driven edits: after selecting a clip, the editor should be able to make small timing changes without dragging handles or leaving the timeline. Video Creater already supports drag moves, ruler/canvas seeking, and ArrowLeft/ArrowRight playhead stepping, but a selected clip cannot be nudged from the keyboard.

## Requirements

- `Shift+ArrowRight` should move the selected unlocked clip later by `timelineSnapSeconds`.
- `Shift+ArrowLeft` should move the selected unlocked clip earlier by `timelineSnapSeconds`.
- Nudging must clamp the clip start at `0` and at `timeline.durationSeconds - item.durationSeconds`.
- Nudging must emit the existing `moveItem` timeline patch through `onTimelinePatch`; it must not mutate project data locally.
- Nudging must not fire while focus is inside editable fields.
- Locked tracks and missing selected clips must leave the shortcut inactive.
- Plain ArrowLeft/ArrowRight must keep moving the playhead.

## Design

- Add Shift+Arrow handling to `TimelineEditor` before the plain Arrow playhead step branch.
- Reuse `timelineSnapSeconds` so keyboard nudging matches snapping, drag, and playhead stepping.
- Use the selected item track as the `targetTrackId`; this slice adjusts timing only, not cross-track movement.
- Use `createMovePatchFromDrag` for the emitted patch to preserve the existing move contract.
- Keep the behavior local to `TimelineEditor`; no schema, backend, Temporal, or project-file changes are needed.

## Verification

- Add tests for selected clip Shift+ArrowRight and Shift+ArrowLeft patches.
- Add tests for clamping at the beginning and end of the timeline.
- Add tests that locked tracks and editable focus do not emit patches.
- Re-run the full timeline editor test file.
- Run `pnpm lint`.
- Browser-smoke the editor with a selected clip and verify Shift+ArrowRight leaves the app usable.
