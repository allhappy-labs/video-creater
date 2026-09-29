# Codex Mention Insert Intent Design

## Context

Palmier chat can act on `@` references in plain language and place media on the timeline. Video
Creater now resolves typed mentions and exposes an `Insert mention on timeline` action, but the
primary composer button still sends an EDL edit request even when the prompt clearly says to place
the mentioned asset.

## Goal

Make the Codex composer primary action route explicit `@media` placement prompts to the existing
validated timeline insertion path.

## Behavior

- When the prompt has a resolved non-current mention with insert capability and includes an explicit
  placement verb such as `insert`, `place`, `put`, `add`, or `drop`, plus timeline context such as
  `timeline`, `cut`, `sequence`, `after`, `before`, `start`, `end`, or `current shot`, the primary
  composer button changes from `Generate edit` to `Insert mention on timeline`.
- Pressing that primary action calls the existing `onInsertSelectedMedia` callback with the resolved
  mention id and records a `project_action` transcript row.
- In this state, the resolved mention card does not repeat the same insertion button.
- Prompts without clear insertion intent continue to send EDL-first edit requests using the resolved
  mention as the media target.
- The workspace still applies only validated project actions through Rust-owned project mutation.

## Non-Goals

- No freeform placement parser, track selection parser, or timecode parser.
- No automatic action while typing.
- No direct project-file mutation from Codex.
- No change to referenced generation or EDL proposal behavior for non-placement prompts.

## Verification

- `AgentPanel` tests prove explicit placement prompts use the insert callback and do not call
  `onGenerateEdit`.
- `AgentPanel` tests prove ordinary resolved mention prompts still call `onGenerateEdit`.
- `EditorWorkspace` tests prove the primary composer insertion path emits an `addItems` project
  action for the mentioned media.
