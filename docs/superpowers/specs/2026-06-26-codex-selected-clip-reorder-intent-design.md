# Codex Selected Clip Reorder Intent Design

## Context

Palmier's connected-agent workflow lets an agent reorder clips directly on the timeline. Video
Creater already exposes selected-clip `Earlier` and `Later` actions in the Codex context panel and
submits them through validated `reorderItems` project actions, but a user still needs to leave the
composer to press those buttons.

## Goal

Route explicit Codex prompts such as `move the selected clip earlier` or `move selected clip later`
through the existing selected-clip reorder action.

## Behavior

- When a selected clip has a valid reorder context and the prompt clearly requests moving the
  selected clip `earlier` or `later`, the primary composer action changes to
  `Move selected clip earlier` or `Move selected clip later`.
- Pressing the action calls the existing `onReorderSelectedTimelineClip` callback with the same
  reorder update used by the manual context buttons.
- The action is available only when the requested direction is valid for the selected clip position
  and the track is unlocked.
- The chat transcript records a `project_action` tool call with the selected clip id and requested
  direction.
- Non-reorder prompts keep the existing EDL generation, split, insertion, and blocked-generation
  behavior.

## Non-Goals

- No arbitrary index, track, time, or multi-clip reorder parser.
- No direct project-file mutation from Codex.
- No changes to the manual selected-clip `Earlier` and `Later` buttons.

## Verification

- `AgentPanel` tests prove explicit earlier/later prompts route through the reorder callback instead
  of `onGenerateEdit`.
- `EditorWorkspace` tests prove the prompt path emits a validated `reorderItems` project action
  through the split-project command.
