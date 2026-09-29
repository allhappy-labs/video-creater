# Palmier Generation Reference Tabs Design

## Intent

Palmier's generation sheet keeps video reference setup compact by separating `First/Last` frame choices from broader `Reference` media. Video Creater currently shows first frame, last frame, and reference controls at the same time, which makes the media generation sheet taller and less focused.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Add a compact tab row inside video generation reference controls with `First/Last` and `Reference`.
- Default video generation to the `First/Last` tab so selected media seeding remains immediately visible.
- Hide the reference media selector and reference drop target until the `Reference` tab is selected.
- Preserve image generation behavior, where the reference selector remains directly visible.
- Preserve selected first-frame, last-frame, and reference media in request payloads, active recipe rows, drag/drop handlers, and remove actions.
- Keep model, duration, aspect, resolution, Temporal workflow, fal.ai provider, and generated asset metadata paths unchanged.

## Testing

- Add `MediaBin` coverage proving video generation renders the tab row, defaults to `First/Last`, hides the reference selector on that tab, and shows it after selecting `Reference`.
- Keep coverage proving explicit reference selection still submits the same payload.
- Keep coverage proving image generation exposes reference media without the video-only tab row.
- Run focused media-bin tests, editor-workspace generation coverage, lint, full tests, and browser QA for desktop and narrow composer layouts.

## Self-Review

- Scope is limited to visible video generation reference chrome plus tests.
- No project file, timeline, provider, Temporal, or generated asset schema changes.
- The active recipe remains the persistent summary for hidden-tab selections, so the tab does not obscure queued request state.
