# Temporal Generate Media Runtime Input Boundary

## Context

Generate-media jobs persist a complete `TemporalWorkflowStartRequest` in text-editable project
files. Recent worker boundaries now validate against that full request before loading the split
project, building fal submissions, running provider generation, and attaching generated outputs.
However, the Temporal client start plan still sent only the nested `input` object to the worker.

That flat payload is not enough for a real `VideoCreaterGenerateMediaWorkflow` because the worker
must know the workflow id, workflow type, task queue, activity list, search attributes, id reuse
policy, and nested provider input to replay and validate the same project-file contract.

## Goal

Make the generate-media Temporal runtime payload self-contained by embedding the full
`TemporalWorkflowStartRequest` under `startRequest` when building a client start plan.

## Behavior

- For `VideoCreaterGenerateMediaWorkflow`, `TemporalWorkflowClientStartPlan.input` is:
  - `{"startRequest": <TemporalWorkflowStartRequest>}`.
- The embedded request includes:
  - workflow id;
  - workflow type;
  - task queue;
  - search attributes;
  - activity types;
  - id reuse policy;
  - nested generate-media input, including `assetId`, `jobId`, `projectDir`, `mockMode`, and
    `providerCredentialEnvVar`.
- The runtime payload stores only the provider credential environment variable name, not a secret
  value.
- Non-generate-media workflows keep their existing flat input payloads until their worker activities
  need the same envelope.

## Non-Goals

- No live Temporal workflow activity sequencing change in this slice.
- No change to persisted project job records.
- No frontend UI changes.
- No real provider network calls.

## Verification

- Add a workflow test proving a generate-media client start plan embeds the full start request and
  keeps `FAL_KEY` as an env-var name only.
- Existing workflow and fal tests continue passing.
- The Temporal feature-gated worker path still compiles under `--features temporal-worker`.
