# Editor Redesign 02 — Hard Cut and Editor Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- Delete the old editor UI in one gate-green commit.
- Land the new CapCut-dark tokens and the Zustand editor store.
- Build the fixed shell at both layouts, in both the Tauri app and the browser fixture runtime:
  - **Desktop:** top bar, left tab panel, preview region, conditional properties panel, and timeline region.
  - **Mobile:** top bar, preview, timeline, bottom tool bar, and bottom sheets.

**Architecture:**
- **App mounting:** `src/editor/editor-root.tsx` replaces `EditorWorkspace` behind the same `App.tsx` view.
- **Store:** it creates one Zustand store per open project, composed of `project`, `selection`, `playback`, `ui` and `timelineView` slices.
  - The `project` slice owns persistence.
  - Split-folder action writes go through `applyProjectActionsToSplitProjectFolder`, with a local fallback when the backend is unavailable.
  - Undo and redo are snapshot-based and persisted through `saveSplitProjectToFolder`.
- **Layout mode:** derived from viewport width. Mobile is below 1024 px, and properties dock at 1280 px and above.
- **Panel content:** regions render empty but accessible placeholders. Plans 03–07 fill them.

**Tech Stack:** React 19, TypeScript 5.7, Tailwind 3.4, `zustand` 5, `@radix-ui/react-tabs`, `@radix-ui/react-tooltip`, `@radix-ui/react-dropdown-menu`, `@radix-ui/react-dialog`, lucide-react 0.468, Vitest + Testing Library, Playwright 1.63.

**Spec:** [2026-09-13 Editor UI/UX Redesign Design](../specs/2026-09-13-editor-ui-ux-redesign-design.md)
**Depends on:** [Plan 01](2026-09-13-editor-redesign-01-foundations-extraction.md) completed.

## Global Constraints

**Process**
- Prefix every repository shell command with `rtk`.
- Use Conventional Commits and stage only the files named by each task.
- Every commit keeps `rtk pnpm lint`, `rtk pnpm test` and `rtk pnpm check:unused` green. Tasks 3, 4 and 12 also run the full `rtk pnpm verify:frontend`.

**Accessible names**
Keep these names; the retained scripts and later plans rely on them:
- `main` "Video editor workspace"
- region "Preview panel"
- region "Preview viewport"
- region "Timeline canvas"
- button "Home"
- button "Export"

**Code standards**
- No editor source file over 600 lines.
- No raw hex colors and no `white/…` or `black/…` opacity classes in `src/editor/**` or `src/components/ui/**`. Use tokens.
- Icons come from `lucide-react`. Every icon-only control has an `aria-label` and a tooltip.
- Primitives go under `src/components/ui/`. Add a primitive only in the commit where it is first imported, which keeps knip green.

---

## File Map

### New files
- `src/components/home/project-home.tsx` and `project-home.test.tsx`, moved from `components/workspace/`.
- `scripts/refresh-browser-visual-baseline-manifest.mjs` and `scripts/refresh-browser-visual-baseline-manifest.test.ts`: rebuild manifest artifact entries from a baseline folder.
- `src/editor/editor-root.tsx`: App-facing root that creates the store, reports native menu state, and chooses the layout.
- `src/editor/editor-root.test.tsx`.
- `src/editor/store/editor-store.ts`: store factory, `EditorState`, `EditorStoreInit`.
- `src/editor/store/project-slice.ts`, `project-slice.test.ts`.
- `src/editor/store/selection-slice.ts`, `playback-slice.ts`, `ui-slice.ts`, `timeline-view-slice.ts`.
- `src/editor/store/slices.test.ts`.
- `src/editor/store/editor-store-context.tsx`: provider plus `useEditorStore` and `useEditorStoreApi`.
- `src/editor/store/persisted-layout.ts`, `persisted-layout.test.ts`: `localStorage` persistence under `video-creater.editor.v2.`.
- `src/editor/shell/use-layout-mode.ts`, `use-layout-mode.test.ts`.
- `src/editor/shell/top-bar.tsx`, `top-bar.test.tsx`.
- `src/editor/shell/left-panel.tsx`, `left-panel.test.tsx`.
- `src/editor/shell/editor-tabs.ts`: tab ids, labels, icons, shortcut ids.
- `src/editor/shell/panel-placeholder.tsx`.
- `src/editor/shell/preview-region.tsx`, `timeline-region.tsx`, `properties-region.tsx`.
- `src/editor/shell/split-resizer.tsx`, `split-resizer.test.tsx`.
- `src/editor/shell/desktop-layout.tsx`, `desktop-layout.test.tsx`.
- `src/editor/shell/mobile-layout.tsx`, `mobile-layout.test.tsx`.
- `src/editor/shell/bottom-sheet.tsx`.
- `src/editor/shell/use-editor-shortcuts.ts`, `use-editor-shortcuts.test.tsx`.
- `src/components/ui/tooltip.tsx`, `tabs.tsx`, `dropdown-menu.tsx`, `dialog.tsx`, `icon-button.tsx`.
- `e2e/editor-shell.spec.ts`.

### Deleted files
- `src/components/workspace/**`, every remaining file (73 source and test files).
- `src/lib/workspace-layout-state.ts` and its test.
- `src/lib/editor-layout-budget.ts` and its test.
- `src/lib/responsive-rail-state.ts` and its test.
- `src/lib/editor-information-architecture.ts` and its test.
- `src/lib/modern-editor-visual-qa-fixtures.ts` and its test.
- `src/browser-visual-qa-palmier-scenarios.test.ts`.
- 75 editor PNGs in each of `docs/visual-qa/browser-visual-baseline/` and `docs/visual-qa/browser-visual-baseline-linux-x64/`.

### Modified files
- `src/App.tsx`, `src/App.test.tsx`.
- `src/captions-workbench-layout.test.ts`.
- `scripts/browser-visual-qa.mjs`, `src/browser-visual-qa-script.test.ts`.
- `scripts/browser-visual-baseline-policy.mjs` (help text), `src/browser-visual-baseline-policy-script.test.ts`.
- `scripts/palmier-visual-comparison.mjs`.
- Both baseline manifests.
- `e2e/app-smoke.spec.ts`.
- `knip.jsonc`.
- `package.json`.
- `src/index.css`, `tailwind.config.ts`.
- `src-tauri/tauri.conf.json`.

---

### Task 1: Move the project home component out of the workspace folder

**Files:**
- Create: `src/components/home/project-home.tsx`, `src/components/home/project-home.test.tsx` (both moved)
- Delete: `src/components/workspace/project-home.tsx`, `src/components/workspace/project-home.test.tsx`
- Modify: `src/App.tsx:4`

- [ ] **Step 1: Move the files with history**

```bash
rtk mkdir -p src/components/home
rtk git mv src/components/workspace/project-home.tsx src/components/home/project-home.tsx
rtk git mv src/components/workspace/project-home.test.tsx src/components/home/project-home.test.tsx
```

- [ ] **Step 2: Update the import**

In `src/App.tsx`, replace
```ts
import { ProjectHome, type RecentProjectEntry } from "@/components/workspace/project-home";
```
with
```ts
import { ProjectHome, type RecentProjectEntry } from "@/components/home/project-home";
```
Then run `rtk rg -n "workspace/project-home" src scripts e2e`. Expected: no output. If any file still references the old path, update it the same way.

- [ ] **Step 3: Verify**

Run: `rtk pnpm vitest run src/components/home src/App.test.tsx && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
rtk git add src/components/home src/components/workspace/project-home.tsx src/components/workspace/project-home.test.tsx src/App.tsx
rtk git commit -m "refactor(home): move project home out of the workspace folder"
```

---

### Task 2: Baseline manifest refresh tool

The baseline manifests pin width, height, byte length and SHA-256 per PNG, and no tool writes them today. Tasks 3 and 4 need a deterministic refresher.

**Files:**
- Create: `scripts/refresh-browser-visual-baseline-manifest.mjs`, `scripts/refresh-browser-visual-baseline-manifest.test.ts`
- Modify: `package.json` (`test:source-quality` list, new `visual:qa:refresh-manifest` script)

- [ ] **Step 1: Write the failing test**

```ts
// scripts/refresh-browser-visual-baseline-manifest.test.ts
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
// @ts-expect-error untyped script module
import { refreshManifest, pngDimensions } from "./refresh-browser-visual-baseline-manifest.mjs";

function png(width: number, height: number): Buffer {
  const buffer = Buffer.alloc(33);
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(buffer, 0);
  buffer.writeUInt32BE(13, 8);
  buffer.write("IHDR", 12, "ascii");
  buffer.writeUInt32BE(width, 16);
  buffer.writeUInt32BE(height, 20);
  return buffer;
}

test("reads PNG dimensions from the IHDR chunk", () => {
  assert.deepEqual(pngDimensions(png(1440, 960)), { width: 1440, height: 960 });
});

test("rewrites screenshots and artifacts for the requested shots and keeps metadata", () => {
  const dir = mkdtempSync(join(tmpdir(), "baseline-"));
  writeFileSync(join(dir, "home-desktop.png"), png(1440, 960));
  writeFileSync(join(dir, "settings-narrow.png"), png(390, 844));
  const manifestPath = join(dir, "manifest.json");
  writeFileSync(
    manifestPath,
    JSON.stringify({
      schemaVersion: 2,
      description: "d",
      screenshots: ["home-desktop.png", "editor-desktop.png"],
      platformThresholds: { "linux-x64": { threshold: 0.01, channelThreshold: 4 } },
      artifacts: { "editor-desktop.png": { width: 1, height: 1, bytes: 1, sha256: "x" } },
      environment: { os: "o" },
    }),
  );

  refreshManifest({
    baselineDir: dir,
    manifestPath,
    screenshots: ["home-desktop.png", "settings-narrow.png"],
  });

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.deepEqual(manifest.screenshots, ["home-desktop.png", "settings-narrow.png"]);
  assert.deepEqual(Object.keys(manifest.artifacts), ["home-desktop.png", "settings-narrow.png"]);
  assert.equal(manifest.artifacts["home-desktop.png"].width, 1440);
  assert.equal(manifest.artifacts["settings-narrow.png"].bytes, 33);
  assert.match(manifest.artifacts["home-desktop.png"].sha256, /^[0-9a-f]{64}$/);
  assert.deepEqual(manifest.platformThresholds, { "linux-x64": { threshold: 0.01, channelThreshold: 4 } });
  assert.deepEqual(manifest.environment, { os: "o" });
});

test("fails when a requested screenshot file is missing", () => {
  const dir = mkdtempSync(join(tmpdir(), "baseline-"));
  const manifestPath = join(dir, "manifest.json");
  writeFileSync(manifestPath, JSON.stringify({ schemaVersion: 2, screenshots: [], artifacts: {} }));
  assert.throws(
    () => refreshManifest({ baselineDir: dir, manifestPath, screenshots: ["missing.png"] }),
    /Missing baseline screenshot .*missing\.png/,
  );
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `rtk node --test scripts/refresh-browser-visual-baseline-manifest.test.ts`
Expected: FAIL, because the module cannot be found.

- [ ] **Step 3: Implement**

```js
// scripts/refresh-browser-visual-baseline-manifest.mjs
#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function pngDimensions(buffer) {
  if (buffer.length < 24 || buffer.toString("ascii", 12, 16) !== "IHDR") {
    throw new Error("Not a PNG with an IHDR chunk");
  }
  return { width: buffer.readUInt32BE(16), height: buffer.readUInt32BE(20) };
}

export function refreshManifest({ baselineDir, manifestPath, screenshots }) {
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const artifacts = {};
  for (const name of screenshots) {
    const path = join(baselineDir, name);
    if (!existsSync(path)) throw new Error(`Missing baseline screenshot ${path}`);
    const buffer = readFileSync(path);
    artifacts[name] = {
      ...pngDimensions(buffer),
      bytes: buffer.length,
      sha256: createHash("sha256").update(buffer).digest("hex"),
    };
  }
  const next = { ...manifest, screenshots: [...screenshots], artifacts };
  writeFileSync(manifestPath, `${JSON.stringify(next, null, 2)}\n`);
  return next;
}

