# Codex Selected Template Field Intent

## Context

Palmier-style editing lets a connected agent adjust project elements with full timeline context
while preserving an editor's manual control. Video Creater already has editable motion templates
through the Template Inspector and a validated `updateTemplateItems` project action. The gap is
that selected template fields are not exposed to the Codex rail, so explicit prompts such as
`Set the selected template headline to "Launch now"` become broad edit requests instead of direct
project actions.

## Goal

Expose selected timeline templates to Codex and let explicit selected-template field prompts update
one template text field through `updateTemplateItems`.

## Behavior

- `EditorWorkspace` passes selected motion templates into `AgentPanel` as selected timeline clip
  context, including the current template field values.
- Prompt recognition requires `selected template`, `current template`, `this template`, or
  `template`, plus a known field id or field label from the selected template.
- The new value is parsed from quoted text after `to`, or from the remaining phrase after `to`.
- Blank parsed values are ignored.
- Locked selected tracks block the action.
- Pressing the primary composer action calls a selected-template-field callback and records a
  `project_action` tool row.
- `EditorWorkspace` handles the callback by writing a single `updateTemplateItems` action that
  preserves the template's existing timing and other field values while replacing one field.

## Non-Goals

- No freeform template selection by name.
- No style, timing, visual-treatment, motion, safe-zone, or avoid prompt parsing in this slice.
- No multi-field prompt parsing.
- No new Rust project action shape.

## Tests

- `AgentPanel` proves explicit selected template field prompts route through the primary composer,
  hide `Generate edit`, and record the project-action transcript.
- `EditorWorkspace` proves the prompt route writes `updateTemplateItems` for the selected template
  with preserved timing and unchanged sibling fields.
