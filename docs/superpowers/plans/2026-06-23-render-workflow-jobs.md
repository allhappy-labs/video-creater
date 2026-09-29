# Render Workflow Job Records Plan

## Goal

Make render controls write Temporal-shaped job records into the project action stream before attaching render reports.

## Steps

1. Add failing editor-workspace tests for draft/final render job actions.
2. Define render workflow constants and a `renderDraftJobSummary` helper beside media-generation workflow helpers.
3. Change `recordRenderReport` to batch `recordJob`, `updateJobStatus`, and `attachRenderReport`.
4. Extend the split-project test mock so batched render actions update `jobs` and `renderReports`.
5. Run focused tests, typecheck, lint, full tests, and diff checks.

## Follow-Up

- Replace the immediate completion action with live Temporal client status once the Rust worker is available.
- Split final WebM into a profile-specific workflow type when render orchestration needs different worker behavior.
