# Timeline UX Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the approved Option 2 timeline as a precise, responsive editor surface with authoritative selection, collision-safe edits, viewport-bounded rendering, readable tracks, and an overview navigator.

**Architecture:** Keep `TimelineEditor` as the workspace-facing coordinator, but move viewport math, edit evaluation, animation-frame scheduling, ruler rendering, item presentation, and overview navigation behind focused interfaces. Preview and commit share one pure edit evaluation, while Rust rejects any timeline action that creates or increases a same-track overlap. Rendering is bounded to the visible time window plus overscan, so project duration no longer determines ruler, grid, clip, or filmstrip work.

**Tech Stack:** React 19, TypeScript 5.7, Tailwind CSS, lucide-react, Vitest, Testing Library, Rust, Tauri 2, Playwright-based browser visual QA.

## Global Constraints

- Preserve the existing `TimelineEditor` public workspace API except for the explicitly added `interactionError` prop.
- Items on one track may not gain overlap until an explicit transition model exists; unchanged legacy overlap remains loadable.
- Toolbar controls never shrink below 28×28 CSS pixels; selection and zoom controls are at least 32×32 CSS pixels.
- Track identity width is `clamp(152px, 15vw, 220px)`.
- Track rows have a 44-pixel minimum; video/audio default to 64 pixels and caption/overlay default to 48 pixels.
- Resize edges expose at least a 16-pixel transparent pointer target.
- Major ruler labels remain at least 72 pixels apart.
- Clip disclosure thresholds are `<48`, `48-119`, `120-219`, and `>=220` pixels.
- Ruler and grid rendering is proportional to viewport width; the 30-minute fixture has at most 400 combined ruler/grid tick elements at 1440×900.
- Pointer previews update no more than once per animation frame and must not produce a task longer than 50 ms in the required local trace.
- Invalid edits show target-local plain text and a deduplicated polite live announcement; color alone is insufficient.
- Use existing shadcn-style controls and `lucide-react`; every icon-only button has an exact `aria-label` and `title`.
- Respect reduced motion and preserve undo, keyboard editing, source timing, ripple trim, track enable/lock, and horizontal synchronization behavior.
- Prefix every shell command with `rtk`; use Conventional Commit messages.

## File Structure

- Create `src/lib/timeline-viewport.ts` and `src/lib/timeline-viewport.test.ts` for visible-window, adaptive-tick, and item-intersection math.
- Create `src/lib/timeline-edit-evaluator.ts` and `src/lib/timeline-edit-evaluator.test.ts` for move/resize compatibility, snapping, collision, and user-facing reasons.
- Create `src/lib/timeline-interaction-frame.ts` and `src/lib/timeline-interaction-frame.test.ts` for requestAnimationFrame coalescing and synchronous flush/cancel.
- Create `src/components/workspace/timeline-ruler.tsx` and `src/components/workspace/timeline-ruler.test.tsx` for bounded ticks and collision-aware playhead/hover labels.
- Create `src/components/workspace/timeline-item.tsx` and `src/components/workspace/timeline-item.test.tsx` for adaptive clip disclosure and distinct selected/focused/dragged states.
- Create `src/components/workspace/timeline-overview.tsx` and `src/components/workspace/timeline-overview.test.tsx` for the canvas overview and viewport-frame controls.
- Modify `src/components/workspace/timeline-editor.tsx` and its test to coordinate the extracted modules and render the approved layout.
- Modify `src/components/workspace/editor-workspace.tsx` and its test to route canonical errors into the timeline.
- Modify `src-tauri/src/project/action.rs` to enforce no-increased-overlap atomically.
- Modify `src/lib/modern-editor-visual-qa-fixtures.ts`, `scripts/browser-visual-qa.mjs`, `src/browser-visual-qa-palmier-scenarios.test.ts`, and `src/browser-visual-qa-script.test.ts` for selected, collision, responsive, and long-project evidence.

---

### Task 1: Add viewport-bounded timeline math

**Files:**
- Create: `src/lib/timeline-viewport.ts`
- Create: `src/lib/timeline-viewport.test.ts`

**Interfaces:**
- Produces: `timelineViewportWindow(input): TimelineViewportWindow`, `adaptiveTimelineTicks(input): TimelineTickSeries`, and `timelineItemIntersectsWindow(item, window, persistentItemIds): boolean`.
- Consumed by: Tasks 5, 7, and 9.

