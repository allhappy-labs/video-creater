# Generation Credit Balance Badge

## Context

Palmier's media generation composer shows the editor's available generation credits near the mode
controls while also showing the estimated cost for the queued generation in the footer. Video Creater
already shows deterministic estimated credits for image, video, and audio requests, but it does not
show any available balance. In development, generation still uses mock workers, so the balance must
be clearly presented as mock UI metadata rather than real account billing.

## Goal

Add a compact mock credit-balance badge to the media generation composer header so the generation
flow has the same balance-versus-cost shape as the Palmier reference UI.

## Behavior

- When the media generation composer is open, show a labelled `Generation credit balance` badge in
  the header.
- The badge displays a deterministic mock balance of `4,400` credits.
- Keep the existing estimated credit cost in the header and footer.
- The balance is UI-only metadata and does not affect request payloads, model choices, folder
  targeting, placement intent, or workflow routing.
- The label should make clear that this is a mock balance during development.

## Non-Goals

- No real billing integration.
- No account, subscription, or payment state.
- No generated asset schema change.
- No Temporal, fal.ai, or mock worker behavior change.

## Verification

- A Media Bin test asserts the generation composer exposes a `Generation credit balance` badge with
  `4,400`.
- Existing media generation request and composer tests continue to pass.
- Browser QA confirms the header badges fit in the media panel without overlapping.
