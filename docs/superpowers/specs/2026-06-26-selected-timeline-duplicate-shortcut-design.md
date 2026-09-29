# Selected Timeline Duplicate Shortcut Design

## Context

Palmier-style manual editing keeps common timeline operations available from both visible controls
and keyboard-first editing. Video Creater now has duplicate buttons in the timeline toolbar and the
selected clip dock, but duplicating still requires a pointer action.

## Goal

Let editors duplicate the selected timeline item with the platform duplicate shortcut while keeping
the same validated project-action path used by the duplicate buttons.

## Behavior

- `Meta+D` on macOS-style keyboards duplicates the selected timeline item.
- `Ctrl+D` duplicates the selected timeline item on non-macOS keyboard conventions.
- The shortcut uses the existing `onDuplicateItem` callback.
- The shortcut is ignored when focus is inside editable controls.
- The shortcut is ignored when no item is selected, no duplicate handler is available, or the
  selected item's track is locked.
- Toolbar and selected clip dock duplicate behavior remains unchanged.

## Non-Goals

- No clipboard or pasteboard integration.
- No multi-select duplicate.
- No new project action type or Rust command.
- No timeline ripple or collision resolution change.

## Verification

- `TimelineEditor` tests prove `Meta+D` and `Ctrl+D` call the duplicate handler for the selected item.
- `TimelineEditor` tests prove the shortcut is ignored from editable input focus.
- Existing duplicate button, locked-track, delete, split, source-mark, and keyboard nudge tests keep
  passing.
