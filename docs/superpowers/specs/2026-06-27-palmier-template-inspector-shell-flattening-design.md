# Palmier Template Inspector Shell Flattening Design

## Context

Palmier-style editors keep selected timeline items editable in the inspector rail without nested dashboard cards. Video Creater has already flattened the caption and text overlay inspectors, but `TemplateInspector` still renders its own shadcn `Card` with `CardHeader`, `CardTitle`, and `CardContent`. The parent workspace already wraps selected template editing in `role="region" aria-label="Template"`, so the internal card shell makes motion-template edits feel like a settings widget rather than a native timeline item editor.

## Goal

Flatten `TemplateInspector` so template fields, timing, style, and visual guidance controls sit directly inside the existing right-rail `Template` region.

## Scope

- Replace the internal `Card` shell with a compact semantic section/div structure.
- Remove `Card`, `CardHeader`, `CardTitle`, and `CardContent` imports from `TemplateInspector`.
- Keep the visible heading `Template`.
- Preserve start and duration inputs, template text fields, style inputs, visual guidance fields, reset guidance action, empty state, and `Apply template` behavior.
- Keep the parent `EditorWorkspace` region name `Template` unchanged.

## UI Requirements

- The template inspector root must be a flat compact rail body with `space-y-3 text-sm`.
- The root must not carry card shell classes such as `rounded-md`, `border`, `bg-card`, or `shadow-sm`.
- The nested `Visual guidance` block may remain a bordered sub-panel because it groups a focused edit decision.
- No new tabs, toggles, modes, duplicated headings, project actions, or template data changes are introduced.

## Tests

- Add a focused `TemplateInspector` test that renders a template item and asserts:
  - the `Template` heading remains present;
  - the root returned by the component is flat and compact;
  - the root does not have `rounded-md`, `border`, `bg-card`, or `shadow-sm`;
  - `Headline`, `Visual treatment`, `Reset guidance`, and `Apply template` remain available.
- Run `template-inspector.test.tsx`.
- Run the existing `EditorWorkspace` test that previews and edits selected template fields.

## Non-Goals

- No source clip inspector shell changes.
- No preview panel shell changes.
- No template catalog, rendering, Temporal workflow, or project data model changes.
