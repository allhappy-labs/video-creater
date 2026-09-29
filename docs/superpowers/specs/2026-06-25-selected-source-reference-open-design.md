# Selected Generated Source Reference Open Design

## Problem

Palmier keeps generated media inspectable after creation: editors can inspect the prompt, first frame, last frame, and references used for any generation. Video Creater already exposes generated source references in the Codex selected source block, but those reference chips only insert `@media` mentions into the chat prompt. The user must switch surfaces to visually inspect the actual source media.

## Goals

- Let editors open selected generated source first-frame, last-frame, and reference media directly from the Codex rail.
- Preserve the existing quick mention action for prompt drafting.
- Reuse the existing source viewer tabs and Source Inspector instead of creating a new inspection surface.
- Keep the flow scoped to selected generated source references.

## Non-Goals

- No project schema or generation payload changes.
- No editing, replacing, or removing reference media from this block.
- No behavior change for non-generated selected sources.

## Design

In the Codex selected source block, generated reference rows use the same interaction model as selected generated timeline clip references:

- The primary reference chip is labelled `Open <role> <mediaId>` and opens the referenced media in the source viewer.
- A compact adjacent `@` button is labelled `Mention <role> <mediaId>` and inserts the prompt mention.
- `EditorWorkspace` wires the open action to `activateViewerSource(mediaId)`, which opens/selects the source tab and switches the preview to source mode.

This makes source-level generation provenance inspectable from the agent rail while keeping chat composition fast.

## Acceptance Criteria

- AgentPanel calls a selected-source reference open callback when a generated source reference chip is clicked.
- AgentPanel still inserts the same `@mediaId` mention through the compact mention action.
- EditorWorkspace opens the referenced media in the source viewer from the Codex selected source block.
- Existing selected source generation, variation, replacement, and timeline selected clip reference controls keep working.
