# Codex MCP Tool Server Parity Design

## Context

Video Creater now has a Rust-owned local Codex tool dispatcher with project context, timeline, media, generation defaults, and validation tools. That closes the first local-control-plane gap, but it is not full Palmier-style parity yet because there is no stdio tool server and the tool surface does not cover the broader project operations agents need.

Palmier-style parity means an external agent can discover tools, call query tools, validate or apply edits through the app boundary, and prepare generation/render/export/transcription work without relying on prompt text alone.

## Goal

Expose a broad local tool control plane through both the existing Tauri command wrappers and an MCP-compatible stdio server binary.

## Scope

Add local tools for:

- project context, timeline, media library, generated assets, workflow jobs, render reports, export artifacts;
- generation defaults and Temporal generate-media start-request construction;
- Codex edit start-request construction and Codex edit proposal validation;
- export profile availability plus Temporal export-media and export-NLE-XML start-request construction;
- transcription readiness checks for generate-edit flows;
- project action validation and split-project application through Rust-owned project-action validation.

Add an MCP-style stdio bridge:

- `initialize`
- `tools/list`
- `tools/call`
- JSON-RPC 2.0 success and error responses
- text `content` plus `structuredContent` for tool call results

The bridge loads the project from `--project-dir <absolute path>` for each call so responses reflect current split-project files.

## Non-Goals

- No network provider execution from MCP tools.
- No direct arbitrary file writes.
- No app-server local-tool registration unless Codex exposes a stable registration contract later.
- No long-running Temporal client start from the MCP server; tools build replayable start requests for the app/worker boundary.

## Safety

- Read-only tools never mutate state.
- `video_creater.apply_project_actions` is the only mutating tool in this slice, and it calls `apply_project_actions_to_split_project`; Rust validation and split-project writers remain the only mutation boundary.
- Tools do not expose provider secret values.
- MCP errors return messages without secrets.

## Testing

Add Rust tests for:

- manifest coverage for all tool names and categories;
- query payloads for generated assets, workflow jobs, render reports, and export artifacts;
- build tools for generation, Codex edit, export media, and NLE XML start requests;
- transcription readiness success/failure payloads;
- `apply_project_actions` mutating a temp split project through Rust validation;
- MCP `initialize`, `tools/list`, `tools/call`, unknown methods, and tool errors.

## Success Criteria

An external agent can run a local stdio server, discover the tool list, call broad project query and operation-prep tools, and apply project edits only through Rust validation.
