# Cross-Track Timeline Move Design

## Context

Palmier documents timeline-native trim, split, reorder, and adjust operations for both humans and connected agents. Video Creater can already drag timeline item bodies horizontally to change `startSeconds`, and Rust project actions can move an item to another compatible track. The React timeline does not yet use that cross-track capability.

## Goal

Let a selected timeline item body drag vertically onto another compatible, unlocked track while preserving the existing horizontal start-time drag behavior.

## Behavior

- Dragging an item body horizontally keeps the existing `moveItem` patch with the original target track.
- Dragging an item body vertically by about one row targets the corresponding timeline row.
- The target row is accepted only when the destination track is unlocked and compatible with the item kind.
- Incompatible or locked destination rows fall back to the original track.
- Rust project-action validation remains the final authority for track compatibility and locked-track failures.

## Implementation Notes

- Extend move interaction state with the item kind, original row index, and starting pointer Y.
- Resolve the target row in `finishInteraction` from `deltaY / rowHeight`.
- Add a small TypeScript compatibility helper mirroring the Rust `item_allowed_on_track` rules.
- Keep resize handles and left/right trim behavior unchanged.

## Verification

- Timeline editor tests prove compatible vertical dragging changes `targetTrackId`.
- Timeline editor tests prove incompatible vertical dragging keeps the original `targetTrackId`.
- Workspace tests prove split-project actions receive the compatible target track id.
