# Mode-Aware Generation Settings Design

## Context

Palmier's generation composer treats images, video, and audio as distinct creation modes inside one
compact media-panel surface. Video Creater already has Image, Video, and Audio modes plus fal.ai
model defaults, but the output settings panel still shows `Duration` for image generation and the
summary combines image aspect ratio with a video-like duration. The submitted project action already
stores image `durationSeconds` and `fps` as `null`, so the UI should match the data contract.

## Goal

Make the media generation composer mode-aware enough that image generation feels like a still-image
workflow while preserving the existing project action payloads.

## Behavior

- Video mode keeps `Duration`, `Aspect`, and `Size` controls and summarizes timing as
  `<aspect> - <duration>`.
- Image mode hides the `Duration` control, keeps `Aspect` and `Size`, and summarizes timing as
  `Still image`.
- Image mode keeps the Reference slot and queues `fal-ai/flux/schnell` with settings
  `{ width, height, durationSeconds: null, fps: null, aspectRatio }`.
- Audio mode keeps only the `Duration` control, hides visual `Aspect` and `Size` controls, and
  summarizes output as `Audio`.
- Switching modes does not erase the current prompt or name.

## Non-Goals

- No new Rust action, Temporal workflow, fal.ai provider code, schema migration, or generated output
  media model change.
- No redesign of the full generation drawer.
- No real fal.ai e2e run in this slice.

## Validation

- Media Bin tests prove image mode does not expose `Generation duration`, still exposes aspect and
  size, and shows `Still image` in the submit summary.
- Existing image generation tests continue proving the queued request uses `fal-ai/flux/schnell` and
  null duration/fps settings.
- Existing video and audio generation tests continue passing.
