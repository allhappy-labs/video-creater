# Palmier Generation Credit Badge Copy Design

## Context

Video Creater's media generation sheet now has compact Palmier-style mode pills, but its credit
balance badge still renders the visible copy `Mock credits`. Palmier's editor chrome treats credit
balance as product UI, not development scaffolding. The deterministic mock balance is useful during
local development, but the visible editor surface should not read like a mock or debug panel.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep the deterministic generation balance badge while changing its visible copy to product-facing
credit-balance UI.

## Behavior

- The open `Media generation` composer still exposes `Generation credit balance`.
- The badge still displays the deterministic balance `4,400`.
- The badge no longer renders visible `Mock credits` text.
- The badge uses compact balance styling with a small credit icon and `4,400` as the primary copy.
- The estimated generation cost remains visible in the header and submit footer.
- Request payloads, queue gating, model choices, generated asset metadata, Temporal records, and
  mock worker behavior remain unchanged.

## Non-Goals

- No real billing integration.
- No account or subscription state.
- No schema, Temporal, fal.ai, or generated asset behavior changes.

## Verification

- Update `MediaBin` coverage to assert the balance badge contains `4,400` and does not contain
  `Mock credits`.
- Keep generation request and queue tests passing.
- Browser QA the open generation sheet and confirm the header badges fit without mock/debug copy.
