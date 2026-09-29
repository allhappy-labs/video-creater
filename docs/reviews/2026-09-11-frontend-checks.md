# Frontend review: baseline checks

Reviewed commit: `19b59f347304b38f3a0d4ced29a9496613afe02a`, 2026-09-11, Ubuntu VM.

## Verification

| Check | Result |
| --- | --- |
| Initial git status | Clean |
| `pnpm install --frozen-lockfile` | Passed; dependencies were initially absent; lockfile unchanged |
| `pnpm lint` | Passed after installation; initial attempt blocked by missing `tsc` |
| `pnpm test -- --maxWorkers=2` | Passed: 106 files, 2,085 tests, 93.47 seconds. The extra separator was forwarded, so this is not evidence of a two-worker limit. |
| `pnpm build` | Passed; Vite warned about a chunk exceeding 500 kB |
| `pnpm visual:qa:browser --only modern-editor-default-desktop` | Blocked: missing `/home/olhapi/.codex/skills/playwright/scripts/playwright_cli.sh` |
| Native macOS / packaged app | Not run; this is an Ubuntu review |

Commands used a shell-local `rtk() { "$@"; }` pass-through because the requested `rtk` executable is unavailable. No application code was changed for these checks.

## Architecture observations for synthesis

### Eager feature loading: measured build concern, unmeasured runtime impact

[`App.tsx`](../../src/App.tsx#L1) eagerly imports the editor, Settings, Project Settings and System Health. [`main.tsx`](../../src/main.tsx#L5) eagerly imports the packaged settings acceptance runner. Production output contains a main application JS chunk of **977.81 kB**, **253.06 kB gzip**, separate from React (194.26 kB), icons (53.44 kB), Tauri (18.49 kB) and other vendor code (23.51 kB). CSS is 84.97 kB.

Consider feature-level lazy loading for the editor and secondary settings surfaces; preserve the existing originating-view lifetimes during Settings/Health navigation (the app does not retain every visited surface). Investigate separating the acceptance runner's enablement check so the runner can load lazily, rather than removing packaged acceptance functionality. Measure cold startup and navigation in the packaged app before assigning a performance severity; bundle size alone does not prove a user-visible slowdown.

### No render error containment

[`main.tsx`](../../src/main.tsx#L23) renders App directly inside StrictMode. A repository source search found no `ErrorBoundary`, `componentDidCatch` or `getDerivedStateFromError`. Component-level async error displays are not render error boundaries. Add a recoverable root boundary, then consider a separate preview boundary if it meaningfully contains failures. This is a resilience improvement, not evidence that a current input causes a crash. Verify with a deliberately throwing child and a recovery action that reloads canonical project state.

### Large ownership surfaces

Production component sizes: EditorWorkspace 11,018 lines, MediaBin 7,095, TimelineEditor 5,868, SourceClipInspector 3,340. EditorWorkspace mixes orchestration and UI; its state initialization starts at [line 3407](../../src/components/workspace/editor-workspace.tsx#L3407), and project synchronization occurs around [line 3714](../../src/components/workspace/editor-workspace.tsx#L3714). The file has 110 matches for `useState`/`useEffect` (a textual count, not a complexity metric).

Use confirmed defects to choose extraction boundaries: project-session ownership, command serialization, event lifetimes, provider request construction, and view-only components. Retain the existing pure timeline helpers and typed Rust command wrappers rather than introduce a new state library solely to reduce file size. Existing test breadth is valuable; supplement it with adversarial ordering and integration tests around these boundaries.
