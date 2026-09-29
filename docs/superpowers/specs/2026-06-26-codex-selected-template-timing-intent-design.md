# Codex Selected Template Timing Intent

## Context

Palmier-style editing lets chat iterate on timing without leaving the project. Video Creater
already lets users edit selected motion template start and duration in the Template Inspector, and
that manual path writes through the text-file-editable `updateTemplateItems` project action. The
remaining gap is that explicit Codex timing prompts for a selected template can fall back to broad
edit generation or generic clip trimming instead of the template-aware action.

## Goal

Let Codex update a selected motion template's timeline start and duration through
`updateTemplateItems`.

## Behavior

- `AgentPanel` recognizes explicit selected-template timing prompts such as
  `Set the selected template start to 1.5s and duration to 3s`.
- The route requires `selected template`, `current template`, `this template`, or `template`, plus
  a timing word such as `start`, `duration`, `length`, `timing`, `move`, or `place`.
- Parsed values are seconds. `startSeconds` must be finite and non-negative. `durationSeconds` must
  be finite and greater than zero.
- Prompts may update start, duration, or both. Missing values preserve the selected template's
  current value.
- Locked selected tracks block the action.
- Pressing the primary composer action calls a selected-template-timing callback and records a
  `project_action` tool row.
- `EditorWorkspace` handles the callback by writing a single `updateTemplateItems` action that
  preserves existing template fields while replacing timing.

## Non-Goals

- No template selection by name.
- No timecode parsing in this slice.
- No visual style, field, motion, safe-zone, or avoid parsing in this slice.
- No new Rust project action shape.

## Tests

- `AgentPanel` proves explicit selected-template timing prompts route through the primary composer,
  hide `Generate edit`, and record the project-action transcript.
- `EditorWorkspace` proves the prompt route writes `updateTemplateItems` for the selected template
  with updated timing and preserved fields.