- [ ] **Step 1: Write failing visible-window and tick-density tests**

  ```ts
  import { describe, expect, it } from "vitest";
  import {
    adaptiveTimelineTicks,
    timelineItemIntersectsWindow,
    timelineViewportWindow,
  } from "./timeline-viewport";

  describe("timeline viewport", () => {
    it("bounds a 30-minute project to the viewport plus one viewport of overscan", () => {
      expect(timelineViewportWindow({
        durationSeconds: 1_800,
        pixelsPerSecond: 80,
        scrollLeft: 8_000,
        viewportWidth: 1_200,
        overscanViewports: 1,
      })).toEqual({
        visibleStartSeconds: 100,
        visibleEndSeconds: 115,
        renderStartSeconds: 85,
        renderEndSeconds: 130,
      });
    });

    it("keeps combined visible ruler and grid ticks below 400", () => {
      const window = timelineViewportWindow({
        durationSeconds: 1_800,
        pixelsPerSecond: 80,
        scrollLeft: 0,
        viewportWidth: 1_200,
        overscanViewports: 1,
      });
      const ticks = adaptiveTimelineTicks({ window, pixelsPerSecond: 80, snapSeconds: 0.25 });
      expect(ticks.majorSeconds.length + ticks.minorSeconds.length).toBeLessThanOrEqual(200);
    });

    it("keeps selected items mounted outside the overscan window", () => {
      const item = { id: "selected", startSeconds: 500, durationSeconds: 4 };
      expect(timelineItemIntersectsWindow(item, { renderStartSeconds: 0, renderEndSeconds: 30 }, new Set(["selected"]))).toBe(true);
    });
  });
  ```

- [ ] **Step 2: Run the new test and verify the red state**

  Run: `rtk pnpm test -- src/lib/timeline-viewport.test.ts`

  Expected: FAIL because `timeline-viewport.ts` does not exist.

- [ ] **Step 3: Implement bounded windows and adaptive ticks**

  Add exported interfaces and pure functions. Use `[0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600]` as the major-step candidates and choose the first whose pixel width is at least 72. Render snap-interval minor ticks only when they are at least 8 pixels apart and are not also major ticks.

  ```ts
  export interface TimelineViewportWindow {
    visibleStartSeconds: number;
    visibleEndSeconds: number;
    renderStartSeconds: number;
    renderEndSeconds: number;
  }

  export interface TimelineTickSeries {
    majorSeconds: number[];
    minorSeconds: number[];
  }

  const majorStepCandidates = [0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600] as const;

  export function timelineViewportWindow(input: {
    durationSeconds: number;
    pixelsPerSecond: number;
    scrollLeft: number;
    viewportWidth: number;
    overscanViewports?: number;
  }): TimelineViewportWindow {
    const pixelsPerSecond = Math.max(0.001, input.pixelsPerSecond);
    const viewportSeconds = Math.max(0, input.viewportWidth) / pixelsPerSecond;
    const visibleStartSeconds = Math.max(0, input.scrollLeft / pixelsPerSecond);
    const visibleEndSeconds = Math.min(input.durationSeconds, visibleStartSeconds + viewportSeconds);
    const overscanSeconds = viewportSeconds * (input.overscanViewports ?? 1);
    return {
      visibleStartSeconds: Number(visibleStartSeconds.toFixed(3)),
      visibleEndSeconds: Number(visibleEndSeconds.toFixed(3)),
      renderStartSeconds: Number(Math.max(0, visibleStartSeconds - overscanSeconds).toFixed(3)),
      renderEndSeconds: Number(Math.min(input.durationSeconds, visibleEndSeconds + overscanSeconds).toFixed(3)),
    };
  }

  function ticksBetween(start: number, end: number, step: number) {
    const first = Math.ceil(start / step) * step;
    const values: number[] = [];
    for (let value = first; value <= end + 0.0001; value += step) {
      values.push(Number(value.toFixed(3)));
    }
    return values;
  }

  export function adaptiveTimelineTicks(input: {
    window: Pick<TimelineViewportWindow, "renderStartSeconds" | "renderEndSeconds">;
    pixelsPerSecond: number;
    snapSeconds: number;
  }): TimelineTickSeries {
    const majorStep = majorStepCandidates.find((step) => step * input.pixelsPerSecond >= 72) ?? 600;
    const majorSeconds = ticksBetween(input.window.renderStartSeconds, input.window.renderEndSeconds, majorStep);
    const majorSet = new Set(majorSeconds.map((seconds) => seconds.toFixed(3)));
    const minorSeconds = input.snapSeconds * input.pixelsPerSecond < 8
      ? []
      : ticksBetween(input.window.renderStartSeconds, input.window.renderEndSeconds, input.snapSeconds)
          .filter((seconds) => !majorSet.has(seconds.toFixed(3)));
    return { majorSeconds, minorSeconds };
  }

  export function timelineItemIntersectsWindow(
    item: { id: string; startSeconds: number; durationSeconds: number },
    window: Pick<TimelineViewportWindow, "renderStartSeconds" | "renderEndSeconds">,
    persistentItemIds: ReadonlySet<string>,
  ) {
    if (persistentItemIds.has(item.id)) return true;
    const endSeconds = item.startSeconds + item.durationSeconds;
    return endSeconds >= window.renderStartSeconds && item.startSeconds <= window.renderEndSeconds;
  }
  ```

- [ ] **Step 4: Run the viewport tests**

  Run: `rtk pnpm test -- src/lib/timeline-viewport.test.ts`

  Expected: PASS.

- [ ] **Step 5: Commit the viewport model**

  ```bash
  rtk git add src/lib/timeline-viewport.ts src/lib/timeline-viewport.test.ts
  rtk git commit -m "feat(timeline): add viewport bounded rendering model"
  ```

