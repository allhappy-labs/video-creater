# Palmier Timeline Editor Readout Deduplication

## Context

After removing the right inspector `Source` / `Timeline` mode switch, a selected
source-backed timeline clip now stacks source details above timeline edit controls.
That keeps both contexts visible, but the timeline editor still repeats the same
clip title, generated-source label, source range, reason, and generated recipe
summary that the source inspector already shows immediately above it.

Palmier's inspector reads as a contextual editor, not a repeated factsheet. Once
source facts are visible, the timeline section should focus on the timeline edit
controls.

## Goal

When the timeline source-clip editor is stacked under a source inspector, remove
the repeated source readout and keep only timeline editing controls.

## Behavior

- The right rail still renders `Source Inspector`, then `Timeline source clip editor`,
  then `Project timeline inspector` for a selected source-backed timeline clip.
- The stacked `Timeline source clip editor` keeps its `Timeline` heading.
- The stacked timeline editor shows trim, opacity, split, and sequence controls as
  applicable to the selected item.
- The stacked timeline editor does not repeat the selected source title, generated
  source label, source range/reason pills, generated recipe summary, generated quick
  actions, or generated details.
- Standalone `SourceClipInspector` timeline usage keeps its source readout so tests
  and any future standalone placement remain understandable.
- Library media source inspection, generated-source AI edit actions, Temporal queue
  actions, project actions, render behavior, and project schema do not change.

## Verification

- Component coverage proves the standalone timeline variant still renders as an
  inspector card with its source readout.
- Workspace coverage proves the stacked right-rail timeline editor contains timeline
  edit controls and omits duplicated source/generation readout.
- Existing source inspector behavior tests continue passing.
- Browser QA captures the right rail with source details above a controls-focused
  timeline editor and no `Inspector rail views` tab strip.
