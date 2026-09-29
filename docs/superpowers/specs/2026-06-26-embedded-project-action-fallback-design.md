# Embedded Project Action Fallback Design

## Context

Palmier-style editing expects direct timeline actions: add, split, delete, trim, move, edit text, and toggle tracks without leaving the editor. Video Creater's browser dev path now handles simple timeline patches locally, but richer schema v1 `ProjectAction` edits still call the Tauri `apply_project_action_to_project` command. In a standalone browser with no Tauri bridge, those edits reject and surface `Timeline edit rejected`, which blocks browser smoke tests and weakens the manual editing loop.

## Requirements

- Embedded schema v1 projects should apply common manual `ProjectAction` edits in memory when the Tauri project-action command is unavailable.
- Split-project folders and schema v2 projects must continue using Rust project-action persistence.
- The fallback must cover editor-critical timeline actions: `addItems`, `removeItems`, `moveItems`, `resizeItems`, `trimItems`, `splitItems`, `createTrack`, `setTrackLocked`, `setTrackEnabled`, `editCaptionText`, `editTextItem`, `updateTextOverlayItems`, and `updateTemplateItems`.
- The fallback must preserve undo history when a local action applies.
- Unsupported actions should continue to surface the original error instead of silently pretending to apply.
- The behavior should be implemented as a pure helper in `src/lib/project.ts` and verified separately from React.

## Design

- Add `applyProjectActionLocally(project, action)` beside the existing timeline patch fallback.
- Reuse the existing immutable item update helper and timeline item sorting.
- Recalculate `timeline.durationSeconds` from item end times after timeline mutations.
- For `splitItems`, create left/right clip segments only for valid split points inside the original item duration.
- For text actions, keep `label`, `source.text`, and `properties.text` aligned where the target item has text source.
- Update `EditorWorkspace.applyProjectAction` so only embedded schema v1 project-action failures try the local helper.
- Leave `applyProjectActions` using `applyProjectAction`, so batches inherit the same fallback one action at a time.

## Verification

- Add unit tests for local add/remove/split/text/track action behavior.
- Add a workspace regression test proving a browser-only embedded split action updates the UI when `apply_project_action_to_project` rejects.
- Re-run affected project/workspace tests and TypeScript lint.
- Browser-smoke split/delete or add-text-overlay actions on the embedded sample and verify no timeline rejection appears.
