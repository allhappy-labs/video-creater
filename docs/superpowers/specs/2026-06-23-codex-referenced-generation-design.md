# Codex Referenced Generation

## Context

Palmier documents a chat workflow where the assistant can generate images, video, and audio in context, use `@` references to match a look or frame, and place generated assets on the timeline. Video Creater already has:

- a Codex panel with project and selected-source context
- a media-generation project action via `recordGeneratedAsset`
- selected-source provenance in the Codex panel
- insertion and variation actions for completed generated outputs

The missing bridge is letting the Codex prompt queue a new generated shot from the selected visual source without switching to the media generation composer.

## Goal

Add a small Codex selected-source action that queues a referenced generated video request using the current Codex prompt and selected visual media.

## Behavior

- When the selected Codex source is visual media, the selected-source block shows `Queue referenced shot`.
- The button is disabled until the Codex prompt contains non-whitespace text.
- Clicking the button trims the Codex prompt and queues a `recordGeneratedAsset` action.
- The generated asset uses the selected media id as both a style/reference media id and first-frame media id.
- The generated request uses the same default video settings as the media generation composer: `fal.ai/fal-ai/wan-25-preview/text-to-video`, 4 seconds, 16:9, 1280x720, 24 fps.
- Audio selections do not show this action.
- Existing selected generated output actions remain available.

## Non-Goals

- No automatic timeline insertion in this slice.
- No multi-reference parsing from prompt text yet.
- No model picker inside the Codex panel.
- No backend generation execution changes.

## Data Flow

1. `EditorWorkspace` marks selected visual media as usable for referenced generation in `AgentSelectedMediaContext`.
2. `AgentPanel` receives `onQueueSelectedGeneration(mediaId, prompt)`.
3. The selected-source block renders `Queue referenced shot` when visual media and callback are present.
4. Clicking the button calls the callback with the selected media id and trimmed Codex prompt.
5. `EditorWorkspace` converts that into a `recordGeneratedAsset` project action using existing generated asset defaults and selected media references.

## Tests

- `AgentPanel` calls the referenced-generation callback with the selected media id and trimmed prompt.
- `EditorWorkspace` queues a generated asset from the Codex selected-source context with selected media references and default video settings.
- Audio selected sources do not expose the referenced-generation action.
