# Palmier Caption Inspector Shell Flattening Design

## Context

Palmier keeps selected clip editing close to the timeline: caption text, generated media actions, source controls, and timeline details appear as direct right-rail editing surfaces instead of nested dashboard cards. Video Creater already moved selected item editors into the inspector rail, but `CaptionInspector` still renders its own shadcn `Card` with `CardHeader`, `CardTitle`, and `CardContent`. Because `EditorWorkspace` already wraps it in `role="region" aria-label="Caption"`, the extra card shell makes a selected caption feel like a framed widget rather than a native timeline inspector.

## Goal

Flatten `CaptionInspector` so a selected caption exposes its editing controls directly inside the existing right-rail `Caption` region.

## Scope

- Replace the internal `Card` shell with a compact semantic section/div structure.
- Remove `Card`, `CardHeader`, `CardTitle`, and `CardContent` imports from `CaptionInspector`.
- Keep the visible heading `Caption`.
- Preserve the current start/end badges, caption textarea, generated/user-edited status, reading-density warning, empty state, and `Apply text correction` behavior.
- Keep the parent `EditorWorkspace` region name `Caption` unchanged.

## UI Requirements

- The caption inspector root must be a flat compact rail body with `space-y-3 text-sm`.
- The caption inspector root must not carry card shell classes such as `rounded-md`, `border`, `bg-card`, or `shadow-sm`.
- Existing child timing badges may remain small bordered chips because they describe concrete cue metadata.
- No new tabs, toggles, modes, or duplicated headings are introduced.

## Tests

- Add a focused `CaptionInspector` test that renders a caption and asserts:
  - the `Caption` heading remains present;
  - the root returned by the component is flat and compact;
  - the root does not have `rounded-md`, `border`, `bg-card`, or `shadow-sm`;
  - the `Caption text` field and `Apply text correction` action remain available.
- Run `caption-inspector.test.tsx`.
- Run the relevant `EditorWorkspace` caption selection tests.

## Non-Goals

- No caption timing editing in this slice.
- No text overlay or template inspector shell changes.
- No project action or Temporal workflow changes.
