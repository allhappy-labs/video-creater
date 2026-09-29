# Compact Media Folder Assignment Design

## Context

The left media panel is becoming a compact source browser, closer to Palmier's editor rhythm. Selected media tiles still expanded into several full-width `Move to ...` buttons, which made folder organization dominate the media browser whenever an asset was selected.

## Decision

Collapse selected media folder assignment behind one `Move to folder` control. Expanding that control reveals the available folder destinations, including nested paths and `Unfiled` when relevant.

## Requirements

- Selected media cards show a single `Move to folder` control when assignment destinations exist.
- Destination buttons are hidden until the folder assignment control is expanded.
- The expanded destination list preserves nested folder labels such as `Generated selects / Scene A`.
- Destination clicks keep the existing `onAssignMediaFolder(mediaId, folderId)` payloads.
- Grid and list media views use the same assignment behavior.

## Testing

- `MediaBin` proves selected media folder destinations are collapsed by default.
- Existing nested-folder assignment coverage opens the assignment control before choosing a destination.
- Browser QA should confirm the left media browser no longer shows a tall stack of `Move to ...` buttons on selected assets.
