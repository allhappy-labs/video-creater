# Source-Aware Right Edge Trim Design

## Context

Palmier's editor treats clips as timeline-native objects that can be trimmed and adjusted without leaving the project. Video Creater now supports left-edge trimming through `trimItem`, but right-edge dragging still emits `resizeItem` for every item. That changes duration without updating `sourceOut`, which can make source-backed clips disagree with their project-file source range.

## Goal

Make right-edge drags source-aware. Media-backed items with numeric `sourceIn` and `sourceOut` should trim through `trimItem` so the timeline duration and source range remain coherent. Items without media source range metadata should keep the existing `resizeItem` behavior.

## Behavior

- Dragging the right edge of a media-backed item with `sourceIn` and `sourceOut` emits one `trimItem` patch.
- The patch keeps `startSeconds` and `sourceIn` fixed.
- The patch updates `durationSeconds` and shifts `sourceOut` by the drag delta.
- The duration remains clamped to at least 0.1 seconds.
- Dragging the right edge of an item without media source range metadata continues to emit `resizeItem`.
- Locked-track behavior is unchanged because handles remain hidden.

## Implementation Notes

- Add `createRightTrimPatchFromDrag` beside `createLeftTrimPatchFromDrag`.
- Store the active right-edge `TimelineItem` in the interaction state instead of only item id and original duration.
- In `finishInteraction`, call the right trim helper and emit its `trimItem` patch when it returns one; otherwise fall back to `resizeItem`.
- Reuse the existing `projectActionFromTimelinePatch` mapping to submit split-project `trimItems`.

## Verification

- Unit tests prove the right trim helper updates `sourceOut` and returns `null` for plain items.
- Timeline editor tests prove source-backed right-edge drag emits `trimItem`, while plain items still emit `resizeItem`.
- Workspace tests prove source-backed right-edge drag submits a split-project `trimItems` action.
