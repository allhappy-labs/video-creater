# Full Suite Green Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or superpowers:subagent-driven-development) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align stale workspace and provider-harness assertions with the current editor and Tauri runtime contracts so `pnpm test` is green.

**Architecture:** Keep production UI and Cargo configuration unchanged because each observed failure is an outdated test expectation. Reuse the source inspector's accessible `AI Edit` tab contract, current media-overflow menu actions, and the optional Tauri dependency feature used by the app runtime, while preserving the provider harness isolation checks.

**Tech Stack:** React Testing Library, Vitest, TypeScript, Cargo.toml manifest assertions.

## Global Constraints

- Preserve the current dirty worktree; edit only failing test expectations and this plan.
- Keep the generated source and imported source tests accessible by interacting through named tabs.
- Do not change provider harness runtime boundaries or Cargo features merely to satisfy a test.

---

### Task 1: Refresh Editor Workspace Test Contracts

**Files:**
- Modify: `src/components/workspace/editor-workspace.test.tsx:101-115, 699-704, 3209-3213`
- Test: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: `SourceClipInspector` tablists named `Generated source inspector views` and `Source inspector views`.
- Produces: Tests that select the `AI Edit` tab before querying generated/imported edit controls.

- [x] **Step 1: Confirm the current failures**

Run: `rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx --reporter=verbose`

Expected: failures report missing `Generated AI edit`, missing `Imported AI edit`, and obsolete `ring-2` selection expectations.

- [x] **Step 2: Update the stale assertions**

```ts
const generatedTabs = within(sourceInspector).queryByRole("tablist", {
  name: "Generated source inspector views",
});
expect(screen.getByRole("button", { name: "Opening clip" })).toHaveClass("ring-1");
expect(within(sourceInspector).getByRole("tablist", { name: "Source inspector views" })).toBeInTheDocument();
```

- [x] **Step 3: Verify the workspace suite**

Run: `rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx`

Expected: exit 0 with all EditorWorkspace tests passing.

### Task 2: Refresh the Provider Harness Manifest Contract

**Files:**
- Modify: `src/provider-e2e-script.test.ts:10302`
- Test: `src/provider-e2e-script.test.ts`

**Interfaces:**
- Consumes: the optional `tauri` dependency declaration in `src-tauri/Cargo.toml`.
- Produces: a regression check that permits the app runtime's `protocol-asset` feature while still proving the harness does not include `app-runtime`.

- [x] **Step 1: Confirm the current manifest mismatch**

Run: `rtk pnpm vitest run src/provider-e2e-script.test.ts`

Expected: the Tauri isolation assertion expects `features = []` while the manifest contains `features = ["protocol-asset"]`.

- [x] **Step 2: Match the current optional dependency declaration**

```ts
expect(rootManifest).toContain(
  'tauri = { version = "2.11.2", features = ["protocol-asset"], optional = true }',
);
```

- [x] **Step 3: Verify the provider E2E test**

Run: `rtk pnpm vitest run src/provider-e2e-script.test.ts`

Expected: exit 0 with all provider E2E script tests passing.

### Task 3: Verify the Entire Repository

**Files:**
- Verify: `src/components/workspace/editor-workspace.test.tsx`, `src/provider-e2e-script.test.ts`, `src-tauri/Cargo.toml`

- [x] **Step 1: Run static checks**

Run: `rtk pnpm lint && rtk git diff --check`

Expected: both commands exit 0.

- [x] **Step 2: Run the full test suite**

Run: `rtk pnpm test`

Expected: exit 0 with zero failed test files and zero failed tests.
