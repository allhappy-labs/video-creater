# Codex Selected Source Insert Design

## Context

Palmier's docs describe chat that can generate assets and place them on the timeline, while connected agents can place generated images, video, and audio directly in the project. Video Creater already lets the media bin insert completed generated outputs onto the timeline, and Codex now shows selected source provenance. The missing step is an agent-side action that lets the user act on the selected generated source from the Codex context.

## Goal

When the selected Codex source is a completed generated output that can be inserted, show a compact `Insert on timeline` action inside the selected source block. Pressing it should call the existing workspace insertion path and create the same validated `addItems` project action used by the media bin.

## Non-Goals

- No new project action type.
- No timeline insertion for imported media in this slice.
- No prompt-driven automatic placement or chat command parsing.
- No new backend protocol; this is a manual action exposed in the Codex surface.

## UI And Data Flow

`AgentSelectedMediaContext` gains a `canInsertOnTimeline` flag. `EditorWorkspace` sets it when the selected media id belongs to a generated asset output. `AgentPanel` receives an optional `onInsertSelectedMedia` callback.

When `selectedMediaContext.canInsertOnTimeline` is true and the callback exists, `AgentPanel` renders a small outline button labeled `Insert on timeline` in the selected source region. Clicking it passes `selectedMediaContext.mediaId` to the callback.

`EditorWorkspace` wires the callback to `insertGeneratedOutputOnTimeline`, preserving the existing validation and project-action behavior.

## Testing

- `AgentPanel` calls `onInsertSelectedMedia(mediaId)` when the source action is clicked.
- `EditorWorkspace` selecting `sample-generated-output` shows the Codex action.
- Clicking the Codex action creates the same `addItems` action for generated output insertion.

## Future Work

- Let Codex insert imported media by choosing the correct target track.
- Support replacing the selected timeline clip from the Codex source block.
- Add natural-language chat commands for placement once Codex has multi-turn command parsing.
