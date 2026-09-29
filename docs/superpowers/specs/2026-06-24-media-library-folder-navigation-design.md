# Media Library Folder Navigation Design

## Context

Palmier's media library presents folders as first-class project containers with counts, then lets editors work inside those folders before placing clips on the timeline. Video Creater already has text-file-editable `mediaFolders`, validated folder actions, and folder assignment controls, but the media-bin folder cards are static summaries. Editors can move assets into folders, yet they cannot enter a folder-scoped view to scan or search a set of assets.

## Goal

Make media folders navigable inside the existing Media Bin so project-library organization feels like an editor workflow rather than metadata only.

## Behavior

- Folder summary cards in `Project library` are buttons.
- Clicking a folder opens a scoped library view for that folder.
- The scoped view shows:
  - a `Back to library` control,
  - the selected folder path,
  - the selected folder's item count,
  - assets directly assigned to that folder,
  - child folder cards for nested folders.
- Search still applies inside the scoped view.
- `Unfiled` remains available in the all-library view and does not become a folder card in this slice.
- Existing folder assignment, create, rename, delete, generated-output, and source-selection workflows continue to work.
- If the selected folder is deleted or no longer exists, the UI automatically returns to the all-library view.

## Visual Treatment

- Keep the folder view compact and utility-focused.
- Folder cards use the existing folder icon and count treatment, but gain hover/focus states and clear button labels.
- The current-folder header uses small metadata text, not a large page title.
- Do not introduce a separate sidebar, modal, or tree browser in this slice.

## Data Flow

1. `MediaBin` keeps a local `activeFolderId` state.
2. `folderNodes` and `folderLabelsById` provide stable display paths.
3. `visibleMedia` remains the search-filtered asset list.
4. The all-library view renders top-level folder buttons plus `Unfiled`.
5. The scoped view renders direct children folders and assets whose `folderId` matches `activeFolderId`.

## Tests

- Folder summary cards open a scoped folder view and hide unrelated folder assets.
- Child folder cards open nested folders and show the full folder path.
- Search filters assets inside the active folder.
- Deleting or removing the active folder returns the Media Bin to the all-library view.

## Acceptance Criteria

- Folder cards are keyboard-accessible buttons.
- Editors can enter a folder, inspect its direct assets, navigate into a child folder, and return to the all-library view.
- No existing media selection, generated-asset, or folder-management tests regress.
- Browser QA captures the all-library folder cards and the active folder view with no overlapping text at desktop and narrow widths.
