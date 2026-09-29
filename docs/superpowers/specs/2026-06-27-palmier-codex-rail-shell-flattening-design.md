# Palmier Codex Rail Shell Flattening

## Context

Palmier presents the assistant rail as part of the editor workspace, not as a dashboard card. Video Creater has already removed fake chat tabs, compacted selected context, and flattened the media, preview, timeline, and inspector shells. The remaining `AgentPanel` root still renders as a shadcn `Card` with `CardHeader` and `CardContent`, which makes the Codex rail feel like a framed widget beside otherwise direct editor panes.

## Goal

Make the Codex rail root a native editor panel while preserving the existing chat transcript, workflow activity, conversation controls, mention suggestions, and bottom composer.

## Requirements

- Replace the outer `Card`, `CardHeader`, and `CardContent` wrapper in `AgentPanel` with semantic editor chrome.
- Keep the root `role="region"` and `aria-label="Codex chat rail"`.
- Keep the rail height constraints, local scrolling, and bottom composer anchoring.
- Keep the `Codex conversations` toolbar with `New chat`, `Start new Codex chat`, and disabled history controls.
- Do not add tabs, toggles, switches, model pickers, or secondary chat modes.
- Do not change generation, Temporal workflow, mention, selected-source, or selected-timeline action contracts.

## Non-Goals

- No backend chat protocol changes.
- No changes to generated media queue payloads or fal.ai defaults.
- No changes to transcript row styling inside the rail.
- No changes to Codex composer behavior or primary-action routing.

## Acceptance Tests

- `AgentPanel` proves the `Codex chat rail` root does not carry card shell classes such as `bg-card`, `text-card-foreground`, `shadow-sm`, or `rounded-md`.
- `AgentPanel` proves the rail still uses a full-height flex column with local overflow control.
- Existing Codex rail chrome, new-chat reset, transcript, workflow activity, blocked composer, and project-action tests continue to pass.
