# Frontend Review Fixes Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development. Work only in `.worktrees/frontend-architecture-fixes`; the user has approved implementation and merging to main.

**Goal:** Address the complete frontend review, install reproducible Playwright tooling, verify the combined changes and merge to main.

**Architecture:** Preserve Rust ownership of canonical project data. Isolate project navigation and session lifetimes, centralize active-view menu and transport ownership, and extract focused helpers where those boundaries reduce coupling. Improve recovery and preview contracts without introducing a global state library.

**Tech Stack:** React 19, TypeScript, Tauri 2, Vite, Vitest, pnpm, Playwright and Knip.

**Spec:** `docs/reviews/2026-09-11-frontend-architecture-review.md`, including supporting reports and Astra corrections. The user's request to address all findings includes lower-priority inspector, accessibility, resilience and cleanup items.

## Global constraints

- Implementation uses Sol at medium effort. Astra is reserved for final architectural review.
- Preserve other worktrees and all unrelated work. Only this branch is merged.
- No direct canonical data mutation from browser code; no native macOS verification claims from Ubuntu.
- Add regressions that fail before the corresponding behavior fix; avoid tests that merely mirror implementation.
- Use Conventional Commits. Root coordinates commits and shared-file edits.
- Run commands with `rtk`; when absent use a shell-local pass-through function.

## Task 1 — Project lifecycle, navigation and recovery

**Owner:** state implementation agent.
**Files:** `src/App.tsx`, `src/App.test.tsx`, `src/components/workspace/editor-workspace.tsx` and its tests, new project-lifecycle/menu helpers, `src/main.tsx`, error boundary tests.

- [ ] Reproduce overlapping project opens using deferred Tauri promises. Assert the newest requested project wins and obsolete finally/error paths cannot commit.
- [ ] Implement a shared navigation request identity and consistent entry guards. Key EditorWorkspace by normalized directory/project ID; verify Settings retains same-project session state.
- [ ] Add an epoch-guarded generated-asset poll helper/hook; resolve a pending read after disposal and assert no additional reads or state commits.
- [ ] Move native menu publication to one active-view owner and test Editor → Settings → Editor without capability changes plus hidden-editor updates.
- [ ] Add preferences bootstrap Retry and verify first failure followed by successful retry.
- [ ] Add a recoverable render boundary with a real throwing-child regression; recovery reloads canonical state.
- [ ] Lazy-load secondary app surfaces and separate packaged acceptance enablement from its implementation. Preserve source QA contracts and originating-view mount lifetimes.
- [ ] Run App/workspace/helper tests and self-review the actual diff. Send root tests and touched interfaces.

**Interfaces:** coordinate new `EditorWorkspace` active/menu callbacks and any preview transport prop with task 2 before editing the same call site. Root serializes shared-file edits.

## Task 2 — Interaction ownership, preview and Settings

**Owner:** interaction implementation agent.
**Files:** `preview-panel.tsx`, `timeline-preview-compositor.tsx`, `timeline-editor.tsx`, `settings.tsx`, related tests and new focused interaction helpers.

- [ ] Reproduce source/timeline keyboard conflict with both active listeners. Route by viewer ownership and prevent handled tab keys entering transport.
- [ ] Implement viewer roving tabs, panel relationships, Home/End and close-active focus restoration; verify keyboard behavior.
- [ ] Reproduce internal Settings navigation followed by a new external request; supersede internal target by external request identity.
- [ ] Harden missing prepared-frame coverage with an explicit recoverable state rather than silently showing approximate canonical imagery. Account for flattened composite IDs.
- [ ] Separate media play/pause transitions and pending play attempts from drift correction; prove repeated clock samples do not repeatedly invoke play, preserving metadata/error recovery.
- [ ] Hide decorative timeline grid/edit markers from accessibility tree while retaining useful timeline descriptions.
- [ ] Run focused regressions and affected component suites; self-review and report.

## Task 3 — Repository Playwright and Knip cleanup

**Owner:** tooling implementation agent.
**Files:** `package.json`, `pnpm-lock.yaml`, Playwright config/tests, `scripts/browser-visual-qa.mjs` and affected script tests, Knip config, unused frontend modules/exports.

- [ ] Install pinned reproducible Playwright tooling and Chromium; remove the browser QA dependency on a user-home skill wrapper, retaining an explicit compatible override if needed.
- [ ] Exercise real app Home, Settings and editor at desktop/narrow widths with a deterministic Tauri fixture. Assert functionality, keyboard interactions, visible content and no uncaught errors; retain screenshots/traces on failure.
- [ ] Remove two dead files, two unused dependencies and three dead functions from the review. Verify active surfaces supersede test-only SkillsSettings/RenderQualityControl and preserve useful coverage there.
- [ ] Remove unnecessary exports with production and test consumer checks, including exported types. Keep locally referenced declarations, PostCSS, Autoprefixer and system binary prerequisites.
- [ ] Add a stable `check:unused` command/config with justified exclusions; run `npx knip` and record remaining intentional findings.
- [ ] Run tooling/unit checks and one real browser pass. Report exact commands and artifacts.

## Task 4 — Inspector drafts and resize performance

**Owner:** next available Sol agent after its primary task.
**Files:** `inspector-dock.tsx`, source/caption/text inspector forms and their tests, reusable draft lifecycle helper.

- [ ] Preserve dirty drafts when unrelated project snapshots replace object identity; reset on selected-item identity change and expose explicit reload when canonical editable values conflict.
- [ ] Coalesce resize updates with requestAnimationFrame and persist final dimensions on pointer-up; preserve keyboard resize/accessibility.
- [ ] Verify distinct-item switching, unrelated refresh, true external conflict and pointer cancellation. Run focused suites.

## Task 5 — Integrated verification, review and merge

**Owner:** root; fixes delegated to Sol.

- [ ] Review each task against its spec and diff; resolve shared interfaces and run meaningful integrated regressions.
- [ ] Run lint, all frontend tests, production build, Knip, relevant source/tooling policy checks and Playwright desktop/narrow scenarios.
- [ ] Have Astra review the combined architectural changes once; route concrete fixes to Sol and verify the amended scope.
- [ ] Record a findings-to-fixes matrix and precise limits (native macOS unverified, performance claims measured only where evidence exists).
- [ ] Commit verified work, merge into current main preserving intervening work, verify merged result, then remove only this completed worktree/branch.

## Preflight and progress ledger

| Tasks | Shared contract / resolution |
| --- | --- |
| 1 + 2 | Workspace preview wiring: task 1 owns workspace source and applies agreed task-2 prop changes. |
| 1 + 3 | Package and build policy: task 3 owns dependencies, task 1 owns lazy imports; tests coordinated. |
| 2 + 3 | Export removal overlaps component files: task 3 waits for task 2 before editing those export modifiers. |
| 2 + 4 | Separate component scopes; task 4 starts after first implementation wave. |
| 1–4 | Root owns commits; agents stage/commit nothing so changes cannot be accidentally mixed. |

The review already supplies approved design and causal source evidence; implementation begins with fresh baseline/regression verification. No additional design approval is needed.
