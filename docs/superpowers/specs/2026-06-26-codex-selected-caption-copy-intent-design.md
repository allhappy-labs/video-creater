# Codex Selected Caption Copy Intent

## Context

Palmier-style chat lets editors iterate on timeline media and timing without leaving the project,
and connected agents can adjust clips while seeing project context. Video Creater already has a
manual Caption Inspector and validated caption correction paths: transcript-backed captions use
`applyCaptionRepair`, while standalone caption text edits use `editCaptionText`. The Codex rail
does not pass selected captions into its selected timeline context, so explicit prompts such as
`Set the selected caption text to "Welcome back"` fall back to broad edit generation.

## Goal

Expose selected caption cues to Codex and let explicit selected-caption copy prompts update the
caption through the same caption correction path as the manual inspector.

## Behavior

- `EditorWorkspace` passes selected caption items into `AgentPanel` as selected timeline clip
  context, including current caption text.
- The route is only available for selected items with `kind: "caption"` and text-backed source
  content.
- Prompt recognition requires `text` or `copy`, plus `selected caption`, `current caption`,
  `this caption`, or `caption`.
- The new copy is parsed from quoted text after `to`, or from the remaining phrase after `to`.
- Blank parsed text is ignored.
- Locked selected tracks block the action.
- Pressing the primary composer action calls a selected-caption callback and records a
  `project_action` tool row.
- `EditorWorkspace` handles the callback by calling the existing `applyCaptionText` path, so
  transcript-backed cues still use `applyCaptionRepair` and standalone cues still use
  `editCaptionText`.

## Non-Goals

- No freeform caption selection by name.
- No caption timing parsing in this slice.
- No transcript word edits beyond the existing `applyCaptionRepair` behavior.
- No new Rust project action shape.

## Tests

- `AgentPanel` proves explicit selected caption copy prompts route through the primary composer,
  hide `Generate edit`, and record the project-action transcript.
- `EditorWorkspace` proves the prompt route writes the existing caption correction action for the
  selected cue.
