# Timeline Generation Placeholder Clips

## Context

Palmier treats generation as a timeline-native edit: generated media can be placed directly on
the timeline, and the user can keep editing around that future clip. Video Creater now shows the
landing target before queueing, but the timeline itself stays unchanged until mock completion adds
the finished media. This still makes timeline generation feel like a detached background job.

Palmier reference: https://www.palmier.io/docs

## Goal

When a media generation is queued with `placementIntent: "timeline"`, reserve the target timeline
span immediately with a generated placeholder clip. When the generated output completes, replace the
placeholder with the completed media at the same track, start time, and duration.

## Requirements

- Queueing from the media generation drawer creates the same Temporal job and generated asset as
  today.
- If the request targets the timeline and an unlocked compatible track exists:
  - add a placeholder timeline item in the same action batch as the job and generated asset;
  - use the first unlocked `video` track for `video`, `image`, and `generated` requests;
  - use the first unlocked `audio` track for `audio` requests;
  - place the item at the current append target and use the requested generated duration;
  - mark the item with `generatedAssetId`, `generatedTimelinePlaceholder: true`, `sourceIn`, and
    `sourceOut`.
- The placeholder uses a generated source, a readable label, generated styling, and the existing
  workflow status badge.
- Completing a timeline-targeted generated asset removes its placeholder and inserts the completed
  output at the placeholder's track and start time.
- If no placeholder exists at completion time, completion falls back to the current append behavior.
- If no unlocked compatible track exists at queue time, the generation still queues without a
  placeholder.
- Queue payloads, Temporal start requests, fal.ai model settings, generated asset metadata, and
  replacement-targeted generations are unchanged.

## Non-Goals

- No selectable track picker.
- No playhead insertion, ripple, overwrite, or range replacement.
- No new project schema field.
- No real provider polling or download changes.
- No placeholder support for replacement-targeted generations.

## Test Plan

- Add an `EditorWorkspace` test proving timeline drawer queueing batches `recordJob`,
  `recordGeneratedAsset`, and placeholder `addItems`.
- Add an `EditorWorkspace` test proving mock completion removes the placeholder and adds the
  completed output at the placeholder start time.
- Run focused workspace tests, full frontend tests, lint, build, browser QA, whitespace checks,
  placeholder scan, and secret scan.
