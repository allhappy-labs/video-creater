# Timeline and Preview Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a coherent timeline-preview editing surface with explicit canvas modes, deterministic live motion, accessible navigation, and a stable final frame.

**Architecture:** Keep `editor-workspace.tsx` as the owner of selection, playhead, and playback state. Normalize preview evaluation time in `timeline-preview.ts`, pass deterministic relative time through the compositor into motion templates, and render canvas controls from one explicit tool state. Improve transport and timeline navigation without changing canonical timeline data or panel sizing.

**Tech Stack:** React 19, TypeScript 5.7, Tailwind CSS, Vitest, Testing Library, Lucide React

## Global Constraints

- Preserve the existing preview and timeline panel sizes.
- Keep canonical timeline item intervals half-open; normalize only the preview evaluation time at exact project end.
- Keep all project mutations on the existing project-action and undo path.
- Canvas modes are mutually exclusive: `transform`, `crop`, or `rotate`.
- Pointer hit regions are at least 44×44 px while visible handles may remain compact.
- Arrow keys adjust canvas values by 1%; Shift+Arrow adjusts by 5%; Escape cancels an active pointer gesture.
- Playback and scrub must render media, captions, overlays, and motion templates from one playhead clock.
- A template frame at a given playhead time must be identical after play, scrub, step, or direct seek.
- Full repository test, lint, build, and release suites remain deferred until the user declares the UI batch complete.
- During implementation, run only the exact narrow Vitest file or test name listed in each task plus real-app visual checks.
- Do not commit unless the user separately authorizes commits; commit commands below document intended checkpoints only.

---

### Task 1: Hold the Final Composite at Exact Timeline End

**Files:**
- Modify: `src/lib/timeline-preview.ts:128-315`
- Test: `src/lib/timeline-preview.test.ts`

**Interfaces:**
- Produces: `resolveTimelinePreviewEvaluationSeconds(timeline: Timeline, playheadSeconds: number): number`
- Produces: `TimelinePreviewFrame.playheadSeconds` remains the requested user-facing time.
- Produces: `TimelinePreviewFrame.evaluationSeconds` contains the normalized time used to build the composite.
- Consumes: `Timeline.durationSeconds`

- [ ] **Step 1: Write the failing exact-end test**

Add a test beside the basic timeline-preview frame cases. Extend the existing final still so it ends exactly at the timeline duration:

```ts
it("holds the final composite at the exact timeline duration", () => {
  const endAlignedTimeline: Timeline = {
    ...timeline,
    tracks: timeline.tracks.map((track) => ({
      ...track,
      items: track.items.map((item) =>
        item.id === "image-1" ? { ...item, durationSeconds: 3 } : item,
      ),
    })),
  };
  const frame = buildTimelinePreviewFrame({
    timeline: endAlignedTimeline,
    media,
    playheadSeconds: endAlignedTimeline.durationSeconds,
  });

  expect(frame.playheadSeconds).toBe(endAlignedTimeline.durationSeconds);
  expect(frame.evaluationSeconds).toBeLessThan(endAlignedTimeline.durationSeconds);
  expect(frame.status).toBe("ready");
  expect(frame.layers.map((layer) => layer.itemId)).toContain("image-1");
});
```

- [ ] **Step 2: Run only the failing test**

Run:

```bash
rtk pnpm exec vitest run src/lib/timeline-preview.test.ts -t "holds the final composite at the exact timeline duration"
```

Expected: FAIL because `frame.status` is `empty` at the exclusive item end.

- [ ] **Step 3: Add centralized preview-time normalization**

Add this exported helper near `PREVIEW_OPACITY_SAMPLE_FPS`:

```ts
export function resolveTimelinePreviewEvaluationSeconds(
  timeline: Timeline,
  playheadSeconds: number,
): number {
  const durationSeconds = Math.max(0, timeline.durationSeconds);
  const clampedSeconds = Math.max(0, Math.min(durationSeconds, playheadSeconds));
  if (durationSeconds === 0 || clampedSeconds < durationSeconds) {
    return clampedSeconds;
  }
  return Math.max(0, durationSeconds - 1 / PREVIEW_OPACITY_SAMPLE_FPS);
}
```

