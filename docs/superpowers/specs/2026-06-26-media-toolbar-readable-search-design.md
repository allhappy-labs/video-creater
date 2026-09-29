# Media Toolbar Readable Search Design

## Intent

Palmier keeps the media library search field readable in the browser toolbar. Video Creater can collapse the search control to an icon-sized square when folder, move, import, and generate actions share the same wrapped toolbar row.

## Requirements

- Keep the existing media toolbar actions and order unchanged.
- Make the search control keep a readable minimum width instead of shrinking to icon-only.
- Allow the search control to wrap to its own line when the media panel is narrow or selected-media actions are present.
- Do not force the media toolbar into a desktop no-wrap row; selected-media actions must not squeeze Search.
- Keep search behavior, placeholder text, accessible label, and filtering unchanged.
- Do not add a new view mode, sort control, or search toggle.

## Testing

- Update `MediaBin` toolbar coverage to assert the search wrapper has a real minimum width and flex basis for wrapping.
- Keep existing search filtering and toolbar action tests passing.
- Run focused `MediaBin` tests, lint/typecheck, and browser QA for the media panel toolbar.
