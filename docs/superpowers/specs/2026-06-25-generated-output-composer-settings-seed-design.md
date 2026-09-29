# Generated Output Composer Settings Seed

## Context

Palmier keeps AI-generated clips editable in place: users can inspect the source prompt,
first/last/reference inputs, model, duration, resolution, and aspect ratio, then continue or vary
the shot without rebuilding the setup manually. Video Creater now seeds the media generation
composer from selected generated-output references, but still opens with default output settings.

Palmier reference: https://www.palmier.io/docs

## Goal

When opening the media generation composer from a selected generated output, reuse the generated
asset's compatible mode and output settings so the next generation starts from the same production
setup.

## Behavior

- If the selected media is not a generated output, composer opening keeps the existing mode and
  setting behavior.
- If the selected media is a generated output, derive the composer mode from the source generated
  asset.
- Seed supported duration, aspect ratio, and resolution controls from the generated asset settings.
- Seed the model selector only when the generated asset model is supported by that composer mode.
- Do not copy the previous prompt automatically; prompt reuse remains an explicit generation-history
  action.
- Submitting still emits the existing `MediaGenerationRequest` shape.

## Non-Goals

- No schema change.
- No new Temporal workflow or fal.ai provider behavior.
- No automatic generation history selection.
- No support for unsupported provider/model options in the composer.

## Verification

- Media Bin test: selecting a generated output with 9:16, 8s, 1080x1920 provenance opens the video
  composer with those settings selected.
- Media Bin test: submitting that composer sends the same model and settings in the generated media
  request.
- Existing Media Bin generation, reference, and history tests continue passing.