async function main() {
  const args = process.argv.slice(2).filter((value) => value !== "--");
  const read = (flag) => {
    const index = args.indexOf(flag);
    return index >= 0 ? args[index + 1] : undefined;
  };
  const baselineDir = read("--baseline");
  const manifestPath = read("--manifest");
  if (!baselineDir || !manifestPath) {
    console.error("Usage: refresh-browser-visual-baseline-manifest.mjs --baseline <dir> --manifest <file>");
    process.exit(2);
  }
  const { visualQaScenarios, scenarioScreenshotName } = await import("./browser-visual-qa.mjs");
  const screenshots = visualQaScenarios.map(scenarioScreenshotName);
  refreshManifest({ baselineDir: resolve(baselineDir), manifestPath: resolve(manifestPath), screenshots });
  console.log(`Refreshed ${screenshots.length} baseline artifacts in ${manifestPath}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
```

- [ ] **Step 4: Register the test and the script**

In `package.json`:
- Append ` scripts/refresh-browser-visual-baseline-manifest.test.ts` to the file list inside `test:source-quality`.
- Add `"visual:qa:refresh-manifest": "node scripts/refresh-browser-visual-baseline-manifest.mjs"` after `visual:qa:baseline-policy`.

Do not change `verify:frontend` or `verify:release`, because `scripts/zero-debt-gate.test.ts` pins those strings.

- [ ] **Step 5: Verify**

Run: `rtk pnpm test:source-quality && rtk pnpm check:tooling-source`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add scripts/refresh-browser-visual-baseline-manifest.mjs scripts/refresh-browser-visual-baseline-manifest.test.ts package.json
rtk git commit -m "test(visual-qa): add a deterministic baseline manifest refresher"
```

---

### Task 3: Hard cut

This is one commit. The editor becomes a placeholder root, and all old editor code, tests, scenarios and baselines go away.

**Files:**
- Create: `src/editor/editor-root.tsx` (placeholder), `src/editor/editor-root.test.tsx`
- Delete: every remaining file in `src/components/workspace/`; the five old-layout `src/lib` modules and their tests; `src/browser-visual-qa-palmier-scenarios.test.ts`; 75 editor baseline PNGs per baseline folder
- Modify: `src/App.tsx`, `src/App.test.tsx`, `src/captions-workbench-layout.test.ts`, `scripts/browser-visual-qa.mjs`, `src/browser-visual-qa-script.test.ts`, `scripts/browser-visual-baseline-policy.mjs`, `src/browser-visual-baseline-policy-script.test.ts`, `scripts/palmier-visual-comparison.mjs`, both manifests, `e2e/app-smoke.spec.ts`, `knip.jsonc`

- [ ] **Step 1: Write the placeholder root test**

```tsx
// src/editor/editor-root.test.tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createSampleProject } from "@/lib/sample-project";
import { EditorRoot } from "./editor-root";

describe("EditorRoot", () => {
  it("renders the editor landmark with the project name and reports editor menu state", () => {
    const onNativeMenuStateChange = vi.fn();
    const onOpenProjectHome = vi.fn();
    render(
      <EditorRoot
        isActive
        projectDir="/tmp/project"
        initialProject={createSampleProject()}
        appPreferences={null}
        configurationRefreshId={0}
        transcriptionModelReady={false}
        runtimeReady
        nativeMenuRequest={null}
        onNativeMenuStateChange={onNativeMenuStateChange}
        onOpenProjectHome={onOpenProjectHome}
        onOpenModelSettings={vi.fn()}
        onOpenSettings={vi.fn()}
      />,
    );

    expect(screen.getByRole("main", { name: "Video editor workspace" })).toBeInTheDocument();
    expect(screen.getByText("Edison Restoration Demo")).toBeInTheDocument();
    expect(onNativeMenuStateChange).toHaveBeenCalledWith(expect.objectContaining({ view: "editor", canExport: false }));
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(onOpenProjectHome).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Read the current App contract**

Read `src/App.tsx` L685–727, the `<EditorWorkspace>` render. Record every prop name and type so the placeholder signature matches exactly:
- `isActive`, `transcriptionModelReady`, `runtimeReady`, `projectDir`, `initialProject`
- `nativeMenuRequest`, `onNativeMenuStateChange`
- `appPreferences`, `configurationRefreshId`
- `onOpenProjectHome`, `onOpenModelSettings`, `onOpenSettings`

`visualQaScenarioId` is removed. Read the argument types of `onOpenModelSettings` and `onOpenSettings` from their call sites in the old `EditorWorkspace` props interface before deleting it. Keep them identical, and adjust the test above if they take arguments.

- [ ] **Step 3: Implement the placeholder root**

```tsx
// src/editor/editor-root.tsx
import { House } from "lucide-react";
import { useEffect } from "react";
import type { AppSettingsPreferences } from "@/lib/app-settings";
import { inactiveNativeMenuState, type NativeMenuRequest, type NativeMenuState } from "@/lib/native-menu";
import type { VideoProject } from "@/lib/project";

export interface EditorRootProps {
  readonly isActive: boolean;
  readonly projectDir: string;
  readonly initialProject: VideoProject;
  readonly appPreferences: AppSettingsPreferences | null;
  readonly configurationRefreshId: number;
  readonly transcriptionModelReady: boolean;
  readonly runtimeReady: boolean;
  readonly nativeMenuRequest: NativeMenuRequest | null;
  readonly onNativeMenuStateChange: (state: NativeMenuState) => void;
  readonly onOpenProjectHome: () => void;
  readonly onOpenModelSettings: () => void;
  readonly onOpenSettings: () => void;
}

export function editorNativeMenuState(): NativeMenuState {
  return { ...inactiveNativeMenuState("home"), view: "editor" };
}

export function EditorRoot(props: EditorRootProps) {
  const { initialProject, isActive, onNativeMenuStateChange, onOpenProjectHome } = props;

  useEffect(() => {
    if (isActive) onNativeMenuStateChange(editorNativeMenuState());
  }, [isActive, onNativeMenuStateChange]);

  return (
    <main aria-label="Video editor workspace" className="flex h-full flex-col bg-background text-foreground">
      <header className="flex h-12 items-center gap-2 px-3">
        <button
          type="button"
          aria-label="Home"
          onClick={onOpenProjectHome}
          className="grid h-8 w-8 place-items-center rounded-md text-muted-foreground hover:bg-secondary hover:text-foreground"
        >
          <House className="h-4 w-4" aria-hidden />
        </button>
        <span className="font-semibold">{initialProject.name}</span>
      </header>
    </main>
  );
}
```

Match `AppSettingsPreferences | null` and the callback types to what Step 2 recorded. If `appPreferences` is non-nullable in App, drop the `| null` and pass a fixture value in the test instead.

- [ ] **Step 4: Delete the old editor UI and old-layout lib modules**

```bash
rtk git rm -r src/components/workspace
rtk git rm src/lib/workspace-layout-state.ts src/lib/workspace-layout-state.test.ts \
  src/lib/editor-layout-budget.ts src/lib/editor-layout-budget.test.ts \
  src/lib/responsive-rail-state.ts src/lib/responsive-rail-state.test.ts \
  src/lib/editor-information-architecture.ts src/lib/editor-information-architecture.test.ts \
  src/lib/modern-editor-visual-qa-fixtures.ts src/lib/modern-editor-visual-qa-fixtures.test.ts \
  src/browser-visual-qa-palmier-scenarios.test.ts
```

Before the second command, verify that each test file exists with `rtk ls src/lib/*layout* src/lib/responsive-rail-state* src/lib/editor-information-architecture* src/lib/modern-editor-visual-qa-fixtures*`. Drop missing names from the command.

- [ ] **Step 5: Rewire `src/App.tsx`**

- Replace L49–51:
  ```ts
  const EditorRoot = lazy(() => import("@/editor/editor-root").then((module) => ({ default: module.EditorRoot })));
  ```
- Replace the `<EditorWorkspace …>` element (L694–726) with `<EditorRoot …>`, passing the same props minus `visualQaScenarioId`.
- In `openSampleProject` (L439–502), delete the DEV visual-QA editor fixture block (L447–465) that imports `modern-editor-visual-qa-fixtures`. Keep the `settings-visual-qa-fixtures` import only if a remaining branch still uses it; confirm with `rtk rg -n "settingsVisualQa" src/App.tsx`.
- Delete the `type ModernEditorVisualQaFixture` import (L32), the `activeVisualQaFixture` state (L122–123), and every use of them.
- Keep `projectSessionKey`, the native menu wiring, `createEmptySplitProject` and all Settings/Health origins unchanged.

Run: `rtk pnpm lint`
Expected: no errors in `src/App.tsx`. Fix any remaining reference to deleted state.

- [ ] **Step 6: Update `src/App.test.tsx`**

- Change the mock target at L147 from `@/components/workspace/editor-workspace` to `@/editor/editor-root`.
- Export the mock component as `EditorRoot` instead of `EditorWorkspace`.
- Keep the mock's `aria-label="Mock editor workspace"` and its buttons ("Session count", "Home", "Editor model settings", "Configure provider") so the existing App tests keep their selectors.
- Delete the test at L424 that exercises the visual-QA editor fixture, and any assertion on `visualQaScenarioId`.

Run: `rtk pnpm vitest run src/App.test.tsx`
Expected: PASS.

- [ ] **Step 7: Trim `src/captions-workbench-layout.test.ts`**

- Delete the `readFileSync` reads of `media-bin.tsx` and `editor-workspace.tsx` (L6–9) and the string assertions over them (L22–28).
- Keep the CSS and Tailwind checks (L14–20) only if the selectors they assert still exist in `src/index.css`. Otherwise delete the file.
- Task 4 replaces the editor CSS, so if the checks target `.editor-template-grid` or `.editor-generation-footer-grid`, delete the whole file now:

```bash
rtk git rm src/captions-workbench-layout.test.ts
```

- [ ] **Step 8: Remove editor scenarios from `scripts/browser-visual-qa.mjs`**

Delete these editor-only ranges, re-locating each by function name first:

| Lines | What |
|---|---|
| 16–17 | Storage key constants |
| 108–154 | `modernEditorVisualQaScenarios` |
| 203–221, 223–236 | Editor entries in `visualQaScenarios`, keeping the Home entries at 199–202 and 222 and the Settings spread at 237 |
| 499–643 | Editor geometry helpers |
| 653–685 | Editor branches of `palmierGeometryAssertions`, keeping the Home branch |
| 689–1324 | Modern-editor and timeline geometry |
| 1638–1681, 1683–1774 | `prepareSurface`: delete the editor branches, keeping `home` returning `""` |
| 1786–2945 | Editor fixture and interaction helpers |
| 2973–3149 | Editor cleanups and capture helpers |

Also make these edits:
- In `runScenario` (3164–3243), remove the editor surface branches.
- In `main` (3301–3445), remove the `responsiveRailStorageKey` storage cleanup at 3394 and the editor-only sample open at 3405–3415, but only if no remaining scenario needs it.
- Keep `waitForSurface` (1326–1345) with its editor keys removed, except `main[aria-label='Video editor workspace']`. The two Settings project-context fixtures still open the sample project and wait for that landmark, and the placeholder root keeps it.
- Update the help text at 314 and 316–326 to list only Home and Settings captures.

Run:
```bash
rtk node --check scripts/browser-visual-qa.mjs
rtk node -e 'import("./scripts/browser-visual-qa.mjs").then(m=>console.log(m.visualQaScenarios.map(m.scenarioScreenshotName)))'
```
Expected: exactly the 7 non-editor screenshots: `home-desktop.png`, `home-narrow.png`, `home-missing-recent-desktop.png`, `home-missing-recent-narrow.png`, `home-palmier-desktop.png`, `settings-desktop.png`, `settings-narrow.png`.

- [ ] **Step 9: Update script tests and the policy help text**

- **`src/browser-visual-qa-script.test.ts`:** delete the editor sections (L86–268 and L270–326). Keep L18–84.
- **`scripts/browser-visual-baseline-policy.mjs` L86:** change "82-shot" to "7-shot".
- **`src/browser-visual-baseline-policy-script.test.ts`:**
  - Set `expectedScreenshotCount = 7` at L27.
  - Replace the editor filenames used as fixtures (L67, 84, 124, 172, 211, 298, 319, 330, 347) with `home-desktop.png`, `home-narrow.png`, `settings-desktop.png` or `settings-narrow.png`. Keep each test's intent: for example, the missing-file test still removes one required shot, now `settings-narrow.png`.
- **`scripts/palmier-visual-comparison.mjs` L6–16:** keep only the Home pair (L7).

Run: `rtk pnpm vitest run src/browser-visual-qa-script.test.ts src/browser-visual-baseline-policy-script.test.ts src/palmier-visual-comparison-script.test.ts src/settings-visual-qa-scenarios.test.ts`
Expected: PASS.

- [ ] **Step 10: Remove editor baselines and refresh manifests**

```bash
rtk node -e '
const fs=require("fs");
const keep=new Set(["home-desktop.png","home-narrow.png","home-missing-recent-desktop.png","home-missing-recent-narrow.png","home-palmier-desktop.png","settings-desktop.png","settings-narrow.png"]);
for (const dir of ["docs/visual-qa/browser-visual-baseline","docs/visual-qa/browser-visual-baseline-linux-x64"]) {
  for (const f of fs.readdirSync(dir)) if (f.endsWith(".png") && !keep.has(f)) fs.rmSync(`${dir}/${f}`);
  console.log(dir, fs.readdirSync(dir).filter(f=>f.endsWith(".png")).length);
}'
rtk pnpm visual:qa:refresh-manifest -- --baseline docs/visual-qa/browser-visual-baseline --manifest docs/visual-qa/browser-visual-baseline-manifest.json
rtk pnpm visual:qa:refresh-manifest -- --baseline docs/visual-qa/browser-visual-baseline-linux-x64 --manifest docs/visual-qa/browser-visual-baseline-linux-x64-manifest.json
rtk git diff --stat docs/visual-qa/*.json
```
Expected:
- Each folder prints 7.
- The manifest diffs remove 75 entries per file.
- The 7 kept artifact hashes are unchanged, since the kept PNGs were not modified.

- [ ] **Step 11: Update the app smoke e2e**

In `e2e/app-smoke.spec.ts`:
- Replace the editor section (L66–100) with an assertion that `page.getByRole("main", { name: "Video editor workspace" })` is visible after "Open sample project".
- Remove the preview and video assertions and the `editor-*.png` screenshots.
- Rename the tests at L106 and L112 from "… and editor" to "… and editor shell".

`scripts/playwright-cli.test.ts:36` still matches `/Video editor workspace/`.

- [ ] **Step 12: Record temporarily consumer-less backend exports in knip**

Run: `rtk pnpm check:unused`
Expected: FAIL. It lists unused exports, mostly in `src/lib/project.ts`, and files with no importer: `src/components/ui/separator.tsx`, `src/components/ui/textarea.tsx` and `src/lib/generation-variations.ts`.

Add a scoped ignore to `knip.jsonc` for exactly those reported symbols and files. Use knip's `ignoreIssues` map, or `ignore` for files, following knip 6 config syntax; confirm with `rtk pnpm exec knip --help`. Put this comment above it:

```jsonc
// Editor redesign: backend contract exports and primitives whose UI consumers were
// deleted in plan 02 and are restored in plans 03–07. Each plan removes the entries it
// consumes; plan 09 asserts this block is empty.
```

Run: `rtk pnpm check:unused`
Expected: PASS.

- [ ] **Step 13: Run the full frontend gate**

Run: `rtk pnpm verify:frontend`
Expected: PASS. `visual:qa:browser-release` compares the 7 kept shots against unchanged baselines. The Settings project-context scenarios are not in the default baseline.

Scripts outside the gate that are now non-functional until later plans restore the accessible names:
- `scripts/preview-playback-performance.mjs` (plan 04)
- `scripts/browser-preview-render-qa.mjs` (plan 04)
- `scripts/linux-desktop-smoke.mjs` (plan 09)

Their tests assert only script strings and keep passing.

- [ ] **Step 14: Commit**

```bash
rtk git add -A src/components/workspace src/lib src/editor src/App.tsx src/App.test.tsx src/captions-workbench-layout.test.ts src/browser-visual-qa-palmier-scenarios.test.ts scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts scripts/browser-visual-baseline-policy.mjs src/browser-visual-baseline-policy-script.test.ts scripts/palmier-visual-comparison.mjs docs/visual-qa e2e/app-smoke.spec.ts knip.jsonc
rtk git status --short
rtk git commit -m "refactor(editor)!: remove the legacy editor workspace ahead of the redesign

BREAKING CHANGE: the editor view is a placeholder shell until the redesign plans land."
```

Before committing, confirm `rtk git status --short` shows no unrelated files staged.

---

### Task 4: Global CapCut-dark tokens

**Files:**
- Modify: `src/index.css`, `tailwind.config.ts`
- Modify: the 7 PNGs and both manifests under `docs/visual-qa/`

- [ ] **Step 1: Replace `src/index.css`**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    /* shadcn semantic tokens (Home and Settings consume these) */
    --background: 228 11% 7%;
    --foreground: 228 7% 92%;
    --card: 228 9% 10%;
    --card-foreground: 228 7% 92%;
    --popover: 230 9% 13%;
    --popover-foreground: 228 7% 92%;
    --primary: 184 75% 48%;
    --primary-foreground: 185 83% 9%;
    --secondary: 230 9% 14%;
    --secondary-foreground: 228 7% 92%;
    --muted: 230 9% 14%;
    --muted-foreground: 222 6% 57%;
    --accent: 184 75% 48%;
    --accent-foreground: 185 83% 9%;
    --destructive: 0 100% 71%;
    --destructive-foreground: 228 11% 7%;
    --border: 228 8% 16%;
    --input: 230 9% 14%;
    --ring: 184 75% 48%;
    --radius: 0.5rem;

    /* editor surface tokens */
    --panel: 228 9% 10%;
    --raised: 230 9% 14%;
    --hover: 234 9% 18%;
    --line: 228 8% 16%;
    --dim: 222 6% 39%;
    --accent-soft: 186 65% 16%;
    --warning: 41 89% 66%;
    --success: 154 67% 62%;
    --keyframe: 49 100% 65%;
    --clip-video: 209 49% 36%;
    --clip-text: 257 61% 59%;
    --clip-caption: 36 65% 48%;
    --clip-audio: 166 57% 32%;
    --clip-graphics: 331 45% 50%;
  }

  * {
    @apply border-border;
  }

  html,
  body,
  #root {
    height: 100%;
  }

  body {
    @apply bg-background text-foreground antialiased;
    margin: 0;
    font-size: 13px;
    font-family:
      Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
      sans-serif;
  }
}

@layer utilities {
  .tabular-time {
    font-variant-numeric: tabular-nums;
  }
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

These HSL values are the spec hex tokens. For example, `#0f1013` is `228 11% 7%` and `#1fc7d4` is `184 75% 48%`. Before writing, verify every conversion with this one-liner and fix any value whose rounded hex differs by more than 1 per channel:

```bash
rtk node -e 'const h=(hx)=>{let r=parseInt(hx.slice(1,3),16)/255,g=parseInt(hx.slice(3,5),16)/255,b=parseInt(hx.slice(5,7),16)/255;const M=Math.max(r,g,b),m=Math.min(r,g,b),l=(M+m)/2;let H=0,S=0;if(M!==m){const d=M-m;S=l>.5?d/(2-M-m):d/(M+m);H=M===r?(g-b)/d+(g<b?6:0):M===g?(b-r)/d+2:(r-g)/d+4;H*=60}return `${Math.round(H)} ${Math.round(S*100)}% ${Math.round(l*100)}%`};for(const [n,x] of Object.entries({bg:"#0f1013",panel:"#17181c",raised:"#202127",hover:"#2a2b32",line:"#26282e",text:"#e9eaed",muted:"#8b8f98",dim:"#5d616a",accent:"#1fc7d4",accentSoft:"#0e3d42",destructive:"#ff6b6b",warning:"#f5c45b",success:"#5ee0a8",keyframe:"#ffd84a",video:"#2f5d8a",text2:"#7a55d6",caption:"#c98a2b",audio:"#23806c",graphics:"#b8467a"}))console.log(n,h(x))'
```

The old `.editor-*` and `.timeline-grid` classes are deleted because their only consumers were removed in Task 3. Confirm with `rtk rg -n "editor-shell|editor-source-container|editor-template-grid|editor-generation-footer-grid|editor-rail-focus|editor-drawer|editor-drop-target|timeline-grid" src`. Expected: no output.

- [ ] **Step 2: Extend `tailwind.config.ts`**

Add the following to `theme.extend.colors`, next to the existing entries:

```ts
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        panel: "hsl(var(--panel))",
        raised: "hsl(var(--raised))",
        hover: "hsl(var(--hover))",
        line: "hsl(var(--line))",
        dim: "hsl(var(--dim))",
        "accent-soft": "hsl(var(--accent-soft))",
        warning: "hsl(var(--warning))",
        success: "hsl(var(--success))",
        keyframe: "hsl(var(--keyframe))",
        clip: {
          video: "hsl(var(--clip-video))",
          text: "hsl(var(--clip-text))",
          caption: "hsl(var(--clip-caption))",
          audio: "hsl(var(--clip-audio))",
          graphics: "hsl(var(--clip-graphics))",
        },
```

Also add the following to `theme.extend.borderRadius`:

```ts
        panel: "10px",
        control: "8px",
        clip: "6px",
```

- [ ] **Step 3: Check Home and Settings in the browser**

Run: `rtk pnpm visual:qa:browser`
Expected: the command completes and writes the 7 screenshots to `output/playwright/browser-visual-qa/`.

Open each PNG with the Read tool. Check for:
- unreadable contrast (text on `--primary` backgrounds now uses the dark `--primary-foreground`),
- invisible borders,
- broken Settings status colors.

Fix regressions with token-based classes in the affected Settings or Home component. Do not change layouts.

- [ ] **Step 4: Refresh the Linux baseline**

```bash
rtk cp output/playwright/browser-visual-qa/{home-desktop,home-narrow,home-missing-recent-desktop,home-missing-recent-narrow,home-palmier-desktop,settings-desktop,settings-narrow}.png docs/visual-qa/browser-visual-baseline-linux-x64/
rtk pnpm visual:qa:refresh-manifest -- --baseline docs/visual-qa/browser-visual-baseline-linux-x64 --manifest docs/visual-qa/browser-visual-baseline-linux-x64-manifest.json
```

Confirm the screenshot filenames in the output folder match `scenarioScreenshotName` output. If the script writes into a subfolder, copy from there.

- [ ] **Step 5: Refresh the macOS baseline on a macOS host**

On an Apple Silicon Mac at this commit, run `rtk pnpm visual:qa:browser`. Then copy the 7 PNGs into `docs/visual-qa/browser-visual-baseline/` and run the refresher with the macOS manifest.

If no macOS host is available in this session:
- Stop after committing Steps 1–4.
- Record "macOS browser baseline refresh pending" in the commit body.
- Leave this step unchecked. `verify:frontend` on macOS fails until it is done. Linux `verify:frontend` passes.

- [ ] **Step 6: Verify**

Run: `rtk pnpm verify:frontend`
Expected: PASS on Linux.

- [ ] **Step 7: Commit**

```bash
rtk git add src/index.css tailwind.config.ts docs/visual-qa src/components/settings src/components/home
rtk git commit -m "feat(ui): adopt the CapCut-dark global design tokens"
```

---

### Task 5: Editor store — project slice

**Files:**
- Create: `src/editor/store/editor-store.ts`, `src/editor/store/project-slice.ts`, `src/editor/store/project-slice.test.ts`
- Modify: `package.json`, `pnpm-lock.yaml`

- [ ] **Step 1: Add zustand**

Run: `rtk pnpm add zustand@5.0.15`
Expected: `dependencies.zustand` is `5.0.15` or `^5.0.15`. Pin it to exactly `5.0.15` in `package.json` to match the repository's exact-pin style for runtime-critical packages.

- [ ] **Step 2: Write the failing test**

```ts
// src/editor/store/project-slice.test.ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("./editor-store");

function splitProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: 3 };
}

function opacityAction(project: VideoProject): ProjectAction {
  return { type: "updateVisualClipOpacity", itemId: fixtureItem(project, "video").id, opacity: 0.5 } as ProjectAction;
}

describe("project slice", () => {
  beforeEach(() => backendRequest.mockReset());
  afterEach(() => vi.useRealTimers());

  it("persists split-project actions through the backend and records one undo step", async () => {
    const project = splitProject();
    const saved = { ...project, contentRevision: 4, name: "saved" };
    backendRequest.mockResolvedValueOnce({ project: saved });
    const store = createEditorStore({ projectDir: "/p", project });

    const result = await store.getState().applyActions([opacityAction(project)]);

    expect(backendRequest).toHaveBeenCalledWith("apply_project_actions_to_split_project_folder", {
      projectDir: "/p",
      actions: [opacityAction(project)],
    });
    expect(result).toEqual(saved);
    expect(store.getState().project).toEqual(saved);
    expect(store.getState().saveStatus).toBe("saved");
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().canUndo()).toBe(true);
  });

  it("falls back to local application when the backend is unavailable", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValueOnce(new BackendUnavailableError());
    const store = createEditorStore({ projectDir: "/p", project });

    const result = await store.getState().applyActions([opacityAction(project)]);

    expect(result).not.toBeNull();
    expect(result).not.toBe(project);
    expect(store.getState().saveStatus).toBe("unsaved");
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("does not record undo steps for job bookkeeping actions", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValueOnce(new BackendUnavailableError());
    const store = createEditorStore({ projectDir: "/p", project });
    const job = { id: "job-1", kind: "render_draft", status: "queued", updatedAt: "2026-09-13T00:00:00Z" };

    await store.getState().applyActions([{ type: "recordJob", job } as ProjectAction]);

    expect(store.getState().history.past).toHaveLength(0);
  });

  it("reports failure without mutating when the backend rejects the action", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValueOnce(new Error("track is locked"));
    const store = createEditorStore({ projectDir: "/p", project });

    const result = await store.getState().applyActions([opacityAction(project)]);

    expect(result).toBeNull();
    expect(store.getState().project).toBe(project);
    expect(store.getState().saveStatus).toBe("failed");
    expect(store.getState().lastError).toBe("track is locked");
  });

  it("undoes and redoes by persisting snapshots with the expected revision", async () => {
    const project = splitProject();
    const edited = { ...project, contentRevision: 4, name: "edited" };
    const restored = { ...project, contentRevision: 5 };
    const redone = { ...edited, contentRevision: 6 };
    backendRequest
      .mockResolvedValueOnce({ project: edited })
      .mockResolvedValueOnce({ project: restored })
      .mockResolvedValueOnce({ project: redone });
    const store = createEditorStore({ projectDir: "/p", project });

    await store.getState().applyActions([opacityAction(project)]);
    await store.getState().undo();

    expect(backendRequest).toHaveBeenLastCalledWith("save_split_project_to_folder", {
      projectDir: "/p",
      project,
      expectedRevision: 4,
    });
    expect(store.getState().project).toEqual(restored);
    expect(store.getState().canRedo()).toBe(true);

    await store.getState().redo();
    expect(store.getState().project).toEqual(redone);
    expect(store.getState().canRedo()).toBe(false);
  });

  it("caps undo history at 100 snapshots", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    const store = createEditorStore({ projectDir: "/p", project });
    for (let index = 0; index < 105; index += 1) {
      const current = store.getState().project;
      await store.getState().applyActions([
        { type: "updateVisualClipOpacity", itemId: fixtureItem(current, "video").id, opacity: (index % 10) / 10 } as ProjectAction,
      ]);
    }
    expect(store.getState().history.past.length).toBeLessThanOrEqual(100);
  });

  it("serializes concurrent writes so each uses the latest project", async () => {
    const project = splitProject();
    let resolveFirst: (value: unknown) => void = () => undefined;
    backendRequest
      .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; }))
      .mockResolvedValueOnce({ project: { ...project, contentRevision: 5, name: "second" } });
    const store = createEditorStore({ projectDir: "/p", project });

    const first = store.getState().applyActions([opacityAction(project)]);
    const second = store.getState().applyActions([opacityAction(project)]);
    expect(backendRequest).toHaveBeenCalledTimes(1);
    resolveFirst({ project: { ...project, contentRevision: 4, name: "first" } });
    await first;
    await second;

    expect(backendRequest).toHaveBeenCalledTimes(2);
    expect(store.getState().project.name).toBe("second");
    expect(store.getState().history.past).toHaveLength(2);
  });
});
```

Check the exact field names of `updateVisualClipOpacity` and `recordJob` in `src/lib/project.ts` (ProjectAction union L593–894) and adjust the literals. The `as ProjectAction` casts are allowed only in tests.

- [ ] **Step 3: Run to verify it fails**

Run: `rtk pnpm vitest run src/editor/store/project-slice.test.ts`
Expected: FAIL with "Cannot find module './editor-store'".

- [ ] **Step 4: Implement the project slice**

```ts
// src/editor/store/project-slice.ts
import { applyProjectActionsLocally } from "@/lib/agent/project-merge";
import {
  applyProjectActionsToSplitProjectFolder,
  saveSplitProjectToFolder,
  type ProjectAction,
  type VideoProject,
} from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { EditorSliceCreator } from "./editor-store";

export const maxUndoSnapshots = 100;

const nonUndoableActionTypes: ReadonlySet<ProjectAction["type"]> = new Set<ProjectAction["type"]>([
  "recordJob",
  "updateJobStatus",
  "updateJobProviderRequest",
  "attachRenderReport",
  "recordExportArtifact",
  "recordGeneratedAsset",
  "updateGeneratedAssetStatus",
]);

export type SaveStatus = "saved" | "saving" | "unsaved" | "failed";

export interface ProjectHistory {
  readonly past: readonly VideoProject[];
  readonly future: readonly VideoProject[];
}

export interface ApplyActionsOptions {
  /** Force-disable history recording (e.g. agent batches recorded elsewhere). */
  readonly recordHistory?: boolean;
}

export interface ProjectSlice {
  readonly projectDir: string;
  readonly project: VideoProject;
  readonly saveStatus: SaveStatus;
  readonly lastError: string | null;
  readonly history: ProjectHistory;
  applyActions(actions: readonly ProjectAction[], options?: ApplyActionsOptions): Promise<VideoProject | null>;
  undo(): Promise<void>;
  redo(): Promise<void>;
  canUndo(): boolean;
  canRedo(): boolean;
  replaceProject(project: VideoProject): void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function usesSplitFolder(project: VideoProject, projectDir: string): boolean {
  return project.schemaVersion >= 2 && projectDir.trim().length > 0;
}

function shouldRecord(actions: readonly ProjectAction[], options?: ApplyActionsOptions): boolean {
  if (options?.recordHistory === false) return false;
  return actions.some((action) => !nonUndoableActionTypes.has(action.type));
}

function pushPast(history: ProjectHistory, snapshot: VideoProject): ProjectHistory {
  const past = [...history.past, structuredClone(snapshot)];
  return { past: past.slice(Math.max(0, past.length - maxUndoSnapshots)), future: [] };
}

export function createProjectSlice(init: { projectDir: string; project: VideoProject }): EditorSliceCreator<ProjectSlice> {
  return (set, get) => {
    let writeQueue: Promise<unknown> = Promise.resolve();

    function enqueue<T>(work: () => Promise<T>): Promise<T> {
      const next = writeQueue.then(work, work);
      writeQueue = next.catch(() => undefined);
      return next;
    }

    async function persistSnapshot(snapshot: VideoProject, expectedRevision: number): Promise<VideoProject> {
      const { projectDir } = get();
      if (!usesSplitFolder(snapshot, projectDir)) return snapshot;
      try {
        const result = await saveSplitProjectToFolder({ projectDir, project: snapshot, expectedRevision });
        return result.project;
      } catch (error) {
        if (isBackendUnavailableError(error)) return snapshot;
        throw error;
      }
    }

    return {
      projectDir: init.projectDir,
      project: init.project,
      saveStatus: "saved",
      lastError: null,
      history: { past: [], future: [] },

      applyActions(actions, options) {
        return enqueue(async () => {
          const base = get().project;
          const { projectDir } = get();
          const record = shouldRecord(actions, options);
          set({ lastError: null });
          try {
            if (usesSplitFolder(base, projectDir)) {
              set({ saveStatus: "saving" });
              const result = await applyProjectActionsToSplitProjectFolder({ projectDir, actions: [...actions] });
              set((state) => ({
                project: result.project,
                saveStatus: "saved",
                history: record ? pushPast(state.history, base) : state.history,
              }));
              return result.project;
            }
            throw new LocalOnlyProject();
          } catch (error) {
            if (error instanceof LocalOnlyProject || isBackendUnavailableError(error)) {
              const next = applyProjectActionsLocally(base, [...actions]);
              if (next === base) {
                set({ saveStatus: "failed", lastError: "The edit could not be applied." });
                return null;
              }
              set((state) => ({
                project: next,
                saveStatus: "unsaved",
                history: record ? pushPast(state.history, base) : state.history,
              }));
              return next;
            }
            set({ saveStatus: "failed", lastError: errorMessage(error) });
            return null;
          }
        });
      },

      undo() {
        return enqueue(async () => {
          const { history, project } = get();
          const previous = history.past.at(-1);
          if (!previous) return;
          try {
            const restored = await persistSnapshot(previous, project.contentRevision ?? 0);
            set({
              project: restored,
              history: { past: history.past.slice(0, -1), future: [structuredClone(project), ...history.future] },
              lastError: null,
            });
          } catch (error) {
            set({ saveStatus: "failed", lastError: errorMessage(error) });
          }
        });
      },

      redo() {
        return enqueue(async () => {
          const { history, project } = get();
          const next = history.future[0];
          if (!next) return;
          try {
            const restored = await persistSnapshot(next, project.contentRevision ?? 0);
            set({
              project: restored,
              history: { past: [...history.past, structuredClone(project)], future: history.future.slice(1) },
              lastError: null,
            });
          } catch (error) {
            set({ saveStatus: "failed", lastError: errorMessage(error) });
          }
        });
      },

      canUndo: () => get().history.past.length > 0,
      canRedo: () => get().history.future.length > 0,
      replaceProject: (project) => set({ project }),
    };
  };
}

class LocalOnlyProject extends Error {}
```

Import notes:
- `applyProjectActionsLocally` comes from Plan 01 Task 12 (`src/lib/agent/project-merge.ts`). If Plan 01 named it differently, use that name.
- The generated-asset action type names must exist in the `ProjectAction` union. Remove any that do not.

- [ ] **Step 5: Implement the store factory**

This first version composes only the project slice. Task 6 adds the other slices.

```ts
// src/editor/store/editor-store.ts
import { createStore, type StoreApi } from "zustand/vanilla";
import type { VideoProject } from "@/lib/project";
import { createProjectSlice, type ProjectSlice } from "./project-slice";

export type EditorState = ProjectSlice;

export type EditorSliceCreator<T> = (
  set: StoreApi<EditorState>["setState"],
  get: StoreApi<EditorState>["getState"],
) => T;

export interface EditorStoreInit {
  readonly projectDir: string;
  readonly project: VideoProject;
}

export type EditorStore = StoreApi<EditorState>;

export function createEditorStore(init: EditorStoreInit): EditorStore {
  return createStore<EditorState>()((set, get) => ({
    ...createProjectSlice(init)(set, get),
  }));
}
```

- [ ] **Step 6: Run to verify it passes**

Run: `rtk pnpm vitest run src/editor/store/project-slice.test.ts && rtk pnpm lint && rtk pnpm check:unused`
Expected:
- The tests PASS.
- knip may report `createEditorStore` as used only by tests. That is acceptable, because test files are knip entries in this repo.
- knip may also report that `zustand` is used only in tests. If it does, make `src/editor/editor-root.tsx` import `createEditorStore` now: create the store with `useState(() => createEditorStore({ projectDir, project: initialProject }))` and read the project name from it.

- [ ] **Step 7: Commit**

```bash
rtk git add package.json pnpm-lock.yaml src/editor/store/editor-store.ts src/editor/store/project-slice.ts src/editor/store/project-slice.test.ts src/editor/editor-root.tsx
rtk git commit -m "feat(editor): add the zustand editor store with persisted project history"
```

---

### Task 6: Selection, playback, UI and timeline-view slices plus provider

**Files:**
- Create: `src/editor/store/selection-slice.ts`, `playback-slice.ts`, `ui-slice.ts`, `timeline-view-slice.ts`, `persisted-layout.ts`, `persisted-layout.test.ts`, `slices.test.ts`, `editor-store-context.tsx`
- Modify: `src/editor/store/editor-store.ts`

- [ ] **Step 1: Write failing tests**

```ts
// src/editor/store/persisted-layout.test.ts
import { beforeEach, describe, expect, it } from "vitest";
import { loadEditorLayout, loadTimelineView, saveEditorLayout, saveTimelineView } from "./persisted-layout";

describe("persisted layout", () => {
  beforeEach(() => window.localStorage.clear());

  it("returns defaults and clamps stored values", () => {
    expect(loadEditorLayout()).toEqual({ activeTab: "ai", leftWidth: 360 });
    window.localStorage.setItem("video-creater.editor.v2.layout", JSON.stringify({ activeTab: "nope", leftWidth: 9999 }));
    expect(loadEditorLayout()).toEqual({ activeTab: "ai", leftWidth: 440 });
  });

  it("round-trips layout and per-project timeline view", () => {
    saveEditorLayout({ activeTab: "captions", leftWidth: 320 });
    expect(loadEditorLayout()).toEqual({ activeTab: "captions", leftWidth: 320 });
    saveTimelineView("/p", { splitHeight: 280, zoomPercent: 150, snapEnabled: false, keyframesVisible: true });
    expect(loadTimelineView("/p")).toEqual({ splitHeight: 280, zoomPercent: 150, snapEnabled: false, keyframesVisible: true });
    expect(loadTimelineView("/other")).toEqual({ splitHeight: 300, zoomPercent: 100, snapEnabled: true, keyframesVisible: false });
  });

  it("ignores corrupt storage", () => {
    window.localStorage.setItem("video-creater.editor.v2.layout", "{");
    expect(loadEditorLayout()).toEqual({ activeTab: "ai", leftWidth: 360 });
  });
});
```

```ts
// src/editor/store/slices.test.ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: vi.fn(),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("./editor-store");

describe("editor slices", () => {
  beforeEach(() => window.localStorage.clear());

  it("selects, toggles, and clears timeline items", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    const id = fixtureItem(project, "video").id;
    store.getState().selectItems([id]);
    expect(store.getState().selectedItemIds).toEqual([id]);
    expect(store.getState().hasSelection()).toBe(true);
    store.getState().toggleItemSelection(id);
    expect(store.getState().selectedItemIds).toEqual([]);
    store.getState().selectItems([id]);
    store.getState().clearSelection();
    expect(store.getState().hasSelection()).toBe(false);
  });

  it("drops selected ids that no longer exist after a project replacement", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().selectItems(["missing", fixtureItem(project, "video").id]);
    store.getState().replaceProject(project);
    expect(store.getState().selectedItemIds).toEqual([fixtureItem(project, "video").id]);
  });

  it("clamps the playhead to the timeline duration and switches preview source", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().seek(-5);
    expect(store.getState().playheadSeconds).toBe(0);
    store.getState().seek(10_000);
    expect(store.getState().playheadSeconds).toBe(project.timeline.durationSeconds);
    store.getState().previewAsset("media-1");
    expect(store.getState().previewSource).toEqual({ kind: "asset", mediaId: "media-1" });
    store.getState().previewTimeline();
    expect(store.getState().previewSource).toEqual({ kind: "timeline" });
  });

  it("persists active tab and left width, and clamps width", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().setActiveTab("effects");
    store.getState().setLeftWidth(100);
    expect(store.getState().activeTab).toBe("effects");
    expect(store.getState().leftWidth).toBe(300);
    const reopened = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    expect(reopened.getState().activeTab).toBe("effects");
  });

  it("opens one mobile sheet at a time", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().openSheet("media");
    store.getState().openSheet("ai");
    expect(store.getState().openSheetId).toBe("ai");
    store.getState().closeSheet();
    expect(store.getState().openSheetId).toBeNull();
  });

  it("clamps and persists the timeline split height per project", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().setSplitHeight(10);
    expect(store.getState().splitHeight).toBe(160);
    store.getState().setZoomPercent(5000);
    expect(store.getState().zoomPercent).toBe(1000);
    const reopened = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    expect(reopened.getState().splitHeight).toBe(160);
  });
});
```

If `project.timeline.durationSeconds` is named differently in `VideoProject`, adjust.

- [ ] **Step 2: Run to verify they fail**

Run: `rtk pnpm vitest run src/editor/store`
Expected: FAIL. The modules are missing, and `selectItems` is not a function.

- [ ] **Step 3: Implement persisted layout**

```ts
// src/editor/store/persisted-layout.ts
export const editorTabIds = ["ai", "media", "audio", "text", "captions", "effects"] as const;
export type EditorTabId = (typeof editorTabIds)[number];

export interface EditorLayout {
  readonly activeTab: EditorTabId;
  readonly leftWidth: number;
}

export interface TimelineViewPreferences {
  readonly splitHeight: number;
  readonly zoomPercent: number;
  readonly snapEnabled: boolean;
  readonly keyframesVisible: boolean;
}

export const leftWidthBounds = { min: 300, max: 440, default: 360 } as const;
export const splitHeightBounds = { min: 160, default: 300 } as const;
export const zoomBounds = { min: 10, max: 1000, default: 100 } as const;

const layoutKey = "video-creater.editor.v2.layout";
const timelineViewKey = (projectDir: string) => `video-creater.editor.v2.timeline-view:${projectDir}`;

export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, value));
}

function readJson(key: string): Record<string, unknown> | null {
  try {
    const raw = window.localStorage.getItem(key);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage full or unavailable: layout persistence is best-effort.
  }
}

function isTabId(value: unknown): value is EditorTabId {
  return typeof value === "string" && (editorTabIds as readonly string[]).includes(value);
}

export function loadEditorLayout(): EditorLayout {
  const stored = readJson(layoutKey);
  return {
    activeTab: isTabId(stored?.activeTab) ? stored.activeTab : "ai",
    leftWidth:
      typeof stored?.leftWidth === "number"
        ? clamp(stored.leftWidth, leftWidthBounds.min, leftWidthBounds.max)
        : leftWidthBounds.default,
  };
}

export function saveEditorLayout(layout: EditorLayout): void {
  writeJson(layoutKey, layout);
}

export function loadTimelineView(projectDir: string): TimelineViewPreferences {
  const stored = readJson(timelineViewKey(projectDir));
  return {
    splitHeight: typeof stored?.splitHeight === "number" ? Math.max(splitHeightBounds.min, stored.splitHeight) : splitHeightBounds.default,
    zoomPercent:
      typeof stored?.zoomPercent === "number" ? clamp(stored.zoomPercent, zoomBounds.min, zoomBounds.max) : zoomBounds.default,
    snapEnabled: typeof stored?.snapEnabled === "boolean" ? stored.snapEnabled : true,
    keyframesVisible: typeof stored?.keyframesVisible === "boolean" ? stored.keyframesVisible : false,
  };
}

export function saveTimelineView(projectDir: string, view: TimelineViewPreferences): void {
  writeJson(timelineViewKey(projectDir), view);
}
```

- [ ] **Step 4: Implement the slices**

```ts
// src/editor/store/selection-slice.ts
import type { VideoProject } from "@/lib/project";
import type { EditorSliceCreator } from "./editor-store";

export interface HighlightRange {
  readonly trackIds: readonly string[];
  readonly startSeconds: number;
  readonly endSeconds: number;
}

export interface SelectionSlice {
  readonly selectedItemIds: readonly string[];
  readonly highlightedItemIds: readonly string[];
  readonly highlightedRanges: readonly HighlightRange[];
  selectItems(itemIds: readonly string[]): void;
  toggleItemSelection(itemId: string): void;
  clearSelection(): void;
  hasSelection(): boolean;
  setHighlights(itemIds: readonly string[], ranges: readonly HighlightRange[]): void;
  pruneSelection(project: VideoProject): void;
}

export function projectItemIds(project: VideoProject): Set<string> {
  const ids = new Set<string>();
  const timelines = project.timelines?.map((entry) => entry.timeline) ?? [project.timeline];
  for (const timeline of timelines) {
    for (const track of timeline.tracks) {
      for (const item of track.items) ids.add(item.id);
    }
  }
  return ids;
}

export const createSelectionSlice: EditorSliceCreator<SelectionSlice> = (set, get) => ({
  selectedItemIds: [],
  highlightedItemIds: [],
  highlightedRanges: [],
  selectItems: (itemIds) => set({ selectedItemIds: [...new Set(itemIds)] }),
  toggleItemSelection: (itemId) =>
    set((state) => ({
      selectedItemIds: state.selectedItemIds.includes(itemId)
        ? state.selectedItemIds.filter((id) => id !== itemId)
        : [...state.selectedItemIds, itemId],
    })),
  clearSelection: () => set({ selectedItemIds: [] }),
  hasSelection: () => get().selectedItemIds.length > 0,
  setHighlights: (itemIds, ranges) => set({ highlightedItemIds: [...itemIds], highlightedRanges: [...ranges] }),
  pruneSelection: (project) => {
    const ids = projectItemIds(project);
    set((state) => ({ selectedItemIds: state.selectedItemIds.filter((id) => ids.has(id)) }));
  },
});
```

```ts
// src/editor/store/playback-slice.ts
import type { EditorSliceCreator } from "./editor-store";

export type PreviewSourceSelection = { readonly kind: "timeline" } | { readonly kind: "asset"; readonly mediaId: string };

export interface PlaybackSlice {
  readonly playheadSeconds: number;
  readonly playing: boolean;
  readonly previewSource: PreviewSourceSelection;
  readonly fullscreen: boolean;
  seek(seconds: number): void;
  setPlaying(playing: boolean): void;
  togglePlaying(): void;
  previewAsset(mediaId: string): void;
  previewTimeline(): void;
  setFullscreen(fullscreen: boolean): void;
}

export const createPlaybackSlice: EditorSliceCreator<PlaybackSlice> = (set, get) => ({
  playheadSeconds: 0,
  playing: false,
  previewSource: { kind: "timeline" },
  fullscreen: false,
  seek: (seconds) => {
    const duration = get().project.timeline.durationSeconds;
    const next = Number.isFinite(seconds) ? Math.min(Math.max(0, seconds), Math.max(0, duration)) : 0;
    set({ playheadSeconds: next });
  },
  setPlaying: (playing) => set({ playing }),
  togglePlaying: () => set((state) => ({ playing: !state.playing })),
  previewAsset: (mediaId) => set({ previewSource: { kind: "asset", mediaId }, playing: false }),
  previewTimeline: () => set({ previewSource: { kind: "timeline" } }),
  setFullscreen: (fullscreen) => set({ fullscreen }),
});
```

```ts
// src/editor/store/ui-slice.ts
import type { EditorSliceCreator } from "./editor-store";
import { clamp, leftWidthBounds, loadEditorLayout, saveEditorLayout, type EditorTabId } from "./persisted-layout";

export type SheetId = EditorTabId | "properties";

export interface UiSlice {
  readonly activeTab: EditorTabId;
  readonly leftWidth: number;
  readonly openSheetId: SheetId | null;
  setActiveTab(tab: EditorTabId): void;
  setLeftWidth(width: number): void;
  openSheet(sheet: SheetId): void;
  closeSheet(): void;
}

export const createUiSlice: EditorSliceCreator<UiSlice> = (set, get) => {
  const layout = loadEditorLayout();
  const persist = () => saveEditorLayout({ activeTab: get().activeTab, leftWidth: get().leftWidth });
  return {
    activeTab: layout.activeTab,
    leftWidth: layout.leftWidth,
    openSheetId: null,
    setActiveTab: (tab) => {
      set({ activeTab: tab });
      persist();
    },
    setLeftWidth: (width) => {
      set({ leftWidth: clamp(Math.round(width), leftWidthBounds.min, leftWidthBounds.max) });
      persist();
    },
    openSheet: (sheet) => set({ openSheetId: sheet }),
    closeSheet: () => set({ openSheetId: null }),
  };
};
```

```ts
// src/editor/store/timeline-view-slice.ts
import type { EditorSliceCreator } from "./editor-store";
import { clamp, loadTimelineView, saveTimelineView, splitHeightBounds, zoomBounds } from "./persisted-layout";

export interface TimelineViewSlice {
  readonly splitHeight: number;
  readonly zoomPercent: number;
  readonly snapEnabled: boolean;
  readonly keyframesVisible: boolean;
  setSplitHeight(height: number): void;
  setZoomPercent(percent: number): void;
  setSnapEnabled(enabled: boolean): void;
  setKeyframesVisible(visible: boolean): void;
}

export const createTimelineViewSlice: EditorSliceCreator<TimelineViewSlice> = (set, get) => {
  const view = loadTimelineView(get().projectDir);
  const persist = () => {
    const state = get();
    saveTimelineView(state.projectDir, {
      splitHeight: state.splitHeight,
      zoomPercent: state.zoomPercent,
      snapEnabled: state.snapEnabled,
      keyframesVisible: state.keyframesVisible,
    });
  };
  return {
    ...view,
    setSplitHeight: (height) => {
      set({ splitHeight: Math.max(splitHeightBounds.min, Math.round(height)) });
      persist();
    },
    setZoomPercent: (percent) => {
      set({ zoomPercent: clamp(percent, zoomBounds.min, zoomBounds.max) });
      persist();
    },
    setSnapEnabled: (enabled) => {
      set({ snapEnabled: enabled });
      persist();
    },
    setKeyframesVisible: (visible) => {
      set({ keyframesVisible: visible });
      persist();
    },
  };
};
```

`createTimelineViewSlice` calls `get().projectDir` during creation. The store factory in Step 5 therefore spreads the project slice first and builds this slice from a getter that already includes it.

- [ ] **Step 5: Compose the store and prune selection on project replacement**

```ts
// src/editor/store/editor-store.ts
import { createStore, type StoreApi } from "zustand/vanilla";
import type { VideoProject } from "@/lib/project";
import { createPlaybackSlice, type PlaybackSlice } from "./playback-slice";
import { createProjectSlice, type ProjectSlice } from "./project-slice";
import { createSelectionSlice, type SelectionSlice } from "./selection-slice";
import { createTimelineViewSlice, type TimelineViewSlice } from "./timeline-view-slice";
import { createUiSlice, type UiSlice } from "./ui-slice";

export type EditorState = ProjectSlice & SelectionSlice & PlaybackSlice & UiSlice & TimelineViewSlice;

export type EditorSliceCreator<T> = (
  set: StoreApi<EditorState>["setState"],
  get: StoreApi<EditorState>["getState"],
) => T;

export interface EditorStoreInit {
  readonly projectDir: string;
  readonly project: VideoProject;
}

export type EditorStore = StoreApi<EditorState>;

export function createEditorStore(init: EditorStoreInit): EditorStore {
  const store = createStore<EditorState>()((set, get) => {
    const project = createProjectSlice(init)(set, get);
    const initGet = () => ({ ...project, ...get() }) as EditorState;
    return {
      ...project,
      ...createSelectionSlice(set, get),
      ...createPlaybackSlice(set, get),
      ...createUiSlice(set, get),
      ...createTimelineViewSlice(set, initGet),
    };
  });

  store.subscribe((state, previous) => {
    if (state.project !== previous.project) state.pruneSelection(state.project);
  });
  return store;
}
```

- [ ] **Step 6: Implement the React provider**

```tsx
// src/editor/store/editor-store-context.tsx
import { createContext, useContext, type ReactNode } from "react";
import { useStore } from "zustand";
import type { EditorState, EditorStore } from "./editor-store";

const EditorStoreContext = createContext<EditorStore | null>(null);

export function EditorStoreProvider({ store, children }: { store: EditorStore; children: ReactNode }) {
  return <EditorStoreContext.Provider value={store}>{children}</EditorStoreContext.Provider>;
}

export function useEditorStoreApi(): EditorStore {
  const store = useContext(EditorStoreContext);
  if (!store) throw new Error("useEditorStore must be used inside EditorStoreProvider");
  return store;
}

export function useEditorStore<T>(selector: (state: EditorState) => T): T {
  return useStore(useEditorStoreApi(), selector);
}
```

- [ ] **Step 7: Run to verify they pass**

Run: `rtk pnpm vitest run src/editor/store && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS. `editor-store-context.tsx` is unused until Task 8. If knip flags it, finish Task 8 before committing and commit Tasks 6 and 8 together.

- [ ] **Step 8: Commit**

```bash
rtk git add src/editor/store
rtk git commit -m "feat(editor): add selection, playback, ui, and timeline view store slices"
```

---

### Task 7: UI primitives

**Files:**
- Create: `src/components/ui/tooltip.tsx`, `tabs.tsx`, `dropdown-menu.tsx`, `dialog.tsx`, `icon-button.tsx`
- Modify: `package.json`, `pnpm-lock.yaml`

Do this task in the same working session as Task 8 and commit both together, so every primitive has a consumer when knip runs.

- [ ] **Step 1: Add Radix packages**

Run: `rtk pnpm add @radix-ui/react-tooltip @radix-ui/react-tabs @radix-ui/react-dropdown-menu @radix-ui/react-dialog`
Expected: four new dependencies.

- [ ] **Step 2: Write the primitives**

```tsx
// src/components/ui/tooltip.tsx
import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/lib/utils";

export const TooltipProvider = TooltipPrimitive.Provider;

export function Tooltip({
  content,
  children,
  side = "bottom",
  className,
}: {
  content: ReactNode;
  children: ReactNode;
  side?: ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>["side"];
  className?: string;
}) {
  return (
    <TooltipPrimitive.Root delayDuration={400}>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Content
          side={side}
          sideOffset={6}
          className={cn("z-50 rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground shadow-lg", className)}
        >
          {content}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}
```

```tsx
// src/components/ui/icon-button.tsx
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Tooltip } from "./tooltip";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly label: string;
  readonly tooltip?: ReactNode;
  readonly size?: "sm" | "md";
  readonly active?: boolean;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, tooltip, size = "md", active = false, className, children, ...props },
  ref,
) {
  const button = (
    <button
      ref={ref}
      type="button"
      aria-label={label}
      aria-pressed={props["aria-pressed"]}
      className={cn(
        "grid place-items-center rounded-control text-muted-foreground transition-colors hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40",
        size === "md" ? "h-8 w-8" : "h-[26px] w-[26px] rounded-md",
        active && "bg-raised text-foreground",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
  return <Tooltip content={tooltip ?? label}>{button}</Tooltip>;
});
```

```tsx
// src/components/ui/tabs.tsx
import * as TabsPrimitive from "@radix-ui/react-tabs";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "@/lib/utils";

export const Tabs = TabsPrimitive.Root;

export const TabsList = forwardRef<ElementRef<typeof TabsPrimitive.List>, ComponentPropsWithoutRef<typeof TabsPrimitive.List>>(
  function TabsList({ className, ...props }, ref) {
    return <TabsPrimitive.List ref={ref} className={cn("flex", className)} {...props} />;
  },
);

export const TabsTrigger = forwardRef<
  ElementRef<typeof TabsPrimitive.Trigger>,
  ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(function TabsTrigger({ className, ...props }, ref) {
  return (
    <TabsPrimitive.Trigger
      ref={ref}
      className={cn(
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=active]:text-foreground",
        className,
      )}
      {...props}
    />
  );
});

export const TabsContent = forwardRef<
  ElementRef<typeof TabsPrimitive.Content>,
  ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(function TabsContent({ className, ...props }, ref) {
  return <TabsPrimitive.Content ref={ref} className={cn("min-h-0 flex-1 focus-visible:outline-none", className)} {...props} />;
});
```

```tsx
// src/components/ui/dropdown-menu.tsx
import * as MenuPrimitive from "@radix-ui/react-dropdown-menu";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "@/lib/utils";

export const DropdownMenu = MenuPrimitive.Root;
export const DropdownMenuTrigger = MenuPrimitive.Trigger;

export const DropdownMenuContent = forwardRef<
  ElementRef<typeof MenuPrimitive.Content>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.Content>
>(function DropdownMenuContent({ className, sideOffset = 6, ...props }, ref) {
  return (
    <MenuPrimitive.Portal>
      <MenuPrimitive.Content
        ref={ref}
        sideOffset={sideOffset}
        className={cn("z-50 min-w-48 rounded-control border border-line bg-popover p-1 text-popover-foreground shadow-xl", className)}
        {...props}
      />
    </MenuPrimitive.Portal>
  );
});

export const DropdownMenuItem = forwardRef<
  ElementRef<typeof MenuPrimitive.Item>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.Item>
>(function DropdownMenuItem({ className, ...props }, ref) {
  return (
    <MenuPrimitive.Item
      ref={ref}
      className={cn(
        "flex cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-[13px] outline-none data-[disabled]:opacity-40 data-[highlighted]:bg-raised",
        className,
      )}
      {...props}
    />
  );
});

export const DropdownMenuSeparator = forwardRef<
  ElementRef<typeof MenuPrimitive.Separator>,
  ComponentPropsWithoutRef<typeof MenuPrimitive.Separator>
>(function DropdownMenuSeparator({ className, ...props }, ref) {
  return <MenuPrimitive.Separator ref={ref} className={cn("my-1 h-px bg-line", className)} {...props} />;
});
```

```tsx
// src/components/ui/dialog.tsx
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "@/lib/utils";

export const Dialog = DialogPrimitive.Root;
export const DialogTitle = DialogPrimitive.Title;
export const DialogDescription = DialogPrimitive.Description;
export const DialogClose = DialogPrimitive.Close;

export const DialogOverlay = forwardRef<
  ElementRef<typeof DialogPrimitive.Overlay>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(function DialogOverlay({ className, ...props }, ref) {
  return <DialogPrimitive.Overlay ref={ref} className={cn("fixed inset-0 z-40 bg-background/70", className)} {...props} />;
});

export const DialogContent = forwardRef<
  ElementRef<typeof DialogPrimitive.Content>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(function DialogContent({ className, children, ...props }, ref) {
  return (
    <DialogPrimitive.Portal>
      <DialogOverlay />
      <DialogPrimitive.Content
        ref={ref}
        className={cn("fixed z-50 bg-panel text-foreground shadow-2xl focus-visible:outline-none", className)}
        {...props}
      >
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
});
```

`bg-background/70` is a token with an alpha channel, not a raw `black/…` class.

- [ ] **Step 3: Continue to Task 8 before verifying and committing.**

---

### Task 8: Layout mode, top bar, left panel and regions

**Files:**
- Create: `src/editor/shell/use-layout-mode.ts`, `use-layout-mode.test.ts`, `editor-tabs.ts`, `panel-placeholder.tsx`, `top-bar.tsx`, `top-bar.test.tsx`, `left-panel.tsx`, `left-panel.test.tsx`, `preview-region.tsx`, `timeline-region.tsx`, `properties-region.tsx`

- [ ] **Step 1: Write failing tests**

```ts
// src/editor/shell/use-layout-mode.test.ts
import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { layoutModeForWidth, useLayoutMode } from "./use-layout-mode";

describe("layout mode", () => {
  afterEach(() => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1024 });
  });

  it("maps widths to the two layouts and properties docking", () => {
    expect(layoutModeForWidth(402)).toBe("mobile");
    expect(layoutModeForWidth(1023)).toBe("mobile");
    expect(layoutModeForWidth(1024)).toBe("desktop-overlay");
    expect(layoutModeForWidth(1279)).toBe("desktop-overlay");
    expect(layoutModeForWidth(1280)).toBe("desktop-docked");
  });

  it("updates on resize", () => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: 1440 });
    const { result } = renderHook(() => useLayoutMode());
    expect(result.current).toBe("desktop-docked");
    act(() => {
      Object.defineProperty(window, "innerWidth", { configurable: true, value: 402 });
      window.dispatchEvent(new Event("resize"));
    });
    expect(result.current).toBe("mobile");
  });
});
```

```tsx
// src/editor/shell/top-bar.test.tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { TopBar } from "./top-bar";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderTopBar(overrides: Partial<Parameters<typeof TopBar>[0]> = {}) {
  const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
  const props = {
    compact: false,
    onOpenProjectHome: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenProjectSettings: vi.fn(),
    ...overrides,
  };
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <TopBar {...props} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, props };
}

