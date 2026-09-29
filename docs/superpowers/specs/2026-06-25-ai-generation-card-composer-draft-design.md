# AI Generation Card Composer Draft

## Context

Palmier keeps generated media visible in the media panel and lets editors continue from previous
AI outputs without rebuilding prompts, references, model choices, or output settings by hand. Video
Creater already supports this from generation history and selected generated source details, but the
AI generations list still requires selecting an output before the composer handoff appears.

Palmier reference: https://www.palmier.io/docs

## Goal

Let editors reuse a generated asset directly from its AI generation card as an editable media
generation composer draft.

## Behavior

- Each AI generation card shows `Use in composer` when media generation is available.
- Clicking it opens the media generation composer in library placement mode.
- The composer draft reuses the same restore rules as generation history:
  - prompt copied into `Generation prompt`;
  - generated asset name becomes `<name> variation` when available;
  - compatible mode, model, duration, aspect ratio, and resolution are restored;
  - compatible first frame, last frame, and reference media are restored.
- The action does not queue generation, select timeline clips, or insert generated outputs.

## Non-Goals

- No backend action, Temporal workflow, fal.ai provider behavior, or project schema change.
- No automatic output selection or timeline insertion.
- No new composer model options beyond compatible existing choices.

## Verification

- Media Bin test: a generated asset card exposes `Use in composer`.
- Media Bin test: clicking the card action opens the composer with prompt, variation name,
  references, duration, aspect ratio, and resolution restored.
- Existing Media Bin generation history and selected generated source composer tests continue
  passing.
