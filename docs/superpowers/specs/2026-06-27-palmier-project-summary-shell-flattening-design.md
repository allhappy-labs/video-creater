# Palmier Project Summary Shell Flattening

## Goal

Make the project timeline summary rows read as inline inspector facts instead of a nested card. The right rail should keep project status visible for manual and agent-controlled editing without wrapping read-only values in extra panel chrome.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Expose the render/locked/media/generated summary rows as an accessible `Project summary` group.
- Keep the existing row labels and values: render format, locked tracks, media count, and generated asset count.
- Remove the summary group's rounded border, background shell, and padded panel treatment.
- Keep duration and contents metric tiles unchanged.
- Keep `Project format`, `AI media`, exports, workflow queue, and Temporal preflight sections unchanged.

## Non-Goals

- No project schema, media index, generated asset, Temporal, render, or timeline data changes.
- No changes to the split-file project workflow.
- No changes to repeated generated/export cards.
- No changes to the compact `Timeline context` component.

## Testing

- `ProjectTimelineInspector` verifies the `Project summary` group is present and still shows render, locked, media, and generated values.
- Regression coverage verifies the group no longer carries card-shell classes.
- Existing project timeline inspector tests continue to cover unchanged format, workflow, generated media, and export behavior.
