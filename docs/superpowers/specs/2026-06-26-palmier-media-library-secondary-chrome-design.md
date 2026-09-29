# Palmier Media Library Secondary Chrome Design

## Context

Palmier's media rail keeps the persistent library chrome compact: the top row exposes `Import`,
`New Folder`, `Generate`, and search, then a small item count and icon cluster sit above the asset
grid. Video Creater already matches the top action row, but it still renders a second `Project
library` header with persistent `Grid view`, `List view`, a `Sort project media` combobox, and a
sort-direction button. That extra chrome competes with media thumbnails and makes the rail feel less
like Palmier's dense editor panel.

## Goal

Simplify the media rail by removing the persistent secondary view/sort controls while keeping media
actions, folders, search, and deterministic asset ordering intact.

## Behavior

- The media rail still exposes `Import`, `New Folder`, `Generate`, and `Search project media`.
- The secondary library header becomes a compact item-count row, such as `3 items`.
- Persistent `Grid view`, `List view`, and `Sort project media` controls are removed from the
  default rail chrome.
- The media grid remains the only default presentation in this slice.
- Existing project-order sorting remains the deterministic default; no schema or project-action
  behavior changes.
- Folder cards, folder navigation, folder assignment controls, generated cards, and search results
  continue to work.

## Non-Goals

- Do not remove folder management or media-generation behavior.
- Do not add a new Palmier-style overflow menu in this slice.
- Do not change media folder schemas, generated asset schemas, Temporal workflow payloads, or fal.ai
  generation settings.
- Do not redesign the full media card layout.

## Verification

- `MediaBin` tests assert the rail still shows the top action row and item count.
- `MediaBin` tests assert the persistent `Grid view`, `List view`, and `Sort project media` controls
  are absent.
- Existing media-bin tests continue to prove folders, search, generated assets, composer seeding,
  replacement, insertion, mock completion, and retry behavior.
- Browser QA captures the left media rail and verifies the secondary header reads as compact count
  chrome without the removed toggles.