describe("TopBar", () => {
  it("shows project name, save state, and disabled undo/redo on a fresh project", () => {
    renderTopBar();
    expect(screen.getByText("Edison Restoration Demo")).toBeInTheDocument();
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Redo" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Export" })).toBeDisabled();
  });

  it("goes home and opens settings from the gear menu", async () => {
    const { props } = renderTopBar();
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(props.onOpenProjectHome).toHaveBeenCalledTimes(1);
    fireEvent.pointerDown(screen.getByRole("button", { name: "Editor menu" }), { button: 0, ctrlKey: false });
    fireEvent.click(await screen.findByRole("menuitem", { name: "App settings" }));
    expect(props.onOpenSettings).toHaveBeenCalledTimes(1);
  });
});
```

```tsx
// src/editor/shell/left-panel.test.tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { LeftPanel } from "./left-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

describe("LeftPanel", () => {
  beforeEach(() => window.localStorage.clear());

  it("renders six tabs in order with AI active by default and switches tabs", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    render(
      <TooltipProvider>
        <EditorStoreProvider store={store}>
          <LeftPanel />
        </EditorStoreProvider>
      </TooltipProvider>,
    );
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual(["AI", "Media", "Audio", "Text", "Captions", "Effects"]);
    expect(screen.getByRole("tab", { name: "AI" })).toHaveAttribute("data-state", "active");
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Captions" }));
    expect(store.getState().activeTab).toBe("captions");
    expect(screen.getByRole("tabpanel", { name: "Captions" })).toBeInTheDocument();
  });
});
```

If Radix Tabs does not respond to `mouseDown` in jsdom, use `fireEvent.pointerDown(tab, { button: 0 })` followed by `fireEvent.mouseDown`, or use keyboard activation with `fireEvent.keyDown(tab, { key: "Enter" })`. Keep the assertion.

- [ ] **Step 2: Run to verify they fail**

Run: `rtk pnpm vitest run src/editor/shell`
Expected: FAIL, because the modules are missing.

- [ ] **Step 3: Implement**

```ts
// src/editor/shell/use-layout-mode.ts
import { useSyncExternalStore } from "react";

