# Palmier-First Visual E2E and Parity Tracker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove the Palmier-first home and editor states function correctly and are visibly similar at matched states and viewports, then make `docs/parity.md` accurately reflect the result.

**Architecture:** Extend the existing deterministic browser visual harness instead of creating an unrelated runner. Add a dependency-free comparison-board generator that embeds each Palmier reference beside the matching Video Creater capture, then gate closure on both automated geometry assertions and recorded human visual review.

**Tech Stack:** Node.js, existing Playwright CLI wrapper, Vite, Vitest, TypeScript, Tauri 2, Rust/Cargo, ffprobe/native runtime checks, Markdown tracker evidence.

---

## File Structure

- Modify `scripts/browser-visual-qa.mjs`: add Palmier parity viewports, states, and exact geometry assertions.
- Create `scripts/palmier-visual-comparison.mjs`: generate one self-contained HTML board and JSON manifest from reference/implementation pairs.
- Create `scripts/palmier-visual-comparison.test.mjs`: validate pair discovery, missing-file failure, HTML escaping, and manifest output.
- Modify `package.json`: add `visual:qa:palmier` and `visual:qa:palmier-board` scripts.
- Modify `docs/parity.md`: record exact screenshot, test, build, package, and visual-review evidence without overclaiming.
- Retain evidence under ignored `output/parity-audit-2026-07-13/`.

### Task 1: Add the Palmier parity scenario matrix

**Files:**
- Modify: `scripts/browser-visual-qa.mjs:16-42`

- [ ] **Step 1: Add a failing scenario-registry assertion**

Export the registry only when the module is imported:

```js
export const visualQaScenarios = [
  // existing scenarios
];
```

Create `scripts/browser-visual-qa-scenarios.test.mjs` with:

```ts
import { describe, expect, it } from "vitest";
import { visualQaScenarios } from "./browser-visual-qa.mjs";

describe("Palmier visual QA scenarios", () => {
  it("covers every approved reference state and pane-budget viewport", () => {
    expect(visualQaScenarios).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ surface: "home", width: 1440, height: 960, state: "palmier" }),
        expect.objectContaining({ surface: "editor", width: 1440, height: 960, state: "palmier" }),
        expect.objectContaining({ surface: "editor", width: 1280, height: 720, state: "pane-budget" }),
        expect.objectContaining({ surface: "editor", width: 1024, height: 768, state: "pane-budget" }),
        expect.objectContaining({ surface: "generation", state: "attached" }),
        expect.objectContaining({ surface: "media", state: "overflow" }),
        expect.objectContaining({ surface: "captions", state: "workbench" }),
        expect.objectContaining({ surface: "editor", state: "speech-details" }),
        expect.objectContaining({ surface: "inspector", state: "details" }),
        expect.objectContaining({ surface: "inspector", state: "ai-edit" }),
        expect.objectContaining({ surface: "export", state: "video" }),
        expect.objectContaining({ surface: "agent", state: "starters" }),
      ]),
    );
  });
});
```

- [ ] **Step 2: Run the registry test to verify failure**

Run: `rtk pnpm exec vitest run scripts/browser-visual-qa-scenarios.test.mjs`

Expected: FAIL because the new scenarios and export do not exist.

- [ ] **Step 3: Add the approved states**

Append these deterministic scenarios:

```js
{ surface: "home", viewport: "desktop", width: 1440, height: 960, state: "palmier" },
{ surface: "editor", viewport: "desktop", width: 1440, height: 960, state: "palmier" },
{ surface: "editor", viewport: "laptop", width: 1280, height: 720, state: "pane-budget" },
{ surface: "editor", viewport: "compact-desktop", width: 1024, height: 768, state: "pane-budget" },
{ surface: "generation", viewport: "desktop", width: 1440, height: 960, state: "attached" },
{ surface: "media", viewport: "desktop", width: 1440, height: 960, state: "overflow" },
{ surface: "captions", viewport: "desktop", width: 1440, height: 960, state: "workbench" },
{ surface: "editor", viewport: "desktop", width: 1440, height: 960, state: "speech-details" },
{ surface: "inspector", viewport: "desktop", width: 1440, height: 960, state: "details" },
{ surface: "inspector", viewport: "desktop", width: 1440, height: 960, state: "ai-edit" },
{ surface: "export", viewport: "desktop", width: 1440, height: 960, state: "video" },
{ surface: "agent", viewport: "desktop", width: 1440, height: 960, state: "starters" },
```

Guard CLI execution so Vitest can import the registry without launching the harness:

```js
const isDirectRun =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isDirectRun) main();
```