At the start of `buildTimelinePreviewFrame`, derive one evaluation value and replace all frame-building reads of `input.playheadSeconds` with it. Preserve the requested time in the returned frame:

```ts
const evaluationSeconds = resolveTimelinePreviewEvaluationSeconds(
  input.timeline,
  input.playheadSeconds,
);
```

Use `evaluationSeconds` for `isActiveAt`, `buildOverlayLayer`, local item time, motion, and source-time calculation. Keep:

```ts
playheadSeconds: input.playheadSeconds,
evaluationSeconds,
```

Add `evaluationSeconds: number` to `TimelinePreviewFrame`.

- [ ] **Step 4: Run the narrow timeline-preview test file**

Run:

```bash
rtk pnpm exec vitest run src/lib/timeline-preview.test.ts
```

Expected: PASS, including the new exact-end case and existing adjacent-clip boundary cases.

- [ ] **Step 5: Verify in the open app**

Jump the preview transport to `00:08.000`. Confirm the final composited frame remains visible and the transport still reports the exact duration. Capture `iteration-01-final-frame.png` under `output/playwright/timeline-preview-iterations/`.

- [ ] **Step 6: Document the intended commit checkpoint**

Do not run without user authorization:

```bash
rtk git add src/lib/timeline-preview.ts src/lib/timeline-preview.test.ts
rtk git commit -m "fix(preview): hold the final timeline frame"
```

---

### Task 2: Drive Motion Templates from Deterministic Timeline Time

**Files:**
- Modify: `src/components/workspace/motion-template-preview.tsx:1-160`
- Modify: `src/components/workspace/timeline-preview-compositor.tsx:443-1040`
- Test: `src/components/workspace/motion-template-preview.test.tsx`
- Test: `src/components/workspace/timeline-preview-compositor.test.tsx`

**Interfaces:**
- Produces: `MotionTemplatePreviewProps`
- Produces: `motionTemplatePhase(timeSeconds: number, periodSeconds: number): number`
- Consumes: `TimelinePreviewOverlayLayer.timelineStartSeconds` and `.timelineEndSeconds`
- Consumes: compositor `frame.evaluationSeconds` and `isPlaying`

- [ ] **Step 1: Write failing phase and rendering tests**

In `motion-template-preview.test.tsx`, import the new helper and render the same Holographic item at two times:

```tsx
it("derives a repeatable phase from timeline time", () => {
  expect(motionTemplatePhase(0.6, 2.4)).toBeCloseTo(0.25);
  expect(motionTemplatePhase(3, 2.4)).toBeCloseTo(0.25);
});

it("renders distinct holographic frames at distinct timeline times", () => {
  const { rerender, getByLabelText } = render(
    <MotionTemplatePreview item={item} timeSeconds={0} durationSeconds={4} isPlaying />,
  );
  const startStyle = getByLabelText(/preview$/).getAttribute("style");

  rerender(
    <MotionTemplatePreview item={item} timeSeconds={0.8} durationSeconds={4} isPlaying />,
  );

  expect(getByLabelText(/preview$/).getAttribute("style")).not.toBe(startStyle);
});
```

In the compositor test, assert that a template beginning at 2 seconds receives `data-template-time="0.815"` when the playhead is 2.815 seconds.

- [ ] **Step 2: Run only the motion-template tests**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/motion-template-preview.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx -t "timeline time|holographic|template time"
```

Expected: FAIL because the component currently accepts only `item` and renders a static Holographic variant.

- [ ] **Step 3: Define deterministic template-time props and phase**

Add:

```ts
export interface MotionTemplatePreviewProps {
  item: TimelineItem | null;
  timeSeconds: number;
  durationSeconds: number;
  isPlaying: boolean;
}

