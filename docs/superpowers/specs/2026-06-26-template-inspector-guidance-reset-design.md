# Template Inspector Visual Guidance Reset

## Context

Video Creater now lets both Codex and manual editors change a motion template's
`visualTreatment`, `motion`, `safeZone`, and `avoid` guidance. That makes template overrides more
useful, but it also creates a common manual-editing need: an editor may experiment with Codex or
hand-written guidance and then want to return the selected template to the built-in catalog
guidance without reselecting or reinserting the template.

Palmier-style editing keeps these corrections inside the active editor surface. The reset belongs
beside the visual guidance fields in the right-side selected-item inspector, not in a modal or
project-level settings view.

## Goal

Add a compact reset control to the `TemplateInspector` visual guidance block that restores the four
guidance fields from the selected motion template defaults.

## Behavior

- The `Visual guidance` block shows a `Reset guidance` control.
- Clicking it replaces local `visualTreatment`, `motion`, `safeZone`, and `avoid` state with the
  selected built-in template defaults.
- The reset is local until the editor clicks `Apply template`.
- Applying after reset writes the default guidance values in the existing
  `TemplateInspectorUpdate.override`.
- Text fields, timing, and style color edits are not changed by the reset.
- The control is disabled when no selected template is available.

## UI Shape

- Place the control in the `Visual guidance` block header.
- Use a compact outline button with a reset/rotate icon and the label `Reset guidance`.
- Keep the block dense; do not add nested cards, dialogs, or explanatory copy.

## Non-Goals

- No reset for template text fields, timing, or color style.
- No persisted history, undo stack, or default override deletion.
- No project action or backend schema changes.
- No Codex prompt parsing changes.

## Verification

- Add a `TemplateInspector` test proving `Reset guidance` restores all four metadata fields to
  template defaults while leaving a changed headline and accent color intact.
- Add a test proving Apply after reset sends the default guidance values in the override.
- Run the focused `TemplateInspector` tests and `pnpm lint`.
