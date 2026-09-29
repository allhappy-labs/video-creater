# Palmier Project Title Chrome Design

## Context

Palmier's editor header shows the project name and edit state as one compact centered line, such
as `Aesthetic video - Edited`. Video Creater currently stacks the project name above a separate
`Edited` line, which makes the top chrome taller and less like the reference editor.

Palmier reference: https://www.palmier.io/docs

## Goal

Render the workspace project identity as a single compact title line while preserving navigation,
help, export, and profile controls.

## Behavior

- The project chrome exposes one heading with `{project.name} - Edited`.
- The visible `Edited` state no longer renders as a separate line.
- The title remains centered on desktop and left-aligned when the header stacks on narrow screens.
- Long project names truncate within the center title area instead of pushing navigation or export
  controls out of the header.
- Home, Codex chat, Help, Export, and profile controls remain unchanged.

## Non-Goals

- No editable project-name field in this slice.
- No changes to export, MCP setup, model settings, or project persistence.
- No changes to project dirty-state tracking beyond the existing visible `Edited` copy.

## Verification

- Update `EditorWorkspace` tests to assert the single combined title heading and absence of a
  separate `Edited` line.
- Keep existing header navigation, help menu, export, and profile assertions passing.
- Browser QA the desktop editor header to confirm the title fits as one compact line.
