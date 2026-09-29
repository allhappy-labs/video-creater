# Timeline Audio Waveform Clips

## Context

Palmier's timeline makes audio clips legible at a glance: audio tracks show waveform structure, clip boundaries, and fade/level intent instead of plain blocks. Video Creater already has audio tracks and compact track controls, but audio items render as flat green buttons and the sample project does not demonstrate an audio lane.

## Goal

Render timeline audio clips with a compact waveform treatment so the manual editor better communicates sound presence, timing, and clip identity.

## Behavior

- Audio timeline items render a waveform-style visual inside the clip body.
- The waveform is deterministic and uses `properties.waveformPeaks` when it is present as a numeric array.
- If no valid peaks are present, the waveform falls back to deterministic synthetic bars based on the clip id and duration.
- The clip label and source range remain visible above the waveform.
- Selected, locked, move, resize, and toolbar behavior remain unchanged.
- The sample timeline includes an audio clip so the default workspace shows the audio lane in use.

## Non-Goals

- No audio decoding, ffprobe, waveform extraction, or media cache work.
- No timeline schema migration; waveform peaks remain optional item properties.
- No new Tauri command or Temporal workflow.
- No fade handle editing in this slice.

## Tests

- Audio clips expose an accessible waveform region inside the timeline item.
- Audio clips use supplied waveform peak data when present.
- The sample timeline renders an audio clip in the audio lane.
