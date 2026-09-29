# Export Artifact Index Split Project Design

## Context

Palmier exposes export as a native editor workflow with MP4 variants and NLE XML handoff for
Premiere Pro and DaVinci Resolve. Video Creater already records `ProjectExportArtifact` entries in
`video-creater.project.json`, but split projects do not expose a focused export catalog for agents
or file-based tools. Agents must parse the full manifest before they can find the latest exported
MP4, MOV, WebM, or XML artifact.

Palmier reference: https://www.palmier.io/docs

## Goal

Write an agent-readable `exports/index.json` catalog whenever split projects are saved, without
changing the canonical export artifact model.

## Behavior

- `save_split_project` writes `exports/index.json`.
- The index includes `schemaVersion` and an `artifacts` array.
- Each entry includes `artifactId`, `kind`, `format`, `path`, `mimeType`, `jobId`, and
  `createdAt`.
- Entries are sorted by `createdAt` descending, then `artifactId` ascending for deterministic diffs
  and latest-export discovery.
- `kind` uses the same stable snake-case labels as the canonical artifact kind enum.
- `path` remains project-relative and points to the exported artifact, not the index entry.
- The index never includes credentials, full workflow requests, absolute project paths, render logs,
  or provider input payloads.
- `load_split_project` ignores `exports/index.json`; canonical export artifacts remain the manifest
  `exportArtifacts` array.
- `validate_split_project` ignores `exports/index.json` as catalog metadata and continues to
  validate the canonical split-project files.

## Non-Goals

- No `ProjectExportArtifact` schema change.
- No export job, Temporal workflow, ffmpeg, or NLE XML behavior change.
- No migration from manifest-backed export artifacts to per-export sidecars.
- No validation that exported files exist on disk in this slice.

## Verification

- Split project tests prove `exports/index.json` is written with export metadata sorted by recency.
- Split project tests prove loading and validating a project with `exports/index.json` does not
  create extra export artifacts or treat index metadata as canonical state.
