# Captions Workbench Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent the captions workbench footer and agent-action menu from visually overlapping at the default 320px media-panel width.

**Architecture:** Restore the missing global popover design tokens and expose them through Tailwind so all `bg-popover` surfaces are opaque. Keep exactly one `p-3` inset around the captions workbench by removing the captions-tab wrapper inset, allowing its two footer controls to fit without changing their labels or actions.

**Tech Stack:** React, TypeScript, Tailwind CSS, Vitest, Playwright visual QA.

## Global Constraints

- Preserve existing pending media-panel behavior and do not alter caption action callbacks.
- Shell commands are prefixed with `rtk`.
- Validate the visible 1440px desktop editor with its default 320px media panel.

---

### Task 1: Make caption popovers opaque and reclaim the duplicated horizontal inset

**Files:**
- Modify: `src/index.css:6-23`
- Modify: `tailwind.config.ts:8-39`
- Modify: `src/components/workspace/media-bin.tsx:6831-6838`
- Create: `src/captions-workbench-layout.test.ts`

**Interfaces:**
- Consumes: Tailwind `bg-popover` and `text-popover-foreground` utilities used by caption agent actions.
- Produces: defined color variables and a captions-tab wrapper with no extra `p-3` inset.

- [x] **Step 1: Write the failing layout-contract test**

Create `src/captions-workbench-layout.test.ts` with source-level assertions for the two styling contracts:

```ts
expect(indexCss).toMatch(/--popover:\s*30 7% 9%/);
expect(indexCss).toMatch(/--popover-foreground:\s*38 14% 92%/);
expect(tailwindConfig).toContain('DEFAULT: "hsl(var(--popover))"');
expect(mediaBin).toContain('panelTab === "captions" ? (\n        <div className="h-full min-h-0 overflow-y-auto">');
```

- [x] **Step 2: Run the focused test and verify it fails**

Run: `rtk pnpm vitest run src/captions-workbench-layout.test.ts`

Expected: FAIL because the popover variables are absent and the captions wrapper still has `p-3`.

- [x] **Step 3: Apply the minimal CSS and wrapper changes**

Add to the root theme tokens:

```css
--popover: 30 7% 9%;
--popover-foreground: 38 14% 92%;
```

Remove only `p-3` from the captions-tab scroll wrapper:

```tsx
<div className="h-full min-h-0 overflow-y-auto">
```

Add the matching Tailwind color mapping so `bg-popover` emits a background declaration:

```ts
popover: {
  DEFAULT: "hsl(var(--popover))",
  foreground: "hsl(var(--popover-foreground))",
},
```

- [x] **Step 4: Run the focused test and verify it passes**

Run: `rtk pnpm vitest run src/captions-workbench-layout.test.ts`

Expected: PASS.

- [x] **Step 5: Run visual verification**

With a fresh Vite process running, open the sample project, switch to Captions, open Agent Mode, and capture `output/playwright/captions-layout-audit-final.png`. The computed Agent Mode menu background is `rgb(25, 23, 21)` and both footer button labels remain within the source panel.
