# Project Recent Export Artifacts Design

## Context

Palmier-style editing treats exports as part of the project workflow, not a one-off download.
Video Creater now writes NLE XML and media export artifact sidecars under `exports/`, records them
in split project files, and shows the latest export in the Project Timeline Inspector. When a
project has several exports, the right rail still hides the artifact history that agents and editors
need to inspect before handing files to an NLE or deciding whether another export is required.

## Goal

Show recent export artifacts in the project inspector so the export file workflow is visible without
opening `exports/index.json` by hand.

## Behavior

- Keep the existing `Latest export` region unchanged.
- Add a `Recent exports` region below it when export artifacts exist.
- Sort artifacts newest first by `createdAt` and show at most three entries.
- Each entry shows id, kind, format, path, job id, and created timestamp.
- When no export artifacts exist, omit the recent list and keep the existing latest-export empty
  state.

## Non-Goals

- No new export command, Temporal workflow, project schema field, or file mutation.
- No click-to-open behavior in this slice.
- No changes to export artifact validation or index generation.

## Verification

- `ProjectTimelineInspector` tests prove recent exports render newest first and are capped at three.
- Existing latest-export tests continue to pass.
