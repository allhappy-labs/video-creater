# Right Rail Source Inspector Design

## Context

Palmier keeps the selected asset or timeline context in the right inspector. The attached screenshots show the right rail switching from `Timeline` project details to `Source` details with `Details` and `AI Edit` tabs when a generated or imported clip is selected. Video Creater already has a capable `SourceClipInspector`, including trim, split, reorder, generated provenance, references, and AI edit queue controls. The gap is placement: source editing appears below the timeline, which makes the main editing area taller and separates selection context from the inspector rail.

Palmier docs also emphasize that clips can be edited, regenerated, inspected, and swapped without leaving the timeline. Moving source inspection into the right rail makes that workflow more direct while preserving the existing timeline-native behavior.

## Goals

- Show `SourceClipInspector` at the top of the right rail whenever a source clip is selected.
- Keep `ProjectTimelineInspector` at the top of the right rail when no source clip is selected.
- Remove the duplicate below-timeline `SourceClipInspector` for source selections.
- Preserve the existing below-timeline `CaptionInspector` and `TemplateInspector` behavior for caption and template selections.
- Preserve all source trim, split, reorder, reveal source, queue variation, and referenced generation behavior.

## Non-Goals

- No redesign of the `SourceClipInspector` internals in this slice.
- No new queue, scheduling, or workflow module.
- No Temporal integration in this UI-only change. Future queue/workflow work must use Temporal Rust per project direction.
- No change to canonical project files, action schema, or generated asset schema.

## UI Behavior

The workspace uses a selection-aware inspector rail:

- Source clip selected: right rail shows `Source Inspector` first, then `Codex`.
- Caption selected: center lower editor shows `CaptionInspector`; right rail shows `ProjectTimelineInspector`, then `Codex`.
- Template selected: center lower editor shows `TemplateInspector`; right rail shows `ProjectTimelineInspector`, then `Codex`.
- No selected source clip: right rail keeps the project timeline inspector.

The center column remains focused on preview, timeline, and the selected caption/template editor. Source clip controls move out of the center column to recover timeline space and match the Palmier layout model.

## Testing

- Add an editor workspace test that selects a source clip and verifies `Source Inspector` appears before the `Codex` panel in the right rail.
- Verify that the same source selection does not leave a duplicate source inspector in the center column.
- Verify that a caption selection still shows the `Project timeline inspector` in the right rail.
- Keep existing source inspector behavior tests passing.

## Acceptance Criteria

- Source clip selections put source details and AI edit controls in the right rail.
- The center column no longer renders source clip controls below the timeline.
- Captions and templates keep their existing editing surfaces.
- Existing project action, Codex, generation, source inspector, and timeline tests keep passing.
- Browser QA confirms the right rail source inspector fits without overlapping the preview, timeline, or Codex panel.
