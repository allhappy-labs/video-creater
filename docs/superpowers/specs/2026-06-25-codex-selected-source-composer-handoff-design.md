# Codex Selected Source Composer Handoff Design

## Problem

Palmier lets editors inspect generated media, then iterate on the same recipe without leaving the project. Video Creater already lets the media bin restore a generated asset into the media generation composer, including prompt, first frame, last frame, references, model, duration, resolution, and aspect ratio. The Codex selected source block exposes the same generated source context, but it cannot hand that source back to the composer. Editors must switch surfaces and find the matching media-bin action manually.

## Goals

- Add a Codex selected generated source action that opens the media generation composer with the source's stored generation recipe.
- Reuse MediaBin's existing generated-asset composer seeding behavior.
- Keep the handoff local to UI state; no project schema, Temporal workflow, or fal request changes.
- Preserve existing Codex variation, replacement, insertion, reference open/mention, and prompt actions.

## Non-Goals

- No new generation workflow action.
- No automatic queue submission.
- No composer handoff for imported media without a generated asset id.

## Design

`AgentPanel` gets an optional `onUseSelectedSourceInComposer(assetId)` callback. When the selected media context has a `generatedAssetId`, the selected source action list shows a compact `Use in composer` button.

`EditorWorkspace` handles the callback by switching the source panel to Media and setting a one-shot pending composer seed asset id. `MediaBin` receives that id, finds the generated asset, reuses `useGeneratedAssetInComposer(asset)`, and reports the seed handled so the command does not replay on unrelated renders.

## Acceptance Criteria

- AgentPanel calls `onUseSelectedSourceInComposer` with the selected generated asset id.
- The action is hidden for selected sources without a generated asset id.
- EditorWorkspace opens MediaBin's media generation composer from the Codex selected source block.
- The composer is seeded with the generated source prompt, model, settings, and visual references.
- Existing selected source queue/insert/replace actions keep working.
