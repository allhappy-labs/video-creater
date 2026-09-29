# Desktop Editor Chrome and Timeline Actions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify desktop editor chrome, prevent source-library overlap, and compact selected-clip actions without losing any command or accessibility affordance.

**Architecture:** Keep render choice and execution in the existing ExportSheet; the header only opens that sheet. Retain the existing project-save state machine and capability callbacks, but render their current state through a smaller header and icon-only desktop toolbar. Containment remains a CSS responsibility at the source-panel scroll boundary.

**Tech Stack:** React, TypeScript, Tailwind CSS, lucide-react, shadcn-style Button, Vitest, Testing Library, Playwright visual QA.

## Global Constraints

- Desktop editor changes must use `lucide-react` plus the existing `Button` primitive; every icon-only button needs an exact `aria-label` and native `title` tooltip.
- The desktop header must not expose `Render quality`, `Render draft`, or `Render final`; ExportSheet remains the sole header route to render/export selection.
- `Edited` is visible only for `projectSaveStatus === "unsaved"`; saved projects show no success-status copy.
- Source-library content must scroll inside the source-library panel and never paint across the timeline.
- The narrow action menu remains text-based and keeps every existing command.
- Shell commands are prefixed with `rtk`; commits use Conventional Commit messages.

---

### Task 1: Simplify the editor header and make save state exclusive

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx:216,3168,7770-7860`
- Modify: `src/components/workspace/editor-workspace.test.tsx:11640-11690`

**Interfaces:**
- Consumes: `projectSaveStatus: ProjectSaveStatus`, `selectedRenderQuality: RenderQualityProfile`, `exportSheetOpen`, and the existing `ExportSheet` submit handler.
- Produces: header markup that opens ExportSheet without direct render-quality selection or direct WebM render action.

- [ ] **Step 1: Write the failing header contract test**

  In `editor-workspace.test.tsx`, replace the existing expectations for header render controls with a test that asserts only Export is available, saved state is silent, and unsaved state displays `Edited`.

  ```tsx
  expect(within(headerActions).getByRole("button", { name: "Export" })).toBeInTheDocument();
  expect(within(headerActions).queryByRole("combobox", { name: /render quality/i })).not.toBeInTheDocument();
  expect(within(headerActions).queryByRole("button", { name: /render draft/i })).not.toBeInTheDocument();
  expect(screen.queryByText("Saved")).not.toBeInTheDocument();
  expect(screen.queryByText("Edited")).not.toBeInTheDocument();
  ```

  Add a browser-fallback project mutation in the same test file, then assert the changed header has the dirty indicator:

  ```tsx
  fireEvent.click(screen.getByRole("button", { name: "Opening clip" }));
  fireEvent.click(screen.getByRole("button", { name: "Apply grain" }));
  expect(await screen.findByText("Edited")).toBeInTheDocument();
  expect(screen.queryByText("Unsaved changes")).not.toBeInTheDocument();
  ```

- [ ] **Step 2: Run the focused test and verify it fails**

  Run: `rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx`

  Expected: FAIL because the header still exposes Render quality, Render draft, `Edited`, and `Saved` simultaneously.

- [ ] **Step 3: Remove duplicate header rendering controls and make status copy exclusive**

  Remove the `RenderQualityControl` import and the header-only selector/button. Keep `selectedRenderQuality` because `ExportSheet`, range rendering, and render-report surfaces still consume it. Change the center title/status markup to conditionally render only actionable non-saved state:

  ```tsx
  <h1 className="truncate text-sm font-semibold tracking-normal" title={project.name}>
    {project.name}
    {projectSaveStatus === "unsaved" ? (
      <span className="text-muted-foreground"> - Edited</span>
    ) : null}
  </h1>
  {projectSaveStatus !== "saved" && projectSaveStatus !== "unsaved" ? (
    <div
      role={projectSaveStatus === "failed" ? "alert" : "status"}
      className="mt-1 text-[11px] text-muted-foreground"
    >
      {projectSaveStatus === "saving" ? "Saving" : "Save failed"}
    </div>
  ) : null}
  ```

- [ ] **Step 4: Run the focused test and verify it passes**

  Run: `rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx`

  Expected: PASS, including the changed header contract.

- [ ] **Step 5: Commit the header slice**

  ```bash
  rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
  rtk git commit -m "refactor(editor): simplify desktop render chrome"
  ```

### Task 2: Convert desktop selected-clip actions to accessible icon controls

**Files:**
- Modify: `src/components/workspace/selected-clip-actions.tsx:1-114`
- Modify: `src/components/workspace/selected-clip-actions.test.tsx:1-57`
- Modify: `src/components/workspace/editor-workspace.test.tsx:9700-9720`

**Interfaces:**
- Consumes: existing `SelectedClipActionsProps` callbacks and `can*` capability booleans.
- Produces: an unchanged callback contract, with icon buttons named `Link selected`, `Unlink selected`, `Remove selected`, `Nudge −0.25s`, `Nudge +0.25s`, `Apply grain`, `Apply vignette`, `Clear effects`, and conditional `Decompose sequence`.

- [ ] **Step 1: Write failing desktop icon-toolbar tests**

  Add a `showDesktopActions` test that renders every capability, asserts semantic button names and tooltips, and proves the destructive button remains disabled when removal is not allowed.

  ```tsx
  const desktopActions = screen.getByRole("group", { name: "Selected clip actions" });
  expect(desktopActions).toHaveTextContent("2 clips selected");
  for (const name of [
    "Link selected", "Unlink selected", "Remove selected", "Nudge −0.25s",
    "Nudge +0.25s", "Apply grain", "Apply vignette", "Clear effects", "Decompose sequence",
  ]) {
    expect(within(desktopActions).getByRole("button", { name })).toHaveAttribute("title", name);
  }
  expect(within(desktopActions).getByRole("button", { name: "Remove selected" })).toHaveClass("text-destructive");
  ```

- [ ] **Step 2: Run the focused test and verify it fails**

  Run: `rtk pnpm vitest run src/components/workspace/selected-clip-actions.test.tsx`

  Expected: FAIL because the desktop component has group name `Selected clip linking`, text buttons, and no tooltips.

- [ ] **Step 3: Implement the icon-only desktop toolbar**

  Replace the desktop-only markup with `Button size="icon" variant="ghost"` controls, importing `Link2`, `Unlink2`, `Trash2`, `ChevronLeft`, `ChevronRight`, `Sparkles`, `Aperture`, `RotateCcw`, and `Ungroup` (or the nearest available lucid equivalents). Every button keeps the current `disabled` flag and callback and adds its exact accessible name/title:

  ```tsx
  <Button
    type="button"
    size="icon"
    variant="ghost"
    className="h-8 w-8"
    aria-label="Link selected"
    title="Link selected"
    disabled={!canLink}
    onClick={onLink}
  >
    <Link2 className="h-4 w-4" aria-hidden="true" />
  </Button>
  ```

  Use the same pattern for all nine commands. Wrap them in `role="group" aria-label="Selected clip actions"`, retain the selection count, preserve separators between command families, and apply `text-destructive hover:text-destructive` to Remove. Do not change the narrow action menu.

- [ ] **Step 4: Run component and integration tests**

  Run: `rtk pnpm vitest run src/components/workspace/selected-clip-actions.test.tsx src/components/workspace/editor-workspace.test.tsx`

  Expected: PASS, proving all commands remain actionable both in the desktop toolbar and narrow menu.

- [ ] **Step 5: Commit the toolbar slice**

  ```bash
  rtk git add src/components/workspace/selected-clip-actions.tsx src/components/workspace/selected-clip-actions.test.tsx src/components/workspace/editor-workspace.test.tsx
  rtk git commit -m "refactor(timeline): compact selected clip actions"
  ```

### Task 3: Contain media-library content within the source panel

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx:8670-8690`
- Modify: `src/components/workspace/editor-workspace.test.tsx:1886-1915,2094-2105`
- Modify: `scripts/browser-visual-qa.mjs:532-535`

