# Project Format Summary Design

## Context

Palmier's right rail keeps project format metadata visible beside the timeline: resolution, frame
rate, aspect ratio, and duration are separate scannable rows. Video Creater already exposes project
files, duration, and a combined render-settings row, but the right rail does not show the aspect
ratio or a dedicated format block. That makes the project state less inspectable for agents and
manual editors reviewing a text-file-backed project.

Palmier reference: https://www.palmier.io/docs

## Goal

Make the project timeline inspector show project format metadata as first-class rows.

## Behavior

- The project timeline inspector shows a `Format` section near the existing timeline summary.
- The section includes separate rows for:
  - `Resolution`, formatted as `<width> x <height>`;
  - `Frame rate`, formatted as `<fps> fps`;
  - `Aspect ratio`, reduced from render width and height, for example `16:9`;
  - `Captions`, using the project's render caption setting.
- The existing duration card, project file rows, AI media section, workflow queue, and export
  artifact summaries remain unchanged.
- Invalid or missing width/height values fall back to `unknown` for aspect ratio.

## Non-Goals

- No project schema change.
- No render settings editing UI.
- No automatic correction of project render settings.

## Verification

- Add a `ProjectTimelineInspector` regression test that verifies the new format rows, including
  reduced aspect ratio.
- Run the focused inspector test file.
- Run `pnpm lint`.
