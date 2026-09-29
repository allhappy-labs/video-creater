# Codex Selected Audio Fade Intent Design

## Context

Palmier-style agents can adjust clips directly, and Video Creater already exposes selected audio fade-out controls in the Source Inspector and Codex rail. Those controls call the validated `updateAudioFadeOut` project action and persist through split-project files. The missing parity is a Codex composer route for explicit prompts such as `Set the selected clip fade out to 1.5s.`

## Goal

Let Codex apply a numeric fade-out duration to the selected audio clip when the prompt is explicit, while preserving the existing Rust-validated `updateAudioFadeOut` action path.

## Behavior

- Recognize explicit prompts that mention fade out, refer to the selected/current clip, and include a numeric seconds value.
- When the selected clip is an editable `audio_clip` and the parsed value is `>= 0`, replace the composer primary action with `Set selected clip fade`.
- Pressing the action calls `onApplySelectedAudioFadeOut(itemId, fadeOutSeconds)`.
- The chat transcript records the action as a `project_action` tool call with the selected clip id and `set fade out to <value>s` detail.
- Locked tracks, non-audio clips, missing values, and negative values do not expose this primary action.

## Architecture

`AgentPanel` owns a lightweight prompt parser and routes valid fade prompts before generic EDL generation. `EditorWorkspace` already passes `applyAudioFadeOut` into the rail, so no new Rust command or project action is required.

## Tests

- `AgentPanel` proves an explicit selected audio fade prompt calls `onApplySelectedAudioFadeOut`, hides `Generate edit`, and records a project-action transcript row.
- `EditorWorkspace` proves the prompt route writes an `updateAudioFadeOut` split-project action for the selected audio clip.
