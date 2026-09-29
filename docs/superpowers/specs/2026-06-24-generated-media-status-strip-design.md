# Generated Media Status Strip

## Context

Palmier's media library thumbnails make AI assets easy to scan: generated clips carry an `AI` badge, a duration badge, and a thin colored strip that distinguishes generated assets from imported footage. Video Creater already shows the `AI` badge and duration on generated media tiles, and the generated history list shows workflow status. The missing cue is a compact status strip directly on generated output media tiles.

## Goal

Make generated media outputs in the project library visually carry their generation status without opening the generated asset history panel.

## Behavior

- Grid media tiles for generated outputs show a thin bottom status strip.
- List media rows for generated outputs show the same status strip inside the thumbnail column.
- The strip uses the related generated asset status when the media id is one of that asset's outputs.
- Completed outputs use a cyan strip; queued/running outputs use amber; failed outputs use destructive red.
- Imported video, image, and audio media do not show a generation status strip.

## Non-Goals

- No new generated asset schema fields.
- No changes to workflow status calculation.
- No new queue actions or provider calls.
- No changes to generated asset cards in the `AI generations` section.

## Tests

- Add a `MediaBin` test that renders generated output media linked to completed and running generated assets.
- Assert both grid and list media views expose `Generation status completed` and `Generation status running`.
- Assert an imported media tile does not expose a generation status strip.
