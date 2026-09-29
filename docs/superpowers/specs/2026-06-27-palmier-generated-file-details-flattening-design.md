# Palmier Generated File Details Flattening Design

## Goal

Make generated source file metadata in the right inspector match Palmier's plain section-and-row rhythm. The file details should stay readable and accessible without appearing as another nested card.

## Design

- Keep the generated `File` section and its current rows: type, duration, dimensions, frame rate, and path.
- Add an accessible `Generated file details` group around the file metadata.
- Remove the rounded bordered background from the generated file-details section.
- Preserve the existing `detailRow` label/value layout and all generated source actions.

## Testing

- `SourceClipInspector` proves generated file details have an accessible group.
- `SourceClipInspector` proves the file detail group no longer uses rounded, border, or background card styling.
- Existing generated detail tests continue to prove the file metadata, references, prompt, and generated actions remain visible.