Import `fileURLToPath` from `node:url` beside the existing Node imports.

- [ ] **Step 4: Run the registry test**

Run: `rtk pnpm exec vitest run scripts/browser-visual-qa-scenarios.test.mjs`

Expected: PASS.

- [ ] **Step 5: Commit the matrix**

```bash
rtk git add scripts/browser-visual-qa.mjs scripts/browser-visual-qa-scenarios.test.mjs
rtk git commit -m "test(visual): cover Palmier parity states"
```

### Task 2: Add exact Palmier geometry assertions

**Files:**
- Modify: `scripts/browser-visual-qa.mjs`

- [ ] **Step 1: Add failing data hooks in component tests**

Before relying on browser geometry, require stable hooks in the relevant component tests:

```tsx
expect(screen.getByTestId("project-home-sidebar")).toBeInTheDocument();
expect(screen.getByTestId("editor-upper-deck")).toBeInTheDocument();
expect(screen.getByTestId("editor-timeline-pane")).toBeInTheDocument();
expect(screen.getByTestId("media-generation-drawer")).toBeInTheDocument();
```

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx -t "Palmier geometry hooks"`

Expected: FAIL until Plans 01–03 add the hooks.

- [ ] **Step 2: Add browser-side measurement helpers**

Add:

```js
function assertRect(selector, expected, tolerance = 1) {
  return pageCode`
const node = page.locator(${selector});
await node.waitFor({ state: "visible", timeout: 10000 });
const rect = await node.boundingBox();
if (!rect) throw new Error("Missing geometry for " + ${selector});
const expected = ${expected};
for (const [key, value] of Object.entries(expected)) {
  if (Math.abs(rect[key] - value) > ${tolerance}) {
    throw new Error(${selector} + " " + key + " expected " + value + " got " + rect[key]);
  }
}
`;
}
```

Add a ratio helper for variable workspace heights:

```js
function assertHeightRatio(selector, containerSelector, min, max) {
  return pageCode`
const rect = await page.locator(${selector}).boundingBox();
const container = await page.locator(${containerSelector}).boundingBox();
if (!rect || !container) throw new Error("Missing ratio geometry");
const ratio = rect.height / container.height;
if (ratio < ${min} || ratio > ${max}) throw new Error("Height ratio out of range: " + ratio);
`;
}
```

- [ ] **Step 3: Gate desktop home geometry**

For `home:palmier`, assert:

```js
assertRect("[data-testid='project-home-sidebar']", { width: 220 }, 1);
assertRect("[data-testid='project-card']:first-of-type", { width: 150, height: 120 }, 1);
```

Also assert the main left edge equals sidebar right edge, main inner content begins 24px later, and `assertNoHorizontalOverflow` passes.

- [ ] **Step 4: Gate editor geometry and pane budgets**

For `editor:palmier`, assert:

- Panel gaps are 4–6px.
- Header bars are 28px.
- Timeline toolbar is 38px.
- Timeline pane height ratio is 0.62–0.72.
- Track header width is 100px.
- Ruler height is 24px.
- Default track row is 50px.
- Preview canvas has no 28vh cap and fills its content cell.

For 1280×720 and 1024×768, assert no pane crosses the workspace bounds and no root overflow occurs. The 1024 state may collapse agent or inspector; the assertion must require at least Media, Preview, and Timeline remain reachable.

- [ ] **Step 5: Gate contextual surfaces**

For the remaining states, require:

```js
await page.getByRole("tablist", { name: "Source panel views" }).waitFor();
await page.getByTestId("media-generation-drawer").waitFor();
await page.getByRole("tablist", { name: "Generated source inspector views" }).waitFor();
await page.getByRole("dialog", { name: "Export" }).waitFor();
await page.getByRole("button", { name: "Generate an AI video" }).waitFor();
```

Assert the export dialog is 560×520 when the viewport allows it and remains within 16px of every viewport edge otherwise.

- [ ] **Step 6: Run focused harness tests and lint**

Run: `rtk pnpm exec vitest run scripts/browser-visual-qa-scenarios.test.mjs && rtk pnpm lint`

Expected: both commands PASS.

- [ ] **Step 7: Commit geometry gates**

```bash
rtk git add scripts/browser-visual-qa.mjs src/components/workspace/project-home.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx
rtk git commit -m "test(visual): gate Palmier editor geometry"
```

### Task 3: Create the side-by-side comparison board generator

**Files:**
- Create: `scripts/palmier-visual-comparison.mjs`
- Create: `scripts/palmier-visual-comparison.test.mjs`
- Modify: `package.json`

- [ ] **Step 1: Write missing-file and output tests**

```ts
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildComparisonBoard } from "./palmier-visual-comparison.mjs";

