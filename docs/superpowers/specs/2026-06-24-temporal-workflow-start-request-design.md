# Temporal Workflow Start Request Design

## Context

Video Creater now records Temporal-shaped job metadata and has a feature-gated worker binary, but
the queue boundary still stops at `JobSummary`. A later Temporal client needs a stable request shape
for starting workflows without rereading UI-specific state or rebuilding workflow names ad hoc.

## Goal

Add a serializable Temporal workflow start contract generated from the existing workflow manifest.
The contract should be usable by the future Temporal Rust client, app-server queue bridges, and
tests without requiring a running Temporal service.

## Contract

Introduce `TemporalWorkflowStartRequest` with:

- `workflowId`;
- `workflowType`;
- `taskQueue`;
- `input`, as JSON payload;
- `searchAttributes`, as JSON object for project/job indexing;
- `activityTypes`, copied from the registered workflow kind;
- `idReusePolicy`, initially `rejectDuplicate`.

Generation workflow inputs must include:

- `projectId`;
- `projectDir`;
- `assetId`;
- `jobId`;
- `mockMode`;
- `providerCredentialEnvVar`, set to `FAL_KEY` rather than a secret value.

## Safety

The start request must never embed provider credentials. Tests should assert that the fal credential
field is an environment-variable name only.

## Follow-Ups

- Add a Temporal client command that submits this request and records the returned run id.
- Use the same request builder for render, transcription, Codex edit, and NLE export workflows.
- Replace local mock generation queue paths with a development-mode Temporal start command once the
  client can run against `temporal server start-dev`.
