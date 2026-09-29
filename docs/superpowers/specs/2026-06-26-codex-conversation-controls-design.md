# Codex Conversation Controls

## Context

Palmier's assistant rail presents chat as a workspace object: the header has a `New chat` tab plus quick controls for starting another conversation and opening recent chat history. Video Creater already has a Palmier-style Codex rail, derived tool rows, a transcript area, and a bottom composer, but the conversation strip is currently static. That makes the agent feel less like a controllable editing surface even though the underlying edit and generation actions are already validated.

## Goal

Add compact conversation controls to `AgentPanel` so editors can see the expected chat affordances beside the active `New chat` tab without introducing fake persisted chat state.

## Behavior

- The Codex conversation tab strip keeps the selected `New chat` tab.
- The tab strip adds icon buttons for `Start new Codex chat` and `Open Codex chat history`.
- The buttons use familiar symbols, accessible labels, and tooltips/titles.
- `Start new Codex chat` clears the local prompt draft, mention suggestions, selected mention context, and local transcript messages, then records the default context tool rows again.
- `Open Codex chat history` is disabled until persisted conversation history exists.
- Existing workflow count, context tool rows, proposal review, selected-source actions, and generate-edit behavior stay unchanged.

## Non-Goals

- No persisted chat history format.
- No backend thread creation, Codex thread switching, or MCP protocol changes.
- No project file changes.
- No decorative chat chrome that suggests unavailable remote state.

## Tests

- `AgentPanel` renders the new conversation icon controls in the `Codex conversations` tablist.
- `Open Codex chat history` is disabled.
- After typing a prompt, clicking `Start new Codex chat` clears the composer and leaves the default context tool rows available.
- Existing AgentPanel tests continue to pass.
