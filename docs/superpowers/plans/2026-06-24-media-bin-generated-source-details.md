# Media Bin Generated Source Details Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the media-bin selected generated source panel match the Palmier-style source workflow with file metadata, output provenance, and grouped generated details.

**Architecture:** Keep `MediaBin` presentational and reuse existing generated asset, selected media, and `onSelectMedia` data flow. The details tab will render four compact sections: File, References, Generated, and Prompt. Reference cards continue to call `onSelectMedia`, and the new output card uses the same reveal path.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, existing shadcn/ui `Button` and local media thumbnail helpers.

---

### Task 1: Test Media-Bin Generated Source Details

**Files:**
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] Add a failing test named `shows selected generated source file provenance and output reveal`.
- [ ] Render `MediaBin` with `selectedMediaId="generated-shot-1-output"`, existing `media`, existing `generatedAssets`, and `onSelectMedia`.
- [ ] Assert the selected generated source details include `File`, `Type`, `generated`, `Duration`, `4s`, `Dimensions`, `1280 x 720`, `Frame Rate`, `24 fps`, `Path`, and `sample/generated/product-reveal.mp4`.
- [ ] Assert the details include a `References` section with `Reveal first frame media-2`, `Reveal reference media-2`, and `Reveal output generated-shot-1-output`.
- [ ] Click `Reveal output generated-shot-1-output` and expect `onSelectMedia` to receive `generated-shot-1-output`.
- [ ] Run `rtk pnpm vitest run src/components/workspace/media-bin.test.tsx -t "file provenance" --reporter=dot` and confirm the failure is caused by the missing output card or file rows.

### Task 2: Implement Grouped Details

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`

- [ ] In `renderSelectedGeneratedSource`, derive `selectedGeneratedOutputMedia` from `media.find((asset) => asset.id === selectedGeneratedOutput.mediaId)`.
- [ ] Build the `references` array with first frame, last frame, each reference media id, and the selected output media id.
- [ ] Replace the existing flat name/prompt/model grid in the details tab with compact sections:
  - `File`: type, duration, dimensions, frame rate, path.
  - `References`: the existing thumbnail reveal cards, now including the output.
  - `Generated`: model, aspect ratio, resolution, duration.
  - `Prompt`: optional name row and the prompt text.
- [ ] Keep existing `Replace selected clip` and `Rerun same prompt` buttons at the top of the details tab.
- [ ] Preserve the `AI Edit` tab and variation-set behavior unchanged.
- [ ] Run the focused test and confirm it passes.

### Task 3: Verification

**Files:**
- Test: `src/components/workspace/media-bin.test.tsx`
- Visual QA: local app via Playwright.

- [ ] Run `rtk pnpm vitest run src/components/workspace/media-bin.test.tsx --reporter=dot`.
- [ ] Run `rtk pnpm lint`.
- [ ] Open the app, select `product-reveal.mp4`, and capture desktop plus narrow screenshots.
- [ ] Verify the selected generated source panel has readable grouped sections and no overlapping text.
- [ ] Run `rtk git diff --check`.
- [ ] Run a repository secret-fragment scan for user-provided API key material.
- [ ] Commit with `feat: polish generated source details`.
