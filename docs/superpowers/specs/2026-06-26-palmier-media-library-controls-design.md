# Palmier Media Library Controls Design

## Intent

Palmier keeps the media library header compact: primary actions stay in one row, and the library view/sort controls are small icon controls beside the item count. Video Creater currently spends too much space on a segmented grid/list switch and a labeled Sort select. Tighten this area so the Media panel reads more like Palmier without changing media sorting, folder navigation, generated media, or selection behavior.

## Requirements

- The Project library section keeps the library/folder title and item count visible.
- The library controls are exposed as one compact `Project library controls` group.
- Grid view, List view, sort mode, and sort direction remain keyboard-accessible with their existing labels.
- The visible `Sort` label is removed from the library header.
- The sort mode control becomes an icon-sized control instead of a wide labeled select.
- Folder navigation actions such as `Up to ...` and `Back to library` remain available when inside folders.
- No media ordering, folder grouping, generated asset, drag/drop, or timeline behavior changes.

## Verification

- Add a `MediaBin` test for the compact control group and no visible Sort label.
- Keep existing grid/list and sort behavior tests passing.
- Browser-smoke the editor and confirm the media library header matches the Palmier-style density.
