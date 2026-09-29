# Codex Mentioned Source Insert Design

## Context

Palmier-style chat editing lets users reference media with `@` mentions and ask the agent to place
or transform those sources on the timeline. Video Creater already resolves typed media mentions and
can insert the currently selected source through validated project actions, but a resolved typed
mention only exposes referenced generation. Users still need to manually select the mentioned media
before placing it.

## Goal

Let the Codex composer insert a resolved `@media` mention onto the timeline when the mentioned media
has a compatible unlocked target track.

## Behavior

- Mention targets expose whether they can be inserted on the timeline.
- A resolved non-current mention with insert capability shows `Insert mention on timeline`.
- Pressing the action calls the existing selected-media insertion callback with the mentioned media
  id.
- The workspace computes insert capability with the same `mediaTimelineAction` helper used for
  selected-source insertion.
- The insertion still emits validated project actions only; Codex does not mutate project files
  directly.
- The chat transcript records the placement as a `project_action` tool call so the user can audit
  what the agent changed.

## Non-Goals

- No natural-language placement parser.
- No replacement of selected clips from typed mentions.
- No new Rust action schema or Temporal workflow.
- No automatic insertion when the user submits a normal edit prompt.

## Verification

- `AgentPanel` unit tests cover the resolved mention insertion button and callback payload.
- `EditorWorkspace` tests cover a typed imported media mention emitting an `addItems` project action
  on the compatible timeline track.
