# Responsive Workspace Containment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Contain wide timeline/editor content at stacked widths so narrow workspace panes do not overlap each other.

**Architecture:** Keep the existing `EditorWorkspace` layout and `TimelineEditor` internals. Add stable landmarks and width containment classes to the workspace grid and center editor column so wide descendants use their existing local scrollers rather than expanding the grid item. Avoid parent overflow clipping so the stacked center editor keeps its natural vertical height. Use Testing Library class assertions as a regression guard for the layout contract.

**Tech Stack:** React, TypeScript, Tailwind CSS, Vitest, Testing Library, Playwright CLI for visual QA.

---

### Task 1: Workspace Containment Regression

**Files:**
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`

- [x] **Step 1: Write the failing layout test**

Add a test near the existing shell/layout tests:

```tsx
it("contains the center editor column when workspace panes stack", () => {
  render(<EditorWorkspace />);

  const workspaceGrid = screen.getByRole("region", { name: "Editor panes" });
  const centerEditor = screen.getByRole("region", { name: "Preview and timeline editor" });

  expect(workspaceGrid).toHaveClass("grid-cols-1");
  expect(workspaceGrid).toHaveClass("auto-rows-max");
  expect(workspaceGrid).toHaveClass("lg:grid-cols-[260px_minmax(0,1fr)_300px]");
  expect(centerEditor).toHaveClass("w-full");
  expect(centerEditor).toHaveClass("min-h-fit");
  expect(centerEditor).toHaveClass("lg:min-h-0");
  expect(centerEditor).toHaveClass("min-w-0");
  expect(centerEditor).toHaveClass("max-w-full");
});
```

- [x] **Step 2: Run the focused test to verify red**

Run:

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because the workspace grid has no `Editor panes` landmark and the center editor column has no landmark or overflow containment.

- [x] **Step 3: Add workspace and center editor landmarks**

Change the workspace pane wrapper in `src/components/workspace/editor-workspace.tsx` to:

```tsx
<section
  role="region"
  aria-label="Editor panes"
  className="grid min-h-0 w-full flex-1 auto-rows-max grid-cols-1 gap-3 overflow-auto p-3 lg:grid-cols-[260px_minmax(0,1fr)_300px]"
>
```

Change the center editor wrapper to:

```tsx
<section
  role="region"
  aria-label="Preview and timeline editor"
  className="flex min-h-fit w-full max-w-full min-w-0 flex-col gap-3 lg:min-h-0"
>
```

Keep the source library and inspector rail unchanged except for any formatting required by the wrapper element.

- [x] **Step 4: Run the focused test to verify green**

Run:

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: PASS.

### Task 2: Verification And Visual QA

**Files:**
- Modify only if verification exposes a defect.

- [x] **Step 1: Run full checks**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk git diff --check
```

Expected: all commands exit 0.

- [x] **Step 2: Browser QA**

Run the dev server, open the app in Playwright, and capture:

```bash
output/playwright/responsive-workspace-desktop.png
output/playwright/responsive-workspace-narrow.png
```

At desktop width, verify the source library, preview/timeline, and inspector rail still share one row. At narrow width, scroll through the stacked panes and verify the timeline no longer paints over the project/Codex rail.

- [x] **Step 3: Clean temporary Playwright state**

Run:

```bash
rtk rm -rf .playwright-cli
```

Expected: no transient Playwright state remains in `git status --short`.

### Task 3: Commit

**Files:**
- Stage: `docs/superpowers/specs/2026-06-23-responsive-workspace-containment-design.md`
- Stage: `docs/superpowers/plans/2026-06-23-responsive-workspace-containment.md`
- Stage: `src/components/workspace/editor-workspace.tsx`
- Stage: `src/components/workspace/editor-workspace.test.tsx`

- [x] **Step 1: Commit the slice**

Run:

```bash
rtk git add docs/superpowers/specs/2026-06-23-responsive-workspace-containment-design.md docs/superpowers/plans/2026-06-23-responsive-workspace-containment.md src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "fix: contain stacked editor workspace"
```

Expected: commit succeeds with only the planned files staged.

---

## Self-Review

- Spec coverage: the plan covers stacked pane containment, desktop preservation, tests, visual QA, and no workflow/Temporal changes.
- Placeholder scan: no TBD/TODO placeholders or undefined implementation steps.
- Type consistency: landmark labels and Tailwind class names match the planned test assertions.
