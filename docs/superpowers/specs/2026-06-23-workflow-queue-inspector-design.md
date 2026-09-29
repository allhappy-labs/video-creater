# Workflow Queue Inspector Design

## Context

Video Creater now has Temporal-shaped `JobSummary` records and project actions that can record and update them, but the editor does not show workflow activity in the manual UI. Palmier keeps generation and agent activity close to the library, viewer, timeline, and inspector, so queued generation work should be visible without leaving the editor.

## Goals

- Add a compact workflow queue section to the existing project timeline inspector.
- Show active and recent jobs from `project.jobs` with status, kind, update time, Temporal queue, workflow type, run ID, and activity count.
- Keep the section useful when no jobs exist.
- When queueing generated media from the editor, persist a `recordJob` action together with the existing `recordGeneratedAsset` action so project files capture both the asset request and workflow state.

## Non-Goals

- No live Temporal worker or polling client is added in this slice.
- No progress streaming or cancellation controls are added yet.
- No right-rail layout redesign beyond the compact queue section.

## Acceptance Criteria

- The project timeline inspector renders a "Workflow queue" region from `project.jobs`.
- Empty projects show a clear "No workflow jobs" state.
- Workflow-backed jobs show human-readable kind/status plus Temporal metadata.
- Media generation queueing writes a batched `recordJob` and `recordGeneratedAsset` project action.
- Focused component tests, editor workspace tests, TypeScript type-checking, and frontend tests pass.
