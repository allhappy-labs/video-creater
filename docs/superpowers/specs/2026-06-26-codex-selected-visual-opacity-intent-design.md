# Codex Selected Visual Opacity Intent Design

## Context

Palmier-style agents can adjust clips while seeing the project context. Video Creater already has a validated `updateVisualClipOpacity` action plus Source Inspector and Codex rail controls for selected visual clips. The missing parity is a Codex composer route for explicit prompts such as `Set the selected clip opacity to 0.4.`

## Goal

Let Codex apply a numeric opacity value to the selected visual clip when the prompt is explicit, while preserving the existing Rust-validated `updateVisualClipOpacity` action path.

## Behavior

- Recognize explicit prompts that mention opacity, refer to the selected/current clip, and include a numeric value.
- When the selected clip is a visual opacity item (`video_clip`, `overlay`, or `hyperframe_scene`) and the parsed value is between `0` and `1`, replace the composer primary action with `Set selected clip opacity`.
- Pressing the action calls `onApplySelectedVisualOpacity(itemId, opacity)`.
- The chat transcript records the action as a `project_action` tool call with the selected clip id and `set opacity to <value>` detail.
- Locked tracks, audio clips, captions, missing values, and out-of-range values do not expose this primary action.
- This slice supports decimal opacity values only; blank reset remains available through the manual rail control.

## Architecture

`AgentPanel` owns a lightweight prompt parser and routes valid opacity prompts before generic EDL generation. `EditorWorkspace` already passes `applyVisualClipOpacity` into the rail, so no new Rust command or project action is required.

## Tests

- `AgentPanel` proves an explicit selected visual opacity prompt calls `onApplySelectedVisualOpacity`, hides `Generate edit`, and records a project-action transcript row.
- `EditorWorkspace` proves the prompt route writes an `updateVisualClipOpacity` split-project action for the selected visual clip.