export type LayoutMode = "mobile" | "desktop-overlay" | "desktop-docked";

export const mobileMaxWidth = 1023;
export const dockedPropertiesMinWidth = 1280;

export function layoutModeForWidth(width: number): LayoutMode {
  if (width <= mobileMaxWidth) return "mobile";
  return width >= dockedPropertiesMinWidth ? "desktop-docked" : "desktop-overlay";
}

function subscribe(onChange: () => void): () => void {
  window.addEventListener("resize", onChange);
  return () => window.removeEventListener("resize", onChange);
}

export function useLayoutMode(): LayoutMode {
  return useSyncExternalStore(subscribe, () => layoutModeForWidth(window.innerWidth), () => "desktop-docked");
}
```

```ts
// src/editor/shell/editor-tabs.ts
import { Captions, Image, Music, Sparkles, Type, WandSparkles, type LucideIcon } from "lucide-react";
import type { EditorTabId } from "../store/persisted-layout";

export interface EditorTabDefinition {
  readonly id: EditorTabId;
  readonly label: string;
  readonly icon: LucideIcon;
  readonly shortcutId: string;
}

export const editorTabs: readonly EditorTabDefinition[] = [
  { id: "ai", label: "AI", icon: Sparkles, shortcutId: "editor.tab.ai" },
  { id: "media", label: "Media", icon: Image, shortcutId: "editor.tab.media" },
  { id: "audio", label: "Audio", icon: Music, shortcutId: "editor.tab.audio" },
  { id: "text", label: "Text", icon: Type, shortcutId: "editor.tab.text" },
  { id: "captions", label: "Captions", icon: Captions, shortcutId: "editor.tab.captions" },
  { id: "effects", label: "Effects", icon: WandSparkles, shortcutId: "editor.tab.effects" },
];
```

```tsx
// src/editor/shell/panel-placeholder.tsx
export function PanelPlaceholder({ title }: { title: string }) {
  return (
    <div className="flex h-full items-center justify-center p-6 text-center text-xs text-dim">
      <p>{title}</p>
    </div>
  );
}
```

```tsx
// src/editor/shell/top-bar.tsx
import { House, Redo2, Settings, Undo2, Upload } from "lucide-react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { IconButton } from "@/components/ui/icon-button";
import { useEditorStore } from "../store/editor-store-context";
import type { SaveStatus } from "../store/project-slice";

