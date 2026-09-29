# AI Generation Output Selection State

## Context

Palmier keeps generated media cards in the media panel visually connected to the active source:
selected generated outputs use the same strong selection treatment as regular media tiles. Video
Creater can select generated outputs from the AI generations list, but the list does not mark the
selected output, making it feel separate from the rest of the editor selection model.

Palmier reference: https://www.palmier.io/docs

## Goal

Make generated output cards in the AI generations list show when they are the active selected media
source.

## Behavior

- When `selectedMediaId` matches a generated output media id, the corresponding output card exposes
  `aria-pressed="true"`.
- The selected output card uses the same visible selected treatment as media tiles: ring border,
  accent background, foreground text, and selection shadow.
- Unselected generated output cards keep their existing compact thumbnail, AI badge, duration, file
  metadata, and click behavior.

## Non-Goals

- No selection model or backend changes.
- No generated asset schema change.
- No timeline insertion, replacement, or generation queue behavior change.

## Verification

- Media Bin test: a selected generated output card in the AI generations list has pressed state and
  selected styling.
- Existing generated output selection, metadata, thumbnail, and selected source tests continue
  passing.
