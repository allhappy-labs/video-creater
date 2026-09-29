---
name: video-creater-graphics
description: Use when creating or briefing generated video graphics for Video Creater, including HyperFrames scenes, overlays, title cards, lower thirds, captions, diagrams, callouts, visual layers, transparent assets, and preview graphics.
---

# Video Creater Graphics

## Overview

Generated graphics are timeline assets with jobs to do. Each asset must carry a beat in the edit: hook, explain, label, transition, emphasize, or clarify.

## When to Use

- HyperFrames full-frame scenes or transparent overlay assets.
- Title cards, lower thirds, chapter cards, callouts, diagrams, arrows, highlights, or visual explainers.
- Caption style as visual design, not only subtitle text.
- Prompting or reviewing generated visual layers for draft MP4 renders.

## Asset Contract

Define this before generation:

```json
{
  "role": "title_card | overlay | lower_third | caption | diagram | transition",
  "timelineStart": 12.4,
  "duration": 2.8,
  "dimensions": "1920x1080",
  "fps": 30,
  "alpha": true,
  "safeZone": "keep essential text inside 10% margins",
  "sourceBeat": "what this graphic helps the viewer understand"
}
```

## Creative Standard

- The graphic must be legible over real video, not just on a dark test background.
- Use sparse words; let shape, scale, color, timing, and position carry meaning.
- Captions can be bold and expressive, but must stay readable at phone size.
- Overlays need alpha, clear edges, and predictable safe zones.
- Full-frame scenes should feel like edited footage beats, not slides.
- Visuals must feel current, intentional, and native to the prompt. Do not accept generic template graphics.
- Avoid full-width opaque black caption slabs, static text-only cards, centered text on plain boxes, default system fonts, and long holds with no motion.
- Prefer transparent or semi-transparent materials, shaped masks, kinetic typography, accent strokes, subtle depth, responsive placement, and quick in/out motion.
- Every visual asset brief must include `visualTreatment`, `motion`, `safeZone`, and `avoid` notes.

## Quality Gate

Reject or revise a visual layer before render if any of these are true:

- It could be described as "big text on a black rectangle."
- It covers more than 30% of the frame for more than 1.5 seconds without a deliberate title-card reason.
- It hides faces, hands, product details, or the main action when a smaller treatment would work.
- It repeats the same composition for three or more captions in a row.
- It uses transcript text that conflicts with the requested language or prompt.
- It lacks a concrete motion plan: pop, slide, wipe, scale, type-on, tracking callout, or timed reveal.

## Visual Language

| Asset | Use |
| --- | --- |
| Title card | Hook, chapter break, dramatic reset, or story context. |
| Lower third | Person, place, term, metric, or quoted source. |
| Callout | Direct attention to a visual detail in the underlying clip. |
| Diagram | Explain structure, sequence, comparison, or system behavior. |
| Highlight | Add emphasis to a selected moment without hiding the clip. |
| Transition | Bridge two cuts only when the pacing benefits from it. |

## Prompt Checklist

- State the asset role and timeline beat.
- Include exact text, if any; do not let generated systems rewrite it.
- Specify background: transparent overlay or full-frame scene.
- Specify dimensions, fps, duration, and safe-zone constraints.
- Specify where the asset enters, holds, and exits.
- Name the palette and typography mood in concrete terms.

## Review Checklist

- First, middle, and last frames communicate the intended beat.
- No text falls outside safe margins or sits under app/video controls.
- Overlay does not obscure faces, hands, product details, or captions unless intentional.
- Alpha edges, motion, and scaling do not flicker.
- The graphic matches the preset: trailer, highlight reel, or story cut.

## Common Mistakes

- Making decorative graphics that do not change the viewer's understanding.
- Adding more effects instead of refining hierarchy and timing.
- Treating captions as generic subtitles when the preset calls for designed captions.
- Designing overlays without checking them against the actual video frame.
