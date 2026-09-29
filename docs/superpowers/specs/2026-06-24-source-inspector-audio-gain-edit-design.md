# Source Inspector Audio Gain Edit Design

## Context

Palmier's timeline screenshots treat audio clips as editable timeline objects with waveform, level,
and fade intent visible in the edit. Video Creater now renders audio waveform and fade-out metadata,
and Source Inspector can edit `properties.fadeOutSeconds`. Editors and agents still lack a validated
way to adjust an individual audio clip's level without hand-editing project JSON.

## Goal

Add a compact Source Inspector control and validated project action for editing an audio clip's gain
in decibels.

## Behavior

- When the selected timeline item is an `audio_clip`, Source Inspector shows `Volume dB` near
  `Fade out`.
- The input defaults to the current numeric `properties.volumeDb` value, or blank when absent.
- Applying the value sends a dedicated `updateAudioVolume` project action.
- The action accepts a finite value from `-60` through `24` dB.
- A blank value resets the clip to unity by removing `properties.volumeDb`.
- A value of `0` is stored as `0` so agents can explicitly state that the clip was normalized to
  unity gain.
- The action rejects missing items, locked tracks, non-audio items, non-finite values, and
  out-of-range values.
- Timeline audio clip metadata includes a compact `volume +NdB` / `volume -NdB` cue when the value
  is present.

## Non-Goals

- No GStreamer/ffmpeg audio mix changes in this slice.
- No keyframed volume automation.
- No fade-in support.
- No draggable gain handles.
- No generic arbitrary-property mutation action.

## Verification

- `SourceClipInspector` tests prove audio clips show `Volume dB`, submit numeric gain, and submit
  blank resets as `null`.
- `EditorWorkspace` tests prove applying the inspector value sends `updateAudioVolume` to the
  split-project action command.
- `TimelineEditor` tests prove audio clips with `volumeDb` expose compact level metadata.
- Rust reducer tests prove the action stores valid values, removes blank resets, and rejects
  non-audio, locked-track, and out-of-range edits.
