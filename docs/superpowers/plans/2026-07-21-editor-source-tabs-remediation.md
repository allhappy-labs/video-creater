# Editor Source Tabs Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the timeline stable and useful while making every left-rail source destination responsive, direct, non-duplicative, accessible, and verifiably console-clean.

**Architecture:** `WorkspaceLayoutState` becomes the only owner of desktop deck geometry and persists a bounded timeline ratio. The existing source panel becomes an inline-size query container; each destination continues to use existing project actions while focused components own local draft or navigation behavior. Browser visual QA measures the real rendered panes and treats unexpected page/console errors as failures.

**Tech Stack:** React 19, TypeScript 5.9, Tailwind CSS 3.4 plus native CSS container queries, shadcn-style local UI primitives, lucide-react, Vitest, Testing Library, Playwright browser QA, existing Tauri project actions.

## Global Constraints

- Prefix every shell command with `rtk`.
- Preserve project, timeline, media, transcript, template, generated-asset, and Rust schemas.
- Preserve canonical project actions, undo/redo, native validation, Media folder/source flows, inspector routing, and compact single-pane navigation.
- The active source destination must never choose workspace row geometry.
- Store `timelineRatio` in `video-creater.workspace-layout.v1`, default it to `0.45`, and clamp it to `0.30–0.70`.
- Use source-container inline size, never browser viewport breakpoints, for source-panel card and footer layout.
- Explicit editor Insert actions use the current playhead; timeline drops keep their explicit drop time.
- Generate is the only complete image/video/audio generation composer.
- All new icon-only controls require accessible labels and hover/focus help.
- No task is complete without a witnessed RED test, the minimal GREEN implementation, focused regression tests, and a Conventional Commit.
- Browser evidence is reported separately from packaged Tauri or physical-device evidence.

---

### Task 1: Stable Persisted Timeline Deck Geometry

**Files:**
- Modify: `src/lib/workspace-layout-state.ts`
- Modify: `src/lib/workspace-layout-state.test.ts`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: existing `WorkspaceLayoutState`, `workspaceLayoutReducer`, `workspacePresetGridTemplate`, `WorkspacePaneResizeHandle`, and workspace persistence effect.
- Produces: `timelineRatio: number`, `{ type: "resizeTimeline"; ratio: number }`, `workspaceTimelineRatioBounds`, and an accessible `Resize timeline deck` separator.

- [ ] **Step 1: Write reducer tests for default, parsing, clamping, persistence, and preset preservation**

Add expectations to `workspace-layout-state.test.ts`:

```ts
expect(defaultWorkspaceLayoutState.timelineRatio).toBe(0.45);
expect(workspaceLayoutReducer(defaultWorkspaceLayoutState, {
  type: "resizeTimeline",
  ratio: 0.1,
}).timelineRatio).toBe(0.3);
expect(workspaceLayoutReducer(defaultWorkspaceLayoutState, {
  type: "resizeTimeline",
  ratio: 0.9,
}).timelineRatio).toBe(0.7);
expect(parseWorkspaceLayoutState({ timelineRatio: "bad" }).timelineRatio).toBe(0.45);
expect(parseWorkspaceLayoutState({ timelineRatio: 0.6 }).timelineRatio).toBe(0.6);
expect(workspacePresetGridTemplate({
  ...defaultWorkspaceLayoutState,
  timelineRatio: 0.6,
}).rows).toBe("minmax(17rem, 40fr) minmax(16rem, 60fr)");
```

- [ ] **Step 2: Run the reducer tests and verify RED**

Run:

```bash
rtk pnpm vitest run src/lib/workspace-layout-state.test.ts --reporter=verbose
```

Expected: TypeScript/test failures report missing `timelineRatio`, `resizeTimeline`, or ratio-derived rows.

- [ ] **Step 3: Implement the bounded state and row template**

Add:

```ts
export const workspaceTimelineRatioBounds = { min: 0.3, max: 0.7 } as const;

function clampTimelineRatio(value: number) {
  const finite = Number.isFinite(value) ? value : defaultWorkspaceLayoutState.timelineRatio;
  return Math.min(workspaceTimelineRatioBounds.max, Math.max(workspaceTimelineRatioBounds.min, finite));
}
```

Extend the state/action/parser and build default/media rows with integer percentage weights:

