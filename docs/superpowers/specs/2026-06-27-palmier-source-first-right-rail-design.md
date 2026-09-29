# Palmier Source-First Right Rail Design

## Context

Palmier keeps the right rail focused on the selected source: source facts, references, generated settings, prompt, and AI edit controls are the first things an editor sees. Video Creater already removed right-rail mode tabs and keeps timeline edit controls in the same rail, but selected source clips still render the `Timeline source clip editor` above `Source Inspector`. That makes trim/split fields dominate the rail before the selected media context and AI actions.

Palmier reference: https://www.palmier.io/docs

## Goal

Render the selected source inspector before the timeline clip edit form in the right rail, while preserving the existing manual timeline edit controls below it.

## Requirements

- For a selected source clip, `Source Inspector` appears before `Timeline source clip editor` in the inspector rail DOM and visual order.
- The Source Inspector remains read-only for source facts; trim, split, opacity, audio, and sequence controls remain in `Timeline source clip editor`.
- Do not reintroduce right-rail Source/Timeline tabs, context buttons, toggles, or duplicated headings.
- Do not remove any manual editing capability.
- Generated source AI edit controls remain directly below generated source details.

## Tests

- Update `EditorWorkspace` right-rail coverage to prove `Source Inspector` precedes `Timeline source clip editor`.
- Existing trim/split/reorder tests continue to find and use the timeline editor controls.
- Browser QA verifies the default selected generated clip shows source/provenance details above timeline edit controls.