describe("Palmier visual comparison board", () => {
  it("embeds reference and implementation images in one self-contained board", () => {
    const dir = mkdtempSync(join(tmpdir(), "palmier-board-"));
    const png = Buffer.from("89504e470d0a1a0a", "hex");
    writeFileSync(join(dir, "reference.png"), png);
    writeFileSync(join(dir, "implementation.png"), png);

    const result = buildComparisonBoard({
      pairs: [{ id: "home", label: "Project home", reference: join(dir, "reference.png"), implementation: join(dir, "implementation.png") }],
      htmlOut: join(dir, "index.html"),
      manifestOut: join(dir, "manifest.json"),
    });

    expect(result.pairs).toHaveLength(1);
    expect(readFileSync(join(dir, "index.html"), "utf8")).toContain("data:image/png;base64,");
    expect(JSON.parse(readFileSync(join(dir, "manifest.json"), "utf8"))).toMatchObject({ pairs: [{ id: "home" }] });
  });

  it("fails closed when either side is missing", () => {
    expect(() => buildComparisonBoard({ pairs: [{ id: "missing", label: "Missing", reference: "/missing/reference.png", implementation: "/missing/implementation.png" }], htmlOut: "/tmp/board.html", manifestOut: "/tmp/manifest.json" })).toThrow(/missing/i);
  });
});
```

- [ ] **Step 2: Run the test to verify the generator is missing**

Run: `rtk pnpm exec vitest run scripts/palmier-visual-comparison.test.mjs`

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement safe pair validation and embedding**

Export:

```js
export function buildComparisonBoard({ pairs, htmlOut, manifestOut }) {
  const normalized = pairs.map((pair) => {
    for (const file of [pair.reference, pair.implementation]) {
      if (!existsSync(file) || statSync(file).size === 0) {
        throw new Error(`Missing visual comparison input: ${file}`);
      }
    }
    return {
      ...pair,
      referenceData: `data:image/png;base64,${readFileSync(pair.reference).toString("base64")}`,
      implementationData: `data:image/png;base64,${readFileSync(pair.implementation).toString("base64")}`,
    };
  });
  mkdirSync(dirname(htmlOut), { recursive: true });
  writeFileSync(htmlOut, renderBoard(normalized));
  writeFileSync(manifestOut, JSON.stringify({ generatedAt: new Date().toISOString(), pairs: normalized.map(({ referenceData, implementationData, ...pair }) => pair) }, null, 2));
  return { pairs: normalized };
}
```

Escape labels and paths before placing them in HTML. Render each pair in a two-column grid with fixed labels `Palmier reference` and `Video Creater implementation`, `object-fit: contain`, neutral `#0a0a0a` background, and no CSS filters or image transformations.

- [ ] **Step 4: Add the fixed 2026-07-13 pair map**

CLI defaults pair these files:

```js
const pairs = [
  ["home", "Project home", "01-home.png", "home-palmier-desktop.png"],
  ["editor", "Default editor", "02-editor.png", "editor-palmier-desktop.png"],
  ["export", "Video export", "03-export.png", "export-video-desktop.png"],
  ["speech-details", "Speech and details", "04-speech-details.png", "editor-speech-details-desktop.png"],
  ["captions", "Captions workbench", "05-captions.png", "captions-workbench-desktop.png"],
  ["generation", "Generation drawer", "06-generation.png", "generation-attached-desktop.png"],
  ["media-overflow", "Media overflow", "07-media-overflow.png", "media-overflow-desktop.png"],
  ["agent-editor", "Agent editor", "08-agent-editor.png", "agent-starters-desktop.png"],
  ["ai-edit", "AI Edit inspector", "09-ai-edit.png", "inspector-ai-edit-desktop.png"],
];
```

Reference root defaults to `output/parity-audit-2026-07-13/reference`; implementation root defaults to `output/parity-audit-2026-07-13/implementation`; board output defaults to `output/parity-audit-2026-07-13/comparison/index.html`.

`10-ai-edit-duplicate.png` is retained as source evidence but intentionally excluded from the unique pair map because it is pixel-identical duplicate coverage of reference 09. Record that exclusion in the generated manifest metadata.

- [ ] **Step 5: Add package scripts**

```json
"visual:qa:palmier": "node scripts/browser-visual-qa.mjs --out output/parity-audit-2026-07-13/implementation",
"visual:qa:palmier-board": "node scripts/palmier-visual-comparison.mjs"
```

- [ ] **Step 6: Run tests and generator help**

