# Project Timeline Inspector Design

## Context

Palmier keeps project and timeline metadata visible beside the editor: project name, media path, output settings, frame rate, aspect ratio, and duration are inspectable without opening a file browser. Video Creater already has text-file-editable project, media, generation, render, and timeline models, but the main editor does not expose a compact project-level summary. Users and agents can edit files, yet the app gives little immediate confirmation of which project mode, timeline shape, render settings, or latest render report is active.

## Goals

- Add a compact right-rail inspector that summarizes the currently loaded project and timeline.
- Make text-file workflow state visible: schema version, split-project directory, timeline file, media index file, and render report path expectations.
- Show timeline editing facts that help manual and agent workflows: duration, track count, clip/item count, locked track count, media count, generated asset count, and latest render report status.
- Keep the panel dense and operational, matching the current dark editor chrome and Palmier-style metadata inspector.

## Non-Goals

- No backend changes or new project actions.
- No editable metadata fields in this slice.
- No replacement of the existing source, caption, template, or Codex panels.
- No broad layout redesign.

## UI

Add `ProjectTimelineInspector` as a small card above the Codex panel in the right rail. The card should use compact rows and monospaced values where paths or numeric metadata benefit from alignment. It should expose:

- Project name and id.
- Project mode: `Split files` for schema v2 projects with a project directory, otherwise `Embedded project`.
- Project directory.
- Timeline file location: `timeline.json` for split projects, otherwise `project manifest`.
- Media index location: `media/index.json` for split projects, otherwise `project manifest`.
- Timeline duration and render output settings.
- Track, locked track, and item counts.
- Media asset and generated asset counts.
- Latest render report status and output path, or `No render report`.

## Data Flow

The component receives `project` and `projectDir` from `EditorWorkspace`. It derives summary values from the in-memory `VideoProject` only. The panel is read-only, so no callbacks are needed. The latest render report is the last entry in `project.renderReports`, preserving the current append-only report behavior.

## Testing

- Workspace integration tests verify split-project metadata, counts, render settings, and latest render status from the supplied project.
- Existing render, media, generation, and timeline tests should remain unchanged.

## Future Work

- Add quick actions to reveal `project.json`, `timeline.json`, `media/index.json`, and latest render logs once the app exposes native file reveal commands.
- Add validation issue counts when split-project validation is wired into the frontend.
- Let agents reference this same bounded summary in MCP/app-server context so the visible app and agent prompt share the same project facts.
