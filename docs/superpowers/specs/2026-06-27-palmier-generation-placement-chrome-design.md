# Palmier Generation Placement Chrome Design

## Context

Video Creater's media generation composer still shows a prominent `Library / Timeline` placement
toggle in the media-panel flow. Palmier's media-panel composer keeps that area focused on media type,
references, prompt, model, cost, and send. Generation from the media panel should default into the
library without making the user choose a route before a generated-asset or agent context asks for one.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove unnecessary placement chrome from the media generation sheet while preserving agent-seeded
timeline or replacement placement metadata.

## Behavior

- The media-panel composer does not render the visible `Library / Timeline` placement toggle.
- The default request still queues to the project library with `placementIntent: "library"`.
- The active generation recipe still records `Library` as the route for screen-reader and testable
  metadata.
- When an agent or generated asset seeds timeline placement, the composer keeps compact placement
  status text such as `Will append to Video at 00:12.`.
- If a generated asset is being rerun as a replacement, existing replacement placement behavior
  remains unchanged.
- Mode selection, references, prompt, destination folders, model settings, costs, and generation
  request payload fields remain unchanged.

## Non-Goals

- No full composer redesign.
- No removal of agent-seeded timeline insertion support.
- No queue, Temporal, fal.ai, generated asset, or schema changes.

## Verification

- Update `MediaBin` coverage so the media-panel composer has no visible placement toggle but still
  submits `placementIntent: "library"`.
- Keep seeded timeline coverage proving compact placement status and timeline payloads still work
  when the composer is opened from generated-asset context.
- Keep generation request tests passing.
- Browser QA the default composer and confirm the media-type pills are followed by the next relevant
  generation controls instead of a redundant placement switch.
