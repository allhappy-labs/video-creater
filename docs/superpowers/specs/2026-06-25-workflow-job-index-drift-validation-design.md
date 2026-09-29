# Workflow Job Index Drift Validation Design

## Context

Palmier's connected-agent workflow depends on agents seeing project activity and queue state while
the editor remains usable. Video Creater persists Temporal-shaped workflow jobs to
`jobs/<job-id>/job.json` sidecars and writes a bounded `jobs/index.json` catalog for safe agent
discovery. The index intentionally omits full workflow input, search attributes, project paths, and
provider payloads, but it is the first queue file an agent will read. If it drifts after manual
edits, an agent can make decisions from stale job status or workflow readiness.

Temporal Rust reference: https://docs.temporal.io/develop/rust
Palmier reference: https://www.palmier.io/docs

## Goal

Report stale `jobs/index.json` entries during split-project validation while keeping
`jobs/<job-id>/job.json` sidecars as the canonical workflow source.

## Behavior

- `load_split_project` continues to ignore `jobs/index.json`.
- `validate_split_project` reads `jobs/index.json` only when the file exists and is valid JSON.
- Invalid `jobs/index.json` JSON is reported as a validation issue, not a hard validation error.
- Each index entry must match an existing job sidecar by `jobId`.
- Each index entry's `kind`, `status`, `updatedAt`, `workflowId`, `workflowType`, `taskQueue`,
  `runId`, `activityCount`, `hasStartRequest`, `startRequestReady`, `idReusePolicy`, and
  `credentialEnvVar` must match the referenced sidecar-derived values.
- Each sidecar job must have a matching index entry when `jobs/index.json` exists.
- Duplicate index `jobId` values are reported.
- The index remains a safe discovery artifact and still must not require or expose full workflow
  input, search attributes, absolute project paths, or provider credentials.

## Non-Goals

- No `JobSummary`, `TemporalWorkflowMetadata`, or `TemporalWorkflowStartRequest` schema change.
- No Temporal client, worker, polling, cancellation, retry, or provider behavior change.
- No runtime loading semantics change.
- No validation of actual Temporal service state in this slice.

## Verification

- Split project tests prove a stale workflow job index status is reported with a fix that tells the
  agent to regenerate `jobs/index.json`.
- Split project tests prove missing sidecar jobs in the index are reported.
- Existing split-project tests continue to prove valid workflow job sidecars pass validation and
  that the index does not expose unsafe workflow input fields.
