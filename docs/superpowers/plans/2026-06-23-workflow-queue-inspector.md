# Workflow Queue Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for this implementation. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface project workflow jobs in the editor and persist a Temporal-backed job when generated media is queued.

**Architecture:** Extend `ProjectTimelineInspector` with a compact queue section derived from `VideoProject.jobs`. Add a small frontend helper in `EditorWorkspace` that builds a Temporal-shaped generation job summary and batches it with the existing generated asset action.

**Tech Stack:** React, TypeScript, Vitest, Testing Library.

---

## Tasks

- [x] Add a failing `ProjectTimelineInspector` test for workflow job rendering and empty state.
- [x] Add a failing `EditorWorkspace` test that expects queued media generation to call `apply_project_actions_to_split_project_folder` with `recordJob` and `recordGeneratedAsset`.
- [x] Implement workflow queue rendering in `project-timeline-inspector.tsx`.
- [x] Implement generation job summary creation and batched generation actions in `editor-workspace.tsx`.
- [x] Run focused tests, TypeScript type-checking, and frontend tests.
