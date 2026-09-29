# fal.ai Generation Result Completion Boundary

## Context

Video Creater now has separate Rust helpers for fal queue submit, status fetch, result fetch, and
generated-output download. A Temporal `RunMediaProviderGeneration` / `ImportGeneratedOutput` /
`AttachGeneratedAssetResult` path still needs a single worker-facing handoff that turns a completed
fal model result into downloaded media plus validated project actions.

## Goal

Add a worker-ready Rust helper that accepts a generated asset, completed fal result JSON, project
directory, timestamp, optional Temporal run id, and optional replacement timeline item id, then:

- derives the expected project-relative output path,
- downloads the returned fal media URL into that path,
- returns the existing `completeGeneratedAsset` and `updateJobStatus` project actions.

## Behavior

- Flux image generations default to `generated/<asset-id>/fal-output.png`.
- WAN text-to-video generations default to `generated/<asset-id>/fal-output.mp4`.
- Unsupported providers or model ids fail before download.
- Output download uses the existing generated-output path validation.
- Returned project actions preserve existing reducer ownership of canonical mutation.
- The helper does not start Temporal workflows, poll fal queue status, or apply project actions.

## Verification

- Provider tests use a local HTTP listener to prove the result media is downloaded to the derived
  path and the returned actions reference that same path.
- Existing fal request, queue, result, download, and completion-action tests keep passing.

## Non-Goals

- No live fal.ai e2e generation in CI.
- No polling loop or retry schedule in this slice.
- No direct project file mutation in this slice.
- No Temporal worker registration in this slice.
