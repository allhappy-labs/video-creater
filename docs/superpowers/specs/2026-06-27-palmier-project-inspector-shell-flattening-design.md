# Palmier Project Inspector Shell Flattening Design

## Context

Palmier presents project editing as a direct media/timeline workspace: media generation starts in the media panel, generated clips stay editable on the timeline, and the assistant can trim, split, reorder, regenerate, and place media without switching into a dashboard. Video Creater has already flattened the source rail, timeline shell, preview transport, and selected-source inspector. The remaining empty-selection `Project timeline inspector` still uses a shadcn `Card` wrapper with a `CardHeader`, `CardTitle`, and `CardContent`, which makes the right rail read as a dashboard panel instead of an editor inspector.

## Goal

Flatten the project-level right rail inspector shell while preserving all current project, AI media, export, and Temporal workflow content.

## Scope

- Replace the outer `Card` shell in `ProjectTimelineInspector` with a direct semantic `section` that keeps `role="region"` and `aria-label="Project timeline inspector"`.
- Replace `CardHeader`, `CardTitle`, and `CardContent` with compact div/header elements.
- Keep the visible title `Timeline`, project name, split/embedded status, project identity, format metrics, AI media, latest export, recent export artifacts, workflow queue, and Temporal worker setup behavior.
- Do not change `ProjectTimelineContext`; it remains a compact contextual card for cases where the source inspector needs project context.
- Do not remove any Temporal workflow start buttons or diagnostic rows in this slice.

## UI Requirements

- The inspector root must be a flat rail section with vertical spacing and compact text.
- The root must not carry shadcn card shell classes such as `rounded-md`, `border`, `bg-card`, or `shadow-sm`.
- Inner repeated assets and metric blocks may remain bordered cards because they represent individual repeated or framed items.
- The accessible region name and all existing user-facing controls remain stable.

## Tests

- Add a focused `ProjectTimelineInspector` test that renders the inspector and asserts:
  - the `Project timeline inspector` region exists;
  - the root is a flat compact rail section;
  - the root does not have `rounded-md`, `border`, `bg-card`, or `shadow-sm`;
  - the existing `Project identity` region remains available inside it.
- Run the focused inspector test file, then the relevant workspace tests that select the project inspector.

## Non-Goals

- No generated asset model/provider changes.
- No Temporal workflow migration changes.
- No project data model changes.
- No replacement of the existing project metrics, export list, or workflow queue content.
