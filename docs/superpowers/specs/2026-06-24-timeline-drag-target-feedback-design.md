# Timeline Drag Target Feedback Design

## Context

Palmier presents the timeline as the place where human and agent edits land: generated clips can be swapped in place, clips can be trimmed or reordered, and the editor shows enough context to understand the edit before committing it. Video Creater now supports cross-track body drags, but the UI does not preview whether the row under the pointer is a valid destination.

## Goal

Make manual timeline item moves feel predictable by showing the destination row while the pointer is moving and keeping the final `moveItem` patch aligned with that preview.

## Behavior

- When a draggable item body is pressed, the item captures pointer movement until release.
- While moving, the row under the pointer gets an explicit target state.
- Compatible unlocked rows show an accepted target treatment.
- Locked or incompatible rows show a rejected target treatment and keep the item on its original track when released.
- Horizontal start-time movement continues to work with or without vertical row movement.
- Resize handles keep their existing behavior and do not show row target feedback.

## Visual Treatment

- Accepted rows use a subtle cyan/blue inset ring and transparent fill so clips remain readable.
- Rejected rows use a restrained amber/red inset ring and transparent fill.
- Track labels mirror the same target state so the user can understand the destination even when clips cover the timeline grid.
- States are exposed with `aria-label` text for testing and assistive technology.

## Implementation Notes

- Extend move interaction state with the current pointer Y.
- Reuse the same row-resolution and compatibility rules for both preview and final patch creation.
- Add pointer capture on the item body when available, with a fallback for test environments.
- Keep the compatibility helper local to the timeline editor until another component needs it.

## Verification

- Timeline editor tests prove valid target rows expose accepted feedback during drag.
- Timeline editor tests prove incompatible rows expose rejected feedback during drag.
- Existing move tests prove final patches still target the compatible row or fall back to the original row.
- Browser QA checks desktop and narrow timeline layouts after a representative drag.
