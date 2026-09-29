# Export File Type Row Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the read-only `File Type` settings row while retaining the selected format and extension in the export-dialog footer.

**Architecture:** Keep `ExportSheetProfile.fileType` because the footer still consumes it. Change only presentation and visual-QA expectations; export selection, capability state, and workflow payloads remain unchanged.

**Tech Stack:** React, TypeScript, Testing Library, Vitest, Playwright-based browser visual QA, Markdown parity documentation.

## Global Constraints

- Keep `Codec`, `Quality`, `Resolution`, and `Frame Rate` in the video settings area.
- Keep the selected format and extension in the footer, such as `WebM (.webm)`.
- Do not change codec selection, export payloads, capability gating, or render behavior.
- Prefix every shell command with `rtk`.

---

### Task 1: Remove the read-only row and refresh its QA contract

**Files:**
- Modify: `src/components/workspace/export-sheet.test.tsx`
- Modify: `src/components/workspace/export-sheet.tsx`
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `src/browser-visual-qa-script.test.ts`
- Modify: `docs/parity.md`

**Interfaces:**
- Consumes: `ExportSheetProfile.fileType` for footer summary text.
- Produces: an export dialog with no `File Type` settings row and an unchanged footer format summary.

- [ ] **Step 1: Write the failing component regression**

Add this test to `src/components/workspace/export-sheet.test.tsx`:

```tsx
it("shows file type only as footer summary metadata", () => {
  render(<ExportSheet {...exportSheetProps()} />);

  fireEvent.change(screen.getByRole("combobox", { name: "Codec" }), {
    target: { value: "webm" },
  });
  expect(screen.queryByText("File Type", { exact: true })).not.toBeInTheDocument();
  expect(screen.getByText("WebM (.webm)")).toBeVisible();
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
rtk pnpm vitest run src/components/workspace/export-sheet.test.tsx -t "shows file type only as footer summary metadata"
```

Expected: FAIL because the dialog still renders the `File Type` label.

- [ ] **Step 3: Remove the settings row only**

Delete this line from the video settings block in `src/components/workspace/export-sheet.tsx`:

```tsx
<SettingRow label="File Type">{outputProfile?.fileType ?? "—"}</SettingRow>
```

Keep the existing footer summary unchanged:

```tsx
<span>{outputProfile?.fileType ?? "—"}</span>
```

- [ ] **Step 4: Update browser QA and parity wording**

In `scripts/browser-visual-qa.mjs`, replace the positive `File Type` visibility wait with:

```js
if (await exportDialog.getByText("File Type", { exact: true }).count()) {
  throw new Error("Export dialog still shows read-only File Type row");
}
```

In `src/browser-visual-qa-script.test.ts`, assert the new rejection copy and remove the old positive visibility assertion:

```ts
expect(scriptSource).toContain("Export dialog still shows read-only File Type row");
```

Update `docs/parity.md` so the Export row and 2026-07-14 evidence describe codec, quality, user-selectable resolution, frame rate, and footer format summary without claiming a visible `File Type` control.

- [ ] **Step 5: Run focused and static verification**

Run:

```bash
rtk pnpm vitest run src/components/workspace/export-sheet.test.tsx src/browser-visual-qa-script.test.ts
rtk pnpm lint
```

Expected: all focused tests pass and TypeScript exits successfully.

- [ ] **Step 6: Refresh browser visual evidence**

With the local Vite server available, run:

```bash
rtk pnpm visual:qa:browser -- --only export:video:desktop --out output/visual-qa/export-codec-quality
```

Expected: `output/visual-qa/export-codec-quality/export-video-dialog-desktop.png` shows no `File Type` row and retains `WebM (.webm)` in the footer.

- [ ] **Step 7: Commit the implementation**

```bash
rtk git add src/components/workspace/export-sheet.test.tsx src/components/workspace/export-sheet.tsx scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts docs/parity.md
rtk git commit -m "fix(export): remove read-only file type row"
```
