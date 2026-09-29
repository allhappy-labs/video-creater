# Codex Selected Source Variation

## Context

Palmier documents an assistant workflow where generated clips can be rerun or tweaked by prompt, and where the assistant can work directly with selected generated media in the editor. Video Creater already has generated asset provenance, selected-source Codex context, and a project action path for queueing generated clip variations from inspectors.

The missing piece is connecting the Codex prompt box to the selected generated source, so an editor can select a generated output, describe the desired change, and queue a variation without leaving the agent panel.

## Goals

- Show a Codex variation action when the selected source is a generated output with a known generated asset id.
- Use the current Codex prompt text as the variation instruction.
- Reuse the existing `recordGeneratedAsset` project action flow through `queueGeneratedClipVariation`.
- Preserve provenance by keeping the new variation linked to the selected generated asset.

## Non-Goals

- No new generation backend or provider integration.
- No automatic timeline replacement after queueing a variation.
- No separate variation composer in the Codex panel.
- No changes to generated asset persistence schema beyond carrying the selected generated asset id in UI context.

## UI Behavior

When the selected Codex source is a generated output, the selected-source block shows `Queue variation`. The button is disabled until the Codex prompt contains non-whitespace text. Clicking it trims the prompt and queues a variation for the generated asset that produced the selected output.

The existing `Insert on timeline` action remains available when a selected generated output can be inserted.

## Data Flow

1. `EditorWorkspace` derives `generatedAssetId` while building `AgentSelectedMediaContext`.
2. `AgentPanel` receives `onQueueSelectedVariation(assetId, prompt)`.
3. The selected-source block renders the variation button only when both `generatedAssetId` and the callback exist.
4. Clicking the button calls `onQueueSelectedVariation(generatedAssetId, prompt.trim())`.
5. `EditorWorkspace` wires the callback to the existing `queueGeneratedClipVariation`, preserving model, references, settings, parent asset id, and retry asset id.

## Tests

- `AgentPanel` calls the variation callback with the selected generated asset id and trimmed Codex prompt.
- `EditorWorkspace` queues a `recordGeneratedAsset` action from the Codex selected-source context.
