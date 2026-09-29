# Embedded Timeline Patch Fallback Design

## Context

Palmier-style manual editing needs timeline actions to feel direct while the editor is open. Video Creater's standalone browser dev path currently renders the full embedded sample workspace, but timeline edits route through Tauri `invoke` and show a rejection when the browser lacks the Tauri bridge. This blocks browser smoke tests from exercising the manual editing UI even though the sample project is already in memory.

## Requirements

- Embedded single-file projects should apply timeline patches in memory when the Tauri `apply_timeline_patch_to_project` command fails.
- Split-project folders and schema v2 projects must continue using the Rust project-action path for text-file-editable persistence.
- The fallback must support the timeline patch types emitted by the React timeline: `moveItem`, `resizeItem`, `trimItem`, and `editCaptionText`.
- The fallback must record undo history the same way a successful Rust patch does.
- The fallback must not hide errors for split projects or other persistence-backed commands.
- The fallback should be implemented as a pure helper in `src/lib/project.ts` so behavior is testable without React.

## Design

- Add `applyTimelinePatchLocally(project, patch)` in `src/lib/project.ts`.
- For `moveItem`, remove the item from its current track, update `startSeconds`, add it to the target track, and sort affected tracks by start time.
- For `resizeItem`, update `durationSeconds`.
- For `trimItem`, update `startSeconds`, `durationSeconds`, and optional `sourceIn`/`sourceOut` properties.
- For `editCaptionText`, update text timeline items and keep the item label/source text aligned.
- Update `EditorWorkspace.applyTimelinePatch` so schema v1 embedded projects try Rust first, then fall back to the pure helper if Rust invoke fails.
- Leave schema v2 split-project edits on the existing Rust path.

## Verification

- Add unit tests for local move, trim, resize, and caption patch application.
- Add a workspace test proving an embedded timeline edit still updates the UI when the Tauri patch command rejects.
- Re-run the relevant project and workspace tests.
- Run `pnpm lint`.
- Browser-smoke a selected clip nudge or dock action and verify no timeline rejection appears for embedded sample edits.
