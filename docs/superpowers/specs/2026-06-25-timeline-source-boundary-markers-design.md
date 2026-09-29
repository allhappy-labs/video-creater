# Timeline Source Boundary Markers

## Context

Palmier treats source ranges as timeline-native editing state: clips can be opened in the source viewer, trimmed, split, reordered, and adjusted by a human or connected agent. Video Creater already stores `sourceIn` and `sourceOut` in text-editable project data and shows that range as compact clip metadata, but the clip body does not visually mark the selected source boundaries.

## Goal

Render compact source-in and source-out boundary ticks on media-backed timeline clips so the source range is visible directly in the timeline.

## Requirements

- Video, generated video, and audio clips with numeric `sourceIn` and `sourceOut` render non-interactive source boundary ticks.
- Each tick has an accessible label naming the clip, mark type, and source time.
- Markers do not appear for timeline items without a complete source range.
- Markers do not appear for text-only timeline items, even if they contain source-like properties.
- Existing clip selection, drag, resize, waveform, filmstrip, provenance, and inline action dock behavior remains unchanged.

## Non-Goals

- Draggable source boundary handles.
- Changing source range math or project schema.
- Adding new mark-in or mark-out callbacks.
- Timeline retiming or speed controls.

## Test Plan

- Add a TimelineEditor component test for source-in and source-out tick labels on a source-backed video clip.
- Add a TimelineEditor component test proving text-only clips do not render source boundary ticks.
- Run focused timeline editor tests, full frontend tests, lint, build, browser QA, whitespace checks, placeholder scan, and secret scan.
