# Media Generation Sticky Sheet Controls Design

## Context

The focused media generation sheet gives Video Creater enough desktop width for Palmier-style Image,
Video, and Audio generation controls. The remaining usability gap is vertical scrolling: when the
sheet content is long, the close/history header and `Queue generation` footer scroll out of view.
Palmier's generation panel keeps creation controls and the final generate action immediately
available while editors adjust references, prompts, and output settings.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep the generation sheet's header controls and queue action anchored inside the focused desktop
sheet while preserving the current in-flow narrow layout.

## Behavior

- The sheet header, containing `Media generation`, credit metadata, history, and close, is a named
  `Generation sheet header` group.
- On desktop focused-sheet layouts, the header sticks to the top of the sheet scroll area.
- The existing `Generation submit footer` sticks to the bottom of the sheet scroll area on desktop.
- The footer continues to contain tuning controls, settings summary, workflow route, and
  `Queue generation`.
- On narrow stacked layouts, header and footer remain normal in-flow content.
- Existing generation request construction and callbacks remain unchanged.

## Non-Goals

- No modal focus trap.
- No queue behavior, provider, Temporal, or schema changes.
- No new composer fields.

## Verification

- Add a `MediaBin` test proving the header and submit footer carry desktop sticky positioning.
- Existing media generation request tests continue to pass.
- Browser-smoke the open composer after scrolling inside the sheet and verify the header and queue
  action remain visible with no console errors.
