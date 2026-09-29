# Palmier Workflow Queue Shell Flattening Design

## Context

The project inspector has been moving toward Palmier-style flat rail sections: project identity, format, AI media, latest export, recent exports, and latest render now read as inline editor facts. `Workflow queue` still has an outer rounded bordered panel even though its inner entries already carry their own status framing.

Workflow state is important for Video Creater because queues, scheduling, and execution are Temporal-backed. The right rail should keep that state visible and actionable without making the entire queue feel like a separate infrastructure widget.

## Goal

Flatten the `Workflow queue` outer region while preserving Temporal job visibility, preflight status, and start actions.

## Requirements

- Keep the `Workflow queue` named region and job count summary.
- Remove the outer region's `rounded-md`, `border`, `bg-background`, and `p-2` shell classes.
- Keep Temporal worker preflight, missing-tool rows, `No workflow jobs`, workflow job cards, start-request context, and `Start workflow` behavior unchanged.
- Keep the existing compacting contract that hides workflow IDs, task queues, workflow type names, run IDs, activity chips, reuse policies, and credential environment variable labels from the inspector.
- Do not change Temporal workflow payloads, Rust worker paths, project persistence, job sorting, queue caps, or fal.ai generation behavior.

## Verification

- Extend `ProjectTimelineInspector` workflow queue coverage to assert the outer region is flat while job cards still render and hide Temporal internals.
- Run the focused workflow queue test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to the `Workflow queue` outer presentation shell.
- Temporal-backed execution and workflow start behavior remain unchanged.
