# Codex Mention Resolver Design

## Context

Palmier documents assistant workflows where the user can reference project media with `@` from chat, then ask the agent to generate or edit against that referenced source. Video Creater already shows the active `@media-id` in the Codex rail and can target the currently selected media, but typed mentions in the composer are still only text.

## Goal

Resolve typed `@` mentions in the Codex composer against project media and generated output media ids. When the prompt contains a resolvable mention, Codex should show the resolved target and send the edit request for that media id.

## Behavior

- `EditorWorkspace` passes a compact mention target list to `AgentPanel`.
- Mention targets include imported media ids and generated output media ids.
- `AgentPanel` scans the prompt for `@id` tokens and uses the first token that matches a mention target.
- The composer shows a small resolved mention row when a typed mention overrides the selected/default target.
- `Generate edit` keeps the existing `EditJobRequest` shape, but uses the resolved mentioned media id.
- If the prompt has no resolvable mention, Codex keeps using the selected/default `mediaId`.
- Unknown mentions remain plain prompt text and do not block generation.

## Non-Goals

- No autocomplete dropdown in this slice.
- No multi-reference backend chat protocol.
- No change to media selection state when the user types a mention.
- No timeline mutation or generated-asset queueing from free-form chat text.

## Testing

- `AgentPanel` resolves a typed generated-output mention, displays it, and sends the edit request for that id.
- `AgentPanel` ignores an unknown mention and keeps the selected/default target.
- `EditorWorkspace` provides generated output ids as mention targets so a prompt mentioning `@sample-generated-output` targets that generated output without selecting it first.

## Future Work

- Add mention autocomplete and keyboard insertion.
- Pass multi-reference context to the backend once Codex chat turns support it.
- Let typed mentions drive referenced media generation and replacement actions.
