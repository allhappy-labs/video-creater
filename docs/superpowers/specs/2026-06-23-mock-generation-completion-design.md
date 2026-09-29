# Mock Generation Completion

## Problem

Video Creater can queue fal-backed generated assets and can build fal request payloads, but development still has no local completion path. Editors need a mock path that exercises the same project-file and Temporal job mutations a real fal worker will eventually perform.

## Scope

Add a development mock completion path for queued generated assets:

- Build ordinary `ProjectAction` values for status and completion updates.
- Use the same `completeGeneratedAsset` action real fal completion will use.
- Expose a split-project Tauri command for development and tests.
- Do not create real media bytes in this slice; output metadata remains deterministic and project-relative.

## Behavior

For a queued or running generated asset, the mock path returns:

1. `updateJobStatus` to `running` with a deterministic mock run id.
2. `completeGeneratedAsset` with one output under `generated/<assetId>/mock-output.<ext>`.
3. `updateJobStatus` to `completed`.

FLUX still-image mock outputs use PNG metadata. Wan text-to-video mock outputs use MP4 metadata and fal-normalized duration behavior.

## Acceptance

- Rust tests prove mock actions complete a queued generated asset through existing project-action validation.
- Rust tests prove the split-project command persists the completed generated asset and job status.
- TypeScript tests prove the command wrapper calls the Rust command with project dir, asset id, and update timestamp.
