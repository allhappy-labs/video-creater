# Palmier Timeline Snapping Toggle Design

## Intent

Palmier's timeline toolbar is compact and does not expose a separate snapping switch. Video Creater currently shows a magnet toggle between the pointer and split tools, adding another mode control to an already dense toolbar. Keep snapping behavior enabled, but remove the visible `Timeline snapping` toggle so the toolbar reads closer to the Palmier reference.

## Requirements

- The center timeline toolbar must no longer render a `Timeline snapping` button or switch.
- Timeline snapping remains enabled for drag, resize, playhead stepping, and source mark workflows that already snap to timeline increments.
- The pointer, split, delete, duplicate, open source, source mark, text, add-track, playhead, and zoom controls remain accessible.
- Timeline drag/move/resize behavior must not change except that users can no longer disable snapping from the toolbar.
- Browser QA must confirm the toolbar has one fewer visible toggle and still begins with undo, redo, pointer, and split controls.

## Verification

- Update `EditorWorkspace` tests to prove the center timeline toolbar has no `Timeline snapping` control.
- Keep focused timeline interaction tests passing.
- Browser-smoke the editor and confirm the magnet toggle is gone from the toolbar.
