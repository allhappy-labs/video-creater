# Palmier Timeline Context Actions Design

## Intent

Palmier keeps the timeline toolbar focused on edit tools: undo, redo, pointer, split, source-range
marks, text, and zoom. Video Creater still mixes selected-clip actions into that global toolbar by
showing open-source, delete, and duplicate buttons next to the pointer and split tools. Move the
visible affordance for those contextual actions out of the global toolbar and rely on the existing
selected clip dock and keyboard shortcuts.

## Requirements

- The global `Timeline tools` toolbar must not render `Open selected source`,
  `Delete selected timeline item`, or `Duplicate selected timeline item`.
- The selected clip action dock remains the visible home for opening source, deleting, and
  duplicating the selected item.
- Delete and duplicate keyboard shortcuts continue to call the same callbacks.
- Split, source in/out marks, text overlay, add-track, playhead navigation, and zoom controls remain
  in the toolbar.
- Locked-track gating for contextual delete and duplicate remains enforced in the selected clip
  action dock and shortcuts.

## Verification

- Update `EditorWorkspace` layout coverage to assert those contextual buttons are absent from the
  center timeline toolbar.
- Update `TimelineEditor` tests so delete and duplicate are verified through the selected clip dock
  and keyboard shortcuts, not toolbar buttons.
- Browser-smoke the editor and confirm the first visible timeline buttons read like the Palmier
  reference: undo, redo, pointer, split, source marks, text, then timeline navigation and zoom.
