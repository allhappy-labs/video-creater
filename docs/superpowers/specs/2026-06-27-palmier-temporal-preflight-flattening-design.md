# Palmier Temporal Preflight Flattening Design

## Context

The workflow queue now reads as a flat right-rail section, and workflow start-request context is inline within each job card. The `Temporal worker preflight` status still renders as a rounded bordered muted panel inside that flat queue, which makes setup state feel like infrastructure chrome rather than compact editor readiness.

Temporal remains the required workflow runtime for queues and scheduling. The editor should show readiness and missing setup only as much as needed to explain disabled actions, while detailed commands and routing stay out of the right rail.

## Goal

Flatten the `Temporal worker preflight` wrapper while preserving readiness status, missing-tool guidance, and hidden operator details.

## Requirements

- Keep the `Temporal worker preflight` status landmark and accessible name.
- Remove the preflight wrapper's `rounded`, `border`, `bg-muted/20`, and `p-2` shell classes.
- Keep the `Temporal worker` label, ready/setup-needed status pill, missing-tool list, and required-tools-available message.
- Keep hidden operator details hidden: task queue, service URLs, web UI URL, dev server label, start-dev command, worker feature name, and worker cargo command.
- Do not change Temporal preflight data, worker setup logic, start dispatch behavior, project files, job sorting, or fal.ai generation behavior.

## Verification

- Extend the existing `ProjectTimelineInspector` preflight test to assert the preflight status wrapper is flat while setup text, missing tools, and hidden operator details remain correct.
- Run the focused preflight test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the `Temporal worker preflight` presentation wrapper.
- Temporal runtime behavior and queue actionability remain unchanged.
