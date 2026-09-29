# Timeline Generated Provenance Row Design

## Context

Palmier keeps AI-generated media understandable at the editing surface: generated clips can be regenerated, swapped, and inspected from the timeline without first opening a separate panel. Video Creater already shows `AI` and workflow status badges on generated timeline clips, but the clip body does not identify the generated asset, model, or prompt. Editors can see that a clip is AI-backed, but not which generated shot it represents.

## Requirements

- Generated timeline clips should show a compact provenance row inside the clip body.
- The row should include the generated asset ID and model label when available.
- The row should include a short prompt preview when clip width allows text to remain readable.
- Imported clips should keep the current timeline layout and should not show generated provenance.
- Provenance should be derived from existing project state: timeline items, generated asset outputs, and `generatedAssets`.
- No project schema, Temporal workflow, render pipeline, or action contract changes.

## Design

- Add a `generatedProvenance` prop to `TimelineEditor`, keyed by timeline item ID.
- Each provenance entry contains:
  - `assetId`
  - `modelLabel`
  - `prompt`
- In `EditorWorkspace`, derive the map by resolving each timeline item through `generatedAssetForTimelineItem`.
- In `TimelineEditor`, render a compact `Generated provenance` row for generated clips with matching provenance.
- Keep the row non-interactive and visually secondary so the existing selected action dock, trim handles, workflow badge, title, and metadata remain primary.

## Acceptance

- A generated timeline clip with provenance renders a `Generated provenance for <clip label>` group containing the asset ID, model label, and prompt.
- `EditorWorkspace` passes generated provenance from project state into the timeline for clips backed by generated outputs.
- Imported timeline clips do not render generated provenance.
- Existing generated clip badges, workflow badges, selected dock actions, drag, trim, split, and source opening behavior remain unchanged.
