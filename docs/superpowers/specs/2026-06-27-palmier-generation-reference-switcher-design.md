# Palmier Generation Reference Switcher Design

## Context

Palmier's generation composer keeps the media library visible and opens a compact sheet for generation settings. For video generation, the sheet exposes a small choice between `First/Last` frame controls and general `Reference` media instead of showing every reference control at once. Video Creater already supports first frame, last frame, and reference media in generated asset requests, but the current composer renders the first/last slots and the reference list together. That makes the sheet taller than the Palmier target and pushes the prompt and submit controls away from the media grid.

## Goal

Make the Video Creater media generation composer closer to Palmier by replacing the always-visible video reference stack with a compact switcher:

- `First/Last` shows only the first-frame and last-frame drop/select slots.
- `Reference` shows only the general reference-media picker and selected reference chips.
- Image generation keeps the existing reference-only picker.
- Audio generation keeps no visual reference controls.

This is a presentation change. Existing generation request payloads, generated asset records, timeline placement behavior, mock completion, and fal.ai model defaults stay unchanged.

## UI Behavior

When the composer opens in video mode:

1. The reference area shows a two-option control labelled `Generation reference mode`.
2. `First/Last` is selected by default.
3. Only the first-frame and last-frame slots are visible in that mode.
4. Selecting `Reference` hides the first/last slots and shows the reference picker.
5. Switching modes does not clear selected first frame, last frame, or reference media. Hidden values remain part of the queued request.

When the composer opens in image mode:

1. There is no `First/Last` option.
2. The reference picker remains visible, matching the current image generation behavior.

When the composer opens in audio mode:

1. No visual reference slots or switcher are rendered.

## Implementation Notes

- Add local composer state for the selected video reference pane.
- Reuse the existing `renderGenerationReferenceSlot` and `renderGenerationReferenceList` helpers.
- Use compact button styling consistent with the existing mode selector rather than adding a card.
- Keep accessible names stable enough for tests:
  - group: `Generation reference mode`
  - buttons: `First/Last` and `Reference`
  - first/last group: `Generation first and last frame slots`
  - reference group: `Reference slot`

## Tests

Add or update media-bin tests to prove:

- Video generation defaults to `First/Last`, shows first/last slots, and hides the general reference slot.
- Selecting `Reference` shows the reference slot and hides first/last slots.
- Switching back to `First/Last` keeps the first-frame selection visible.
- Image generation still shows the reference slot without rendering the video switcher.
- Audio generation still renders no visual reference controls.

Run:

```sh
rtk ./node_modules/.bin/vitest run src/components/workspace/media-bin.test.tsx
rtk ./node_modules/.bin/tsc --noEmit
rtk git diff --check
```
