# Generated Output Composer Provenance Seed

## Context

Palmier keeps generated media provenance close to editing: selected generated outputs show the
prompt, first frame, last frame, references, and model settings, and editors can continue or vary a
shot without manually rebuilding that context. Video Creater already shows generated source
provenance and has a Palmier-style media generation composer, but opening the composer while a
generated output is selected seeds the output itself as the next first frame and reference.

Palmier reference: https://www.palmier.io/docs

## Goal

When a generated output is selected, seed the media generation composer from the source generated
asset's visual provenance instead of treating the output as the only reference.

## Behavior

- Opening `Generate media` with a normal selected visual media asset keeps the current behavior:
  the selected media preloads as video `First frame` and `Reference`.
- Opening it with a selected generated output checks the generated asset that produced that output.
- If valid visual provenance exists, the composer preloads:
  - `First frame` from `references.firstFrameMediaId`.
  - `Last frame` from `references.lastFrameMediaId`.
  - `Reference` from the first valid entry in `references.mediaIds`.
- Missing or non-visual provenance falls back to the selected generated output, preserving the
  existing single-reference workflow.
- Submitting the composer keeps the existing `MediaGenerationRequest` shape and model defaults.

## Non-Goals

- No schema changes.
- No new Temporal workflow or fal.ai provider behavior.
- No automatic prompt reuse; generation history still owns prompt reuse.
- No change to source inspector replacement or variation queue actions.

## Verification

- Media Bin test: selecting a generated output and opening the composer shows the original first
  frame, last frame, and reference slots.
- Media Bin test: submitting that composer sends the original provenance ids in the generated media
  request.
- Existing Media Bin generation and history tests continue passing.
