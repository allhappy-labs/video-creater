# Palmier Preview Transport Flattening Design

## Goal

Make the preview controls feel like a native editor transport strip instead of a nested settings card. Palmier keeps playback controls, timecode, scrubber, aspect, fps, quality, and fit controls in a compact flat bar directly under the viewer.

## Design

- Keep the `Preview transport` region and all existing playback, stepping, seeking, timecode, and metadata behavior.
- Replace the rounded bordered transport container with a flat top-divided strip inside the preview panel.
- Render resolution, aspect, frame rate, quality, and fit as plain compact transport readouts instead of boxed badges.
- Preserve icon-only playback buttons with existing accessible labels and keyboard shortcuts.

## Testing

- `PreviewPanel` proves the transport uses a flat `border-t` strip rather than `rounded-md border bg-background`.
- `PreviewPanel` proves transport readouts remain visible but are not boxed as badge pills.
- Existing preview tests continue to cover source playback, frame stepping, scrubber seeking, source tabs, and timeline preview behavior.