const saveStatusLabel: Record<SaveStatus, string> = {
  saved: "Saved",
  saving: "Saving…",
  unsaved: "Not saved",
  failed: "Save failed",
};

export interface TopBarProps {
  readonly compact: boolean;
  readonly onOpenProjectHome: () => void;
  readonly onOpenSettings: () => void;
  readonly onOpenProjectSettings: () => void;
}

export function TopBar({ compact, onOpenProjectHome, onOpenSettings, onOpenProjectSettings }: TopBarProps) {
  const name = useEditorStore((state) => state.project.name);
  const saveStatus = useEditorStore((state) => state.saveStatus);
  const canUndo = useEditorStore((state) => state.history.past.length > 0);
  const canRedo = useEditorStore((state) => state.history.future.length > 0);
  const undo = useEditorStore((state) => state.undo);
  const redo = useEditorStore((state) => state.redo);

  return (
    <header className="flex h-12 shrink-0 items-center gap-1.5 px-2.5">
      <IconButton label="Home" onClick={onOpenProjectHome}>
        <House className="h-4 w-4" aria-hidden />
      </IconButton>
      <span className="min-w-0 truncate px-2 font-semibold" title={name}>
        {name}
      </span>
      {!compact && (
        <span className="text-xs text-dim" role="status">
          {saveStatusLabel[saveStatus]}
        </span>
      )}
      <div className="flex-1" />
      <IconButton label="Undo" disabled={!canUndo} onClick={() => void undo()}>
        <Undo2 className="h-4 w-4" aria-hidden />
      </IconButton>
      <IconButton label="Redo" disabled={!canRedo} onClick={() => void redo()}>
        <Redo2 className="h-4 w-4" aria-hidden />
      </IconButton>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <IconButton label="Editor menu">
            <Settings className="h-4 w-4" aria-hidden />
          </IconButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={onOpenProjectSettings}>Project settings</DropdownMenuItem>
          <DropdownMenuItem onSelect={onOpenSettings}>App settings</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <button
        type="button"
        disabled
        className="flex h-8 items-center gap-1.5 rounded-control bg-primary px-4 font-semibold text-primary-foreground disabled:opacity-50"
      >
        <Upload className="h-4 w-4" aria-hidden />
        Export
      </button>
    </header>
  );
}
```

```tsx
// src/editor/shell/left-panel.tsx
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useEditorStore } from "../store/editor-store-context";
import type { EditorTabId } from "../store/persisted-layout";
import { editorTabs } from "./editor-tabs";
import { PanelPlaceholder } from "./panel-placeholder";

