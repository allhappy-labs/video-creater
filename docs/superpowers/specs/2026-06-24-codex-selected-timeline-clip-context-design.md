# Codex Selected Timeline Clip Context Design

## Context

Palmier's chat works beside the timeline and can operate on the editor's current context. Its docs state that a connected agent sees the full project context and can trim, split, reorder, adjust, rerun, and tweak clips. Video Creater already shows selected media and generated-source context in the Codex rail, but selecting a timeline clip does not make that clip explicit in chat. The rail can keep showing a stale selected media source while the actual user selection is a timeline clip.

## Goal

When a source-backed timeline clip is selected, the Codex rail should show a compact `Selected timeline clip` context block with timing and source information. The block should let the user mention the clip's source media in the prompt without leaving the chat rail.

## Behavior

- Add a Codex timeline context only for selected source clips, not captions, templates, or text overlays.
- Show:
  - clip label and kind;
  - clip id;
  - timeline start, end, and duration;
  - source media id and source range when available.
- Add a `Mention source` button that inserts `@<sourceMediaId>` into the Codex prompt using the same mention insertion behavior as generated-source references.
- Update the Codex context tool summary from `context @media` to `clip <clipId>` when a timeline clip is selected, while keeping the existing target media id for edit generation.
- Keep existing selected media/generation actions intact. The selected clip block is additive and does not replace the generated source block.

## Non-Goals

- No new project schema.
- No new Rust action or MCP contract.
- No direct trim/split/reorder buttons from Codex in this slice.
- No changes to Temporal, fal.ai, render, or generation queues.

## Validation

- `AgentPanel` renders selected timeline clip timing and inserts the source mention.
- `EditorWorkspace` passes the selected source clip context into the Codex rail when a timeline source clip is selected.
- Caption selections do not show the selected timeline clip block.
