# Timeline Interaction Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make timeline clips readable and directly resizable, keep interaction feedback and vertical context inside the timeline, and prove committed edits and long-project responsiveness in headed browser QA.

**Architecture:** Keep the existing accessible DOM timeline, viewport virtualization, and canonical edit evaluator. `TimelineItemShell` owns visual hierarchy and resize presentation, `timeline-viewport.ts` owns the pure visible-track calculation, `TimelineEditor` coordinates editability and vertical scroll state, and `browser-visual-qa.mjs` proves release/undo and deterministic performance contracts.

**Tech Stack:** React 19, TypeScript 5.9, Tailwind CSS 3.4, Vitest, Testing Library, Playwright CLI browser QA, Rust project-action tests.

## Global Constraints

- Preserve the timeline/project schemas and canonical move, resize, trim, snap, collision, ripple, and undo semantics.
- Preserve pointer, keyboard, and assistive-technology parity.
- Editable wide clips expose 20 CSS pixel pointer resize regions before selection; unselected handles use `tabIndex=-1`.
- Clips narrower than 48 pixels retain the selected compact resize dock.
- Resize feedback stays inside the active clip and never enters the ruler lane.
- The fixed timeline header exposes `5 tracks` or `Tracks 3–5 of 5` from canonical vertical geometry.
- The 30-minute headed fixture enforces a 50 ms move and resize budget and a 400-element ruler/grid ceiling.
- Prefix every shell command with `rtk`.
- Do not modify or stage unrelated Settings or native-runtime work.

---

### Task 1: Readable Clip Layers And Direct Resize Acquisition

**Files:**
- Modify: `src/components/workspace/timeline-item.tsx`
- Modify: `src/components/workspace/timeline-item.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx`
- Modify: `src/components/workspace/timeline-editor.test.tsx`

**Interfaces:**
- Consumes: existing `TimelineItemShellProps`, `TimelineResizeHandle`, `resizeControlModeForWidth`, item selection, and track lock state.
- Produces: `resizeTabbable?: boolean` and `richMedia?: boolean` `TimelineItemShell` props; wide unlocked clips mount pointer resize handles before selection while compact clips retain selected docks.

- [ ] **Step 1: Write failing shell tests for rich-media text protection and compact resize feedback**

Add focused expectations to `timeline-item.test.tsx`:

```tsx
it("protects rich media text without covering the full clip", () => {
  render(
    <TimelineItemShell
      item={item}
      pixelWidth={240}
      timelineStartSeconds={5}
      selected={false}
      focused={false}
      dragging={false}
      enabled
      richMedia
      visuals={<span data-testid="rich-visual">Filmstrip</span>}
      status={<span>Generated</span>}
    />,
  );

  const content = screen.getByTestId("timeline-item-content");
  expect(content).toHaveAttribute("data-rich-media", "true");
  expect(content).toHaveClass("bg-slate-950/75", "max-w-[calc(100%-2rem)]");
});

it("keeps resize feedback inside the clip with compact visible copy", () => {
  render(
    <TimelineItemShell
      item={item}
      pixelWidth={240}
      timelineStartSeconds={5}
      selected
      focused={false}
      dragging={false}
      enabled
      interactionEdge="right"
      interactionPlacement={{
        itemId: item.id,
        trackId: "track-video",
        startSeconds: 5,
        durationSeconds: 3,
      }}
    />,
  );

  const bubble = screen.getByTestId("timeline-resize-bubble");
  expect(bubble).toHaveAttribute("data-placement", "inside");
  expect(bubble).toHaveClass("top-1", "max-w-[calc(100%-0.5rem)]");
  expect(bubble).toHaveTextContent("Right 00:00:08.000 · 3.000s");
  expect(bubble).toHaveAccessibleName(
    "Right edge 00:00:08.000, duration 00:00:03.000",
  );
});
```

Update the existing 16-pixel/above-placement assertions to the new 20-pixel and always-inside contract.

- [ ] **Step 2: Run the shell tests and verify RED**

Run:

