# Generated Source Quick Action Row

## Context

The generated Source Inspector now shows generated details and AI edit controls inline, but selected generated sources still render `Reveal source media` and `Use in composer` as separate full-width buttons before the readout. Palmier-style inspector actions should stay direct without turning the right rail into a stack of command bars.

## Goal

Render generated-source reveal and composer handoff as one compact quick-action row below the source identity. Preserve existing accessible action names and callbacks.

## Requirements

- Generated sources render a `Generated source quick actions` group when reveal or composer handoff actions are available.
- The group uses a two-column compact layout when both actions are available.
- `Reveal source media` continues to call `onRevealSource(mediaId)`.
- `Use generated source in composer` continues to call `onUseGeneratedAssetInComposer(assetId, placementIntent)`.
- Imported source behavior remains unchanged in this slice.
- Do not remove generated details, generated AI edit, rerun, replacement, variation, variation-set, or upscale actions.

## Verification

- Source Inspector tests prove generated quick actions render in a compact row and still call their callbacks.
- Existing generated Source Inspector behavior tests pass.
- Browser QA confirms the right rail no longer stacks generated quick actions as separate full-width rows.
