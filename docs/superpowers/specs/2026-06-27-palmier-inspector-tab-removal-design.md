# Palmier Inspector Tab Removal

## Context

Palmier's right inspector reads as a contextual project/source panel. The supplied screenshots show
timeline/project metadata directly in the rail, without a persistent `Source` / `Timeline` switch.
Video Creater still renders an `Inspector rail views` tablist when a source-backed clip is selected.
That creates another mode switch in an area that should simply explain and edit the selected context.

Palmier references: supplied editor screenshots and https://www.palmier.io/docs

## Goal

Remove the right inspector `Source` / `Timeline` mode switch while preserving source inspection,
timeline source-clip edits, and project workflow details.

## Behavior

- The right inspector no longer renders the `Inspector rail views` tablist.
- The persistent `Source` and `Timeline` inspector tab buttons are removed.
- When a selected timeline clip has source context, the inspector shows source details first and the
  timeline source-clip editor/project inspector below it.
- Source detail cards remain read-only where they were read-only before.
- Timeline source-clip edits remain available without requiring a tab click.
- When a selected library media item has no selected timeline item, the inspector continues to show
  source media details only.
- Caption, text overlay, template, project timeline, Temporal workflow, render report, and selected item
  editor behavior keep their existing contracts.
- No project schema, timeline action, render, queue, Temporal, or media generation provider contract
  changes.

## Verification

- `EditorWorkspace` tests assert that the right rail no longer contains `Inspector rail views`, `Source`,
  or `Timeline` inspector tabs.
- Existing source inspector tests keep proving source details render in the right rail.
- Existing source-clip editing tests are updated to assert the timeline editor is visible without a tab
  click.
- Browser QA confirms the right rail moves directly into contextual inspector content with no tab strip.
