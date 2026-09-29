# Export Artifact Sidecar Validation Design

## Context

Video Creater now persists export metadata to `exports/<artifact-id>/artifact.json`, keeps
`exports/index.json` as an agent-readable discovery catalog, and mirrors export history in manifest
`exportArtifacts` for compatibility. The next gap is validation: an edited export sidecar can drift
into invalid ids, paths, or formats without `validate_split_project` reporting it.

Palmier reference: https://www.palmier.io/docs

## Goal

Validate export artifact sidecars as the canonical export metadata source when sidecars exist, with
manifest fallback for older projects.

## Behavior

- `validate_split_project` reads `exports/<artifact-id>/artifact.json` files when any export
  sidecars exist.
- If no sidecars exist, validation checks manifest `exportArtifacts`.
- `exports/index.json` remains ignored as derived metadata.
- Sidecar malformed JSON is reported as a validation issue rather than failing the whole validation
  command.
- Each export artifact must have schema version 1, a non-empty safe `id`, non-empty `format`,
  non-empty project-relative `path`, non-empty `mimeType`, and non-empty `createdAt`.
- Export artifact ids must be unique, and a sidecar artifact id must match its containing directory.
- Export artifact paths must stay inside the project folder.
- Optional `jobId` must not be an empty string when present.

## Non-Goals

- No `ProjectExportArtifact` schema change.
- No export render, Temporal workflow, ffmpeg, or NLE XML generation change.
- No validation of `exports/index.json` contents.
- No requirement that exported MP4/XML files physically exist on disk in this slice.

## Verification

- Split project tests prove invalid sidecar ids are reported at the sidecar path.
- Split project tests prove export path escapes in sidecars are reported.
- Split project tests prove validation falls back to manifest export artifacts when no sidecars
  exist.
