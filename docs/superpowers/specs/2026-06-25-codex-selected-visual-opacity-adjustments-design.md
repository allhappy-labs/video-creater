# Codex Selected Visual Opacity Adjustments

## Context

Palmier's agent can trim, split, reorder, and adjust clips while seeing the full project context. Video Creater now has a validated `updateVisualClipOpacity` project action, a Source Inspector opacity control, and timeline opacity affordance. The Codex selected timeline clip rail still exposes only audio clip adjustments, so visual opacity cannot be changed from the agent-side selected-clip context.

## Goal

Add compact visual opacity adjustment controls to the Codex selected timeline clip block.

## Behavior

- The `Selected timeline clip` block shows a `Visual` adjustment group for `video_clip`, `overlay`, and `hyperframe_scene` items.
- The opacity input defaults to the selected clip's finite `properties.opacity` value or blank when absent.
- Applying blank opacity sends `1` to reset the property to full opacity through the existing project action.
- Applying a numeric opacity accepts values from `0` through `1`.
- The apply button is disabled when the selected track is locked, the value is invalid, the selected clip is not visual, or the callback is unavailable.
- Audio clips and captions do not show the visual opacity group.
- Switching selected clips refreshes the opacity draft from the new context.

## Non-Goals

- No new Rust project action; use `updateVisualClipOpacity`.
- No render-pipeline opacity composition changes.
- No keyframes, animation lanes, blend modes, crop, or transform controls.
- No changes to the existing Source Inspector opacity control.

## Tests

- `AgentPanel` applies selected visual opacity and blank reset values from its Codex context.
- `AgentPanel` blocks selected visual opacity adjustments on locked tracks and invalid values.
- `EditorWorkspace` includes selected clip opacity in Codex context and dispatches `updateVisualClipOpacity` through split-project actions.
