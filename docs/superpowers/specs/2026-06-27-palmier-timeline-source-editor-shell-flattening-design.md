# Palmier Timeline Source Editor Shell Flattening Design

## Context

Palmier-style editing keeps source-backed timeline clips editable directly in the inspector rail: clip timing, source in/out, split, opacity, and audio edits should feel native to the selected clip rather than nested inside a card. Video Creater already separates read-only source details from the `Timeline source clip editor`, and the caption, overlay, template, media rail, and project inspector shells have been flattened. The timeline source editor still renders its timeline variant as a shadcn `Card`, which makes the main manual clip editor look heavier than adjacent right-rail editors.

## Goal

Flatten the `SourceClipInspector` timeline variant so manual source clip controls sit directly inside the `Timeline source clip editor` region.

## Scope

- Replace the timeline variant `Card`, `CardHeader`, `CardTitle`, and `CardContent` wrapper with a direct semantic `section`.
- Remove the shadcn card import from `SourceClipInspector` if no longer needed.
- Keep the accessible region name `Timeline source clip editor`.
- Keep the visible `Timeline` heading.
- Preserve all timeline clip edit controls and behavior: clip start, duration, source in/out, trim validation, split, opacity, audio fade/volume, and source metadata rows.
- Do not change the default `Source Inspector` variant, generated-source tabs, AI edit controls, or read-only details layout.

## UI Requirements

- The timeline source editor root must use compact rail classes: `space-y-3 text-sm`.
- The root must not carry card shell classes such as `rounded-md`, `border`, `bg-card`, or `shadow-sm`.
- Internal metadata and validation chips may remain bordered where they represent concrete editable/readout objects.
- No new tabs, toggles, modes, duplicated headings, or project actions are introduced.

## Tests

- Update the timeline-variant `SourceClipInspector` test so it asserts:
  - `Timeline source clip editor` exists;
  - the root is flat and compact;
  - the root does not have `rounded-md`, `border`, `bg-card`, or `shadow-sm`;
  - `Timeline`, metadata rows, source in/out, split, and trim controls remain available.
- Run `source-clip-inspector.test.tsx`.
- Run the existing `EditorWorkspace` tests for selected source clip edit placement and clip trim.

## Non-Goals

- No changes to generated source inspector tab behavior.
- No AI edit, fal.ai, Temporal, or project action contract changes.
- No preview panel or Codex rail shell changes.
