# Live Timeline Drag and Resize Preview Design

## Context

The timeline already records pointer deltas, snap targets, compatible destination rows, and ripple-trim preview plans while an interaction is active. Normal clip moves and non-ripple resizes do not currently apply that transient state to the rendered clip geometry, so the visible clip stays at its original position and size until pointer release commits the timeline action.

## Goal

Make timeline clips follow drag and resize gestures immediately while preserving pointer release as the single canonical project mutation and undo boundary.

## Interaction Contract

- A normal horizontal drag moves the rendered clip continuously to its snapped preview start time.
- A compatible vertical drag moves the rendered clip to the preview destination row.
- An invalid vertical destination keeps the clip on its original row and preserves the existing rejected-target feedback.
- Every item in a selected drag group previews the same time delta. Compatible visual lanes translate together while linked companions that cannot translate remain on their original compatible lanes, matching the final move plan.
- Option-drag keeps the original items in place and continues to render duplicate previews at their proposed positions.
- A right-edge resize updates the rendered width continuously, including snapping and the existing minimum-duration clamp.
- A left-edge resize updates both rendered left position and width continuously, including snapping, source-preserving trim constraints, and the existing minimum-duration clamp.
- Shift-ripple trims continue to use the existing ripple preview plan for the resized item and all shifted items.
- Pointer cancellation removes transient geometry and restores the canonical timeline without emitting a timeline action.
- Pointer release emits exactly one canonical move, group move, duplicate, trim, resize, or ripple-trim action using the same resolved geometry shown immediately before release.

## Architecture

`TimelineEditor` remains the owner of transient pointer interaction state. It derives preview geometry during render instead of mutating the `timeline` prop.

For every positioned item, the editor resolves a preview record containing:

- `startSeconds`
- `durationSeconds`
- `trackId` or resolved row position
- whether the item is actively moving or resizing

The resolver combines the canonical item geometry with the active interaction:

1. Normal moves use the existing snapped move delta and compatible target-row resolution.
2. Non-ripple resizes use the same snapped edge calculation and clamps used by the final patch.
3. Ripple trims use the existing ripple preview plan.
4. Items unaffected by the interaction retain canonical geometry.

The rendered item wrapper consumes this preview record for `left`, `top`, and `width`. Positional CSS transitions are disabled for actively moved or resized items so pointer feedback is direct rather than eased or delayed. Existing opacity, outline, snap-guide, accepted-target, and rejected-target treatments remain intact.

Shared geometry helpers will be used by both live preview and pointer-up commit calculation. This prevents the last preview frame from disagreeing with the committed result because of separate snapping or clamping logic.

## State and Persistence

Transient geometry lives only inside `TimelineEditor` for the duration of the pointer gesture. `onTimelinePatch`, `onMoveItems`, `onDuplicateItemsAtPositions`, and `onRippleTrim` are not called from pointer movement.

Pointer release resolves the final geometry once and invokes the appropriate callback once. The parent project model, persistence layer, render preview, and undo history therefore receive one canonical edit rather than a stream of intermediate edits.

## Error and Cancellation Behavior

- Invalid cross-track targets keep the existing local rejection state and do not preview a row change that cannot be committed.
- Invalid ripple-trim plans continue to show the existing inline error and never commit an ordinary trim as a fallback.
- Pointer cancellation clears the active interaction and snap guide without changing project state.
- A press and release below the existing movement threshold remains a selection gesture and emits no move action.
- Timeline start and minimum-duration constraints are applied during preview, not only during commit, so rendered clips never show impossible geometry.

## Testing

Focused `TimelineEditor` tests will prove:

- a normal clip changes its rendered `left` position during pointer movement before any callback fires;
- a compatible cross-track drag changes its rendered row during pointer movement;
- an invalid cross-track drag keeps the original row and shows rejected feedback;
- all selected group items preview their final relative movement;
- right-edge resize changes rendered width before release;
- left-edge resize changes rendered left position and width before release;
- snapped and clamped preview geometry equals the geometry committed on release;
- pointer cancellation restores canonical geometry and emits no action;
- pointer release emits exactly one canonical action;
- Option-drag and ripple-trim previews retain their existing behavior.

After the focused tests pass, run the TypeScript lint command and the existing browser visual QA against desktop and narrow timeline layouts. Visual review must confirm that clips track the pointer without transition lag and that handles, labels, snap guides, and valid or invalid target feedback remain readable.

## Out of Scope

- Persisting intermediate pointer positions.
- Adding a new undo or transaction system.
- Changing timeline snapping thresholds or compatibility rules.
- Replacing the existing Option-drag duplicate ghost.
- Changing preview-compositor playback while a timeline gesture is active.