export function motionTemplatePhase(timeSeconds: number, periodSeconds: number): number {
  if (!Number.isFinite(timeSeconds) || !Number.isFinite(periodSeconds) || periodSeconds <= 0) {
    return 0;
  }
  return ((Math.max(0, timeSeconds) % periodSeconds) + periodSeconds) % periodSeconds / periodSeconds;
}
```

Change the component signature:

```ts
export function MotionTemplatePreview({
  item,
  timeSeconds,
  durationSeconds,
  isPlaying,
}: MotionTemplatePreviewProps) {
```

Derive a stable phase and expose it for verification:

```ts
const phase = motionTemplatePhase(timeSeconds, 2.4);
const wave = Math.sin(phase * Math.PI * 2);
const pulse = Math.cos(phase * Math.PI * 2);
```

On the root preview element, add:

```tsx
data-template-time={timeSeconds.toFixed(3)}
data-template-playing={isPlaying ? "true" : "false"}
style={{
  "--template-phase": phase,
  "--template-duration": durationSeconds,
} as CSSProperties}
```

Use `wave` and `pulse` to change the Holographic logo position, scale, glow positions, and gradient angle with inline transforms. Do not introduce free-running `animate-*` classes.

- [ ] **Step 4: Pass relative template time from the compositor**

In `renderOverlayLayers`, derive time from the authoritative playhead and layer bounds:

```tsx
const durationSeconds = Math.max(
  0,
  layer.timelineEndSeconds - layer.timelineStartSeconds,
);
const timeSeconds = Math.max(
  0,
  Math.min(durationSeconds, frame.evaluationSeconds - layer.timelineStartSeconds),
);

return item ? (
  <MotionTemplatePreview
    key={layer.itemId}
    item={item}
    timeSeconds={timeSeconds}
    durationSeconds={durationSeconds}
    isPlaying={isPlaying}
  />
) : null;
```

Update existing direct test renders with `timeSeconds={0}`, the item duration, and `isPlaying={false}`.

- [ ] **Step 5: Run the two narrow test files**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/motion-template-preview.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Verify play, pause, scrub, and reverse seek in the open app**

Insert Holographic Logo Cutout, then record frames at 2.000, 2.400, and 2.815 seconds. Confirm each is visibly distinct, pausing holds its exact frame, and seeking back to 2.400 reproduces the same frame. Capture `iteration-02-holographic-*.png`.

- [ ] **Step 7: Document the intended commit checkpoint**

Do not run without user authorization:

```bash
rtk git add src/components/workspace/motion-template-preview.tsx src/components/workspace/motion-template-preview.test.tsx src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/timeline-preview-compositor.test.tsx
rtk git commit -m "fix(preview): synchronize motion templates with timeline time"
```

---

### Task 3: Make Canvas Manipulation Explicitly Modal

**Files:**
- Modify: `src/components/workspace/viewer-context-toolbar.tsx`
- Modify: `src/components/workspace/editor-workspace.tsx:9018-9090,10699-10734`
- Modify: `src/components/workspace/preview-panel.tsx:35-90,550-660,1060-1115`
- Modify: `src/components/workspace/timeline-preview-compositor.tsx:443-1240`
- Test: `src/components/workspace/viewer-context-toolbar.test.tsx`
- Test: `src/components/workspace/timeline-preview-compositor.test.tsx`

**Interfaces:**
- Produces: `export type TimelinePreviewCanvasTool = "transform" | "crop" | "rotate"`
- Produces: `ViewerContextToolbarProps.activeCanvasTool`
- Produces: `ViewerContextToolbarProps.onCanvasToolChange(tool)`
- Consumes: selected item identity and existing transform/rotation/crop commit callbacks

- [ ] **Step 1: Write failing toolbar and compositor tests**

Add toolbar tests:

```tsx
it("exposes one pressed canvas tool", () => {
  renderToolbar({ activeCanvasTool: "crop" });
  expect(screen.getByRole("button", { name: "Crop" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Transform" })).toHaveAttribute("aria-pressed", "false");
});

it("changes the active canvas tool", async () => {
  const onCanvasToolChange = vi.fn();
  renderToolbar({ onCanvasToolChange });
  await userEvent.click(screen.getByRole("button", { name: "Rotate" }));
  expect(onCanvasToolChange).toHaveBeenCalledWith("rotate");
});
```

Add compositor assertions that Transform renders move plus four resize buttons but no Crop or Rotate buttons; repeat for Crop and Rotate.

- [ ] **Step 2: Run the two exact test files**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/viewer-context-toolbar.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx -t "canvas tool|pressed|Transform|Crop|Rotate"
```

Expected: FAIL because the toolbar does not own pressed tool state and the compositor renders all controls simultaneously.

- [ ] **Step 3: Add tool state at the workspace boundary**

Export from the compositor:

```ts
export type TimelinePreviewCanvasTool = "transform" | "crop" | "rotate";
```

In `EditorWorkspace`, add:

```ts
const [timelineCanvasTool, setTimelineCanvasTool] =
  useState<TimelinePreviewCanvasTool>("transform");

useEffect(() => {
  setTimelineCanvasTool("transform");
}, [selectedTimelineItem?.id]);
```

Pass the state to `PreviewPanel` and `ViewerContextToolbar`. Replace `focusSelectedCanvasControl` with a state update followed by focus on the active tool button:

```ts
function selectTimelineCanvasTool(tool: TimelinePreviewCanvasTool) {
  setTimelineCanvasTool(tool);
  window.requestAnimationFrame(() => {
    document.querySelector<HTMLButtonElement>(`[data-canvas-tool="${tool}"]`)?.focus();
  });
}
```

- [ ] **Step 4: Make toolbar controls mutually exclusive**

Update toolbar props:

```ts
activeCanvasTool?: TimelinePreviewCanvasTool;
onCanvasToolChange?: (tool: TimelinePreviewCanvasTool) => void;
```

For each Transform, Crop, and Rotate button, render `aria-pressed`, `data-canvas-tool`, and an active treatment using existing toolbar tokens. The toolbar’s callbacks call `onCanvasToolChange` instead of finding canvas DOM controls.

- [ ] **Step 5: Render only the selected canvas overlay controls**

Add `canvasTool?: TimelinePreviewCanvasTool` to compositor and preview-panel props. Default to `"transform"` for direct test renders.

Change `CanvasSelectionOverlay` to accept `tool` and conditionally render:

```tsx
{tool === "transform" ? <>{moveButton}{resizeButtons}</> : null}
{tool === "crop" ? <>{cropShade}{cropButtons}</> : null}
{tool === "rotate" ? rotateButton : null}
```

Keep a single selected-layer outline. Increase each control’s outer button to `h-11 w-11`; keep the visible inner marker compact. The Crop mode adds four absolute dimming panels around the retained crop rectangle.

- [ ] **Step 6: Add Escape cancellation**

Store the initial interaction values already present in `canvasInteraction`. Add a viewport key handler:

```ts
function cancelCanvasInteraction(event: KeyboardEvent<HTMLDivElement>) {
  if (event.key !== "Escape" || !canvasInteraction) return;
  event.preventDefault();
  setCanvasInteraction(null);
}
```

Because transient values are not committed until pointer-up, clearing the interaction restores the canonical layer state.

- [ ] **Step 7: Run narrow toolbar and compositor tests**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/viewer-context-toolbar.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Verify all three modes in the open app**

Select the same clip and capture Transform, Crop, and Rotate states. Tab through each state and confirm only its controls receive focus. Verify visible 44 px hit regions by pointer use without changing the preview panel size. Capture `iteration-03-transform.png`, `iteration-03-crop.png`, and `iteration-03-rotate.png`.

- [ ] **Step 9: Document the intended commit checkpoint**

Do not run without user authorization:

```bash
rtk git add src/components/workspace/viewer-context-toolbar.tsx src/components/workspace/viewer-context-toolbar.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/preview-panel.tsx src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/timeline-preview-compositor.test.tsx
rtk git commit -m "feat(preview): add explicit canvas editing modes"
```

---

### Task 4: Coordinate Selection Chrome with Playback and Playhead

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx:9018-9095,10699-10734`
- Modify: `src/components/workspace/preview-panel.tsx:1150-1170`
- Modify: `src/components/workspace/timeline-preview-compositor.tsx:500-1045`
- Test: `src/components/workspace/preview-panel.test.tsx`
- Test: `src/components/workspace/timeline-preview-compositor.test.tsx`

**Interfaces:**
- Produces: `selectedItemActive: boolean` derived from preview frame membership
- Produces: inactive-selection guidance copy
- Consumes: `selectedItem`, `playheadSeconds`, `timelinePlaying`

- [ ] **Step 1: Write failing playback and inactive-selection tests**

Add cases asserting:

```tsx
expect(screen.queryByLabelText(/Canvas transform controls/)).not.toBeInTheDocument();
expect(screen.getByText(/Move the playhead into the clip to edit it on canvas/)).toBeVisible();
```

for a selected clip outside the playhead. Add a playing case that asserts both context toolbar slot and canvas controls are absent while the compositor still renders the selected clip.

- [ ] **Step 2: Run the exact failing tests**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx -t "inactive selection|hides edit chrome while playing"
```

Expected: FAIL because toolbar visibility follows selection alone and selection chrome is not explicitly gated by playback.

- [ ] **Step 3: Gate compositor controls and render guidance**

Derive:

```ts
const selectedTimelineItem = selectedItemId ? findTimelineItem(timeline, selectedItemId) : null;
const selectedLayer = interactiveMediaLayers.find(
  (layer) => layer.itemId === selectedItemId,
) ?? null;
const selectedItemActive = selectedTimelineItem !== null && (
  frame.layers.some((layer) => layer.itemId === selectedTimelineItem.id) ||
  frame.overlayLayers.some((layer) => layer.itemId === selectedTimelineItem.id)
);
```

Render controls only when `selectedItemActive && !isPlaying`. When a selection exists but is inactive, render:

```tsx
<div role="status" className="pointer-events-none absolute bottom-5 left-1/2 ...">
  <strong>{selectedTimelineItem.label} is selected at {formatRange}.</strong>
  <span>Move the playhead into the clip to edit it on canvas.</span>
</div>
```

Use the existing time-formatting utility rather than adding another timecode formatter.

- [ ] **Step 4: Gate the context toolbar at the preview-panel boundary**

Pass `selectedItemActive` through `onCanvasStateChange` or a dedicated compositor callback. Render the `contextToolbar` slot only when timeline playback is paused and the selected item is active:

```tsx
{activeViewerMode === "timeline" &&
 contextToolbar &&
 !previewIsPlaying &&
 timelinePreviewCanvasState.selectedItemActive &&
 !canonicalFrameFailed &&
 resolvedContextToolbarPlacement ? toolbar : null}
```

Extend `TimelinePreviewCanvasState` with `selectedItemActive` and include it in the layout-effect payload.

- [ ] **Step 5: Run the two narrow test files**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Verify four state transitions in the open app**

Capture: selected+paused, selected+playing, selected+inactive, and cleared selection. Confirm no toolbar or handles remain stale after undo or selection clear. Capture `iteration-04-selection-*.png`.

- [ ] **Step 7: Document the intended commit checkpoint**

Do not run without user authorization:

```bash
rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-preview-compositor.tsx src/components/workspace/timeline-preview-compositor.test.tsx
rtk git commit -m "fix(preview): synchronize edit chrome with playback"
```

---

### Task 5: Make Preview Transport Targets Accessible Without Resizing the Panel

**Files:**
- Modify: `src/components/workspace/preview-panel.tsx:280-390`
- Test: `src/components/workspace/preview-panel.test.tsx`

**Interfaces:**
- Produces: 44×44 transport targets and scrubber hit region
- Consumes: existing transport callbacks and `isEditableKeyboardTarget`

- [ ] **Step 1: Write failing transport accessibility tests**

Assert each transport button uses the shared 44 px class and the scrubber has a 44 px wrapper:

```tsx
expect(screen.getByRole("button", { name: "Preview transport play" })).toHaveClass("h-11", "w-11");
expect(screen.getByTestId("preview-scrubber-hit-region")).toHaveClass("h-11");
expect(screen.getByText(/00:00:02.815/)).not.toHaveClass("truncate");
```

- [ ] **Step 2: Run only transport-related tests**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/preview-panel.test.tsx -t "transport|scrubber|timecode"
```

Expected: FAIL on target classes and timecode truncation.

- [ ] **Step 3: Enlarge hit regions while preserving visual density**

Change the shared transport class to include:

```ts
"flex h-11 w-11 items-center justify-center rounded-md ..."
```

Wrap the visually thin range input:

```tsx
<div
  data-testid="preview-scrubber-hit-region"
  className="group relative -mb-5 flex h-11 items-start pt-5"
>
  <input className="block h-1 w-full ..." />
</div>
```

Increase the transport row height only enough to contain the targets; do not change the preview viewport or panel split. Reserve a nonshrinking timecode width:

```tsx
<div className="min-w-[13.5rem] whitespace-nowrap font-mono text-[11px] text-foreground">
```

At constrained widths, allow Fit metadata to collapse before temporal controls.

- [ ] **Step 4: Run the narrow preview-panel test file**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/preview-panel.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Verify pointer and keyboard transport in the open app**

At wide and constrained widths, verify all five controls, the full timecode, focus indicators, Space, step keys, and the larger scrubber target. Capture `iteration-05-transport-wide.png` and `iteration-05-transport-constrained.png`.

- [ ] **Step 6: Document the intended commit checkpoint**

Do not run without user authorization:

```bash
rtk git add src/components/workspace/preview-panel.tsx src/components/workspace/preview-panel.test.tsx
rtk git commit -m "fix(preview): improve transport accessibility"
```

---

### Task 6: Add a Persistent Playhead Lane, Edge-safe Time Bubble, and Gentle Follow

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx:10806-10845`
- Modify: `src/components/workspace/timeline-editor.tsx:1840-1970,3820-3970,4860-4980`
- Test: `src/components/workspace/timeline-editor.test.tsx`

**Interfaces:**
- Produces: `clampTimelinePlayheadBubbleOffset(playheadX: number, viewportWidth: number, bubbleWidth: number): number`
- Produces: `shouldFollowTimelinePlayhead(playheadX: number, scrollLeft: number, viewportWidth: number): boolean`
- Produces: `TimelineEditorProps.isPlaying?: boolean`
- Consumes: existing `canvasScrollRef`, `clampedPlayheadSeconds`, and `pixelsPerSecond`

- [ ] **Step 1: Write failing geometry tests**

Export two pure helpers and test edge behavior:

```ts
it("keeps the playhead bubble inside the viewport", () => {
  expect(clampTimelinePlayheadBubbleOffset(4, 800, 88)).toBe(40);
  expect(clampTimelinePlayheadBubbleOffset(796, 800, 88)).toBe(-40);
  expect(clampTimelinePlayheadBubbleOffset(400, 800, 88)).toBe(0);
});

it("follows only outside the central safe region", () => {
  expect(shouldFollowTimelinePlayhead(100, 0, 800)).toBe(true);
  expect(shouldFollowTimelinePlayhead(400, 0, 800)).toBe(false);
  expect(shouldFollowTimelinePlayhead(760, 0, 800)).toBe(true);
});
```

- [ ] **Step 2: Run only the geometry tests**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/timeline-editor.test.tsx -t "playhead bubble|central safe region"
```

Expected: FAIL because the helpers and bubble do not exist.

- [ ] **Step 3: Add pure playhead geometry helpers**

Implement:

```ts
export function clampTimelinePlayheadBubbleOffset(
  playheadX: number,
  viewportWidth: number,
  bubbleWidth: number,
): number {
  const half = bubbleWidth / 2;
  if (playheadX < half) return half - playheadX;
  if (playheadX > viewportWidth - half) return viewportWidth - half - playheadX;
  return 0;
}

export function shouldFollowTimelinePlayhead(
  playheadX: number,
  scrollLeft: number,
  viewportWidth: number,
): boolean {
  const relativeX = playheadX - scrollLeft;
  return relativeX < viewportWidth * 0.2 || relativeX > viewportWidth * 0.8;
}
```

- [ ] **Step 4: Reserve the ruler interaction lane and render the time bubble**

Keep the timeline canvas playhead line, but move its head and label into a sticky overlay lane above clip rows. The overlay uses the same horizontal scroll position as the canvas and includes:

```tsx
<div className="relative h-11" aria-label="Timeline ruler">
  <div
    className="pointer-events-none absolute top-0 z-30"
    style={{ left: playheadCanvasX - scrollLeft }}
  >
    <span
      className="absolute top-0 -translate-x-1/2 whitespace-nowrap rounded bg-red-500 px-1.5 py-1 font-mono text-[10px] text-white"
      style={{ transform: `translateX(calc(-50% + ${bubbleOffset}px))` }}
    >
      {formatTimecode(clampedPlayheadSeconds)}
    </span>
  </div>
</div>
```

The entire 44 px ruler lane remains seekable and exposes the current playhead time in its accessible name.

- [ ] **Step 5: Add safe-region follow with manual-scroll suspension**

Track manual scrolling:

```ts
const manualTimelineScrollUntilRef = useRef(0);
```

On user-originated scroll or wheel, set `Date.now() + 1200`. While playback is active and suspension has expired, scroll only if `shouldFollowTimelinePlayhead` returns true. Use `scrollTo({ left, behavior: "auto" })`; do not animate every frame.

Add `isPlaying = false` to `TimelineEditor`, add `isPlaying?: boolean` to `TimelineEditorProps`, and pass `isPlaying={timelinePlaying}` from `EditorWorkspace`. Run follow logic only while `isPlaying` is true.

- [ ] **Step 6: Run the narrow timeline-editor test file**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/timeline-editor.test.tsx
```

Expected: PASS.

- [ ] **Step 7: Verify edge, zoom, scroll, and playback states in the open app**

At start, middle, and exact end, confirm the head and full bubble stay visible. Zoom in, scroll away, play, manually scroll, and confirm follow waits before resuming. Capture `iteration-06-playhead-start.png`, `iteration-06-playhead-end.png`, and `iteration-06-playhead-follow.png`.

- [ ] **Step 8: Document the intended commit checkpoint**

Do not run without user authorization:

```bash
rtk git add src/components/workspace/editor-workspace.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
rtk git commit -m "feat(timeline): add accessible playhead navigation"
```

---

## Final Batch Verification — Run Only When the User Declares the UI Batch Complete

- [ ] Run focused changed-area tests together:

```bash
rtk pnpm exec vitest run src/lib/timeline-preview.test.ts src/components/workspace/motion-template-preview.test.tsx src/components/workspace/timeline-preview-compositor.test.tsx src/components/workspace/viewer-context-toolbar.test.tsx src/components/workspace/preview-panel.test.tsx src/components/workspace/timeline-editor.test.tsx
```

- [ ] Run the full repository test, lint, build, and visual release checks only after explicit user confirmation that iteration is finished:

```bash
rtk pnpm test
rtk pnpm lint
rtk pnpm build
rtk pnpm visual:qa:browser-release
```

- [ ] Compare the final real-app screenshots against the accepted HTML interaction contracts and the original audit screenshots in one side-by-side review.

- [ ] Fix failures, repeat the affected exact command, then repeat the final batch commands until clean.

- [ ] If the user authorizes a final commit, stage only the scoped implementation, tests, design spec, and implementation plan, then use a Conventional Commit message.
