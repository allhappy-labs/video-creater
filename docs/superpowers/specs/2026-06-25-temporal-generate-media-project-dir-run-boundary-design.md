# Temporal Generate Media Project Directory Run Boundary

## Context

Video Creater can build a fal queue submission from a Temporal start request and split project
folder. The next worker step is to run a supplied fal submission from the same project-folder
context, so Temporal activities can submit, poll, fetch results, download outputs, and return
validated project actions without needing a frontend-owned project snapshot.

## Goal

Add a worker-facing fal run boundary that accepts a `TemporalWorkflowStartRequest`, loads the split
project from `projectDir`, validates the supplied fal queue submission against the current generated
asset file, runs the provider through an injected HTTP client, and returns completion actions.

## Behavior

- Validate the start request as `VideoCreaterGenerateMediaWorkflow`.
- Load the split project from `projectDir`.
- Reuse the existing in-memory fal run validation:
  - live mode is required;
  - `jobId` must match `assetId`;
  - the supplied submission must match the generated asset request body, endpoint, method, and
    credential env-var name.
- Submit, poll, fetch result JSON, and download the output through the existing fal helper.
- Return `completeGeneratedAsset` and `updateJobStatus` actions without applying them.
- Keep credentials in the Authorization header only. No credential may appear in returned project
  actions, project files, request bodies, specs, or logs.

## Non-Goals

- No live Temporal workflow orchestration change in this slice.
- No direct project mutation or split-project save after provider completion.
- No real fal network call in automated tests.
- No UI changes.

## Verification

- Rust workflow tests run against a fake local fal queue server and prove the boundary loads the
  generated asset from the split project, submits the expected request, downloads
  `generated/<asset-id>/fal-output.png`, and returns completion actions without leaking credentials.
- Existing workflow and fal-filtered tests continue passing.