### Task 2: Add one pure edit evaluator for preview and commit

**Files:**
- Create: `src/lib/timeline-edit-evaluator.ts`
- Create: `src/lib/timeline-edit-evaluator.test.ts`
- Modify: `src/lib/timeline-snap.ts`
- Modify: `src/lib/timeline-snap.test.ts`

**Interfaces:**
- Consumes: `Timeline`, `TimelineItemKind`, `itemAllowedOnTrack`, `resolveTimelineSnap`, and `StickyTimelineSnap`.
- Produces: `evaluateTimelineMove(input): TimelineMoveEvaluation`, `evaluateTimelineResize(input): TimelineResizeEvaluation`, and `TimelineEditReason`.
- Consumed by: Task 4.

- [ ] **Step 1: Write failing evaluator tests**

  Cover exact-seam acceptance, incompatible and locked tracks, collision rejection, atomic group rejection, legacy overlap preservation, and right-resize clamping. The resize assertion is the regression captured in `05-overlapping-resize.png`.

  ```ts
  const result = evaluateTimelineResize({
    timeline,
    itemId: "opening",
    edge: "right",
    proposedStartSeconds: 0,
    proposedDurationSeconds: 4.5,
    guideSeconds: null,
  });
  expect(result.state).toBe("clamped");
  expect(result.placement).toMatchObject({ startSeconds: 0, durationSeconds: 4 });
  expect(result.reason).toMatchObject({
    code: "collision",
    blockingItemId: "alternate",
    message: "Overlaps Restored Edison alternate",
  });
  expect(result.patch).toEqual({ type: "resizeItem", itemId: "opening", durationSeconds: 4 });
  ```

  Add a group test that moves two clips into one occupied destination and expects `state: "rejected"`, `patches: []`, and no partial placement.

- [ ] **Step 2: Run evaluator and snap tests in the red state**

  Run: `rtk pnpm test -- src/lib/timeline-edit-evaluator.test.ts src/lib/timeline-snap.test.ts`

  Expected: FAIL because the evaluator module and neighboring-edge target kind do not exist.

- [ ] **Step 3: Extend snap targets with a seam kind**

  Change `TimelineSnapTarget["kind"]` to `"playhead" | "edit" | "seam"`. Keep playhead priority first, seam second, and generic edit points third. Existing callers remain valid.

  ```ts
  function targetPriority(kind: TimelineSnapTarget["kind"]) {
    if (kind === "playhead") return 0;
    if (kind === "seam") return 1;
    return 2;
  }
  ```

- [ ] **Step 4: Implement the evaluator contracts**

  Define these stable result shapes:

  ```ts
  export type TimelineEditState = "accepted" | "clamped" | "rejected";
  export type TimelineEditReasonCode = "track_locked" | "track_incompatible" | "collision";

  export interface TimelineEditReason {
    code: TimelineEditReasonCode;
    message: string;
    trackId: string;
    blockingItemId: string | null;
  }

  export interface TimelineResolvedPlacement {
    itemId: string;
    trackId: string;
    startSeconds: number;
    durationSeconds: number;
  }

  export interface TimelineMoveEvaluation {
    state: TimelineEditState;
    placements: readonly TimelineResolvedPlacement[];
    patches: readonly { itemId: string; targetTrackId: string; startSeconds: number }[];
    reason: TimelineEditReason | null;
    guideSeconds: number | null;
  }

  export interface TimelineResizeEvaluation {
    state: TimelineEditState;
    placement: TimelineResolvedPlacement;
    patch: TimelinePatch | null;
    reason: TimelineEditReason | null;
    guideSeconds: number | null;
  }
  ```

  Use half-open intervals: items collide only when `max(startA, startB) < min(endA, endB) - 0.001`. Exclude all moving item IDs when evaluating a group against stationary items. Compare proposed pair overlap against canonical pair overlap so a legacy overlap may remain equal but cannot increase. Resize clamps to the nearest stationary boundary and returns `state: "clamped"`; move/group collisions return `state: "rejected"` and no patches. Every accepted or clamped result contains the exact patch data that Task 4 will commit.

- [ ] **Step 5: Run evaluator and snap tests**

  Run: `rtk pnpm test -- src/lib/timeline-edit-evaluator.test.ts src/lib/timeline-snap.test.ts`

  Expected: PASS.

- [ ] **Step 6: Commit the shared edit model**

  ```bash
  rtk git add src/lib/timeline-edit-evaluator.ts src/lib/timeline-edit-evaluator.test.ts src/lib/timeline-snap.ts src/lib/timeline-snap.test.ts
  rtk git commit -m "feat(timeline): centralize edit evaluation"
  ```

### Task 3: Reject new overlaps in canonical Rust project actions

**Files:**
- Modify: `src-tauri/src/project/action.rs:654-880,882-1170,1843-2055,6635-8045`

**Interfaces:**
- Consumes: pre-action and post-action `Timeline` values inside atomic `apply_project_action`.
- Produces: `ProjectActionError::TrackItemOverlap { track_id, item_id, blocking_item_id }`.

