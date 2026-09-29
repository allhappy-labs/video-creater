# Media Library Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Palmier-like media-library chrome with item count, sort controls, and folder count cards above the media grid.

**Architecture:** Keep the change inside `MediaBin`. Add local sort state, a sorted visible media array, and a small `renderLibraryChrome()` helper. Existing folder grouping and media tile rendering consume the sorted array instead of the raw filtered array.

**Tech Stack:** React, TypeScript, Tailwind, Vitest, Testing Library.

---

### Task 1: Add Failing Media Bin Tests

**Files:**
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Add a test for library count and folder cards**

Add this test near the folder grouping tests:

```tsx
it("renders project library count, sort control, and folder cards", () => {
  render(
    <MediaBin
      media={[
        {
          ...media[0],
          folderId: "folder-broll",
        },
        {
          id: "generated-shot-1-output",
          relativePath: "generated/generated-shot-1/output.mp4",
          kind: "generated",
          durationSeconds: 4,
          width: 1280,
          height: 720,
          fps: 24,
          folderId: "folder-generated",
        },
        {
          id: "media-unfiled",
          relativePath: "media/unfiled.mp4",
          kind: "video",
          durationSeconds: 2,
          width: 1920,
          height: 1080,
          fps: 24,
        },
      ]}
      mediaFolders={mediaFolders}
    />,
  );

  const library = screen.getByRole("region", { name: "Project library" });
  expect(within(library).getByText("3 items")).toBeInTheDocument();
  expect(within(library).getByLabelText("Sort project media")).toHaveValue("project");
  expect(
    within(library).getByRole("group", { name: "Folder Generated selects" }),
  ).toHaveTextContent("1 item");
  expect(within(library).getByRole("group", { name: "Folder B-roll" })).toHaveTextContent(
    "1 item",
  );
});
```

- [ ] **Step 2: Add a test for name sorting**

```tsx
it("sorts visible unfiled media tiles by filename", () => {
  render(
    <MediaBin
      media={[
        {
          id: "media-z",
          relativePath: "media/z-camera.mp4",
          kind: "video",
          durationSeconds: 2,
          width: 1920,
          height: 1080,
          fps: 24,
        },
        {
          id: "media-a",
          relativePath: "media/a-camera.mp4",
          kind: "video",
          durationSeconds: 2,
          width: 1920,
          height: 1080,
          fps: 24,
        },
      ]}
    />,
  );

  fireEvent.change(screen.getByLabelText("Sort project media"), {
    target: { value: "name" },
  });

  const mediaButtons = screen.getAllByRole("button", { name: /Select media .*camera\.mp4/ });
  expect(mediaButtons.map((button) => button.getAttribute("aria-label"))).toEqual([
    "Select media a-camera.mp4",
    "Select media z-camera.mp4",
  ]);
});
```

- [ ] **Step 3: Run red tests**

Run:

```bash
rtk pnpm test -- src/components/workspace/media-bin.test.tsx
```

Expected: FAIL because `Project library` and `Sort project media` do not exist yet.

### Task 2: Implement Library Chrome

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`

- [ ] **Step 1: Add sort types and helper**

Add:

```tsx
type MediaSortMode = "project" | "name" | "kind";

function sortMediaAssets(assets: MediaAsset[], mode: MediaSortMode) {
  if (mode === "project") {
    return assets;
  }

  const sorted = [...assets];
  sorted.sort((left, right) => {
    if (mode === "kind") {
      const kindCompare = left.kind.localeCompare(right.kind);
      if (kindCompare !== 0) {
        return kindCompare;
      }
    }

    return filenameFromPath(left.relativePath).localeCompare(filenameFromPath(right.relativePath), undefined, {
      sensitivity: "base",
    });
  });
  return sorted;
}
```

- [ ] **Step 2: Add sort state and sorted media**

Inside `MediaBin`, add:

```tsx
const [mediaSortMode, setMediaSortMode] = useState<MediaSortMode>("project");
const visibleMedia = sortMediaAssets(filteredMedia, mediaSortMode);
```

Keep existing search behavior unchanged; only replace the existing filtered `visibleMedia` computation with sorted output.

- [ ] **Step 3: Render library chrome**

Add `renderLibraryChrome()` after the search field. It should render:

- `role="region"` with `aria-label="Project library"`.
- Count text from `media.length`.
- `<select aria-label="Sort project media">`.
- Folder cards for `mediaFolders`, each `role="group"` and `aria-label={`Folder ${folder.name}`}`.

Use direct asset counts per folder for this first slice.

### Task 3: Verify and Commit

**Files:**
- Source: `src/components/workspace/media-bin.tsx`
- Tests: `src/components/workspace/media-bin.test.tsx`
- Docs: `docs/superpowers/specs/2026-06-23-media-library-chrome-design.md`, `docs/superpowers/plans/2026-06-23-media-library-chrome.md`

- [ ] **Step 1: Run focused checks**

Run:

```bash
rtk pnpm test -- src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx
rtk pnpm lint
```

Expected: tests and lint pass.

- [ ] **Step 2: Run full frontend tests**

Run:

```bash
rtk pnpm test
```

Expected: all tests pass.

- [ ] **Step 3: Browser QA**

Run the app, inspect the media panel, and save:

```bash
output/playwright/media-library-chrome.png
```

Check that the library count, sort select, and folder cards fit without overlapping in the left rail.

- [ ] **Step 4: Commit**

Run:

```bash
rtk git add docs/superpowers/specs/2026-06-23-media-library-chrome-design.md docs/superpowers/plans/2026-06-23-media-library-chrome.md src/components/workspace/media-bin.tsx src/components/workspace/media-bin.test.tsx
rtk git commit -m "feat: polish media library chrome"
```
