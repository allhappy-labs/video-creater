# Codex Selected Source Upscale Design

Palmier treats AI edits as native to the timeline and agent context: an editor can select a clip or generated source, then ask the assistant to keep working with that same source. Video Creater already lets the right Source Inspector queue an upscale for visual media, but the Codex selected-source block only offers insert, replace, variation, and referenced generation. This leaves an agent-facing gap for a common edit action that already exists in the workspace.

## Scope

- Add an optional `canQueueUpscale` flag to the selected media context exposed to `AgentPanel`.
- Show `Queue upscale` in the Codex selected-source action group when the selected source is visual and an upscale callback is available.
- Route the action through the existing `queueMediaUpscale` workspace path so it records a Temporal workflow job and generated asset with the established upscale prompt and reference settings.
- Hide the action for audio sources.

## Out of Scope

- New upscale model selection UI.
- Changing the existing upscale prompt or settings.
- Directly mutating project media files.

## Acceptance

- AgentPanel tests prove the selected-source block renders and calls `Queue upscale` only when supported.
- EditorWorkspace tests prove clicking `Queue upscale` from the Codex selected source records a queued Temporal-backed generated asset action.
- Existing insert, replace, variation, referenced generation, and right-inspector upscale behavior remains unchanged.