- [ ] **Step 1: Add failing atomic overlap tests**

  Add tests for `MoveItems`, `ResizeItems`, and `TrimItems`. Each test creates a second video item at `4.0-8.0`, attempts to extend or move `item-1` into it, expects `TrackItemOverlap`, and asserts the full project equals its pre-action clone.

  ```rust
  assert_eq!(
      result,
      Err(ProjectActionError::TrackItemOverlap {
          track_id: "track-video".to_string(),
          item_id: "item-1".to_string(),
          blocking_item_id: "video-after".to_string(),
      })
  );
  assert_eq!(project, before);
  ```

  Add one test whose project already has a 0.25-second legacy overlap; moving both unrelated captions without increasing that overlap must succeed.

- [ ] **Step 2: Run the Rust tests and verify failure**

  Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project::action::tests::resize_items_reject_new_same_track_overlap -- --exact --nocapture`

  Expected: FAIL because the action currently accepts the resize.

- [ ] **Step 3: Add overlap-map validation before atomic commit**

  Add the error variant:

  ```rust
  #[error("timeline items overlap on track {track_id}: {item_id} overlaps {blocking_item_id}")]
  TrackItemOverlap {
      track_id: String,
      item_id: String,
      blocking_item_id: String,
  },
  ```

  Add `timeline_overlap_amounts(&Timeline) -> BTreeMap<(String, String, String), f64>` that sorts item IDs for stable keys and stores only overlap greater than `0.001`. Add `validate_no_increased_timeline_overlap(before, after)` that rejects when an after-value exceeds its before-value by more than `0.001`.

  Before consuming `action`, compute:

  ```rust
  let validates_overlap = matches!(
      &action,
      ProjectAction::AddItems { .. }
          | ProjectAction::InsertItems { .. }
          | ProjectAction::MoveItems { .. }
          | ProjectAction::ReorderItems { .. }
          | ProjectAction::ResizeItems { .. }
          | ProjectAction::TrimItems { .. }
          | ProjectAction::RippleTrimItem { .. }
          | ProjectAction::SplitItems { .. }
  );
  ```

  After action mutation and before `*project = next_project`, call the validator when `validates_overlap` is true. This location preserves the existing clone-and-commit atomicity and allows unchanged legacy overlap.

- [ ] **Step 4: Run focused and module Rust tests**

  Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml project::action::tests -- --nocapture`

  Expected: PASS.

- [ ] **Step 5: Commit canonical validation**

  ```bash
  rtk git add src-tauri/src/project/action.rs
  rtk git commit -m "fix(timeline): reject increased track overlaps"
  ```

### Task 4: Make pointer selection authoritative and coalesce preview frames

**Files:**
- Create: `src/lib/timeline-interaction-frame.ts`
- Create: `src/lib/timeline-interaction-frame.test.ts`
- Modify: `src/components/workspace/timeline-editor.tsx:1670-1755,2600-2900,4760-4927`
- Modify: `src/components/workspace/timeline-editor.test.tsx:4760-5760`

**Interfaces:**
- Consumes: Task 2 evaluator results.
- Produces: one `TimelineInteractionPreview` ref/state and a frame coalescer with `schedule`, `flush`, and `cancel`.

- [ ] **Step 1: Add failing selection, cancellation, and frame-coalescing tests**

  Add a component regression with a small controlled harness whose `onSelectItem` updates its `selectedItemId` state. Start with Caption 1 selected, pointer-down Opening clip, and expect `onSelectItem("item-1")` before any pointer move. After the harness rerenders and movement begins, only Opening clip carries `data-dragging="true"`, Opening clip owns the resize handles, and Caption 1 owns none.

  Add a pure coalescer test with fake `requestFrame`/`cancelFrame`: schedule three coordinates, execute one queued callback, and assert the evaluator ran once with the third coordinate. Assert `flush` synchronously returns the latest result and `cancel` prevents delivery.

- [ ] **Step 2: Run the red tests**

  Run: `rtk pnpm test -- src/lib/timeline-interaction-frame.test.ts src/components/workspace/timeline-editor.test.tsx`

  Expected: FAIL because selection occurs on click and pointer movement updates React state for every event.

- [ ] **Step 3: Implement the generic frame coalescer**

  ```ts
  export function createTimelineFrameCoalescer<TInput, TResult>(input: {
    evaluate: (value: TInput) => TResult;
    deliver: (result: TResult) => void;
    requestFrame: (callback: FrameRequestCallback) => number;
    cancelFrame: (handle: number) => void;
  }) {
    let pending: TInput | null = null;
    let handle: number | null = null;
    const run = () => {
      handle = null;
      if (pending === null) return null;
      const value = pending;
      pending = null;
      const result = input.evaluate(value);
      input.deliver(result);
      return result;
    };
    return {
      schedule(value: TInput) {
        pending = value;
        if (handle === null) handle = input.requestFrame(run);
      },
      flush(value?: TInput) {
        if (value !== undefined) pending = value;
        if (handle !== null) input.cancelFrame(handle);
        handle = null;
        return run();
      },
      cancel() {
        if (handle !== null) input.cancelFrame(handle);
        handle = null;
        pending = null;
      },
    };
  }
  ```

