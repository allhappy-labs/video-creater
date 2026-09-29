# Selected Generated Source Composer Draft

## Context

Palmier treats generated clips as reusable editing context: users can inspect prompt, references,
model, duration, resolution, and aspect ratio, then continue from that setup without manually
rebuilding it. Video Creater already has generation history reuse and selected generated source
details, but the selected source details do not offer a direct handoff into the media generation
composer.

Palmier reference: https://www.palmier.io/docs

## Goal

Let editors turn the currently selected generated source into an editable media generation composer
draft from the source details panel.

## Behavior

- In selected generated source `Details`, show `Use in composer` when media generation is available.
- Clicking it opens the media generation composer.
- The composer draft reuses the same rules as generation history:
  - prompt copied into `Generation prompt`;
  - generated asset name becomes `<name> variation` when available;
  - compatible mode, model, duration, aspect ratio, and resolution are restored;
  - compatible first frame, last frame, and reference media are restored.
- The action does not queue generation; the draft remains editable until the user presses
  `Queue generation`.

## Non-Goals

- No new backend action, Temporal workflow, fal.ai provider behavior, or project schema change.
- No automatic queueing or timeline insertion.
- No unsupported model options added to the composer.

## Verification

- Media Bin test: selected generated source details expose the composer handoff action.
- Media Bin test: clicking it opens the composer with prompt, variation name, references, model, and
  output settings restored.
- Existing Media Bin generation, history, and selected source tests continue passing.
