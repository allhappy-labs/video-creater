# Temporal Generate Media Attach Result Boundary

## Context

Video Creater can now run a fal generation from a Temporal start request and split project folder,
returning `completeGeneratedAsset` and `updateJobStatus` actions. The next generate-media worker
activity is `AttachGeneratedAssetResult`: it must persist those validated actions into the
text-file-editable project folder without giving the provider helper direct ownership of canonical
project state.

## Goal

Add a worker-facing attach-result boundary that validates the generate-media Temporal start request
and applies the returned project actions to the split project at `projectDir`.

## Behavior

- Accept a `TemporalWorkflowStartRequest` and a `ProjectAction` batch.
- Validate the start request as `VideoCreaterGenerateMediaWorkflow`.
- Use `projectDir` from the validated request.
- Apply the batch through the existing split-project action applier.
- Return `ProjectActionWriteResult` so callers can inspect the persisted project and written files.
- Preserve Rust project-action validation as the only canonical mutation path.
- Keep provider credentials out of project files, actions, specs, and logs.

## Non-Goals

- No live Temporal SDK workflow orchestration change in this slice.
- No fal network call, media download, or provider polling.
- No new project action variant.
- No UI changes.

## Verification

- Rust workflow tests save a split project with a queued generated asset and Temporal job, apply
  completion actions through the boundary, reload the split project, and verify:
  - generated asset status is completed;
  - generated output media is recorded in `media/index.json`;
  - job status and run id are persisted;
  - generated asset sidecar is written.
- Existing workflow tests continue passing.
