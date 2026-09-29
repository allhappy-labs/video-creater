# Preview Transport Status Strip

## Context

Palmier's viewer keeps editing context close to the preview: tabs identify timeline/source views, the viewer shows media, and a compact transport/status strip exposes time, playback controls, aspect, frame rate, and fit state. Video Creater now has source-aware preview content, but the preview well still jumps directly from the canvas to render review with only a central play button.

## Goal

Add a compact preview transport/status strip that makes the active viewer context legible without introducing playback state.

## Behavior

- The preview panel renders a `Preview transport` region under the preview well.
- The strip shows current time at `00:00:00` and the active duration.
- In source mode, duration and format metadata come from the selected source.
- In timeline mode, duration comes from the selected timeline item when present.
- The strip shows source range when the active source was opened from a timeline clip.
- The strip includes compact transport icon buttons and a disabled scrubber-style progress rail.
- The existing central play button and render review panel remain unchanged.

## Non-Goals

- No real playback, seeking, frame stepping, or media decoding.
- No timeline playhead synchronization.
- No Tauri command or schema change.
- No render report behavior change.

## Tests

- Source mode shows preview transport with source duration, range, resolution, and fps.
- Timeline mode shows preview transport with selected item duration and kind.
- The transport scrubber is present and disabled/static.
