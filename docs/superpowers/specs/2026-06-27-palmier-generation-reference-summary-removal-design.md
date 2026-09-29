# Palmier Generation Reference Summary Removal Design

## Context

The generation composer now has three places that expose visual references: editable first/last/reference slots, the active generation recipe, and a read-only `Generation references` block after the prompt. The third surface repeats the same reference state without adding a decision, making the composer taller and less like Palmier's direct editor controls.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the duplicated read-only `Generation references` block and keep reference state visible through the editable slots plus the active generation recipe.

## Behavior

- `Active generation recipe` remains the canonical read-only summary for first frame, last frame, and reference media.
- Editable first/last/reference slots remain unchanged and continue to show thumbnails, filenames, empty states, and removal or `Use selected` actions.
- The composer no longer renders the standalone `Generation references` group after the prompt.
- Image mode still shows reference state in the recipe and editable reference slot.
- Audio mode still has no visual reference slots or reference summary.
- Generation request payloads, Temporal records, fal.ai provider calls, generated asset metadata, and reference seeding remain unchanged.

## Verification

- Update `MediaBin` coverage so visual reference expectations use the active recipe and editable slots instead of `Generation references`.
- Add absence assertions for the removed standalone group in video and image modes.
- Browser-smoke desktop and narrow composer layouts to confirm the recipe stays readable and the submit footer remains compact.
