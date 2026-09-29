# Palmier Chat Header Chrome Removal Design

## Intent

Palmier's left assistant rail uses the conversation tab row as the rail header. Video Creater still renders an extra brand/status row above `New chat`: a `Codex` title and an active workflow count. That duplicates information already present in the chat transcript and workflow activity, consumes vertical space, and makes the rail look less like Palmier's conversation-first panel.

## Requirements

- Remove the standalone `Codex` title row from the chat rail header.
- Remove the header-level active workflow count badge.
- Keep the `New chat` tab, new-chat button, and disabled history button as the top rail controls.
- Keep active workflow counts visible inside `Codex workflow activity` when workflow jobs exist.
- Preserve composer, mention, transcript, selected source/clip context, and workflow activity behavior.

## Testing

- Update `AgentPanel` rail chrome coverage to assert the conversation header does not include `Codex` or workflow-count text.
- Keep coverage proving active workflow count still appears in the `Codex workflow activity` transcript section.
- Run focused `AgentPanel` tests, lint/typecheck, and browser QA for the left rail.
