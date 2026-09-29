# Palmier Completed Generation Grid Dedup Design

## Context

Palmier shows generated images and videos as normal media-library tiles with AI badges. Video Creater already renders generated output media inside the `Project media grid`, but it also renders completed generated assets again in a separate `AI generations` card surface. That duplicates completed results and makes the media browser feel less like the Palmier reference grid.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep completed generated outputs in the normal project media grid and reserve the `AI generations` surface for generation work that still needs workflow attention.

## Requirements

- Completed generated assets whose outputs all exist in `media` do not render in the `AI generations` region.
- Their output media remains selectable from `Project media grid` with the existing AI badge, duration, and selection behavior.
- Queued, running, failed, or otherwise unresolved generated assets still render in `AI generations`.
- Completed generated assets with missing output media still render in `AI generations` so the user can inspect the incomplete record.
- Do not add media-library tabs, toggles, switches, or view-mode controls.
- Preserve existing generated output insert/replace/select behavior where an AI generation card remains visible.

## Tests

- `MediaBin` proves completed generated assets with present output media are absent from `AI generations` and selectable from the project media grid.
- `MediaBin` proves queued/running/failed or missing-output generated assets still render in `AI generations`.
- `EditorWorkspace` default media-library smoke proves generated output media remains visible without duplicating a completed generation-history card.