- [ ] **Step 4: Integrate authoritative selection and evaluator-backed preview**

  On pointer-down, calculate the drag selection before pointer capture:

  ```ts
  const additive = event.metaKey || event.ctrlKey;
  const dragItemIds = selectedItemIdSet.has(item.id)
    ? new Set(selectedItemIdSet)
    : additive
      ? new Set([...selectedItemIdSet, item.id])
      : new Set([item.id]);
  selectTimelineItem(item.id, additive);
  const groupItems = items.filter((candidate) => dragItemIds.has(candidate.id));
  ```

  Replace per-pointer-event `setActiveInteraction` with the coalescer. Its evaluator calls `evaluateTimelineMove` or `evaluateTimelineResize`; its deliver callback updates only `interactionPreview`. Pointer-up calls `flush` with the release coordinates and commits the returned result’s patches. Rejected results emit no patch. `pointercancel`, lost capture, `Escape`, and unmount call `cancel`, clear the preview, snap state, target reason, and live message.

- [ ] **Step 5: Run interaction tests**

  Run: `rtk pnpm test -- src/lib/timeline-interaction-frame.test.ts src/components/workspace/timeline-editor.test.tsx`

  Expected: PASS, including existing move, group move, Option-drag, resize, ripple, cancel, and keyboard tests.

- [ ] **Step 6: Commit the interaction slice**

  ```bash
  rtk git add src/lib/timeline-interaction-frame.ts src/lib/timeline-interaction-frame.test.ts src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
  rtk git commit -m "fix(timeline): align selection preview and commit"
  ```

### Task 5: Virtualize ruler, grid, clips, and filmstrip work

**Files:**
- Create: `src/components/workspace/timeline-ruler.tsx`
- Create: `src/components/workspace/timeline-ruler.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx:1850-1975,3820-4305`
- Modify: `src/components/workspace/timeline-editor.test.tsx:318-545`

**Interfaces:**
- Consumes: Task 1 `TimelineViewportWindow` and `TimelineTickSeries`.
- Produces: `TimelineRuler` and a visible-item set shared by canvas markup and filmstrip requests.

- [ ] **Step 1: Add failing long-duration ruler and item-window tests**

  Render a 1,800-second timeline with a mocked 1,200-pixel scroll viewport. Assert no more than 200 ruler elements and 200 grid elements, no off-window ordinary clip, continued presence of an off-window selected clip, and filmstrip requests only for visible/overscanned video items. Assert the playhead and hover badges never render within 80 pixels of each other.

- [ ] **Step 2: Run the red component tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-ruler.test.tsx src/components/workspace/timeline-editor.test.tsx`

  Expected: FAIL because the editor creates full-duration ticks, grids, clips, and filmstrip requests.

- [ ] **Step 3: Implement `TimelineRuler`**

  Give the component exact props for `canvasWidth`, `pixelsPerSecond`, `ticks`, `playheadSeconds`, `hoverSeconds`, `viewportWindow`, and `onSeek`. Major ticks remain buttons with `Seek timeline to <time>`. Minor ticks are noninteractive. Clamp badges to the visible window and suppress hover when its pixel distance from the playhead is below 80.

- [ ] **Step 4: Observe viewport width and apply the shared visible window**

  Add `viewportWidth` and `scrollLeft` state beside existing refs. A `ResizeObserver` updates width; both ruler and canvas scroll handlers update the single scroll value while preserving peer synchronization. Derive the window/ticks with Task 1 functions. Filter rendered positioned items and `onRequestFilmstrip` calls through `timelineItemIntersectsWindow`, passing selected and actively edited IDs as persistent IDs.

- [ ] **Step 5: Run ruler/editor tests and lint**

  Run: `rtk pnpm test -- src/lib/timeline-viewport.test.ts src/components/workspace/timeline-ruler.test.tsx src/components/workspace/timeline-editor.test.tsx`

  Expected: PASS.

  Run: `rtk pnpm lint`

  Expected: PASS.

- [ ] **Step 6: Commit viewport rendering**

  ```bash
  rtk git add src/components/workspace/timeline-ruler.tsx src/components/workspace/timeline-ruler.test.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
  rtk git commit -m "perf(timeline): bound rendering to visible time"
  ```

### Task 6: Apply the approved track, clip, toolbar, and feedback design

**Files:**
- Create: `src/components/workspace/timeline-item.tsx`
- Create: `src/components/workspace/timeline-item.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx:430-590,3700-4760`
- Modify: `src/components/workspace/timeline-editor.test.tsx:180-450,3000-3110,5470-5760`

**Interfaces:**
- Consumes: `TimelineResolvedPlacement`, existing special visual layers, and exact width thresholds.
- Produces: `TimelineItemShell`, visible track-local reason text, and a deduplicated live status.

- [ ] **Step 1: Add failing visual-contract tests**

  Assert fixed `shrink-0` 32-pixel selection/zoom buttons; the exact `clamp(152px, 15vw, 220px)` track column; visible text `V1 Video`, `H1 HyperFrames`, `O1 Overlays`, `C1 Captions`, and `A1 Audio`; 44-pixel minimum rows; a 16-pixel resize hit target; distinct `data-selected`, `data-focused`, and `data-dragging`; target-local reason text; one polite live region update per changed reason; and reduced-motion classes on animated state changes.

  In `timeline-item.test.tsx`, render widths 40, 80, 160, and 240 and assert the disclosure sequence: no body text, title, timeline timing, and rich visual slot. Give the item deliberately different `sourceIn`/`sourceOut` properties and assert the medium body prints timeline start/end rather than source timing.

- [ ] **Step 2: Run the red tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-editor.test.tsx`

  Expected: FAIL against the current 7-pixel toolbar controls, lane-code-only labels, source-range metadata, and color-only target state.

