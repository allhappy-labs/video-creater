# Codex Text Overlay Action Schema

## Problem

The Codex video edit output schema currently exposes `projectActions` as an array of generic objects with only a `type` enum. That lets agents see the available action names, but it does not tell them which fields are required for the text overlay actions added for manual overlay editing.

For Palmier-style agent-controlled editing, the app-server contract should be narrow enough that agents can return replayable project-file mutations without relying on prose alone.

## Scope

Tighten the structured output schema for:

- `editTextItem`
- `updateTextOverlayItems`

Keep other `ProjectAction` variants accepted through the existing generic action branch until each has its own dedicated schema.

## Contract

`editTextItem` requires:

- `type: "editTextItem"`
- `itemId`: non-empty timeline item id
- `text`: non-empty replacement text

`updateTextOverlayItems` requires:

- `type: "updateTextOverlayItems"`
- `updates`: at least one update

Each update requires:

- `itemId`: non-empty overlay timeline item id
- `startSeconds`: finite non-negative timeline start
- `durationSeconds`: positive duration
- `text`: non-empty overlay copy
- `visualTreatment`: non-empty visual treatment note
- `motion`: non-empty motion note
- `safeZone`: non-empty safe-zone note
- `avoid`: non-empty anti-pattern note

Detailed branches should reject unknown fields so malformed agent proposals fail schema validation before Rust action validation.

## Acceptance

- The Codex turn output schema uses a `oneOf` project action union.
- The generic action branch excludes `editTextItem` and `updateTextOverlayItems`.
- Dedicated branches for text overlay actions define required fields and reject additional properties.
- Existing app-server and project-action tests pass.
