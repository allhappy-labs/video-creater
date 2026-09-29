# Temporal Generate Media Run Attach Boundary

## Context

Video Creater now has separate worker-facing boundaries for loading a split project to build a fal
queue submission, running that supplied submission through an injected HTTP client, and applying
returned completion actions back into split project files. A Temporal activity needs a single
orchestration boundary that composes those steps without taking ownership of canonical project
mutation outside the existing project-action writer.

Palmier-style editing requires agent-generated assets to become normal project media as soon as the
provider run finishes, so a connected agent can generate, place, replace, rerun, and inspect results
from text-file-editable project state rather than transient frontend memory.

## Goal

Add a worker-facing run-and-attach helper that accepts a `TemporalWorkflowStartRequest`, a validated
fal queue submission, an injected HTTP client, provider credential value, completion timestamp, and
optional Temporal run id. The helper runs the provider request from the split project context, then
applies the returned `completeGeneratedAsset` and `updateJobStatus` actions to the same split
project folder.

## Behavior

- Reuse `temporal_generate_media_fal_run_submission_from_project_dir_with_client` for start-request,
  project-dir, generated-asset, live-mode, submission, and credential-env-var validation.
- Reuse `temporal_generate_media_attach_generated_asset_result_to_project_dir` to mutate canonical
  project files through the project-action validation path.
- Return both the provider run metadata and the project-action write result to the worker caller.
- Preserve the supplied `run_id` on the project job workflow metadata when completion actions are
  applied.
- Keep provider credentials in request headers only. Credentials must not be written to project JSON,
  returned action JSON, specs, or request bodies.

## Non-Goals

- No live Temporal SDK workflow registration or worker polling change in this slice.
- No frontend UI changes.
- No direct split-project file writes outside the existing project-action writer.
- No real fal network call in automated tests.

## Verification

- Add a workflow test that creates a split project with a queued generated asset and running
  generate-media job, builds a fal queue submission from the project folder, points that submission at
  a fake local fal queue server, runs the one-shot helper, and asserts:
  - the provider output is downloaded into `generated/<asset-id>/fal-output.png`;
  - the generated asset is completed with one output;
  - generated media appears in the project media list;
  - the job is completed with the supplied Temporal run id;
  - the provider credential appears only in the Authorization header and not in the request body or
    written project JSON.
- Existing workflow and fal boundary tests continue passing.
