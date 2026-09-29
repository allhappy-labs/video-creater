# Copyable Agent Setup Design

Palmier's connection docs make external agents feel first-class by giving editors concrete setup steps instead of only explaining the concept. Video Creater now exposes an MCP instructions panel from the editor chrome, but the panel is still passive text. Editors should be able to copy a project-aware setup brief for Codex, Claude Desktop, or Cursor without hunting through docs.

## Goal

Add copyable setup snippets to the existing `MCP connection instructions` panel. The snippets must make the current implementation boundary clear:

- Codex is launched through `codex app-server --stdio`.
- External agents should use the visible project folder as their workspace.
- Project files remain text-editable, especially `timeline.json`, `media/index.json`, templates, and queue files.
- Agents must return structured proposals only; Rust validates project actions before canonical state changes.

## Non-Goals

- Do not add a new MCP server binary.
- Do not write client config files automatically.
- Do not claim Claude Desktop or Cursor can directly mutate project state.
- Do not change project storage, proposal schemas, Temporal workflows, or render behavior.

## UI

Inside the existing MCP instructions panel, show a compact `Copy setup` group with three icon buttons:

- `Copy Codex setup`
- `Copy Claude Desktop setup`
- `Copy Cursor setup`

Each button copies plain text. The copied content should include the current `projectDir`, the `codex app-server --stdio` command where relevant, and the structured-proposal constraint.

## Tests

- `EditorWorkspace` opens the MCP instructions panel and shows the three setup copy buttons.
- Clicking `Copy Codex setup` writes text containing the current project folder, `codex app-server --stdio`, and `Structured proposals only`.
- Clicking `Copy Claude Desktop setup` writes text containing the current project folder and `timeline.json`.
- Clicking `Copy Cursor setup` writes text containing the current project folder and `media/index.json`.
