# Audio Fade-Out Timeline Visual Design

## Context

Palmier's timeline screenshots show music clips with waveform bodies and visible end fades. Video Creater already renders audio clips with waveform peaks and stores timeline items as text-editable JSON properties, but there is no visible representation for audio fade metadata. That makes music-bed endings look abrupt even when an agent or user stores fade intent in the project file.

## Goal

Render a compact fade-out ramp on audio timeline clips when the item has a positive `fadeOutSeconds` property.

## Behavior

- Audio timeline items read optional numeric `properties.fadeOutSeconds`.
- Values less than or equal to zero, non-finite values, and missing values render no fade ramp.
- Positive values render a right-edge fade overlay whose width is proportional to the clip duration and capped to the clip width.
- The fade overlay is accessible as `Fade out for <clip label>, <seconds> seconds`.
- The clip title metadata includes `fade out <seconds>` so project-file fade data is inspectable on hover.
- The sample timeline music bed includes a short fade-out value so the default workspace demonstrates the cue.

## Non-Goals

- No audio rendering or GStreamer mix change in this slice.
- No fade-in support.
- No draggable fade handles.
- No new Rust project action; `fadeOutSeconds` remains a text-file-editable timeline item property.

## Verification

- `TimelineEditor` tests prove audio clips render a fade-out ramp from `fadeOutSeconds`.
- `TimelineEditor` tests prove zero or invalid fade values do not render the ramp.
- Existing waveform, track-control, drag, trim, and resize tests continue to pass.
