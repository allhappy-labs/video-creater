# Palmier Imported Source Details Wrapper Flattening Design

## Goal

Remove the last nested wrapper shell around imported source details in the right inspector. Imported file metadata and imported AI edit controls are already flat; the combined wrapper should not reintroduce a rounded bordered panel around them.

## Design

- Expose the combined imported source detail area as an accessible `Imported source details` group.
- Remove the combined wrapper's rounded border, muted background, and padding shell.
- Preserve imported file details, imported AI edit controls, source insertion, referenced generation, and upscale behavior.
- Keep timeline editing, queue payloads, Temporal workflow behavior, fal.ai integration, and media selection unchanged.

## Testing

- `SourceClipInspector` verifies imported source details are a flat accessible group.
- Existing assertions continue to prove imported file details and imported AI edit controls are flat and functional.
