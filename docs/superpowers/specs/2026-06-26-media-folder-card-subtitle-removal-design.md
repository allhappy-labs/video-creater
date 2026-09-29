# Media Folder Card Subtitle Removal Design

## Purpose

Make project-library folder cards closer to Palmier's compact source panel tiles. Folder cards already show a folder icon, name, and count, so repeating `Folder` as a subtitle adds noise without adding useful editing context.

## Behavior

- Remove the generic `Folder` subtitle from folder cards that have no child folders.
- Keep the child-folder count subtitle when a folder contains nested folders, because that communicates useful hierarchy.
- Preserve folder names, item count badges, open-folder actions, and accessible labels.

## UI Details

Root and active-folder folder cards should be compact tiles: icon + name + count. Cards with nested folders may keep a muted `1 folder` / `N folders` line.

## Testing

- Update the folder grouping test to assert folder cards no longer render the generic `Folder` subtitle.
- Keep existing nested folder tests so child-folder hierarchy remains visible and navigable.
