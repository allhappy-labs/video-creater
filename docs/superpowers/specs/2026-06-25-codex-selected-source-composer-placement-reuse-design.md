# Codex Selected Source Composer Placement Reuse Design

## Problem

Palmier lets chat and agent workflows iterate on generated assets in the same project context as manual editing. Video Creater's media bin now preserves a generated asset's `placementIntent` when reopening it in the media composer, but the Codex rail still opens selected generated sources with hard-coded Library placement. A replacement-targeted generated source selected in Codex therefore loses its replacement destination when the user clicks `Use in composer`.

## Goals

- Preserve the selected generated asset's stored `placementIntent` when Codex opens it in the media generation composer.
- Keep Library as the fallback for generated assets without placement metadata.
- Keep the AgentPanel callback simple; EditorWorkspace can resolve placement from canonical project state.
- Keep Source Inspector and MediaBin composer seeding behavior unchanged.

## Non-Goals

- No new AgentPanel prop shape or project schema field.
- No automatic generation submission from the Codex rail.
- No changes to variation queueing, which already reuses generated asset placement.

## Design

`EditorWorkspace` will resolve the generated asset by id in the `onUseSelectedSourceInComposer` handler passed to `AgentPanel`. It will seed MediaBin with `generatedAssetPlacementIntent(asset)` instead of always using `"library"`. If the asset is missing, the existing safe fallback remains Library.

This keeps Codex aligned with the manual editor: selected generated sources, generated asset cards, and Source Inspector all reuse the same placement context when opening the composer, while users can still retarget the draft from the composer controls.

## Acceptance Criteria

- A Codex-selected generated source with `placementIntent: "replace:<itemId>"` opens the media composer with Replacement placement visible.
- Submitting that composer draft carries the same replacement placement intent.
- Generated sources without placement metadata still open in Library placement.
