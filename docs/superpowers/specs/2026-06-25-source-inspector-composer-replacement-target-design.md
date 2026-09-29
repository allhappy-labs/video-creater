# Source Inspector Composer Replacement Target Design

## Problem

Palmier documents timeline-native AI edits where generated clips can be rerun or tweaked and swapped on the timeline without leaving the project. Video Creater already records replacement-targeted generated assets with `replace:<timelineItemId>`, but the Source Inspector `Use in composer` handoff still opens selected generated timeline clips as ordinary Timeline generations. That loses the selected clip as the intended replacement target.

## Goals

- Preserve the selected generated timeline clip as the composer replacement target.
- Queue new composer submissions with `placementIntent: "replace:<selectedTimelineItemId>"`.
- Show replacement placement in the media generation composer so the editor can verify the target before queuing.
- Keep generated library media handoffs in Library placement.

## Non-Goals

- No new project schema field.
- No provider, Temporal workflow, or fal activity changes.
- No automatic replacement when the generation completes; existing completed-output replacement controls remain the application point.

## Design

`SourceClipInspector` derives the composer placement intent from the current view. For generated library sources it sends `"library"`. For selected generated timeline clips it sends `replace:<item.id>`, reusing the existing `GenerationPlacementIntent` union and replacement variation semantics.

`EditorWorkspace` keeps passing the intent through its one-shot composer seed. `MediaBin` already accepts external composer seeds, so it only needs to render and submit replacement placement as first-class UI state: the active recipe and footer should say `Replacement`, and the note should name the retained target label when available.

## Acceptance Criteria

- A generated timeline clip `Use in composer` callback sends `replace:<timelineItemId>`.
- MediaBin seeded with replacement placement shows a replacement target note.
- Submitting that composer request carries `placementIntent: "replace:<timelineItemId>"`.
- Generated library source handoffs still send `"library"`.
