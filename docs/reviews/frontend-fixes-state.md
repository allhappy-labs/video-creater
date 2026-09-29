# Frontend fixes implementation state

## Task 1 — project lifecycle, navigation, and recovery

- Project open/create/sample navigation uses a monotonically increasing request identity. Only the latest request may commit project, error, or busy state; Home and Settings-family navigation invalidate pending requests.
- Editor sessions are keyed by normalized project directory plus project ID. POSIX whitespace and browser URI schemes remain distinct/preserved. The retained editor stays mounted through Settings for the same project, while a different project gets fresh local state.
- Generated-asset polling has one owned poller per project directory. Duplicate starts for one job are ignored; disposal suppresses pending commits and timers; transient invalid reads retry without entering React state.
- `App` is the native-menu publisher. `EditorWorkspace` reports capability state through `onNativeMenuStateChange`, and inactive retained workspaces neither consume native commands nor own Preview/Timeline keyboard shortcuts.
- Preferences bootstrap exposes a retry action and rejects stale completions.
- The root render boundary offers an app reload after a render failure, with instructions to reopen the saved project after reload.
- Settings, project settings, system health, and the editor workspace load through React lazy boundaries. Normal startup probes the lightweight packaged-acceptance enablement module and loads the acceptance runner only for a valid acceptance context.

### Shared interfaces

- `EditorWorkspace`: optional `isActive` and `onNativeMenuStateChange` props.
- `PreviewPanel`: workspace passes `interactionEnabled={isActive}`.
- `TimelineEditor`: workspace passes `keyboardShortcutsEnabled={isActive}` and `transportShortcutsEnabled={isActive && viewerMode === "timeline"}`.

### TDD and verification evidence

- Project navigation regressions failed before request isolation: an obsolete completion cleared the newer busy state and an obsolete failure surfaced after the sample project won. Both pass after the request guard.
- Preferences retry and editor lazy-load tests failed before their controls/boundary existed, then passed.
- Native menu ownership failed before App owned editor capability publication, then passed across Editor → Settings → Editor, including a hidden-editor update.
- Poll disposal began with a missing helper; duplicate-start and transient-invalid-read regressions then exposed two implementation defects before passing.
- Project-session URI and POSIX-whitespace regressions failed against over-normalization before passing.
- Render recovery and acceptance-enablement tests failed with missing modules before their implementations were added.
- Focused final run: `src/App.test.tsx` and `src/lib/generated-asset-poll.test.ts` — 41 tests passed (38 App, 3 poller).
- Earlier combined lifecycle/helper run — 8 tests passed; final project-session helper now has 4 cases and poll helper has 3 cases.
- Full `src/components/workspace/editor-workspace.test.tsx` assertions passed (334), but that run reported 21 unhandled errors caused by committing transient `undefined` poll reads. After the fix, four affected generation/poll cases passed with no unhandled errors; root owns the clean full-suite rerun.
- Runtime boundary policy: 2 tests passed. A Task 1-only TypeScript diagnostic filter is empty. Repository-wide lint/build are currently blocked by concurrent readonly-fixture errors in `source-clip-inspector.test.tsx`; no successful build is claimed here.

### Verification limit

Native macOS behavior is not verified from this Ubuntu worktree.
