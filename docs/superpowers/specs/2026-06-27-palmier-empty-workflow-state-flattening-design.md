# Palmier Empty Workflow State Flattening Design

## Context

The workflow queue, Temporal preflight, missing-tools rows, ready-tools message, and start-request context have been flattened to match the project inspector's Palmier-style rail rhythm. The `No workflow jobs` empty state still renders as a dashed bordered block, which makes the empty queue look like a standalone utility card.

When there are no workflow jobs, the right rail only needs a compact status line. The absence of jobs should not compete visually with project facts, export history, render status, or timeline editing controls.

## Goal

Flatten the workflow queue empty state while preserving the empty message.

## Requirements

- Keep the `No workflow jobs` text when the project has no visible jobs.
- Expose the empty state as a named status for accessibility and tests.
- Remove the empty state's `rounded`, `border`, `border-dashed`, `bg-muted/20`, `px-2`, and `py-3` shell classes.
- Keep workflow job cards, Temporal preflight, missing-tools rows, ready-tools message, job sorting, and start actions unchanged.
- Do not change Temporal workflow payloads, job persistence, project files, or fal.ai generation behavior.

## Verification

- Extend the existing empty workflow queue test to assert the empty status is flat while the message remains visible.
- Run the focused empty-state test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the empty workflow queue presentation.
- Workflow runtime behavior and queue actionability remain unchanged.
