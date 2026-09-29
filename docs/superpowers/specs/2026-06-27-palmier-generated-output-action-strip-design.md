# Palmier Generated Output Action Strip Design

## Goal

Keep generated output cards thumbnail-first like Palmier's media grid. Video Creater currently renders `Replace selected clip` and `Insert on timeline` as full-width text buttons under each generated output, which makes a single generated asset consume too much vertical space and reads more like a command form than a media browser.

## Requirements

- Keep output selection unchanged: clicking the output card still selects that generated media output.
- Keep replacement and timeline insertion callbacks unchanged.
- Replace the full-width text rows with a compact icon action strip aligned to the output card.
- Preserve accessible labels:
  - `Replace <clip label> with <media id>`
  - `Insert <media id> on timeline`
- Do not hide these actions behind a menu, popover, or mode switch.
- Do not change workflow status, mock generation, retry, output metadata, or selected-output styling.

## Testing

- Update media-bin tests so replacement and insertion actions remain available by role/name.
- Assert those actions no longer expose visible `Replace selected clip` or `Insert on timeline` text.
- Assert they are compact icon buttons rather than full-width action rows.
- Keep existing output-selection tests unchanged.
- Run focused media-bin tests, the full media-bin suite, TypeScript, diff check, full Vitest, and browser QA at desktop and narrow widths.
