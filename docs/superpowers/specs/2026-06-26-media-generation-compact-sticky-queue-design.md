# Media Generation Compact Sticky Queue Design

## Context

The focused media generation sheet now keeps its header and submit footer visible while scrolling.
The footer previously contained model tuning, output settings, summary metadata, workflow route, and
the `Queue generation` button. When sticky, that whole block consumes most of the sheet viewport and
pushes reference, prompt, and settings context out of sight.

Palmier's generation panel keeps the final action available without turning the entire settings
area into a pinned block.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the sticky bottom area a compact queue action bar while leaving generation tuning, settings
summary, and workflow route in normal scroll flow.

## Behavior

- `Generation footer tuning controls` remain visible in the composer but are not desktop-sticky.
- `Active generation recipe` is the canonical composer summary; the duplicated standalone
  `Generation settings summary` is removed.
- `Generation workflow route` remains hidden from the composer.
- The existing `Generation submit footer` becomes a compact sticky queue bar at the bottom of the
  focused desktop sheet.
- The sticky queue bar contains only `Queue generation` and its placement/estimate badges.
- The queue button remains disabled until the prompt is non-empty and still calls the existing
  generation submit handler.
- Narrow stacked layouts keep all footer content in normal flow.
- Generation request payloads, placement, destination, history, reference seeding, and provider
  behavior remain unchanged.

## Non-Goals

- No schema, Temporal, fal.ai, or Rust workflow changes.
- No new fields or settings.
- No modal focus trap or resizable pane work.

## Verification

- Add/update a `MediaBin` regression proving the sticky `Generation submit footer` no longer
  contains tuning controls, settings summary, or workflow route.
- Existing media generation payload tests continue to pass.
- Browser-smoke the open composer after internal scrolling and confirm the visible sticky bottom area
  is compact and the queue action remains visible.
