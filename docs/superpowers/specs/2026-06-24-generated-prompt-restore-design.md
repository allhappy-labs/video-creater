# Generated Prompt Restore Design

Palmier documents AI-generated clips that can be rerun with the same prompt or tweaked and
regenerated without leaving the timeline. Video Creater already seeds generated variation prompts
from the original prompt and has a `Rerun same prompt` action. The missing manual-edit affordance is
a quick way to return a tweaked variation prompt to the original generation prompt while staying in
the AI edit panel.

## Goal

Add a compact restore action to generated AI edit panels so editors can safely experiment with a
tweaked prompt and then return to the original prompt.

## Behavior

- In `SourceClipInspector` generated AI edit single-variation mode:
  - show `Restore original prompt` after the variation prompt differs from the generated asset prompt;
  - clicking it restores the textarea to the generated asset prompt;
  - the action disappears again when the prompt matches.
- In `MediaBin` selected generated source AI edit single-variation mode:
  - apply the same behavior to `Generated variation prompt`.
- Existing `Rerun same prompt`, `Queue variation`, `Queue variation set`, references, and workflow
  actions remain unchanged.

## Non-Goals

- No changes to queued variation payload shape.
- No persisted draft prompt history.
- No model/provider changes.
- No backend or Temporal changes.

## Tests

- `SourceClipInspector` shows `Restore original prompt` only after editing the variation prompt and
  restores the original prompt on click.
- `MediaBin` shows `Restore original prompt` only after editing the selected generated variation
  prompt and restores the original prompt on click.
