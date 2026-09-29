# Palmier Workflow Start Request Flattening Design

## Context

The project inspector workflow queue now has a flat outer region, but each queued workflow job still renders its `Start request` context as a bordered panel inside the job card. That creates nested card chrome around profile, format, output, validation, and the `Start workflow` action.

Palmier-style editor rails keep action context close to the editable object without stacking panels. Workflow job cards can remain framed as repeated queue entries, but the start-request facts inside each job should read as inline job metadata.

## Goal

Flatten the `Start request` context inside workflow job cards while preserving Temporal-backed actionability.

## Requirements

- Keep `Start request`, request readiness status, profile, format, output path, video validation, audio validation, and the `Start workflow` button when currently shown.
- Remove the start-request wrapper's `rounded`, `border`, `bg-background`, `px-2`, and `py-1.5` shell classes.
- Keep workflow job cards, Temporal worker preflight, missing-tool rows, empty workflow state, job sorting, and job count summary unchanged.
- Keep hidden-infrastructure behavior unchanged: workflow IDs, task queues, workflow type names, run IDs, activity counts, reuse policy, and credential environment labels stay out of the inspector.
- Do not change Temporal workflow payloads, start dispatch, Rust worker modules, project persistence, or fal.ai generation behavior.

## Verification

- Extend the `ProjectTimelineInspector` start-request readiness test to assert the `Start request` context has no nested shell while the label, readiness status, hidden infrastructure fields, and start action remain intact.
- Run the focused start-request test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to nested start-request presentation inside workflow job cards.
- Temporal workflow behavior and project-file workflows remain unchanged.
