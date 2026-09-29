# Codex Selected Source Replace

## Context

Palmier lets editors regenerate or choose an AI-generated clip and swap it into the timeline without leaving the project. Video Creater already supports generated-output replacement from the media bin and selected-source insertion from the Codex panel, but the Codex selected-source block cannot replace the currently selected timeline clip yet.

## Goal

When a generated output is selected as the Codex source and a timeline clip is selected, show a compact `Replace selected clip` action in the Codex selected-source block. Pressing it should use the existing validated `replaceTimelineItemWithGeneratedOutput` project action path.

## Behavior

- The action is only shown when the selected Codex source can be inserted on the timeline, a replacement target label is available, and a replacement callback is provided.
- The visible button label is `Replace selected clip`; the accessible label names the target and generated output.
- Clicking it passes the selected generated output media id to `EditorWorkspace.replaceSelectedClipWithGeneratedOutput`.
- Existing `Insert on timeline`, `Queue variation`, and `Queue referenced shot` actions remain available under their current conditions.

## Non-Goals

- No automatic replacement after a queued variation completes.
- No new Temporal workflow job; this is a synchronous manual timeline mutation through the existing project action validator.
- No new project action schema.

## Tests

- `AgentPanel` renders and calls a replacement callback when a selected generated source and replacement target are present.
- `EditorWorkspace` applies `replaceTimelineItemWithGeneratedOutput` when the Codex selected-source replacement action is clicked.
- `EditorWorkspace` clears the replacement target when the user selects a non-source timeline item before choosing a generated source.
