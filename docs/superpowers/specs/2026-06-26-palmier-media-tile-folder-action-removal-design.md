# Palmier Media Tile Folder Action Removal Design

## Intent

Palmier's media browser keeps media cards visually flat: selecting a tile highlights it without adding persistent action controls underneath. Video Creater currently appends a `Move to folder` dropdown below the selected media tile, which changes card height, interrupts the grid, and adds visible chrome that Palmier does not show.

## Requirements

- Remove the persistent selected-tile `Move to folder` control from media cards.
- Preserve folder assignment by exposing a compact selected-media folder action in the media toolbar.
- Keep folder destination choices, nested folder labels, and `Unfiled` assignment behavior unchanged.
- Keep selected tile highlighting, media selection, drag behavior, folder cards, search, import, folder creation, and generation controls unchanged.

## Testing

- Update `MediaBin` tests to assert selected media cards do not render a tile-level `Move ... to folder` button.
- Keep coverage proving selected media can be moved to a top-level folder from the toolbar.
- Keep coverage proving nested folder paths are used in toolbar destination actions.
- Run focused `MediaBin` tests, lint/typecheck, and browser QA for the media browser.
