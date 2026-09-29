# Workflow Job Sidecar Validation Design

## Context

Video Creater now persists Temporal workflow jobs to `jobs/<job-id>/job.json` while keeping
`jobs/index.json` as a safe discovery catalog and manifest `jobs` as a compatibility mirror. The
next gap is validation: `validate_split_project` still checks only manifest-backed jobs, so an
agent-edited job sidecar can drift into invalid workflow metadata without a validation issue.

Temporal Rust reference: https://docs.temporal.io/develop/rust
Palmier reference: https://www.palmier.io/docs

## Goal

Validate workflow job sidecars as the canonical job source when sidecars exist, with manifest
fallback for older projects.

## Behavior

- `validate_split_project` reads `jobs/<job-id>/job.json` files when any job sidecars exist.
- If no sidecars exist, validation continues to validate manifest `jobs`.
- `jobs/index.json` remains ignored as derived metadata.
- Sidecar validation reports malformed JSON as a validation issue rather than failing the whole
  validation command.
- Each sidecar job must have a non-empty safe `id`, non-empty `kind`, and non-empty `updatedAt`.
- Job ids must be unique across sidecar jobs, and the containing directory name must match the job
  id.
- Workflow metadata must keep non-empty `workflowId`, `workflowType`, and `taskQueue`; `runId`, if
  present, must not be empty; `activityTypes` must contain at least one activity type.
- When both `workflow` and `startRequest` exist, the start request must match workflow id, workflow
  type, task queue, and activity list so `jobs/index.json` can truthfully report
  `startRequestReady`.
- Start requests must keep non-empty workflow id, workflow type, task queue, id reuse policy, and at
  least one activity type.
- Provider credentials remain indirect: validation can allow an env var name such as `FAL_KEY`, but
  does not require or inspect credential values.

## Non-Goals

- No `JobSummary`, `TemporalWorkflowMetadata`, or `TemporalWorkflowStartRequest` schema change.
- No Temporal client, worker, polling, cancellation, retry, or provider behavior change.
- No validation of `jobs/index.json` contents.

## Verification

- Split project tests prove invalid sidecar job ids are reported at the sidecar path.
- Split project tests prove mismatched sidecar start requests are reported.
- Split project tests prove validation falls back to manifest jobs when no sidecars exist.
