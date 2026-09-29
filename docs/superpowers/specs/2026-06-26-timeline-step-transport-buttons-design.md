# Timeline Step Transport Buttons Design

## Context

Palmier-style editing keeps timeline transport controls visible so mouse-first editors can inspect cuts without remembering every shortcut. Video Creater already supports ArrowLeft and ArrowRight quarter-second playhead stepping, and the toolbar now exposes start/end jumps, but the same fine stepping is still keyboard-only.

## Requirements

- The timeline toolbar exposes compact icon buttons for stepping the playhead backward and forward.
- The backward button subtracts the existing timeline snap interval.
- The forward button adds the existing timeline snap interval.
- Stepping clamps to `0` and the project duration.
- The backward button is disabled at `0`; the forward button is disabled at the timeline duration.
- The buttons update the same playhead state used by keyboard shortcuts, split, trim, and add-text actions.
- Existing start/end, zoom, selection, source mark, and keyboard shortcut behavior remains unchanged.

## Design

- Add two icon-only buttons around the timecode readout in the existing timeline transport cluster.
- Use explicit accessible labels: `Step timeline playhead backward` and `Step timeline playhead forward`.
- Reuse `timelineSnapSeconds`, `clampedPlayheadSeconds`, and `clampPlayheadSeconds`.
- Keep this local to `TimelineEditor`; no project-file, backend, Temporal, or render changes are needed.

## Verification

- Add a `TimelineEditor` test that the toolbar step buttons move forward by `0.25s` and back to `0`.
- Extend toolbar coverage for enabled and disabled button states.
- Run the focused toolbar transport tests, then the full timeline-editor test file and TypeScript checks.
- Browser-smoke the editor toolbar and verify the buttons update the playhead label/readout without console errors.
