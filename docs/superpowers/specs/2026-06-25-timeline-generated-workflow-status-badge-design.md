# Timeline Generated Workflow Status Badge Design

## Context

Palmier treats AI generation as native timeline work: generated clips can be inspected, regenerated, swapped, and edited without leaving the project. Video Creater already marks generated timeline clips with an `AI` badge and exposes selected clip actions for rerun, replace, and prompt tweak. The remaining timeline-level gap is that queued, running, or failed generation jobs are only easy to inspect in side panels, not directly on the clip that will be replaced or varied.

## Requirements

- Show a compact workflow status badge directly on generated timeline clips when the project has a matching generation job.
- Derive the badge from text-file-backed project state: timeline item provenance, `generatedAssets`, and `jobs`.
- Keep `TimelineEditor` decoupled from the full project schema by passing a small item-id keyed status view.
- Preserve existing generated clip detection, filmstrip treatment, and selected action dock behavior.
- Keep the visible badge short enough for dense timelines, with an accessible label/title that includes workflow type, queue, and updated time.

## Design

- Add a `GeneratedTimelineWorkflowStatus` view model to `TimelineEditor`.
- Add a `generatedWorkflowStatuses` prop keyed by timeline item id.
- In `EditorWorkspace`, derive `generatedTimelineItemWorkflowStatuses` by:
  - finding each timeline item with a matching generated asset;
  - finding the project job with the same asset id;
  - mapping job status, updated time, workflow type, and task queue into the compact status view.
- In clip rendering:
  - keep the existing `AI` chip;
  - render a second chip only when status exists;
  - visible copy is `queued`, `running`, `failed`, etc.;
  - `aria-label`/`title` provide the full workflow route context.

## Acceptance

- A generated clip with a running `VideoCreaterGenerateMediaWorkflow` job renders an `AI` badge and a workflow status badge.
- The status badge is reachable by accessible label and includes status, workflow type, queue, and timestamp.
- Existing timeline generated rerun, replacement rerun, AI edit, and badge tests remain passing.
