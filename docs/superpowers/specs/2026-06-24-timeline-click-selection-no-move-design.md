# Timeline Click Selection Without Move

## Context

Palmier-style timeline editing relies on clear separation between clicking a clip to edit it and dragging a clip to move it. Video Creater already supports clip selection, source opening, drag moves, and source inspector editing. During browser QA, a plain click on a timeline clip also committed a zero-distance `moveItem` patch because `pointerdown` always started a move interaction and `pointerup` always finished it.

That behavior is noisy in the Tauri shell and especially disruptive in Vite QA, where the accidental patch can attempt a backend project write and surface a Tauri `invoke` error. It also weakens manual editing confidence: selection should not mutate the timeline.

## Goal

Make a plain click on a timeline clip selection-only. Dragging remains the way to emit move patches.

## Behavior

- Pointer down/up with no meaningful pointer movement must not call `onTimelinePatch`.
- The normal click handler still calls `onSelectItem`.
- Existing horizontal and vertical drag moves still emit `moveItem` patches.
- Resize handles keep their existing behavior.
- No project schema, Temporal workflow, or Rust command changes.

## Implementation

Add a small pointer-movement threshold to move interactions. When finishing a `move` interaction, compare pointer-up coordinates with the pointer-down coordinates. If both axes stay below the threshold, clear the interaction and skip patch creation.

## Tests

- Add a `TimelineEditor` test proving a clip pointer down/up at the same coordinates followed by click selects the item without calling `onTimelinePatch`.
- Keep existing drag tests proving move patches still work.
