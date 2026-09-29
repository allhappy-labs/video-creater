# Codex Tool Call Disclosure Design

## Context

Palmier's assistant screenshots show tool calls as compact chat rows that can be inspected while the
editor stays focused on media, preview, and timeline. Video Creater already records Codex transcript
tool rows for context loads, timeline edits, and generation queue actions, but each row truncates its
detail and target metadata. That makes agent-controlled edits harder to audit from the rail.

Palmier reference: https://www.palmier.io/docs

## Goal

Make Codex transcript tool calls inspectable without changing the chat protocol, project schema, or
workflow execution.

## Behavior

- Each transcript tool row keeps its compact status presentation by default.
- Rows with a message or metadata expose a disclosure button labeled
  `Show <tool> tool call details`.
- Expanding the row reveals the full detail message, target metadata when present, and the current
  status.
- Collapsing the row hides those details and restores the compact row.
- Existing transcript status roles and labels remain available for tests and assistive technology.

## Non-Goals

- No new MCP or Temporal execution model.
- No persisted multi-turn chat history.
- No changes to generated-media, proposal, project-action, or workflow-job data.

## Verification

- `AgentPanel` test proves a queued tool call can reveal and hide its full detail and target.
- Existing AgentPanel transcript tests continue to pass.
- Run focused AgentPanel tests and `pnpm lint`.
- Browser-smoke the Codex rail to confirm the disclosure is compact and does not crowd the composer.
