# Media Library Chrome Design

## Context

Palmier's media panel behaves like a project library, not only a raw list of imported files. The screenshots show an item count, visible folder cards, and compact sort/view controls above a dense media grid. Video Creater already supports media folders, search, generation, folder editing, and media tiles. The missing surface is a small library chrome that makes the panel easier to scan and organize before placing assets on the timeline.

## Goals

- Show a compact project-library count near the search area.
- Add a sort control so users can reorder visible media by project order, filename, or kind.
- Show folder cards with item counts above grouped media when folders exist.
- Keep existing folder editing, media selection, search filtering, generated provenance, replacement, and insertion behavior unchanged.

## Non-Goals

- No folder navigation or drilling into folders.
- No persistent user preference for sort order.
- No backend schema changes.
- No drag-and-drop folder assignment.
- No new generated asset write behavior.

## UI Behavior

The media bin shows a `Project library` summary block after search and before generation/folder sections:

- Count text: `N items` for all media assets currently in the project.
- Sort selector: `Project order`, `Name`, and `Kind`.
- Folder cards: one compact card per declared folder, showing the folder name and count of media assets assigned directly to that folder.

Sort order applies to visible media groups after search filtering:

- `Project order` preserves the incoming media array order.
- `Name` sorts by filename, case-insensitive.
- `Kind` sorts by media kind first and filename second.

Folder cards are informational in this slice. Folder management inputs remain the editable control surface for create, rename, delete, and assignment.

## Testing

- Unit test count text, sort selector, and folder cards render with folder counts.
- Unit test changing sort to `Name` reorders unfiled media tiles by filename.
- Unit test search filtering still works while the library summary remains visible.

## Acceptance Criteria

- The media bin communicates library size and folder composition before the asset grid.
- Users can sort media tiles without losing folder grouping.
- Existing media generation, selection, folder assignment, and generated-output tests keep passing.
- Focused media-bin tests, full frontend tests, lint, and browser visual QA pass.
