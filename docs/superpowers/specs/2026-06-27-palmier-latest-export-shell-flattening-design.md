# Palmier Latest Export Shell Flattening

## Goal

Make the latest export readout feel like inline project status in the right rail, not a nested panel. Export state is part of the editor workflow, so the current artifact should stay visible without an extra rounded muted shell.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Keep the `Latest export` region and accessible name.
- Keep the `Latest export` label and the format, output path, and job rows.
- Keep the `No export artifact` empty state.
- Remove the region's rounded border, muted background, and padded panel treatment.
- Keep recent export artifact cards, workflow queue, Temporal preflight, AI media, project summary, and project format unchanged.

## Non-Goals

- No export artifact schema, render pipeline, Temporal workflow, or file path changes.
- No changes to export ordering or recent export cards.
- No changes to render controls or draft review UI.

## Testing

- `ProjectTimelineInspector` verifies `Latest export` still shows the persisted export artifact fields.
- Regression coverage verifies the region no longer carries shell classes.
- Existing export and workflow tests continue to cover ordering, caps, and queued workflow behavior.
