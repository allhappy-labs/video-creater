# Generate Edit Tool Transcript Design

Palmier's assistant screenshots show edit requests followed by compact tool rows such as
`get_timeline`, making the agent's project-context work visible without taking over the editor.
Video Creater now shows `generate_media` rows for selected-source queue actions, but the primary
`Generate edit` path still jumps from a prompt to a generic status message.

## Goal

When an editor starts a Codex EDL edit from the composer, the Codex chat transcript should show the
request and the project-context tools the app uses for that request.

## Behavior

- Clicking `Generate edit` keeps the existing `onGenerateEdit(EditJobRequest)` callback.
- The local transcript records:
  - a `You` message with the prompt and target media id;
  - a `get_timeline` tool row marked loaded;
  - a `project_context` tool row marked loaded;
  - a concise Codex acknowledgement that an EDL-first edit request was sent.
- If the parent later passes the same request through `latestRequest`, the same prompt should not
  appear a second time.
- Existing selected-source queue tool rows and workflow activity behavior remain unchanged.

## Non-Goals

- No new backend chat protocol.
- No new Temporal workflow or project action.
- No persisted transcript storage.
- No claim that the generated proposal is complete before the existing proposal/status state says so.

## Tests

- `AgentPanel` records `get_timeline` and `project_context` tool rows when `Generate edit` is clicked.
- `AgentPanel` suppresses the duplicate `latestRequest` prompt when it matches the locally recorded
  generate-edit transcript.
