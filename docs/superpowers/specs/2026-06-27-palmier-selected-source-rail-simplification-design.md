# Palmier Selected Source Rail Simplification Design

## Context

Palmier keeps the right rail focused on the active editing context. Video Creater showed three stacked panels when a source-backed timeline clip was selected: generated/imported source provenance, timeline clip editing controls, and the full project timeline inspector. That made selected-clip work feel heavier than Palmier's compact inspector.

## Decision

When a source-backed timeline clip is selected, keep the source inspector and the timeline clip editor, but hide the full project timeline inspector. The project inspector remains available for project-level and non-source selections, including caption/template selections.

## Requirements

- Selected source clips show `Source Inspector`.
- Selected source clips show `Timeline source clip editor`.
- Selected source clips do not show `Project timeline inspector`.
- Caption and other non-source selections continue to show the project timeline inspector.
- Source clip editing, reveal, generation, replacement, trim, split, fade, volume, opacity, and reorder actions remain unchanged.

## Testing

- Update the right-rail source/timeline context test to assert the project inspector is absent for selected source clips.
- Keep coverage proving caption selections still show the project timeline inspector.
- Run focused `EditorWorkspace` tests, the full `EditorWorkspace` test file, lint, full tests, and desktop/narrow visual QA.