**Interfaces:**
- Consumes: the existing source-library `<aside aria-label="Source library panel">` and its MediaBin child.
- Produces: a panel-local scroll region that has `min-h-0`, `flex-1`, and `overflow-y-auto`, while the outer panel stays `overflow-hidden`.

- [ ] **Step 1: Add the failing containment assertions**

  Extend the existing viewport-bound test to identify the source-panel scroll child and assert the exact containment classes. Add `data-testid="source-library-scroll-region"` to make the relationship stable.

  ```tsx
  const sourcePanelScrollRegion = screen.getByTestId("source-library-scroll-region");
  expect(sourcePanel).toHaveClass("overflow-hidden");
  expect(sourcePanelScrollRegion).toHaveClass("min-h-0", "flex-1", "overflow-y-auto");
  ```

  Add a visual-QA assertion after the existing `assertWorkspaceBoundsAndReachability` pane checks that verifies the source scroll region’s right and bottom edges are within the source panel’s bounding box.

  ```js
  const sourcePanel = page.getByLabel("Source library panel");
  const sourceScroll = page.getByTestId("source-library-scroll-region");
  const [panelBox, scrollBox] = await Promise.all([sourcePanel.boundingBox(), sourceScroll.boundingBox()]);
  if (!panelBox || !scrollBox || scrollBox.y + scrollBox.height > panelBox.y + panelBox.height + 1) {
    throw new Error("Source library scroll region escapes its panel");
  }
  ```

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run: `rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx src/browser-visual-qa-script.test.ts`

  Expected: FAIL because the scroll region has no test id and the visual-QA script has no source-library bounds assertion.

