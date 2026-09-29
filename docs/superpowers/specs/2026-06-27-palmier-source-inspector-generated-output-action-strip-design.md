# Palmier Source Inspector Generated Output Action Strip Design

## Goal

Keep the right-rail source inspector closer to Palmier's compact generated-source details. Video Creater still renders `Replace selected clip` and `Insert on timeline` as full-width text buttons at the top of generated source details, which makes the rail read like a command form instead of a dense inspector.

## Requirements

- Keep generated source replacement and timeline insertion callbacks unchanged.
- Replace the full-width text rows with a compact icon action strip in the `Generated details` group.
- Preserve accessible labels:
  - `Replace <clip label> with <media id>`
  - `Insert <media id> on timeline`
- Do not hide the actions behind a menu, toggle, popover, or mode switch.
- Leave generated output choices, generation metadata, prompt copy, variation queue, replacement queue, and workflow behavior unchanged.

## Testing

- Update source inspector tests so replacement and insertion actions remain available by role/name.
- Assert those actions no longer expose visible `Replace selected clip` or `Insert on timeline` text.
- Assert those actions are compact icon buttons instead of full-width action rows.
- Run focused source-inspector tests, the full source-inspector suite, affected editor-workspace tests, TypeScript, diff check, full Vitest, and browser QA at desktop and narrow widths.
