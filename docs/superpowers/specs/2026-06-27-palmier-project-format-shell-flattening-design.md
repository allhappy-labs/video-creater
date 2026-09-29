# Palmier Project Format Shell Flattening

## Goal

Make the project format rows read as inline editor facts instead of a nested card. Resolution, frame rate, aspect ratio, and caption mode are project context for both manual edits and agent proposals, so they should be visible without extra panel chrome.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Keep the `Project format` region and accessible name.
- Keep the `Format` label and the resolution, frame rate, aspect ratio, and captions rows.
- Remove the region's rounded border, background shell, and padded panel treatment.
- Keep the main project inspector, project identity, project summary, duration/contents metric tiles, AI media, exports, and workflow queue unchanged.

## Non-Goals

- No render settings, project schema, timeline, Temporal, media, or generation changes.
- No changes to the values displayed in the format rows.
- No changes to repeated generated/export cards or workflow controls.

## Testing

- `ProjectTimelineInspector` verifies the format region remains present and still shows the same metadata.
- Regression coverage verifies the region no longer carries card-shell classes.
- Existing project timeline inspector tests continue to cover unchanged workflow, export, generated media, and project summary behavior.
