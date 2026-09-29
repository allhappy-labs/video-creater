---
name: video-creater-video-pipeline
description: Use when implementing or reviewing Video Creater edit generation, EDL rough cuts, transcript-to-timeline planning, ffmpeg renders, draft MP4 validation, HyperFrames render layers, or video-use-style production workflows adapted to this app.
---

# Video Creater Video Pipeline

## Overview

The app is EDL-first. A generated edit must be a real sequence of selected source ranges before captions, graphics, titles, effects, or HyperFrames layers are applied.

## When to Use

- One-click `Generate edit`, rough-cut generation, preset behavior, or agent edit proposals.
- Transcript, Parakeet, moment scoring, source range selection, captions, or EDL logic.
- ffprobe/ffmpeg render plans, draft/final MP4 export, logs, retries, and validation.
- HyperFrames scene or overlay orchestration in the render pipeline.

## Pipeline

```text
INTENT -> TRANSCRIPT -> MOMENTS -> EDL -> TIMELINE -> LAYERS -> RENDER -> REVIEW
```

1. Capture preset, prompt, target duration, language mode, and media id.
2. Probe media and transcribe to word-level timestamps.
3. Score candidate moments from transcript, pauses, energy, and preset rules.
4. Build an EDL with `sourceIn` and `sourceOut` for every primary video clip.
5. Convert the EDL into canonical timeline items.
6. Add captions, title cards, highlights, HyperFrames scenes, and overlays.
7. Build a render plan and run ffmpeg/sidecars through Rust-owned jobs.
8. Validate the draft MP4 and record artifacts/logs.

## Non-Negotiables

- Do not ship a styled full-source pass-through as a generated edit.
- Rust owns canonical project state, validation, render plans, logs, cancellation, and artifact metadata.
- Codex proposes structured changes; Rust validates before mutation.
- HyperFrames returns assets; it does not own the project timeline.
- Visual layers come after the primary cut is real and validated.

## EDL Quality

| Preset | Cut Standard |
| --- | --- |
| Trailer Cut | 30-60 seconds, strong hook, aggressive pacing, high-impact captions and title cards. |
| Highlight Reel | 45-90 seconds, best spoken/visual moments, enough context to understand. |
| Story Cut | 90-180 seconds, narrative continuity, chapter-like beats, less aggressive removal. |

Every selected range should have a reason: hook, payoff, setup, visual action, quote, transition, or needed context.

## Render Review

Check before calling a render complete:

- Output duration is inside preset bounds or has an explicit reason.
- Primary video duration is built from selected clips, not the full source.
- Captions align to selected source ranges after cuts.
- Audio is cut and normalized with the same EDL.
- HyperFrames and overlays appear at intended times and do not hide critical footage.
- ffmpeg logs and artifact paths are stored for troubleshooting.
- The MP4 is playable and has nonzero video and audio streams when expected.

## Implementation Hints

- Keep deterministic scoring useful without Codex; agent guidance should refine, not replace, the baseline.
- Validate negative durations, missing media/artifacts, unsupported overlaps, and track mismatches before mutation.
- Render draft quality first; reserve final settings for accepted drafts.
- Use tiny fixture media for integration tests so regressions catch pass-through renders.

## Common Mistakes

- Generating captions and effects over the whole source instead of cutting first.
- Letting sidecars write canonical project state.
- Trusting agent proposals without typed validation.
- Calling render success before checking duration, streams, timeline coverage, and artifact metadata.
