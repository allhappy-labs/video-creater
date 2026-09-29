# Palmier Generation Compact Settings Strip Design

## Context

Palmier keeps generation settings close to the queue action in a short strip: model, output details, credit cost, and the submit action are visible without a tall secondary settings block. Video Creater already removed duplicate recipe summaries and made the queue action compact, but the composer still renders model, duration, aspect, and resolution as four stacked field rows inside `Generation footer tuning controls`. That makes the focused sheet taller than the Palmier target and pushes prompt/reference context away from the media grid.

## Goal

Compress the existing generation tuning controls into a compact settings strip while preserving the current request behavior:

- Keep `Generation footer tuning controls` as the accessible group.
- Keep `Generation model`, `Generation duration`, `Generation aspect ratio`, and `Generation resolution` as real select controls.
- Render the visible controls in one compact responsive grid instead of a stacked form.
- Use short visual labels: `Model`, `Duration`, `Aspect`, and `Size`.
- Keep mode-specific visibility:
  - Video: model, duration, aspect, size.
  - Image: model, aspect, size.
  - Audio: model, duration.
- Keep `Generation submit footer`, estimate, readiness, and queue behavior unchanged.

## Non-Goals

- No schema changes.
- No Temporal workflow changes.
- No fal.ai or model-default changes.
- No new settings.
- No popover or modal settings editor.

## UI Requirements

The settings strip should read as one operational control row rather than a card:

1. The `Generation footer tuning controls` group uses a compact grid.
2. It does not render a nested `Generation output settings` group.
3. Each select uses compact height and small text but remains keyboard-accessible.
4. Long model ids are truncated inside the select container rather than expanding the sheet.
5. The submit footer remains separate and continues to contain only the estimate and queue button.

## Tests

Update `MediaBin` coverage to prove:

- The footer tuning group has compact grid styling and no nested `Generation output settings` group.
- Video mode still exposes model, duration, aspect, and resolution controls.
- Image mode still omits duration but keeps model, aspect, and resolution.
- Audio mode still exposes model and duration but omits visual output controls.
- Existing queue payload tests continue to prove settings values are submitted unchanged.

Run:

```sh
rtk ./node_modules/.bin/vitest run src/components/workspace/media-bin.test.tsx
rtk ./node_modules/.bin/tsc --noEmit
rtk git diff --check
rtk ./node_modules/.bin/vitest run
```
