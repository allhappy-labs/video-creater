# Palmier Timeline Disabled Link Control Removal Design

## Context

Palmier's timeline row headers stay compact: lane identity and live media-state controls sit close to
the track rows while editing details move into the timeline body or inspector. Video Creater still
renders a disabled `track linked` button on every row. It has no action, consumes horizontal space,
and reads like an extra switch beside the real visible/audible controls.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the persistent disabled link chrome from timeline row headers while preserving the track
state behavior that editors can actually change from this surface.

## Behavior

- Timeline row headers no longer render `track linked` buttons.
- Visual tracks keep the `visible` / `hidden` control.
- Audio tracks keep the `audible` / `muted` control.
- Track lock state, enabled state, drag/drop validation, source opening, keyboard edits, project
  actions, Temporal workflow data, and generated clip rendering remain unchanged.

## Verification

- `TimelineEditor` tests prove track controls omit `Video track linked` while retaining media-state
  controls.
- Existing `EditorWorkspace` tests continue to prove split-project `setTrackEnabled` actions still
  run from the row headers.
- Browser QA checks the sample timeline at desktop and narrow widths for compact row headers without
  overlapping controls.

## Self-Review

- No schema, project file, Temporal, fal.ai, render, or persistence contract changes.
- The change removes only non-actionable UI chrome.
- Existing media-state controls remain accessible and keyboard reachable.
