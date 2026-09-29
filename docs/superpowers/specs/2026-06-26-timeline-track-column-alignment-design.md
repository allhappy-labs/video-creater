# Timeline Track Column Alignment Design

## Context

Palmier's timeline keeps lane labels, ruler ticks, and clip rows aligned so manual edits feel spatially
predictable. Video Creater's compact lane labels now use short labels such as `V1` and `A1`, but the
timeline can still reserve too much space for the track header after full visible names were removed.
That leaves unused rail width and gives less room to clips in the center editor.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep the timeline ruler header and track row grid on the same compact Palmier-style track-column width.

## Behavior

- The timeline ruler header uses the same 112px track column as the track rows.
- Track lane labels, link/media-state controls, ruler ticks, and the clip canvas remain aligned.
- Existing compact lane labels, timeline click seeking, keyboard shortcuts, zoom, and selected-clip
  actions remain unchanged.

## Non-Goals

- No new timeline sizing preference.
- No changes to clip placement math or project files.
- No redesign of track controls.

## Verification

- `TimelineEditor` test proves the ruler header uses the same compact track-column class as the row
  grid.
- Run the focused `TimelineEditor` test and `pnpm lint`.
- Browser-smoke the timeline to confirm header and lanes visually line up.