- [ ] **Step 3: Implement adaptive `TimelineItemShell`**

  The component accepts `item`, `pixelWidth`, `timelineStartSeconds`, state booleans, `visuals`, `status`, and pointer/focus handlers. It computes density from the four approved thresholds, exposes the item label through `aria-label`, and prints current timeline timing with `formatTimecode(start)-formatTimecode(end)` only at medium/rich density. Keep source ranges out of the body.

  Render the resize edge as a 16-pixel transparent button whose inner 3-pixel rule is amber only when selected. Add an interaction bubble containing active-edge time and resulting duration.

- [ ] **Step 4: Rebuild timeline layout and feedback**

  Use a `flex min-h-0 flex-1` timeline body; make the track viewport `overflow-auto`; keep toolbar, ruler, and later overview outside that scroll owner. Apply the exact track column and row heights. Use full track identity text plus existing enable/lock controls. Make target rows render their `TimelineEditReason.message` inside the row and mirror only changed text to `aria-live="polite" aria-atomic="true"`.

- [ ] **Step 5: Run component tests and constrained workspace tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx`

  Expected: PASS; the final track remains scroll-reachable and no control shrinks below its contract.

- [ ] **Step 6: Commit approved timeline styling**

  ```bash
  rtk git add src/components/workspace/timeline-item.tsx src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
  rtk git commit -m "feat(timeline): apply approved editing hierarchy"
  ```

### Task 7: Add the canvas overview navigator

**Files:**
- Create: `src/components/workspace/timeline-overview.tsx`
- Create: `src/components/workspace/timeline-overview.test.tsx`
- Modify: `src/components/workspace/timeline-editor.tsx:1850-1890,4800-4927`
- Modify: `src/components/workspace/timeline-editor.test.tsx:3890-3935`

**Interfaces:**
- Consumes: timeline, duration, current visible window, canvas width, and the existing `{ zoomPercent, scrollLeft }` view state.
- Produces: `onWindowChange({ startSeconds, endSeconds })` and `onPanByFraction(fraction)`.

- [ ] **Step 1: Add failing geometry and interaction tests**

  Mock a 1,000-pixel overview for a 100-second project whose visible window is `20-40`. Assert the viewport frame is at `left: 200px; width: 200px`. Click at 75 seconds and expect the main window to center on `65-85`. Drag the frame and each edge, then test `ArrowLeft`/`ArrowRight` pan by 10 percent of visible duration. Assert there is no `Fit timeline zoom` or full-screen button.

- [ ] **Step 2: Run the red overview tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-overview.test.tsx src/components/workspace/timeline-editor.test.tsx`

  Expected: FAIL because the overview does not exist.

- [ ] **Step 3: Implement overview geometry and canvas drawing**

  Draw one horizontal band per track. Paint item bounds with existing track-kind colors; use cached preview frames or waveform summaries when supplied, otherwise keep the semantic color block. Draw only to one `<canvas>` and overlay one focusable viewport-frame button plus two pointer handles. Do not create per-item DOM nodes.

  Convert a requested time window back to view state in the parent:

  ```ts
  const nextPixelsPerSecond = viewportWidth / Math.max(0.25, endSeconds - startSeconds);
  const nextZoomPercent = clampZoomPercent((nextPixelsPerSecond / basePixelsPerSecond) * 100);
  const resolvedPixelsPerSecond = basePixelsPerSecond * (nextZoomPercent / 100);
  const nextScrollLeft = startSeconds * resolvedPixelsPerSecond;
  setZoomPercent(nextZoomPercent);
  applyTimelineScroll("overview", nextScrollLeft);
  ```

  Replace `synchronizeTimelineScroll(source: "ruler" | "canvas", ...)` with `applyTimelineScroll(source: "ruler" | "canvas" | "overview", ...)`. Ruler-originated updates assign canvas scroll; canvas-originated updates assign ruler scroll; overview-originated updates assign both before emitting `onViewStateChange`.

- [ ] **Step 4: Integrate the fixed overview footer**

  Render `TimelineOverview` below the internally scrolling track viewport. Move the existing minus/plus zoom controls beside it without adding Fit or expansion commands. Ensure overview, ruler, and canvas consume the same view state and cannot recurse through scroll callbacks.

