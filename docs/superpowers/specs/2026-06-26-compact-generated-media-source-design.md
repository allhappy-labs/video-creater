# Compact Generated Media Source Design

## Context

Palmier keeps the media panel as a browsing and source-selection surface. Video Creater still expanded a selected generated media source into multiple full-width action buttons, which made the left panel feel like a command panel instead of a compact source browser.

## Decision

Keep selected generated media sources compact by default. The collapsed summary shows identity, output path, preview, status, and a single details toggle. Actions such as replacement, timeline insertion, rerun, and composer reuse remain available only after the user opens generated source details.

## Requirements

- The collapsed selected generated source summary must not show `Replace selected clip`, `Insert on timeline`, `Rerun same prompt`, or `Use in composer`.
- `Show generated source details` remains the single collapsed summary action.
- Opening generated source details shows the available generated source actions before file, project, reference, workflow, and prompt details.
- Existing callbacks for replacement, insertion, rerun, and composer reuse keep their current payloads.
- Generated output cards in the AI generations list keep their existing direct output actions.

## Testing

- `MediaBin` proves the collapsed selected generated source summary excludes the generated action stack.
- Existing `MediaBin` action tests open generated source details before using replacement, insertion, rerun, or composer reuse.
- Browser QA should confirm the left media panel is shorter and the details toggle is the only collapsed generated source command.
