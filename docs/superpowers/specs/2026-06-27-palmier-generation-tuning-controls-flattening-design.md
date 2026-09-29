# Palmier Generation Tuning Controls Flattening

## Intent

Palmier keeps generation format controls in the sheet flow instead of framing them as a secondary settings card. Video Creater's media generation sheet already removed duplicate recipe and settings-summary blocks, but `Generation footer tuning controls` still renders as a rounded bordered mini-card around model, duration, aspect, and size controls.

## Requirements

- Keep `Generation footer tuning controls` as the accessible group for model and output controls.
- Remove the rounded border, background shell, and padded card styling from that group.
- Preserve the `Generation model`, `Generation duration`, `Generation aspect ratio`, and `Generation resolution` controls and their current payload behavior.
- Preserve audio/image/video mode-specific visibility rules.
- Preserve the `Generation submit footer`, estimate, readiness, and queue behavior.
- Do not change generated asset metadata, Temporal workflow records, fal.ai provider selection, or project schema.

## Testing

- `MediaBin` verifies `Generation footer tuning controls` is present but no longer uses rounded bordered card styling.
- Existing generation tests continue to prove model, duration, aspect, resolution, estimate, and queued request payloads remain unchanged.
- Browser QA checks the open generation sheet at desktop and narrow widths for readable controls and no horizontal overflow.
