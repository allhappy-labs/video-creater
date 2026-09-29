# Palmier Timeline Context Shell Flattening

## Goal

Make the compact timeline context shown beside source-focused editing read as part of the right rail, not as a standalone card. Palmier-style editing keeps the selected clip, source, and project context close together without heavy nested panel borders.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Keep the `Timeline context` region and accessible name.
- Keep the project name, split/embedded project state, duration, format, contents, workflows, and optional viewer status.
- Remove the outer card shell from `ProjectTimelineContext`: no rounded border, `bg-card`, card foreground, padding panel, or shadow.
- Keep the existing compact metric boxes inside the context for now so the visual hierarchy remains readable.
- Keep `ProjectTimelineInspector` behavior and workflow queue sections unchanged.

## Non-Goals

- No project schema, timeline model, Temporal workflow, render, or generation changes.
- No changes to the main project timeline inspector.
- No changes to the metric contents or ordering.
- No browser routing or source viewer behavior changes.

## Testing

- `ProjectTimelineContext` verifies the region remains present and still shows format/aspect information.
- Regression coverage verifies the context region no longer carries the outer card-shell classes.
- Existing project timeline inspector tests continue to cover the unchanged workflow and project metadata behavior.