export function LeftPanel() {
  const activeTab = useEditorStore((state) => state.activeTab);
  const setActiveTab = useEditorStore((state) => state.setActiveTab);

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => setActiveTab(value as EditorTabId)}
      className="flex h-full min-h-0 flex-col overflow-hidden rounded-panel bg-panel"
    >
      <TabsList aria-label="Editor tools" className="gap-0.5 px-1.5 pt-1.5">
        {editorTabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            className="group flex flex-1 flex-col items-center gap-1 rounded-control py-2 text-[11.5px] text-muted-foreground data-[state=active]:bg-raised"
          >
            <tab.icon className="h-[19px] w-[19px] group-data-[state=active]:text-primary" aria-hidden />
            {tab.label}
          </TabsTrigger>
        ))}
      </TabsList>
      {editorTabs.map((tab) => (
        <TabsContent key={tab.id} value={tab.id} aria-label={tab.label} className="overflow-y-auto">
          <PanelPlaceholder title={tab.label} />
        </TabsContent>
      ))}
    </Tabs>
  );
}
```

Radix `TabsContent` is labelled by its trigger automatically. The explicit `aria-label` keeps the accessible name exactly equal to the tab label for tests and later plans.

```tsx
// src/editor/shell/preview-region.tsx
export function PreviewRegion() {
  return (
    <section aria-label="Preview panel" className="flex h-full min-h-0 flex-col overflow-hidden rounded-panel bg-panel">
      <div role="region" aria-label="Preview viewport" className="flex min-h-0 flex-1 items-center justify-center p-4">
        <div className="aspect-video h-full max-w-full rounded-sm bg-background" />
      </div>
      <div aria-label="Preview transport" role="group" className="h-11 shrink-0" />
    </section>
  );
}
```

```tsx
// src/editor/shell/timeline-region.tsx
export function TimelineRegion() {
  return (
    <section aria-label="Timeline" className="flex h-full min-h-0 flex-col overflow-hidden rounded-panel bg-panel">
      <div role="toolbar" aria-label="Timeline tools" className="h-10 shrink-0 border-b border-line" />
      <div role="region" aria-label="Timeline canvas" className="min-h-0 flex-1" />
    </section>
  );
}
```

```tsx
// src/editor/shell/properties-region.tsx
import { cn } from "@/lib/utils";

