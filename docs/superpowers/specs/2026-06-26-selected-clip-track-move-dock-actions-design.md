# Selected Clip Track Move Dock Actions Design

## Context

Palmier-style timeline editing keeps common clip adjustments close to the selected clip. Video Creater now supports keyboard cross-track moves with `Shift+ArrowUp` and `Shift+ArrowDown`, but that capability is invisible unless an editor already knows the shortcut. The selected clip action dock should expose the same compatible-track move behavior as compact icon buttons.

## Requirements

- The selected clip action dock should show `Move <clip> to previous track` and `Move <clip> to next track` buttons.
- Each button should emit the existing `moveItem` patch through `onTimelinePatch`.
- Buttons must preserve the clip's current `startSeconds`.
- Buttons must target the nearest compatible unlocked track in the requested direction.
- Buttons must be disabled when the selected clip is on a locked track or no compatible unlocked destination exists.
- Existing selected-clip actions for source, AI actions, source marks, split, and delete must remain available.

## Design

- Reuse the `selectedVerticalMoveTargetTrackId` helper so dock buttons and keyboard shortcuts share compatibility rules.
- Add compact lucide arrow buttons beside the source mark/split controls in the selected clip action dock.
- Keep labels and titles action-oriented and accessible.
- Keep this feature local to `TimelineEditor`; no schema, backend, Temporal, or project-file changes are needed.

## Verification

- Add tests that clicking the next-track dock action emits a `moveItem` patch to the nearest compatible unlocked track.
- Add tests that clicking the previous-track dock action emits a `moveItem` patch to the nearest compatible unlocked track.
- Add tests that dock move actions are disabled when no compatible target exists.
- Re-run the full timeline editor test file.
- Run `pnpm lint`.
- Browser-smoke the selected clip dock remains visible and usable.
