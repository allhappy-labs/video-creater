# Codex Generated Timeline Reference Mentions Design

## Context

Palmier makes generation references usable while editing: AI clips expose their prompt, first frame,
last frame, and references, and chat accepts `@` mentions when the editor wants a new generation to
match a look, subject, or frame. Video Creater now passes generated reference metadata into the
Codex selected timeline clip context, but the `AI clip` subsection only shows a reference count.
The editor cannot insert those references into the Codex prompt without switching to the selected
source panel.

## Goal

Let the editor mention first-frame, last-frame, and reference media directly from a selected
generated timeline clip in the Codex rail.

## Behavior

- When the selected timeline clip resolves to a generated asset and has references, show compact
  reference mention chips in the existing `AI clip` subsection.
- Each chip shows:
  - reference role, such as `First frame`, `Last frame`, or `Reference`;
  - `@<mediaId>`;
  - a compact label from the resolved media filename when available.
- Clicking a chip inserts that media mention into the Codex prompt using the same insertion behavior
  as selected source references.
- Preserve the existing reference count, prompt block, rerun button, and variation button.
- Hide the reference chip group when the generated timeline clip has no resolved references.

## Non-Goals

- No new project schema, Rust action, Temporal workflow, fal.ai behavior, or MCP contract.
- No thumbnail rendering in the Codex rail for this slice.
- No source reveal behavior; Source Inspector and Media Bin remain the richer provenance surfaces.

## Validation

- `AgentPanel` tests prove selected generated timeline clip references render as mention chips and
  insert into the Codex prompt.
- `EditorWorkspace` tests prove references resolved from a selected generated timeline clip are
  visible in the Codex selected clip block.
- Existing selected source reference mention tests continue passing.