- [ ] **Step 5: Run overview and view-state tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-overview.test.tsx src/components/workspace/timeline-editor.test.tsx`

  Expected: PASS.

- [ ] **Step 6: Commit the navigator**

  ```bash
  rtk git add src/components/workspace/timeline-overview.tsx src/components/workspace/timeline-overview.test.tsx src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
  rtk git commit -m "feat(timeline): add project overview navigator"
  ```

### Task 8: Route canonical rejection errors back into the timeline

**Files:**
- Modify: `src/components/workspace/timeline-editor.tsx:96-180,1602-1666,4800-4927`
- Modify: `src/components/workspace/editor-workspace.tsx:3489,4525-4555,10500-10710`
- Modify: `src/components/workspace/editor-workspace.test.tsx`
- Modify: `src/components/workspace/timeline-editor.test.tsx`

**Interfaces:**
- Produces: `interactionError?: string | null` on `TimelineEditor`.
- Consumes: existing `timelinePatchError` state.

- [ ] **Step 1: Add failing error-routing tests**

  Render `TimelineEditor interactionError="timeline items overlap on track track-video: item-1 overlaps video-after"`. Expect a target-adjacent `role="alert"` containing `Overlaps video-after` and the live region to announce it once. In the workspace test, make the project-action call reject and assert the message appears inside `Timeline editor`, while the existing workspace alert remains available.

- [ ] **Step 2: Run the red tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx`

  Expected: FAIL because the error only renders below the timeline component.

- [ ] **Step 3: Add and wire `interactionError`**

  Pass `interactionError={timelinePatchError}` from `EditorWorkspace`. Map `TrackItemOverlap` strings to concise `Overlaps <blocking item>` copy; preserve the full native message in `title`. Render the local alert at the current target row when available, otherwise in a fixed timeline-status strip above the overview. Clear it when a new interaction begins; keep the workspace-level alert for persistence failures and nonlocal actions.

- [ ] **Step 4: Run workspace and timeline tests**

  Run: `rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx`

  Expected: PASS.

- [ ] **Step 5: Commit error routing**

  ```bash
  rtk git add src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.tsx src/components/workspace/editor-workspace.test.tsx
  rtk git commit -m "fix(timeline): show canonical rejection at target"
  ```

### Task 9: Add deterministic responsive, collision, and performance evidence

**Files:**
- Modify: `src/lib/modern-editor-visual-qa-fixtures.ts`
- Modify: `scripts/browser-visual-qa.mjs:87-130,590-880,1622-1705`
- Modify: `src/browser-visual-qa-palmier-scenarios.test.ts`
- Modify: `src/browser-visual-qa-script.test.ts`
- Modify only after approved comparison: `docs/visual-qa/browser-visual-baseline/modern-editor-timeline-*.png`

**Interfaces:**
- Produces scenarios: `modern-editor-timeline-selection-drag`, `modern-editor-timeline-collision`, `modern-editor-timeline-long-project`, `modern-editor-timeline-1280x720`, and `modern-editor-timeline-1024x720`.

- [ ] **Step 1: Add failing scenario-contract tests**

  Extend `ModernEditorVisualQaScenarioId`, its fixture map, and the required scenario list with the five exact IDs. Assert viewports 1440×900, 1280×720, and 1024×720. Assert the generated browser script checks: full track names, toolbar target sizes, final-track scroll reachability, target-local rejection text, overview visibility, and at most 400 `[data-timeline-tick]` plus `[data-timeline-grid]` elements.

- [ ] **Step 2: Run the red scenario tests**

  Run: `rtk pnpm test -- src/browser-visual-qa-palmier-scenarios.test.ts src/browser-visual-qa-script.test.ts`

  Expected: FAIL because the scenarios and assertions are absent.

- [ ] **Step 3: Add the 30-minute deterministic project fixture**

  Add `"timeline-long-project"` to `ModernEditorVisualQaProjectState`. In `applyModernEditorVisualQaProjectFixture`, expand the active timeline to 1,800 seconds and add deterministic nonoverlapping four-second clips every 12 seconds across video, caption, and audio tracks. Reuse existing sample media IDs; do not create network-backed assets.

- [ ] **Step 4: Add interaction preparation and geometry assertions**

  The selection-drag scenario starts with Caption 1 selected, pointer-downs Opening clip, and waits for Opening clip to become selected before moving. The collision scenario right-resizes Opening clip past Restored Edison alternate and waits for visible `Overlaps Restored Edison alternate`. The responsive scenarios assert every toolbar button is at least 28×28, the selection and zoom buttons are at least 32×32, and the audio row can be reached by scrolling the timeline viewport. The long-project scenario asserts the tick/grid ceiling and records one move plus resize trace marker.

