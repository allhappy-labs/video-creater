# Agent MCP Instructions Design

## Context

Palmier documents agent connection as a first-class workflow: users open in-app MCP instructions and connect Codex, Claude, Cursor, or another agent so it can generate media, inspect project context, and edit the timeline. Video Creater already has agent-oriented project files, Codex chat, Temporal-shaped workflow jobs, and validated project actions, but the editor chrome does not expose any setup guidance for connecting an external agent.

## Goal

Add a compact in-app MCP instructions surface from the editor chrome. The surface should make the agent contract visible without claiming that a full MCP server is implemented.

## Behavior

- Add a `MCP instructions` icon button to the top editor navigation.
- Clicking the button toggles a dismissible `MCP connection instructions` panel below the chrome.
- The panel explains:
  - use the project folder as the agent workspace;
  - project files are text-editable JSON assets such as `video-creater.project.json`, `timeline.json`, `media/index.json`, and template files;
  - agents should return structured proposals or project actions, not directly mutate canonical state;
  - Rust validates timeline edits, generated media jobs, and render/export actions;
  - Temporal workflow records and fal generation jobs are visible in the queue.
- The panel includes compact setup rows for Codex app-server, Claude/Cursor MCP clients, and manual file editing.
- Toggling the button again hides the panel.

## Non-Goals

- No new MCP server implementation in this slice.
- No clipboard writing or external app setup.
- No project schema changes.
- No new agent mutation permissions.

## Testing

- `EditorWorkspace` renders an accessible `MCP instructions` button in the project chrome.
- Clicking the button shows the instructions panel and expected agent contract copy.
- Clicking the button again hides the panel.
- Existing editor chrome, Codex rail, render, export, and inspector tests continue to pass.

## Acceptance

- A user can discover the connected-agent workflow from the editor itself.
- The copy is honest about the current contract: agents work through structured proposals/actions and Rust validation.
- The panel aligns with the Palmier docs' "Connect your agent" workflow while preserving Video Creater's local-first safety model.
