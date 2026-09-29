# Temporal Generate Media Project Directory Boundary

## Context

Video Creater now records Temporal `VideoCreaterGenerateMediaWorkflow` jobs and has fal.ai helpers
that can submit, poll, download, and return project actions. The remaining worker gap is the handoff
between a Temporal start request and the text-file-editable project folder. Worker activities should
not require the frontend to pass a full in-memory project snapshot.

## Goal

Add a worker-facing boundary that loads the split project from `projectDir` in the Temporal start
request, resolves the generated asset, and builds the fal queue submission from the current project
files.

## Behavior

- Accept a `TemporalWorkflowStartRequest`.
- Validate it as a generate-media workflow start request.
- Load the split project from the request `projectDir`.
- Reuse the existing `temporal_generate_media_fal_queue_submission` validation.
- Return the same `FalQueueSubmission` as the in-memory path.
- Keep provider credentials out of project files and request bodies; the submission only names the
  credential environment variable.

## Non-Goals

- No live Temporal workflow orchestration change in this slice.
- No fal network call, provider polling, media download, or project mutation.
- No UI changes.
- No secret persistence.

## Verification

- Rust workflow tests save a split project with a queued fal generated asset, build a Temporal start
  request, and prove the project-directory boundary returns the expected fal endpoint, prompt input,
  and `FAL_KEY` env var name.
- Existing workflow, fal, and split-project tests continue passing.
