# Preview Transport Kind Label Removal Design

## Context

Palmier's viewer footer keeps playback controls, time, source range when useful, compact format
badges, and `Fit`. It does not repeat generic media-kind labels in the transport chrome. Video
Creater still shows timeline item kind twice in timeline preview mode (`video clip` as the center
label and again as a badge) and adds a source-kind badge such as `VIDEO` in source preview mode.

## Requirements

- Timeline preview transport must not render generic item-kind text such as `video clip`.
- Source preview transport must not render source-kind badges such as `VIDEO`, `AUDIO`, or
  `GENERATED`.
- Source range labels remain visible when available.
- Resolution, aspect ratio, fps, quality, time readout, scrubber, transport buttons, and `Fit` stay
  unchanged.
- Source viewer detail panels outside the transport may continue to show media kind labels.

## Validation

- Component tests assert timeline preview transport has no duplicated item-kind label.
- Component tests assert source preview transport has no source-kind badge while keeping source
  range and format metadata.
- Browser QA confirms the viewer footer has compact Palmier-style metadata instead of repeated kind
  labels.
