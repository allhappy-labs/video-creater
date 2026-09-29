# Render Report Index Drift Validation Design

## Context

Palmier presents export and timeline-native AI editing as project-aware workflows, and its agent
connection can see full project context while trimming, splitting, reordering, generating, and
placing media on the timeline. Video Creater is moving in the same direction with split,
text-file-editable project folders. Render reports already capture review-critical facts such as
duration, stream presence, failed checks, artifacts, and logs in canonical
`renders/<report-id>/report.json` sidecars. The derived `renders/index.json` catalog gives agents a
fast entry point, but stale catalog data can mislead agent decisions about which output passed
review and where artifacts live.

Palmier reference: https://www.palmier.io/docs

## Goal

Make `validate_split_project` report drift in `renders/index.json` while keeping individual render
report sidecars as the canonical source of truth.

## Behavior

- `load_split_project` continues to ignore `renders/index.json`.
- `validate_split_project` reads `renders/index.json` only when it exists.
- Invalid render index JSON is reported as a validation issue, not a hard load error.
- Each render index entry must have a non-empty `reportId`.
- Duplicate render index `reportId` values are reported.
- Each index entry must reference an existing `renders/<report-id>/report.json` sidecar.
- Each render sidecar must have a matching index entry.
- Entry fields must match the canonical sidecar values for `path`, `status`, `outputPath`,
  `durationSeconds`, `video`, `audio`, `checkCount`, `failedCheckCount`, `artifactCount`,
  `logPath`, and `createdAt`.
- Field mismatches tell the user to regenerate `renders/index.json` from render report sidecars.

## Non-Goals

- No render report schema change.
- No render execution, ffmpeg, Temporal, fal.ai, UI, or export artifact behavior change.
- No attempt to make `renders/index.json` canonical or to repair it automatically during
  validation.
- No validation of detailed check names beyond the existing per-report validation.

## Verification

- Split project tests prove validation reports stale render report index metadata.
- Split project tests prove validation reports a render sidecar missing from `renders/index.json`.
- Existing render report validation and load behavior stay unchanged.
