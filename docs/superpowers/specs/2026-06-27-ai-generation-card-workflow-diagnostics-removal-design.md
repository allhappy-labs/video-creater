# AI Generation Card Workflow Diagnostics Removal

## Context

Palmier's media browser treats generated assets as visual source cards with concise status. Video
Creater's AI generation cards still show Temporal workflow internals such as workflow type, task
queue, run ID, and activity count. Those details are useful for queue inspection, but they make the
media browser read like an infrastructure panel instead of a compact editor library.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep AI generation cards focused on the generated asset, prompt, destination, output actions, and a
human-readable workflow status while moving Temporal diagnostics out of the media browsing surface.

## Behavior

- AI generation cards still show matching workflow status such as `Workflow running - generate media`.
- AI generation cards no longer show workflow type, task queue, run ID, activity count, or render
  workflow metadata.
- The dedicated project workflow queue, Codex workflow activity, serialized project jobs, and
  Temporal start/run records remain unchanged.
- Generated asset previews, output selection, replacement, insertion, folder destination, and
  completion actions remain unchanged.

## Verification

- `MediaBin` tests assert generation cards keep the human workflow status but omit Temporal routing
  details.
- Existing project timeline and Codex workflow activity coverage continue to verify diagnostic
  surfaces.
- Browser QA confirms the AI generations list is visually compact and no longer shows workflow route
  text on the card.
