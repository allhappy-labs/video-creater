# Codex Chat Rail Chrome

## Context

Palmier's assistant screenshots show chat as a primary editing surface: a compact chat tab, visible context/tool activity, a large prompt composer anchored at the bottom, and the timeline/media workspace always visible beside it. Video Creater already has Codex chat state, selected-source actions, workflow activity, and validated project actions, but the panel still reads partly like a settings form because preset controls and status cards compete with the chat.

## Goal

Polish `AgentPanel` so it feels more like an editor chat rail while preserving the existing `EditJobRequest`, selected-source callbacks, proposal application, and workflow-job display.

## Behavior

- The rail header shows a compact `New chat` tab and a concise workflow activity count.
- A small context-tools region shows the available project context calls as status rows, starting with `get_timeline` and `project_context`.
- The chat transcript remains visible above the composer and keeps current assistant/user/status messages.
- Preset and language controls remain available in a compact `Edit setup` section.
- The composer is visually anchored as the final section, includes the active `@media-id`, and shows the selected local edit agent label.
- Existing generate, blocked-model, selected-source, workflow, proposal, and template behaviors remain unchanged.

## Non-Goals

- No multi-turn backend chat protocol.
- No new MCP tools, Temporal workflows, project actions, or persistence format changes.
- No parsed `@` mention autocomplete.
- No fake tool execution history beyond derived context availability.

## Tests

- `AgentPanel` renders the chat rail chrome with `New chat`, active workflow count, and context tool rows.
- `AgentPanel` renders a bottom composer footer with the active media mention and local edit agent label.
- Existing AgentPanel behavior tests continue to pass.
