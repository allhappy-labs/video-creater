# Timeline Start End Transport Buttons Design

## Context

Palmier keeps basic timeline navigation visible in the editor chrome: the keyboard can move quickly, but transport-style controls also make start/end jumps discoverable for mouse-first editing. Video Creater now supports `Home` and `End` keyboard jumps, but the timeline toolbar only shows the current playhead readout beside zoom controls.

## Requirements

- The timeline toolbar exposes compact icon buttons for jumping the playhead to the timeline start and end.
- The start button moves the playhead to `0`.
- The end button moves the playhead to the clamped project duration.
- The start button is disabled at `0`; the end button is disabled at the timeline duration.
- The buttons update the same playhead state used by keyboard shortcuts, split, trim, and add-text actions.
- Existing zoom controls, selection tools, source mark buttons, and keyboard shortcuts keep their behavior.

## Design

- Add two icon-only buttons in the timeline toolbar playhead cluster, immediately before the current time readout.
- Use lucide transport-style icons and explicit accessible labels: `Jump timeline playhead to start` and `Jump timeline playhead to end`.
- Reuse `clampedPlayheadSeconds` and `clampPlayheadSeconds` for disabled state and end-jump clamping.
- Keep this local to `TimelineEditor`; no project-file, backend, Temporal, or render changes are needed.

## Verification

- Add a `TimelineEditor` test that the toolbar buttons jump to end and back to start.
- Add toolbar coverage for start/end button enabled and disabled states.
- Run the focused timeline toolbar test, then the full timeline-editor test file and TypeScript checks.
- Browser-smoke the editor toolbar and verify the buttons update the playhead label/readout without console errors.