```bash
rtk pnpm vitest run src/components/workspace/timeline-item.test.tsx --reporter=verbose
```

Expected: failures mention missing `data-rich-media`, missing scrim classes, `w-4` instead of `w-5`, old resize copy, or `above` placement.

- [ ] **Step 3: Implement the shell presentation contract**

In `timeline-item.tsx`:

```tsx
interface TimelineItemShellProps {
  // existing props
  richMedia?: boolean;
  resizeTabbable?: boolean;
}
```

Pass `resizeTabbable` into `TimelineResizeHandle`, set `tabIndex={resizeTabbable ? 0 : -1}`, use `w-5` for edge layout, and make the visible 3-pixel rule respond to selected state, clip hover/focus-within, and handle focus.

Add `group/timeline-clip` to the shell. Give the content stack `data-testid="timeline-item-content"`, `data-rich-media`, horizontal resize-safe inset, and this conditional localized scrim:

```tsx
className={`relative z-10 flex h-full min-w-0 flex-col justify-center overflow-hidden pl-1 pr-1 ${
  showRich && richMedia
    ? "m-1 h-[calc(100%-0.5rem)] max-w-[calc(100%-2rem)] self-start rounded-sm bg-slate-950/75 px-1.5 shadow-sm backdrop-blur-[1px]"
    : ""
}`}
```

Render resize feedback with `data-placement="inside"`, `top-1`, a clip-bounded maximum width, and visible copy produced by:

```tsx
`${interactionEdge === "left" ? "Left" : "Right"} ${formatInteractionTimecode(edgeSeconds)} · ${durationSeconds.toFixed(3)}s`
```

Keep the explicit existing `aria-label` edge/duration wording.

- [ ] **Step 4: Verify the shell tests GREEN**

Run the Step 2 command. Expected: all `timeline-item.test.tsx` tests pass.

- [ ] **Step 5: Write failing editor tests for preselection resize and fade copy**

Add tests to `timeline-editor.test.tsx` that prove:

```tsx
it("exposes pointer resize edges before selection without adding tab stops", () => {
  const onSelectItem = vi.fn();
  render(
    <TimelineEditor
      timeline={sampleTimeline}
      selectedItemId={null}
      onSelectItem={onSelectItem}
      onTimelinePatch={vi.fn()}
    />,
  );
  for (const edge of ["left", "right"] as const) {
    expect(screen.getByRole("button", { name: `Resize Opening clip ${edge} edge` }))
      .toHaveAttribute("tabindex", "-1");
  }
  fireEvent.pointerDown(
    screen.getByRole("button", { name: "Resize Opening clip right edge" }),
  );
  expect(onSelectItem).toHaveBeenCalledWith("item-1");
});

it("makes selected resize edges keyboard reachable", () => {
  render(<TimelineEditor timeline={sampleTimeline} selectedItemId="item-1" onTimelinePatch={vi.fn()} />);
  for (const edge of ["left", "right"] as const) {
    expect(screen.getByRole("button", { name: `Resize Opening clip ${edge} edge` }))
      .toHaveAttribute("tabindex", "0");
  }
});

it("keeps fade metadata accessible without persistent waveform copy", () => {
  render(<TimelineEditor timeline={audioFadeTimeline} />);
  const clip = screen.getByRole("button", { name: "Music bed" });
  expect(within(clip).getByLabelText("Fade out for Music bed, 1.50 seconds"))
    .not.toHaveTextContent("fade out 1.50s");
  expect(clip.getAttribute("title")).toContain("fade out 1.50s");
});
```

Use the existing inline audio fade fixture rather than introducing a new test helper.

- [ ] **Step 6: Run the focused editor tests and verify RED**

Run:

```bash
rtk pnpm vitest run src/components/workspace/timeline-editor.test.tsx -t "exposes pointer resize edges before selection|makes selected resize edges keyboard reachable|keeps fade metadata accessible"
```

Expected: unselected handles are absent, selected handles have no explicit `tabindex`, or fade ramp still exposes visible text.

- [ ] **Step 7: Implement editor resize eligibility and audio cleanup**

