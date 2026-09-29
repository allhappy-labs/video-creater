# Codex Selected Template Visual Metadata Intent

## Context

Palmier-style editing lets an agent refine the selected timeline element's look, motion, and
constraints while the editor stays in context. Video Creater already stores motion template
`visualTreatment`, `motion`, `safeZone`, and `avoid` metadata in template overrides, and Codex
project context exposes those overrides to agents. The gap is that explicit selected-template
prompts for these visual guidance fields still fall back to broad edit generation.

## Goal

Let Codex update selected motion template visual metadata through `updateTemplateOverride`.

## Behavior

- `AgentPanel` recognizes explicit selected-template metadata prompts such as
  `Set the selected template motion to "snap in, hold, and drift out"`.
- Supported keys are `visualTreatment`, `motion`, `safeZone`, and `avoid`.
- Prompt recognition requires `selected template`, `current template`, `this template`, or
  `template`, plus a supported key and a non-empty value after `to`.
- Locked selected tracks block the action.
- Pressing the primary composer action calls a selected-template-metadata callback and records a
  `project_action` tool row.
- `EditorWorkspace` handles the callback by writing a single `updateTemplateOverride` action that
  preserves the selected template's existing/default fields, style, and other visual metadata while
  replacing one metadata key.

## Non-Goals

- No freeform template selection by name.
- No multi-key prompt parsing.
- No natural-language conversion into motion preset IDs.
- No new manual inspector fields in this slice.
- No new Rust project action shape.

## Tests

- `AgentPanel` proves explicit selected-template metadata prompts route through the primary
  composer, hide `Generate edit`, and record the project-action transcript.
- `EditorWorkspace` proves the prompt route writes `updateTemplateOverride` for the selected
  template with updated metadata and preserved fields/style.
