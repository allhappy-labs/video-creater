# Palmier Product Shell Polish Design

## Context

Palmier feels productized because project entry, recent projects, panel layout, onboarding, agent connection, model/account status, export feedback, and empty states are part of one coherent native editor shell.

Video Creater has a dense editor workspace with media, preview, timeline, inspector, and Codex rail, but first-run guidance, project lifecycle, narrow-width behavior, recovery copy, and shortcut discoverability still feel closer to an engineering console than a polished editor.

## Goal

Polish the Video Creater shell so a user can open the app, understand project state, connect an agent, import or generate media, edit on the timeline, and render/export without relying on hidden knowledge or technical error strings.

## Requirements

- Add a project home/recent projects entry surface for local projects.
- Preserve editor-first workflow once a project is open.
- Add a compact onboarding/tour path for media import, generation, timeline editing, agent rail, and export.
- Add a keyboard shortcut/help panel reachable from top chrome.
- Add responsive panel behavior for narrow widths: media, Codex, inspector, and timeline controls remain one action away without overlapping.
- Replace terse technical error messages in primary UI flows with action-oriented recovery copy.
- Keep visible status for transcription runtime, generation queue, render/export jobs, and project save state.
- Do not add marketing-page UI or decorative landing sections.

## Non-Goals

- No cloud account system in this slice.
- No billing or credit purchase flow; existing mock credit display can become provider/status copy later.
- No visual redesign of the whole editor.
- No mobile-first editing experience; narrow-width support means usable desktop reflow, not phone-grade editing.

## Architecture

Add shell-level state and UI in focused components:

- `project-home.tsx`: recent projects, open folder, new split project, missing project warnings.
- `workspace-help-menu.tsx`: MCP setup, shortcuts, model settings, diagnostics.
- `editor-tour.tsx`: local, dismissible, step-based tour with anchors for media, preview, timeline, agent, and export.
- `responsive-rail-state.ts`: deterministic rail collapse decisions and persisted user preferences.
- `recovery-copy.ts`: maps common error categories to short user-facing recovery text.

`EditorWorkspace` should orchestrate these pieces but should not absorb more large inline rendering blocks. New components receive typed props and callbacks.

## User Flows

### First Open

1. User sees project home with recent projects and `Open project folder`.
2. If no recent project exists, user sees direct actions: `New project`, `Open folder`, `Open sample`.
3. Opening a project enters the editor workspace immediately.

### Agent Setup

1. User opens Help or profile menu.
2. `Connect agent` shows Codex, Claude Code, Claude Desktop, and Cursor setup.
3. Copy buttons use the current project path and never require reading docs.

### Editor Tour

1. User can start tour from Help.
2. Tour highlights media import/generation, source preview, timeline edit, Codex rail, and render/export.
3. User can skip permanently; state is local.

### Failure Recovery

Primary UI failures show:

- what failed;
- why it likely failed;
- the safest next action;
- where logs/details are available.

## Error Handling

- Missing project folder: show recent project card warning and remove/relink actions.
- Tauri bridge unavailable: show local-preview fallback copy only where browser fallback exists.
- Transcription runtime unavailable: show model settings action.
- Render/export failed: show retry plus log/report link if present.
- Agent proposal rejected: show concise validation reason and `Ask Codex to revise` action.

## Tests

- Frontend tests for project home empty/recent/missing states.
- Frontend tests for help menu setup copy.
- Frontend tests for tour state and skip behavior.
- Frontend tests for narrow-width rail collapse classes and no overlapping critical controls.
- Frontend tests for recovery copy mapping known error categories.
- Browser QA at desktop and narrow widths for editor shell, generation composer, preview, timeline, inspector, and Codex rail.

## Success Criteria

The app opens into a clear project workflow, not an unexplained dense editor. Users can find agent setup, shortcuts, model settings, import/generation, and export status from the shell. Narrow layouts remain usable, and primary failures tell the user what to do next.

## Self-Review

- Scope is product shell polish, not timeline preview or render implementation.
- Requirements preserve the editor-first rule and avoid marketing-page UI.
- Components are split to avoid making `EditorWorkspace` larger.
