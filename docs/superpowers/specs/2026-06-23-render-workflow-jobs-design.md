# Render Workflow Job Records Design

## Context

Video Creater already stores render reports in project text files through `attachRenderReport`.
After the workflow queue inspector work, render controls need to leave the same durable job trail as media generation so agents and humans can inspect active and completed work from the project files.

Temporal's Rust SDK organizes durable work around workflow clients, workers, workflows, and activities. The current editor still performs a local sample render review, but project actions should already match the future Temporal worker contract.

## Scope

- Persist render button actions as a batch of project actions:
  - `recordJob` with Temporal workflow metadata.
  - `updateJobStatus` to mark the local sample render complete.
  - `attachRenderReport` with the validation report.
- Use the existing workflow task queue: `video-creater-workflows`.
- Use workflow type `VideoCreaterRenderDraftWorkflow` for both draft and final WebM render intents until profile-specific render workflows exist.
- Use activity names that describe the eventual worker boundary:
  - `BuildRenderPlan`
  - `RenderMedia`
  - `ValidateRenderedMedia`
  - `AttachRenderReport`

## Workflow Metadata

Render jobs should use deterministic, project-readable workflow IDs:

```text
video-creater/{projectId}/render-draft/{jobId}
```

IDs are slugged with the same helper used by generated-media jobs so the records remain safe in logs, status panels, and file-based diffs.

## Current Behavior

The UI still creates sample render reports synchronously for `draftWebm` and `finalWebm`. Because no live Temporal client is wired into the frontend yet, the job is immediately recorded as `completed` with `runId: null`.

This is intentional for the current slice: the text-file project contract becomes ready for the worker integration without blocking the manual editor.

## Acceptance Criteria

- Embedded project render actions call `apply_project_action_to_project` for the job record and render report.
- Split-project render actions use `apply_project_actions_to_split_project_folder` so the job status and render report are written atomically.
- The project timeline inspector can show the latest render report after a split-project render.
- Existing render review UI remains unchanged for draft and final WebM reports.
