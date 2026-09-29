# AI Generations Media Card Design

## Context

Palmier treats generated media as first-class library objects: generated assets appear with visual
thumbnails, `AI` identity, duration/status cues, and quick reuse actions in the same media panel
used for imported footage. Video Creater already records generated assets, renders output summaries,
supports history reuse, and can insert or replace generated outputs. The remaining visual gap is the
asset-level `AI generations` list: each generated asset still starts as a text card, so the library
does not scan like a media grid.

## Goal

Make each generated asset card read like a compact media object before the user drills into output
rows.

## Behavior

- Each generated asset card shows a leading preview tile when the asset has at least one output that
  resolves to a project media item.
- The preview tile uses the existing media thumbnail renderer, duration badge, and failed-preview
  fallback path.
- The card header keeps the generated title, status, model, prompt, timeline/replacement/destination
  badges, and `Use in composer` action.
- The preview tile exposes an accessible label, `Generated asset preview <asset id>`.
- Queued, running, or failed assets without usable output keep the existing text-first card layout.
- Existing output rows, mock completion actions, retry actions, insert/replace actions, and
  workflow status blocks remain unchanged.

## Non-Goals

- No changes to project schema, generated asset storage, Temporal workflow contracts, or fal.ai
  provider behavior.
- No new media selection model.
- No automatic opening of the composer or viewer from the preview tile.

## Verification

- Add a `MediaBin` regression proving a completed generated asset renders an asset-level preview
  tile with a duration badge while preserving the existing output selection row.
- Existing generated asset, composer, replacement, insertion, and workflow tests continue to pass.
- Browser-smoke the editor at desktop width and confirm the `AI generations` section reads as media
  content without overlap.
