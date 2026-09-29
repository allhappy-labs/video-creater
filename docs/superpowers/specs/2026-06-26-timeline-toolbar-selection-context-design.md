# Timeline Toolbar Selection Context Design

## Context

Palmier keeps timeline editing grounded in visible context: the user can see the selected clip and
then act with split, trim, delete, text, or source controls nearby. Video Creater has the manual
editing actions, but the toolbar does not summarize the selected target, so icon-only actions can
feel detached from the clip they will affect.

Palmier reference: https://www.palmier.io/docs

## Goal

Add a compact selected-item context strip inside the timeline toolbar so manual edits are visibly
anchored to the current clip.

## Behavior

- When a timeline item is selected, the toolbar shows a `Timeline selection context` group.
- The group shows the selected item label, track name, item kind, and timeline range.
- Locked tracks are called out with a compact `locked` badge so disabled edit actions have visible
  context.
- If no selected item is available, the toolbar shows no selection context group.
- Existing toolbar buttons, shortcuts, callback payloads, zoom controls, and timeline mutations
  remain unchanged.

## Non-Goals

- No new timeline patch type, persistence schema, or project file change.
- No multi-selection behavior.
- No Temporal, fal.ai, render, or workflow changes.

## Verification

- Add a `TimelineEditor` test proving the selected-item context group renders with label, track,
  kind, range, and locked state.
- Add/keep coverage proving the context group is absent without a valid selection.
- Existing timeline toolbar action tests continue to pass.
- Browser-smoke the editor and confirm the context strip fits in the toolbar without overlapping
  zoom controls.
