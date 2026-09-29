# Generated Source Workflow Status

## Context

Palmier shows AI-generated media as first-class editor objects: users can inspect prompt, references, model settings, and generation state without leaving the timeline or source viewer. Video Creater already records Temporal-backed generation jobs and shows a global workflow queue, but the selected generated source inspector does not connect a generated asset back to its workflow job.

Superseded by `2026-06-27-generated-source-inspector-workflow-diagnostics-removal-design.md`: generated workflow metadata remains in job, activity, media-card, and timeline surfaces, while the Source Inspector now stays focused on generated media recipe, references, prompt, and edit actions.

## Goal

When a generated source or generated timeline clip is selected, the Source Inspector should show the related Temporal workflow status near the generated details. This makes queued, running, failed, and completed generations understandable in the same place where the user reviews prompt, references, and AI Edit actions.

## Behavior

- `SourceClipInspector` accepts the current project workflow jobs.
- For a generated asset, it finds a related job whose id matches the generated asset id.
- The generated Source Inspector no longer shows a compact workflow section or AI Edit workflow status line.
- Workflow status remains visible in workflow/job-oriented surfaces, media generation cards, and timeline badges.
- The default sample project includes a completed generation workflow for its sample generated output so the inspector demonstrates this state.
- Existing queue actions still go through Temporal-backed `recordJob` and generated-asset project actions.

## Non-Goals

- No automatic timeline replacement when a queued variation completes.
- No new Temporal workflow type or activity list.
- No project schema change; this uses existing `ProjectJobSummary.workflow` metadata.
- No global workflow queue redesign.

## Tests

- `SourceClipInspector` does not render workflow diagnostics for a selected generated source when a matching job exists.
- `EditorWorkspace` keeps project jobs available for workflow/activity surfaces and timeline/media-card status.