Run: `rtk pnpm exec vitest run scripts/palmier-visual-comparison.test.mjs && rtk pnpm visual:qa:palmier-board -- --help`

Expected: tests PASS and help lists `--reference`, `--implementation`, `--html-out`, and `--manifest-out`.

- [ ] **Step 7: Commit the board generator**

```bash
rtk git add scripts/palmier-visual-comparison.mjs scripts/palmier-visual-comparison.test.mjs package.json
rtk git commit -m "test(visual): add Palmier comparison board"
```

### Task 4: Capture every approved state

**Files:**
- Evidence only: `output/parity-audit-2026-07-13/implementation/`

- [ ] **Step 1: Start the deterministic Vite app**

Run in a retained terminal session: `rtk pnpm dev -- --port 1420`

Expected: Vite reports `http://127.0.0.1:1420` and remains running.

- [ ] **Step 2: Run the Palmier visual harness**

Run: `rtk pnpm visual:qa:palmier`

Expected: the harness writes every approved PNG, rejects blank roots and horizontal overflow, and exits 0.

- [ ] **Step 3: Verify required captures exist and are non-empty**

Run:

```bash
rtk ls -lh output/parity-audit-2026-07-13/implementation
rtk find output/parity-audit-2026-07-13/implementation -name '*.png' -size 0 -print
```

Expected: home, editor, generation, captions, details, AI Edit, export, media overflow, and agent starter captures exist; the zero-size search prints nothing.

- [ ] **Step 4: Generate the comparison board**

Run: `rtk pnpm visual:qa:palmier-board`

Expected: writes `output/parity-audit-2026-07-13/comparison/index.html` and `manifest.json` with nine complete pairs.

### Task 5: Perform the visual design QA loop

**Files:**
- Modify only the component/test files responsible for visible mismatches.
- Evidence: `output/parity-audit-2026-07-13/comparison/`

- [ ] **Step 1: Serve and open the self-contained board in the chosen in-app browser**

Run in a retained terminal session:

```bash
rtk node -e 'const http=require("node:http"),fs=require("node:fs"),path=require("node:path");const root=path.resolve("output/parity-audit-2026-07-13/comparison");http.createServer((req,res)=>{const target=path.resolve(root,"."+decodeURIComponent(req.url==="/"?"/index.html":req.url));if(!target.startsWith(root+path.sep)&&target!==root){res.writeHead(403);return res.end("Forbidden")}fs.readFile(target,(error,data)=>{if(error){res.writeHead(404);return res.end("Not found")}res.writeHead(200,{"Content-Type":target.endsWith(".html")?"text/html; charset=utf-8":"application/octet-stream"});res.end(data)})}).listen(1421,"127.0.0.1",()=>console.log("http://127.0.0.1:1421"))'
```

Open `http://127.0.0.1:1421` in the in-app browser.

Expected: every Palmier image and Video Creater image appears together in one row at natural aspect ratio.

- [ ] **Step 2: Inspect every pair using the same checklist**

For each pair record pass/fail for:

- Overall pane proportions and dominant visual hierarchy.
- Typography family, size, weight, and tracking.
- Padding, margins, panel gaps, and row height.
- Borders, dividers, radii, shadows, and material opacity.
- Image crop/aspect and unused space.
- Active, hover, disabled, busy, error, and selected states.
- Horizontal/vertical clipping and overflow.

Write the verdict into `output/parity-audit-2026-07-13/comparison/review.json` using:

```json
{
  "pairs": [
    {
      "id": "home",
      "status": "pass",
      "reviewedAt": "2026-07-13T00:00:00Z",
      "notes": []
    }
  ]
}
```

- [ ] **Step 3: Fix one visible mismatch at a time with TDD**

For each failing pair:

1. Add or tighten one focused component/geometry assertion.
2. Run it and confirm failure.
3. Apply the smallest component/style correction.
4. Run the focused test and confirm pass.
5. Recapture only the affected scenario.
6. Regenerate the board and reinspect.

Do not change a reference image, apply CSS filters, or loosen geometry thresholds to manufacture a pass.

- [ ] **Step 4: Require all comparison pairs to pass**

Run: `rtk node -e 'const r=require("./output/parity-audit-2026-07-13/comparison/review.json"); if(r.pairs.some((p)=>p.status!=="pass")) process.exit(1)'`

Expected: exit 0 only when every pair is recorded as pass.

- [ ] **Step 5: Commit each coherent correction**

Example for a preview correction:

```bash
rtk git add src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx
rtk git commit -m "fix(preview): match Palmier canvas allocation"
```

Use the actual component scope in each Conventional Commit; do not commit ignored comparison images.

### Task 6: Run full functional and native verification

