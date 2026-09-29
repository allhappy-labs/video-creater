# fal.ai Queue Submission Boundary

## Context

Palmier documents media generation from the editor, timeline-aware generation, and agent-controlled
generation through connected tools. Video Creater already records generated assets, builds
Temporal-shaped start requests, and can construct fal.ai queue payloads without persisting secrets.
The remaining provider gap is the first live submission boundary that a future Temporal
`RunMediaProviderGeneration` activity can call.

## Goal

Add a small Rust helper that submits an existing `FalQueueSubmission` to the fal.ai queue endpoint
and returns the provider tracking response.

## Behavior

- The helper sends `POST` with the existing generated payload as JSON.
- Credentials are read from the environment variable named by `authEnvVar`, normally `FAL_KEY`.
- The credential is used only in the HTTP `Authorization` header.
- The request body, returned response, project actions, workflow start requests, and error variants
  do not contain the credential value.
- Missing or empty credentials return actionable errors that name only the environment variable.
- Non-success HTTP responses return status-only errors so response bodies cannot echo secrets.

## Verification

- Provider tests use a local HTTP listener to prove the payload and authorization header shape.
- Provider tests prove the request body does not contain the credential.
- Provider tests prove missing environment credentials report only the variable name.
- Existing fal request, response, and completion-action tests continue passing.

## Non-Goals

- No live fal.ai network test in CI.
- No polling loop or result download in this slice.
- No Temporal worker activity registration in this slice.
- No project schema changes.
