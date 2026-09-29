# Generic Timeline Item Resize Design

## Context

Palmier's docs describe timeline-native agent edits where clips can be trimmed, split, reordered, and adjusted without leaving the timeline. Video Creater already has a generic `resizeItem` patch and `resizeItems` project action, but the visible timeline resize handles are limited to caption items.

## Goal

Expose the existing right-edge duration resize workflow for every selected timeline item on an unlocked track. This keeps manual editing behavior aligned across video clips, generated clips, overlays, HyperFrames scenes, audio clips, and captions.

## Behavior

- Selected timeline items on unlocked tracks show the same edge handles currently used by captions.
- Dragging the right edge emits `resizeItem` with the item id and the updated duration.
- Duration remains clamped to at least 0.1 seconds by the existing timeline interaction logic.
- Locked tracks hide resize handles and do not emit resize patches.
- The left handle remains visual only until a separate source-in and start-time trim design is implemented.
- Source range trim buttons remain the toolbar path for source-mark changes.

## Verification

- Component coverage proves a selected video clip right-edge drag emits a generic `resizeItem` patch.
- Workspace coverage proves the same interaction is translated to a split-project `resizeItems` action.
- Existing locked-track coverage continues to prove resize handles are hidden for locked items.
- Existing Rust project-action tests continue to own backend validation for duration and locked-track mutations.
