# Palmier Project Metric Card Removal Design

## Context

The project timeline inspector now presents identity, summary rows, format, AI media, exports, workflow state, and render status with a flatter Palmier-style rail rhythm. A remaining two-card block near the top still shows `Duration` and `Contents` as rounded bordered cards. Those same facts are already available through flat project summary rows and timeline context, so the cards duplicate information and reintroduce dashboard-style chrome.

Palmier-style editor rails should prioritize direct editing context and avoid redundant one-off cards. Repeated generated assets, export artifacts, and workflow jobs can remain card-like because they are object lists; project-level metrics should read as rows.

## Goal

Remove the duplicated `Duration` and `Contents` cards from the project timeline inspector.

## Requirements

- Remove the top-level two-card block that renders `Duration` and `Contents` in `ProjectTimelineInspector`.
- Keep duration, track count, item count, render format, locked tracks, media count, generated count, project identity, format rows, AI media, exports, workflow queue, and render status visible where currently exposed outside that block.
- Keep `ProjectTimelineContext` unchanged; its compact metric boxes remain outside this inspector slice.
- Do not change timeline data, project persistence, Temporal workflows, generated media behavior, or export/render behavior.

## Verification

- Extend `ProjectTimelineInspector` tests to prove the inspector no longer renders card-style `Duration` and `Contents` metric regions while the flat project summary still shows render, locked, media, and generated facts.
- Run the focused inspector test, full inspector suite, TypeScript, `git diff --check`, and the full Vitest suite.

## Self-Review

- No placeholders or unresolved questions.
- Scope is limited to duplicated project metric cards in `ProjectTimelineInspector`.
- Durable project/media/timeline/workflow behavior remains unchanged.
