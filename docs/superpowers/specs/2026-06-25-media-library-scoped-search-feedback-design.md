# Media Library Scoped Search Feedback

## Context

Palmier keeps media generation and organization in the media panel: users can generate media, organize generations into project-library folders, and search/browse without leaving the editor. Video Creater already has nested project-library folders, folder cards, folder-scoped browsing, and project-wide search. The remaining friction is that the library chrome shows only the folder's total item count while a search is active.

When an editor is inside a folder and searches, the empty state can say no media matches the folder, but the header still looks like the folder contains its full total. That is ambiguous for manual editing and for agent-led reviews of a project library.

## Decision

Show scoped search feedback in the media library chrome whenever `Search project media` has a non-empty query.

The chrome will:

- Keep the existing current library or folder title.
- Keep the existing total item count when no search is active.
- Show `Showing <visible> of <total> for "<query>"` when search is active.
- Scope `visible` and `total` to the active folder subtree when browsing inside a folder.
- Use the full library as the scope when no folder is active.

This makes the current browse/search state explicit without changing folder navigation, media selection, generated-asset filtering, or the saved project model.

## Scope

In scope:

- React media-bin chrome.
- Tests for full-library and active-folder scoped search feedback.

Out of scope:

- New search indexing.
- Changing media or folder persistence.
- Generated asset search result counts.
- Layout-wide redesign.

## Verification

- Focused MediaBin tests fail before implementation.
- Focused MediaBin tests pass after implementation.
- TypeScript lint passes for the UI change.
- Secret scan confirms no generation credentials were written to docs or source.
