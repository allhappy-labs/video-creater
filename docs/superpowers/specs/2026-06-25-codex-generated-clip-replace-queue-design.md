# Codex Generated Clip Replace Queue

## Context

Palmier's MCP/agent docs say connected agents can rerun or tweak AI-generated clips by prompt just
like a user can in the app. Video Creater already lets the manual Source Inspector queue
tweaked-prompt replacements with `placementIntent: "replace:<timeline-item-id>"`, and the selected
timeline clip dock can queue same-prompt replacements. The Codex rail still only exposed
make-another-output actions for selected generated timeline clips.

Palmier reference: https://www.palmier.io/docs

## Goal

Give the Codex rail the same replacement-intent controls for selected generated timeline clips that
the manual editor has.

## Behavior

- In the selected timeline clip context, keep `Rerun same prompt` and `Queue variation` as
  non-destructive make-another-output actions.
- Add `Rerun and replace` for same-prompt replacement reruns.
- Add `Queue and replace` for prompt-tweak replacement variations.
- Both replacement actions call the generated variation queue with the selected timeline item id.
- EditorWorkspace records generated assets and Temporal start requests with
  `placementIntent: "replace:<timeline-item-id>"`.
- Blank Codex prompts disable the prompt-tweak replacement action, matching the existing variation
  action.

## Non-Goals

- No new Rust project action or completion path.
- No automatic choice among variation-set outputs.
- No replacement controls for imported timeline clips; imported AI edits still queue referenced
  generations or upscales.
- No changes to fal.ai models, generation settings, or provider activity code.

## Verification

- AgentPanel test: selected generated timeline clips expose replacement rerun and replacement
  variation actions and call the replacement callback with asset id, prompt, and item id.
- EditorWorkspace test: the Codex rail replacement variation records the generated asset and
  Temporal start request with `placementIntent: "replace:<timeline-item-id>"`.
- Run AgentPanel and EditorWorkspace suites, TypeScript checks, whitespace checks, and the
  secret-fragment scan.
