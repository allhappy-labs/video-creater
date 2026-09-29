# Generation Mode Selector Metadata Design

## Context

Palmier's generation composer keeps mode, model, duration, aspect, and credit cost close to the
submit controls so editors can check the request before queueing it. Video Creater already has mode
tabs, model/settings fields, an active recipe, and a credit estimate, but the Image/Video/Audio mode
selector only shows the mode names. When switching modes, the selected model and cost are not
visible until the user scans other parts of the composer.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the media generation mode selector show the selected provider model and estimated credit cost
for each generation mode.

## Behavior

- Each Image, Video, and Audio mode button shows:
  - the mode icon and label;
  - the currently selected model label for that mode;
  - an estimated credit cost for that mode using the current duration setting.
- The active mode remains visually selected.
- Changing duration updates the estimates for all modes.
- Clicking a mode still changes only the active generation mode and does not mutate prompt,
  references, placement, or queued request shape.

## Non-Goals

- No pricing or real billing integration.
- No new model options.
- No request contract or Temporal workflow change.

## Verification

- Add a `MediaBin` test proving the mode selector exposes model and credit metadata.
- Run the focused media bin test file.
- Run `pnpm lint`.
- Browser-smoke the media composer at desktop width and confirm the mode buttons render without
  clipped or overlapping text.
