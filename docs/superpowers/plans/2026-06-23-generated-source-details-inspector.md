# Generated Source Details Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the selected generated-source inspector show Palmier-like file metadata, reference thumbnails, generated settings, and prompt details.

**Architecture:** Keep the change inside `SourceClipInspector`; it already receives timeline item, media assets, and generated assets. Add small render helpers for metadata rows and compact provenance cards, then replace the generated `Details` tab body without changing AI Edit callbacks or project actions.

**Tech Stack:** React, TypeScript, Tailwind, Vitest, Testing Library.

---

### Task 1: Add Failing Details Tests

**Files:**
- Modify: `src/components/workspace/source-clip-inspector.test.tsx`

- [ ] **Step 1: Add a test for Palmier-style generated details**

Add this test after the existing generated-source details tests:

```tsx
it("renders generated source file, reference, settings, and prompt details", () => {
  render(
    <SourceClipInspector
      item={generatedClipItem}
      media={generatedMedia}
      generatedAssets={[generatedAsset]}
    />,
  );

  const details = screen.getByLabelText("Generated details");
  expect(within(details).getByText("File")).toBeInTheDocument();
  expect(within(details).getByText("Type")).toBeInTheDocument();
  expect(within(details).getByText("generated")).toBeInTheDocument();
  expect(within(details).getByText("Dimensions")).toBeInTheDocument();
  expect(within(details).getAllByText("1280 x 720").length).toBeGreaterThan(0);
  expect(within(details).getByText("Frame Rate")).toBeInTheDocument();
  expect(within(details).getByText("24 fps")).toBeInTheDocument();
  expect(within(details).getByText("References")).toBeInTheDocument();
  expect(within(details).getByRole("group", { name: "First Frame media-1" })).toBeInTheDocument();
  expect(within(details).getByRole("group", { name: "Last Frame media-2" })).toBeInTheDocument();
  expect(within(details).getByRole("group", { name: "Reference media-3" })).toBeInTheDocument();
  expect(within(details).getByText("Generated")).toBeInTheDocument();
  expect(within(details).getByText("seedance/seedance-2-fast")).toBeInTheDocument();
  expect(within(details).getByText("16:9")).toBeInTheDocument();
  expect(within(details).getByText("Prompt")).toBeInTheDocument();
  expect(within(details).getByText("slow push-in on the product")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
rtk pnpm test -- src/components/workspace/source-clip-inspector.test.tsx
```

Expected: FAIL because the current generated details view does not render file metadata rows such as `File` and `Dimensions`.

### Task 2: Implement Details Helpers

**Files:**
- Modify: `src/components/workspace/source-clip-inspector.tsx`

- [ ] **Step 1: Add helper functions**

Add helpers near the existing formatting helpers:

```tsx
function formatDimensions(width: number | null | undefined, height: number | null | undefined) {
  return width && height ? `${width} x ${height}` : "unknown";
}

function formatFrameRate(fps: number | null | undefined) {
  return fps ? `${fps} fps` : "still";
}

function detailRow(label: string, value: string) {
  return (
    <div className="grid grid-cols-[5.75rem_1fr] gap-2 border-b border-border/60 py-1 last:border-b-0">
      <div className="text-muted-foreground">{label}</div>
      <div className="min-w-0 break-words text-right font-medium text-foreground">{value}</div>
    </div>
  );
}
```

- [ ] **Step 2: Replace `renderProvenanceMediaCard` layout**

Keep the same function signature and reveal behavior, but render compact thumbnail rows with filename, kind/dimensions/duration metadata, and stable group labels. Use the existing `provenanceIcon`, `filenameFromPath`, and `formatSeconds` helpers.

### Task 3: Replace Generated Details Body

**Files:**
- Modify: `src/components/workspace/source-clip-inspector.tsx`

- [ ] **Step 1: Compute generated detail values**

Inside the generated details branch, derive:

```tsx
const selectedOutputPath = output?.relativePath ?? outputMediaAsset?.relativePath ?? mediaId ?? "unlinked";
const selectedOutputKind = outputMediaAsset?.kind ?? "generated";
const selectedOutputDuration = output?.durationSeconds ?? outputMediaAsset?.durationSeconds ?? generatedAsset.settings.durationSeconds;
const selectedOutputWidth = output?.width ?? outputMediaAsset?.width ?? generatedAsset.settings.width;
const selectedOutputHeight = output?.height ?? outputMediaAsset?.height ?? generatedAsset.settings.height;
const selectedOutputFps = output?.fps ?? outputMediaAsset?.fps ?? generatedAsset.settings.fps;
```

- [ ] **Step 2: Render sections**

Replace the generated details tab content with four sections: `File`, `References`, `Generated`, and `Prompt`. Use `detailRow` for row/value metadata and keep `renderGeneratedReferences()` for reference cards.

- [ ] **Step 3: Run focused tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/source-clip-inspector.test.tsx
```

Expected: PASS.

### Task 4: Verify and Commit

**Files:**
- Source: `src/components/workspace/source-clip-inspector.tsx`
- Tests: `src/components/workspace/source-clip-inspector.test.tsx`
- Docs: `docs/superpowers/specs/2026-06-23-generated-source-details-inspector-design.md`, `docs/superpowers/plans/2026-06-23-generated-source-details-inspector.md`

- [ ] **Step 1: Run verification**

Run:

```bash
rtk pnpm test -- src/components/workspace/source-clip-inspector.test.tsx src/components/workspace/editor-workspace.test.tsx
rtk pnpm lint
rtk pnpm test
```

Expected: all tests pass and lint exits 0.

- [ ] **Step 2: Run browser QA**

Open the app, select a generated timeline clip or generated output state, and capture the inspector screenshot under:

```bash
output/playwright/generated-source-details-inspector.png
```

Check that section labels and row values do not overlap at desktop width.

- [ ] **Step 3: Commit**

Run:

```bash
rtk git add docs/superpowers/specs/2026-06-23-generated-source-details-inspector-design.md docs/superpowers/plans/2026-06-23-generated-source-details-inspector.md src/components/workspace/source-clip-inspector.tsx src/components/workspace/source-clip-inspector.test.tsx
rtk git commit -m "feat: polish generated source inspector"
```
