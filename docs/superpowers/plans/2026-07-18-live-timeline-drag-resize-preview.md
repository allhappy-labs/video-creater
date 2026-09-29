# Live Timeline Drag and Resize Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make timeline clips visibly follow drag and resize gestures while emitting one canonical project edit only when the pointer is released.

**Architecture:** Keep transient gesture state inside `TimelineEditor` and derive rendered `left`, `top`, and `width` values from that state. Reuse the same snapped move target and resize patch resolution for the live preview and the pointer-up callback so previewed and committed geometry cannot diverge.

**Tech Stack:** React 19, TypeScript, Vitest, Testing Library, Tailwind CSS, Vite.

## Global Constraints

- Do not mutate the `timeline` prop during pointer movement.
- Do not call `onTimelinePatch`, `onMoveItems`, `onDuplicateItemsAtPositions`, or `onRippleTrim` during pointer movement.
- Pointer release remains the single canonical project mutation and undo boundary.
- Pointer cancellation restores canonical geometry and emits no project action.
- Preserve existing snapping, minimum-duration, source-range, ripple-trim, Option-drag, valid-target, and invalid-target behavior.
- Disable positional CSS transitions only for items participating in the active move or resize gesture.
- Preserve unrelated dirty worktree changes and stage only files named by each task.
- Prefix every shell command with `rtk`.

---

## File Structure

- Modify `src/components/workspace/timeline-editor.tsx`: derive transient move and resize geometry and share final resize-patch resolution with pointer-up commit behavior.
- Modify `src/components/workspace/timeline-editor.test.tsx`: prove live geometry, commit timing, group/cross-track behavior, cancellation, and retained Option-drag/ripple behavior.
- Read `docs/superpowers/specs/2026-07-18-live-timeline-drag-resize-preview-design.md`: acceptance contract for both tasks.

No new production file is needed. The gesture state, snapping functions, target-row resolver, and positioned-item renderer already live together in `TimelineEditor`; extracting a new module would require exposing component-specific state without adding a reusable boundary.

---

### Task 1: Preview Normal and Group Moves on the Rendered Clips

**Files:**
- Modify: `src/components/workspace/timeline-editor.test.tsx:4617-4782`
- Modify: `src/components/workspace/timeline-editor.tsx:4301-4355`

**Interfaces:**
- Consumes: existing `activeInteraction`, `activeMovePreviewDeltaSeconds`, `activeMoveTarget`, `trackGeometry`, and `pixelsPerSecond` values inside `TimelineEditor`.
- Produces: per-item `previewStartSeconds`, `previewTrackId`, `previewRowTop`, and `itemHasLiveGeometry` render values used by the positioned item wrapper.

- [ ] **Step 1: Add failing tests for live horizontal and grouped movement**

In the existing horizontal video-drag test, retain the final patch assertion and add assertions immediately after `pointermove`:

```tsx
const clipWrapper = clip.parentElement;
expect(clipWrapper).not.toBeNull();

fireEvent(
  clip,
  new MouseEvent("pointermove", { clientX: 180, bubbles: true }),
);

expect(onPatch).not.toHaveBeenCalled();
expect(clipWrapper).toHaveStyle({ left: "80px" });
expect(clipWrapper?.className).not.toContain("transition-[left,width,opacity]");

fireEvent(
  clip,
  new MouseEvent("pointerup", { clientX: 180, bubbles: true }),
);

expect(onPatch).toHaveBeenCalledTimes(1);
```

In the selected-group test, capture the two wrappers and assert both have moved by the same snapped delta before release:

```tsx
const firstClip = screen.getByRole("button", { name: "A" });
const secondClip = screen.getByRole("button", { name: "B" });
const firstWrapper = firstClip.parentElement;
const secondWrapper = secondClip.parentElement;
expect(firstWrapper).not.toBeNull();
expect(secondWrapper).not.toBeNull();

fireEvent(firstClip, new MouseEvent("pointerdown", {
  bubbles: true,
  clientX: 80,
  clientY: 20,
}));
fireEvent(firstClip, new MouseEvent("pointermove", {
  bubbles: true,
  clientX: 180,
  clientY: 20,
}));

expect(onMoveItems).not.toHaveBeenCalled();
expect(firstWrapper).toHaveStyle({ left: "184px" });
expect(secondWrapper).toHaveStyle({ left: "344px" });
```

The expected positions use the existing 80 pixels-per-second scale and the snapped `1.3` second group delta already asserted by the final move plan.

In the existing compatible cross-track test, capture the wrapper and prove its row changes before release:

