# Palmier Timeline Source Mark Contextual Controls Design

## Context

The timeline toolbar has been compacted, but it still renders disabled `Set source in point` and
`Set source out point` buttons whenever the selected item cannot accept source marks. For caption,
overlay, empty, or non-source selections, those buttons read as inactive settings chrome instead of
available editing tools. Palmier-style editor chrome keeps the visible tools tied to the current
editing context and avoids permanently disabled controls.

## Goal

Render source in/out toolbar buttons only when the selected timeline item can accept source mark
edits. Keep the existing button labels, icons, callbacks, and keyboard shortcuts for source clips.

## Requirements

- Hide `Set source in point` and `Set source out point` when `canSetSourceMark` is false.
- Keep both buttons visible and enabled when `canSetSourceMark` is true.
- Preserve the existing `I` and `O` keyboard shortcut behavior for eligible source clips.
- Do not change split, select, undo, redo, add text overlay, zoom, ruler, track state, project file,
  Temporal workflow, or generated asset behavior.
- Avoid introducing replacement text, toggles, tabs, dropdowns, or disabled placeholders.

## UI Treatment

The toolbar should remain a compact row of active editing tools. Source mark controls are contextual
clip tools, so they appear beside split only when they can act on the selected source clip. When they
are absent, the toolbar should not leave an orphan divider between split and add-text controls.

## Tests

Update `timeline-editor.test.tsx` to prove:

- caption/non-source selections do not render the source in/out buttons
- a selected media source clip still renders both buttons enabled
- the existing click and keyboard shortcut trim tests continue to pass

Run focused timeline editor tests first, then the full suite before committing.
