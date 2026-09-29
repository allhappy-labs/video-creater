# Source Inspector Inner Title Removal

## Context

Palmier's right rail names the active inspector at the rail level, then moves directly into
`Details` / `AI Edit` content and source metadata. Video Creater already has right-rail
`Source` / `Timeline` tabs, but the source body repeats a visible `Source Inspector`
title inside the card. That duplicate title consumes vertical space and makes the rail
feel heavier than the Palmier reference.

## Goal

Remove the visible inner `Source Inspector` heading from source-mode inspector cards
while preserving the accessible source inspector region and all source actions.

## Requirements

- The source-mode inspector region remains accessible as `Source Inspector`.
- The timeline variant keeps its visible `Timeline` heading because it can appear outside
  the right rail as a standalone source clip editor.
- Generated and imported source inspectors start with the selected source identity or
  Details / AI Edit tabs, not a duplicated `Source Inspector` title.
- Existing generated source actions, imported source actions, Details / AI Edit tabs,
  rerun/replace/insert actions, and provenance readouts are unchanged.
- The right rail continues switching between Source and Timeline inspectors.

## Verification

- Component tests assert `Source Inspector` remains the region name but is not rendered
  as a visible source-mode heading.
- EditorWorkspace tests assert the right rail no longer exposes a visible `Source
  Inspector` heading while generated source content remains visible.
- Existing source inspector action tests continue to pass.
- Browser QA confirms the right rail starts with source content under the rail `Source`
  tab and no duplicated inner title.
