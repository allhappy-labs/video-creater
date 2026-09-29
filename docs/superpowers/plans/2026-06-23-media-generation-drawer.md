# Media Generation Drawer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the media panel generation drawer closer to Palmier with compact reference tabs and a top-level folder action.

**Architecture:** Keep all behavior inside `MediaBin` local UI state. Preserve the existing `MediaGenerationRequest` callback contract so `EditorWorkspace` and project actions do not change.

**Tech Stack:** React, TypeScript, Tailwind, shadcn Button, lucide-react icons, Vitest/Testing Library.

---

### Task 1: Reference Tabs And Folder Action

**Files:**
- Modify: `src/components/workspace/media-bin.tsx`
- Modify: `src/components/workspace/media-bin.test.tsx`

- [ ] **Step 1: Write failing tests**

Add tests that open the generation drawer, verify `First/Last` is active, switch to `Reference`, confirm first/last slots hide while the reference slot remains visible, switch back, and verify queued video payloads still include all selected references. Add a test that clicks top-level `New Folder` and verifies the folder manager appears.

- [ ] **Step 2: Run the focused test**

Run: `rtk pnpm test -- src/components/workspace/media-bin.test.tsx`

Expected: FAIL because generation reference tabs and the top `New Folder` button do not exist yet.

- [ ] **Step 3: Add minimal implementation**

Add local state:

```ts
type GenerationReferenceTab = "first-last" | "reference";
const [generationReferenceTab, setGenerationReferenceTab] =
  useState<GenerationReferenceTab>("first-last");
const [folderManagerOpen, setFolderManagerOpen] = useState(false);
```

Render a compact tab bar for Video mode. Render first/last slots only on `first-last`, render reference only on `reference`, render Image reference directly, and hide tabs for Audio. Add a `New Folder` button to the header that opens the folder manager.

- [ ] **Step 4: Verify focused behavior**

Run: `rtk pnpm test -- src/components/workspace/media-bin.test.tsx`

Expected: PASS.

- [ ] **Step 5: Run full verification**

Run:

```bash
rtk pnpm lint
rtk pnpm test
rtk git diff --check
```

Expected: all commands pass.
