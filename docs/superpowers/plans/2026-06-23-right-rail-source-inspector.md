# Right Rail Source Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move selected source clip inspection into the right rail so Video Creater behaves more like Palmier's selection-aware Source inspector.

**Architecture:** Keep the existing `SourceClipInspector` component and handlers. Change only `EditorWorkspace` layout so the center column renders caption/template inspectors, while the right rail chooses between `SourceClipInspector` and `ProjectTimelineInspector` based on the current source selection.

**Tech Stack:** React, TypeScript, Tailwind, Vitest, Testing Library.

---

### Task 1: Add Failing Layout Tests

**Files:**
- Modify: `src/components/workspace/editor-workspace.test.tsx`

- [ ] **Step 1: Add a test for right rail source inspection**

Add this test near the existing source inspector workspace tests:

```tsx
it("shows selected source clip controls in the right rail", () => {
  render(<EditorWorkspace initialProject={createProjectWithGeneratedClipSequence()} />);

  fireEvent.click(screen.getByRole("button", { name: "Generated product push" }));

  const rightRail = screen.getByRole("complementary", { name: "Inspector and Codex rail" });
  expect(within(rightRail).getByRole("heading", { name: "Source Inspector" })).toBeInTheDocument();
  expect(within(rightRail).getByRole("heading", { name: "Codex" })).toBeInTheDocument();
  expect(
    screen.getByRole("region", { name: "Selected item editor" }),
  ).not.toHaveTextContent("Source Inspector");
});
```

- [ ] **Step 2: Add a test for caption fallback**

```tsx
it("keeps the timeline inspector in the right rail for caption selections", () => {
  render(<EditorWorkspace initialProject={createSplitProject()} />);

  fireEvent.click(screen.getByRole("button", { name: "Caption 1" }));

  const rightRail = screen.getByRole("complementary", { name: "Inspector and Codex rail" });
  expect(
    within(rightRail).getByRole("region", { name: "Project timeline inspector" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("region", { name: "Selected item editor" }),
  ).toHaveTextContent("Caption");
});
```

- [ ] **Step 3: Run red tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: FAIL because the right rail has no `complementary` landmark and source inspection still renders below the timeline.

### Task 2: Move Source Inspection Into the Right Rail

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`

- [ ] **Step 1: Add a selected item editor landmark**

Wrap the center column's conditional editor area in:

```tsx
<section role="region" aria-label="Selected item editor">
  {selectedTemplateItem ? (
    <TemplateInspector item={selectedTemplateItem} onApply={applyTemplateUpdate} />
  ) : selectedCaptionItem ? (
    <CaptionInspector item={selectedCaptionItem} onApply={applyCaptionText} />
  ) : null}
</section>
```

Do not render `SourceClipInspector` in this center section.

- [ ] **Step 2: Add a right rail landmark**

Change the right rail wrapper to:

```tsx
<aside
  role="complementary"
  aria-label="Inspector and Codex rail"
  className="flex min-w-0 flex-col gap-3"
>
```

- [ ] **Step 3: Render the source or project inspector in the right rail**

Inside the right rail, before `AgentPanel`, render:

```tsx
{selectedSourceClipItem ? (
  <SourceClipInspector
    item={selectedSourceClipItem}
    media={project.media}
    generatedAssets={project.generatedAssets}
    reorderContext={selectedSourceClipReorderContext}
    onApply={applySourceClipTrim}
    onSplit={applySourceClipSplit}
    onReorder={applySourceClipReorder}
    onRevealSource={revealSourceMedia}
    onQueueVariation={queueGeneratedClipVariation}
    onQueueReferencedGeneration={queueReferencedMediaGeneration}
  />
) : (
  <ProjectTimelineInspector project={project} projectDir={projectDir} />
)}
```

- [ ] **Step 4: Run green tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx
```

Expected: all editor workspace tests pass.

### Task 3: Verification and Commit

**Files:**
- Source: `src/components/workspace/editor-workspace.tsx`
- Tests: `src/components/workspace/editor-workspace.test.tsx`
- Docs: `docs/superpowers/specs/2026-06-23-right-rail-source-inspector-design.md`, `docs/superpowers/plans/2026-06-23-right-rail-source-inspector.md`

- [ ] **Step 1: Run focused checks**

```bash
rtk pnpm test -- src/components/workspace/editor-workspace.test.tsx src/components/workspace/source-clip-inspector.test.tsx
rtk pnpm lint
```

Expected: tests and lint pass.

- [ ] **Step 2: Run full frontend tests**

```bash
rtk pnpm test
```

Expected: all tests pass.

- [ ] **Step 3: Browser QA**

Run the app, select a source clip in the timeline, and save:

```bash
output/playwright/right-rail-source-inspector.png
```

Check that the right rail shows `Source Inspector` above `Codex`, the center column does not duplicate source controls, and no text overlaps at desktop width.

- [ ] **Step 4: Commit**

```bash
rtk git add docs/superpowers/specs/2026-06-23-right-rail-source-inspector-design.md docs/superpowers/plans/2026-06-23-right-rail-source-inspector.md src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat: move source inspector to right rail"
```
