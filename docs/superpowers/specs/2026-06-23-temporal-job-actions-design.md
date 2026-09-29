# Temporal Job Actions Design

## Context

The Temporal workflow contract added workflow metadata to `JobSummary`, but project files still cannot record or update workflow-backed jobs through validated project actions. That leaves Temporal orchestration state outside the same text-file-editable project workflow used for timeline edits, generated assets, render reports, folders, transcripts, and templates.

Palmier-style agent control needs jobs to be inspectable and editable in project files while still flowing through a safe action contract. Render drafts, media generation, transcription, and Codex edit proposals should all be able to create durable job records before worker execution begins, then update status and Temporal run IDs as orchestration progresses.

## Goals

- Add validated project actions for recording and updating `JobSummary` entries.
- Preserve atomic action application: invalid job mutations must leave the project unchanged.
- Validate job IDs and workflow metadata enough to keep project files useful and deterministic.
- Expose typed frontend contracts for workflow-backed jobs and job actions.
- Keep this slice independent from a running Temporal server.

## Non-Goals

- No Temporal worker process is started in this slice.
- No live workflow client calls are added.
- No queue/status UI is changed in this slice.
- No generated asset or render report workflow is migrated yet.

## Action Contract

New project actions:

- `recordJob`: append a new `JobSummary`.
- `updateJobStatus`: update status, `updatedAt`, and optional Temporal `runId` for an existing job.

Validation rules:

- Job IDs must be non-empty safe path segments.
- Job kinds and `updatedAt` must be non-empty.
- Duplicate job IDs are rejected.
- Status updates must target an existing job.
- Workflow metadata, when present, must include non-empty workflow ID, workflow type, task queue, and at least one activity type.
- Empty activity type names are rejected.
- Empty run IDs are rejected when a status update attempts to set one.

## Acceptance Criteria

- Rust project action tests cover recording a workflow-backed job, updating job status/run ID, duplicate job rejection, and invalid workflow metadata.
- TypeScript project contracts represent job summaries, Temporal workflow metadata, and the two job actions.
- Focused Rust tests and frontend project contract tests pass.
