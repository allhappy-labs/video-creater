# Default Sample Generated Timeline Clip Design

## Problem

Palmier puts generated media directly on the timeline: editors can click an AI clip, inspect its
prompt and references, rerun it, tweak it, or swap it without leaving the project. Video Creater's
default React sample project already has a completed generated media asset and Temporal job record,
but the visible timeline still starts with only imported footage. This hides the generated-clip
workflow in the first-run editor and weakens visual QA for the Palmier-style timeline path.

Palmier reference: https://www.palmier.io/docs

## Goals

- Put the existing completed sample generated output on the default workspace timeline.
- Preserve the existing generated asset, output media, folder, and workflow job records.
- Make the generated timeline clip selectable so the inline dock and Codex selected-timeline
  context expose prompt, model, settings, references, rerun, tweak, and replacement actions.
- Keep the generic shared `sampleTimeline` fixture unchanged for lower-level timeline tests.

## Non-Goals

- No schema migration or new project action.
- No fal.ai or Temporal worker behavior changes.
- No change to imported-media timeline editing behavior.

## Design

`createSampleProject()` will derive a richer default timeline from `sampleTimeline`: it keeps the
imported opening clip at the start, then appends a completed generated video clip that points at
`sample-generated-output` and records `generatedAssetId: "sample-generated-shot"` in item
properties. The sample timeline duration grows to cover both clips.

The existing generated asset already stores the prompt, model, first-frame reference, output media,
and completed workflow job. Existing timeline and Codex context helpers can therefore identify the
new item as generated without new data flow.

## Acceptance Criteria

- The default workspace timeline renders a selectable generated clip.
- Selecting that clip shows generated timeline context in the Codex rail, including asset id,
  model/settings, prompt, and reference thumbnail.
- The inline selected-clip action dock exposes rerun, rerun-and-replace, and tweak prompt actions.
- Existing generated-source selection and media-bin behavior continue to work.
