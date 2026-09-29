# Palmier Generation Footer Summary Removal Design

## Context

The active generation recipe now exposes mode, placement, destination, model, output, timing, estimate, and reference context in normal scroll flow. The composer still renders a separate `Generation settings summary` immediately above the submit footer, duplicating the model/output/timing content and competing with the compact queue bar.

Palmier keeps the final generation action compact and leaves detailed settings in the editable recipe area.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the duplicated standalone settings summary so the generation composer has one canonical recipe summary plus a compact submit footer.

## Behavior

- `Active generation recipe` remains the canonical visible summary for mode, model, output, timing, references, destination, placement, and estimate.
- `Generation submit footer` remains visible with `Generation submit estimate` and `Queue generation`.
- The composer no longer renders `Generation settings summary`.
- Generation mode switching still updates the active recipe for image, video, and audio modes.
- Generation request payloads, provider metadata, placement, history, and media reference behavior remain unchanged.

## Verification

- Update `MediaBin` regression coverage so mode and recipe tests no longer depend on `Generation settings summary`.
- Add assertions that the composer does not render `Generation settings summary`.
- Browser-smoke desktop and narrow composer layouts to confirm the active recipe remains readable and the submit footer stays compact.
