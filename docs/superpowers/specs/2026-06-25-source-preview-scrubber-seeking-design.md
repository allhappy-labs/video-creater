# Source Preview Scrubber Seeking

## Context

Palmier keeps source footage inspection in the same workspace as timeline-native edits and agent-controlled trimming. Video Creater now shows live source position in the preview transport, but the scrubber is still passive. Users can step by button or shortcut, yet they cannot jump directly to a source moment from the transport.

## Goal

Make the source preview scrubber a compact seek control for playable source media, while preserving passive timeline-mode behavior.

## Requirements

- Playable video, generated video, and audio source previews expose `Preview scrubber` as an accessible range slider.
- Moving the scrubber sets the active media element's `currentTime`.
- The transport current-time readout and slider value update immediately after seeking.
- Slider bounds use the media duration when available, falling back to the source `durationLabel`.
- The slider remains disabled when there is no playable source duration, such as image previews or timeline mode.
- Keyboard focus on the slider does not trigger document-level source step shortcuts.
- Existing step buttons and arrow shortcuts continue to work.

## Non-Goals

- Waveform thumbnails.
- Timeline playhead scrubbing.
- Persisting source current time in project files.
- Mark-in or mark-out controls.

## Test Plan

- Add a component test for seeking a video source through the scrubber.
- Add a component test that image source previews expose a disabled scrubber.
- Update source progress assertions for the slider contract.
- Run focused preview panel tests, full frontend tests, lint, build, browser QA, whitespace checks, placeholder scan, and secret scan.