```ts
const lowerWeight = Math.round(state.timelineRatio * 100);
const upperWeight = 100 - lowerWeight;
const rows = `minmax(17rem, ${upperWeight}fr) minmax(16rem, ${lowerWeight}fr)`;
```

Keep Vertical's areas but use the same ratio-derived weights.

- [ ] **Step 4: Verify reducer tests GREEN**

Run the Step 2 command. Expected: all layout-state tests pass.

- [ ] **Step 5: Write workspace RED tests for tab-stable rows and the separator**

In `editor-workspace.test.tsx`, render desktop mode and assert:

```tsx
const grid = screen.getByTestId("workspace-preset-grid");
const initialRows = grid.style.gridTemplateRows;
for (const label of ["Generate", "Templates", "Text", "Captions", "Transcript", "Audio", "Effects"]) {
  fireEvent.click(screen.getByRole("button", { name: label }));
  expect(grid.style.gridTemplateRows).toBe(initialRows);
}

const separator = screen.getByRole("separator", { name: "Resize timeline deck" });
expect(separator).toHaveAttribute("aria-orientation", "horizontal");
fireEvent.keyDown(separator, { key: "ArrowUp" });
expect(separator).toHaveAttribute("aria-valuenow", "47");
```

Add a remount assertion that local storage restores the new ratio and a pointer test that ignores a non-owning `pointerId`.

- [ ] **Step 6: Run the workspace tests and verify RED**

Run:

```bash
rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx -t "keeps workspace rows stable across source destinations|resizes and persists the timeline deck" --reporter=verbose
```

Expected: rows change after Generate and no timeline separator exists.

- [ ] **Step 7: Remove tab-derived rows and implement the separator**

Delete `sourceToolExpanded` and `workspacePresetRows`; pass `workspacePresetGrid.rows` directly. Add a horizontal pointer/keyboard separator at the timeline top edge, dispatching `resizeTimeline` with the available grid height. Use the same pointer ownership and cleanup pattern as `WorkspacePaneResizeHandle`, with 16-pixel and Shift+64-pixel keyboard steps. Hide it in maximize and single-pane modes.

- [ ] **Step 8: Verify workspace and reducer GREEN**

Run:

```bash
rtk pnpm vitest run src/lib/workspace-layout-state.test.ts src/components/workspace/editor-workspace.test.tsx --reporter=dot
```

Expected: both files pass.

- [ ] **Step 9: Commit Task 1**

```bash
rtk git add src/lib/workspace-layout-state.ts src/lib/workspace-layout-state.test.ts src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "fix(editor): stabilize timeline deck geometry"
```

### Task 2: Source-Container Responsive Generate, Templates, And Captions

**Files:**
- Modify: `src/index.css`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/motion-template-library.tsx`
- Modify: `src/components/workspace/motion-template-library.test.tsx`
- Modify: `src/components/workspace/captions-workbench.tsx`
- Modify: `src/components/workspace/captions-workbench.test.tsx`
- Modify: `src/captions-workbench-layout.test.ts`

**Interfaces:**
- Consumes: `source-library-scroll-region`, `media-generation-drawer`, `MotionTemplateLibrary`, and `CaptionsWorkbench.preview`.
- Produces: `.editor-source-container`, `.editor-source-responsive-grid`, `.editor-generation-footer-grid`, one owned caption preview frame, and sticky source action bars.

- [ ] **Step 1: Write RED component/source-policy tests**

Assert the source aside has `editor-source-container`; Generate header/footer have `sticky` without `xl:sticky`; tuning controls use `editor-generation-footer-grid` without `sm:grid-cols-4`; Templates uses `editor-template-grid` without fixed `grid-cols-2`; and the caption preview root has one `data-testid="caption-preview-frame"` descendant with no nested `aspect-video` caller.

- [ ] **Step 2: Run the responsive tests and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/motion-template-library.test.tsx src/components/workspace/captions-workbench.test.tsx src/captions-workbench-layout.test.ts --reporter=verbose
```

Expected: missing container classes, viewport breakpoint classes still present, and two aspect-ratio wrappers remain.

- [ ] **Step 3: Implement source-local layout classes**

Add native CSS:

```css
.editor-source-container { container: editor-source / inline-size; }
.editor-template-grid,
.editor-generation-footer-grid { display: grid; grid-template-columns: minmax(0, 1fr); }
@container editor-source (min-width: 360px) {
  .editor-template-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .editor-generation-footer-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
```

