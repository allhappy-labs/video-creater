# Export Artifact Sidecars Split Project Design

## Context

Palmier treats export as part of the editable project workflow: editors can export MP4 variants or
NLE XML, and connected agents can see enough project context to decide what to generate, adjust, or
handoff. Video Creater now writes `exports/index.json` for discovery, but export artifacts
themselves are still only editable inside `video-creater.project.json`. That differs from the rest
of the split-project model, where transcripts, templates, generated assets, and render reports have
focused sidecar files that agents can inspect and patch without rewriting the full manifest.

Palmier reference: https://www.palmier.io/docs

## Goal

Persist export artifact metadata as per-artifact sidecars while preserving manifest
`exportArtifacts` as a compatibility mirror.

## Behavior

- `save_split_project` writes each export artifact to `exports/<artifact-id>/artifact.json`.
- `save_split_project` keeps writing `exports/index.json`; index entries continue to point to the
  exported media/XML artifact path, not the metadata sidecar.
- The manifest continues to include `exportArtifacts` so older project readers retain export
  history.
- `load_split_project` reads export sidecars when sidecar files exist, sorted by path for
  deterministic ordering; malformed sidecar JSON fails load like other split-project sidecars.
- `load_split_project` falls back to manifest `exportArtifacts` when no export sidecars exist,
  preserving projects saved before this sidecar split.
- `load_split_project` ignores `exports/index.json` and non-sidecar exported files such as `.mp4`,
  `.mov`, `.webm`, and `.xml`.
- Stale managed export sidecars are removed on save when their artifact id is no longer in the
  project. Exported media/XML files are never deleted by this cleanup.

## Validation

- `validate_split_project` should continue validating the canonical split-project files and ignore
  `exports/index.json`.
- Sidecar validation can be added in a later slice. This slice focuses on save/load semantics and
  stale sidecar cleanup.

## Non-Goals

- No `ProjectExportArtifact` schema change.
- No export render, Temporal workflow, ffmpeg, or NLE XML generation change.
- No deletion of exported media/XML artifacts.
- No change to the export index schema.

## Verification

- Split project tests prove export sidecars are written and included in the write report.
- Split project tests prove loading prefers sidecar metadata over stale manifest export metadata.
- Split project tests prove loading still works for manifest-only export history.
- Split project tests prove stale export sidecar metadata is removed without deleting exported
  artifact files.