export function PropertiesRegion({ overlay }: { overlay: boolean }) {
  return (
    <aside
      aria-label="Properties"
      className={cn(
        "flex min-h-0 w-[330px] flex-col overflow-hidden rounded-panel bg-panel",
        overlay && "absolute bottom-0 right-0 top-0 z-20 shadow-2xl",
      )}
    />
  );
}
```

- [ ] **Step 4: Wire the store into the root now so every module has a consumer**

Replace `src/editor/editor-root.tsx` body. Keep the props interface and `editorNativeMenuState` from Task 3.

```tsx
export function EditorRoot(props: EditorRootProps) {
  const { initialProject, isActive, onNativeMenuStateChange, projectDir } = props;
  const [store] = useState(() => createEditorStore({ projectDir, project: initialProject }));

  useEffect(() => {
    if (isActive) onNativeMenuStateChange(editorNativeMenuState());
  }, [isActive, onNativeMenuStateChange]);

  return (
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <EditorShell {...props} />
      </EditorStoreProvider>
    </TooltipProvider>
  );
}
```

Until Task 9, `EditorShell` is a local component. It renders:

```tsx
<main aria-label="Video editor workspace" className="flex h-full flex-col bg-background text-foreground">
  <TopBar compact={false} onOpenProjectHome={...} onOpenSettings={...} onOpenProjectSettings={...} />
  <div className="grid min-h-0 flex-1 grid-cols-[360px_1fr] gap-1.5 px-1.5 pb-1.5">
    <LeftPanel />
    <PreviewRegion />
  </div>
  <TimelineRegion />
</main>
```

`onOpenProjectSettings` has no App prop yet. Task 11 adds `onOpenProjectSettings` to `EditorRootProps` and App. Until then, pass `props.onOpenSettings` for both menu items. Also add the `useState` and `TooltipProvider` imports.

- [ ] **Step 5: Verify**

Run: `rtk pnpm vitest run src/editor && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS. Update `editor-root.test.tsx` if its "Home" query now finds the new top bar button. It should still pass unchanged.

- [ ] **Step 6: Commit Tasks 6–8 together if any were deferred**

```bash
rtk git add package.json pnpm-lock.yaml src/components/ui src/editor
rtk git commit -m "feat(editor): add the editor top bar, tab panel, and shell regions"
```

---

### Task 9: Desktop layout with resizers and conditional properties

**Files:**
- Create: `src/editor/shell/split-resizer.tsx`, `split-resizer.test.tsx`, `desktop-layout.tsx`, `desktop-layout.test.tsx`
- Modify: `src/editor/editor-root.tsx`

- [ ] **Step 1: Write failing tests**

```tsx
// src/editor/shell/split-resizer.test.tsx
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SplitResizer } from "./split-resizer";

describe("SplitResizer", () => {
  it("reports pointer drag deltas along its axis and supports arrow keys", () => {
    const onResize = vi.fn();
    render(<SplitResizer orientation="vertical" label="Resize left panel" value={360} min={300} max={440} onResize={onResize} />);
    const handle = screen.getByRole("separator", { name: "Resize left panel" });
    expect(handle).toHaveAttribute("aria-valuenow", "360");

    fireEvent.pointerDown(handle, { clientX: 100, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 130, clientY: 0, pointerId: 1 });
    expect(onResize).toHaveBeenLastCalledWith(390);
    fireEvent.pointerUp(handle, { pointerId: 1 });

    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(onResize).toHaveBeenLastCalledWith(350);
  });

  it("inverts deltas for handles that grow toward the top", () => {
    const onResize = vi.fn();
    render(<SplitResizer orientation="horizontal" invert label="Resize timeline" value={300} min={160} max={600} onResize={onResize} />);
    const handle = screen.getByRole("separator", { name: "Resize timeline" });
    fireEvent.pointerDown(handle, { clientX: 0, clientY: 500, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 0, clientY: 450, pointerId: 1 });
    expect(onResize).toHaveBeenLastCalledWith(350);
  });
});
```

```tsx
// src/editor/shell/desktop-layout.test.tsx
import "@testing-library/jest-dom/vitest";
import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { DesktopLayout } from "./desktop-layout";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderLayout(mode: "desktop-docked" | "desktop-overlay") {
  const project = fixtureProject();
  const store = createEditorStore({ projectDir: "/p", project });
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <DesktopLayout mode={mode} topBar={<div>top</div>} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, project };
}

describe("DesktopLayout", () => {
  beforeEach(() => window.localStorage.clear());

  it("hides properties without a selection and docks them with one", () => {
    const { store, project } = renderLayout("desktop-docked");
    expect(screen.queryByRole("complementary", { name: "Properties" })).not.toBeInTheDocument();
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    const properties = screen.getByRole("complementary", { name: "Properties" });
    expect(properties).not.toHaveClass("absolute");
  });

  it("overlays properties in the narrow desktop mode", () => {
    const { store, project } = renderLayout("desktop-overlay");
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    expect(screen.getByRole("complementary", { name: "Properties" })).toHaveClass("absolute");
  });

  it("renders all fixed regions and both resizers", () => {
    renderLayout("desktop-docked");
    expect(screen.getByRole("tablist", { name: "Editor tools" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Preview viewport" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Timeline canvas" })).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize left panel" })).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize timeline" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `rtk pnpm vitest run src/editor/shell/split-resizer.test.tsx src/editor/shell/desktop-layout.test.tsx`
Expected: FAIL, because the modules are missing.

- [ ] **Step 3: Implement**

```tsx
// src/editor/shell/split-resizer.tsx
import { useRef, type KeyboardEvent, type PointerEvent } from "react";
import { cn } from "@/lib/utils";

export interface SplitResizerProps {
  readonly orientation: "vertical" | "horizontal";
  readonly label: string;
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly invert?: boolean;
  readonly step?: number;
  readonly onResize: (value: number) => void;
}

export function SplitResizer({ orientation, label, value, min, max, invert = false, step = 10, onResize }: SplitResizerProps) {
  const drag = useRef<{ origin: number; start: number } | null>(null);
  const coordinate = (event: PointerEvent) => (orientation === "vertical" ? event.clientX : event.clientY);
  const clamp = (next: number) => Math.min(max, Math.max(min, next));

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    drag.current = { origin: coordinate(event), start: value };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!drag.current) return;
    const delta = coordinate(event) - drag.current.origin;
    onResize(clamp(drag.current.start + (invert ? -delta : delta)));
  }

  function onPointerUp(event: PointerEvent<HTMLDivElement>) {
    drag.current = null;
    event.currentTarget.releasePointerCapture?.(event.pointerId);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const decrease = orientation === "vertical" ? "ArrowLeft" : invert ? "ArrowDown" : "ArrowUp";
    const increase = orientation === "vertical" ? "ArrowRight" : invert ? "ArrowUp" : "ArrowDown";
    if (event.key === decrease) onResize(clamp(value - step));
    else if (event.key === increase) onResize(clamp(value + step));
    else return;
    event.preventDefault();
  }

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onKeyDown={onKeyDown}
      className={cn(
        "shrink-0 touch-none rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        orientation === "vertical" ? "w-1.5 cursor-col-resize" : "h-1.5 cursor-row-resize",
      )}
    />
  );
}
```

```tsx
// src/editor/shell/desktop-layout.tsx
import type { ReactNode } from "react";
import { useEditorStore } from "../store/editor-store-context";
import { leftWidthBounds, splitHeightBounds } from "../store/persisted-layout";
import { LeftPanel } from "./left-panel";
import { PreviewRegion } from "./preview-region";
import { PropertiesRegion } from "./properties-region";
import { SplitResizer } from "./split-resizer";
import { TimelineRegion } from "./timeline-region";

const minimumPreviewHeight = 240;
const topBarHeight = 48;

export function DesktopLayout({ mode, topBar }: { mode: "desktop-docked" | "desktop-overlay"; topBar: ReactNode }) {
  const leftWidth = useEditorStore((state) => state.leftWidth);
  const setLeftWidth = useEditorStore((state) => state.setLeftWidth);
  const splitHeight = useEditorStore((state) => state.splitHeight);
  const setSplitHeight = useEditorStore((state) => state.setSplitHeight);
  const hasSelection = useEditorStore((state) => state.selectedItemIds.length > 0);
  const docked = mode === "desktop-docked";
  const maxSplit = Math.max(splitHeightBounds.min, window.innerHeight - topBarHeight - minimumPreviewHeight);
  const timelineHeight = Math.min(splitHeight, maxSplit);

  return (
    <main aria-label="Video editor workspace" className="flex h-full flex-col overflow-hidden bg-background text-foreground">
      {topBar}
      <div className="flex min-h-0 flex-1 px-1.5">
        <div style={{ width: leftWidth }} className="min-h-0 shrink-0">
          <LeftPanel />
        </div>
        <SplitResizer
          orientation="vertical"
          label="Resize left panel"
          value={leftWidth}
          min={leftWidthBounds.min}
          max={leftWidthBounds.max}
          onResize={setLeftWidth}
        />
        <div className="relative flex min-h-0 min-w-0 flex-1 gap-1.5">
          <div className="min-h-0 min-w-0 flex-1">
            <PreviewRegion />
          </div>
          {hasSelection && <PropertiesRegion overlay={!docked} />}
        </div>
      </div>
      <SplitResizer
        orientation="horizontal"
        invert
        label="Resize timeline"
        value={timelineHeight}
        min={splitHeightBounds.min}
        max={maxSplit}
        onResize={setSplitHeight}
      />
      <div style={{ height: timelineHeight }} className="shrink-0 px-1.5 pb-1.5">
        <TimelineRegion />
      </div>
    </main>
  );
}
```

- [ ] **Step 4: Use the layout in the root**

Replace the local `EditorShell` in `editor-root.tsx` with:
- `const mode = useLayoutMode()`,
- `<TopBar compact={mode === "mobile"} … />` passed as `topBar`,
- `mode === "mobile" ? <MobileLayout topBar={topBar} /> : <DesktopLayout mode={mode} topBar={topBar} />`.

`MobileLayout` is added in Task 10. For this commit, render `DesktopLayout` for all modes and leave a `// Task 10` note out of the code. Until Task 10 lands, the mobile branch simply uses `mode === "mobile" ? "desktop-overlay" : mode`.

- [ ] **Step 5: Verify**

Run: `rtk pnpm vitest run src/editor && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/editor
rtk git commit -m "feat(editor): add the desktop editor layout with resizable panels"
```

---

### Task 10: Mobile layout with bottom tool bar and sheets

**Files:**
- Create: `src/editor/shell/bottom-sheet.tsx`, `mobile-layout.tsx`, `mobile-layout.test.tsx`
- Modify: `src/editor/editor-root.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
// src/editor/shell/mobile-layout.test.tsx
import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { MobileLayout } from "./mobile-layout";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderMobile() {
  const project = fixtureProject();
  const store = createEditorStore({ projectDir: "/p", project });
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <MobileLayout topBar={<div>top</div>} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, project };
}

describe("MobileLayout", () => {
  beforeEach(() => window.localStorage.clear());

  it("stacks preview and timeline above a six-tab bottom tool bar", () => {
    renderMobile();
    expect(screen.getByRole("region", { name: "Preview viewport" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Timeline canvas" })).toBeInTheDocument();
    const toolbar = screen.getByRole("toolbar", { name: "Editor tools" });
    expect(Array.from(toolbar.querySelectorAll("button")).map((button) => button.textContent)).toEqual([
      "AI", "Media", "Audio", "Text", "Captions", "Effects",
    ]);
  });

  it("opens a tab as a labelled bottom sheet and closes it", () => {
    const { store } = renderMobile();
    fireEvent.click(screen.getByRole("button", { name: "Media" }));
    expect(store.getState().openSheetId).toBe("media");
    expect(screen.getByRole("dialog", { name: "Media" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close Media" }));
    expect(screen.queryByRole("dialog", { name: "Media" })).not.toBeInTheDocument();
  });

  it("switches to clip tools while an item is selected and back", () => {
    const { store, project } = renderMobile();
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    expect(screen.getByRole("toolbar", { name: "Clip tools" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to editor tools" }));
    expect(store.getState().selectedItemIds).toEqual([]);
    expect(screen.getByRole("toolbar", { name: "Editor tools" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `rtk pnpm vitest run src/editor/shell/mobile-layout.test.tsx`
Expected: FAIL, because the module is missing.

- [ ] **Step 3: Implement**

```tsx
// src/editor/shell/bottom-sheet.tsx
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";

export function BottomSheet({
  title,
  open,
  height,
  onClose,
  children,
}: {
  title: string;
  open: boolean;
  height: "compact" | "tall";
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent
        aria-describedby={undefined}
        className={`inset-x-0 bottom-0 flex flex-col rounded-t-[18px] ${height === "tall" ? "h-[70dvh]" : "h-[40dvh]"}`}
      >
        <div className="mx-auto mb-1 mt-2 h-[5px] w-[38px] rounded-full bg-hover" aria-hidden />
        <div className="flex items-center px-3.5 pb-2">
          <DialogTitle className="font-semibold">{title}</DialogTitle>
          <div className="flex-1" />
          <DialogClose aria-label={`Close ${title}`} className="grid h-8 w-8 place-items-center rounded-control text-muted-foreground hover:bg-raised">
            <X className="h-4 w-4" aria-hidden />
          </DialogClose>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      </DialogContent>
    </Dialog>
  );
}
```

```tsx
// src/editor/shell/mobile-layout.tsx
import { ChevronLeft } from "lucide-react";
import type { ReactNode } from "react";
import { useEditorStore } from "../store/editor-store-context";
import { editorTabs } from "./editor-tabs";
import { BottomSheet } from "./bottom-sheet";
import { PanelPlaceholder } from "./panel-placeholder";
import { PreviewRegion } from "./preview-region";
import { TimelineRegion } from "./timeline-region";

