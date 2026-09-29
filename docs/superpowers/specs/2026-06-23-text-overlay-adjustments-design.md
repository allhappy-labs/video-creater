# Text Overlay Adjustments Design

## Context

Video Creater can now insert and copy-edit manual text overlays. Palmier's editor treats timeline items as native editable objects: users select a clip, adjust it in context, and agents can make the same kind of validated changes through project actions. The remaining gap for manual text overlays is that their timing and visual production notes are created once but cannot be edited from the focused inspector.

## Goal

Make manual text overlays first-class timeline objects with editable timing and visual metadata. The UI and project action should let a human or agent update the overlay's start, duration, copy, visual treatment, motion, safe zone, and avoid rules in one validated operation.

## Behavior

- Selecting a text overlay shows timing inputs for start and duration.
- The inspector exposes editable fields for:
  - `Overlay text`
  - `Visual treatment`
  - `Motion`
  - `Safe zone`
  - `Avoid`
- Applying the inspector dispatches `updateTextOverlayItems` with a single update object.
- The Rust action trims all strings, rejects empty overlay text and empty visual metadata, rejects negative start or non-positive duration, and rejects non-overlay or non-text targets.
- On success the action updates `startSeconds`, `durationSeconds`, `source.text`, the visible timeline `label`, and the matching text/visual fields in `properties`.
- The action marks `properties.textEdited = true` and sorts the affected track after timing changes.

## Non-Goals

- No typography picker or rendered style preview in this slice.
- No Temporal workflow. This is a synchronous manual/agent project edit.
- No change to generated media queues, render jobs, or template overrides.

## Acceptance Criteria

- Frontend contract accepts `updateTextOverlayItems`.
- Rust wire contract serializes to `type: "updateTextOverlayItems"` with camelCase fields.
- Rust mutation test proves timing, label, source text, and visual metadata update together.
- Rust rejection tests prove invalid text, invalid visual metadata, and wrong targets do not mutate the project.
- Workspace test proves the inspector dispatches the new action and the visible timeline label/timing update.
