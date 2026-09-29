# Generated Clip Rerun Action Design

## Context

Palmier describes AI edits as timeline-native: generated clips can be rerun with the same prompt, tweaked into a variation, inspected for prompt/reference provenance, or swapped without leaving the editor. Video Creater already exposes generated-source details and an `AI Edit` tab with a variation prompt, but rerunning the same prompt is implicit: the editor has to open `AI Edit` and submit the prefilled prompt.

## Goal

Add an explicit `Rerun same prompt` action for selected generated clips in the Source Inspector.

## Behavior

- When the selected source resolves to a generated asset and `onQueueVariation` is available, the Source Inspector shows `Rerun same prompt`.
- Pressing `Rerun same prompt` calls `onQueueVariation(generatedAsset.id, generatedAsset.prompt)`.
- The action is available from the generated details view so an editor can rerun while inspecting provenance.
- The existing `AI Edit` tab, editable `Variation prompt`, `Queue variation`, and `Queue upscale` behavior remain unchanged.
- If no generated asset is selected, or if no queue callback is provided, the rerun action is not shown.

## Visual Treatment

- Keep the action compact and inspector-native: an outline button near the generated details summary.
- Use the same button language as other expensive actions.
- Do not introduce a popover or confirmation in this slice.

## Verification

- Source inspector tests prove the details tab renders `Rerun same prompt` for generated sources with a queue callback.
- Source inspector tests prove clicking it calls the existing variation callback with the original generated prompt.
- Existing variation-prompt tests keep proving the tweak path still works.
- Browser QA checks the action appears in the right-rail generated source inspector without pushing controls into overlap.
