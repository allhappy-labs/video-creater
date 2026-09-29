# Right Rail Context Tabs Design

## Context

Palmier's right inspector presents one focused context at a time: `Timeline` for project metadata
or `Source` for the active source/generation details. Video Creater currently keeps both contexts
available, but when a generated timeline source is selected it stacks the full Source Inspector and
Project Timeline Inspector in the same rail. That preserves access, but it makes the rail crowded
and less like a focused editor inspector.

## Goal

Add a compact right-rail tab switcher that lets editors toggle between source details and timeline
metadata when both contexts are available.

## Behavior

- When a selected source inspector exists, the right rail shows `Source` and `Timeline` tabs.
- The rail defaults to `Source` for a selected source-backed timeline clip or selected library
  source.
- The `Source` tab renders the existing Source Inspector unchanged.
- The `Timeline` tab renders the existing Project Timeline Inspector unchanged.
- When no source inspector is available, the rail renders the Timeline inspector directly without
  extra tabs.
- Changing timeline/source selection may return the rail to `Source`; this keeps the active
  selection visible by default.

## Non-Goals

- No source inspector field changes.
- No project action, schema, Temporal, fal.ai, or preview behavior changes.
- No removal of the compact timeline context banner.

## Verification

- Add an EditorWorkspace test proving the generated sample opens with `Source` selected, exposes a
  `Timeline` tab, and swaps the visible rail content without losing either inspector.
- Existing right-rail, source inspector, workflow queue, and generated source tests continue to pass.
- Browser-smoke the editor at desktop width and check the rail is not overcrowded.
