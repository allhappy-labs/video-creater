# Viewer Source Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Palmier-like Timeline/source tabs above the preview viewport.

**Architecture:** Keep viewer mode as local React state in `EditorWorkspace`. `PreviewPanel` stays presentational and receives selected media metadata plus callbacks for tab selection.

**Tech Stack:** React, TypeScript, Tailwind, shadcn Button, lucide-react, Vitest/Testing Library.

---

### Task 1: Preview Panel Source Tabs

**Files:**
- Modify: `src/components/workspace/preview-panel.tsx`
- Create: `src/components/workspace/preview-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [x] **Step 1: Write failing tests**

Add a `PreviewPanel` test that renders a selected source and expects `Viewer tabs`, `Timeline`, source filename, source metadata, and callback behavior. Add an `EditorWorkspace` integration test that double-clicks a generated timeline clip and expects the viewer source tab to become active.

- [x] **Step 2: Run focused tests**

Run: `rtk pnpm test -- src/components/workspace/preview-panel.test.tsx src/components/workspace/editor-workspace.test.tsx`

Expected: FAIL because `PreviewPanel` has no viewer tabs yet.

- [x] **Step 3: Implement minimal UI**

Add `ViewerMode`, `PreviewSource`, and tab props to `PreviewPanel`. Render a compact tablist and source metadata only when a selected source exists. Wire `EditorWorkspace` to derive the selected source from `project.media` and `selectedMediaId`.

- [x] **Step 4: Verify focused behavior**

Run: `rtk pnpm test -- src/components/workspace/preview-panel.test.tsx src/components/workspace/editor-workspace.test.tsx`

Expected: PASS.

- [x] **Step 5: Run full checks and visual QA**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk git diff --check
```

Then open Vite with Playwright and check desktop/narrow layouts for the viewer tab strip.
