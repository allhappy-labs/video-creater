# Codex Selected Clip Delete Intent Design

## Context

Palmier-style editing expects clip operations to be available both manually and through the contextual agent. Video Creater already has a validated manual delete path: the timeline toolbar calls `removeTimelineItem`, which writes a `removeItems` project action through Rust-owned project validation and split-project persistence. The Codex rail can now route selected-clip split, trim, and reorder prompts through validated actions, but it cannot yet delete the selected clip from an explicit chat prompt.

## Goal

Let Codex delete the selected timeline clip when the prompt is an explicit selected-clip removal request, without generating a broad EDL proposal and without mutating canonical files directly from React.

## Behavior

- Recognize explicit prompts that contain a delete/remove verb and refer to the selected clip, such as `Delete the selected clip.` or `Remove this clip from the timeline.`
- When the selected clip is editable, replace the composer primary action label with `Delete selected clip`.
- Pressing the action calls a new `onRemoveSelectedTimelineClip(itemId)` callback.
- The chat transcript records the action as a `project_action` tool call with the selected clip id and `delete clip` detail.
- Locked-track selected clips do not expose this primary action.
- Ambiguous prompts that do not mention a clip continue to use the normal EDL generation path.

## Architecture

`AgentPanel` owns the lightweight prompt-intent parser and primary-action routing. `EditorWorkspace` passes the existing `removeTimelineItem` operation into the rail as `onRemoveSelectedTimelineClip`, preserving the established `removeItems` project action path and split-project persistence. Rust remains the owner of canonical project validation.

## Tests

- `AgentPanel` proves an explicit selected-clip delete prompt routes to `onRemoveSelectedTimelineClip`, hides the generic `Generate edit` action, and records a project-action transcript row.
- `EditorWorkspace` proves the prompt route submits a split-project `removeItems` action for the selected generated timeline clip.
