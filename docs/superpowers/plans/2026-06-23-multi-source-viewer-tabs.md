# Multi Source Viewer Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the preview viewer support multiple Palmier-style open source tabs with close controls.

**Architecture:** `EditorWorkspace` owns local open-tab state and derives `PreviewSource` records from `project.media`. `PreviewPanel` remains presentational: it renders the ordered tab list, active source details, and select/close callbacks. No canonical project or Temporal workflow paths change.

**Tech Stack:** React, TypeScript, Tailwind, lucide-react, Vitest/Testing Library, existing `PreviewPanel` and `EditorWorkspace` components.

---

### Task 1: Multi Source Viewer Tabs

**Files:**
- Modify: `src/components/workspace/preview-panel.tsx`
- Modify: `src/components/workspace/preview-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [x] **Step 1: Write failing `PreviewPanel` test**

Add a test that renders two source tabs, expects `Timeline`, `input.mp4`, and `output.mp4`, verifies `output.mp4` is active, clicks `input.mp4`, and clicks `Close output.mp4 viewer tab`.

- [x] **Step 2: Write failing `EditorWorkspace` integration test**

Add a test that starts from the generated clip fixture, verifies the default `input.mp4` tab, double-clicks `Generated product push`, verifies both `input.mp4` and `output.mp4` remain open with `output.mp4` active, switches back to `input.mp4`, closes `output.mp4`, then closes `input.mp4` and verifies `Timeline` becomes active.

- [x] **Step 3: Verify red**

Run:

```bash
rtk pnpm test -- src/components/workspace/preview-panel.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because `PreviewPanel` only accepts one `selectedSource` tab and no close callbacks.

- [x] **Step 4: Implement presentational tab list**

Extend `PreviewPanel` props with `sourceTabs?: PreviewSource[]`, `activeSourceId?: string | null`, `onSelectSourceTab?: (sourceId: string) => void`, and `onCloseSourceTab?: (sourceId: string) => void`. Render all source tabs after `Timeline`. Use `X` from `lucide-react` for close buttons with accessible labels.

- [x] **Step 5: Implement workspace tab state**

In `EditorWorkspace`, add `openViewerSourceIds` and `activeViewerSourceId` state. Update source-opening paths to ensure the source id is in the ordered list, activate it, and update `selectedMediaId`. Derive `viewerSourceTabs` from `openViewerSourceIds`, and pass the active source to `PreviewPanel`.

- [x] **Step 6: Verify green**

Run:

```bash
rtk pnpm test -- src/components/workspace/preview-panel.test.tsx src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS.

- [x] **Step 7: Full checks and visual QA**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk git diff --check
```

Then open the app with Playwright and verify desktop/narrow viewer tabs: multiple source tabs fit or truncate, close buttons are visible, selected tab styling is clear, and source metadata still fits.
