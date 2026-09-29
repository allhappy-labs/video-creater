# fal.ai Queue Status And Result Boundary

## Context

Palmier-style generated media needs a durable worker path: submit a generation request, watch queue
state, fetch the model result, import the output, and then update the editable project files.
Video Creater can now build and submit fal.ai queue requests. fal's async REST API returns
`status_url` and `response_url`; a future Temporal `RunMediaProviderGeneration` activity needs a
small, secret-safe Rust boundary for those follow-up calls.

## Goal

Add Rust helpers that fetch fal.ai queue status and final result JSON using an existing queue URL
and an environment-sourced credential.

## Behavior

- `fetch_fal_queue_status` sends authenticated `GET` to the recorded `status_url`.
- When logs are requested, the helper appends `logs=1` while preserving existing query parameters.
- `fetch_fal_queue_result` sends authenticated `GET` to the recorded `response_url`.
- Both helpers parse JSON into the existing `FalQueueStatusResponse` or model-specific result
  `serde_json::Value`.
- Credentials are used only in the HTTP `Authorization` header.
- HTTP status errors remain status-only; response bodies are not embedded in errors.
- The helpers do not mutate project files or download media outputs.

## Verification

- Provider tests use a local HTTP listener for status and result calls.
- Tests assert the expected fal queue REST paths and `logs=1` query behavior.
- Tests assert credentials are present in the authorization header and absent from request bodies.
- Existing fal submit, response decode, and completion-action tests keep passing.

## Non-Goals

- No live fal.ai network tests in CI.
- No polling loop or retry schedule in this slice.
- No media URL download/import in this slice.
- No Temporal worker registration in this slice.
