# Workflow Job Sidecars Split Project Design

## Context

Palmier's connected-agent workflow depends on agents seeing full project context and making native
timeline or generation edits. Video Creater already records Temporal-shaped `JobSummary` entries in
`video-creater.project.json` and writes `jobs/index.json` for safe queue discovery, but the complete
workflow metadata and start request remain embedded in the manifest. That makes durable workflow
state harder to inspect, patch, or hand to a worker without rewriting unrelated project metadata.

Temporal Rust reference: https://docs.temporal.io/develop/rust
Palmier reference: https://www.palmier.io/docs

## Goal

Persist workflow job metadata as per-job sidecars while preserving manifest `jobs` as a
compatibility mirror.

## Behavior

- `save_split_project` writes each job to `jobs/<job-id>/job.json`.
- `save_split_project` keeps writing `jobs/index.json` as the bounded, safe discovery catalog.
- The manifest continues to include `jobs` so older project readers retain workflow queue state.
- `load_split_project` reads job sidecars when sidecar files exist, sorted by path for deterministic
  ordering; malformed sidecar JSON fails load like other split-project sidecars.
- `load_split_project` falls back to manifest `jobs` when no sidecars exist, preserving projects
  saved before this sidecar split.
- `load_split_project` ignores `jobs/index.json`.
- Stale managed job sidecars are removed on save when their job id is no longer in the project.

## Data Boundary

- `jobs/index.json` remains safe for broad agent context and excludes full workflow input, search
  attributes, absolute project directory values, and provider request payloads.
- `jobs/<job-id>/job.json` stores the full `JobSummary`, including optional `startRequest`, because
  it is the editable workflow contract used by the project and worker boundary.
- Provider credentials remain indirect: start requests may name an environment variable such as
  `FAL_KEY`, but must not contain credential values.

## Validation

- `validate_split_project` continues validating manifest-backed jobs in this slice.
- Sidecar job validation can be added later so edited job sidecars get the same duplicate-id,
  safe-id, workflow metadata, and start-request checks.

## Non-Goals

- No `JobSummary`, `TemporalWorkflowMetadata`, or `TemporalWorkflowStartRequest` schema change.
- No Temporal client, worker, polling, cancellation, retry, or provider behavior change.
- No change to the workflow job index schema.

## Verification

- Split project tests prove job sidecars are written and included in the write report.
- Split project tests prove loading prefers sidecar metadata over stale manifest job metadata.
- Split project tests prove loading still works for manifest-only jobs.
- Split project tests prove stale job sidecars are removed when jobs are no longer present.
