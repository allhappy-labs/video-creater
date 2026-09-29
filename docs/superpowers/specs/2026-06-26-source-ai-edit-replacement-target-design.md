# Source AI Edit Replacement Target Design

## Context

Palmier keeps AI edits native to the timeline: when a generated clip is tweaked as a replacement,
the editor should make the affected timeline clip obvious at the point of action. Video Creater's
Source Inspector can already queue a generated replacement variation, but the AI Edit tab only shows
the generic workflow placement `Replacement`.

Palmier reference: https://www.palmier.io/docs

## Goal

Show the selected replacement target in the generated Source Inspector AI Edit tab before the user
queues a replacement variation.

## Behavior

- When `Queue and replace selected clip` is available, AI Edit shows a compact
  `AI edit replacement target` group.
- The group shows the provided `replacementTargetLabel` when available.
- If no label is provided, the group falls back to the selected timeline item label.
- The existing queue callback and `replace:<itemId>` workflow placement behavior remain unchanged.
- Plain variation, variation-set, upscale, generated details, and source context behavior remain
  unchanged.

## Non-Goals

- No new replacement target picker.
- No automatic replacement before generation completes.
- No project schema, Temporal workflow, Rust action, or provider changes.

## Verification

- Add a `SourceClipInspector` test proving AI Edit shows the replacement target label next to the
  replacement queue action.
- Existing generated AI Edit variation and replacement callback tests continue to pass.
- Browser-smoke a selected generated clip AI Edit tab and confirm the target cue fits in the right
  rail without overlapping the prompt controls.
