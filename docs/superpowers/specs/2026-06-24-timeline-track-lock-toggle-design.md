# Timeline Track Lock Toggle Design

## Context

Palmier's editor treats timeline clips as editable objects that can be adjusted, regenerated, and placed without leaving the timeline. Video Creater already renders compact track controls and already respects `TimelineTrack.locked` for drag/drop, delete, move, resize, and project-action validation. The lock control is currently read-only, which makes the manual editor feel less complete and forces lock state changes through file edits only.

## Goal

Make timeline track lock state a first-class validated edit:

- The timeline track header lock button toggles the selected track between locked and unlocked.
- The toggle is persisted through the same `ProjectAction` path used by other manual timeline edits.
- Existing lock protections continue to block item edits on locked tracks.

## Behavior

- `TimelineEditor` accepts `onToggleTrackLock(trackId, locked)`.
- Link and visibility/audibility controls remain read-only until those states exist in the project model.
- The lock button is enabled only when `onToggleTrackLock` is supplied.
- Clicking an unlocked track lock button requests `locked: true`.
- Clicking a locked track lock button requests `locked: false`.
- `EditorWorkspace` maps the callback to a `setTrackLocked` project action.
- Rust applies `setTrackLocked` only to existing timeline tracks and rejects unknown track ids with `TrackNotFound`.

## Verification

- Component test: clicking lock controls emits the target track id and next locked state.
- Workspace test: a split project dispatches `setTrackLocked` and reflects the returned locked state.
- Rust reducer test: `SetTrackLocked` updates an existing track and rejects a missing track.
