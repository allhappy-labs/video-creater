# Timeline Fit Zoom Design

## Context

Palmier-style editing keeps the timeline dense but recoverable: editors can zoom into detailed clip
work and quickly return to an overview of the whole project. Video Creater already has zoom in,
zoom out, and a zoom slider, but returning from a zoomed-in state requires repeated clicks or manual
slider adjustment.

## Goal

Add a compact timeline fit control that returns the timeline to an overview zoom using the existing
internal zoom model.

## Behavior

- The timeline zoom cluster exposes a `Fit timeline zoom` icon button.
- The fit button computes a project overview zoom from timeline duration, base pixels per second,
  and the existing minimum canvas width.
- The computed zoom is clamped to the existing 50-200% range and aligned to the existing 25% zoom
  step.
- The button is disabled when the timeline is already at its fit zoom.
- Existing zoom in, zoom out, slider, ruler spacing, clip sizing, seeking, dragging, and keyboard
  edit behavior remains unchanged.

## Non-Goals

- No viewport measurement, scroll positioning, minimap, or responsive zoom model change.
- No persistence of zoom into project files.
- No new Rust, Temporal, render, or project action behavior.

## Verification

- `TimelineEditor` tests prove a zoomed long timeline can return to the computed overview zoom.
- Existing zoom button and slider tests keep passing.
- Browser QA checks that the fit button appears in the toolbar, changes the zoom readout, and keeps
  the timeline controls readable.
