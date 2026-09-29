# Media Bin Generated Workflow Status

## Context

Palmier keeps AI generation activity close to the media library and timeline. Video Creater already records Temporal-shaped `JobSummary` entries and shows workflow state in the right rail, but generated cards in the Media Bin only show the generated asset status. Editors cannot tell which Temporal workflow is attached to a queued or running generation without looking elsewhere.

## Goal

Show related workflow status directly on generated asset cards in the Media Bin.

## Behavior

- `MediaBin` accepts `workflowJobs`.
- For each generated asset, it finds the job whose `id` matches the generated asset id.
- When a related job exists, the generated card shows a compact workflow line with human-readable job status and kind.
- When workflow metadata exists, the card also shows workflow type, task queue, run id when present, and activity count.
- Cards without a matching job keep their existing layout.
- `EditorWorkspace` passes `project.jobs` to the Media Bin.

## Non-Goals

- No Temporal polling or cancellation controls.
- No job mutation from generated cards.
- No new project schema fields.

## Acceptance

- Media Bin tests prove generated cards render matching workflow metadata and do not show unrelated jobs.
- EditorWorkspace tests prove project jobs are passed into the media panel by rendering a generated card workflow line from project state.
- Existing frontend checks pass.
