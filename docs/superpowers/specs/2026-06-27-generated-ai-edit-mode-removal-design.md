# Generated AI Edit Mode Removal Design

## Context

The generated Source Inspector has been flattened into inline generated details and AI edit
sections, but `Generated AI edit` still uses a `Single` / `Variation set` segmented control.
That control exposes implementation shape instead of an editing decision: both paths are
regeneration actions for the selected generated asset.

Palmier reference: https://www.palmier.io/docs

## Goal

Make generated-source regeneration feel like direct editor actions by removing the mode switch
and keeping the single-variation and variation-set workflows available in one compact panel.

## Behavior

- `Generated AI edit` no longer renders `Single` or `Variation set` mode buttons.
- The panel keeps `Queue upscale` as a direct action when an output media file can be upscaled.
- The single variation prompt is always visible when variation queueing is available.
- `Queue variation` and `Queue and replace selected clip` keep using the trimmed single
  variation prompt.
- Variation-set drafts render as a compact `Variation set` subsection when variation-set
  queueing is available.
- `Queue variation set` keeps sending the valid named drafts and remains disabled until at
  least two draft prompts are non-empty.
- No project schema, Temporal workflow, fal.ai payload, generated asset lineage, or media
  folder inheritance behavior changes in this slice.

## UI Contract

- The panel should read as one action stack, not as a tabbed or modal editor.
- Section labels may clarify the prompt and variation-set drafts, but they should not act as
  toggles.
- Existing generated provenance blocks (`Recipe`, `File`, `References`, `Prompt`) remain
  unchanged.
- Empty/unavailable action copy remains concise: if neither single nor set queueing is
  available, the panel states that AI edit actions are unavailable for the project state.

## Verification

- Update `SourceClipInspector` tests to assert the generated AI edit panel contains both direct
  variation controls and variation-set drafts without `Single` / `Variation set` mode buttons.
- Keep tests for single variation, replacement variation, variation sets, and upscale payloads.
- Browser-smoke a selected generated source at desktop and narrow widths to confirm the direct
  action stack does not overflow or hide the prompt/draft controls.
