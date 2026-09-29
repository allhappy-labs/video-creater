# Timeline Keyboard Split Shortcut Design

## Context

Palmier presents splitting as a first-class timeline action in the compact editor
tool row. Video Creater already has a split toolbar button and inline selected
clip action, but keyboard editing only supports delete/backspace. Editors should
be able to split a selected clip at the current playhead without moving focus to
the toolbar.

## Goals

- Add an `S` keyboard shortcut for splitting the selected timeline clip.
- Reuse the existing split eligibility rules: selected item, unlocked track,
  valid duration, and a split point strictly inside the clip.
- Keep text entry safe. The shortcut must do nothing when focus is in an input,
  select, textarea, or content-editable element.
- Surface the shortcut on the existing toolbar button through its title.

## Non-Goals

- Adding multi-clip split, razor mode, or range-based cutting.
- Persisting keyboard shortcut preferences.
- Adding a shortcut overlay or command palette.
- Changing the existing delete/backspace shortcut behavior.

## Interaction

- Pressing `S` or `s` calls `onSplitItem(selectedItem.id, selectedSplitSeconds)`
  when the selected clip can be split.
- The split point remains the existing selected split point: current playhead if
  the playhead is inside the selected clip, otherwise the selected clip midpoint.
- The shortcut ignores modified key combinations so browser/app commands keep
  their normal behavior.
- The toolbar split button title becomes `Split selected clip (S)`.

## Verification

- Unit test: pressing `S` splits the selected clip at the current playhead.
- Unit test: pressing `S` while focus is inside an editable control does not
  split.
- Existing split button and delete/backspace tests continue to pass.
- Browser QA: the split toolbar button exposes the shortcut title and the app
  layout remains unchanged.
