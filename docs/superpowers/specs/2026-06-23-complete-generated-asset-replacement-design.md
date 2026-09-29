# Complete Generated Asset Replacement

## Context

Palmier lets editors regenerate or tweak AI clips and swap the result into the timeline without leaving the project. Video Creater already has two validated project actions: `completeGeneratedAsset` attaches generated outputs to a pending asset, and `replaceTimelineItemWithGeneratedOutput` swaps an existing generated output into a timeline clip. The missing workflow primitive is an atomic completion action that lets a Temporal generation activity attach its output and replace the target clip in one validated project mutation.

## Goal

Extend `completeGeneratedAsset` with an optional `replacement` payload. When present, Rust should complete the generated asset, register the generated media output, and replace the named timeline item with that output inside the same project action.

## Behavior

- `completeGeneratedAsset` continues to accept `assetId` and `outputs`.
- It may also accept `replacement: { itemId, mediaId }`.
- `replacement.mediaId` must be one of the outputs completed by the same action.
- Rust completes the asset and then applies the existing generated-output replacement validation.
- If replacement validation fails, the whole project action fails and the original project remains unchanged.
- Frontend TypeScript project-action types accept the optional replacement field so Codex, UI, and workflow commands share the same wire contract.

## Non-Goals

- No UI checkbox yet for “replace when ready.”
- No new Temporal workflow type; this is a project action used by the existing `VideoCreaterGenerateMediaWorkflow` completion activity.
- No schema change for `GeneratedAsset` or `JobSummary`.
- No automatic replacement for already-completed assets.

## Tests

- Rust wire contract serializes/deserializes `completeGeneratedAsset` with `replacement`.
- Rust project action completes a pending generated asset and swaps the selected timeline clip when replacement is present.
- Rust rejects a completion replacement that points at a media id not produced by the same completion action.
- Frontend TypeScript accepts the optional replacement field on `completeGeneratedAsset`.
