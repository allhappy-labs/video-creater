# Mention Referenced Generation Action Design

## Context

Palmier chat lets editors use `@` to reference a specific image, look, subject, or frame, then ask the assistant to generate assets in context. Video Creater can already resolve typed mentions for edit requests, and the selected Codex source block can queue a referenced shot. The missing bridge is that a typed mention is not actionable unless the user also selects that same media in the library.

## Goal

When the prompt resolves to a visual `@` mention, expose a compact referenced-generation action beside the resolved mention so the editor can queue a new shot from the mentioned media directly from chat.

## Behavior

- The resolved mention row remains visible when a typed mention overrides the selected/default target.
- If the resolved mention is not audio, the prompt is non-empty, and `onQueueSelectedGeneration` exists, the row shows `Queue referenced shot from mention`.
- Clicking the action calls `onQueueSelectedGeneration(mentionTarget.mediaId, trimmedPrompt)`.
- The action records the same visible chat transcript/tool-call pattern as selected-source referenced generation.
- Audio mention targets do not show the action.
- Existing selected-source actions remain unchanged.

## Non-Goals

- No free-form command parser.
- No automatic media selection changes.
- No new project action type or Temporal workflow contract.
- No multi-reference generation payload.

## Testing

- `AgentPanel` shows and executes the mention referenced-generation action for a visual typed mention.
- `AgentPanel` hides the action for audio typed mentions.
- `EditorWorkspace` queues a Temporal-backed generated asset from a generated-output typed mention using the existing project action path.
