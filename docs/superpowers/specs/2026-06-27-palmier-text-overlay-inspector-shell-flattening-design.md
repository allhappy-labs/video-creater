# Palmier Text Overlay Inspector Shell Flattening Design

## Context

Palmier-style timeline editing treats overlays as native timeline items: selecting one should expose copy, timing, visual treatment, motion, safe-zone, and avoid controls directly in the inspector rail. Video Creater already moved selected item editors into the right rail and flattened the caption inspector, but `TextOverlayInspector` still renders an internal shadcn `Card` with `CardHeader`, `CardTitle`, and `CardContent`. The parent workspace already wraps it in `role="region" aria-label="Text overlay"`, so the inner card makes the overlay editor feel like a nested settings panel rather than a direct timeline item editor.

## Goal

Flatten `TextOverlayInspector` so manual overlay editing controls sit directly inside the existing right-rail `Text overlay` region.

## Scope

- Replace the internal `Card` shell with a compact semantic section/div structure.
- Remove `Card`, `CardHeader`, `CardTitle`, and `CardContent` imports from `TextOverlayInspector`.
- Keep the visible heading `Text overlay`.
- Preserve the start/end badges, clip start and duration inputs, overlay text field, visual guidance fields, edited/manual status, empty state, and `Apply overlay details` behavior.
- Keep the parent `EditorWorkspace` region name `Text overlay` unchanged.

## UI Requirements

- The text overlay inspector root must be a flat compact rail body with `space-y-3 text-sm`.
- The root must not carry card shell classes such as `rounded-md`, `border`, `bg-card`, or `shadow-sm`.
- Existing child timing badges may remain bordered metadata chips.
- No new tabs, toggles, modes, duplicated headings, or project action changes are introduced.

## Tests

- Add a focused `TextOverlayInspector` test that renders an overlay and asserts:
  - the `Text overlay` heading remains present;
  - the root returned by the component is flat and compact;
  - the root does not have `rounded-md`, `border`, `bg-card`, or `shadow-sm`;
  - `Overlay text`, `Visual treatment`, and `Apply overlay details` remain available.
- Run `text-overlay-inspector.test.tsx`.
- Run the existing `EditorWorkspace` tests for manual text overlay editing and metadata updates.

## Non-Goals

- No template inspector shell changes.
- No source clip inspector shell changes.
- No overlay rendering, Temporal workflow, or project data model changes.
