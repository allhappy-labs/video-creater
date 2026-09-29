# Palmier Timeline Navigation Cluster Design

## Context

Palmier keeps the timeline toolbar focused on edit tools and zoom while playback and navigation controls live in the viewer or keyboard/ruler interactions. Video Creater had accumulated start/end, edit-point jump, step, and inline timecode controls inside the timeline toolbar, making the manual editing strip visually heavier than the Palmier reference.

This supersedes the visible toolbar placement from the earlier timeline transport specs. The underlying navigation behaviors remain required through keyboard shortcuts, ruler seeking, dragging, and the playhead badge.

## Requirements

- The timeline toolbar keeps edit actions: undo, redo, select, split, source marks, text overlay, and selected-clip context.
- The toolbar keeps timeline zoom controls: zoom out, fit, slider, zoom in, and zoom percentage.
- The toolbar does not render start/end, previous/next edit point, step backward/forward, or inline playhead timecode controls.
- The playhead time remains visible in the canvas playhead badge.
- Keyboard shortcuts remain functional:
  - `Home` / `End` jump to timeline start/end.
  - `PageUp` / `PageDown` jump between edit points.
  - `ArrowLeft` / `ArrowRight` step by the snap interval.
- Ruler seeking and playhead dragging remain functional.

## Validation

- Component tests assert the transport/navigation controls are absent from the toolbar.
- Existing keyboard, ruler, drag, and playhead badge tests continue to pass.
- Browser QA confirms the toolbar visually reads as editing tools plus zoom, without a second transport cluster.
