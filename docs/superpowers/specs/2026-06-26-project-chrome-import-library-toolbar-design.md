# Project Chrome Import Library Toolbar Design

## Purpose

Align the workspace chrome with the Palmier-style editor layout: global chrome should stay focused on project navigation, Help, and export, while source acquisition lives in the media library toolbar.

## Behavior

- Remove the global `Import` button from `Workspace actions`.
- Keep `Export` as the only persistent project action in the top-right chrome.
- Preserve the existing media library `Import media` toolbar button and its `onImport` behavior.
- Keep `New folder`, `Generate media`, and `Search project media` grouped with `Import media` in the media library toolbar.

## UI Details

The top chrome should read as a project-level strip: Home, Codex chat, Help, centered project title, Export, and profile. The media library remains the place for acquiring or creating source assets, matching the screenshot pattern where Import, New Folder, Generate, and Search sit directly above media tiles.

## Testing

- Update the workspace chrome test to assert `Import` is not present in `Workspace actions`.
- Assert the media library toolbar still exposes `Import media`, `Generate media`, and `Search project media`.
- Update the dedicated top chrome test so it expects Export-only global actions and verifies Import remains in the media library toolbar.
