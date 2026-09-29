# Generated Card Replacement Target

## Context

Palmier lets editors regenerate or tweak AI-generated clips and swap the result on the timeline
without leaving the project. Video Creater already records replacement queue intent as
`placementIntent: "replace:<timeline-item-id>"`, but AI generation cards only show a visible badge
for plain timeline placement.

Palmier reference: https://www.palmier.io/docs

## Goal

Make replacement-targeted generated assets visible in the AI generations list so editors can
distinguish queued replacements from ordinary library or timeline-targeted generations.

## Behavior

- If a generated asset has `placementIntent` beginning with `replace:`, its AI generation card shows
  a compact `Replacement target` badge.
- Existing `Timeline target` and folder `Destination` badges keep their behavior.
- The badge is informational only and does not change completion, replacement, or timeline insertion
  behavior.

## Non-Goals

- No project schema, Temporal workflow, or fal.ai provider change.
- No replacement target label lookup from the timeline item.
- No automatic selection or timeline navigation.

## Verification

- Media Bin test: replacement-targeted generated assets render `Replacement target` in the AI
  generations list.
- Existing generated-card output, timeline target, destination, and selected source tests continue
  passing.
