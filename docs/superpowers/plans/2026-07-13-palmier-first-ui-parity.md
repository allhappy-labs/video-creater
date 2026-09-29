# Palmier-First UI Parity Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Video Creater's project home and complete editor workflow visibly follow the approved Palmier-first design while retaining every existing Video Creater capability and proving the result end to end.

**Architecture:** Execute five ordered, independently testable plans. Shared visual tokens and home land first; editor geometry then establishes the pane contract; media/generation and inspector/captions/export compose into that contract; final visual E2E compares matched Palmier and Video Creater states before `docs/parity.md` can close any visual row.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, Lucide, Vitest, Testing Library, Vite, existing Playwright CLI visual harness, Tauri 2, Rust/Cargo, Markdown evidence.

---

## Ordered Plan Set

1. `docs/superpowers/plans/2026-07-13-palmier-first-01-foundations-home.md`
2. `docs/superpowers/plans/2026-07-13-palmier-first-02-editor-shell-preview-timeline.md`
3. `docs/superpowers/plans/2026-07-13-palmier-first-03-media-generation.md`
4. `docs/superpowers/plans/2026-07-13-palmier-first-04-inspector-captions-export.md`
5. `docs/superpowers/plans/2026-07-13-palmier-first-05-visual-e2e-parity-tracker.md`

Plans 01–04 produce working, focused software slices and Conventional Commits. Plan 05 is the closure gate and may send a visible mismatch back to its owning plan; it cannot redefine the approved design or loosen evidence thresholds.

## Shared Execution Rules

- Prefix every shell command with `rtk`.
- Use the project-local `video-creater-visuals` skill throughout implementation and visual QA.
- Use test-driven development: failing focused test, observed failure, minimal implementation, passing focused test, then commit.
- Preserve all unrelated dirty-tree changes and stage only named files.
- Use Conventional Commits.
- Keep screenshots and generated comparison artifacts under ignored `output/parity-audit-2026-07-13/`.
- Do not remove a Video Creater-only feature because Palmier lacks it; relocate it within the approved hierarchy and record it in `docs/parity.md`.
- Do not fake account sign-in, Create Matte, provider support, codec availability, or export success.
- Do not close a visual tracker row from unit tests alone.

### Task 1: Record the exact pre-execution worktree boundary

**Files:**
- Evidence only: `output/parity-audit-2026-07-13/preflight/`

- [ ] **Step 1: Capture current branch, head, and dirty paths**

Run:

```bash
rtk mkdir -p output/parity-audit-2026-07-13/preflight
rtk git branch --show-current
rtk git rev-parse HEAD
rtk git status --short
```

Expected: current branch and head are recorded in the execution log; the existing dirty paths are reviewed before any staging.

- [ ] **Step 2: Confirm all ten references are ignored and readable**

Run:

```bash
rtk ls -lh output/parity-audit-2026-07-13/reference
rtk git check-ignore -v output/parity-audit-2026-07-13/reference/01-home.png
rtk sips -g pixelWidth -g pixelHeight output/parity-audit-2026-07-13/reference/*.png
```

Expected: ten non-empty PNG files, an ignore rule match, and valid dimensions for every reference.

- [ ] **Step 3: Run the focused current-tree baseline**

Run:

```bash
rtk pnpm exec vitest run \
  src/components/workspace/project-home.test.tsx \
  src/components/workspace/editor-workspace.test.tsx \
  src/components/workspace/media-bin.test.tsx \
  src/components/workspace/preview-panel.test.tsx \
  src/components/workspace/timeline-editor.test.tsx \
  src/components/workspace/source-clip-inspector.test.tsx
```

Expected: record exact pass/fail counts. Pre-existing failures must be assigned to their current owner before the first parity commit; they are not silently attributed to the parity work.

### Task 2: Execute Plan 01 — foundations and home

**Files:**
- Plan: `docs/superpowers/plans/2026-07-13-palmier-first-01-foundations-home.md`

- [ ] **Step 1: Complete every unchecked Plan 01 step in order**

Expected: shared neutral tokens, 220px desktop navigation, compact 150×120 cards, path/recovery dialogs, and narrow navigation are implemented with their focused tests and commits.

