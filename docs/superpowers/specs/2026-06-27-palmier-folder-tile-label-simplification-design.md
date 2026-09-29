# Palmier Folder Tile Label Simplification

## Goal

Make source library folder cells read more like Palmier's media browser tiles. A folder tile should show the folder preview icon, item count, and folder name without a second folder glyph or a secondary child-folder metadata line.

## Design

- Keep folder tiles inside the existing `Project library` region and grid.
- Keep the large folder preview icon and the top-right recursive item-count badge.
- Keep the folder name visible under the preview.
- Remove the redundant small folder icon from the label row.
- Remove the child-folder count line from the tile. Nested folders remain navigable after opening the parent folder, and recursive item counts remain available in the badge.
- Keep active-folder navigation buttons, search scoping, media grid behavior, and folder management actions unchanged.

## Testing

- `MediaBin` verifies folder tiles keep their preview/count/name structure without the extra label-row folder icon.
- Existing nested-folder tests continue to prove child folders are reachable and scoped search still works.
- Browser QA confirms the left source library renders compact folder cells with no duplicated folder chrome.
