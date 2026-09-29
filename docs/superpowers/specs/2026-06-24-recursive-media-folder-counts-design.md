# Recursive Media Folder Counts Design

## Context

Palmier presents the media library as an editor-native project organizer: folders show item counts,
can contain generated media, and remain useful while the editor, chat, preview, and timeline stay
visible. Video Creater already has project media folders, nested folder paths, folder cards, scoped
folder views, and media assignment actions. The current Media Bin still treats folder counts as
direct-only and can show an active parent folder as empty even when child folders contain matching
media.

## Goal

Make nested project folders read like real media library containers by counting descendant media and
showing child-folder contents inside the active folder view.

## Behavior

- A folder card count includes media assigned directly to that folder and media assigned to any
  descendant folders.
- The active folder header uses the same recursive count.
- Opening a folder shows direct media first, then nested child folder groups that contain visible
  media.
- If a folder has no direct media but has child folders with visible media, the content area shows
  those child groups instead of `Folder is empty`.
- Search remains scoped to the active folder subtree: matching descendants are visible, while media
  outside the active folder subtree stays hidden.

## Non-Goals

- No changes to the project manifest schema or folder storage format.
- No recursive move/delete behavior changes.
- No new folder thumbnails, drag reparenting, or bulk selection.
- No backend, Temporal, or generation workflow changes.

## Validation

- Media Bin tests cover recursive folder card counts, active-folder counts, and rendering child
  folder contents when the parent has no direct media.
- Existing folder navigation, assignment, search, and nested folder tests keep passing.
- Browser QA verifies nested folder cards and contents remain compact in the source library panel.
