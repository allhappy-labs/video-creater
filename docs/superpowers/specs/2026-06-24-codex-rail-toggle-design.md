# Codex Rail Toggle Design

## Context

Palmier keeps the AI assistant close to the editing workspace, but its manual editing screenshots also give the media library, preview, inspector, and timeline most of the width. Video Creater now has a four-pane layout with Codex, source library, preview/timeline, and inspector always visible at the desktop breakpoint. That is useful for agent-first work, but it leaves the media library and center editor cramped when a user is doing manual editing.

The workspace header already has a `Codex chat` icon button, but it does not change the workspace. This creates a natural place to make the agent rail one click away without adding a new toolbar.

## Goal

Let editors toggle the Codex rail from the project chrome so the app can switch between agent-assisted four-pane editing and a wider Palmier-style manual editing layout.

## Behavior

- The Codex rail is visible by default.
- The existing `Codex chat` header button toggles the rail open or closed.
- The button exposes `aria-pressed` so assistive technology can read whether the rail is open.
- When the rail is closed, the editor panes grid renders only the source library, preview/timeline, and inspector at the desktop breakpoint.
- The closed layout gives the source library and preview/timeline more room than the four-pane layout.
- Clicking `Codex chat` again restores the Codex rail and the four-pane grid.
- Source library, preview/timeline, inspector, and project actions remain mounted and usable in both modes.

## Non-Goals

- No persisted user preference.
- No draggable or resizable split panes.
- No changes to the Codex chat content, generation behavior, Temporal workflow records, or project files.
- No new compact floating assistant widget.

## Testing

- `EditorWorkspace` tests prove the Codex button starts pressed and the Codex rail is present by default.
- Tests prove clicking `Codex chat` hides the rail and switches the panes grid to the three-pane desktop class.
- Tests prove clicking it again restores the rail and four-pane class.
- Existing layout, render, media, source inspector, and Codex tests continue to pass.

## Browser QA

- Desktop: verify the default four-pane layout still shows Codex, media, preview/timeline, and inspector.
- Desktop after toggle: verify the Codex rail disappears and the media/preview areas widen without overlapping controls.
- Narrow: verify the `Codex chat` button remains reachable and toggling the rail does not create horizontal overflow.