At widths under 360 pixels, keep the generation queue action on its own row. Apply `min-w-0`, wrapping status copy, and source-relative `sticky top-0`/`sticky bottom-0` to the generation header/footer. Do not add viewport variants.

- [ ] **Step 4: Remove the nested caption frame**

Keep `aspect-video` only on `CaptionsWorkbench`'s `data-testid="caption-preview-frame"`. Change the workspace preview prop to content-only:

```tsx
preview={<span className="px-4 py-3 text-[11px] font-semibold text-white">Caption preview</span>}
```

Make the caption action row `sticky bottom-0 z-10 bg-background/95 backdrop-blur`.

- [ ] **Step 5: Verify responsive component tests GREEN**

Run the Step 2 command. Expected: all focused files pass.

- [ ] **Step 6: Commit Task 2**

```bash
rtk git add src/index.css src/components/workspace/editor-workspace.tsx src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/motion-template-library.tsx src/components/workspace/motion-template-library.test.tsx src/components/workspace/captions-workbench.tsx src/components/workspace/captions-workbench.test.tsx src/captions-workbench-layout.test.ts
rtk git commit -m "fix(editor): make source tools panel responsive"
```

### Task 3: Playhead-Correct Templates And Actionable Text Treatments

**Files:**
- Modify: `src/components/workspace/motion-template-library.tsx`
- Modify: `src/components/workspace/motion-template-library.test.tsx`
- Modify: `src/components/workspace/text-library-panel.tsx`
- Modify: `src/components/workspace/text-library-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: `timelinePlayheadSeconds`, `insertTemplate(templateId, startSeconds?)`, `insertShaderBackgroundTemplate(templateId, startSeconds?)`, and motion template definitions.
- Produces: `playheadSeconds: number` on `MotionTemplateLibrary`, `onInsertTreatment(templateId: string)` on `TextLibraryPanel`, explicit Style/Places-on metadata, and `templateInsertError: string | null` for missing-track recovery.

- [ ] **Step 1: Write RED library tests**

Assert:

```tsx
render(<MotionTemplateLibrary templates={[template]} playheadSeconds={1.325} onInsertTemplate={onInsert} />);
expect(screen.getByRole("button", { name: "Insert Kinetic Lower Third at 00:01.325" })).toBeVisible();
fireEvent.click(screen.getByRole("button", { name: /Insert Kinetic Lower Third/ }));
expect(onInsert).toHaveBeenCalledWith(template.id, 1.325);
expect(screen.getByText("Style: Captions")).toBeVisible();
expect(screen.getByText("Places on: Overlays")).toBeVisible();
```

For Text, click a treatment and expect `onInsertTreatment(treatment.id)` while retaining Add Text and selection-dependent inspector routing.

- [ ] **Step 2: Run library tests and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/motion-template-library.test.tsx src/components/workspace/text-library-panel.test.tsx --reporter=verbose
```

Expected: missing props/actions and old unlabeled metadata.

- [ ] **Step 3: Implement library props, previews, and actions**

Change callbacks to `(templateId: string, startSeconds: number) => void`, format the playhead with the existing editor time formatter, label metadata as `Style:` and `Places on:`, and render treatments as compact buttons with `Add at playhead`. Keep drag payloads unchanged.

- [ ] **Step 4: Verify library tests GREEN**

Run the Step 2 command. Expected: both files pass.

- [ ] **Step 5: Write workspace RED tests for every editor insertion entrance**

Set the playhead to 1.325 seconds, then assert source Templates, Text treatment, timeline template drawer, shader-background insertion, and the template suggestion each issue an `addItems` action whose first item starts at 1.325. Retain the existing drop test expecting its explicit dropped time. Render a project without the compatible Overlay or HyperFrame track and assert the panel shows `Add an Overlay track before inserting this template` or `Add a HyperFrames track before inserting this background` without issuing an action.

