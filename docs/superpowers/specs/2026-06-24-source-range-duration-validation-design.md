# Source Range Duration Validation

## Context

Video Creater stores timeline clips as text-editable project data with `durationSeconds` and optional `sourceIn` / `sourceOut` properties. The manual source inspector lets users edit those fields directly. Today it validates that `sourceOut` is after `sourceIn`, but it does not validate that the selected source span length matches the clip duration.

Palmier-style timeline editing makes clip source ranges inspectable and editable without creating ambiguous playback state. Until Video Creater has explicit speed or retiming fields, a clip with duration `3s` and source span `5s` has no clear rendering semantics.

## Goal

Prevent manual trim edits from creating mismatched source range duration data.

## Behavior

- When both `Source in` and `Source out` are empty, the existing clip-duration-only edit remains valid.
- When exactly one source boundary is filled, the existing partial-range validation remains invalid.
- When both source boundaries are filled:
  - `sourceOut` must be greater than `sourceIn`.
  - `sourceOut - sourceIn` must match `Clip duration`.
  - A small rounding tolerance is allowed for decimal input.
- If the source span duration does not match the clip duration:
  - `Apply clip trim` is disabled.
  - The inspector shows a concise warning that states the source span and clip duration.

## Non-Goals

- No retiming or speed controls.
- No project schema change.
- No renderer behavior change.
- No changes to drag-trim helpers, which already preserve matching source spans.

## Tests

- Extend `SourceClipInspector` tests with a mismatch case where `Clip duration` and `Source out - Source in` differ, the warning appears, and `Apply clip trim` does not call `onApply`.
