# Palmier Generated Output Choices Flattening

## Goal

Make generated output swapping read like the rest of the Palmier-style right inspector. Alternate generated outputs appear as a flat read-only/action group, not as a bordered card section with nested bordered rows.

## Requirements

- Keep `Generated outputs` as a region so tests and assistive technology can find the output chooser.
- Remove card shell classes from the `Generated outputs` region: no rounded border, background shell, or padded panel treatment.
- Remove bordered row shells from each output candidate.
- Keep the output filename, summary, media id, current indicator, and swap action.
- Keep `Swap <clip> to <media id>` behavior and replacement validation unchanged.
- Keep generated metadata, references, prompt, and AI edit actions unchanged.

## Non-Goals

- No generated asset schema changes.
- No Temporal, fal.ai, queue, or workflow changes.
- No changes to first-frame, last-frame, reference, or output provenance preview cards.

## Testing

- `SourceClipInspector` verifies the `Generated outputs` region and output rows are flat.
- Existing tests continue to verify current output labeling and swap callbacks.
- Browser QA checks a generated timeline clip with alternate outputs at desktop and narrow widths for no horizontal overflow.
