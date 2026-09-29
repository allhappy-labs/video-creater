# Source Inspector Generated Output Replace Design

Palmier keeps generated media edits close to the timeline: an editor can inspect a generated result, then swap it into the selected clip without hunting through another panel. Video Creater already has validated generated-output replacement in the project action layer, Media Bin, and Codex selected-source panel. The missing surface is the right-rail Source Inspector for a selected generated library output.

## Scope

- When a timeline source clip has been selected, keep it as the replacement target after the user opens a generated library output.
- In the Source Inspector for that generated library output, show a compact replacement action when a replacement target label and replacement callback are available.
- Clicking the action calls the existing `replaceTimelineItemWithGeneratedOutput` workspace path with the selected generated output media id.
- Keep the generated clip inspector focused on details, AI edit, rerun, variation, and upscale; do not introduce a new backend action.

## Out of Scope

- Automatic replacement when a queued variation completes.
- Replacing with imported media.
- Changing canonical project files directly from the inspector.

## Acceptance

- Source Inspector unit coverage proves the generated library output replacement action calls its callback with the output media id.
- EditorWorkspace coverage proves selecting a timeline clip, opening a generated output, and replacing from the inspector applies `replaceTimelineItemWithGeneratedOutput`.
- Existing Media Bin and Codex replacement behavior remains unchanged.
