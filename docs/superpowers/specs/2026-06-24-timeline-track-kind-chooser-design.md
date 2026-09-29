# Timeline Track Kind Chooser Design

## Context

Video Creater can create a new video track through a Rust-validated `createTrack` project action. The underlying track draft helper already supports video, HyperFrames, overlays, captions, and audio, but the timeline toolbar only exposes video creation.

## Goal

Let editors choose which kind of empty timeline lane to create from the compact timeline toolbar.

## Behavior

- The timeline toolbar shows a small `New track kind` selector next to the existing `Add timeline track` button.
- The selector supports `Video`, `HyperFrames`, `Overlays`, `Captions`, and `Audio`.
- The plus button creates a track using the currently selected kind.
- The default remains `Video` so the existing one-click behavior is preserved.
- The selected kind is local UI state only; the persisted project state is still the resulting `createTrack` action.
- Generated ids and names continue to use the existing stable draft convention, such as `track-audio-2` / `Audio 2`.

## Visual Treatment

- Keep the selector compact enough to fit in the toolbar at desktop widths.
- On narrow widths, wrap the timeline tools into compact rows so the selector and plus button stay visible without horizontal clipping.
- Use plain text labels rather than icons for the kind names, because track kind choice is a semantic decision.
- The add button remains icon-only with the existing accessible label.

## Verification

- Timeline editor tests prove changing the selector changes the kind passed to `onAddTrack`.
- Workspace tests prove creating an audio track submits `createTrack` with `track-audio-2`, `Audio 2`, and `afterTrackId: track-audio`.
- Browser QA checks the selector and plus button fit in desktop and narrow timeline toolbars.
