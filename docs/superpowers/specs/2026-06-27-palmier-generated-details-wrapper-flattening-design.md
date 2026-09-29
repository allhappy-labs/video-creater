# Palmier Generated Details Wrapper Flattening Design

## Goal

Remove the remaining nested wrapper shell around generated source details in the right inspector. Generated metadata, prompt, and AI edit controls are already flat, so the combined wrapper should not reintroduce a rounded bordered panel around them.

## Design

- Expose the generated detail area as an accessible `Generated details` group.
- Remove the generated details wrapper's rounded border, muted background, and padding shell.
- Preserve generated file details, reference previews, prompt copy, output actions, AI edit controls, variation queueing, and replacement queueing.
- Keep timeline editing, queue payloads, Temporal workflow behavior, fal.ai integration, and media selection unchanged.

## Testing

- `SourceClipInspector` verifies generated details are a flat accessible group.
- Existing assertions continue to prove generated file details, generated prompt, and generated AI edit controls are flat and functional.
