# Template Inspector Visual Metadata Editing

## Context

Palmier-style editing keeps generation, manual timeline work, and agent edits in the same project
surface. Palmier's docs describe timeline-native AI edits, clip inspection, prompt/reference review,
and agent actions that can trim, split, reorder, adjust, rerun, and place media directly on the
timeline. The supplied screenshots show a dense editor layout with the media/chat rail, preview,
right inspector, and timeline visible together.

Video Creater now lets Codex update selected motion template `visualTreatment`, `motion`,
`safeZone`, and `avoid` metadata through `updateTemplateOverride`, but the manual
`TemplateInspector` still exposes only timing, text fields, and color style. When an editor applies a
manual template change, the inspector writes the built-in template metadata back into the override,
which can overwrite metadata that Codex previously refined.

## Goal

Expose selected motion template visual metadata in the manual `TemplateInspector` so editors can
inspect and refine the same guidance fields that agents can edit.

## Behavior

- The template inspector shows editable controls for:
  - `visualTreatment`
  - `motion`
  - `safeZone`
  - `avoid`
- Each control starts from the selected item's existing property when present, then falls back to
  the built-in motion template default.
- Applying a template edit writes one `TemplateInspectorUpdate` whose `override` includes:
  - current template fields
  - current timing
  - current style colors
  - the edited visual metadata values
- Existing Codex-authored metadata is preserved when the editor changes only text, timing, or color.
- Empty visual metadata values disable Apply. These fields are production guidance, not optional
  cosmetic notes, and downstream render/proposal validators already treat them as required.
- Reset behavior remains selection-driven: selecting a different template item reloads metadata from
  that item's properties/defaults.

## UI Shape

Add a compact `Visual guidance` block below the color style controls and above `Apply template`.
Use textareas because these values are descriptive phrases, not scalar settings. Keep labels short
and editor-facing:

- `Visual treatment`
- `Motion`
- `Safe zone`
- `Avoid`

The block should stay dense enough for the right rail: two-row textareas are sufficient by default,
with normal textarea resizing left to the browser component behavior.

## Data Flow

`TemplateInspector` should add local state for the four metadata fields:

1. Read `item.properties[key]` when it is a non-empty string.
2. Fall back to the selected motion template default.
3. Include trimmed metadata values in `override` on Apply.

No new Rust action is needed. The existing `updateTemplateOverride` action already carries the
metadata keys and persists them in split-project template override files.

## Non-Goals

- No new Codex prompt parsing in this slice.
- No motion preset picker or conversion from prose to preset IDs.
- No template library redesign.
- No new project action type or Rust schema shape.
- No visual QA scoring changes.

## Tests

- Update `TemplateInspector` tests to prove edited visual metadata values are sent in the
  `override`.
- Add a regression test proving existing item metadata is preserved when only a normal text/color
  edit is applied.
- Add a validation test proving Apply is disabled when a required visual metadata field is empty.
- Run the focused `TemplateInspector` test file and TypeScript lint.

## Implementation Notes

- Prefer small helpers parallel to `getStyleValue`, for example `getMetadataValue`.
- Keep the inspector API unchanged if possible; the existing `TemplateInspectorUpdate.override`
  carries all required fields.
- Avoid a nested card. The right rail already frames the inspector, so this should be a simple
  labeled block inside the existing card content.
