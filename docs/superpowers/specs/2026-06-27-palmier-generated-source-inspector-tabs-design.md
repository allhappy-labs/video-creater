# Palmier Generated Source Inspector Tabs Design

## Intent

Palmier keeps generated source inspection focused with a compact `Details` / `AI Edit` tab row. `Details` shows file metadata, output choices, references, and the prompt; `AI Edit` contains actions that create new generated media, such as prompt tweaks, reruns, replacements, and upscales.

Video Creater currently shows generated provenance and generated edit controls together in one long source inspector. That makes the right rail harder to scan and differs from the Palmier interaction pattern shown in the reference screenshots.

Palmier reference: https://www.palmier.io/docs

## Requirements

- Add a generated source inspector tab row with `Details` and `AI Edit` for generated assets.
- Default generated assets to `Details`.
- Keep file metadata, output choices, references, and prompt in `Details`.
- Keep the compact generated recipe summary above the tabs so model, aspect, resolution, duration, and status remain visible while editing.
- Move generated media actions into `AI Edit`, including rerun, rerun-and-replace, upscale, variation prompt, queue variation, replacement target, and queue replacement.
- Preserve existing Temporal workflow queue actions and fal.ai generation payload behavior.
- Preserve imported source AI edit behavior unless the selected source is generated.
- Keep the tab row compact and readable in the right inspector rail.

## Testing

- Add `SourceClipInspector` coverage proving generated sources render the `Details` / `AI Edit` tab row and default to `Details`.
- Prove the variation prompt and queue buttons are hidden on `Details` and visible after selecting `AI Edit`.
- Keep coverage proving generated references remain revealable from `Details`.
- Keep coverage proving rerun, replacement rerun, variation, replacement variation, and upscale actions still call the same callbacks.
- Run focused source inspector tests, editor workspace integration tests that touch generated inspectors, lint, full tests, and browser QA.

## Self-Review

- Scope is limited to generated source inspector chrome and tests.
- No project schema, timeline schema, provider, Temporal, fal.ai, or generated asset metadata changes.
- The tab row changes presentation only; queued action callbacks and payloads remain unchanged.
