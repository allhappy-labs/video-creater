# Active Generation Recipe Strip

## Context

Palmier keeps generation decisions close to the editing surface: the composer shows mode, cost,
first/last frame or reference inputs, output settings, and whether the result will land in the media
library or timeline. Video Creater already has those controls, Temporal start requests, fal.ai model
defaults, generated history reuse, and reference slots. The missing piece is a compact always-visible
recipe strip that lets editors confirm the active generation plan before queueing.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the media generation drawer read like an editor-native generation recipe by showing the active
mode, placement, destination, model, output shape, timing, and selected references in one compact
summary panel.

## Requirements

- When the media generation drawer is open, render an `Active generation recipe` region above the
  prompt.
- The recipe shows:
  - mode (`Image`, `Video`, or `Audio`);
  - placement (`Library` or `Timeline`);
  - destination folder label, falling back to `Project library root`;
  - selected model label;
  - output size and timing;
  - estimated credits;
  - selected references with filenames when media exists and `None` when empty.
- Video mode shows `First frame`, `Last frame`, and `Reference` entries.
- Image mode shows only `Reference`.
- Audio mode shows `No visual references` instead of visual reference entries.
- Updating mode, placement, destination, model, settings, or references updates the recipe
  immediately.
- The existing queue payload, Temporal workflow route, fal.ai model IDs, mock worker behavior,
  drag/drop reference controls, generation history reuse, and generated source reuse remain
  unchanged.
- The recipe uses compact editor styling and no nested card layout.

## Non-Goals

- No new generation provider, model option, credit backend, or paid-plan enforcement.
- No project schema change.
- No new Temporal workflow type or activity.
- No automatic timeline insertion beyond the existing `placementIntent` request value.

## Test Plan

- Add a MediaBin component test that opens the composer, verifies the recipe defaults, changes
  placement/destination/settings/references, and verifies the recipe updates.
- Add a MediaBin component test that switches to audio mode and verifies the recipe says no visual
  references.
- Run the focused MediaBin suite, full frontend tests, type checks, build, browser QA, whitespace
  checks, placeholder scan, and secret scan.
