# Generation Queue Replacement Target Badge Design

## Context

Palmier keeps AI-generated clip replacement in the editor context: users can rerun or tweak a
generated clip and understand which timeline clip will be swapped. Video Creater preserves
replacement placement through `replace:<itemId>` and shows the target in the composer body, but the
sticky `Queue generation` action still labels the target generically as `Replacement`.

Palmier reference: https://www.palmier.io/docs

## Goal

Show the concrete replacement target label in the compact sticky queue action when the composer is
queueing a replacement generation.

## Behavior

- Library placement keeps the existing `Library` queue target badge.
- Timeline placement keeps the concrete track/time target badge.
- Replacement placement shows `Replace <target label>` when `replacementTargetLabel` is available.
- Replacement placement falls back to `Replacement` when no label is available.
- The queue action continues to submit the same `replace:<itemId>` placement intent.
- Generation payloads, Temporal route metadata, fal.ai/mock provider behavior, references, and
  settings remain unchanged.

## Non-Goals

- No new replacement target picker.
- No automatic replacement before generation completes.
- No project schema, Rust action, Temporal workflow, or provider changes.

## Verification

- Add a `MediaBin` test proving replacement placement shows `Generation queue target` as
  `Replace <target label>` in the sticky queue action.
- Existing replacement placement and generation payload tests continue to pass.
- Browser-smoke a replacement-seeded composer and confirm the sticky queue badges fit without
  overlap.
