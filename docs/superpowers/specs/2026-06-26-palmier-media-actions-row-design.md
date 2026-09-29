# Palmier Media Actions Row Design

## Intent

Palmier places `Import`, `New Folder`, `Generate`, and `Search` in a compact row at the top of the media browser. Video Creater still renders a duplicate Media title, three tall action tiles, and a separate full-width search field. Make this area denser and more editor-like while preserving the same import, folder, generation, and search behavior.

## Requirements

- The media panel action toolbar contains Import, New Folder, Generate, and Search project media in one row at the current editor width.
- Import, New Folder, and Generate use 32px icon controls with accessible labels and tooltips.
- Search remains accessible as `Search project media` and keeps filtering media, folders, and generated assets.
- The duplicate in-panel `Media` title is removed because the enclosing left sidebar tab already identifies the panel.
- Existing import disabled/loading state, folder manager toggle, and generation composer toggle remain unchanged.
- No media data, generation request, folder action, or timeline behavior changes.

## Verification

- Update `MediaBin` toolbar tests to require the one-row toolbar and compact button sizing.
- Keep existing search, folder, and generation tests passing.
- Browser-smoke the editor and confirm the media browser top controls stay on one compact row.
