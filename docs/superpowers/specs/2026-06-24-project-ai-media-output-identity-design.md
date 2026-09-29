# Project AI Media Output Identity

## Context

Palmier keeps generated media visible across the editor: library cards, source details, timeline clips, and the project rail all make AI assets feel like first-class editing objects. Video Creater's project `AI media` inspector already summarizes generated assets, workflow state, references, models, and prompt excerpts when no source is selected. The missing project-level cue is the generated output identity itself: the media id and project-relative output path.

## Goal

Let the project inspector answer which generated media files are ready without requiring the user or agent to switch into the media bin or source inspector.

## Behavior

- Each recent generated asset card in `ProjectTimelineInspector` shows up to two output rows.
- Each row shows the output media id and relative path.
- Output rows use compact text and truncation so they do not expand the right rail beyond its normal width.
- Assets without outputs keep their existing output count and do not show an empty output block.
- No selection, insertion, replacement, workflow, or schema behavior changes.

## Tests

- Extend the project AI media inspector test to assert that a generated asset card exposes `generated-shot-1-output` and `generated/generated-shot-1/output.mp4`.
- Existing empty-state and workflow queue tests keep passing.
