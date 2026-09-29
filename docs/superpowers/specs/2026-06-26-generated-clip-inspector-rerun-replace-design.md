# Generated Clip Inspector Rerun Replace Design

## Context

Video Creater removed the floating selected-clip action dock to keep the timeline canvas closer to the Palmier reference: clean clips, toolbar editing, and contextual actions in side panels. The removed dock also carried a one-click generated-clip replacement rerun. The Source Inspector already owns generated source details, AI Edit, prompt variation, output swap, insertion, and composer handoff, so replacement rerun belongs there instead of returning to the canvas.

## Decision

Add a compact `Rerun and replace selected clip` action to the Source Inspector generated details surface when the selected source is a generated timeline clip and replacement queueing is available. The action uses the generated asset's original prompt and queues a replacement variation for the selected timeline item.

## Requirements

- The action appears only when all are true:
  - a generated asset is selected,
  - a timeline item is selected,
  - a replacement target label is available,
  - `onQueueReplacementVariation` is provided.
- Pressing the action calls `onQueueReplacementVariation(generatedAsset.id, generatedAsset.prompt, item.id)`.
- The existing `Rerun same prompt` action continues to queue a non-replacement variation.
- Prompt editing stays in the existing `AI Edit` tab.
- `TimelineEditor` no longer exposes or receives the deleted inline-dock AI callback props.

## Testing

- `SourceClipInspector` proves the new details-tab action queues a replacement rerun with the original prompt and selected item id.
- `EditorWorkspace` updates the stale selected-dock integration coverage to use the Source Inspector replacement rerun action and confirm the same Temporal job/project actions are submitted.
- Existing generated clip AI Edit tests continue to prove prompt tweaks open through the inspector.
- Run focused inspector/workspace tests, TypeScript checks, and browser QA of the selected generated clip inspector.
