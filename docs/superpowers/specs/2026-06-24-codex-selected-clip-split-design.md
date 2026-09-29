# Codex Selected Clip Split Design

## Context

Palmier's docs describe a connected agent that can generate media, place it on the timeline, and
perform native timeline edits such as trimming, splitting, reordering, and adjusting clips. Video
Creater already exposes split editing in the manual timeline and Source Inspector, and the Codex
rail now shows the selected timeline clip. The missing bridge is a direct Codex-side action that
uses that selected clip context without asking the editor to move to another panel.

## Goal

Let the user split the currently selected timeline clip from the Codex rail, using the existing
timeline split action path and keeping the selected clip's timing visible beside the control.

## Behavior

- In the `Selected timeline clip` block, show a compact `Split at` numeric control when a source
  timeline clip is selected and a split callback is available.
- Default the value to the selected clip midpoint:
  `timelineStartSeconds + durationSeconds / 2`.
- Enable `Split clip` only when the entered time is finite and strictly inside the selected clip's
  timeline range.
- Invoke the existing split action with the selected `itemId` and the entered timeline seconds.
- Keep `Mention source` available for source-backed clips.
- Reset the draft split time when the selected clip changes.

## Non-Goals

- No new project schema.
- No new Rust action, reducer behavior, or split file layout.
- No agent planning/autonomous split proposal in this slice.
- No reorder, ripple edit, trim, or delete control from Codex in this slice.
- No changes to Temporal, fal.ai, render, or generation workflows.

## Validation

- `AgentPanel` renders the split control for selected timeline clips, defaults it to the midpoint,
  validates boundary values, and calls the split callback with the selected item id and entered
  split time.
- `EditorWorkspace` wires the Codex split control to the existing split-project action path.
- Focused workspace tests pass before full test, lint, browser QA, and diff checks.
