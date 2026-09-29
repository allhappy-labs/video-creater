# Generic Timeline Item Drag Design

## Context

Palmier's editor treats timeline clips as native editable objects. The current Video Creater timeline can move captions by dragging, but imported clips, generated clips, template overlays, HyperFrames, and audio clips do not start a move interaction from their clip body. The validated project action path already supports moving any compatible item through `moveItems`; the UI is the limiting layer.

## Goal

Allow every unlocked timeline item kind to move horizontally on its current track by dragging the clip body.

## Behavior

- Pointer-dragging an unlocked item body starts a move interaction for video clips, generated clips, overlays, HyperFrame scenes, captions, and audio clips.
- The move remains same-track in this slice.
- The move emits the existing `moveItem` timeline patch with the item id, current track id, and clamped start seconds.
- Locked tracks continue to reject body drags and do not emit move patches.
- Resize handles remain caption-only until non-caption resize affordances are designed.
- Split, trim, delete, template drop, and source-open behavior remains unchanged.

## Verification

- Component test proves a video clip drag emits a `moveItem` patch.
- Component test coverage for locked tracks continues to prove locked items do not move.
- Workspace test proves dragging an imported/video item routes to the Rust-validated split-project `moveItems` action.
- Existing Rust project-action tests continue to validate cross-kind move compatibility and locked-track rejection.
