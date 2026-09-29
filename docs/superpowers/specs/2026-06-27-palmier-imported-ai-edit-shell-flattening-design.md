# Palmier Imported AI Edit Shell Flattening Design

## Goal

Make imported-source AI edit controls match the direct Palmier-style inspector treatment already used for generated-source edits. The controls should remain available, but the wrapper should not look like a nested card inside the source inspector.

## Design

- Expose imported-source actions as an accessible `Imported AI edit` region.
- Remove the region's rounded border, background, and padding shell.
- Preserve `Queue upscale`, referenced generation prompt editing, and `Queue referenced shot`.
- Keep imported source file details, source selection, timeline editing, queue payloads, Temporal workflow behavior, and fal.ai integration unchanged.

## Testing

- `SourceClipInspector` verifies imported source details and AI edit controls remain inline and tabless.
- The same test verifies `Imported AI edit` is a flat accessible region.
- Existing imported source tests continue to cover referenced generation and upscale callbacks.
