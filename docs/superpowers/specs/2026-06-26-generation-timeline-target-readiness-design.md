# Generation Timeline Target Readiness Design

## Context

Palmier treats generated media as timeline-native: the editor shows where a generation will land
before the user queues it. Video Creater already shows a `No unlocked <kind> track available`
message when timeline placement is selected without a compatible target, but the compact sticky
queue bar still reports `Ready` once the prompt is filled and allows the request to be queued.

Palmier reference: https://www.palmier.io/docs

## Goal

Make timeline-placement readiness visible in the sticky queue bar and prevent queueing when there is
no compatible unlocked target track.

## Behavior

- Library and replacement placement keep the existing prompt-required readiness behavior.
- Timeline placement requires both a non-empty prompt and a compatible target from
  `timelineGenerationTargets`.
- When timeline placement has no target, the queue readiness badge reads `No target` and the queue
  button is disabled.
- When the user switches to a mode with a compatible timeline target, the badge returns to `Ready`.
- The scroll-body placement note and active recipe target remain unchanged.
- Generation payload shape, Temporal route metadata, fal.ai/mock provider behavior, and placement
  intent values remain unchanged.

## Non-Goals

- No automatic track creation.
- No new placement parser, timecode parser, project schema field, or Rust action.
- No changes to library placement, replacement placement, or selected-source variation queueing.

## Verification

- Add/update `MediaBin` tests proving timeline placement without a target disables the queue action
  and does not call `onGenerateMedia`.
- Add a regression that switching to a generation mode with a compatible timeline target restores
  the `Ready` badge.
- Existing generation payload tests continue to pass.
- Browser-smoke the media generation composer with timeline placement selected and confirm the
  compact queue bar shows the target state without overlap.
