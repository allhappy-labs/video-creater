# Palmier Generated Recipe Summary Rows Design

## Goal

Make generated source details in the right inspector read like Palmier's project/source inspector: compact label/value rows instead of a grid of boxed mini-cards. The summary should preserve the same recipe facts while reducing visual weight.

## Design

- Keep the `Generated recipe summary` group and its current fields: model, aspect, resolution, duration, and status.
- Render those fields as flat rows using the inspector's label/value rhythm.
- Remove the rounded bordered summary container and per-field boxed backgrounds.
- Preserve ordering above `Generated details` and before direct `Generated AI edit` controls.

## Testing

- `SourceClipInspector` proves the summary remains before details and AI edit controls.
- `SourceClipInspector` proves all recipe values remain visible.
- `SourceClipInspector` proves the summary and each row no longer use rounded bordered mini-card styling.
