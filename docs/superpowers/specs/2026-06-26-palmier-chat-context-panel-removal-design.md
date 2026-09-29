# Palmier Chat Context Panel Removal Design

## Intent

Palmier's assistant rail is conversation-first: tool calls and project context appear as chat transcript events, while the rail keeps the composer anchored at the bottom. Video Creater currently renders a permanent `Codex context tools` panel and a separate `Project context` card above the transcript, duplicating the same `list_models`, `get_timeline`, and `project_context` information already shown in the chat. This makes the left rail read like a debug sidebar instead of an agent conversation.

## Requirements

- Remove the standalone `Codex context tools` region from the Codex rail.
- Remove the standalone `Project context` card from the Codex rail.
- Keep the transcript default tool-call entries for `list_models`, `get_timeline`, and `project_context`.
- Preserve selected timeline clip context, selected media provenance, workflow job summaries, edit setup, latest request state, proposal actions, template suggestions, and composer behavior.
- Keep the conversation tab header and bottom composer behavior unchanged.

## Testing

- Update `AgentPanel` coverage to assert the standalone context panels are absent.
- Keep coverage proving default tool-call context still renders inside the `Codex chat` transcript.
- Keep coverage proving the conversation controls reset local chat state and rehydrate default tool-call transcript entries.
- Run focused `AgentPanel` tests, lint/typecheck, and browser QA for the left rail.
