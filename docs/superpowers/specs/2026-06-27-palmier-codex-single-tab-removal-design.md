# Palmier Codex Single Tab Removal

## Context

Video Creater's Codex rail still renders a `Codex conversations` tablist with one
selected `New chat` tab. Earlier Palmier-alignment work removed extra chat header
chrome and kept that row as the rail header, but the app now treats other single
purpose panels as direct surfaces instead of fake mode switches.

Palmier's chat rail reads as a conversation surface with quick actions. A single
selected tab suggests hidden conversation switching that does not exist yet, adds
underline chrome, and makes the assistant panel feel less direct than the media,
timeline, and inspector panels.

## Goal

Replace the one-item Codex conversation tablist with a flat compact header row
that keeps the same visible chat identity and action buttons.

## Behavior

- `AgentPanel` no longer renders the `Codex conversations` tablist.
- `AgentPanel` no longer renders a role `tab` named `New chat`.
- The top rail still shows a compact `New chat` label with the message icon.
- `Start new Codex chat` remains available and clears the local prompt draft,
  mention suggestions, selected mention context, and local transcript messages.
- `Open Codex chat history` remains visible and disabled until persisted history
  exists.
- Workflow activity, selected media/timeline context, tool transcript rows, the
  composer, mention suggestions, and project actions keep their existing contracts.
- No persisted chat schema or conversation history behavior is introduced.

## Verification

- `AgentPanel` tests assert the rail has no `Codex conversations` tablist and no
  `New chat` tab.
- `AgentPanel` tests assert the flat `Codex conversations` toolbar keeps
  the start-new and disabled history buttons.
- Existing new-chat reset coverage still proves the local prompt and mention
  suggestions are cleared.
- Browser QA confirms the Codex rail header is a compact action row instead of a
  tab strip at desktop and narrow widths.