- [ ] **Step 2: Run the Plan 01 exit gate**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx src/App.test.tsx
rtk pnpm lint
rtk git diff --check
```

Expected: all home/App tests pass; lint and diff checks exit 0.

### Task 3: Execute Plan 02 — editor shell, preview, timeline, and agent

**Files:**
- Plan: `docs/superpowers/plans/2026-07-13-palmier-first-02-editor-shell-preview-timeline.md`

- [ ] **Step 1: Complete every unchecked Plan 02 step in order**

Expected: compact title/chrome, safe pane budgets, fill-available preview, five-control transport, Palmier timeline geometry, waveform/filmstrip density, and starter-based Codex state are implemented with focused commits.

- [ ] **Step 2: Run the Plan 02 exit gate**

Run:

```bash
rtk pnpm exec vitest run \
  src/components/workspace/editor-workspace.test.tsx \
  src/components/workspace/preview-panel.test.tsx \
  src/components/workspace/timeline-editor.test.tsx \
  src/components/workspace/agent-panel.test.tsx
rtk pnpm lint
rtk git diff --check
```

Expected: every named test file passes; lint and diff checks exit 0.

### Task 4: Execute Plan 03 — media and generation

**Files:**
- Plan: `docs/superpowers/plans/2026-07-13-palmier-first-03-media-generation.md`

- [ ] **Step 1: Complete every unchecked Plan 03 step in order**

Expected: Media/Captions/Audio rail, compact toolbar and overflow, direct grid, safe organizer, honest matte state, and attached generation drawer land without regressing provider/import/folder/silence behavior.

- [ ] **Step 2: Run the Plan 03 exit gate**

Run:

```bash
rtk pnpm exec vitest run src/components/workspace/media-bin.test.tsx src/components/workspace/editor-workspace.test.tsx
rtk pnpm lint
rtk git diff --check
```

Expected: media and workspace tests pass; lint and diff checks exit 0.

### Task 5: Execute Plan 04 — inspector, captions, speech, and export

**Files:**
- Plan: `docs/superpowers/plans/2026-07-13-palmier-first-04-inspector-captions-export.md`

- [ ] **Step 1: Complete every unchecked Plan 04 step in order**

Expected: accessible contextual tabs, Project/Activity, Details/AI Edit, caption workbench, speech controls, and destination-based export sheet land while retaining every current workflow callback and test.

- [ ] **Step 2: Run the Plan 04 exit gate**

Run:

```bash
rtk pnpm exec vitest run \
  src/components/workspace/contextual-inspector-tabs.test.tsx \
  src/components/workspace/source-clip-inspector.test.tsx \
  src/components/workspace/project-timeline-inspector.test.tsx \
  src/components/workspace/captions-workbench.test.tsx \
  src/components/workspace/caption-inspector.test.tsx \
  src/components/workspace/transcript-panel.test.tsx \
  src/components/workspace/export-sheet.test.tsx \
  src/components/workspace/media-bin.test.tsx \
  src/components/workspace/editor-workspace.test.tsx
rtk pnpm lint
rtk git diff --check
```

Expected: every named test passes; lint and diff checks exit 0.

### Task 6: Execute Plan 05 — matched visual E2E and tracker closure

**Files:**
- Plan: `docs/superpowers/plans/2026-07-13-palmier-first-05-visual-e2e-parity-tracker.md`

- [ ] **Step 1: Complete every unchecked Plan 05 step in order**

Expected: deterministic screenshots exist for all approved states and required viewports; a self-contained comparison board pairs each reference and implementation; every pair has a recorded pass after any mismatch loops.

- [ ] **Step 2: Run the complete program exit gate**

Run:

```bash
rtk pnpm test
rtk pnpm lint
rtk pnpm build
rtk cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=1
rtk pnpm check:native-runtime
rtk pnpm visual:qa:browser-release
rtk git diff --check
```

Expected: frontend, TypeScript, production build, Rust library, native runtime, release visual policy, and whitespace checks all exit 0.

### Task 7: Audit commits and tracker truth

**Files:**
- Review: `docs/parity.md`
- Review: all files named in Plans 01–05

- [ ] **Step 1: Confirm every meaningful slice has a Conventional Commit**

Run: `rtk git log --oneline --decorate -40`

Expected: the log shows separate foundation/home, shell/preview/timeline, media/generation, inspector/captions/export, visual harness, and final tracker commits. Any visual mismatch correction appears as its own component-scoped Conventional Commit.

- [ ] **Step 2: Confirm only intended paths were staged by the program**

Run: `rtk git status --short`

Expected: unrelated pre-existing dirty files remain owned by their original work; no ignored output artifact is staged.

- [ ] **Step 3: Confirm the tracker does not overclaim**

Run:

```bash
rtk rg -n "visual-regression|visual QA pending|Passed side-by-side review|Video Creater-only|account|Create Matte|notarization|clean Mac" docs/parity.md
rtk git diff --check -- docs/parity.md
```

Expected: implemented states cite exact evidence, unverified states remain open, Palmier gaps and Video Creater-only features are separate, and external release limits remain explicit.
