# Palmier Chat Edit Setup Removal

## Context

Palmier's assistant rail is prompt-first: the user types what they want, references media with `@`,
and sees tool activity in the conversation. Video Creater still renders a persistent `Edit setup`
form above the composer with `Trailer Cut`, `Highlight Reel`, `Story Cut`, and language buttons. That
keeps the rail looking like a rough-cut settings form instead of a conversation-driven editor.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the visible preset/language setup form from the Codex rail while preserving the existing
`EditJobRequest` defaults and transcript behavior.

## Behavior

- The Codex rail no longer renders the `Edit setup` section.
- The rail no longer renders persistent `Trailer Cut`, `Highlight Reel`, `Story Cut`, `English`,
  `Ukrainian`, or `Auto` setup buttons.
- New edit requests keep the existing default preset and language: `trailer_cut` and `en`.
- The prompt field remains the primary place to describe edit intent and mention media.
- Latest-request transcript and status copy still display the preset/language that was actually sent.
- No backend, project-action, Temporal workflow, proposal, media generation, or persistence contract
  changes.

## Verification

- `AgentPanel` tests assert the setup controls are absent while generated edit requests still use the
  default preset and language.
- Existing transcript tests keep proving the sent request appears in chat.
- Browser QA confirms the left rail moves from selected context/tool activity directly toward the
  composer without the settings-form block.
