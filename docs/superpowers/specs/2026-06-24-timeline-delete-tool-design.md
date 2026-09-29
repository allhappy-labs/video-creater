# Timeline Delete Tool Design

## Context

Palmier's editor keeps timeline actions immediately available in the main toolbar: select, split, trim, text, zoom, and other direct editing controls sit above the tracks. Video Creater already has a compact timeline toolbar and Rust-validated `removeItems` project actions, but deleting the selected timeline item is not exposed as a direct manual editing command.

## Goal

Add a toolbar action that removes the currently selected timeline item through the existing validated project action path.

## Behavior

- The timeline toolbar shows a delete/remove button beside the split and source tools.
- The button is enabled only when a selected item exists, its track is not locked, and the workspace provides a remove handler.
- Clicking the button calls the workspace with the selected item id.
- `EditorWorkspace` converts the selected item id into a `removeItems` project action.
- Successful removal clears the selected item if the removed item was selected.
- Split-project projects use the same Rust-backed `apply_project_action_to_split_project_folder` path as other timeline mutations.

## Non-Goals

- No multi-select delete in this slice.
- No ripple delete or gap closing.
- No confirmation dialog.
- No keyboard shortcut yet.

## Testing

- `TimelineEditor` enables the delete button for an unlocked selected item and calls the remove callback with that id.
- `TimelineEditor` disables the delete button for selected items on locked tracks.
- `EditorWorkspace` sends a `removeItems` action when deleting the selected timeline item from a split project.
- After removal, the selected item inspector is no longer shown.

## Future Work

- Add multi-select and keyboard deletion.
- Add ripple delete for source-track workflows.
- Add undo labels that name the removed clip.