- [ ] **Step 6: Run workspace insertion tests and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx -t "inserts source templates at the playhead|inserts text treatments at the playhead|keeps template drop time authoritative" --reporter=verbose
```

Expected: button/suggestion insertions start at zero.

- [ ] **Step 7: Route all explicit Insert actions through the playhead**

Pass `timelinePlayheadSeconds` to both libraries and call `insertTemplate(id, timelinePlayheadSeconds)` or `insertShaderBackgroundTemplate(id, timelinePlayheadSeconds)` from source, drawer, suggestion, and Text. Keep the timeline `onTemplateDrop`/`onShaderBackgroundDrop` callbacks unchanged. Set `templateInsertError` before returning from either missing-track branch, clear it before a new attempt and after success, and render it as a source-panel `role="alert"` beside the insertion controls.

- [ ] **Step 8: Verify workspace insertion GREEN and commit**

```bash
rtk pnpm vitest run src/components/workspace/motion-template-library.test.tsx src/components/workspace/text-library-panel.test.tsx src/components/workspace/editor-workspace.test.tsx --reporter=dot
rtk git add src/components/workspace/motion-template-library.tsx src/components/workspace/motion-template-library.test.tsx src/components/workspace/text-library-panel.tsx src/components/workspace/text-library-panel.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "fix(editor): insert source assets at playhead"
```

### Task 4: Dense Seekable Batch Transcript Editing

**Files:**
- Modify: `src/components/workspace/transcript-panel.tsx`
- Modify: `src/components/workspace/transcript-panel.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: `Transcript`, `TranscriptWordRepairInput`, shared playhead setter, and the existing `editTranscriptWords` project action.
- Produces: `onSeek(seconds: number)`, `onApplyMany(inputs: readonly TranscriptWordRepairInput[])`, local word drafts, and sticky batch actions.

- [ ] **Step 1: Write TranscriptPanel RED tests**

Test that compact rows expose time buttons; two edited rows produce one `onApplyMany` call in word-index order; Discard restores canonical values; invalid end-before-start displays an alert and disables Apply; and repaired/modified indicators are visible without per-row Apply buttons.

```tsx
fireEvent.change(screen.getByLabelText("Word 1 text"), { target: { value: "Hello" } });
fireEvent.change(screen.getByLabelText("Word 2 text"), { target: { value: "world" } });
fireEvent.click(screen.getByRole("button", { name: "Apply 2 changes" }));
expect(onApplyMany).toHaveBeenCalledWith([
  expect.objectContaining({ wordIndex: 0, text: "Hello" }),
  expect.objectContaining({ wordIndex: 1, text: "world" }),
]);
```

- [ ] **Step 2: Run transcript tests and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/transcript-panel.test.tsx --reporter=verbose
```

Expected: no seek/batch/discard controls and one Apply per card.

- [ ] **Step 3: Implement local drafts and batch validation**

Replace `TranscriptWordRow` local state with a panel-level `Record<number, WordDraft>`. Compute changed inputs against canonical words, validate text/non-negative/strictly increasing times, and render one compact grid row. Keep labels explicit and use a sticky footer with `Apply N changes` and `Discard changes`.

- [ ] **Step 4: Verify transcript component GREEN**

Run the Step 2 command. Expected: all transcript tests pass.

- [ ] **Step 5: Write workspace RED tests for seek and one canonical batch action**

Edit two transcript rows, Apply, and assert one invocation with:

```ts
expect.objectContaining({
  action: expect.objectContaining({
    type: "editTranscriptWords",
    edits: [
      expect.objectContaining({ wordIndex: 0 }),
      expect.objectContaining({ wordIndex: 1 }),
    ],
  }),
})
```

Click the first word time and assert the shared playhead time badge changes to that word's start.

- [ ] **Step 6: Run workspace transcript tests and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/editor-workspace.test.tsx -t "applies transcript repairs as one action|seeks from a transcript word" --reporter=verbose
```

Expected: callback signature mismatch or multiple individual actions.

- [ ] **Step 7: Implement workspace batch/seek routing and commit**

Build all edits with one timestamp and unique repair IDs, issue one `editTranscriptWords` action, and pass `setTimelinePlayheadSeconds` through a clamped `onSeek` callback.

```bash
rtk pnpm vitest run src/components/workspace/transcript-panel.test.tsx src/components/workspace/editor-workspace.test.tsx --reporter=dot
rtk git add src/components/workspace/transcript-panel.tsx src/components/workspace/transcript-panel.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "feat(transcript): add compact batch repair flow"
```

