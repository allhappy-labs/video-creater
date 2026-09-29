# Palmier Timeline Workflow Badge Tooltip Design

## Context

Palmier-style editor surfaces keep the timeline canvas focused on editing facts. Video Creater already
shows compact workflow status badges on generated timeline clips, which is useful while AI media is
running or blocked. The remaining mismatch is the badge tooltip: it exposes Temporal workflow type and
task queue names directly on the clip.

## Goal

Keep generated clip workflow status visible on the timeline while making the tooltip human-facing and
free of infrastructure route details.

## Requirements

- Generated timeline clips still show the compact workflow status badge.
- The badge accessible label remains `Workflow <status> for <clip label>`.
- The badge tooltip shows status and updated time in plain product copy.
- The badge tooltip does not show workflow type names, task queues, run IDs, or activity names.
- Workflow route metadata remains unchanged in project jobs, Codex activity, project JSON, and
  Temporal start/runtime paths.
- Timeline selection, double-click source opening, drag, trim, resize, and AI provenance badges remain
  unchanged.

## Verification

- Update `TimelineEditor` coverage to assert the badge keeps status text while the tooltip omits
  `VideoCreaterGenerateMediaWorkflow` and `video-creater-workflows`.
- Update `EditorWorkspace` generated workflow badge coverage to assert the same compact tooltip copy.
- Run focused timeline/editor workspace tests, lint, full tests, and browser QA.
