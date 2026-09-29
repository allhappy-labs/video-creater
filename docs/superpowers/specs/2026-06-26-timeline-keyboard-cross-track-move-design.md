# Timeline Keyboard Cross-Track Move Design

## Context

Palmier-style editing treats timeline clips as native objects that can be trimmed, split, reordered, and adjusted by both humans and connected agents. Video Creater already supports cross-track dragging to compatible unlocked tracks and keyboard nudging on the same track, but selected clips cannot move vertically between lanes without pointer precision.

## Requirements

- `Shift+ArrowDown` should move the selected unlocked clip to the next lower compatible unlocked track.
- `Shift+ArrowUp` should move the selected unlocked clip to the next higher compatible unlocked track.
- The emitted patch must preserve the selected clip's current `startSeconds`.
- The shortcut must emit the existing `moveItem` patch through `onTimelinePatch`.
- The shortcut must skip incompatible tracks and locked tracks.
- If no compatible unlocked track exists in the requested direction, no patch should be emitted.
- The shortcut must not fire while focus is inside editable fields.
- Existing `Shift+ArrowLeft`/`Shift+ArrowRight` timing nudges and plain Arrow playhead stepping must keep their behavior.

## Design

- Add a helper in `TimelineEditor` that scans `rows` from the selected item's current row index in the requested vertical direction.
- Reuse `itemAllowedOnTrack` and each track's `locked` state, matching the existing drag target acceptance rule.
- Add a Shift+ArrowUp/Down branch to `handleTimelineKeyboardShortcut` before the horizontal Shift+Arrow nudge branch.
- Emit `createMovePatchFromDrag({ itemId, targetTrackId, startSeconds })` with the selected item's unchanged start time.
- Keep the feature local to the React timeline; no schema, backend, Temporal, or project-file contract changes are required.

## Verification

- Add tests proving `Shift+ArrowDown` moves a selected caption to the next compatible caption track.
- Add tests proving `Shift+ArrowUp` moves back to the prior compatible caption track.
- Add tests proving incompatible and locked tracks are skipped.
- Add tests proving no patch is emitted when no compatible track exists or focus is editable.
- Re-run the full timeline editor test file.
- Run `pnpm lint`.
- Browser-smoke the editor shortcut path and document any Tauri-bridge limitation separately from the component behavior.
