# Preview Spacebar Playback Shortcut Design

## Context

Palmier's editor screenshots show a conventional video-editor playback surface:
viewer, transport, timeline, and keyboard-first manual editing controls all stay
close together. Video Creater already supports real source preview playback for
video, generated video, and audio through the large preview button and the
transport play button. The missing manual-editing affordance is the standard
spacebar play/pause shortcut.

## Goals

- Add a guarded Space shortcut that toggles playback for the active source
  preview.
- Reuse the existing `toggleSourcePlayback` path so button clicks and keyboard
  playback share the same media element behavior.
- Keep typing safe. The shortcut must not fire when focus is inside an input,
  select, textarea, or content-editable element.
- Expose the shortcut on existing play/pause controls through titles.

## Non-Goals

- Implementing timeline playback, scrubbing, or frame-accurate preview.
- Adding global shortcut preferences or a command palette.
- Playing image-only source previews.
- Changing source tab or timeline selection behavior.

## Interaction

- Pressing Space or `Spacebar` toggles source playback when the active viewer is
  `source`, the selected source has a preview URL, playback has not failed, and
  the source kind is `video`, `generated`, or `audio`.
- The shortcut prevents page scroll only when it actually attempts to toggle
  playback.
- Modified Space combinations are ignored.
- The large preview button and transport play button expose
  `Play preview (Space)` or `Pause preview (Space)` as their title.

## Verification

- Unit test: Space starts source playback through the existing media element
  `play()` path.
- Unit test: Space pauses when the source preview is already playing.
- Unit test: Space does nothing while focus is inside an editable field.
- Existing click-based playback tests continue to pass.
- Browser QA: preview play controls expose the Space title and the layout remains
  unchanged.
