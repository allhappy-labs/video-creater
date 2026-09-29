# Palmier AI Media Shell Flattening

## Goal

Make the project inspector's AI media section read as part of the right rail instead of a nested panel. Generated media is project context for both manual editing and agent actions, so the section should stay visible without an extra rounded background shell.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Keep the `AI media` region and accessible name.
- Keep the generated count, outputs/jobs/timeline/selects metrics, recent generated assets, output inspect actions, destination context, workflow status, prompt excerpt, and empty state.
- Remove the outer `AI media` region's rounded border, muted background, and padded panel treatment.
- Keep the internal metric boxes and generated asset cards unchanged because they are compact repeated items.
- Keep project identity, summary, format, exports, workflow queue, and Temporal preflight unchanged.

## Non-Goals

- No generated asset schema, media folder, fal.ai, Temporal, render, or timeline changes.
- No changes to generated output selection behavior.
- No changes to repeated generated asset cards or output rows.
- No changes to empty-state copy.

## Testing

- `ProjectTimelineInspector` verifies the `AI media` region remains present and still summarizes generated media.
- Regression coverage verifies the outer region no longer carries shell classes.
- Existing generated media tests continue to cover replacement/destination context, output inspection, and empty state behavior.
