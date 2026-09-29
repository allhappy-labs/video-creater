# Timeline Generated Clip Badges Design

## Context

Palmier makes AI-generated media visible at the editing surface: library tiles carry an `AI` badge, generated clips remain visually distinct on the timeline, and the selected clip can be inspected or regenerated without losing context. Video Creater already stores generated provenance on timeline items through generated sources and `generatedAssetId` / `generatedOutputMediaId` properties, but the timeline clip body does not expose that provenance.

## Goal

Show a compact AI provenance badge directly on timeline clips that are backed by generated assets, so editors can identify generated shots before opening the inspector.

## Behavior

- A timeline item is considered AI-backed when either:
  - `item.source.type === "generated"`, or
  - `item.properties.generatedAssetId` is a non-empty string, or
  - `item.properties.generatedOutputMediaId` is a non-empty string.
- AI-backed clips render a small `AI` badge inside the clip body.
- The badge stays visible on video, HyperFrames, overlay, caption, and audio clip styles without changing selection, drag, trim, or double-click behavior.
- The clip accessible name stays the clip label; the badge is decorative and does not create a second focus target.
- Very short clips may visually compress the badge and label, but the clip body must not resize the timeline item or overlap neighboring clips.

## Visual Treatment

- Place the badge before the clip label in the top row.
- Use a compact rounded badge with high contrast against the clip color.
- Preserve the existing metadata row for source ranges and reasons.
- Do not use a separate card, popover, or inspector-only treatment for this slice.

## Verification

- Timeline editor tests prove generated clips render an AI badge for explicit generated sources.
- Timeline editor tests prove generated clips render an AI badge when generated provenance is stored in properties.
- Existing timeline click, double-click, drag, trim, waveform, and toolbar tests keep passing.
- Browser QA captures desktop and narrow timeline views and confirms the badge does not overlap the toolbar or clip text.
