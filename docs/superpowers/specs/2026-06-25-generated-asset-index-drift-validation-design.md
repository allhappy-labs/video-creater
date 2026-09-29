# Generated Asset Index Drift Validation Design

## Context

Palmier's connected-agent workflow depends on generated media being easy to inspect, rerun, and
place on the timeline from project context. Video Creater now writes
`generated/<asset-id>/asset.json` sidecars plus a derived `generated/index.json` catalog for agents.
The index is deliberately not canonical, but it is the first file an agent will read when deciding
which generated clips exist. If it drifts after manual edits, the project can validate while still
handing agents a stale map.

Palmier reference: https://www.palmier.io/docs

## Goal

Report stale `generated/index.json` entries during split-project validation while keeping generated
asset sidecars as the canonical runtime source.

## Behavior

- `load_split_project` continues to ignore `generated/index.json`.
- `validate_split_project` reads `generated/index.json` only when the file exists and is valid JSON.
- Invalid `generated/index.json` JSON is reported as a validation issue, not a hard validation
  error.
- Each index entry must point to an existing generated asset sidecar path.
- Each index entry's `assetId`, `status`, `modelProvider`, `modelId`, `targetFolderId`,
  `placementIntent`, `outputCount`, and `createdAt` must match the referenced sidecar.
- Each generated asset sidecar must have a matching index entry when `generated/index.json` exists.
- Duplicate index `assetId` values are reported.

## Non-Goals

- No generated asset schema change.
- No runtime loading semantics change.
- No requirement that output media files physically exist beyond the existing sidecar/media-index
  consistency checks.
- No prompt duplication in the index.

## Verification

- Split project tests prove a stale generated index status is reported with a fix that tells the
  agent to regenerate `generated/index.json`.
- Split project tests prove missing generated sidecar entries in the index are reported.
- Existing split-project tests continue to prove valid projects pass validation.
