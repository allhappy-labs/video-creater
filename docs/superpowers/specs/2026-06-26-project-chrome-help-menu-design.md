# Project Chrome Help Menu Design

## Context

Palmier keeps the editor's top-left chrome sparse and places MCP setup under Help in the
documentation. Video Creater currently exposes `MCP instructions` as a persistent icon beside Home,
Codex, and Settings. That makes setup chrome compete with editing controls even though MCP setup is
an occasional task.

## Goal

Move MCP connection instructions behind a compact Help menu while keeping setup discoverable and all
copy actions intact.

## Behavior

- Project chrome shows a `Help` icon button instead of an always-visible `MCP instructions` button.
- Clicking `Help` opens a small menu with an `MCP instructions` item.
- Choosing `MCP instructions` toggles the existing MCP connection instructions panel and closes the
  Help menu.
- Clicking `Help` again closes the menu without opening the instructions panel.
- The existing MCP instructions panel content and copy setup buttons stay unchanged.
- The chrome still keeps Home, Codex chat, Settings, Export, and profile access.

## Non-Goals

- Do not redesign the full header layout in this slice.
- Do not remove MCP setup, copy snippets, or project-file editing guidance.
- Do not change Codex, Temporal, fal.ai, render, or project schema behavior.

## Verification

- Workspace tests prove the persistent `MCP instructions` chrome button is gone.
- Workspace tests prove `Help` opens a menu with `MCP instructions`.
- Existing MCP panel and copy setup tests pass through the new menu path.
- Browser QA confirms the top-left chrome is less cluttered and MCP setup remains reachable.
