# Generated AI Edit Recipe Summary Design

## Context

Palmier keeps generated-media context visible while users tweak prompts or generate variations. Video
Creater's AI Edit tab already shows model, output, settings, and references, but the model/settings
context is presented as detail rows rather than the compact recipe summary used in the Details tab.

Palmier reference: https://www.palmier.io/docs

## Goal

Show the same compact generated recipe summary in the AI Edit tab so prompt iteration starts with an
at-a-glance model, aspect, resolution, duration, and status readout.

## Behavior

- The `Generation context` region in AI Edit renders an `AI edit recipe summary` group before the
  existing model/output/settings rows and references.
- The group uses the same compact badge-style cells as the generated Details summary.
- Existing AI edit rows, reference tiles, workflow route, prompt restore, variation, replacement,
  variation-set, upscale, and callback payload behavior remain unchanged.

## Non-Goals

- No generated asset schema, prompt semantics, fal.ai, Temporal, workflow, or queue changes.
- No change to the Details tab summary.
- No new generated edit modes.

## Verification

- Add a `SourceClipInspector` test proving the AI Edit generation context renders model, aspect,
  resolution, duration, and status inside the compact summary group.
- Existing AI Edit queue and generation-context tests continue to pass.
- Browser-smoke the AI Edit tab and confirm the summary fits above references without overlapping
  the prompt controls.
