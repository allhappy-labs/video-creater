# Source AI Edit Workflow Route Strip

## Problem

Palmier keeps source details and AI edit actions in the right inspector, so a user can select a clip, tweak or upscale it, and understand where the generation will go. Video Creater's source inspector can already queue referenced shots, variations, replacements, and upscales, but the AI Edit tab does not show the Temporal/fal route before submission.

## Goal

Add a compact `Source AI edit workflow route` strip to source inspector AI Edit surfaces. The strip should make queued generation feel native to the editor workflow and match the media generation composer route language.

## UI Contract

When the AI Edit tab is open and a queue action is available, show:

- Workflow: `VideoCreaterGenerateMediaWorkflow`
- Queue: `video-creater-workflows`
- Provider: `fal.ai`
- Placement: `Referenced shot`, `Variation`, `Replacement`, or `Upscale`
- Mode: `Mock worker`

For imported visual sources, the placement is `Referenced shot` when referenced generation is available and `Upscale` when only upscale is available. For generated sources, use `Variation`, `Replacement`, or `Upscale` based on the selected AI edit action context.

## Non-Goals

- Do not change queued request payloads.
- Do not add new provider choices.
- Do not expose provider credential values or start-request input payloads.

## Tests

Add source inspector tests that verify:

- Imported visual AI Edit shows the route strip with the generate-media workflow, task queue, fal.ai provider, mock-worker mode, and referenced-shot placement.
- Generated source AI Edit shows the route strip with the same workflow route and variation placement.
