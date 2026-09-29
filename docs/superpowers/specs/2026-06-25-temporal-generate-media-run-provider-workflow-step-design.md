# Temporal Generate Media Run Provider Workflow Step

## Context

`VideoCreaterGenerateMediaWorkflow` now schedules `BuildFalGenerationRequest`, which loads the split
project and returns a validated fal queue submission. The workflow still needed the next handoff:
turning that submission into the `RunMediaProviderGeneration` activity input so the provider run can
download output media and attach completion actions to text-editable project files.

Palmier-style generation expects generated media to move from queue state to editable project media
without a frontend-owned snapshot. The workflow therefore needs to carry the persisted start request,
validated provider submission, workflow run id, and deterministic timestamp into the provider-run
activity.

## Goal

Schedule `RunMediaProviderGeneration` from the generate-media workflow after
`BuildFalGenerationRequest` succeeds.

## Behavior

- Build a JSON-safe provider-run activity input from:
  - the original runtime payload containing `startRequest`;
  - the fal queue submission returned by `BuildFalGenerationRequest`;
  - the Temporal workflow run id;
  - the workflow start timestamp converted to RFC3339.
- Use default provider-run polling options unless a future workflow slice introduces explicit
  runtime tuning.
- Schedule `RunMediaProviderGeneration` through Temporal SDK `start_activity`.
- Return a workflow result with `status: completed` and the provider-run activity output when the
  provider activity succeeds.
- Return `status: failed` with an error summary when build or provider activity scheduling fails.
- Do not include provider credential values in workflow payloads or outputs.

## Non-Goals

- No real fal network call in automated tests.
- No retry/backoff policy tuning beyond the current Temporal activity timeout.
- No frontend UI changes.
- No changes to non-generate-media workflow stubs.

## Verification

- Add a workflow test for the pure provider-run activity input planner proving it embeds
  `startRequest`, submission, run id, updated timestamp, default polling options, and no credential
  value.
- Compile the feature-gated Temporal worker path under `--features temporal-worker`.
- Existing workflow and fal tests continue passing.
