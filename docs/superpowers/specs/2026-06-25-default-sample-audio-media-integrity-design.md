# Default Sample Audio Media Integrity Design

## Problem

Video Creater's default React sample project demonstrates imported footage, generated timeline
clips, captions, and an audio bed. The sample media index names the audio asset
`media-voiceover`, but the cloned shared timeline still points the `Music bed` item at `media-2`.
That leaves the first-run project with a timeline/media mismatch, which weakens the
text-file-editable project contract agents depend on.

Palmier's connected-agent workflow depends on full project context for trim, split, reorder, and
adjust operations. The default sample should model that contract with valid media ids.

Palmier reference: https://www.palmier.io/docs

## Goals

- Make the default React sample timeline reference the existing `media-voiceover` audio asset.
- Preserve the shared `sampleTimeline` fixture for lower-level timeline tests.
- Keep the `Music bed` clip label, waveform, fade-out, and source range behavior unchanged.
- Ensure the Codex selected timeline clip context mentions the real audio media id.

## Non-Goals

- No project schema migration.
- No changes to Rust fixtures or split-project validators.
- No new media import, waveform, render, Temporal, or fal.ai behavior.

## Design

`createDefaultSampleTimeline()` will continue to clone `sampleTimeline`, but it will also rewrite
the cloned `Music bed` timeline item source from `media-2` to `media-voiceover`. This keeps the
generic fixture unchanged while making the first-run editor sample internally consistent.

The existing `agentSelectedTimelineClipContext()` data path will then expose `@media-voiceover`
when the user selects the audio clip, and existing audio controls will keep reading fade-out and
volume properties from the timeline item.

## Acceptance Criteria

- Selecting `Music bed` in the default workspace shows `@media-voiceover` in the Codex selected
  timeline clip context.
- The mention-source action inserts `@media-voiceover` into the Codex prompt.
- Existing audio waveform and fade-out visuals remain visible.
- The shared `sampleTimeline` fixture remains unchanged.
