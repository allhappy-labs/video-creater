# Palmier Timeline Preview Compositor Design

## Context

Palmier's agent workflow depends on the editor preview being the shared source of truth. Agents can inspect what the user actually sees, and users can trust that timeline playback reflects edits, generated clips, text, captions, overlays, and timing.

Video Creater already has a source viewer with real media playback and a timeline preview model, but timeline preview is still partial. `TimelinePreviewCompositor` resolves active timeline layers, renders still images and motion templates, and shows missing-media states, but video clips are represented by a label placeholder instead of a synced media element.

## Goal

Make the timeline preview panel render ordinary timeline content as real playback: video and generated clips from `sourceIn`/`sourceOut`, images, text overlays, captions, and template overlays at the current playhead.

## Requirements

- Timeline preview renders the active topmost video or generated media clip using a real `<video>` element when a playable local URL exists.
- Timeline preview seeks the media element to `sourceIn + (playheadSeconds - item.startSeconds)` whenever the playhead changes.
- Timeline preview renders images as still layers with opacity and active-time gating.
- Timeline preview renders captions, text overlays, and motion templates over video/image media in timeline order.
- Disabled tracks are excluded from preview.
- Missing media, unsupported sources, and media load failures render inline preview errors with a clear recovery action.
- Source viewer playback remains unchanged; timeline playback uses separate state and does not hijack source tabs.
- The compositor remains deterministic enough for tests: active layer resolution lives in `src/lib/timeline-preview.ts`, while DOM/media concerns live in React components.

## Non-Goals

- No multi-video compositing beyond the currently selected top visible media layer in this slice.
- No audio mixing in browser preview.
- No GPU/effect preview parity yet for color, masks, transitions, or arbitrary GStreamer output.
- No replacement for Rust render validation; this is an interactive preview, not the final render authority.

## Architecture

Extend `src/lib/timeline-preview.ts` from a single-layer helper into a bounded preview-frame model. The model returns media layers, overlay layers, timing, opacity, source offsets, and issues. It stays pure TypeScript and accepts only `Timeline`, `MediaAsset[]`, and `playheadSeconds`.

Split React rendering into small units:

- `timeline-preview-compositor.tsx`: orchestrates the preview frame and layer order.
- `timeline-preview-media-layer.tsx`: renders video/image/generated media, seeks video safely, and reports load errors.
- `timeline-preview-overlay-layer.tsx`: renders captions, text overlays, and template previews.

`PreviewPanel` continues to own viewer mode and transport shell. Timeline mode passes timeline, media, and playhead state into the compositor. Source mode keeps its current `<video>`, `<audio>`, and `<img>` behavior.

## Data Flow

1. Timeline playhead changes through existing timeline controls.
2. `PreviewPanel` passes `timeline`, `media`, and `playheadSeconds` into `TimelinePreviewCompositor`.
3. `buildTimelinePreviewFrame` returns ordered active media and overlay layers.
4. The media layer resolves `convertFileSrc(relativePath)`, renders real video or image, and seeks video to the computed source time.
5. Overlay layers render on top with timeline-local timing and visual priority.
6. Any load or model issue is shown in the preview viewport and exposed through tests.

## Error Handling

- Missing media id: show the item id and missing media id.
- Missing or blank relative path: show an unsupported-source error.
- Video load error: keep the timeline preview shell visible and offer `Open source` when a source tab exists.
- Source offset outside media duration: clamp for preview and include an issue in the preview model.
- Unsupported generated media kind: render a labeled placeholder with an issue instead of a blank preview.

## Tests

- `src/lib/timeline-preview.test.ts` covers active media layer ordering, disabled tracks, source offset clamping, overlay extraction, and missing media issues.
- `src/components/workspace/timeline-preview-compositor.test.tsx` covers real video element rendering, seek synchronization, image rendering, overlay rendering, and load-failure messaging.
- `src/components/workspace/preview-panel.test.tsx` covers timeline/source mode isolation and keyboard transport not controlling the wrong viewer.
- Browser QA checks a desktop and narrow viewport with video, image, caption, and template layers.

## Success Criteria

The preview panel no longer shows a label-only placeholder for ordinary video clips. A user can scrub the timeline and see the selected source range at the expected frame with captions/templates overlaid, while failures are visible in the preview rather than hidden in logs.

## Self-Review

- Scope is focused on interactive timeline preview, not final rendering or full multi-layer compositing.
- No placeholders or deferred requirements remain.
- The design keeps Rust render authority intact and keeps source viewer behavior separate from timeline preview behavior.
