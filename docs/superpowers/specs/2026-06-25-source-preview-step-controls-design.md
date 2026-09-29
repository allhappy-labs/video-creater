# Source Preview Step Controls Design

## Context

Palmier's editor screenshots show a transport row with step backward, play, and
step forward controls directly under the viewer. Video Creater already renders
the same step buttons in the preview transport, but they are inert. That weakens
manual source inspection because an editor can play/pause media but cannot nudge
to adjacent frames or nearby audio timing from the transport.

## Goals

- Make preview transport step backward and step forward buttons act on the active
  source media element.
- Step video and generated video sources by one frame using `fpsLabel` when it is
  available.
- Step audio sources by one second.
- Clamp stepping to the media element's available `[0, duration]` range when a
  finite duration exists.
- Disable step buttons when the active viewer cannot step source media.

## Non-Goals

- Timeline playback or timeline playhead stepping.
- Frame-accurate decoding guarantees beyond setting the media element's
  `currentTime`.
- Scrubber dragging or current-time display updates.
- Stepping still images.

## Interaction

- `Preview step backward` subtracts the source step size from
  `mediaElement.currentTime`.
- `Preview step forward` adds the source step size.
- For `fpsLabel: "24 fps"`, the video step is `1 / 24` seconds.
- If the FPS label is missing or invalid for a video/generated source, the step
  falls back to `1 / 24` seconds.
- Audio uses a one-second step because audio preview has no frame rate.
- Step buttons expose titles: `Step source backward` and `Step source forward`.

## Verification

- Unit test: clicking forward on a 24 fps video source increments
  `currentTime` by one frame.
- Unit test: clicking backward clamps to zero.
- Unit test: audio source stepping advances by one second.
- Unit test: image source step buttons are disabled and do not mutate playback.
- Browser QA: the transport exposes enabled source step buttons in source mode
  and the layout remains stable.
