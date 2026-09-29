# Codex Selected Clip Trim Range Design

## Context

Palmier keeps timing edits close to the timeline and its assistant. Its docs state that chat can
iterate on timing without leaving the project, and that connected agents can trim, split, reorder,
and adjust clips while seeing full project context. Video Creater now shows the selected timeline
clip in the Codex rail and can split it from that context. Manual trim already exists in Source
Inspector and flows through the validated `trimItems` project action. Codex still cannot tighten a
selected clip's start or end from the chat rail.

## Goal

Add a compact `Trim range` control to the Codex selected timeline clip block so the user can shorten
the selected clip in context while reusing the existing project action path.

## Behavior

- Show `Trim range` only when a selected timeline clip is present and a trim callback is available.
- Default the draft start and end to the selected clip's current timeline start and end.
- Allow only inward trims in this slice:
  - draft start must be finite and greater than or equal to the current clip start;
  - draft end must be finite and less than or equal to the current clip end;
  - draft end must be greater than draft start.
- When the selected clip has both `sourceIn` and `sourceOut`, preserve source alignment:
  - new `sourceIn` is old `sourceIn + (draftStart - oldStart)`;
  - new `sourceOut` is old `sourceOut - (oldEnd - draftEnd)`;
  - new duration is `draftEnd - draftStart`.
- When the selected clip lacks a complete source range, submit only `startSeconds` and
  `durationSeconds`, matching the existing inspector behavior.
- Reset the draft start and end when the selected clip changes.
- Keep the existing `Mention source` and `Split clip` controls available.

## Non-Goals

- No ripple trim or gap closing.
- No extending a clip earlier or later than its current range.
- No source-duration lookup from the media library.
- No new Rust project action or schema.
- No autonomous agent proposal planner in this slice.
- No changes to Temporal, fal.ai, render, or generation workflows.

## Validation

- `AgentPanel` renders the trim controls, defaults to the selected clip range, rejects out-of-range
  edits, and calls the trim callback with aligned `startSeconds`, `durationSeconds`, `sourceIn`, and
  `sourceOut`.
- `EditorWorkspace` wires the Codex trim control to the existing split-project `trimItems` path.
- Focused tests pass before full test, lint, browser QA, and diff checks.
