# Palmier Single Grid Media Library Design

## Goal

Keep the source library closer to Palmier's direct asset-bin model by removing the persistent Grid/List mode switch. The project media area should stay grid-first, compact, and selectable without a secondary view mode.

## Design

- Remove the `Project media view` control and its `Grid view` / `List view` buttons.
- Remove list-view state and the list-row renderer from `MediaBin`.
- Keep folder cards, scoped folder navigation, search, import, folder creation, generation, media selection, drag start, duration badges, AI badges, and generated status strips unchanged.
- Keep the accessible media collection name as `Project media grid`.

## Testing

- Update media-bin tests to assert that the media-view toggle is absent.
- Keep generated media selectable through the single project media grid.
- Run focused media-bin tests, the full media-bin suite, workspace tests, lint, full tests, and Playwright desktop/narrow visual QA.
