# Parallel Variation Sets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Palmier-style named variation-set queueing for selected generated sources.

**Architecture:** Extend the current single-variation path with a shared `GenerationVariationDraft` payload. `SourceClipInspector` and `AgentPanel` emit draft arrays; `EditorWorkspace` converts each draft into the existing Temporal-backed `recordJob` plus `recordGeneratedAsset` actions in one split-project batch.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, Tauri invoke wrappers, existing split-project `ProjectAction` contracts.

---

### Task 1: Shared Draft Contract

**Files:**
- Modify: `src/components/workspace/source-clip-inspector.tsx`
- Modify: `src/components/workspace/agent-panel.tsx`
- Test: `src/components/workspace/source-clip-inspector.test.tsx`
- Test: `src/components/workspace/agent-panel.test.tsx`

- [ ] Add an exported `GenerationVariationDraft` type with `name` and `prompt`.
- [ ] Add `onQueueVariationSet?: (assetId: string, drafts: GenerationVariationDraft[]) => void` to generated-source UI props.
- [ ] Keep existing single `onQueueVariation(assetId, prompt)` unchanged.

### Task 2: Source Inspector Variation Set

**Files:**
- Modify: `src/components/workspace/source-clip-inspector.tsx`
- Test: `src/components/workspace/source-clip-inspector.test.tsx`

- [ ] Write a failing test that opens generated-source `AI Edit`, switches to variation-set mode, edits two draft prompts, and expects `onQueueVariationSet(assetId, drafts)`.
- [ ] Implement compact single/set mode controls inside the existing generated `AI Edit` panel.
- [ ] Default to four named directions: Storm Clouds, Radiant Backlight, Noir Drama, Soft Melancholy.
- [ ] Disable `Queue variation set` until at least two prompts are non-empty.

### Task 3: Codex Variation Set Action

**Files:**
- Modify: `src/components/workspace/agent-panel.tsx`
- Test: `src/components/workspace/agent-panel.test.tsx`

- [ ] Write a failing test that selected generated-source context plus a prompt renders `Queue variation set`.
- [ ] Clicking it emits four deterministic named drafts based on the current Codex prompt.
- [ ] Preserve the existing `Queue variation` single-prompt action.

### Task 4: Workspace Batch Queue

**Files:**
- Modify: `src/components/workspace/editor-workspace.tsx`
- Test: `src/components/workspace/editor-workspace.test.tsx`

- [ ] Write a failing test that queues a variation set and expects one split-project batch with one `recordJob` and one `recordGeneratedAsset` per draft.
- [ ] Implement `queueGeneratedClipVariationSet(assetId, drafts)`.
- [ ] Preserve source model, settings, references, target folder, parent lineage, retry lineage, and placement intent.
- [ ] Pass the callback to `AgentPanel`, media bin generated-source UI, and right-rail `SourceClipInspector`.

### Task 5: Verification

**Files:**
- Test: focused Vitest suites.

- [ ] Run focused tests for `source-clip-inspector`, `agent-panel`, and `editor-workspace`.
- [ ] Run `rtk pnpm lint`.
- [ ] Run relevant browser QA at desktop and narrow widths.
- [ ] Run `rtk git diff --check`.
- [ ] Run a repository secret-fragment scan for any user-provided API key material.
