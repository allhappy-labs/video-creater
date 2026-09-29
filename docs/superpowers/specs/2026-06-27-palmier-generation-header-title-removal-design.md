# Palmier Generation Header Title Removal Design

## Context

Video Creater's media generation sheet still renders a visible `Media generation` title in the
sticky header. Palmier's composer screenshots use that space for compact mode and action controls
instead of repeating the panel name. The accessible region name is still useful, but the visible
title makes the sheet feel more like a settings drawer than a dense editor composer.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the redundant visible generation title while preserving screen-reader structure and the
existing generation workflow behavior.

## Behavior

- The composer remains discoverable as the `Media generation` region.
- The sticky header remains exposed as `Generation sheet header`.
- The header no longer renders visible `Media generation` text.
- The header starts directly with the compact credit, estimate, history, and close controls.
- Mode pills, placement, references, prompt, tuning controls, submit footer, queue behavior, and
  generation request payloads remain unchanged.

## Non-Goals

- No full composer redesign.
- No mode, model, cost, queue, Temporal, fal.ai, or generated asset behavior changes.
- No change to the media panel's `Generate media` entry button.

## Verification

- Update `MediaBin` coverage so opening the composer asserts the accessible region/header instead
  of visible title text.
- Add coverage that the sheet header does not render visible `Media generation` text.
- Keep generation request and composer tests passing.
- Browser QA the open generation sheet and confirm the header controls fit without a redundant
  title line.