```tsx
const caption = screen.getByRole("button", { name: "Caption 1" });
const wrapper = caption.parentElement;
expect(wrapper).not.toBeNull();
fireEvent(
  caption,
  new MouseEvent("pointerdown", {
    clientX: 100,
    clientY: 100,
    bubbles: true,
  }),
);
fireEvent(
  caption,
  new MouseEvent("pointermove", {
    clientX: 180,
    clientY: 145,
    bubbles: true,
  }),
);

expect(onPatch).not.toHaveBeenCalled();
expect(wrapper).toHaveStyle({ top: "54px" });
```

In the existing incompatible-row test, add the corresponding assertion before pointer release:

```tsx
const wrapper = caption.parentElement;
expect(wrapper).not.toBeNull();

// After the existing pointermove over the video row:
expect(onPatch).not.toHaveBeenCalled();
expect(wrapper).toHaveStyle({ top: "4px" });
```

Add cancellation coverage while the move preview is still expected to fail:

```tsx
it("restores canonical move geometry when pointer drag is cancelled", () => {
  const onPatch = vi.fn();
  render(<TimelineEditor timeline={sampleTimeline} onTimelinePatch={onPatch} />);

  const clip = screen.getByRole("button", { name: "Opening clip" });
  const wrapper = clip.parentElement;
  expect(wrapper).not.toBeNull();
  fireEvent(clip, new MouseEvent("pointerdown", { clientX: 100, bubbles: true }));
  fireEvent(clip, new MouseEvent("pointermove", { clientX: 180, bubbles: true }));
  expect(wrapper).toHaveStyle({ left: "80px" });

  fireEvent(clip, new MouseEvent("pointercancel", { bubbles: true }));

  expect(wrapper).toHaveStyle({ left: "0px" });
  expect(onPatch).not.toHaveBeenCalled();
});
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx -t "creates a move patch when a video clip body is dragged horizontally|moves a selected clip group atomically|targets a compatible track|keeps the original track|restores canonical move geometry"
```

Expected: FAIL because the wrappers still expose their canonical `left` styles until pointer release.

- [ ] **Step 3: Apply transient move geometry to normal dragged items**

Inside `renderedPositionedItems.map`, replace the current ripple-only preview geometry declarations with move-aware values after `itemIsDuplicatePreview` is known:

```tsx
const itemHasMovePreview =
  itemIsDragged &&
  !itemIsDuplicatePreview &&
  activeMovePreviewDeltaSeconds !== null;
const previewTrackId =
  itemHasMovePreview && activeMoveTarget
    ? (activeMoveTarget.targetTrackIds.get(item.id) ?? item.trackId)
    : item.trackId;
const previewStartSeconds =
  rippleShift?.startSeconds ??
  (itemHasMovePreview
    ? Number((item.startSeconds + activeMovePreviewDeltaSeconds).toFixed(3))
    : item.startSeconds);
const previewDurationSeconds =
  rippleResize?.durationSeconds ?? item.durationSeconds;
const previewRowTop =
  trackGeometry.byId.get(previewTrackId)?.top ??
  trackGeometry.entries[item.rowIndex]?.top ??
  0;
const itemHasLiveGeometry = itemHasMovePreview;
```

Update the wrapper class so active normal moves do not use the position transition:

```tsx
className={`absolute ${
  itemHasLiveGeometry ? "transition-none" : "transition-[left,width,opacity]"
} ${
  itemIsRippleShifted || rippleResize || rippleShift
    ? "opacity-70 ring-1 ring-cyan-400"
    : ""
} ${
  itemIsDuplicatePreview
    ? "opacity-65 outline outline-1 outline-dashed outline-amber-300"
    : itemIsDragged
      ? "opacity-75 outline outline-1 outline-cyan-300"
      : ""
}`}
```

Update `top` to consume the resolved preview row:

```tsx
style={{
  left: `${previewStartSeconds * pixelsPerSecond}px`,
  top: `${previewRowTop + trackVerticalInset}px`,
  height: `${Math.max(
    1,
    (trackGeometry.byId.get(previewTrackId)?.height ??
      trackGeometry.entries[item.rowIndex]?.height ??
      defaultTrackDisplayHeight) - trackVerticalInset * 2,
  )}px`,
  width: `${previewDurationSeconds * pixelsPerSecond}px`,
}}
```

Do not change the Option-drag duplicate overlay. Its canonical wrapper stays in place and its existing transformed duplicate remains the live preview.

