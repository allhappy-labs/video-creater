# Source Preview Playback Controls

## Context

Palmier keeps real source footage available in the center viewer while the media bin, timeline,
assistant, and inspector remain in context. Its docs describe double-clicking source footage in the
viewer and iterating on prompts and timing without leaving the project. Video Creater already opens
local video, generated video, image, and audio sources in `PreviewPanel`, but the visible play
controls are still presentational. The source monitor can show media but cannot start or pause it
from the editor chrome.

## Goal

Make source preview playback controls operate on the currently opened local video or audio source
without changing project files, timeline playhead state, render behavior, or workflow records.

## Behavior

- When source mode is active and the selected source has a real video or audio preview element,
  `Play preview` and `Preview transport play` toggle that media element.
- The controls show `Pause preview` / `Preview transport pause` while playback is active.
- Pausing, media ending, media errors, switching source tabs, or returning to timeline mode resets
  playback state to paused.
- Image sources, synthetic fallback previews, and timeline mode keep non-mutating play controls.
- If browser media playback rejects the `play()` call, the controls remain paused and no project
  state is changed.
- The implementation remains UI-only. Preview URLs stay derived runtime state and are never written
  to project actions, generated assets, jobs, render reports, or split project files.

## Non-Goals

- No frame-accurate source monitor, scrubber, or timecode sync.
- No timeline compositing playback.
- No audio waveform extraction or thumbnail caching.
- No remote media loading.
- No Temporal workflow, fal.ai, render, export, or project schema changes.

## Tests

- `PreviewPanel` plays and pauses a real video source from the large preview button.
- `PreviewPanel` plays and pauses a real audio source from the transport play button.
- Switching source ids resets the visible control state to paused.
- Image and synthetic fallback previews do not call media playback APIs.

## Visual QA

- Desktop: open a real source preview and verify the large preview button and transport button
  remain aligned, labelled, and do not overlap source metadata.
- Narrow width: verify the controls and metadata wrap without clipping.
- Failure path: verify the synthetic fallback still renders after preview load failure.