const tallSheets = new Set(["ai", "media", "audio"]);

export function MobileLayout({ topBar }: { topBar: ReactNode }) {
  const openSheetId = useEditorStore((state) => state.openSheetId);
  const openSheet = useEditorStore((state) => state.openSheet);
  const closeSheet = useEditorStore((state) => state.closeSheet);
  const hasSelection = useEditorStore((state) => state.selectedItemIds.length > 0);
  const clearSelection = useEditorStore((state) => state.clearSelection);
  const openTab = editorTabs.find((tab) => tab.id === openSheetId);

  return (
    <main aria-label="Video editor workspace" className="flex h-full flex-col overflow-hidden bg-background text-foreground">
      {topBar}
      <div className="shrink-0">
        <PreviewRegion />
      </div>
      <div className="min-h-0 flex-1">
        <TimelineRegion />
      </div>
      <nav className="h-[78px] shrink-0 border-t border-line bg-panel px-1 pt-2">
        {hasSelection ? (
          <div role="toolbar" aria-label="Clip tools" className="flex h-full">
            <button
              type="button"
              aria-label="Back to editor tools"
              onClick={clearSelection}
              className="grid w-11 place-items-start justify-center border-r border-line pt-3.5 text-muted-foreground"
            >
              <ChevronLeft className="h-5 w-5" aria-hidden />
            </button>
          </div>
        ) : (
          <div role="toolbar" aria-label="Editor tools" className="flex h-full">
            {editorTabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => openSheet(tab.id)}
                className="flex flex-1 flex-col items-center gap-1 rounded-control py-1.5 text-[11px] text-muted-foreground"
              >
                <tab.icon className="h-[21px] w-[21px]" aria-hidden />
                {tab.label}
              </button>
            ))}
          </div>
        )}
      </nav>
      {openTab && (
        <BottomSheet title={openTab.label} open height={tallSheets.has(openTab.id) ? "tall" : "compact"} onClose={closeSheet}>
          <PanelPlaceholder title={openTab.label} />
        </BottomSheet>
      )}
    </main>
  );
}
```

In the jsdom test the "Media" tab button's accessible name comes from its text. The icon is `aria-hidden`, so `getByRole("button", { name: "Media" })` resolves.

- [ ] **Step 4: Use MobileLayout in the root** for `mode === "mobile"`.

- [ ] **Step 5: Verify**

Run: `rtk pnpm vitest run src/editor && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/editor
rtk git commit -m "feat(editor): add the mobile editor layout with bottom tool bar and sheets"
```

---

### Task 11: Global shortcuts, native menu state and project settings wiring

**Files:**
- Create: `src/editor/shell/use-editor-shortcuts.ts`, `use-editor-shortcuts.test.tsx`
- Modify: `src/editor/editor-root.tsx`, `src/editor/editor-root.test.tsx`, `src/App.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
// src/editor/shell/use-editor-shortcuts.test.tsx
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import type { ProjectAction } from "@/lib/project";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (p: string) => p,
}));

const { createEditorStore } = await import("../store/editor-store");
const { EditorStoreProvider } = await import("../store/editor-store-context");
const { useEditorShortcuts } = await import("./use-editor-shortcuts");

function setup(platform: "macos" | "linux") {
  const project = { ...fixtureProject(), schemaVersion: 2, contentRevision: 1 };
  const store = createEditorStore({ projectDir: "/p", project });
  const wrapper = ({ children }: { children: ReactNode }) => <EditorStoreProvider store={store}>{children}</EditorStoreProvider>;
  renderHook(() => useEditorShortcuts(platform), { wrapper });
  return { store, project };
}

describe("useEditorShortcuts", () => {
  beforeEach(() => {
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    window.localStorage.clear();
  });

  it("switches tabs with Mod+number", () => {
    const { store } = setup("linux");
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "5", ctrlKey: true }));
    });
    expect(store.getState().activeTab).toBe("captions");
  });

  it("undoes with Mod+Z and ignores editable targets", async () => {
    const { store, project } = setup("macos");
    await act(async () => {
      await store.getState().applyActions([
        { type: "updateVisualClipOpacity", itemId: fixtureItem(project, "video").id, opacity: 0.25 } as ProjectAction,
      ]);
    });
    const input = document.createElement("input");
    document.body.append(input);
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true, bubbles: true }));
    });
    expect(store.getState().history.past).toHaveLength(1);

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "z", metaKey: true }));
      await Promise.resolve();
    });
    await vi.waitFor(() => expect(store.getState().history.past).toHaveLength(0));
    input.remove();
  });

  it("clears selection with Escape", () => {
    const { store, project } = setup("linux");
    act(() => store.getState().selectItems([fixtureItem(project, "video").id]));
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(store.getState().selectedItemIds).toEqual([]);
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `rtk pnpm vitest run src/editor/shell/use-editor-shortcuts.test.tsx`
Expected: FAIL, because the module is missing.

- [ ] **Step 3: Implement**

```ts
// src/editor/shell/use-editor-shortcuts.ts
import { useEffect } from "react";
import { isEditableKeyboardTarget } from "@/lib/timeline-ops/navigation";
import { matchShortcut, type ShortcutPlatform } from "@/lib/keymap";
import { useEditorStoreApi } from "../store/editor-store-context";
import type { EditorTabId } from "../store/persisted-layout";

const tabByShortcut: Record<string, EditorTabId> = {
  "editor.tab.ai": "ai",
  "editor.tab.media": "media",
  "editor.tab.audio": "audio",
  "editor.tab.text": "text",
  "editor.tab.captions": "captions",
  "editor.tab.effects": "effects",
};

export function useEditorShortcuts(platform: ShortcutPlatform): void {
  const store = useEditorStoreApi();

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || isEditableKeyboardTarget(event.target)) return;
      const shortcut = matchShortcut(event, "global", platform);
      if (!shortcut) return;
      const state = store.getState();
      const tab = tabByShortcut[shortcut.id];
      if (tab) {
        state.setActiveTab(tab);
      } else if (shortcut.id === "editor.undo") {
        void state.undo();
      } else if (shortcut.id === "editor.redo") {
        void state.redo();
      } else if (shortcut.id === "editor.clearSelection") {
        if (state.openSheetId) state.closeSheet();
        else state.clearSelection();
      } else {
        return;
      }
      event.preventDefault();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [platform, store]);
}
```

`isEditableKeyboardTarget` comes from Plan 01 Task 4, which moved it into `src/lib/timeline-ops/navigation.ts`. The input-target test dispatches on the input with `bubbles: true`, so the window listener sees an editable target and ignores it.

- [ ] **Step 4: Wire the platform, native menu and project settings**

In `editor-root.tsx`:
- Read the platform with the existing platform store from `src/lib/runtime/platform.ts`. Use its exported hook, found with `rtk rg -n "^export function use" src/lib/runtime/platform.ts`. Map `macos` to `"macos"` and everything else to `"linux"`.
- Call `useEditorShortcuts(platform)` inside a child rendered under `EditorStoreProvider`.
- Add `readonly onOpenProjectSettings: () => void` to `EditorRootProps` and pass it to `TopBar`.

In `src/App.tsx`, pass `onOpenProjectSettings` to `<EditorRoot>`. Implement it with the existing handler that the native `openProjectSettings` command uses; `rtk rg -n "openProjectSettings" src/App.tsx` finds the function that sets the `projectSettings` view with the editor origin. Update `editor-root.test.tsx` to pass `onOpenProjectSettings={vi.fn()}`.

`editorNativeMenuState()` keeps reporting `view: "editor"` with every capability false until plan 07 rewires the menu.

- [ ] **Step 5: Verify**

Run: `rtk pnpm vitest run src/editor src/App.test.tsx && rtk pnpm lint && rtk pnpm check:unused`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
rtk git add src/editor src/App.tsx
rtk git commit -m "feat(editor): wire global shortcuts and project settings into the editor shell"
```

---

### Task 12: Window minimum size, shell e2e and full gate

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `e2e/editor-shell.spec.ts`

- [ ] **Step 1: Lower the Tauri minimum window size**

In `src-tauri/tauri.conf.json`, set `"minWidth": 400` and `"minHeight": 640` for the main window (L19–20).

Run: `rtk rg -n "minWidth|minHeight" src-tauri scripts src`
Expected: only `tauri.conf.json`. No test asserts these values.

- [ ] **Step 2: Write the shell e2e**

```ts
// e2e/editor-shell.spec.ts
import { expect, test, type Page } from "@playwright/test";

const preferences = {
  schemaVersion: 1,
};

async function openSampleEditor(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  await page.addInitScript((prefs) => {
    (window as unknown as { __EDITOR_FIXTURE_RUNTIME__: unknown }).__EDITOR_FIXTURE_RUNTIME__ = {
      enabled: true,
      settingsFixtureId: "settings-ai-models-no-project",
      preferences: prefs,
      exportCapabilities: [],
    };
  }, preferences);
  await page.goto("/");
  await page.getByRole("button", { name: "Open sample project" }).first().click();
  await expect(page.getByRole("main", { name: "Video editor workspace" })).toBeVisible();
  return errors;
}

test("desktop editor shell renders the fixed layout and switches tabs", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);

  await expect(page.getByRole("tablist", { name: "Editor tools" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Preview viewport" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Timeline canvas" })).toBeVisible();
  await expect(page.getByRole("complementary", { name: "Properties" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Undo" })).toBeDisabled();

  await page.getByRole("tab", { name: "Captions" }).click();
  await expect(page.getByRole("tabpanel", { name: "Captions" })).toBeVisible();
  await page.keyboard.press("Control+1");
  await expect(page.getByRole("tab", { name: "AI" })).toHaveAttribute("data-state", "active");

  await page.getByRole("button", { name: "Home" }).click();
  await expect(page.getByRole("main", { name: "Video editor workspace" })).toBeHidden();
  expect(errors).toEqual([]);
});

test("iPhone 17 Pro editor shell uses the bottom tool bar and sheets", async ({ page }) => {
  await page.setViewportSize({ width: 402, height: 874 });
  const errors = await openSampleEditor(page);

  await expect(page.getByRole("toolbar", { name: "Editor tools" })).toBeVisible();
  await expect(page.getByRole("tablist", { name: "Editor tools" })).toHaveCount(0);
  await page.getByRole("toolbar", { name: "Editor tools" }).getByRole("button", { name: "Media" }).click();
  await expect(page.getByRole("dialog", { name: "Media" })).toBeVisible();
  await page.getByRole("button", { name: "Close Media" }).click();
  await expect(page.getByRole("dialog", { name: "Media" })).toHaveCount(0);

  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  expect(overflow).toBeLessThanOrEqual(0);
  expect(errors).toEqual([]);
});
```

Before running:
- Copy the exact fixture marker (preferences shape, `settingsFixtureId`) and the "Open sample project" button name from `e2e/app-smoke.spec.ts`, replacing the placeholders above. Also add its `installFixture` media routing if the sample project requests fixture media.
- The Home page may show "Open Sample" in the sidebar and "Open sample project" on the card. Use whichever `app-smoke.spec.ts` uses.
- Use `Meta+1` instead of `Control+1` if the fixture platform is `macos`.

- [ ] **Step 3: Run the e2e**

Run: `rtk pnpm test:browser`
Expected: PASS for `app-smoke.spec.ts` and `editor-shell.spec.ts`.

- [ ] **Step 4: Check the shell visually at each size**

Run the Vite server with `rtk pnpm dev:web-runtime`. With Playwright, capture the editor at 1440×900, 1280×720, 1024×768, 820×1180 and 402×874 into `output/editor-shell/`:
- Adapt `e2e/editor-shell.spec.ts` into a temporary script under `output/`, which is not committed, or
- use the `run` skill.

Read each PNG and check:
- no horizontal overflow,
- regions visible,
- top bar not clipped at 402 px (the project name truncates),
- sheet covers the bottom on mobile,
- focus ring visible on a tabbed control.

Fix any issue and re-run the unit tests.

- [ ] **Step 5: Run the full gate**

Run: `rtk pnpm verify:frontend`
Expected: PASS on Linux.

- [ ] **Step 6: Commit**

```bash
rtk git add src-tauri/tauri.conf.json e2e/editor-shell.spec.ts src/editor
rtk git commit -m "test(editor): cover the desktop and phone editor shell in Playwright"
```

## Self-Review Checklist (for the plan executor)

- `src/components/workspace/` no longer exists.
- `rtk rg -n "components/workspace" src scripts e2e` returns nothing.
- The editor is reachable from Home and Settings round-trips still work (`src/App.test.tsx`, `e2e/app-smoke.spec.ts`).
- The accessible names listed in Global Constraints exist in the shell.
- `knip.jsonc` has a single documented temporary block for the backend exports consumed later.
- No `src/editor/**` file exceeds 600 lines.
- No raw hex or `white/…`/`black/…` classes in `src/editor/**` or `src/components/ui/**`: `rtk rg -n "\[#|white/|black/" src/editor src/components/ui`.
- `rtk pnpm verify:frontend` passes on Linux. The macOS baseline refresh status is recorded.