In `TimelineEditor`, derive:

```tsx
const resizeControlMode = activeResizeMode ?? resizeControlModeForWidth(previewPixelWidth);
const showResizeHandles = !trackLocked && (
  selected || resizeControlMode === "edge-handles"
);
const resizeTabbable = selected;
```

Pass `richMedia={item.kind === "video_clip" || item.kind === "audio_clip"}` and `resizeTabbable={resizeTabbable}` to `TimelineItemShell`. Keep `compactResizeTrackIds` selected-only. Always pass `interactionBubblePlacement="inside"`.

Retain `AudioFadeOut` as a visual ramp with its accessible name and style. Do not render `metadata.reason` as visible status text for audio clips; the full reason remains in the clip title. Reduce the waveform layer behind content without hiding peaks or masks.

At the start of `onResizePointerDown`, call `selectTimelineItem(item.id, false)` before pointer capture so direct edge resizing owns selection and inspector context immediately.

- [ ] **Step 8: Verify editor and shell GREEN**

Run:

```bash
rtk pnpm vitest run src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-editor.test.tsx --reporter=dot
```

Expected: both test files pass.

- [ ] **Step 9: Commit Task 1**

```bash
rtk git add src/components/workspace/timeline-item.tsx src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
rtk git commit -m "feat(timeline): polish clip interactions"
```

### Task 2: Visible Track Orientation

**Files:**
- Modify: `src/lib/timeline-viewport.ts`
- Modify: `src/lib/timeline-viewport.test.ts`
- Modify: `src/components/workspace/timeline-editor.tsx`
- Modify: `src/components/workspace/timeline-editor.test.tsx`

**Interfaces:**
- Consumes: `buildTimelineTrackGeometry().entries`, viewport `scrollTop`, and viewport `clientHeight`.
- Produces: `visibleTimelineTrackRange(entries, scrollTop, viewportHeight)` returning `{ first: number; last: number; total: number; label: string }`.

- [ ] **Step 1: Write failing pure range tests**

Add to `timeline-viewport.test.ts`:

```ts
const tracks = [
  { top: 0, bottom: 64 },
  { top: 64, bottom: 112 },
  { top: 112, bottom: 160 },
  { top: 160, bottom: 208 },
  { top: 208, bottom: 272 },
];

expect(visibleTimelineTrackRange(tracks, 0, 272).label).toBe("5 tracks");
expect(visibleTimelineTrackRange(tracks, 112, 160).label).toBe("Tracks 3–5 of 5");
expect(visibleTimelineTrackRange(tracks, 64, 96).label).toBe("Tracks 2–4 of 5");
expect(visibleTimelineTrackRange([], 0, 100).label).toBe("No tracks");
```

- [ ] **Step 2: Run the viewport tests and verify RED**

Run:

```bash
rtk pnpm vitest run src/lib/timeline-viewport.test.ts --reporter=verbose
```

Expected: import/export failure for `visibleTimelineTrackRange`.

- [ ] **Step 3: Implement the pure range helper**

Add to `timeline-viewport.ts`:

```ts
export function visibleTimelineTrackRange(
  entries: readonly { top: number; bottom: number }[],
  scrollTop: number,
  viewportHeight: number,
) {
  const total = entries.length;
  if (total === 0) return { first: 0, last: 0, total, label: "No tracks" };
  const visibleTop = Math.max(0, scrollTop);
  const visibleBottom = visibleTop + Math.max(0, viewportHeight);
  const firstIndex = Math.max(0, entries.findIndex(({ bottom }) => bottom > visibleTop));
  const lastIndex = entries.reduce(
    (current, { top }, index) => top < visibleBottom ? index : current,
    firstIndex,
  );
  const first = firstIndex + 1;
  const last = lastIndex + 1;
  return {
    first,
    last,
    total,
    label: first === 1 && last === total ? `${total} tracks` : `Tracks ${first}–${last} of ${total}`,
  };
}
```

- [ ] **Step 4: Verify the viewport tests GREEN**

Run the Step 2 command. Expected: all viewport tests pass.

