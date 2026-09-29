# Palmier Selected Item Rail Inspector

## Context

Palmier keeps clip/source properties in a compact right-side inspector while the timeline stays dedicated to track editing. Video Creater already moved source clip details toward that model, but selected caption, template, and manual text overlay editors still render as boxed "Inspector" cards below the timeline.

That splits the selected-item workflow across two places:

- source clips are edited in the right rail;
- captions/templates/text overlays are edited in a separate main-column panel;
- the main timeline column gains extra vertical chrome whenever a non-source timeline item is selected.

## Goal

Move selected caption, template, and manual text overlay editing into the existing inspector rail so all selected timeline item properties share the same right-side locus.

## UX Requirements

- Selecting a caption shows its editor in the inspector rail.
- Selecting a template overlay shows its editor in the inspector rail.
- Selecting a manual text overlay shows its editor in the inspector rail.
- The timeline column must not render the selected-item editor below the timeline.
- The visible section titles should be terse object names such as `Caption`, `Template`, and `Text overlay`, not `Caption Inspector`, `Template Inspector`, or `Text Overlay Inspector`.
- Existing edit behaviors, validation, and apply callbacks must remain unchanged.
- Source clip inspectors and project/timeline inspector behavior should remain unchanged except for coexisting with the selected-item editor in the same rail where appropriate.

## Non-Goals

- Redesigning the full inspector rail layout.
- Changing timeline action schemas or Rust project mutations.
- Changing source clip trim/generation behavior.

## Acceptance Criteria

- Tests verify selected non-source item editors render inside `Inspector rail`.
- Tests verify the main `Selected item editor` region is no longer present for selected non-source timeline items.
- Tests verify the old `* Inspector` headings are not exposed for caption/template/text overlay editors.
- Existing editor workspace, caption inspector, template inspector, and text overlay edit tests pass.
