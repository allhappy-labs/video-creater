# Workflow Start Request Inspector Design

## Context

Queued generated-media jobs can now persist a `startRequest` payload with the exact Temporal
workflow start contract. The project timeline inspector still shows only workflow metadata, so an
editor or agent cannot confirm from the UI whether a queued job has a replayable start request.

## Goal

Expose the persisted Temporal start request inside the existing Workflow queue section without
embedding or revealing provider credentials.

## Behavior

- Workflow queue job cards show a compact `Start request` block when `job.startRequest` is present.
- The block shows `workflowId`, `idReusePolicy`, activity count, and credential environment variable
  when the input names one.
- The block labels the request as `ready` only when the workflow id, workflow type, task queue, and
  activity list match the rendered workflow metadata.
- Jobs without a start request keep the existing layout.

## Non-Goals

- Do not start a Temporal workflow from the inspector in this slice.
- Do not show full JSON input, search attributes, provider credentials, or project directory paths.
- Do not add polling, cancellation, or retry controls.

## Verification

- `ProjectTimelineInspector` tests prove start-request details render for queued generated-media
  jobs.
- The test fixture includes `providerCredentialEnvVar: "FAL_KEY"` and no secret value.
- Existing workflow queue empty and metadata tests keep passing.
- Secret-fragment scan remains clean.
