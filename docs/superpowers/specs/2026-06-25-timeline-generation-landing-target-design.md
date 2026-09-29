# Timeline Generation Landing Target

## Context

Palmier presents generation as an editing action: generated video can land on the timeline, and
agents can place generated assets directly into the edit. Video Creater already records
`placementIntent: "timeline"` and the mock completion path appends completed outputs to the first
unlocked compatible timeline track. The Media generation drawer still describes this as a generic
future insertion instead of showing the actual track and time that will be used.

Palmier reference: https://www.palmier.io/docs

## Goal

When the editor chooses `Timeline` placement, show the timeline landing target before queueing so
the generation recipe feels like a concrete edit operation.

## Requirements

- `EditorWorkspace` computes landing targets from the current timeline:
  - `video` and `image` generation append to the first unlocked `video` track;
  - `audio` generation appends to the first unlocked `audio` track;
  - the append time is the end of the target track's latest item, rounded like timeline actions.
- `MediaBin` accepts landing target metadata and displays it only when `Timeline` placement is
  selected.
- The placement note says `Will append to <track> at <time>` when a target exists.
- The active generation recipe includes a `Timeline target` row with the same track and time when a
  target exists.
- If no compatible unlocked track exists, the placement note and recipe show `No unlocked <kind>
  track available`.
- Queue payloads, Temporal start requests, mock completion actions, generated asset metadata, and
  fal.ai model settings are unchanged.

## Non-Goals

- No selectable track picker.
- No playhead insertion, overwrite, ripple, or range replacement.
- No new project schema field or Temporal activity.
- No change to the existing completion insertion action.

## Test Plan

- Add a MediaBin component test proving the Timeline placement note and recipe show the provided
  video landing target.
- Add a MediaBin component test proving audio mode uses the provided audio target and missing target
  copy.
- Add an EditorWorkspace test proving the media generation drawer receives the current video track
  append target.
- Run focused tests, full frontend tests, type checks, build, browser QA, whitespace checks,
  placeholder scan, and secret scan.
