# Generation Queue Target Badge Design

## Context

Palmier's generation flow keeps timeline placement visible at the point of action. Video Creater
shows the timeline landing target in the composer body and active recipe, but the sticky
`Queue generation` button still compresses that to a generic `Timeline` badge. Once the user has
scrolled, the final action does not say where the generation will land.

Palmier reference: https://www.palmier.io/docs

## Goal

Show the concrete timeline target in the compact sticky queue action when timeline placement has a
compatible track.

## Behavior

- Library placement keeps the existing `Library` queue badge.
- Replacement placement keeps the existing replacement placement label.
- Timeline placement with no compatible target keeps the existing `No target` readiness state.
- Timeline placement with a compatible target shows a `Generation queue target` badge using the
  target track and placement time, for example `Video 00:04`.
- The target badge stays inside the existing `Queue generation` button with readiness and estimate.
- Generation payloads, placement intent, Temporal route metadata, references, and provider behavior
  remain unchanged.

## Non-Goals

- No drag-to-place generation target picker.
- No timecode parser or manual target override.
- No project schema, Rust action, Temporal workflow, or fal.ai provider changes.

## Verification

- Add a `MediaBin` test proving timeline placement with a target shows `Generation queue target`
  as `Video 00:04` in the sticky queue action.
- Existing queue readiness, target-gating, and generation payload tests continue to pass.
- Browser-smoke the media generation composer with timeline placement selected and verify the
  sticky queue badges fit without overlap.
