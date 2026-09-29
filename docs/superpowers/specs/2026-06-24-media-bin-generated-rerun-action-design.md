# Media Bin Generated Rerun Action Design

## Context

Palmier treats AI-generated clips as reusable project assets: an editor can inspect the source prompt and rerun the same generation without leaving the media workflow. Video Creater now exposes `Rerun same prompt` in the Source Inspector, but the Media Bin's selected generated source details still require switching to `AI Edit` and submitting the prefilled prompt.

## Goal

Add an explicit `Rerun same prompt` action to the Media Bin selected generated source details.

## Behavior

- When a selected media item resolves to a generated asset and `onQueueGeneratedVariation` is available, the selected generated source details show `Rerun same prompt`.
- Pressing `Rerun same prompt` calls `onQueueGeneratedVariation(selectedGeneratedAsset.id, selectedGeneratedAsset.prompt)`.
- The action appears in the `Details` tab so an editor can rerun while reading provenance, model, settings, and reference media.
- The existing `AI Edit` tab remains the editable tweak path and continues to queue the edited prompt.
- If no generated source is selected, or no queue callback is available, the rerun action is hidden.

## Visual Treatment

- Keep the action compact and source-panel native: a full-width outline button above the prompt card.
- Use the same `Rerun same prompt` language as the Source Inspector to keep generated asset actions consistent.
- Do not add confirmation, modals, or alternate queue mechanics in this slice.

## Verification

- Media Bin tests prove the selected generated source details render `Rerun same prompt` when a queue callback is available.
- Media Bin tests prove clicking it queues the original generated prompt and generated asset id.
- Media Bin tests prove the action is hidden without a queue callback.
- Existing AI Edit tests continue proving edited variation prompts still work.
- Browser QA checks the Media Bin selected generated source panel remains readable at desktop and narrow widths.
