# Codex Variation Set Intent Design

## Context

Palmier's chat workflow lets an editor ask for several image or video variations in plain language
and keep working while the generated assets appear in the project library. Video Creater already has
a `Queue variation set` action in the selected generated source block, but the primary Codex
composer still treats prompts like `create four variations of this image` as an EDL edit request
unless the editor clicks the secondary source action.

Palmier reference: https://www.palmier.io/docs

## Goal

Route explicit selected-source variation-set prompts through the primary Codex composer action.

## Behavior

- When a generated source with a known generated asset id is selected, and the prompt asks to create
  multiple variations, the primary composer button changes to `Queue variation set`.
- Pressing that button queues the same deterministic variation drafts as the existing selected
  source `Queue variation set` action.
- The transcript records the same user prompt, `list_models` preflight, queued generation tool
  call, Codex acknowledgement, and direction summary as the existing secondary action.
- Existing single variation, replacement variation, referenced generation, mention insertion, and
  EDL edit generation behavior remain unchanged.

## Non-Goals

- No new variation-set project schema.
- No provider or Temporal workflow changes.
- No natural-language count parsing beyond detecting explicit plural variation intent.

## Verification

- `AgentPanel` test proves a `create four variations...` prompt changes the composer primary action
  to `Queue variation set`, queues deterministic drafts, and avoids `onGenerateEdit`.
- Existing selected-source variation-set tests continue to pass.
- Run focused AgentPanel tests, full AgentPanel tests, `pnpm lint`, and a browser smoke of the
  Codex composer action.
