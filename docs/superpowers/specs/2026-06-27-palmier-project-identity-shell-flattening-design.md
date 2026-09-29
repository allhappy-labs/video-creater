# Palmier Project Identity Shell Flattening

## Goal

Make the project identity block in the right rail read as inline project context, not a nested panel. Palmier-style editing keeps the current project and source/timeline context visible without wrapping every read-only fact in a card shell.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Keep the `Project identity` region and accessible name.
- Keep the project label, project name, and split-project path when present.
- Remove the identity region's rounded border, muted background slab, and padded panel treatment.
- Keep the main `Project timeline inspector` region flat.
- Keep duration, contents, format, AI media, export, workflow queue, and Temporal preflight sections unchanged.

## Non-Goals

- No project schema, split-file, timeline, Temporal, render, or generation changes.
- No changes to project path resolution or text-file-editable project data.
- No changes to the compact `Timeline context` component.
- No changes to metric tiles or repeated generated/export cards.

## Testing

- `ProjectTimelineInspector` verifies the project identity region remains present and still shows name/path context.
- Regression coverage verifies the identity region no longer carries the shell classes.
- Existing project inspector tests continue to cover workflow, format, generated media, and export behavior.