- [ ] **Step 5: Run scenario tests and targeted browser captures**

  Run: `rtk pnpm test -- src/browser-visual-qa-palmier-scenarios.test.ts src/browser-visual-qa-script.test.ts`

  Expected: PASS.

  Start the app: `rtk pnpm dev`

  In a second terminal run each exact scenario:

  ```bash
  rtk pnpm visual:qa:browser -- --only modern-editor-timeline-selection-drag --out output/design-review/timeline-redesign
  rtk pnpm visual:qa:browser -- --only modern-editor-timeline-collision --out output/design-review/timeline-redesign
  rtk pnpm visual:qa:browser -- --only modern-editor-timeline-1280x720 --out output/design-review/timeline-redesign
  rtk pnpm visual:qa:browser -- --only modern-editor-timeline-1024x720 --out output/design-review/timeline-redesign
  rtk pnpm visual:qa:browser -- --only modern-editor-timeline-long-project --out output/design-review/timeline-redesign
  ```

  Expected: five nonblank screenshots; every scripted geometry and DOM ceiling assertion passes.

- [ ] **Step 6: Compare the 1440×900 capture with the approved mockup**

  Use `docs/superpowers/specs/assets/2026-07-18-timeline-ux-redesign-option-2.png` as the reference. Inspect the side-by-side images for track identity, ruler density, selected/dragged distinction, resize bubble, target-local reason, internal scrolling, overview geometry, borders, spacing, and typography. Fix mismatches in the owning task’s files before refreshing any baseline.

- [ ] **Step 7: Commit visual and performance evidence**

  ```bash
  rtk git add src/lib/modern-editor-visual-qa-fixtures.ts scripts/browser-visual-qa.mjs src/browser-visual-qa-palmier-scenarios.test.ts src/browser-visual-qa-script.test.ts docs/visual-qa/browser-visual-baseline/modern-editor-timeline-selection-drag.png docs/visual-qa/browser-visual-baseline/modern-editor-timeline-collision.png docs/visual-qa/browser-visual-baseline/modern-editor-timeline-long-project.png docs/visual-qa/browser-visual-baseline/modern-editor-timeline-1280x720.png docs/visual-qa/browser-visual-baseline/modern-editor-timeline-1024x720.png
  rtk git commit -m "test(timeline): add redesign visual and performance evidence"
  ```

### Task 10: Run full verification and document measured performance

**Files:**
- Modify only if evidence needs a durable note: `docs/superpowers/specs/2026-07-18-timeline-ux-redesign-design.md`

**Interfaces:**
- Consumes: all previous task commits.
- Produces: complete test, visual, accessibility, and performance evidence.

- [ ] **Step 1: Run all timeline and workspace tests**

  Run:

  ```bash
  rtk pnpm test -- src/lib/timeline-viewport.test.ts src/lib/timeline-edit-evaluator.test.ts src/lib/timeline-interaction-frame.test.ts src/lib/timeline-snap.test.ts src/components/workspace/timeline-ruler.test.tsx src/components/workspace/timeline-item.test.tsx src/components/workspace/timeline-overview.test.tsx src/components/workspace/timeline-editor.test.tsx src/components/workspace/editor-workspace.test.tsx src/browser-visual-qa-palmier-scenarios.test.ts src/browser-visual-qa-script.test.ts
  ```

  Expected: PASS with zero failed tests.

- [ ] **Step 2: Run Rust action tests, formatting, and clippy**

  ```bash
  rtk env VIDEO_CREATER_HEADLESS_RUST_SUITE=1 cargo test --manifest-path src-tauri/Cargo.toml project::action::tests -- --nocapture
  rtk pnpm rust:fmt
  rtk pnpm rust:clippy
  ```

  `VIDEO_CREATER_HEADLESS_RUST_SUITE=1` keeps the all-target selector deterministic in a headless shell by explicitly deferring the four dedicated native AppKit/GES harness lanes: `media_inspection_appkit`, `precompose_alpha_ges`, `project_export_prores_appkit`, and `project_export_nested_effect_appkit`. Each harness prints its deferral; the dedicated native lanes run separately without the flag.

  Expected: PASS with no formatting or warning failures.

- [ ] **Step 3: Run TypeScript validation and the complete test suite**

  ```bash
  rtk pnpm lint
  rtk pnpm test
  ```

  Expected: PASS with zero TypeScript errors and zero failed tests.

- [ ] **Step 4: Run the complete browser visual QA**

  With `rtk pnpm dev` running, execute:

  ```bash
  rtk pnpm visual:qa:browser -- --out output/design-review/timeline-redesign-full
  ```

  Expected: PASS with all current and new scenarios nonblank and geometrically valid.

- [ ] **Step 5: Record the required long-project trace**

  In the 30-minute scenario, record one Opening clip move and one right-edge resize. Confirm the trace contains no pointer interaction task above 50 ms and that preview updates occur at no more than one delivery per animation frame. Add the measured maximum task duration and tick/grid count to the design spec only when measured values are available; use exact numbers, not qualitative wording.

- [ ] **Step 6: Inspect the final workspace and commit evidence notes if changed**

  Verify 1440×900, 1280×720, and 1024×720 manually: full track names, reachable audio, readable ruler, authoritative selection, collision prevention, target-local explanation, resize acquisition, overview pan/zoom, keyboard focus, and reduced motion.

  If the design spec gained measured evidence:

  ```bash
  rtk git add docs/superpowers/specs/2026-07-18-timeline-ux-redesign-design.md
  rtk git commit -m "docs(timeline): record redesign verification evidence"
  ```

  Otherwise do not create an empty commit.