- [ ] **Step 4: Run the focused move tests and verify GREEN**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx -t "creates a move patch when a video clip body is dragged horizontally|moves a selected clip group atomically|translates compatible group lanes|Option-drags a selected group|targets a compatible track|keeps the original track|shows rejected feedback"
```

Expected: PASS with no callback before pointer release and one existing final move/duplicate callback after release.

- [ ] **Step 5: Commit normal move previews**

```bash
rtk git add src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
rtk git commit -m "feat(timeline): preview clip moves during drag"
```

---

### Task 2: Preview Resize Geometry and Restore It on Cancellation

**Files:**
- Modify: `src/components/workspace/timeline-editor.test.tsx:1058-1249`
- Modify: `src/components/workspace/timeline-editor.tsx:3318-3420`
- Modify: `src/components/workspace/timeline-editor.tsx:4301-4680`

**Interfaces:**
- Consumes: existing `TimelinePatch`, `TimelineInteraction`, `snapInteractionAtPointer`, `snapIfEnabled`, `createLeftTrimPatchFromDrag`, `createRightTrimPatchFromDrag`, and `createResizePatchFromDrag`.
- Produces: `resolvedResizePatch(interaction, clientX): TimelinePatch`, reused for the live resize preview and the final `onTimelinePatch` call.

- [ ] **Step 1: Add failing tests for right-edge and left-edge live geometry**

Extend the existing Lottie right-edge resize test with assertions before release:

```tsx
const wrapper = handle.parentElement;
expect(wrapper).not.toBeNull();

fireEvent(
  handle,
  new MouseEvent("pointermove", { clientX: 180, bubbles: true }),
);

expect(onPatch).not.toHaveBeenCalled();
expect(wrapper).toHaveStyle({ width: "400px" });
expect(wrapper?.className).toContain("transition-none");

fireEvent(handle, new MouseEvent("pointercancel", { bubbles: true }));
expect(wrapper).toHaveStyle({ width: "320px" });
expect(onPatch).not.toHaveBeenCalled();

fireEvent(
  handle,
  new MouseEvent("pointerdown", { clientX: 100, bubbles: true }),
);
fireEvent(
  handle,
  new MouseEvent("pointermove", { clientX: 180, bubbles: true }),
);
fireEvent(
  handle,
  new MouseEvent("pointerup", { clientX: 180, bubbles: true }),
);

expect(onPatch).toHaveBeenCalledTimes(1);
```

This first cancelled gesture proves resize geometry restores without an action; the second gesture proves a release commits once. Add a dedicated left-edge preview test using a caption with deterministic geometry:

```tsx
it("previews left-edge trim geometry before committing once", () => {
  const onPatch = vi.fn();
  render(
    <TimelineEditor
      timeline={{
        durationSeconds: 8,
        tracks: [{
          id: "captions",
          name: "Captions",
          kind: "caption",
          locked: false,
          items: [{
            id: "caption",
            kind: "caption",
            startSeconds: 1,
            durationSeconds: 4,
            source: { type: "text", text: "Preview me" },
            label: "Preview caption",
            properties: {},
          }],
        }],
      }}
      selectedItemId="caption"
      onTimelinePatch={onPatch}
    />,
  );

  const handle = screen.getByLabelText("Resize Preview caption left edge");
  const wrapper = handle.parentElement;
  expect(wrapper).not.toBeNull();
  fireEvent(handle, new MouseEvent("pointerdown", { clientX: 100, bubbles: true }));
  fireEvent(handle, new MouseEvent("pointermove", { clientX: 180, bubbles: true }));

  expect(onPatch).not.toHaveBeenCalled();
  expect(wrapper).toHaveStyle({ left: "160px", width: "240px" });

  fireEvent(handle, new MouseEvent("pointerup", { clientX: 180, bubbles: true }));
  expect(onPatch).toHaveBeenCalledTimes(1);
  expect(onPatch).toHaveBeenCalledWith({
    type: "trimItem",
    itemId: "caption",
    startSeconds: 2,
    durationSeconds: 3,
  });
});
```

- [ ] **Step 2: Run the focused resize/cancel tests and verify RED**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx -t "resizes first-class lottie clips from pointer handles|previews left-edge trim geometry"
```

Expected: FAIL because ordinary resize wrappers still use canonical geometry and resize handles do not clear active interaction state on pointer cancellation.

- [ ] **Step 3: Extract one non-ripple resize patch resolver**

Add this function immediately before `finishInteraction`:

```tsx
function resolvedResizePatch(
  interaction: Extract<
    TimelineInteraction,
    { type: "resizeLeft" | "resizeRight" }
  >,
  clientX: number,
): TimelinePatch {
  const deltaSeconds = secondsFromPointerDelta(
    interaction.pointerStartX,
    clientX,
  );
  const snap = snapInteractionAtPointer(interaction, clientX, false);

  if (interaction.type === "resizeLeft") {
    const rawStartSeconds = interaction.item.startSeconds + deltaSeconds;
    const startSeconds = Number((
      snap.guideSeconds !== null
        ? rawStartSeconds + snap.deltaSeconds
        : snapIfEnabled(rawStartSeconds)
    ).toFixed(3));
    return createLeftTrimPatchFromDrag(interaction.item, startSeconds);
  }

  const rawEndSeconds =
    interaction.item.startSeconds +
    interaction.originalDurationSeconds +
    deltaSeconds;
  const endSeconds = Math.max(
    interaction.item.startSeconds + 0.1,
    snap.guideSeconds !== null
      ? rawEndSeconds + snap.deltaSeconds
      : snapIfEnabled(rawEndSeconds),
  );
  const durationSeconds = Number(
    (endSeconds - interaction.item.startSeconds).toFixed(3),
  );
  return (
    createRightTrimPatchFromDrag(interaction.item, endSeconds) ??
    createResizePatchFromDrag({
      itemId: interaction.item.id,
      durationSeconds,
    })
  );
}
```

