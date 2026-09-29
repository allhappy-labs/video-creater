# Template Index Drift Validation Design

## Context

Palmier's connected-agent workflow depends on agents seeing reusable visual choices and timeline
assets in the same project context as the editor. Video Creater stores project template overrides as
text-editable `templates/<template-id>.json` sidecars and writes a bounded `templates/index.json`
catalog for agent discovery. Template overrides carry the visual contract for lower thirds, title
cards, overlays, and other graphics through `visualTreatment`, `motion`, `safeZone`, and `avoid`.
If the index drifts after manual edits, agents can choose or describe stale visual treatments.

Palmier reference: https://www.palmier.io/docs

## Goal

Report stale `templates/index.json` entries during split-project validation while keeping
`templates/<template-id>.json` sidecars as the canonical template override source.

## Behavior

- `load_split_project` continues to ignore `templates/index.json`.
- `validate_split_project` reads `templates/index.json` only when the file exists and is valid JSON.
- Invalid `templates/index.json` JSON is reported as a validation issue, not a hard validation
  error.
- Each index entry must match an existing template override sidecar by `templateId`.
- Each index entry's `name`, `path`, `visualTreatment`, `motion`, `safeZone`, and `avoid` must
  match the referenced sidecar.
- Each template override sidecar must have a matching index entry when `templates/index.json`
  exists.
- Duplicate index `templateId` values are reported.

## Non-Goals

- No built-in template catalog export in this slice.
- No schema change to `ProjectTemplateOverride`.
- No UI changes, render changes, or template runtime behavior changes.

## Verification

- Split project tests prove a stale template index `visualTreatment` is reported with a fix that
  tells the agent to regenerate `templates/index.json`.
- Split project tests prove missing sidecar template overrides in the index are reported.
- Existing split-project tests continue to prove valid template override sidecars pass validation.
