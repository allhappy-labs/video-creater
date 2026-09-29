# Palmier Generated Card Model Chrome Removal

## Problem

The AI generation card list still renders provider and model identifiers in the visible card body, for example `completed - seedance/seedance-2-fast`. Palmier's media panel treats generated assets as media-grid items first; model provenance belongs in generation setup, inspector details, or project data, not as primary card chrome.

## Requirements

- AI generation cards show a compact status label such as `completed`, `queued`, `running`, or `failed`.
- AI generation cards do not render provider/model strings in the visible card body.
- Existing generated output rows, replacement/insert actions, target-folder chips, timeline/replacement target chips, lineage, pending output, and mock worker actions remain available.
- Generation model selectors and composer drafts continue to show model choices.
- Search/filter behavior may continue to use provider/model metadata.

## Non-Goals

- Do not remove model data from project records, queued workflow payloads, sidecars, or composer defaults.
- Do not change the fal.ai model IDs used for image or video generation.
- Do not redesign the generated output summary cards.

## UI Design

Generated cards use a concise media-card hierarchy:

- title or generated asset id
- placement chips when present
- status-only metadata line
- optional prompt/lineage/output summaries and actions

The status line stays useful for queues without making the media grid look like a diagnostics table.

## Testing

- Add a focused `MediaBin` test proving generated cards render status without provider/model text.
- Keep existing tests proving generated assets can seed the composer, select outputs, replace clips, and keep model selectors populated.
