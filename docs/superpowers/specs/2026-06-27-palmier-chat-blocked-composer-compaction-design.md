# Palmier Chat Blocked Composer Compaction Design

## Context

Palmier's assistant rail keeps the chat composer prompt-first: the user sees the prompt, the primary
action, and compact tool/status feedback. Video Creater already removed the old edit setup form, but
when local transcription is unavailable the composer still expands into a tall amber setup card with
explanatory copy and a full-width settings button. That makes the rail feel like a configuration
screen instead of a chat-controlled editor.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep source-edit generation safely blocked when transcription requirements are not met while making
the blocked state compact and composer-native.

## Behavior

- The Codex composer keeps a visible `Generate edit` primary action even when source-edit generation
  is blocked.
- The blocked `Generate edit` action is disabled and never calls `onGenerateEdit`.
- The blocked reason remains visible in one compact line.
- Model setup remains reachable through `Open model settings`, shown as compact inline composer
  chrome.
- The long explanatory blocked-detail paragraph is no longer rendered in the rail.
- Existing ready states, transcript rows, selected-context routing, Codex primary prompt actions,
  Temporal workflow paths, and project persistence remain unchanged.

## Verification

- `AgentPanel` blocked-state tests prove the primary action stays visible but disabled, the compact
  settings action still fires, and the long explanatory copy is absent.
- Existing ready-state tests prove edit requests still use the default preset and language.
- Browser QA checks the left Codex rail at desktop and narrow widths for a compact composer footer.

## Self-Review

- No backend, schema, Temporal, fal.ai, media, or timeline contracts change.
- The blocked state remains accessible and explicit; this only changes visual density and action
  placement.
- Scope is limited to `AgentPanel` rendering and its tests.
