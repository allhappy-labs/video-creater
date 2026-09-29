# Fal Generation Worker Run Boundary

## Context

The Temporal worker migration plan names four generation activities: build the fal.ai request,
run the provider, import generated output, and attach generated asset results. Previous slices added
the queue submission boundary, queue status/result fetchers, generated output downloader, and
completion action builder. The missing gap is a worker-ready orchestration boundary that executes a
single fal queue run end to end while preserving Video Creater's project-action contract.

## Goals

- Submit a validated fal queue request with credentials kept in the Authorization header only.
- Poll the fal status URL with logs enabled until the queue reaches `COMPLETED`.
- Fetch the response payload, download the expiring media URL under `generated/`, and build the
  existing `completeGeneratedAsset` plus `updateJobStatus` actions.
- Return the downloaded output path, provider result, final status, request id, and actions to the
  caller without mutating canonical project files.
- Bound polling with explicit worker options so stuck provider jobs return a deterministic error.

## Non-Goals

- No Temporal SDK runtime registration in this slice.
- No project-file write or action application inside the fal helper.
- No UI changes, provider model catalog changes, or secret persistence.
- No retry, cancellation, or resume-from-request-id behavior beyond bounded polling.

## Design

`run_fal_generation_submission_with_client` is the testable core boundary. It accepts a prebuilt
`FalQueueSubmission`, a reqwest blocking client, the project directory, the generated asset, a
credential string, completion metadata, and `FalGenerationRunOptions`.

The helper:

1. calls `submit_fal_queue_submission_with_client`;
2. polls `fetch_fal_queue_status_with_client(..., logs = true)` up to `max_status_polls`;
3. fails with `StatusPollLimitExceeded` if the status never completes;
4. fails with `CompletedWithProviderError` if a completed status carries fal error fields;
5. fetches the response JSON from the completed status `response_url`;
6. calls `download_fal_generation_result_and_build_completion_actions`;
7. returns `FalGenerationRun` containing request id, terminal status, result JSON, output path, and
   project actions.

`run_fal_generation_job` builds the default fal queue submission and HTTP client for production worker
use. `run_fal_generation_job_from_env` additionally reads `FAL_KEY` through the existing env boundary.

## Acceptance

- A local fal queue sequence test proves submit, two status polls, result fetch, media download, and
  returned project actions work together.
- The provider credential appears only in fal Authorization headers, never in request bodies or error
  messages.
- A poll-limit test proves stuck jobs return a deterministic worker error without downloading media.
- Existing fal request, result, download, and completion tests continue to pass.