- [ ] **Step 3: Make the scroll owner explicit**

  Keep the source-library `<aside>` clipped. Do not alter the existing `MediaBin` invocation or any of its props; only mark its direct wrapper as the local scroll owner:

  ```tsx
  - <div className="min-h-0 flex-1 space-y-2 overflow-y-auto pr-1">
  + <div
  +   data-testid="source-library-scroll-region"
  +   className="min-h-0 flex-1 space-y-2 overflow-y-auto pr-1"
  + >
  ```

  Add the bounded-rectangle check to the visual-QA helper so the desktop screenshot run fails before a panel can overlap the timeline again.

- [ ] **Step 4: Run focused tests and desktop visual QA**

  Run: `rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx src/browser-visual-qa-script.test.ts`

  Expected: PASS.

  Run: `rtk pnpm dev`

  In a second terminal, run: `rtk pnpm visual:qa:browser -- --only editor::desktop --out output/design-review/desktop-editor-chrome`

  Expected: an accepted 1440×960 editor frame with no media content crossing into timeline chrome.

- [ ] **Step 5: Commit the containment slice**

  ```bash
  rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts
  rtk git commit -m "fix(editor): contain source library content"
  ```

### Task 4: Final verification and desktop capture refresh

**Files:**
- Modify only if the visual baseline is intentionally refreshed: `docs/visual-qa/browser-visual-baseline/editor-desktop.png`

**Interfaces:**
- Consumes: completed header, toolbar, and containment changes.
- Produces: final test evidence and a current desktop review image.

- [ ] **Step 1: Run the relevant full TypeScript test suite**

  Run: `rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx src/components/workspace/selected-clip-actions.test.tsx src/browser-visual-qa-script.test.ts`

  Expected: PASS.

- [ ] **Step 2: Run static validation**

  Run: `rtk pnpm lint`

  Expected: PASS with no TypeScript errors.

- [ ] **Step 3: Capture and inspect the corrected desktop editor**

  Run: `rtk pnpm visual:qa:browser -- --only editor::desktop --out output/design-review/desktop-editor-chrome`

  Expected: one nonblank `editor-desktop.png` at 1440×960. Inspect it: the header has no render selector/draft button, the saved title is clean, media is bounded by its pane, and selected actions are compact icons with tooltips available on hover.

- [ ] **Step 4: Commit any intentional baseline refresh**

  ```bash
  rtk git add docs/visual-qa/browser-visual-baseline/editor-desktop.png
  rtk git commit -m "test(visual): refresh editor desktop baseline"
  ```
