# Codex Selected Audio Volume Intent Design

## Context

Palmier's agent contract includes clip adjustments, not only structural timeline edits. Video Creater already lets a human adjust a selected audio clip's gain from the Source Inspector and Codex rail. Those controls call `updateAudioVolume`, which is persisted through the split-project project-action path. The missing parity is an explicit Codex composer route for prompts like `Set the selected clip volume to -6 dB.`

## Goal

Let Codex apply a numeric dB gain to the selected audio clip when the prompt is explicit, while preserving the existing Rust-validated `updateAudioVolume` action path.

## Behavior

- Recognize explicit prompts that mention volume or gain, refer to the selected/current clip, and include a numeric dB value.
- When the selected clip is an editable `audio_clip` and the parsed value is between `-60` and `24`, replace the composer primary action with `Set selected clip volume`.
- Pressing the action calls `onApplySelectedAudioVolume(itemId, volumeDb)`.
- The chat transcript records the action as a `project_action` tool call with the selected clip id and `set volume to <value>dB` detail.
- Locked tracks, non-audio clips, missing values, and out-of-range values do not expose this primary action.
- This slice supports numeric dB values only; blank reset remains available through the manual rail control.

## Architecture

`AgentPanel` owns a lightweight prompt parser and routes valid prompts before generic EDL generation. `EditorWorkspace` already passes `applyAudioVolume` into the rail, so no new project action or Rust mutation path is required. The existing split-project persistence and validation keep canonical project files authoritative.

## Tests

- `AgentPanel` proves an explicit selected audio volume prompt calls `onApplySelectedAudioVolume`, hides `Generate edit`, and records a project-action transcript row.
- `EditorWorkspace` proves the prompt route writes an `updateAudioVolume` split-project action for the selected audio clip.
