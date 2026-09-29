# Export Split Project Gate Design

## Context

Palmier exposes direct media exports and NLE XML handoff from a durable project timeline. Video
Creater already has local WebM render actions, NLE XML export commands, and MP4-family Temporal
workflow start requests. The remaining UI contract problem is that available MP4-family profiles can
look actionable without proving the project is saved as a schema-v2 split project folder.

Schema-v2 split projects are the text-file-editable format that gives workers stable project-relative
paths for `exports/`, workflow job records, render reports, and export artifacts. Temporal media
exports should not start from an unsaved or legacy single-file project state.

## Goal

Keep MP4-family export actions disabled until both conditions are true:

- Rust reports the selected export profile as available.
- The current project is a saved schema-v2 split project folder.

## Non-Goals

- Do not enable MP4/H.264, MP4/H.265, or ProRes MOV by default.
- Do not add a codec runtime or approve encoder policy.
- Do not change WebM render behavior.
- Do not change the existing NLE XML command implementation.

## UI Contract

- Disabled MP4 profiles continue to show the Rust policy reason when the profile is unavailable.
- Available MP4 profiles without a schema-v2 split project folder show a save/split-project reason,
  not `Start Temporal export workflow`.
- The shared export menu callout names both NLE XML and Temporal media exports as split-project
  gated workflows.
- Enabled MP4 profiles start the existing Temporal export workflow request path.

## Acceptance Criteria

- A schema-v1 project with a project directory does not enable MP4-family export buttons.
- An unsaved project does not enable MP4-family export buttons.
- A schema-v2 split project with an available profile still queues the existing Temporal export
  request.
- No credentials or provider secrets are written into tests, source, logs, or specs.
