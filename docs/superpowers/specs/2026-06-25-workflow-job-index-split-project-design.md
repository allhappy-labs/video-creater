# Workflow Job Index Split Project Design

## Context

Palmier treats generation, export, and agent work as project-visible activity that editors can
monitor while continuing to use the timeline. Video Creater already records Temporal-shaped
`JobSummary` entries in `video-creater.project.json` and shows them in the editor workflow queue,
but split projects have no focused queue catalog for connected agents. Agents must parse the full
manifest before they can inspect queued, running, failed, or completed work.

Temporal Rust reference: https://docs.temporal.io/develop/rust
Palmier reference: https://www.palmier.io/docs

## Goal

Write an agent-readable `jobs/index.json` catalog whenever split projects are saved, without
changing runtime project semantics.

## Behavior

- `save_split_project` writes `jobs/index.json`.
- The index includes `schemaVersion` and a `jobs` array.
- Each entry includes `jobId`, `kind`, `status`, `updatedAt`, `workflowId`, `workflowType`,
  `taskQueue`, `runId`, `activityCount`, `hasStartRequest`, `startRequestReady`,
  `idReusePolicy`, and `credentialEnvVar`.
- Entries are sorted by `updatedAt` descending, then `jobId` ascending for deterministic diffs and
  recency scanning.
- `startRequestReady` is true only when the persisted start request matches the workflow id,
  workflow type, task queue, and activity list recorded on the job.
- `credentialEnvVar` may include the environment variable name such as `FAL_KEY`, but the index
  never includes provider credentials, full workflow input, project directory paths, search
  attributes, or request JSON.
- `load_split_project` ignores `jobs/index.json`; canonical jobs remain the manifest `jobs` array.
- `validate_split_project` ignores `jobs/index.json` as catalog metadata and continues to validate
  manifest job records.

## Non-Goals

- No `JobSummary` schema change.
- No Temporal client, worker, polling, cancellation, or retry behavior change.
- No provider credential storage.
- No migration from manifest-backed jobs to per-job sidecars in this slice.

## Verification

- Split project tests prove `jobs/index.json` is written with queue metadata and no start-request
  input or secret-bearing fields.
- Split project tests prove loading and validating a project with `jobs/index.json` does not create
  an extra job or treat index metadata as canonical job state.
