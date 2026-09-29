# Media Library Parent Folder Navigation Design

## Context

Palmier treats project folders as a primary media-browsing surface: editors move between folders while keeping generated and imported media close to the timeline. Video Creater already supports folder cards, nested folders, scoped folder browsing, search, and folder-targeted generation. The remaining navigation gap is that an active nested folder only offers `Back to library`, which jumps to the root instead of moving one level up.

## Requirements

- When the active project library folder has a valid parent folder, show an `Up to <parent path>` action in the library chrome.
- Pressing the action should make the parent folder active.
- Root-level active folders should keep the existing `Back to library` behavior only.
- Nested folder counts, child folder cards, scoped media results, search behavior, and generation destination should continue to use the active folder.
- No project schema, media folder action, generated asset, or workflow contract changes.

## Design

- Reuse the existing `folderNodes` tree and `activeFolderNode`.
- Add a derived `activeParentFolderNode` by looking up `activeFolderNode.folder.parentId`.
- In `renderLibraryChrome`, render `Up to <parent label>` before `Back to library` only when a parent node exists.
- Keep both controls compact so they fit beside view toggles and sort controls on narrow side panels.
- The action only updates `activeFolderId`; all existing active-folder filtering and destination behavior remains unchanged.

## Acceptance

- Opening a nested folder shows `Up to Generated selects`.
- Pressing that action returns to the parent folder, not the root library.
- Root folders do not show an `Up to ...` action.
- Existing folder browsing, folder counts, scoped search, and folder-targeted generation tests continue passing.
