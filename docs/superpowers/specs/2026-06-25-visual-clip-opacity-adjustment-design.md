# Visual Clip Opacity Adjustment

## Context

Palmier documents timeline-native clip adjustments for both manual editors and connected agents. Video Creater already supports trim, split, reorder, source range edits, generated-output swaps, and audio fade/gain edits through durable `ProjectAction` values. The missing adjustment slice is a visual clip property that can be changed without hand-editing JSON.

## Goal

Add a compact opacity adjustment for visual timeline clips.

## Behavior

- Source Inspector shows an `Opacity` numeric control for `video_clip`, `overlay`, and `hyperframe_scene` items.
- The input defaults to the current `properties.opacity` value, or blank when absent.
- Applying opacity sends a dedicated `updateVisualClipOpacity` project action.
- The action accepts finite values from `0` through `1`.
- Values below `1` are stored as `properties.opacity`.
- A value of `1` removes `properties.opacity`, making full opacity the text-file default.
- The action rejects audio clips, captions, missing items, locked tracks, and out-of-range values.
- Codex app-server schema and prompt guidance list `updateVisualClipOpacity` as a supported project action so agents can use the same validated path.

## Non-Goals

- No transform, crop, scale, blend-mode, or keyframed opacity support.
- No render pipeline composition changes in this slice.
- No draggable opacity handles on timeline clips.
- No separate opacity control for caption text styling.

## Tests

- `SourceClipInspector` submits opacity edits and blank defaults.
- `EditorWorkspace` dispatches `updateVisualClipOpacity` through split-project actions.
- Rust reducer stores sub-1 values, removes 1.0, and rejects non-visual items, locked tracks, and out-of-range values.
- App-server project-action schema includes `updateVisualClipOpacity`.
