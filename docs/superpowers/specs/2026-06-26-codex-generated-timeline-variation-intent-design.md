# Codex Generated Timeline Variation Intent Design

## Context

Palmier treats AI-generated timeline clips as editable objects: an editor can select an AI clip,
ask for a prompt tweak, and keep iterating without leaving the timeline. Video Creater already shows
selected generated timeline clip provenance and local rerun/variation buttons in the Codex rail, but
the primary Codex composer still treats prompt tweaks such as `make this generated clip warmer` as
new EDL edit requests.

Palmier reference: https://www.palmier.io/docs

## Goal

Route explicit selected generated timeline clip tweak prompts through the primary Codex composer
action.

## Behavior

- When a generated timeline clip is selected and the prompt asks to change, tweak, or vary the
  selected clip, the primary composer button changes to `Queue selected clip variation`.
- Pressing the button calls the existing selected generated timeline variation callback with the
  selected generated asset id and trimmed prompt.
- The transcript records the same user prompt, `list_models` preflight, queued generation tool call,
  and Codex acknowledgement used by the existing selected generated clip variation button.
- Locked-track state does not block generation queueing; it only blocks timeline mutation actions.
- Existing split, trim, delete, reorder, variation-set, selected-source, mention, and EDL edit routes
  remain unchanged.

## Non-Goals

- No replacement variation from the primary composer in this slice.
- No new generated asset schema or Temporal workflow shape.
- No generalized freeform intent parser beyond explicit selected generated clip tweak language.

## Verification

- `AgentPanel` test proves a selected generated timeline clip tweak prompt changes the primary
  composer button, queues a variation, avoids `onGenerateEdit`, and records a generation tool call.
- Existing selected generated timeline clip rerun/variation tests continue to pass.
- Run focused AgentPanel tests, full AgentPanel tests, `pnpm lint`, and a browser smoke of the
  selected generated timeline clip composer action.
