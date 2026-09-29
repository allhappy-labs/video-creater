# Palmier Codex Readiness Footer Removal Design

## Goal

Remove the redundant readiness footer from the Codex rail so the assistant surface stays prompt-first like Palmier. The transcript already states Codex status, and the composer already carries the prompt and primary action.

## Design

- Remove the standalone footer card that says `Ready to create an EDL-first draft.` when there is no latest request.
- Keep latest-request status visible when a request exists.
- Keep Codex errors, proposal-ready status, selected-source tool rows, workflow activity, mention suggestions, blocked composer state, and `Generate edit` behavior unchanged.
- Do not change edit request payloads, Temporal jobs, project actions, or persistence.

## Testing

- `AgentPanel` verifies the rail no longer renders `Ready to create an EDL-first draft.` in the default idle state.
- Existing tests continue to prove latest requests render as user chat messages and status copy remains available when present.
