# Left Edge Timeline Trim Design

## Context

Palmier presents timeline clips as directly editable objects that can be trimmed, split, reordered, and adjusted in place. Video Creater already renders left and right edge handles for selected timeline items, but only the right edge changes duration. The left edge is still visual only, which makes the manual editor feel incomplete and leaves a core clip-trimming workflow in toolbar fields.

## Goal

Make the selected item's left edge handle perform a real trim operation through the existing `trimItem` and `trimItems` project-action path.

## Behavior

- Selected items on unlocked tracks keep showing both edge handles.
- Dragging the left edge changes the item start time and duration while preserving the original end time.
- The trim clamps to timeline zero and keeps at least 0.1 seconds of duration.
- When an item has numeric `sourceIn` and `sourceOut`, dragging the left edge also shifts `sourceIn` by the same timeline delta and keeps `sourceOut` fixed.
- Source-backed trims cannot extend left past available source media; `sourceIn` is clamped at zero.
- Items without source range metadata still trim their timeline start and duration.
- Locked tracks continue to hide handles and emit no patch.

## Implementation Notes

- Add a timeline helper that returns a single `trimItem` patch from a left-edge drag.
- Use that helper from `TimelineEditor.finishInteraction` for a new `resizeLeft` interaction type.
- Keep right-edge resizing unchanged.
- Keep source mark toolbar buttons unchanged; they remain precise playhead-based source trim controls.

## Verification

- Unit tests prove left-edge drag trim patches preserve end time, shift source-in, and clamp at available source start.
- Timeline editor tests prove dragging a selected video clip left handle emits a `trimItem` patch.
- Workspace tests prove the same interaction submits a split-project `trimItems` action.
- Existing locked-track tests continue proving handles are not available on locked tracks.
