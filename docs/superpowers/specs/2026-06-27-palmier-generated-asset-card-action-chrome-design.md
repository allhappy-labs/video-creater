# Palmier Generated Asset Card Action Chrome Design

## Goal

Make generated asset cards in the media rail feel more like Palmier's thumbnail-first asset grid. Video Creater currently renders a full-width `Use in composer` text button on every generated asset card, which makes the media rail read like a stack of command panels instead of an asset browser.

## Requirements

- Keep the existing `Use <asset> in composer` action and callback unchanged.
- Replace the always-visible full-width `Use in composer` text row with a compact icon button inside the generated asset card chrome.
- The compact action must keep an accessible label so keyboard and screen-reader users can still find the same command.
- Generated asset previews, status strips, workflow badges, output selection, replacement, insertion, retry, and mock-generation actions remain unchanged.
- The action should sit near the generated asset title so cards without an output preview still expose the composer action.
- Avoid adding a new toggle, menu, popover, or mode switch.

## Testing

- Add or update media-bin tests so generated asset cards still expose `Use <asset> in composer` by role/name.
- Assert the generated asset card no longer contains visible `Use in composer` text or a full-width composer action row.
- Keep coverage for output selection and asset preview rendering.
- Run focused media-bin tests, the full media-bin suite, TypeScript, diff check, full Vitest, and browser QA at desktop and narrow widths.