- [ ] **Step 5: Write a failing editor orientation test**

In `timeline-editor.test.tsx`, render `sampleTimeline`, set deterministic `clientHeight`, `scrollHeight`, and `scrollTop` values on `timeline-track-viewport`, fire `scroll`, and assert:

```tsx
expect(screen.getByLabelText("Timeline visible track range"))
  .toHaveTextContent("Tracks 3–5 of 5");
expect(screen.getByTestId("timeline-track-viewport"))
  .toHaveClass("[scrollbar-gutter:stable]");
```

- [ ] **Step 6: Run the orientation test and verify RED**

```bash
rtk pnpm vitest run src/components/workspace/timeline-editor.test.tsx -t "reports the visible track range while scrolling"
```

Expected: missing `Timeline visible track range` element or stable-gutter class.

- [ ] **Step 7: Wire vertical range state into the fixed header**

Import `visibleTimelineTrackRange`, add state for `{ scrollTop, viewportHeight }`, update it from the track viewport `onScroll`, and initialize/refresh height through the existing viewport measurement effect. Derive the range from `trackGeometry.entries`.

Replace the fixed header's single range span with a compact two-part group that keeps project time and adds:

```tsx
<span
  aria-label="Timeline visible track range"
  className="ml-auto whitespace-nowrap text-[9px] text-muted-foreground"
>
  {visibleTrackRange.label}
</span>
```

Add `[scrollbar-gutter:stable]` to `timeline-track-viewport`.

- [ ] **Step 8: Verify Task 2 GREEN and commit**

```bash
rtk pnpm vitest run src/lib/timeline-viewport.test.ts src/components/workspace/timeline-editor.test.tsx --reporter=dot
rtk git add src/lib/timeline-viewport.ts src/lib/timeline-viewport.test.ts src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
rtk git commit -m "feat(timeline): show visible track range"
```

### Task 3: Headed Commit, Undo, And Performance Contracts

**Files:**
- Modify: `src/lib/modern-editor-visual-qa-fixtures.ts`
- Modify: `src/lib/modern-editor-visual-qa-fixtures.test.ts`
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `src/browser-visual-qa-script.test.ts`
- Modify: `src/browser-visual-qa-palmier-scenarios.test.ts`

**Interfaces:**
- Consumes: existing `timeline-resize-gap` project state, timeline item accessible names, `Undo timeline edit`, grid/item test IDs, and scenario runner.
- Produces: `modern-editor-timeline-commit-move` and `modern-editor-timeline-commit-resize` scenarios plus measured long-project move/resize durations.

- [ ] **Step 1: Write failing fixture and source-policy tests**

Add expectations that both new scenario IDs resolve to `projectState: "timeline-resize-gap"`, are listed in `modernEditorVisualQaScenarios`, release the pointer, verify canonical geometry, invoke `Undo timeline edit`, and verify restoration.

Add source assertions that the long-project case contains `timelineLongMoveElapsedMs`, `timelineLongResizeElapsedMs`, `50`, the grid ceiling `400`, and an error including measured values.

- [ ] **Step 2: Run source-policy tests and verify RED**

```bash
rtk pnpm vitest run src/lib/modern-editor-visual-qa-fixtures.test.ts src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts --reporter=verbose
```

Expected: missing scenario IDs and missing performance-contract source markers.

- [ ] **Step 3: Add the two fixture routes and scenario definitions**

Extend `ModernEditorVisualQaScenarioId` and the fixture map:

```ts
"modern-editor-timeline-commit-move": {
  scenarioId: "modern-editor-timeline-commit-move",
  projectState: "timeline-resize-gap",
},
"modern-editor-timeline-commit-resize": {
  scenarioId: "modern-editor-timeline-commit-resize",
  projectState: "timeline-resize-gap",
},
```

Add corresponding 1440×900 modern-editor scenarios.

- [ ] **Step 4: Implement release and Undo browser flows**

For commit-move, record the opening clip shell box, drag by a valid delta within the gap, release, wait until canonical `left` changes by at least 8 pixels, click `Undo timeline edit`, and wait until `left` returns within 1 pixel.

