# Codex Local Tool Control Plane Design

## Context

Palmier exposes a broad local MCP control plane for inspecting project state, editing through validated actions, and starting generation-oriented work. Video Creater already has Codex app-server integration for chat and EDL-first proposal generation, but the available local tool surface is implicit: prompts describe project context and expected `projectActions`, while Rust validates results after the turn.

That leaves parity missing or hard to discover. Agents cannot ask the app for a tool manifest, call a named local tool, validate a project action batch before proposing it, or validate a Codex edit proposal through a small deterministic API.

## Goal

Add the first explicit Rust-owned local tool control plane for Codex/Palmier parity. The control plane should be discoverable, safe by default, and aligned with the existing rule that Rust owns canonical project mutation.

## Approaches Considered

1. Implement a full MCP server process immediately.
   This offers the strongest protocol parity, but it adds transport and lifecycle scope before the tool contract is stable.

2. Add a typed local tool manifest and dispatcher in Rust, then expose it through Tauri commands.
   This gives the app and Codex integration a clear tool API now, keeps validation in Rust, and can be wrapped by an MCP stdio server later without changing tool semantics.

3. Extend only the Codex prompt text with more tool descriptions.
   This is cheapest, but it does not solve discoverability or create a callable API.

Recommendation: approach 2. It is the smallest useful slice that establishes the contract Palmier-like control surfaces need.

## Tool Surface

Create a `codex::tools` module with:

- `list_codex_local_tools() -> Vec<CodexLocalToolDescriptor>`
- `call_codex_local_tool(project, tool_name, args) -> Result<CodexLocalToolCallResult, CodexLocalToolError>`

Initial tools:

- `video_creater.project_context`: returns project id, name, schema version, counts, Codex thread id, and split-project file hints when a project directory is supplied.
- `video_creater.timeline`: returns bounded timeline tracks and items with ids, timing, kinds, source references, labels, and properties.
- `video_creater.media_library`: returns bounded folders and media assets.
- `video_creater.generation_defaults`: returns default generation settings derived from optional selected or referenced media.
- `video_creater.validate_project_actions`: deserializes a `ProjectAction[]`, applies it to a cloned project, and returns `{ valid: true }` or a structured validation error without mutating the input project.
- `video_creater.validate_codex_edit_proposal`: deserializes a Codex edit proposal plus edit request, runs the existing EDL and project-action validation, and returns proposal duration and clip count.

## Boundaries

- No tool directly writes canonical files or mutates the passed project.
- No network calls or provider credentials are exposed.
- No full MCP stdio server is added in this slice.
- Split-project paths are returned only as local operator hints; tool args remain JSON values.
- Proposal validation continues to require a real EDL before visual layers.

## App API

Expose two Tauri commands:

- `list_codex_local_tools`
- `call_codex_local_tool`

The commands mirror the Rust API so the frontend, app-server bridge, or a future MCP wrapper can discover and execute the same tools.

## Testing

Add Rust integration tests proving:

- the manifest includes query, edit-validation, and generation/proposal tools with stable names and input schemas;
- context/timeline/media tools return bounded project information;
- generation defaults honor selected media dimensions and duration;
- project-action validation returns success for a valid cloned action and does not mutate the original project;
- project-action validation returns a structured error for invalid actions;
- Codex edit proposal validation returns clip count and EDL duration for a valid proposal;
- unknown tools fail with an explicit error.

## Success Criteria

The app has a discoverable local tool API comparable in shape to Palmier's control plane, while preserving Rust-owned validation and proposal-based Codex edits.
