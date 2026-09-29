# Source-Aware Preview Surface

## Context

Palmier's viewer makes opened source footage feel central: timeline and source tabs sit above a visible preview, while the right rail can inspect prompt, references, and AI edit controls. Video Creater already has timeline/source tabs and source metadata, but source mode still leaves the main preview well visually empty unless a motion template item is selected.

## Goal

Make `PreviewPanel` source mode render a source-aware preview surface that communicates what is open, what range is being inspected, and whether the source is video, image, audio, or generated media.

## Behavior

- Timeline mode keeps the existing motion-template preview and play control.
- Source mode renders a dedicated `Source viewer` region inside the preview well.
- The source viewer shows:
  - selected source label,
  - kind badge,
  - duration, resolution, and fps metadata when present,
  - source range when opened from a timeline clip,
  - path label.
- Video/generated sources use a restrained film-frame treatment.
- Image sources use an image-frame treatment.
- Audio sources use an audio waveform-style treatment.
- The play button remains available and accessible.

## Non-Goals

- No actual media decoding or file URL loading in this slice.
- No new Tauri command or project schema change.
- No waveform extraction or thumbnail generation.
- No timeline playback state.

## Tests

- Source mode renders `Source viewer` with label, metadata, range, and path.
- Audio source mode renders an audio treatment instead of the video treatment.
- Timeline mode does not render `Source viewer`.