For commit-resize, record shell width and right handle `aria-valuenow`, drag the handle right, release, wait for width/value growth, click Undo, and wait for both values to return within tolerance.

Do not add these states to `scenarioHoldsPointer` or cleanup cancellation.

- [ ] **Step 5: Add deterministic long-project measurements**

Around the existing two-animation-frame move and resize waits, measure with `performance.now()`. Store `timelineLongMoveElapsedMs` and `timelineLongResizeElapsedMs`; count `[data-testid="timeline-grid-element"]` and mounted shells `[data-selected]`. Throw with all measurements if either duration exceeds 50 ms, grid elements exceed 400, or live geometry is absent. Preserve the trace-marker assertions.

- [ ] **Step 6: Verify Task 3 source tests GREEN and commit**

```bash
rtk pnpm vitest run src/lib/modern-editor-visual-qa-fixtures.test.ts src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts --reporter=dot
rtk git add src/lib/modern-editor-visual-qa-fixtures.ts src/lib/modern-editor-visual-qa-fixtures.test.ts scripts/browser-visual-qa.mjs src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts
rtk git commit -m "test(timeline): cover committed browser edits"
```

### Task 4: Full Verification And Visual Acceptance

**Files:**
- Verify only; update source only if a failing contract exposes a defect.

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: fresh test, lint, build, Rust, headed-browser, screenshot, and diff evidence.

- [ ] **Step 1: Run focused and complete TypeScript verification**

```bash
rtk pnpm vitest run src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-editor.test.tsx src/lib/timeline-viewport.test.ts src/lib/timeline-interaction-frame.test.ts src/lib/modern-editor-visual-qa-fixtures.test.ts src/browser-visual-qa-script.test.ts src/browser-visual-qa-palmier-scenarios.test.ts --reporter=dot
rtk pnpm lint
rtk pnpm build
```

Expected: all commands exit `0` with no failures.

- [ ] **Step 2: Run cross-layer Rust timeline verification**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml project_action -- --test-threads=1
```

Expected: all filtered Rust project-action tests pass.

- [ ] **Step 3: Run headed timeline scenarios**

Start the worktree Vite server, then run:

```bash
rtk node scripts/browser-visual-qa.mjs --only modern-editor-timeline-commit-move --out output/playwright/timeline-polish/commit-move
rtk node scripts/browser-visual-qa.mjs --only modern-editor-timeline-commit-resize --out output/playwright/timeline-polish/commit-resize
rtk node scripts/browser-visual-qa.mjs --only modern-editor-timeline-collision --out output/playwright/timeline-polish/collision
rtk node scripts/browser-visual-qa.mjs --only modern-editor-timeline-long-project --out output/playwright/timeline-polish/long-project
rtk node scripts/browser-visual-qa.mjs --only modern-editor-timeline-1280x720 --out output/playwright/timeline-polish/1280
rtk node scripts/browser-visual-qa.mjs --only modern-editor-timeline-1024x720 --out output/playwright/timeline-polish/1024
```

Expected: every command exits `0` and writes one screenshot.

- [ ] **Step 4: Inspect every accepted screenshot**

Confirm:

- filmstrip/waveform texture remains visible but title/timing/status is readable;
- fade copy no longer sits over the waveform;
- hover/selected resize rules are precise and not visually heavy;
- resize feedback is contained by the active clip;
- target-local collision feedback remains readable;
- `Tracks … of 5` is visible after responsive scenarios scroll to the audio row;
- no toolbar, ruler, track, overview, or inspector text overlaps at 1440×900, 1280×720, or 1024×720.

- [ ] **Step 5: Review scope and diff integrity**

```bash
rtk git diff --check main...HEAD
rtk git status --short --branch
rtk git diff --stat main...HEAD
rtk git diff --name-only main...HEAD
```

Expected: only the spec, plan, timeline source/tests, visual fixture, and browser QA files are changed; no unrelated Settings/native-runtime files appear.
