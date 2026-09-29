# Template Index Split Project Design

## Context

Palmier's connected-agent workflow depends on agents seeing the same project context as the editor:
timeline, media, generated assets, and reusable template choices. Video Creater already stores
template overrides as text-editable `templates/<template-id>.json` files in split projects, but the
templates directory has no stable catalog entry point. Agents must either scan every JSON file or
infer template availability from timeline items.

## Goal

Write an agent-readable `templates/index.json` catalog whenever split projects are saved, without
changing runtime project semantics.

## Behavior

- `save_split_project` writes `templates/index.json`.
- The index includes `schemaVersion` and a `templates` array.
- Each entry includes `templateId`, `name`, `path`, `visualTreatment`, `motion`, `safeZone`, and
  `avoid` for each project template override.
- Entries are sorted by `templateId` for deterministic diffs.
- `load_split_project` ignores `templates/index.json`; canonical template overrides remain the
  individual `templates/<template-id>.json` files.
- `validate_split_project` ignores `templates/index.json` as catalog metadata and continues to
  validate individual template override files.

## Non-Goals

- No built-in template catalog export in this slice.
- No schema change to `ProjectTemplateOverride`.
- No UI changes, render changes, or template runtime behavior changes.

## Verification

- Split project tests prove `templates/index.json` is written with template metadata and the
  project-relative override path.
- Split project tests prove loading and validating a project with `templates/index.json` does not
  create an empty template override or report index metadata as an invalid override.
