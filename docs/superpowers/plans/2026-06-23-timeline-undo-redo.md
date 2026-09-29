# Timeline Undo Redo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Palmier-style undo and redo controls to the timeline toolbar backed by local project snapshot history.

**Architecture:** `EditorWorkspace` owns history snapshots around successful validated project mutations. `TimelineEditor` stays presentational and receives undo/redo availability plus callbacks. Split-folder projects are persisted by saving the restored project snapshot on undo/redo.

**Tech Stack:** React, TypeScript, Tailwind, lucide-react, Vitest/Testing Library, existing Tauri project action APIs.

---

### Task 1: Timeline Toolbar Undo Redo

**Files:**
- Modify: `src/components/workspace/timeline-editor.tsx`
- Modify: `src/components/workspace/timeline-editor.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [x] **Step 1: Write failing toolbar test**

Add a `TimelineEditor` test that renders `canUndo`, `canRedo`, `onUndo`, and `onRedo`, then expects enabled `Undo timeline edit` and `Redo timeline edit` buttons before the select tool and verifies each callback is called.

- [x] **Step 2: Write failing workspace history test**

Add an `EditorWorkspace` integration test that selects `Opening clip`, splits it from the toolbar, sees the split item, clicks undo to remove the split, then clicks redo to restore it.

- [x] **Step 3: Verify red**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because the undo/redo props and toolbar controls do not exist yet.

- [x] **Step 4: Implement toolbar controls**

Add `canUndo`, `canRedo`, `onUndo`, and `onRedo` props to `TimelineEditor`. Import `Undo2` and `Redo2` from `lucide-react`. Render two icon buttons before `Select timeline tool`, with disabled states and accessible labels.

- [x] **Step 5: Implement workspace history**

Add `ProjectHistoryState` to `EditorWorkspace` with `past` and `future`. Push the previous project snapshot only after successful `applyProjectAction` or successful `applyProjectActions`. Add `restoreProjectFromHistory`, `undoProjectEdit`, and `redoProjectEdit`; use `saveSplitProjectToFolder` when restoring schema v2 split projects. Wire history props into `TimelineEditor`.

- [x] **Step 6: Verify green**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS.

- [x] **Step 7: Full checks and visual QA**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk git diff --check
```

Then open the app with Playwright and verify desktop/narrow toolbar layout: undo and redo fit before select, disabled/enabled states are visible, and the toolbar does not overlap zoom controls.
