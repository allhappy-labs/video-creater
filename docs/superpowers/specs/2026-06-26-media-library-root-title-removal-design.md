# Media Library Root Title Removal Design

## Purpose

Reduce repeated source-panel chrome so the media library looks closer to Palmier's compact source panel. The root library already has an accessible region name, so it does not need a second visible `Project library` title above the folder and media grid.

## Behavior

- Keep the media library region labelled `Project library` for accessibility.
- In the root library view, show only the item count under the media toolbar.
- Keep active folder names visible when browsing into a folder, because the user needs local context and back navigation.
- Preserve folder counts, search result counts, folder cards, and navigation buttons.

## UI Details

The root header should read like Palmier's compact count line: `3 items`, `25 items`, or a search result summary. Active folder views keep the folder path as the primary line and the count/search summary as the secondary line.

## Testing

- Update the compact library chrome test to assert the root `Project library` title is not rendered as visible text.
- Assert the root item count remains visible.
- Keep existing folder navigation tests proving active folder names remain visible.
