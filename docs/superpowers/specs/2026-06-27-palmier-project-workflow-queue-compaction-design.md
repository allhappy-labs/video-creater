# Palmier Project Workflow Queue Compaction Design

## Context

The project timeline inspector still renders Temporal internals in the right rail: workflow IDs,
task queues, workflow type names, run IDs, activity type lists, start-request workflow IDs, reuse
policies, and provider credential labels. Palmier's right rail keeps project and source inspection
focused on editing facts while agent/tool execution remains visible in the assistant activity stream
and project files.

Palmier reference: https://www.palmier.io/docs

## Goal

Keep workflow state and start actions visible in the project inspector without exposing
infrastructure diagnostics as first-class right-rail UI.

## Behavior

- `Workflow queue` remains in the project timeline inspector as a compact job summary and start
  surface.
- Each workflow job shows kind, updated time, and status.
- Queued or blocked jobs with a valid persisted start request still show `Start workflow`, disabled
  setup states, and pending `Starting...` states.
- Start-request business context may remain visible when useful: profile, format, output path, and
  validation summary.
- The right rail no longer renders workflow ID, task queue, workflow type, run ID, activity type
  chips, ID reuse policy, start-step counts, or provider credential environment variable labels.
- `AI media` cards no longer render task queue, activity count, start-request, or credential chips.
  They may keep a compact workflow status pill so generated assets still show queue state.
- Full Temporal metadata remains in project JSON, Codex workflow activity, logs, and worker
  preflight code paths. No Temporal start, schema, or fal.ai payload behavior changes in this
  slice.

## UI Contract

- The project inspector should read like Palmier's compact project/source rail, not a Temporal
  debug panel.
- Actionability is preserved: if a workflow can be started from the rail today, it can still be
  started after this change.
- Sensitive or operator-facing labels such as credential environment variable names should not be
  displayed in the editor right rail.

## Verification

- Update `ProjectTimelineInspector` tests to prove workflow jobs remain visible and startable while
  workflow IDs, queues, workflow types, run IDs, activity chips, reuse policies, start-step counts,
  and credentials are absent from the right rail.
- Update workspace tests that cover right-rail workflow starts to assert the compact UI contract.
- Browser-smoke the timeline inspector at desktop and narrow widths to confirm the workflow queue
  is readable and no infrastructure labels appear.
