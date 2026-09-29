# Timeline Canvas Click Seek Design

## Context

Palmier-style editing keeps the playhead directly manipulable: editors can click the timeline body to inspect a moment, then split, trim, add text, or generate at that position. Video Creater currently exposes a visible playhead and ruler tick buttons, but empty track space does not seek. That makes manual edit review feel less direct than a desktop editor.

## Requirements

- Clicking empty timeline canvas space should move the playhead to the clicked time.
- Seeking should use the current timeline zoom and clamp to the project duration.
- Clicking timeline clips, selected clip action buttons, resize handles, or toolbar controls must not accidentally seek.
- Existing ruler tick seek behavior, drag/drop target behavior, clip selection, clip movement, and source trim actions must remain unchanged.
- The timeline canvas needs an accessible label so tests and assistive tooling can identify the seek surface.

## Design

- Add a labelled `Timeline canvas` group to the existing timeline grid container.
- On canvas click, ignore events that originate from a button or interactive control.
- Convert `clientX` to seconds using the canvas bounding rect and current `pixelsPerSecond`.
- Clamp the computed seconds with the same playhead clamping behavior already used by the playhead marker.
- Stop propagation from clip buttons and selected dock controls where they are already interactive so selection and edit actions remain predictable.

## Verification

- Add a timeline-editor test that clicks empty canvas space and verifies the playhead/readout update to the computed time.
- Add a regression test proving clicking a clip still selects it without moving the playhead.
- Run the focused timeline-editor tests.
- Run `pnpm lint`.
- Browser-smoke the timeline by clicking empty track space and verifying the playhead moves without breaking clip selection.
