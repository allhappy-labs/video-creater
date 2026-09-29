# Temporal Generate Media Provider Telemetry Output

## Context

The live generate-media Temporal activity now submits fal queue requests, polls status with logs
enabled, downloads the completed output, and writes generated media back into the split project.
The provider status response already contains useful non-secret run metadata such as terminal
status, queue position, logs, and metrics, but the activity output drops that data before returning
to the workflow.

Palmier-style agent editing needs generated media to remain inspectable after it is placed on the
timeline. Agents and manual editors should be able to review whether a generation completed, which
provider request it came from, and basic provider telemetry without opening fal directly or relying
on ephemeral worker logs.

## Goal

Preserve non-secret provider telemetry in `RunMediaProviderGeneration` activity output. Keep the
project write path unchanged, but return enough structured JSON for the workflow result and app UI
to inspect a completed generation run.

## Behavior

- The activity output includes:
  - `requestId`;
  - terminal `providerStatus`;
  - provider `queuePosition`;
  - provider `logs`;
  - provider `metrics`;
  - project id, asset id, job status, run id, output path, written files, and removed files.
- Logs and metrics are copied from the terminal fal queue status response already fetched with
  `logs=1`.
- The output remains serializable through the existing Temporal activity value helper.
- The output must not include provider credentials, authorization headers, generated binary content,
  or raw local project state.

## Non-Goals

- No database or project-file schema change.
- No frontend rendering of telemetry in this slice.
- No retry/backoff policy change.
- No live fal network call in tests.

## Verification

- A focused workflow test runs the provider activity helper against the fake local fal queue server
  and asserts the serialized activity output contains status, log message, and metric value.
- The same test verifies the serialized output does not contain the provider credential.
- Existing run-and-attach tests continue to verify generated media is written through split-project
  project actions.
