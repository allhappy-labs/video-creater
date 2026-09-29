# Palmier Generation Header Estimate Removal Design

## Context

Video Creater's media generation sheet now has a compact Palmier-style header, but it still shows
two cost estimates: one chip in the header and one in the submit footer. Palmier's composer keeps
the balance in the header and places the per-request cost near the send action. Duplicating the same
estimate makes the header busier than it needs to be.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the duplicate header estimate chip while keeping generation cost visible at the submit action.

## Behavior

- The header still shows the generation credit balance.
- The header no longer renders `Est. <n> credits`.
- The submit footer still renders `Generation submit estimate` with `Est. <n> credits`.
- The settings summary and active generation recipe still expose model, output, timing, and estimate
  metadata for inspection.
- Cost calculations, request payloads, model settings, queue behavior, Temporal records, and fal.ai
  behavior remain unchanged.

## Non-Goals

- No billing or balance behavior changes.
- No removal of the submit-footer estimate.
- No model, duration, or request schema changes.

## Verification

- Update `MediaBin` coverage so the generation header has the balance but not the estimate chip.
- Keep footer estimate coverage proving `Est. 4 credits` remains near `Queue generation`.
- Keep generation request and cost tests passing.
- Browser QA the open generation sheet and confirm the header is reduced to balance, history, close,
  and any active queue status.
