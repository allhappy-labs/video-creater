# Generated Placeholder Completion Label

## Context

Palmier keeps generated clips readable as named shots on the timeline. Video Creater now reserves a
timeline placeholder when generation is queued, but mock completion replaces that placeholder with a
completed output clip labelled by the generated media id. The visible timeline therefore changes
from an editor-facing shot name to a technical output id as soon as generation completes.

## Goal

Preserve the generated shot name when a timeline-targeted placeholder is replaced with a completed
generated output.

## Behavior

- Completed generated-output timeline items use the generated asset `name` when present.
- If the generated asset has no `name`, the completed timeline item falls back to the output media
  id, matching the current behavior.
- Placeholder replacement keeps the same track, start time, source range, generated asset id, and
  generated output media id behavior.
- Manual insertion of completed generated outputs uses the same readable label helper.

## Non-Goals

- No schema change.
- No title editing UI.
- No change to generated source inspector headings.
- No Temporal, fal.ai, render, or media import behavior change.

## Verification

- `EditorWorkspace` test proves placeholder completion adds a timeline item labelled with the
  generated asset name.
- Existing generated-output insertion, replacement, provenance, lint, build, and secret checks pass.
