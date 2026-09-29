# Timeline Playhead Drag Scrub Design

## Context

Palmier-style manual editing keeps the playhead directly manipulable while reviewing cuts. Video
Creater can seek by clicking the ruler or empty timeline canvas, and can step the playhead with
arrow keys, but it cannot scrub the playhead by dragging across the timeline body.

## Goal

Let editors drag across empty timeline canvas space to move the playhead continuously without
selecting, moving, or trimming clips.

## Behavior

- Pointer down on empty timeline canvas begins a playhead scrub and seeks to the pointer time.
- Pointer move while scrubbing updates the playhead using current timeline zoom.
- Pointer up ends the scrub.
- The computed time is clamped to `[0, timeline.durationSeconds]`.
- Pointer events that start on clips, buttons, resize handles, selected-clip controls, or other
  interactive controls keep their existing behavior and do not start a scrub.
- Existing click-to-seek, ruler seeking, keyboard stepping, clip selection, clip drag, resize, and
  trim behaviors remain unchanged.

## Non-Goals

- No timeline playback engine.
- No persisted playhead position in project files.
- No draggable source-preview scrubber change.
- No project action, render, or Temporal workflow changes.

## Verification

- `TimelineEditor` test proves dragging empty timeline canvas moves the playhead and clamps by
  project duration.
- Existing canvas click, clip selection, drag, resize, trim, keyboard, and toolbar tests continue
  passing.
