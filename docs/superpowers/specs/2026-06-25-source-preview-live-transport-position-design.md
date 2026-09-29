# Source Preview Live Transport Position

## Context

Palmier keeps source footage inspection close to timeline-native editing: opening a source, trimming, splitting, and iterating with an agent all depend on a clear source position. Video Creater now supports real source playback and frame or second stepping, but the transport still displays a static `00:00:00` position and a zero-width scrubber.

## Goal

Bind the source preview transport to the active media element so manual reviewers and agents can see the current source position while playing or stepping.

## Requirements

- The transport current-time readout reflects the active video, generated video, or audio element's `currentTime`.
- Whole-second positions render as `HH:MM:SS`.
- Fractional frame positions render with milliseconds, such as `00:00:00.042`.
- The preview scrubber `aria-valuenow` and filled width reflect current position divided by known duration.
- Duration comes from the media element when available and falls back to the source `durationLabel`.
- Step buttons and arrow shortcuts update the readout immediately after changing `currentTime`.
- Switching source media resets current position to zero and reuses the new source duration.
- Timeline-mode preview transport behavior stays unchanged.

## Non-Goals

- Draggable source scrubbing.
- Timeline playhead progress.
- Persisting source current time to project files.
- Frame-number display.

## Test Plan

- Add a component test for `timeupdate` updating current time and scrubber progress.
- Add a component test for step-button updates to the fractional source time readout.
- Run the focused preview panel test file.
- Run full frontend tests, lint, build, whitespace checks, placeholder scan, secret scan, and browser QA.
