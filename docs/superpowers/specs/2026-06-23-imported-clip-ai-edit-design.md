# Imported Clip AI Edit

## Context

Palmier documents timeline-native AI editing: users can click any clip, including imported media, and AI edit it without leaving the timeline. Video Creater already supports generated-clip AI variations and Codex referenced generation from selected media, but imported timeline clips only expose manual trim, split, and reorder controls.

## Goal

Add an AI Edit path for imported visual timeline clips that queues a generated shot using the clip source media as a reference.

## Behavior

- When a selected timeline source clip is backed by visual media and is not already a generated asset, the Source Inspector shows `Details` and `AI Edit` tabs.
- The `Details` tab keeps the existing manual trim, split, reveal, and sequence controls.
- The `AI Edit` tab contains a prompt textarea and `Queue referenced shot` action.
- The action is disabled until the prompt contains non-whitespace text.
- Queueing uses the selected clip's source media id as both `firstFrameMediaId` and the reference media id.
- Audio-only clips do not expose this imported-clip AI Edit action.
- Generated clips keep the existing generated details and variation flow.
- The Codex selected timeline clip block also shows an `AI edit` group for imported visual clips,
  with `Queue referenced shot` and `Queue upscale` actions that use the selected clip source media.

## Non-Goals

- No automatic replacement of the selected clip after generation completes.
- No model picker in the Source Inspector.
- No generated media execution changes.
- No schema change for timeline placement intent.

## Data Flow

1. `SourceClipInspector` accepts `onQueueReferencedGeneration(mediaId, prompt)`.
2. For non-generated visual source clips, it renders a local `AI Edit` tab.
3. Clicking `Queue referenced shot` calls the callback with the clip source media id and trimmed prompt.
4. `EditorWorkspace` wires that callback to the existing referenced generation queue path.
5. The queued generated asset uses the current default video generation settings: `fal.ai/fal-ai/wan-25-preview/text-to-video`, 4 seconds, 16:9, 1280x720, 24 fps.
6. `AgentPanel` reuses the same callbacks from the selected timeline clip context so agents and
   users can queue an imported-clip AI edit without moving focus to the right inspector.

## Tests

- `SourceClipInspector` exposes imported visual clip AI Edit and calls the callback with trimmed prompt.
- `SourceClipInspector` does not expose imported AI Edit for audio clips.
- `EditorWorkspace` queues a `recordGeneratedAsset` action from the selected imported timeline clip.
- `AgentPanel` exposes imported visual selected-clip AI edit actions and calls the referenced
  generation/upscale callbacks with the selected clip source media id.
