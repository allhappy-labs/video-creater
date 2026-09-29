# Palmier Source Viewer Full Frame Design

## Intent

Palmier opens selected media sources in the center viewer as full-frame preview tabs. Metadata, generated provenance, references, and AI edit controls live in the right source inspector, while the viewer itself stays visually dominant. Video Creater still renders a separate selected-source details card above the viewport and shows source media as a small thumbnail inside a glassy overlay card. That duplicates the inspector and makes source viewing feel secondary.

## Requirements

- Source mode must not render the separate `Selected source preview details` region above the viewport.
- Source mode must render video, generated video, and image sources as the main full-frame viewport content when a preview URL is available.
- Audio source mode keeps a compact waveform surface because audio has no frame to fill.
- The source viewer may show a small unobtrusive filename/range overlay, but it must not repeat generic kind labels, full paths, or generated provenance inside the viewport.
- Preview transport keeps the duration, source range, scrubber, playback controls, format badges, and `Fit`.
- Viewer tabs, source tab closing, keyboard playback, source stepping, and fallback treatments remain unchanged.
- Generated source provenance remains available through the existing right rail/source inspector path, not duplicated inside the preview panel.

## UI Details

The viewport should read like Palmier's center panel: source media fills the black frame, the play button floats over the frame, and metadata is kept in the transport or inspector. The previous card-like source summary should be removed so the preview starts closer to the tabs and timeline.

## Testing

- Update `PreviewPanel` tests to assert the selected-source details region is absent in source mode.
- Add tests that real video and image source previews use full-frame viewport classes instead of small thumbnail widths.
- Update source-viewer tests to assert no generic kind label or project path is repeated inside the viewport.
- Run the preview panel tests, affected editor workspace tests, typecheck/lint, and browser visual QA.
