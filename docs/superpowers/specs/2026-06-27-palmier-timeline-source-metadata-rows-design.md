# Palmier Timeline Source Metadata Rows Design

## Goal

Make selected timeline source context in the right inspector read like Palmier metadata rows instead of boxed chips.

## Design

- Keep source range and reason/media fallback visible for generated and imported timeline selections.
- Wrap the rows in an accessible `Timeline source metadata` group.
- Render `Source` and `Reason` as flat label/value rows with the value aligned for scanning.
- Reuse one renderer for generated and imported timeline source branches.
- Preserve trim, split, reorder, generated detail, and imported source detail controls.

## Testing

- `SourceClipInspector` verifies generated timeline source metadata is accessible and unboxed.
- The timeline inspector variant verifies source range and reason still render in the shared row group.
