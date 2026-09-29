# Media Bin Composer Placement Reuse Design

## Problem

Palmier keeps AI generation edits native to the project: timeline-targeted and replacement-targeted generations can be rerun or tweaked from the same editor context. Video Creater already marks generated asset cards as `Timeline target` or `Replacement target`, but the media panel card action `Use in composer` always reopens the composer in Library placement. That breaks the context shown on the card and makes timeline-native regeneration feel like a library detour.

## Goals

- Reuse a generated asset's stored `placementIntent` when opening it from a media-bin generated asset card.
- Reuse the same placement when opening the currently selected generated source in the media-bin details panel.
- Keep Library as the fallback for generated assets that do not have a stored placement intent.
- Keep the existing composer controls editable so the user can retarget the draft after opening it.

## Non-Goals

- No new project schema fields.
- No changes to Source Inspector handoff, which already sends explicit placement context.
- No automatic timeline replacement on completion; completion and replacement actions remain explicit.

## Design

`MediaBin` will treat each generated asset's `placementIntent` as the default composer placement when that asset itself is the source of the composer draft. `useGeneratedAssetInComposer(asset)` will resolve the intent to `asset.placementIntent ?? "library"` unless a caller passes an explicit override. Existing external seeds keep passing their explicit intent, which lets Source Inspector remain authoritative when it knows the selected timeline item.

The active recipe and submit footer already render Library, Timeline, and Replacement placement. This change only fixes which placement is selected when a generated asset card or selected generated source panel reopens the composer.

## Acceptance Criteria

- A generated asset card with `placementIntent: "timeline"` opens the composer in Timeline placement and submits `placementIntent: "timeline"`.
- A generated asset card with `placementIntent: "replace:<itemId>"` opens the composer in Replacement placement and submits that same replacement intent.
- The selected generated source panel uses the generated asset's stored placement intent when opening the composer.
- Generated assets without a placement intent still open in Library placement.
