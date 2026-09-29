# Palmier Generated Source Inspector Metadata

## Goal

Make selected generated-source metadata match the plain Palmier-style right inspector. Generated sources no longer use one mixed `File` block; they expose generation provenance and output format as separate flat groups beside the AI edit controls.

## Requirements

- Keep the `Generated details` group flat, without card, border, or background shell classes.
- Replace the generated `File` metadata heading with two flat read-only groups:
  - `Generation`: `Model`, `Status`, `Aspect`, and `Path`.
  - `Format`: `Type`, `Duration`, `Resolution`, and `Frame Rate`.
- Keep `Generated file details` as the accessibility label for the containing metadata group.
- Keep first-frame, last-frame, reference, and output preview cards unchanged.
- Keep prompt copy, regenerate, rerun, replacement, insert, and upscale actions unchanged.
- Do not expose Temporal workflow internals in this metadata block.

## Non-Goals

- No schema, Temporal, fal.ai, workflow, or media-library changes.
- No redesign of generated reference preview tiles.
- No changes to imported-source metadata, which was handled in the previous slice.

## Testing

- `SourceClipInspector` verifies generated metadata renders `Generation` and `Format` groups.
- Tests verify `File` no longer appears as the generated metadata section heading.
- Existing generated provenance and AI edit tests continue to cover references, prompt copy, queue variation, rerun, replacement, insert, and upscale behavior.
- Browser QA checks a selected generated source at desktop and narrow widths for the new groups and no horizontal overflow.
