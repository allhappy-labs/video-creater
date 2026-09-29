# Generated Asset Index Split Project Design

## Context

Palmier's connected-agent workflow depends on agents seeing generated media as project-library
objects that can be inspected, regenerated, organized, and swapped without leaving the editor.
Video Creater already stores each generated asset as a text-editable
`generated/<asset-id>/asset.json` sidecar, and `media/index.json` stores generated output media.
Unlike templates, generated assets do not yet have a stable catalog entry point, so agents must
scan nested directories before they can understand the generated library.

Palmier reference: https://www.palmier.io/docs

## Goal

Write an agent-readable `generated/index.json` catalog whenever split projects are saved, without
changing runtime project semantics.

## Behavior

- `save_split_project` writes `generated/index.json`.
- The index includes `schemaVersion` and an `assets` array.
- Each entry includes `assetId`, `name`, `path`, `status`, `modelProvider`, `modelId`,
  `targetFolderId`, `placementIntent`, `outputCount`, and `createdAt`.
- Entries are sorted by `assetId` for deterministic diffs.
- `load_split_project` ignores `generated/index.json`; canonical generated assets remain the
  individual `generated/<asset-id>/asset.json` files.
- `validate_split_project` ignores `generated/index.json` as catalog metadata and continues to
  validate individual generated asset files.

## Non-Goals

- No generated asset schema change.
- No media index, timeline, Temporal, fal.ai, UI, or render behavior changes.
- No prompt duplication in the index; prompts remain in per-asset sidecars.

## Verification

- Split project tests prove `generated/index.json` is written with generated asset metadata and the
  project-relative sidecar path.
- Split project tests prove loading and validating a project with `generated/index.json` does not
  create an empty generated asset or report index metadata as an invalid generated sidecar.
