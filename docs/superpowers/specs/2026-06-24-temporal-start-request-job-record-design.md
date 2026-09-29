# Temporal Start Request Job Record Design

## Context

Video Creater can build a `TemporalWorkflowStartRequest` for generated media, and queued media
generations already record `VideoCreaterGenerateMediaWorkflow` job metadata. The project file still
loses the actual start payload, so a future Temporal client cannot submit or replay a queued
generation from the text-file project state alone.

Palmier's agent workflow depends on agents seeing full project context and being able to generate,
rerun, and tweak media in place. For Video Creater, queued jobs should therefore carry the exact
Temporal start request needed to run the durable workflow, without embedding provider credentials.

## Goal

Persist the generated-media Temporal start request on each queued generation job. The request should
be serializable in split-project files, visible to TypeScript consumers, and validated by Rust before
canonical project mutation.

## Contract

Extend `ProjectJobSummary`/`JobSummary` with optional `startRequest`:

- Existing projects without `startRequest` continue to load.
- When present, `startRequest.workflowId`, `workflowType`, `taskQueue`, `activityTypes`, and
  `idReusePolicy` must be non-empty.
- The request must match the job workflow identity when `workflow` is present.
- Generated-media requests include `providerCredentialEnvVar: "FAL_KEY"` and must not contain
  provider credentials.

`EditorWorkspace.queueMediaGeneration` should build the start request using the existing Tauri
adapter and attach it to the `recordJob` action before recording the generated asset.

## Non-Goals

- Do not start a real Temporal workflow in this slice.
- Do not change mock completion behavior.
- Do not add a Temporal dev-server dependency to frontend tests.

## Testing

- Rust project-action coverage validates a job with `startRequest` is accepted and serialized.
- Rust project-action coverage rejects a mismatched start request and workflow metadata.
- EditorWorkspace coverage verifies queued generated media calls the start-request adapter and
  records the returned request in the job action.
- Secret scan remains clean.

## Acceptance

- Queued generated-media jobs in text-file projects can carry a complete Temporal start request.
- Start requests are validated before project mutation.
- Existing job files without start requests remain compatible.
