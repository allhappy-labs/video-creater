# Timeline Source Mark Shortcuts Design

## Context

Palmier presents trim and split operations as native timeline controls that can be
used quickly while inspecting clips. Video Creater already has toolbar and inline
buttons for setting a selected media clip's source in/out marks at the playhead,
but the action still requires a mouse click. After adding keyboard split and
preview playback, the next manual-editing gap is keyboard source marking.

## Goals

- Add `I` and `O` keyboard shortcuts for source in/out marks on the selected
  timeline clip.
- Reuse the existing `setSelectedSourceMark` path and
  `createTrimPatchFromSourceMark` behavior.
- Preserve the existing eligibility rules: selected media-backed item, unlocked
  track, `onTimelinePatch`, and playhead strictly inside the selected item.
- Keep text entry safe by ignoring shortcuts from input, select, textarea, and
  content-editable focus.
- Expose shortcut hints on the toolbar buttons.

## Non-Goals

- Adding separate source monitor in/out markers.
- Trimming non-media text, overlay, caption, or HyperFrames items.
- Adding preference editing or shortcut remapping.
- Changing generated clip source range math.

## Interaction

- Pressing `I` or `i` sets the selected media clip's source in point to the
  playhead.
- Pressing `O` or `o` sets the selected media clip's source out point to the
  playhead.
- Shift may be used to type uppercase `I` or `O`; Alt, Control, and Meta
  combinations are ignored.
- If the current focus is editable, the shortcut does nothing.
- Toolbar button titles become `Trim selected clip in point to the playhead (I)`
  and `Trim selected clip out point to the playhead (O)`.

## Verification

- Unit test: pressing `I` emits the same trim patch as the source-in toolbar
  action.
- Unit test: pressing `O` emits the same trim patch as the source-out toolbar
  action.
- Unit test: pressing `I` or `O` from editable focus does not emit a patch.
- Existing toolbar source-mark tests continue to pass.
- Browser QA: source mark toolbar buttons expose the shortcut titles.
