# Palmier Active Generation Recipe Rows Design

## Goal

Make the media generation composer recipe summary read like Palmier-style editor metadata instead of a nested settings card. The recipe should keep the same facts, but use compact label/value rows that sit directly in the sheet flow.

## Design

- Keep the accessible `Active generation recipe` region.
- Remove the recipe summary's rounded border, background shell, and boxed per-field cells.
- Render mode, placement, model, output, timing, timeline target, references, and estimate as compact rows.
- Keep `Generation submit footer`, `Generation submit estimate`, model/output controls, reference slots, placement status, and queue payloads unchanged.
- Do not change Temporal job records, fal.ai provider payloads, generated asset schemas, or media selection behavior.

## Testing

- `MediaBin` verifies the recipe region remains accessible and visible.
- `MediaBin` verifies the recipe region and its mode row no longer use rounded bordered mini-card styling.
- Existing recipe tests continue to prove mode, placement, estimate, references, and generation payloads remain unchanged.
