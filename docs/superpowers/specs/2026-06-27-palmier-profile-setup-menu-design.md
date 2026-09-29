# Palmier Profile Setup Menu Design

## Context

Palmier's editor chrome keeps the left side sparse: Home and chat sit beside the centered project title, while Export and the profile control sit on the right. Video Creater still shows a separate `Help` icon in the primary left navigation. That keeps setup/help actions visible, but it adds top-level chrome that is not present in the Palmier screenshots.

Existing setup actions remain important: model settings and MCP connection instructions make the agent-controlled workflow usable. They move into the existing profile control instead of occupying a separate navigation slot.

Palmier reference: https://www.palmier.io/docs

## Goal

Remove the standalone `Help` button from the project chrome and make the profile control open a compact setup menu.

## Requirements

- `Workspace navigation` contains `Home` and `Codex chat`, but no `Help` button.
- `Workspace actions` contains `Export` and a clickable `Workspace profile` control.
- Activating `Workspace profile` opens a `Workspace profile menu`.
- The profile menu contains `Model settings` and `MCP instructions`.
- Selecting `Model settings` calls `onOpenModelSettings` and closes the menu.
- Selecting `MCP instructions` toggles the existing `MCP connection instructions` panel and closes the menu.
- The existing MCP instructions panel content, copy setup buttons, and text-file-editable project guidance remain unchanged.
- Do not add a new toolbar item, tab, switch, or modal.

## Testing

- Update project chrome tests to prove `Help` is absent and the profile menu exposes setup actions.
- Update model settings, MCP instructions, and setup copy tests to open the profile menu instead of the old Help menu.
- Run focused workspace tests, full workspace tests, typecheck, full Vitest, diff check, and browser QA for the top chrome and profile menu.
