# Palmier AI Edit Replacement Target Flattening

## Goal

Make the AI edit replacement target read as inline editor context in the right rail, not as a nested card. Palmier keeps generation, prompt tweaks, and timeline placement in one editor flow, so replacement context should stay visible without adding panel chrome.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Keep the `AI edit replacement target` group and accessible name.
- Keep the `Replacement target` label and selected timeline target text.
- Remove the rounded border, background slab, and padded card treatment from the replacement target group.
- Keep generated/imported AI edit prompt controls, queue actions, and replacement callbacks unchanged.
- Keep the target text truncated with the full label exposed via `title`.

## Non-Goals

- No replacement target picker changes.
- No generation workflow, Temporal, fal.ai, project schema, or media index changes.
- No changes to provenance previews, output choices, or source metadata sections.

## Testing

- `SourceClipInspector` verifies the replacement target group remains present and no longer carries card-shell classes.
- Existing replacement variation callback coverage continues to prove the same `replace:<itemId>` placement is queued.
- Component verification is sufficient for this slice because the runtime fixture does not always expose a replacement target context in the live app.
