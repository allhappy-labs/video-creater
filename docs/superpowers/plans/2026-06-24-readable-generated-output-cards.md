# Readable Generated Output Cards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make completed AI generation outputs in the media bin read like usable media assets, with filenames and output metadata instead of only internal media ids.

**Architecture:** Keep the generated asset list in `MediaBin`. Add small output-label helpers that derive filename, resolution, duration, and frame-rate from the registered media asset when available, falling back to generated output metadata. Preserve existing callback payloads and accessible labels that use media ids.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, existing `MediaBin` component and media thumbnail helpers.

---

### Task 1: Test Readable Output Cards

**Files:**
- Modify: `src/components/workspace/media-bin.test.tsx`

- [x] Add a failing test named `renders completed generated outputs with readable file metadata`.
- [x] Render `MediaBin` with `generatedAssets={generatedAssets}`, `onSelectMedia`, and `media` containing a generated output media asset for `generated-shot-1-output`.
- [x] Assert the `AI generations` region shows `output.mp4`, `generated-shot-1-output`, and `1280 x 720 - 4s - 24 fps`.
- [x] Click the existing `Select generated output generated-shot-1-output` button and assert `onSelectMedia` receives `generated-shot-1-output`.
- [x] Run `rtk pnpm vitest run src/components/workspace/media-bin.test.tsx -t "readable file metadata" --reporter=dot` and confirm it fails because the generated output card still only renders the raw id.

### Task 2: Implement Output Metadata Rendering

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`

- [x] Add `generatedOutputFileLabel(output)` that returns `filenameFromPath(output.relativePath)`.
- [x] Add `generatedOutputMeta(output, mediaAsset)` that returns values like `1280 x 720 - 4s - 24 fps`.
- [x] In the generated asset output loop, look up `media.find((asset) => asset.id === output.mediaId)`.
- [x] Replace the raw button label with a compact block:
  - primary visible filename.
  - secondary visible media id.
  - secondary visible metadata when available.
- [x] Preserve `aria-label="Select generated output ${output.mediaId}"` and the callback payload.
- [x] Keep insert and replace actions unchanged.

### Task 3: Verification

**Files:**
- Test: `src/components/workspace/media-bin.test.tsx`
- Visual QA: local app via Playwright.

- [x] Run `rtk pnpm vitest run src/components/workspace/media-bin.test.tsx --reporter=dot`.
- [x] Run `rtk pnpm lint`.
- [x] Run `rtk pnpm test`.
- [x] Open the app and verify the AI generations output row shows a readable filename plus metadata on desktop and narrow widths.
- [ ] Run `rtk git diff --check`.
- [ ] Run a repository secret-fragment scan for user-provided API key material.
- [ ] Commit with `feat: label generated output cards`.
