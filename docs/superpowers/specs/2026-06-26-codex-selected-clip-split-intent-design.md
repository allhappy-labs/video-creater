# Codex Selected Clip Split Intent Design

## Context

Palmier's connected-agent workflow lets agents trim, split, reorder, and adjust clips with full
project context. Video Creater already exposes selected-clip split controls in the Codex context
panel and applies them through validated project actions, but a user still has to leave the chat
composer and use the numeric split control. A clear prompt such as `split the selected clip at 6s`
should operate the selected timeline clip directly.

## Goal

Route explicit Codex split prompts for the selected timeline clip through the existing split action.

## Behavior

- When a selected timeline clip is available, the track is unlocked, and the prompt includes an
  explicit split verb with a numeric second value, the primary composer action changes to
  `Split selected clip`.
- Pressing the action calls `onSplitSelectedTimelineClip(selectedItemId, splitSeconds)` using the
  parsed timeline second value.
- The split second must be inside the selected clip range, not at either edge.
- The chat transcript records the action as a `project_action` tool call with the selected clip id
  and split second.
- Prompts without a valid split intent continue to use the existing EDL generation or blocked
  generation path.

## Non-Goals

- No frame, timecode, beat, percentage, or relative phrase parser.
- No multi-clip split.
- No direct mutation of project files from the React panel.
- No change to the manual split control.

## Verification

- `AgentPanel` tests prove an explicit split prompt routes through the split callback instead of
  `onGenerateEdit`.
- `EditorWorkspace` tests prove the prompt path emits a `splitItems` project action through the
  existing split-project command.
