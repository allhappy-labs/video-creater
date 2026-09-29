# Media Bin Mock Completion Action

## Problem

Queued fal-backed generated assets can now be completed through a mock split-project command, but the editor UI does not expose that action. Development users still have to rely on tests or direct command calls instead of exercising the Palmier-style media generation loop from the media panel.

## Scope

Add a compact action to generated asset cards:

- Show `Complete mock` only for queued or running generated assets.
- Invoke a parent callback with the generated asset id.
- In `EditorWorkspace`, call `complete_mock_generated_asset_in_split_project_folder` with the active split project directory and current timestamp.
- Replace local project state with the command result.

## Constraints

- Completed generated assets keep their existing output selection, replacement, and insertion actions.
- The button is a development/mock completion control; it must use the same Rust project-action path as real fal completion.
- No real fal API credential or network call is used in this slice.

## Acceptance

- MediaBin tests prove queued generated assets expose `Complete mock` and completed assets do not.
- EditorWorkspace tests prove clicking the action calls the split-project command and updates the visible generated asset state.
- Existing media-bin, editor-workspace, project wrapper, and frontend test suites pass.
