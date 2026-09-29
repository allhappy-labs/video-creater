# Project Chrome Model Settings Help Menu Design

## Purpose

Reduce the project chrome's persistent top-left controls while preserving access to model setup. The workspace should keep Home, Codex chat, and Help visible, with lower-frequency setup actions grouped under Help.

## Behavior

- Remove the standalone `Settings` icon button from `Workspace navigation`.
- Add a `Model settings` item to the existing `Help menu`.
- Selecting `Model settings` calls `onOpenModelSettings` when provided and closes the menu.
- Keep `MCP instructions` in the same menu, below `Model settings`, and preserve its current open/close behavior.
- The Help button remains icon-only with `aria-haspopup="menu"` and `aria-expanded` reflecting menu state.

## UI Details

The menu remains compact and aligned to the left edge of the Help button. Each menu row uses a lucide icon, a short label, and an optional muted description so the menu reads as a setup/help cluster rather than a second toolbar.

## Testing

- Update the project chrome test to assert the standalone `Settings` button is no longer present.
- Assert the Help menu contains `Model settings` and `MCP instructions`.
- Add a focused test that selecting `Model settings` invokes `onOpenModelSettings` once and closes the menu.
