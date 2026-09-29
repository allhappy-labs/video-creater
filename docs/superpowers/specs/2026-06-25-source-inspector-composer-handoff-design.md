# Source Inspector Composer Handoff Design

## Problem

Palmier makes generated timeline clips editable from the timeline: editors can inspect the prompt, references, first frame, last frame, and then rerun or tweak the generation without leaving the project. Video Creater already lets the Codex rail and MediaBin restore a generated asset into the media generation composer, but the manual Source Inspector does not expose the same handoff. Editors who select a generated clip in the timeline still need to find the matching generated card or Codex context before they can reopen the generation recipe.

## Goals

- Add a manual Source Inspector action for generated timeline clips and generated library media that opens the media generation composer with the source generation recipe.
- Reuse the existing EditorWorkspace composer seed state and MediaBin composer seeding behavior.
- Keep the action local to UI state; no project schema, Temporal workflow, or fal request changes.
- Preserve the existing Source Inspector details, AI Edit variation queueing, replacement variation queueing, reference reveal, insert, and swap actions.

## Non-Goals

- No automatic generation queue submission.
- No new workflow action or job record.
- No composer handoff for imported media that does not resolve to a generated asset.

## Design

`SourceClipInspector` receives an optional `onUseGeneratedAssetInComposer(assetId)` callback. When the selected source resolves to a generated asset, it shows a compact `Use in composer` button near the source reveal action. The button is available in both generated timeline clip mode and selected generated library media mode.

`EditorWorkspace` handles the callback by switching the source panel to Media and setting the existing one-shot composer seed asset id. `MediaBin` already consumes that id, restores prompt/model/settings/references into the composer, opens the composer, and reports the seed handled.

## Acceptance Criteria

- SourceClipInspector calls `onUseGeneratedAssetInComposer` with the generated asset id when a generated source is selected.
- The action is hidden for imported sources without a generated asset id.
- EditorWorkspace opens MediaBin's media generation composer from the Source Inspector.
- The composer is seeded with the generated source prompt, model, settings, first frame, last frame, and references.
- Existing Source Inspector queue, replacement, reveal, insert, and swap actions keep working.
