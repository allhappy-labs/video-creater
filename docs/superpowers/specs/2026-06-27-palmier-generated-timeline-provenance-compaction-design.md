# Palmier Generated Timeline Provenance Compaction

## Problem

Generated timeline clips currently render detailed provenance inside the clip body: generated asset id, model id, and prompt text. This makes the timeline visually heavier than Palmier's reference editor, where timeline clips remain compact editing objects and detailed source metadata lives in the inspector.

The timeline shows enough to identify and edit the generated clip without turning each clip into a metadata card.

## Requirements

- Generated timeline clips keep the visible `AI` badge, workflow status badge, clip label, and source range.
- Generated timeline clips do not render provenance ids, model ids, or prompt text inside the timeline canvas.
- The detailed generation provenance remains available in the Source Inspector generated details and generated references.
- The clip `title` tooltip may continue to expose concise source-range metadata for inspectability.
- No new toggles, tabs, drawers, or alternate timeline modes are introduced.

## Non-Goals

- Do not remove generated provenance from project data, sidecar records, source references, or inspector panels.
- Do not change generated workflow status calculation or timeline item identity.
- Do not redesign timeline track colors, ruler behavior, drag behavior, or source mark controls.

## UI Design

Generated clips render as compact timeline clips:

- first line: `AI`, workflow status when present, and the clip label
- second line: source/timeline range metadata already shown for editable clips
- no third block for generated asset id, model name, or prompt

This matches the Palmier-style principle that the timeline is for arrangement and trimming, while provenance review belongs in the right rail.

## Testing

- Add a focused `TimelineEditor` test proving generated provenance details are not present inside `Timeline canvas`.
- Keep assertions that the generated clip still shows `AI`, workflow status, label, and range.
- Existing inspector/source tests continue to cover detailed generated metadata outside the timeline.
