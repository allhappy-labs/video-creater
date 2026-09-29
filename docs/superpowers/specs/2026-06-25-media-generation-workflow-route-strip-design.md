# Media Generation Workflow Route Strip

## Problem

Palmier makes generation feel native to the editor: the user composes media, chooses references, and sees the result appear in the media library or timeline. Video Creater already queues media generation through Temporal-backed project actions, but the media generation composer does not show the workflow route before submission.

## Goal

Add a compact route strip to the media generation composer that shows where the queued generation will run and what provider path it will use.

## UI Contract

When the composer is open, show a `Generation workflow route` region near the submit footer with:

- Workflow: `VideoCreaterGenerateMediaWorkflow`
- Queue: `video-creater-workflows`
- Provider: the selected model provider
- Placement: `Library` or `Timeline`
- Mode: `Mock worker` while development queuing remains mock-backed

The strip must stay compact, use existing muted editor styling, and avoid explaining implementation internals beyond the queue route a user needs to trust what will happen.

## Behavior

- Switching Image/Video/Audio updates the provider label from the selected model.
- Switching Library/Timeline updates the destination label.
- The route strip is informational and does not change the queued request payload.

## Tests

Add MediaBin tests that verify:

- The route strip appears when the composer is open.
- It shows the Temporal workflow type, task queue, selected provider, destination, and mock worker mode.
- The destination updates when the user switches from Library to Timeline.
