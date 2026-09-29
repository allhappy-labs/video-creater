# Source Inspector Composer Placement Design

## Problem

Palmier lets editors regenerate and swap clips on the timeline without leaving the project. Video Creater can now reopen a generated source recipe from the Source Inspector, but that handoff always restores the media generation composer in Library mode. When the selected source is a generated timeline clip, this loses the user's timeline editing intent and makes the next generation land in the library unless the editor manually switches placement.

## Goals

- Preserve timeline placement when a generated timeline clip is sent from Source Inspector to the media generation composer.
- Keep generated library media handoffs in Library placement.
- Reuse the existing MediaBin composer seed behavior and existing `placementIntent` request field.
- Keep this UI-only; no schema, Temporal workflow, or fal provider changes.

## Non-Goals

- No automatic submission of a replacement generation.
- No new timeline replacement action.
- No change to Codex rail handoffs, which continue to use the existing default composer placement.

## Design

`SourceClipInspector` changes its composer callback to include an intent: `onUseGeneratedAssetInComposer(assetId, placementIntent)`. If the inspector is rendering a timeline item, it sends `"timeline"`; if it is rendering selected generated library media, it sends `"library"`.

`EditorWorkspace` stores a one-shot composer seed object instead of only an asset id. It passes both `assetId` and `placementIntent` to `MediaBin`.

`MediaBin` accepts `composerSeedPlacementIntent`. When it restores a generated asset from an external seed, it applies the requested placement. Existing internal `Use in composer` buttons still default to Library mode.

## Acceptance Criteria

- Selecting a generated timeline clip and choosing `Use in composer` opens MediaBin's composer in Timeline mode.
- Selecting generated library media and choosing `Use in composer` opens MediaBin's composer in Library mode.
- The submitted generation request from a timeline-seeded composer carries `placementIntent: "timeline"`.
- Existing MediaBin history/card `Use in composer` behavior remains Library mode.
