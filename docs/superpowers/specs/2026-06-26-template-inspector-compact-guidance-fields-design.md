# Template Inspector Compact Guidance Fields

## Context

The template inspector now exposes `visualTreatment`, `motion`, `safeZone`, and `avoid` as editable
visual guidance fields. The design goal for those fields was a dense right-rail block with
two-row textareas. The shared `Textarea` component defaults to a large minimum height, so `rows={2}`
does not actually produce compact fields. Four tall textareas make the selected-item inspector feel
heavier than the Palmier-style right rail shown in the reference screenshots.

## Goal

Make the visual guidance controls in `TemplateInspector` visually compact while preserving readable
multi-line editing.

## Behavior

- The four guidance textareas use a smaller minimum height than the shared textarea default.
- The fields remain full-width, keyboard-focusable, accessible by label, and controlled by the same
  local state.
- Existing Apply, validation, reset, and override payload behavior remain unchanged.

## Non-Goals

- No component-wide `Textarea` style change.
- No project action, schema, template catalog, or Codex prompt changes.
- No hiding or collapsing of the guidance fields.

## Verification

- Add a focused `TemplateInspector` test proving each guidance textarea gets the compact class.
- Existing `TemplateInspector` tests continue to pass.
- Run `pnpm lint`.
