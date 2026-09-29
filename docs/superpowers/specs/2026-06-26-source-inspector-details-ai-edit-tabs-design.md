# Source Inspector Details AI Edit Tabs

## Context

Palmier's right source rail separates source metadata from generation edits with `Details`
and `AI Edit` tabs. Video Creater currently stacks `Generated details` and
`Generated AI edit` in one long generated-source inspector. That makes the rail taller
and exposes prompt-variation controls even when the editor is only checking file,
reference, workflow, or prompt provenance.

## Goal

Make generated sources in the right Source Inspector use a compact `Details` / `AI Edit`
tab row. The default view is `Details`; AI regeneration controls remain available one
click away without changing any generation action semantics.

## Requirements

- Generated Source Inspector content renders a tablist with `Details` and `AI Edit`.
- `Details` is selected by default and shows recipe summary, file metadata, references,
  workflow status, prompt, output replacement, insertion, and rerun actions.
- `AI Edit` shows the existing variation prompt, variation-set controls, replacement
  target, queue buttons, upscale action, and workflow route context.
- Switching tabs does not reset draft prompts, variation mode, or variation-set names
  while the selected generated source remains the same.
- Non-generated imported media keeps its existing inline AI edit affordance.
- No project schema, Temporal workflow, fal.ai provider, or project action contract changes.

## Non-Goals

- Do not redesign the outer right-rail `Source` / `Timeline` context switch.
- Do not change generated asset validation, generated output replacement, or timeline insertion.
- Do not add new generation providers or models.

## Verification

- Source Inspector tests assert the generated rail defaults to `Details`.
- Source Inspector tests assert `AI Edit` controls are hidden until the `AI Edit` tab is selected.
- Source Inspector tests assert switching tabs preserves the edited variation prompt.
- Browser QA confirms the right rail visually matches the Palmier-style `Details` / `AI Edit`
  source panel pattern without overlapping or clipping.
