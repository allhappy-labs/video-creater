# Agent Queue Transcript Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Codex selected-source queue actions leave readable chat transcript entries, so agent-controlled generation feels visible and Palmier-like instead of silent button clicks.

**Architecture:** Keep `AgentPanel` local and presentational. Add lightweight local transcript state for selected-source queue actions only. Existing callbacks still queue Temporal-backed work in `EditorWorkspace`; the transcript simply records the user prompt and a Codex acknowledgement inside the existing `Codex chat` region.

**Tech Stack:** React state, TypeScript, Vitest, Testing Library, existing `AgentPanel` component.

---

### Task 1: Test Queue Transcript Entries

**Files:**
- Modify: `src/components/workspace/agent-panel.test.tsx`

- [ ] Add a failing test named `records selected source queue actions in the Codex chat transcript`.
- [ ] Render `AgentPanel` with a selected generated source named `product-reveal.mp4`, `generatedAssetId="generated-shot-1"`, and `onQueueSelectedVariationSet`.
- [ ] Change the prompt to `Create four variations of this image.`.
- [ ] Click `Queue variation set` inside `Selected Codex source`.
- [ ] Assert the callback still receives four variation drafts.
- [ ] Assert the `Codex chat` region shows a `You` message containing the prompt and a `Codex` message containing `Queued 4 variations for product-reveal.mp4`.
- [ ] Run `rtk pnpm vitest run src/components/workspace/agent-panel.test.tsx -t "queue actions" --reporter=dot` and confirm it fails because transcript entries are not rendered.

### Task 2: Implement Local Transcript State

**Files:**
- Modify: `src/components/workspace/agent-panel.tsx`

- [ ] Define a local transcript item type with `id`, `role: "user" | "codex"`, `message`, and optional `meta`.
- [ ] Add `useState<AgentTranscriptEntry[]>([])`.
- [ ] Add a helper `recordQueueTranscript(userPrompt, codexMessage, meta)` that appends a user entry and Codex entry.
- [ ] In `Queue variation`, after invoking `onQueueSelectedVariation`, record the trimmed prompt and `Queued a variation for {selectedMediaContext.label}.`
- [ ] In `Queue variation set`, build drafts once, invoke the callback with those drafts, and record the trimmed prompt plus `Queued {drafts.length} variations for {selectedMediaContext.label}.`
- [ ] In `Queue referenced shot`, record the trimmed prompt plus `Queued a referenced shot from {selectedMediaContext.label}.`
- [ ] In `Queue upscale`, record a user entry such as `Upscale {selectedMediaContext.label}.` and a Codex entry `Queued an upscale for {selectedMediaContext.label}.`
- [ ] Render transcript entries in the existing `Codex chat` region after the initial assistant context message and before `latestRequest`.

### Task 3: Verification

**Files:**
- Test: `src/components/workspace/agent-panel.test.tsx`
- Visual QA: local app via Playwright.

- [ ] Run `rtk pnpm vitest run src/components/workspace/agent-panel.test.tsx --reporter=dot`.
- [ ] Run `rtk pnpm lint`.
- [ ] Run `rtk pnpm test`.
- [ ] Open the app, select `product-reveal.mp4`, queue a variation set from the Codex source card, and verify the transcript entries are readable in desktop and narrow widths.
- [ ] Run `rtk git diff --check`.
- [ ] Run a repository secret-fragment scan for user-provided API key material.
- [ ] Commit with `feat: show agent queue transcript`.