### Task 5: Canonical Music Routing, Effects Recovery, And Speech Help

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`
- Modify: `src/components/workspace/effect-catalog-panel.tsx`
- Modify: `src/components/workspace/effect-catalog-panel.test.tsx`
- Modify: `src/components/workspace/speech-workbench.tsx`
- Modify: `src/components/workspace/speech-workbench.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: generated-asset output/category metadata, `setSourceDestination`, `setGenerationComposerOpen`, MediaBin's internal generation mode, timeline focus helpers, and existing speech actions.
- Produces: `onOpenGenerateAudio()`, `onChooseVisualClip()`, and a reusable local hover/focus help pattern.

- [ ] **Step 1: Write RED tests for Audio > Music**

Render external Audio, open Music, and assert `queryByRole("region", { name: "Media generation" })` is absent; music outputs are shown; non-music audio is absent; `Generate music` calls `onOpenGenerateAudio`.

- [ ] **Step 2: Write RED tests for Effects and Speech recovery/help**

Assert `Choose a visual clip` appears only with no target and invokes its callback. For Mark Speakers and Mark Silence, focus/click the help buttons and expect role `tooltip` copy describing analysis and explicit timeline actions.

- [ ] **Step 3: Run focused tests and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/effect-catalog-panel.test.tsx src/components/workspace/speech-workbench.test.tsx --reporter=verbose
```

Expected: Music embeds generation, Effects has no recovery button, and Info icons are not controls.

- [ ] **Step 4: Implement focused destination behavior**

Resolve music media IDs by matching completed generated-asset outputs whose `settings.category === "music"`; render those assets and the generation shortcut. Add the Effects recovery button without auto-selection. Replace decorative Info icons with labeled help buttons whose tooltip is shown for hover and focus and linked through `aria-describedby`.

- [ ] **Step 5: Verify focused components GREEN**

Run the Step 3 command. Expected: all three files pass.

- [ ] **Step 6: Write workspace RED routing/focus tests**

Open Audio > Music, click Generate music, and assert Generate is active, Audio mode is pressed, and Generation prompt has focus. With no effect target, click Choose a visual clip and assert Timeline receives focus and the live region announces eligible clip kinds.

- [ ] **Step 7: Implement routing and focus recovery**

Use a workspace routing helper:

```ts
function openGenerateAudio() {
  setSourceDestination("generate");
  setSourcePanelOpen(true);
  setGenerationComposerOpen(true);
}
```

Inside MediaBin, the Music shortcut first calls its existing `setGenerationMode("audio")` and `setGenerationComposerOpen(true)`, records a one-shot prompt-focus flag, then calls `onOpenGenerateAudio`. When `activeDestination` becomes `generate`, focus the existing prompt ref and clear the flag; do not query an arbitrary input. Effects recovery switches compact view to timeline, focuses the named timeline region, and updates the existing polite status message without project mutation.

- [ ] **Step 8: Verify workspace GREEN and commit**

```bash
rtk pnpm vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/effect-catalog-panel.test.tsx src/components/workspace/speech-workbench.test.tsx src/components/workspace/editor-workspace.test.tsx --reporter=dot
rtk git add src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/effect-catalog-panel.tsx src/components/workspace/effect-catalog-panel.test.tsx src/components/workspace/speech-workbench.tsx src/components/workspace/speech-workbench.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "fix(editor): clarify audio and effects workflows"
```

### Task 6: Rail Grouping, Tooltips, And Single-Row Header Ownership

**Files:**
- Create: `src/components/ui/editor-tooltip.tsx`
- Create: `src/components/ui/editor-tooltip.test.tsx`
- Modify: `src/components/workspace/editor-navigation-rail.tsx`
- Modify: `src/components/workspace/editor-navigation-rail.test.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx`
- Modify: `src/components/workspace/editor-workspace.test.tsx`

**Interfaces:**
- Consumes: current rail item array and roving tab-stop logic, workspace layout budget, top-bar controls.
- Produces: `EditorTooltip`, three semantic rail groups with separators, one desktop owner per primary destination, and compact-only duplicate controls.

- [ ] **Step 1: Write EditorTooltip RED tests**

Define the wished-for interface:

```tsx
<EditorTooltip label="Media library">
  <button type="button">Media</button>
