# Timeline Zoom Readout Removal Design

## Context

Palmier's timeline toolbar keeps the zoom cluster compact: icon buttons frame a small slider, and
the timeline does not spend persistent chrome on a textual zoom percentage. Video Creater still
shows a `100%` readout beside the zoom slider, which adds visual noise to the timeline header.

## Requirements

- The timeline zoom cluster must keep the `Zoom out timeline`, `Fit timeline zoom`,
  `Timeline zoom`, and `Zoom in timeline` controls.
- The zoom slider must keep its numeric value for assistive technology and tests.
- The toolbar must not render a visible percentage readout such as `100%`.
- Zoom button, fit zoom, slider, ruler spacing, and clip width behavior must remain unchanged.

## Validation

- Component tests assert the compact toolbar has the zoom controls and no visible zoom percentage.
- Existing zoom behavior tests continue to pass.
- Browser QA confirms the timeline toolbar visually matches the compact Palmier zoom cluster.
