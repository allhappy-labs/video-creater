# Agent Tool Transcript Design

Palmier's assistant screenshots make agent work visible as compact tool rows such as
`get_timeline` and `generate_image` inside the chat stream. Video Creater already shows static
context tools and workflow activity, and selected-source queue actions write user/Codex transcript
messages. The missing polish is an in-chat tool-call row for the action the editor just triggered.

## Goal

When a selected-source queue action starts a media-generation workflow from the Codex rail, the
local transcript should show a compact `generate_media` tool row between the user's prompt and the
Codex acknowledgement.

## Behavior

- Queue variation, variation set, referenced shot, and upscale actions keep their existing
  callbacks and Temporal-backed project action behavior.
- Each queue action records:
  - the user prompt or generated action prompt;
  - one tool row labeled `generate_media`;
  - a concise detail such as `variation set`, `variation`, `referenced shot`, or `upscale`;
  - the existing Codex acknowledgement.
- The tool row is presentational only. It does not start a new backend chat protocol and does not
  replace the workflow activity panel.

## Non-Goals

- No new Temporal workflows, project actions, app-server APIs, or MCP commands.
- No persisted chat history in this slice.
- No fabricated provider completion. The workflow activity panel remains the source for queued,
  running, failed, and completed job state.

## Tests

- `AgentPanel` records a `generate_media` tool row when queuing a selected generated variation set.
- Existing queue transcript, selected-source, workflow activity, and composer tests continue to pass.
