# Claude Code Agent Setup Copy Design

## Context

Palmier's docs describe Help -> MCP Instructions as the setup path for Cursor, Codex, Claude Code,
and Claude Desktop. Video Creater now has an in-app MCP instructions panel with copyable setup
snippets for Codex, Claude Desktop, and Cursor, but Claude Code is missing from the same first-class
agent setup surface.

## Goal

Add a `Copy Claude Code setup` action to the MCP connection instructions panel.

## Behavior

- The Copy setup group includes Codex, Claude Code, Claude Desktop, and Cursor actions.
- `Copy Claude Code setup` writes a plain-text setup brief with the current project folder.
- The snippet names `Claude Code`, points the agent at `timeline.json`, `media/index.json`, and
  template files, and states `Structured proposals only`.
- Existing Codex, Claude Desktop, and Cursor setup snippets keep their current copy and behavior.

## Non-Goals

- No new MCP server binary.
- No automatic client config or one-click install flow.
- No project schema, Temporal workflow, fal.ai, render, or proposal contract changes.

## Verification

- `EditorWorkspace` tests prove the MCP instructions panel shows `Copy Claude Code setup`.
- Clicking it writes text containing the current project folder, `Claude Code`,
  `Structured proposals only`, and `timeline.json`.
