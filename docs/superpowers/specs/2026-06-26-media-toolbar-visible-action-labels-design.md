# Media Toolbar Visible Action Labels Design

## Purpose

Make the media library toolbar read more like Palmier's source panel: acquisition and generation actions should be visible commands, not only icon buttons. This improves manual editing discoverability while keeping the library compact.

## Behavior

- Keep `Import media`, `New folder`, and `Generate media` in the media library toolbar.
- Show visible labels for the primary commands: `Import`, `New Folder`, and `Generate`.
- Preserve the existing callbacks, disabled states, titles, and accessible names.
- Keep search in the same toolbar and allow it to shrink after the action buttons.

## UI Details

Buttons remain 32px tall and compact, with lucide icons followed by short text. The toolbar may wrap when the source panel is narrow so labels do not clip or overlap. Search keeps the remaining width and stays visually grouped with the media actions.

## Testing

- Update the MediaBin toolbar test to assert the command labels are visible.
- Assert the buttons are no longer fixed square icon-only controls.
- Keep existing interaction tests unchanged so import, folder creation, generation, and search behavior remain covered.
