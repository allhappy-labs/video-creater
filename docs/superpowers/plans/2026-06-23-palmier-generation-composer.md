# Palmier Generation Composer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the existing Media panel generation composer into a Palmier-style editor composer with visual reference slots and a compact model/settings footer while preserving generated-asset request payloads.

**Architecture:** Keep the implementation inside `src/components/workspace/media-bin.tsx` because the composer state and submit contract already live there. Add small render helpers for reference slots and footer metadata, avoiding a new component split until the composer grows beyond this slice. Use focused React Testing Library tests in `src/components/workspace/media-bin.test.tsx` to lock visual slot behavior and request compatibility.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, Vitest, React Testing Library, existing shadcn-style `Button` primitive, lucide-react icons.

---

### Task 1: Lock Composer Visual Slot And Footer Behavior

**Files:**
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Write failing tests for slot and footer behavior**

Add tests after `shows selected generation references as visible media chips`:

```tsx
it("renders Palmier-style generation slots and footer metadata", () => {
  render(
    <MediaBin
      media={visualReferenceMedia}
      selectedMediaId="media-2"
      onGenerateMedia={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Generate media" }));

  const composer = screen.getByRole("region", { name: "Media generation" });
  expect(within(composer).getByRole("group", { name: "First frame slot" })).toHaveTextContent(
    "media-2-camera.mp4",
  );
  expect(within(composer).getByRole("group", { name: "Last frame slot" })).toHaveTextContent(
    "Drop a closing frame",
  );
  expect(within(composer).getByRole("group", { name: "Reference slot" })).toHaveTextContent(
    "media-2-camera.mp4",
  );
  expect(within(composer).getByText("seedance/seedance-2-fast")).toBeInTheDocument();
  expect(within(composer).getByText("1280x720")).toBeInTheDocument();
  expect(within(composer).getByText("4 credits est.")).toBeInTheDocument();
});

it("shows audio generation as a prompt-and-model composer without visual slots", () => {
  render(
    <MediaBin
      media={visualReferenceMedia}
      selectedMediaId="media-2"
      onGenerateMedia={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Generate media" }));
  fireEvent.click(screen.getByRole("button", { name: "Audio" }));

  const composer = screen.getByRole("region", { name: "Media generation" });
  expect(within(composer).queryByRole("group", { name: "First frame slot" })).not.toBeInTheDocument();
  expect(within(composer).queryByRole("group", { name: "Reference slot" })).not.toBeInTheDocument();
  expect(within(composer).getByText("elevenlabs/music-v1")).toBeInTheDocument();
  expect(within(composer).getByText("12 credits est.")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `rtk pnpm test -- src/components/workspace/media-bin.test.tsx`

Expected: FAIL because `First frame slot`, `Last frame slot`, `Reference slot`, and footer metadata are not rendered yet.

### Task 2: Implement The Palmier-Style Composer Rendering

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Test: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Add helper metadata functions**

Add helpers near existing generation option helpers:

```tsx
function selectedGenerationModel(mode: MediaGenerationMode) {
  return mode === "audio"
    ? { provider: "elevenlabs", id: "music-v1" }
    : { provider: "seedance", id: "seedance-2-fast" };
}

function selectedGenerationCost(mode: MediaGenerationMode, durationValue: string) {
  const duration =
    generationDurationOptions.find((option) => option.value === durationValue) ??
    generationDurationOptions[0];
  return mode === "audio" ? duration.seconds : duration.seconds;
}
```

- [ ] **Step 2: Replace inline model literals in submit**

Update `submitGeneration()` to call `selectedGenerationModel(generationMode)` for the existing `model` field. Keep the output payload unchanged.

- [ ] **Step 3: Add a visual slot render helper**

Add `renderGenerationReferenceSlot(label, mediaId, emptyText, selectLabel, value, onSelect, clearSelection, removeLabel, disabled)` inside `MediaBin`.

The helper must render:

```tsx
<div role="group" aria-label={`${label} slot`}>
  <div>{label}</div>
  <select aria-label={selectLabel} ... />
  {asset ? selected filename/meta/duration/remove button : emptyText}
</div>
```

It must reuse `visualMedia`, `filenameFromPath`, `mediaAssetMeta`, and `formatDurationBadge`.

- [ ] **Step 4: Replace the existing first/last/reference select grid and reference chip list**

In `renderGenerationComposer()`, remove the separate select grid plus `generationReferenceChips` output. Render:

```tsx
{generationMode === "video" ? (
  <div className="grid gap-1.5">
    {renderGenerationReferenceSlot("First frame", ...)}
    {renderGenerationReferenceSlot("Last frame", ...)}
    {renderGenerationReferenceSlot("Reference", ...)}
  </div>
) : null}
{generationMode === "image" ? (
  <div className="grid gap-1.5">
    {renderGenerationReferenceSlot("Reference", ...)}
  </div>
) : null}
```

- [ ] **Step 5: Add compact footer metadata before submit**

Render a footer after the prompt and before `Queue generation`:

```tsx
<div aria-label="Generation settings summary" className="grid grid-cols-2 gap-1 text-[10px]">
  <span>{model.provider}/{model.id}</span>
  <span>{selectedGenerationCost(generationMode, generationDuration)} credits est.</span>
  <span>{generationMode === "audio" ? "Audio" : generationResolution}</span>
  <span>{generationMode === "audio" ? formatGeneratedDuration(settings.durationSeconds) : `${generationAspectRatio} - ${generationDuration}`}</span>
</div>
```

- [ ] **Step 6: Run focused tests**

Run: `rtk pnpm test -- src/components/workspace/media-bin.test.tsx`

Expected: PASS.

### Task 3: Verify Workspace Integration And Visual QA

**Files:**
- Modify only if verification exposes defects.

- [ ] **Step 1: Run focused workspace tests**

Run: `rtk pnpm test -- src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx`

Expected: PASS.

- [ ] **Step 2: Run TypeScript lint**

Run: `rtk pnpm lint`

Expected: PASS.

- [ ] **Step 3: Browser QA the composer**

Run the app with `rtk pnpm dev -- --port 5177`, open `http://127.0.0.1:1420/` via the Playwright CLI wrapper, click `Generate media`, and capture `output/playwright/palmier-generation-composer.png`.

Expected: The composer shows mode tabs, visual slots, prompt, footer metadata, and queue action without overlapping text at the default desktop viewport.

- [ ] **Step 4: Clean temporary Playwright state**

Run: `rtk rm -rf .playwright-cli`

Expected: `.playwright-cli` no longer exists.

### Task 4: Commit The Implementation

**Files:**
- Stage: `src/components/workspace/media-bin.tsx`
- Stage: `src/components/workspace/media-bin.test.tsx`
- Stage: `docs/superpowers/plans/2026-06-23-palmier-generation-composer.md`

- [ ] **Step 1: Check status**

Run: `rtk git status --short`

Expected: only the planned files are modified or added.

- [ ] **Step 2: Commit**

Run:

```bash
rtk git add docs/superpowers/plans/2026-06-23-palmier-generation-composer.md src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx
rtk git commit -m "feat: polish media generation composer"
```

Expected: commit succeeds.

---

## Self-Review

- Spec coverage: visual slots, audio hiding, footer metadata, existing payload compatibility, and browser QA are covered.
- Placeholder scan: no TBD/TODO placeholders.
- Type consistency: all named fields already exist in `MediaGenerationRequest` and `GeneratedAssetSettings`; new helpers only wrap current literals and option state.
