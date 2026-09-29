# Palmier Generated AI Edit Shell Flattening Design

## Goal

Make generated-source edit actions feel like a direct inspector flow instead of a nested card. Palmier's right rail keeps contextual actions visible with minimal panel chrome; Video Creater still wrapped `Generated AI edit` in its own rounded border and background inside generated details.

## Design

- Keep `Generated AI edit` as an accessible region.
- Remove the region's rounded border, background, and padding shell.
- Preserve existing actions: use in composer, rerun same prompt, rerun and replace, queue upscale, variation prompt, queue variation, and queue replacement.
- Do not change generated asset data, queue payloads, Temporal workflow behavior, fal.ai integration, or media selection behavior.

## Testing

- `SourceClipInspector` verifies generated details and AI edit remain direct and tabless.
- The same test verifies the AI edit region no longer carries the nested card shell classes.
- Existing generated AI edit tests continue to cover rerun, replacement, variation prompt, and queue behavior.
