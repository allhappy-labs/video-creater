# Timeline Fit Control Removal Design

## Context

Palmier's timeline zoom cluster is visually minimal: zoom controls and a slider live at the right
edge of the timeline toolbar without a separate fit-to-overview button. Video Creater still exposes a
`Fit timeline zoom` button in that cluster, which adds one more toolbar control than the Palmier
reference shows.

This supersedes the visible toolbar control from
`2026-06-26-timeline-fit-zoom-design.md`. Fit-like overview behavior can return later through a
non-persistent command palette, shortcut, or contextual menu if needed.

## Requirements

- The timeline toolbar must not render a `Fit timeline zoom` button.
- The zoom cluster must keep `Zoom out timeline`, `Timeline zoom`, and `Zoom in timeline`.
- The zoom slider remains the manual way to return to overview scale.
- Zoom in, zoom out, slider value, `aria-valuetext`, ruler spacing, and clip widths remain
  unchanged.
- Preview-player fit controls outside the timeline toolbar are out of scope.

## Validation

- Component tests assert the timeline toolbar has no `Fit timeline zoom` button.
- Existing zoom button and zoom slider behavior tests continue to pass.
- Browser QA confirms the right-side timeline zoom cluster is icon, slider, icon with no extra fit
  button.