**Files:**
- No planned source changes; fix failures in their owning slice and commit separately.

- [ ] **Step 1: Run the complete frontend test suite**

Run: `rtk pnpm test`

Expected: every Vitest file passes with zero failures.

- [ ] **Step 2: Run static and production build checks**

Run: `rtk pnpm lint && rtk pnpm build && rtk git diff --check`

Expected: TypeScript, Vite production build, and whitespace checks all exit 0.

- [ ] **Step 3: Run relevant Rust tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml project:: -- --test-threads=1
rtk cargo test --manifest-path src-tauri/Cargo.toml timeline:: -- --test-threads=1
```

Expected: project and timeline tests pass in their separate filtered runs.

- [ ] **Step 4: Run the complete Rust library suite**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1`

Expected: all library tests pass.

- [ ] **Step 5: Verify the native runtime and build the debug app**

Run: `rtk pnpm check:native-runtime && rtk pnpm tauri build --debug`

Expected: runtime manifest verification exits 0 and a debug `.app` is produced under `src-tauri/target/debug/bundle/macos/`.

- [ ] **Step 6: Verify the exact debug bundle launches**

Run:

```bash
rtk open "src-tauri/target/debug/bundle/macos/Video Creater.app"
rtk pgrep -fl "Video Creater"
```

Expected: a retained Video Creater process is listed. Close only the process launched for this check after capturing any native-only titlebar evidence.

- [ ] **Step 7: Run the release visual regression policy**

After intentionally updating the committed browser baseline in its own reviewed commit, run: `rtk pnpm visual:qa:browser-release`

Expected: comparison manifest and platform policy pass with zero unreviewed mismatches.

### Task 7: Reconcile the authoritative parity tracker

**Files:**
- Modify: `docs/parity.md`

- [ ] **Step 1: Replace implementation-only wording with exact evidence**

For each approved state, record:

- The focused component and test files.
- The full frontend/lint/build/Rust result.
- The exact ignored capture directory.
- The exact comparison board and review manifest.
- The exact viewport/state names.
- Any remaining account, matte, provider, notarization, or clean-machine limitation.

- [ ] **Step 2: Add a visual-verification table**

```markdown
| State | Reference | Implementation | Viewport | Review |
| --- | --- | --- | --- | --- |
| Home | `reference/01-home.png` | `implementation/home-palmier-desktop.png` | 1440×960 | Passed side-by-side review |
| Editor | `reference/02-editor.png` | `implementation/editor-palmier-desktop.png` | 1440×960 | Passed side-by-side review |
| Export | `reference/03-export.png` | `implementation/export-video-desktop.png` | 1440×960 | Passed side-by-side review |
```

Add the remaining six pairs with their exact filenames. Do not mark a row passed unless `review.json` records `status: "pass"`.

- [ ] **Step 3: Record Palmier gaps and Video Creater-only capabilities separately**

Keep account sign-in and Create Matte as missing unless implemented functionally. Keep Video Creater-only codec, render, provider, transcript, semantic-search, local-model, graphics-contract, and agent-review capabilities in their relevant sections without removing them from the UI.

- [ ] **Step 4: Verify tracker and evidence references**

Run:

```bash
rtk rg -n "parity-audit-2026-07-13|Passed side-by-side review|Video Creater-only|account|Create Matte" docs/parity.md
rtk git diff --check -- docs/parity.md
```

Expected: every evidence path exists locally, remaining gaps are explicit, and no whitespace errors occur.

- [ ] **Step 5: Commit final tracker truth**

```bash
rtk git add docs/parity.md
rtk git commit -m "docs(parity): record Palmier visual E2E evidence"
```

### Task 8: Final clean-scope audit

**Files:**
- No source changes expected.

- [ ] **Step 1: Review commits and worktree ownership**

Run: `rtk git log --oneline --decorate -30 && rtk git status --short`

Expected: every parity slice is a Conventional Commit; unrelated pre-existing dirty files remain unstaged and untouched.

- [ ] **Step 2: Verify ignored screenshot storage**

Run: `rtk git check-ignore -v output/parity-audit-2026-07-13/reference/01-home.png output/parity-audit-2026-07-13/implementation/editor-palmier-desktop.png`

Expected: both paths are ignored by the repository's output rule.

- [ ] **Step 3: Summarize exact closure and remaining external limits**

The handoff must state:

- Which screens passed same-state visual review.
- Exact frontend, build, Rust, native bundle, and browser results.
- Exact ignored evidence directory.
- Any remaining account/Create Matte/provider/notarization/clean-machine gaps.
- That Video Creater-only features were preserved.
