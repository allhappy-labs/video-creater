# Palmier Media Folder Tiles Design

## Intent

Palmier presents folders as media-browser tiles: a stable thumbnail rectangle, a prominent folder glyph, a count badge, and the folder name below. Video Creater currently renders folders as compact pill-like rows inside the project library chrome, which makes folders feel like filters instead of first-class media-grid objects.

## Requirements

- Render project library folders as tile-like controls with an aspect-ratio preview area.
- Keep folder count badges, nested folder labels, descendant media counts, active-folder navigation, and search counts unchanged.
- Keep folder click behavior unchanged: selecting a folder opens its scoped library view.
- Keep media asset tiles unchanged in this slice.
- Avoid adding new sorting, view-mode, or toggle controls.

## Testing

- Update `MediaBin` tests to assert folder cards use a tile preview area and keep their count badges.
- Keep existing folder navigation, nested folder, active folder, and cyclic folder coverage passing.
- Run focused `MediaBin` tests, lint/typecheck, and browser QA for the media browser.
