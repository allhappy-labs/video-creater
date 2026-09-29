# Timeline Snapping Toggle Design

## Context

Palmier's timeline behaves like a manual editor surface: clips, generated outputs, and template
layers can be placed with precise visual feedback. Video Creater can drag clips, resize edges, and
drop templates, but placement currently preserves arbitrary fractional seconds from pointer
movement. That makes hand edits harder to align and leaves the timeline without a basic snap control.

Palmier reference: https://www.palmier.io/docs

## Goal

Add a local timeline snapping toggle that quantizes manual timeline placement while keeping
project-file edits and Rust validation unchanged.

## Behavior

- The timeline toolbar shows a `Timeline snapping` icon button near the selection and split tools.
- Snapping is enabled by default and the button exposes `aria-pressed="true"`.
- When enabled, clip body drags, left/right edge trims, and template drops snap to `0.25` second
  increments before emitting the existing timeline patch or drop callback.
- When disabled, the same interactions preserve the existing unsnapped fractional-second behavior.
- Snapping never changes track selection, cross-track compatibility, locked-track rules, generated
  media actions, or project schema.

## Non-Goals

- No magnetic snapping to clip boundaries, playhead, markers, or transcript words in this slice.
- No persisted user preference.
- No Rust action or project-file schema change.

## Verification

- `TimelineEditor` tests prove the snapping button is pressed by default.
- Dragging a clip with snapping enabled emits a snapped `startSeconds`.
- Toggling snapping off preserves the existing unsnapped decimal placement.
- Template drops snap the inserted start time when snapping is enabled.
