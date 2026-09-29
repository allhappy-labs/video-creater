# Codex Imported Source Insert Design

## Context

Palmier chat can generate assets and place them on the timeline, and connected agents can work with
imported or generated clips in the same project context. Video Creater already lets Codex insert a
selected generated output from the source block. Imported media remains excluded, so a user can
select an imported source in the library, see it in Codex context, and still has to leave the chat
rail to place it on the timeline.

## Goal

Let the Codex selected-source block insert imported visual or audio media onto the timeline through
the same validated project-action path used for generated outputs.

## Behavior

- `AgentSelectedMediaContext.canInsertOnTimeline` is true for imported video, image/generated, and
  audio media when a compatible unlocked target track exists.
- Pressing `Insert on timeline` for imported visual media emits an `addItems` action to the first
  unlocked video track.
- Pressing `Insert on timeline` for imported audio media emits an `addItems` action to the first
  unlocked audio track.
- The inserted item starts at the end of the target track and uses `sourceIn: 0` and `sourceOut`
  equal to the inserted duration.
- Generated output insertion behavior, placeholder replacement, and replacement actions remain
  unchanged.

## Non-Goals

- No prompt-driven automatic placement parser.
- No replacement of selected timeline clips with imported media.
- No new Rust action, schema field, Temporal workflow, or direct project mutation.
- No image-still duration preference UI; images use the existing duration metadata when present.

## Verification

- `EditorWorkspace` tests prove selecting imported video or audio in the media bin exposes the
  Codex insertion action and applies an `addItems` action to the compatible track.
- Existing generated output insertion tests continue to pass.
