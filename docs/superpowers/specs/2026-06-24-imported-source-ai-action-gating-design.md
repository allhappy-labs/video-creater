# Imported Source AI Action Gating

## Context

Palmier documents that clicking any imported or AI clip should expose in-timeline AI edit actions, including upscaling and referenced generation. Video Creater's `SourceClipInspector` already has both actions for imported visual sources, but the whole imported `AI Edit` tab is currently gated by `onQueueReferencedGeneration`.

That means a project state with only `onQueueUpscale` available cannot expose `Queue upscale`, even though upscaling has its own independent callback.

## Goal

Expose imported visual source AI actions independently based on the callbacks that are actually available.

## Behavior

- Imported visual clips and selected imported visual library media show the `AI Edit` tab when either referenced generation or upscale is available.
- `Queue upscale` appears whenever `onQueueUpscale` is available for imported visual media.
- The referenced generation prompt and `Queue referenced shot` appear only when `onQueueReferencedGeneration` is available.
- Audio and generated media keep their existing gating.
- Existing combined referenced-generation-plus-upscale behavior remains unchanged.

## Non-Goals

- No changes to generated clip rerun or variation controls.
- No project schema changes.
- No Temporal workflow changes.
- No new provider request shape.

## Tests

- Add a `SourceClipInspector` test where an imported visual source receives only `onQueueUpscale`; the `AI Edit` tab and `Queue upscale` action should be visible and callable, while referenced-generation controls stay hidden.
