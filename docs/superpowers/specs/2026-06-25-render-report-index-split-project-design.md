# Render Report Index Split Project Design

## Context

Palmier keeps render and export outputs project-linked so editors and connected agents can inspect
what was produced, whether validation passed, and where durable artifacts live. Video Creater
already stores render-review reports as text-editable `renders/<render-id>/report.json` sidecars,
but the renders directory has no stable catalog entry point. Agents must scan nested directories
before they can summarize recent renders or find the latest reviewed artifact.

Palmier reference: https://www.palmier.io/docs

## Goal

Write an agent-readable `renders/index.json` catalog whenever split projects are saved, without
changing runtime project semantics.

## Behavior

- `save_split_project` writes `renders/index.json`.
- The index includes `schemaVersion` and a `reports` array.
- Each entry includes `reportId`, `path`, `status`, `outputPath`, `durationSeconds`, `video`,
  `audio`, `checkCount`, `failedCheckCount`, `artifactCount`, `logPath`, and `createdAt`.
- Entries are sorted by `reportId` for deterministic diffs.
- `load_split_project` ignores `renders/index.json`; canonical render reports remain the
  individual `renders/<render-id>/report.json` files.
- `validate_split_project` ignores `renders/index.json` as catalog metadata and continues to
  validate individual render report files.

## Non-Goals

- No render report schema change.
- No export artifact schema change.
- No renderer, Temporal workflow, fal.ai provider, or UI behavior changes.
- No duplication of full check details; detailed checks remain in per-report sidecars.

## Verification

- Split project tests prove `renders/index.json` is written with render report metadata and the
  project-relative sidecar path.
- Split project tests prove loading and validating a project with `renders/index.json` does not
  create an empty render report or report index metadata as an invalid render sidecar.
