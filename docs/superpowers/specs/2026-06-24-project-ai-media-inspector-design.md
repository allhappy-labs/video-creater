# Project AI Media Inspector Design

## Context

Palmier keeps generation context attached to the editor workspace: AI clips show references,
prompt, model, settings, and generation status beside the preview and timeline. Video Creater
already exposes those details when a generated source is selected, but the fallback project
inspector only summarizes timeline files, render state, export state, and recent jobs.

That leaves a gap when no source is selected: the editor stops showing the AI media system as a
first-class part of the project, even though generated assets, mock completions, fal.ai requests,
and Temporal-shaped jobs are already present in project state.

## Goal

Make the empty-selection inspector act like a compact AI media cockpit. It should show enough
generated-media state for a user or agent to understand what has been generated, what is still
queued, which models and references were used, and which workflow backs the newest generated
assets.

## UI Behavior

Add an `AI media` section to `ProjectTimelineInspector`:

- summary counters for generated assets, completed generated outputs, active generation jobs, and
  timeline-placement requests;
- a recent generated-selects list, newest first, capped to three assets;
- each select shows the asset name or id, status, model, output count, reference count, prompt
  excerpt, and workflow status when a matching job exists;
- empty state explains that no generated media exists yet.

The section is informational only in this slice. Selection, insertion, regeneration, and replacement
remain in `SourceClipInspector` and `MediaBin`.

## Data Flow

The component reads existing `VideoProject` fields only:

- `generatedAssets` for status, prompt, model, references, outputs, and placement intent;
- `jobs` for workflow status, queue name, run id, and activity count;
- `media` is not required for this summary because detailed thumbnails already live in the source
  inspector.

No new project schema fields are needed.

## Testing

Add React tests that prove:

- the AI media section renders project-level generated asset counters;
- recent generated assets render prompt/model/reference/workflow details;
- an empty generated-media state appears when the project has no generated assets.

## Follow-Ups

- Let users select a generated asset directly from the project inspector and switch into the
  detailed source inspector.
- Add worker-backed progress percentages once Temporal activities emit durable progress.
- Add a compact model-cost rollup once real fal.ai billing metadata is persisted without secrets.
