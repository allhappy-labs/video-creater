# Media Library View Toggle Design

## Context

Palmier's media panel exposes compact library controls next to sort/search, including visual view controls for scanning folders and generated assets. Video Creater already has thumbnail media tiles, folder navigation, and sort controls, but the editor cannot switch density when a folder contains many assets.

## Goal

Add a local grid/list view toggle to the media library so editors can move between visual browsing and dense filename scanning without changing project files.

## Behavior

- The project-library chrome shows icon buttons for `Grid view` and `List view` next to the sort control.
- Grid view remains the default and keeps the existing thumbnail tile layout.
- List view renders each media asset as a compact horizontal row with a small thumbnail, filename, media metadata, AI badge when generated, and duration when available.
- Folder navigation, folder grouping, search, sort, selection, generated provenance, and folder assignment actions continue to work in both modes.
- The view choice is UI-local state only; it is not written to the project manifest.

## Tests

- Media Bin proves grid view is selected by default and list view can be selected from the project-library chrome.
- Media Bin proves list view renders the selected media item as a compact row while preserving media selection.
- Existing media generation, folder, generated provenance, and search tests keep passing.

## Acceptance Criteria

- Users can toggle between grid and list views in the media panel.
- List view is readable in the narrow source rail and does not hide generated AI/duration information.
- No canonical project schema or action contract changes are required.
