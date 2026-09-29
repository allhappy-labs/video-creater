# Timeline Video Filmstrip Clips Design

## Context

Palmier's timeline screenshots make video clips readable before the editor opens an inspector: visual clips show thumbnail-like bands, generated shots carry AI context, and audio tracks show waveform structure. Video Creater already renders audio waveform clips and generated AI badges, but visual timeline clips still read mostly as flat blue blocks. That weakens the manual editor because imported and generated video clips do not visually communicate that they are media-backed objects.

## Goal

Render video timeline clips with a compact deterministic filmstrip treatment so imported and generated clips look like editable media on the timeline without requiring decoded thumbnails.

## Behavior

- `video_clip` items render a decorative filmstrip layer inside the clip body.
- The filmstrip uses deterministic cells based on the clip id, duration, and source id so repeated renders are stable.
- Imported video clips and generated output clips use slightly different color accents, while preserving the existing generated `AI` badge.
- The clip label, source range metadata, selection ring, drag body, resize handles, and double-click source opening behavior remain unchanged.
- The filmstrip is hidden from assistive technology and does not add a focus target.
- Very short clips may compress the treatment, but the item must not resize or overlap neighboring timeline items.

## Non-Goals

- No media decoding, ffprobe thumbnail extraction, cache files, or schema migration.
- No changes to generated asset records, Temporal workflow records, render output, or project actions.
- No filmstrip treatment for audio, captions, overlays, or HyperFrames in this slice.

## Testing

- `TimelineEditor` tests prove imported video clips render a filmstrip region.
- Tests prove generated video clips keep the `AI` badge and render the filmstrip with generated styling.
- Existing timeline click, double-click, drag, resize, trim, waveform, toolbar, and track-control tests continue to pass.

## Browser QA

- Desktop: verify imported and generated visual clips show filmstrip texture without obscuring labels or resize handles.
- Narrow: verify the timeline remains contained and clip labels do not overlap adjacent controls.