In `finishInteraction`, retain the existing ripple branches. Replace both non-ripple resize calculations with one call:

Remove the old `snap` constant at the start of `finishInteraction`; the shared resolver now owns non-ripple resize snapping, while move handling already calls `resolvedMoveDeltaSeconds`.

```tsx
} else if (activeInteraction.ripple) {
  const request = {
    itemId: activeInteraction.item.id,
    edge: activeInteraction.type === "resizeLeft" ? "left" as const : "right" as const,
    deltaSeconds: Number(deltaSeconds.toFixed(3)),
  };
  try {
    onPlanRippleTrim?.(request);
    onRippleTrim?.(request);
  } catch {
    // Invalid ripple trims remain visible and never fall back to ordinary trims.
  }
  setActiveInteraction(null);
  clearTimelineSnap();
  return;
} else {
  onTimelinePatch?.(resolvedResizePatch(activeInteraction, clientX));
}
```

This is a mechanical consolidation of the current right- and left-edge commit calculations; do not change move handling.

- [ ] **Step 4: Derive live resize geometry from the shared patch**

After `activeMovePreviewDeltaSeconds`, derive the current non-ripple resize patch:

```tsx
const activeResizePreviewPatch =
  activeInteraction &&
  activeInteraction.type !== "move" &&
  !activeInteraction.ripple
    ? resolvedResizePatch(
        activeInteraction,
        activeInteraction.pointerStartX +
          activeInteraction.deltaSeconds * pixelsPerSecond,
      )
    : null;
```

Inside the positioned-item render, extend the preview geometry declarations:

```tsx
const resizePreviewPatch =
  activeResizePreviewPatch?.itemId === item.id
    ? activeResizePreviewPatch
    : null;
const resizePreviewStartSeconds =
  resizePreviewPatch?.type === "trimItem"
    ? resizePreviewPatch.startSeconds
    : item.startSeconds;
const resizePreviewDurationSeconds =
  resizePreviewPatch?.type === "trimItem" ||
  resizePreviewPatch?.type === "resizeItem"
    ? resizePreviewPatch.durationSeconds
    : item.durationSeconds;
const previewStartSeconds =
  rippleShift?.startSeconds ??
  (itemHasMovePreview
    ? Number((item.startSeconds + activeMovePreviewDeltaSeconds).toFixed(3))
    : resizePreviewStartSeconds);
const previewDurationSeconds =
  rippleResize?.durationSeconds ?? resizePreviewDurationSeconds;
const itemHasLiveGeometry = itemHasMovePreview || resizePreviewPatch !== null;
```

The wrapper styles and transition class added in Task 1 will now update both resize edges immediately. Clearing `activeInteraction` restores canonical render values.

Add cancellation handlers to both resize buttons so cancelled resize gestures use that same restoration path:

```tsx
onPointerCancel={() => {
  setActiveInteraction(null);
  clearTimelineSnap();
}}
```

- [ ] **Step 5: Run focused timeline tests and verify GREEN**

Run:

```bash
rtk pnpm test -- src/components/workspace/timeline-editor.test.tsx
```

Expected: PASS for the complete timeline editor test file with no failures or warnings.

- [ ] **Step 6: Run TypeScript verification and browser visual QA**

Run:

```bash
rtk pnpm lint
rtk pnpm visual:qa:browser
```

Expected: both commands exit `0`. Inspect the generated desktop and narrow timeline screenshots and confirm moved/resized clips retain readable labels and handles, snap guides remain aligned, and valid/invalid row feedback remains visible.

- [ ] **Step 7: Review the final diff against the design contract**

Run:

```bash
rtk git diff --check
rtk git diff -- src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
```

Confirm the diff contains no parent timeline mutation during pointer movement, no pointer-move callback invocation, no snapping or track-compatibility rule changes, and no unrelated files.

- [ ] **Step 8: Commit resize previews and cancellation coverage**

```bash
rtk git add src/components/workspace/timeline-editor.tsx src/components/workspace/timeline-editor.test.tsx
rtk git commit -m "feat(timeline): preview clip resizes during drag"
```
