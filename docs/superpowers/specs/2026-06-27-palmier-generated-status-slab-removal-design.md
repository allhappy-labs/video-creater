# Palmier Generated Status Slab Removal Design

## Goal

Make completed AI outputs in the source library read more like ordinary Palmier media cards. The generated card should keep useful provenance, but avoid a second all-caps status slab that visually competes with the thumbnail and actions.

## Design

- Keep the generated asset title, prompt, model/provider status line, output selection, composer reuse, replacement, insertion, lineage, workflow status, and mock worker actions.
- Remove only the standalone uppercase status pill inside generated asset cards.
- Keep queued/running/failed state visible through the existing status/model line and workflow/action controls.
- Preserve the accessible `AI generations` region and `AI generation grid`.

## Testing

- `MediaBin` proves completed generated cards still show `completed - provider/model`.
- `MediaBin` proves there is no redundant `.uppercase` standalone status element for completed generated cards.
- Existing generated asset tests continue to cover output selection, composer reuse, placement, mock completion, retry, replacement, and insertion.
