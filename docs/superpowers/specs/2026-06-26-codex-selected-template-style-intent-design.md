# Codex Selected Template Style Intent

## Context

Palmier-style editing keeps visual iteration close to the timeline: users can select generated or
manual elements, inspect the prompt and references, then adjust the look without leaving the edit.
Video Creater already lets users edit motion template colors in the Template Inspector. That manual
path writes a project-level `updateTemplateOverride`, which is persisted in split project files and
included in Codex project context. The gap is that explicit Codex prompts for selected template
colors still fall back to broad edit generation.

## Goal

Let Codex update selected motion template color overrides through `updateTemplateOverride`.

## Behavior

- `AgentPanel` recognizes explicit selected-template style prompts such as
  `Set the selected template accent color to #38bdf8`.
- Supported style keys are `accentColor`, `backgroundColor`, and `textColor`.
- Prompt recognition requires `selected template`, `current template`, `this template`, or
  `template`, plus a supported color field and a non-empty value after `to`.
- Locked selected tracks block the action.
- Pressing the primary composer action calls a selected-template-style callback and records a
  `project_action` tool row.
- `EditorWorkspace` handles the callback by writing a single `updateTemplateOverride` action that
  preserves the selected template's existing fields, existing/default style values, and visual
  metadata while replacing one style key.

## Non-Goals

- No color picker UI changes in this slice.
- No natural-language color conversion, palette generation, or contrast checks.
- No freeform template selection by name.
- No timing, template field, visual-treatment, motion, safe-zone, or avoid parsing.
- No new Rust project action shape.

## Tests

- `AgentPanel` proves explicit selected-template style prompts route through the primary composer,
  hide `Generate edit`, and record the project-action transcript.
- `EditorWorkspace` proves the prompt route writes `updateTemplateOverride` for the selected
  template with updated style and preserved fields/metadata.
