# Palmier Media Toolbar Move Dropdown Removal Design

## Context

The Palmier reference media browser keeps the primary header focused on library-level commands: import, folder creation, generation, search, and compact view/navigation controls. Video Creater's selected-media `Move` dropdown made the toolbar change shape based on tile selection and pushed search away from the stable command sequence.

## Decision

Remove the persistent selected-media folder assignment dropdown from the primary media toolbar. Folder organization remains part of the project model and folder-management workflow, but selected-item move controls should return later in a less dominant secondary surface rather than occupying the main browser action row.

## Requirements

- Keep `Import`, `New Folder`, `Generate`, and `Search project media` stable in the primary media toolbar.
- Do not render `Move <filename> to folder` from the primary media toolbar when media is selected.
- Do not expose nested folder destination buttons from the primary toolbar.
- Preserve media selection, folder cards, scoped folder navigation, folder creation, generation, search, and grid/list view controls.

## Testing

- `MediaBin` asserts the compact labeled action row no longer includes a selected-media move dropdown.
- `MediaBin` asserts selected-media and nested destination move controls are absent from the media toolbar.
- Browser QA should compare the media toolbar against the Palmier references at desktop and narrow widths.
