# Codex Selected Source Replace Queue

## Context

Palmier's docs describe generated video as timeline-native: users and connected agents can rerun,
tweak, and swap AI-generated clips in place. Video Creater already retained a selected generated
timeline clip as the replacement target after the user opened one of its generated outputs, and the
Codex rail could replace that clip with the completed output. It could also queue a selected source
variation, but only as another library output.

Palmier reference: https://www.palmier.io/docs

## Goal

Let the Codex rail queue a prompt-tweaked variation from the selected generated source while keeping
the retained timeline clip as the replacement target.

## Behavior

- Pass the retained replacement timeline item id into `AgentPanel` separately from its label.
- In the selected Codex source region, show a replacement variation action only when a selected
  generated source, retained replacement target, prompt text, and replacement callback are present.
- The action calls the existing generated variation queue with the selected source asset id, trimmed
  prompt, and retained timeline item id.
- `EditorWorkspace` records the queued generated asset and Temporal start request with
  `placementIntent: "replace:<timeline-item-id>"`.
- The completed-output replace action remains separate and still swaps an already available output.

## Non-Goals

- No new Rust project action or completion path.
- No automatic replacement for variation sets.
- No replacement controls for non-generated selected sources.
- No changes to fal.ai models, generation settings, or provider activity code.

## Verification

- AgentPanel test: selected generated sources can queue a replacement variation with the retained
  item id.
- EditorWorkspace test: the Codex source replacement variation records the generated asset and
  Temporal start request with `placementIntent: "replace:<timeline-item-id>"`.
- Run AgentPanel and EditorWorkspace suites, lint, whitespace checks, and the secret-fragment scan.