</EditorTooltip>
```

Assert the tooltip appears on hover and focus, disappears on Escape/blur, has role `tooltip`, and its ID is referenced by `aria-describedby` while visible.

- [ ] **Step 2: Run tooltip tests and verify RED**

```bash
rtk pnpm vitest run src/components/ui/editor-tooltip.test.tsx --reporter=verbose
```

Expected: module does not exist.

- [ ] **Step 3: Implement the minimal tooltip primitive**

Use React state and `cloneElement` to preserve the child's existing handlers while adding hover/focus/Escape behavior. Render a fixed-position dark editor tooltip with reduced-motion-safe opacity transition. Do not add a dependency.

- [ ] **Step 4: Write rail/header RED tests**

Assert three groups named Global, Sources, and Project; separators do not enter the keyboard item list; labels use `text-[11px] text-white/75`; Activity status is in the accessible name and visible badge/dot, not a 9-pixel second line; every item exposes `EditorTooltip`; desktop header has no Home/Codex duplicate; compact mode has exactly one equivalent control; and the header has `flex-nowrap`/single-row classes at 1024-plus layout.

- [ ] **Step 5: Run rail/workspace tests and verify RED**

```bash
rtk pnpm vitest run src/components/ui/editor-tooltip.test.tsx src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-workspace.test.tsx -t "tooltip|groups destinations|keeps primary navigation single-owned|keeps the constrained desktop header on one row" --reporter=verbose
```

Expected: no groups/tooltips and duplicate header controls remain.

- [ ] **Step 6: Implement rail grouping and header ownership**

Render grouped item slices inside labeled containers while keeping one flattened `items` list for Arrow/Home/End behavior. Use the tooltip primitive for each button. Replace Activity status copy with a visible dot/badge and full accessible label. Hide header Home/Codex controls in multi-pane layouts and render them only in the compact header; allow project identity to truncate and move secondary layout controls into the existing menu before Export/render status.

- [ ] **Step 7: Verify rail/header GREEN and commit**

```bash
rtk pnpm vitest run src/components/ui/editor-tooltip.test.tsx src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-workspace.test.tsx --reporter=dot
rtk git add src/components/ui/editor-tooltip.tsx src/components/ui/editor-tooltip.test.tsx src/components/workspace/editor-navigation-rail.tsx src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
rtk git commit -m "fix(editor): clarify navigation ownership"
```

### Task 7: Console-Clean Browser QA And Overflow Evidence

**Files:**
- Modify: `src/main.tsx`
- Modify: `src/lib/settings-acceptance-runner.ts`
- Modify: `src/lib/settings-acceptance-runner.test.ts`
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `src/browser-visual-qa-script.test.ts`
- Modify: `src/browser-visual-qa-palmier-scenarios.test.ts`
- Modify: `src/lib/modern-editor-visual-qa-fixtures.ts`
- Modify: `src/lib/modern-editor-visual-qa-fixtures.test.ts`

**Interfaces:**
- Consumes: visual-QA init scripts, settings acceptance enablement, scenario definitions, Playwright page events, and source-destination controls.
- Produces: `window.__VIDEO_CREATER_BROWSER_VISUAL_QA__`, zero-error enforcement, source-panel overflow metrics, and complete source/mode/subtab screenshot scenarios.

- [ ] **Step 1: Write RED tests for visual-QA bootstrap isolation**

Add a browser marker test proving `runSettingsAcceptanceIfEnabled()` returns before touching Tauri when `__VIDEO_CREATER_BROWSER_VISUAL_QA__ === true`, while the existing packaged acceptance marker still runs unchanged.

- [ ] **Step 2: Write RED harness source-policy tests**

Assert the browser script installs the marker before page bootstrap, registers `page.on("pageerror")` and `page.on("console")`, rejects unexpected `console.error`, records `{ clientWidth, scrollWidth }` for the source root/footer, and declares screenshots for every source destination, every Generate mode, and Audio Library/Speech/Music. Add a fixture-bridge test proving any advertised event-listener API returns a callable cleanup function; otherwise the fixture omits that API and lets browser fallback execute.

- [ ] **Step 3: Run the harness tests and verify RED**

```bash
rtk pnpm vitest run src/lib/settings-acceptance-runner.test.ts src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts src/lib/modern-editor-visual-qa-fixtures.test.ts --reporter=verbose
```

Expected: browser marker/error enforcement/overflow evidence and subtab scenarios are absent.

- [ ] **Step 4: Implement explicit browser-QA isolation and error enforcement**

Install the marker with `page.addInitScript` before navigation. Make only the browser-QA marker bypass packaged Settings acceptance. Capture page errors and error-level console messages; after each scenario, fail with their complete text unless the scenario explicitly declares a known failure allowlist. Do not regex-suppress Settings errors.

- [ ] **Step 5: Add interaction and overflow evidence**

For each live destination, click the rail control, wait for its named region, and iterate source widths `[240, 260, 304, 400]` by temporarily setting the source aside and its grid column to the exact test width. At each width evaluate:

```js
({ clientWidth: node.clientWidth, scrollWidth: node.scrollWidth, overflow: node.scrollWidth - node.clientWidth })
```

Fail when overflow exceeds one pixel. While switching destinations at the default width, record the workspace grid rows, separator top, and timeline height and fail when any differs by more than one CSS pixel from Media. Add deterministic scenario controls for Generate Image/Video/Audio and Audio Library/Speech/Music, plus the 1024×720, 1100×800, and 900×900 acceptance viewports.

- [ ] **Step 6: Verify harness tests GREEN and commit**

```bash
rtk pnpm vitest run src/lib/settings-acceptance-runner.test.ts src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts src/lib/modern-editor-visual-qa-fixtures.test.ts --reporter=dot
rtk git add src/main.tsx src/lib/settings-acceptance-runner.ts src/lib/settings-acceptance-runner.test.ts scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts src/lib/modern-editor-visual-qa-fixtures.ts src/lib/modern-editor-visual-qa-fixtures.test.ts
rtk git commit -m "test(editor): enforce clean source tab visual qa"
```

### Task 8: Full Verification And Evidence Review

**Files:**
- Modify only if verification exposes a regression; any fix starts with a focused failing test in the owning task's test file.
- Produce ignored evidence under `output/playwright/editor-source-tabs-remediation-2026-07-21/`.

**Interfaces:**
- Consumes: all Task 1–7 behavior and visual-QA scenarios.
- Produces: fresh test/build/policy logs, screenshots, interaction metrics, and a requirement-by-requirement result matrix.

- [ ] **Step 1: Run all focused source/editor tests**

```bash
rtk pnpm vitest run src/lib/workspace-layout-state.test.ts src/components/workspace/editor-navigation-rail.test.tsx src/components/workspace/editor-workspace.test.tsx src/components/workspace/media-bin.test.tsx src/components/workspace/motion-template-library.test.tsx src/components/workspace/text-library-panel.test.tsx src/components/workspace/captions-workbench.test.tsx src/captions-workbench-layout.test.ts src/components/workspace/transcript-panel.test.tsx src/components/workspace/speech-workbench.test.tsx src/components/workspace/effect-catalog-panel.test.tsx src/lib/settings-acceptance-runner.test.ts src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts src/lib/modern-editor-visual-qa-fixtures.test.ts --reporter=dot
```

Expected: exit 0 with zero failed tests and no unexpected warnings/errors.

- [ ] **Step 2: Run the complete frontend suite**

```bash
rtk pnpm test
```

Expected: exit 0 with zero failed test files.

- [ ] **Step 3: Run lint, production build, and source policy**

```bash
rtk pnpm lint
rtk pnpm build
rtk pnpm test:source-quality
```

Expected: all commands exit 0.

- [ ] **Step 4: Run the live browser screenshot matrix**

```bash
rtk pnpm visual:qa:browser -- --out output/playwright/editor-source-tabs-remediation-2026-07-21
```

Expected: every source/mode/subtab scenario completes with zero unexpected page/console errors and overflow no greater than one pixel.

- [ ] **Step 5: Inspect every acceptance screenshot**

Open the desktop eight-destination captures, Generate modes, Audio subtabs, 1100×800 default, 1024×720 Generate, and 900×900 single-pane captures. Record explicit pass/fail for clipping, blank/nested preview, header wrapping, timeline collapse, rail duplication, action visibility, text contrast, and horizontal scroll.

- [ ] **Step 6: Verify the committed diff and repository state**

```bash
rtk git diff --check
rtk git status --short --branch
rtk git log --oneline --decorate -10
```

Expected: no unstaged implementation changes, no whitespace errors, and only the dedicated remediation commits on the feature branch.
