# Timeline Visual Opacity Affordance

## Context

Palmier's timeline keeps clip state visible in the editing lanes, so manual editors and connected agents can understand adjustments without opening a separate panel first. Video Creater now supports a validated `updateVisualClipOpacity` project action and Source Inspector control, but timeline clips do not yet show that opacity state.

## Goal

Render visual opacity metadata directly on timeline clips.

## Behavior

- `video_clip`, `overlay`, and `hyperframe_scene` items with finite `properties.opacity` values from `0` through less than `1` show an opacity affordance in the clip body.
- The clip body renders a non-interactive dimming overlay whose strength is `1 - opacity`.
- The clip title and metadata row include `opacity NN%` so the value remains inspectable by text and assistive technology.
- `opacity: 1`, missing opacity, non-finite values, out-of-range values, audio clips, and captions do not render opacity metadata.
- Existing selection, drag, resize, source boundary, source mismatch, generated badge, workflow badge, provenance, audio waveform, fade, and volume behavior remains unchanged.

## Non-Goals

- No render-pipeline opacity composition.
- No opacity keyframes or timeline automation lanes.
- No caption text style opacity controls.
- No inspector or project-action changes in this slice.

## Tests

- `TimelineEditor` renders opacity text, title metadata, and a dimming overlay for a visual clip with `opacity: 0.45`.
- `TimelineEditor` omits opacity metadata for full-opacity and invalid opacity values.
- `TimelineEditor` omits opacity metadata for audio and caption items even if they carry an `opacity` property.
