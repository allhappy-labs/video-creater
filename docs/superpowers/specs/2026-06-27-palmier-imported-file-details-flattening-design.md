# Palmier Imported File Details Flattening Design

## Goal

Make imported media file metadata in the source inspector match the flat Palmier-style rows already used for generated metadata. The right rail should read as one compact inspector, not a stack of nested file cards.

## Design

- Expose imported file metadata as an accessible `Imported file details` group.
- Remove the imported file details rounded border, background, and padding shell.
- Preserve the existing `File`, type, duration, dimensions, frame rate, and path rows.
- Keep imported AI edit controls, source selection, timeline editing, queue payloads, Temporal workflow behavior, and fal.ai integration unchanged.

## Testing

- `SourceClipInspector` verifies imported file details are accessible, flat, and still show key metadata values.
- Existing imported source tests continue to cover referenced generation and upscale callbacks.
