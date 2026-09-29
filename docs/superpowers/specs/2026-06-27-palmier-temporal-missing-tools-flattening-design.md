# Palmier Temporal Missing Tools Flattening Design

## Context

The `Temporal worker preflight` status now sits flat in the project inspector workflow queue, but its `Missing tools` list still renders inside a rounded bordered sub-panel. That nested panel adds visual weight around setup facts that should read like compact readiness rows.

Temporal remains the workflow runtime for queues and scheduling. The right rail should make missing local tools visible enough to explain blocked starts without turning the editor inspector into an operator dashboard.

## Goal

Flatten the `Missing tools` block inside Temporal preflight while preserving each missing tool name and install hint.

## Requirements

- Keep the `Missing tools` label and each missing tool row.
- Make the missing-tools wrapper a named group for accessible testing.
- Remove the wrapper's `rounded`, `border`, `bg-background`, `px-2`, and `py-1.5` shell classes.
- Keep the `Required tools available` message path unchanged when there are no missing tools.
- Keep hidden operator details hidden: task queue, service URLs, web UI URL, dev server label, start-dev command, worker feature name, and worker cargo command.
- Do not change Temporal preflight data, worker setup logic, start dispatch behavior, project files, job sorting, or fal.ai generation behavior.

## Verification

- Extend the existing `ProjectTimelineInspector` preflight test to locate the `Missing tools` group, assert it is flat, and verify the existing missing tool names and install hints still render.
- Run the focused preflight test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the missing-tools presentation wrapper.
- Temporal runtime behavior and queue actionability remain unchanged.
