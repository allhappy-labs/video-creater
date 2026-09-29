# Palmier Empty AI Media State Flattening Design

## Context

The project inspector has been flattened so project facts, export status, render status, workflow readiness, and empty workflow state read as compact right-rail metadata. The `AI media` section still renders its no-assets state as a dashed bordered block saying `No generated media yet`.

Palmier-style media generation belongs directly in the editor flow. When a project has no generated assets, the right rail should show a compact status line rather than a standalone empty card.

## Goal

Flatten the `AI media` empty state while preserving the empty message and generated-media summaries.

## Requirements

- Keep `No generated media yet` visible when the project has no generated assets.
- Expose the empty state as a named status for accessibility and tests.
- Remove the empty state's `rounded`, `border`, `border-dashed`, `bg-background`, `px-2`, and `py-3` shell classes.
- Keep AI media summary metrics, generated asset cards, output rows, inspect actions, workflow status pills, prompt excerpts, and generation data unchanged.
- Do not change generated asset persistence, Temporal workflow behavior, fal.ai generation payloads, or media folder routing.

## Verification

- Extend the existing empty AI media test to assert the empty status is flat while the message remains visible.
- Run the focused empty AI media test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the empty AI media presentation.
- Generated media behavior and workflow-backed asset generation remain unchanged.
