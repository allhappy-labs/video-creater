# Frontend state, async, and bridge review

These are preliminary Sol findings. See the [Astra-reviewed synthesis](2026-09-11-frontend-architecture-review.md) for final priorities and corrections to reachability/impact claims.

Scope: `src/App.tsx`, project-bound state in `EditorWorkspace`, Tauri listeners/menu synchronization, and the settings-operation hook. This is a static architecture review backed by the narrow test run listed below; no app code was changed.

## Findings

### P1 — concurrent project opens have no identity or latest-request guard

**Evidence.** Each of `openSampleProject`, `openProjectFolder`, and `createProjectFolder` independently starts work and unconditionally commits its result to `activeProjectDir`, `activeProject`, and `view` when its promise settles (`src/App.tsx:369-419`, `src/App.tsx:422-440`, `src/App.tsx:443-467`). Their `finally` blocks also independently clear a shared Boolean busy flag (`src/App.tsx:417-419`, `src/App.tsx:438-440`, `src/App.tsx:465-467`). The path form disables only its submit button (`src/components/workspace/project-home.tsx:144-158`); **Open Sample**, **New Project**, and recent-project cards remain actionable while another request is pending (`src/components/workspace/project-home.tsx:91-123`, `src/components/workspace/project-home.tsx:229-234`). The recent-card callback is also not guarded in `App` (`src/App.tsx:551-558`).

**Trigger.** Start opening a slow local/recent project, then click **Open Sample**, **New Project**, or another recent-project tile before the first load finishes. Resolve the two Tauri promises in reverse order.

**Impact.** Whichever request settles last silently becomes active, even when it was no longer the user's latest choice. An earlier `finally` can re-enable the form while another request is still running. A late failure can also write an error after another project opened successfully, which resurfaces when the user returns home. This is a project-identity correctness issue, not just a loading-state polish issue.

**Recommended fix.** Give every open/create attempt a monotonically increasing request ID (or an abortable project-navigation controller). Only the current request may commit project, error, busy, recent-project, and view state. Represent navigation as one discriminated state such as `{ status, requestId, target }` rather than two unrelated Booleans. Disable or guard every project-opening entry point while the current transition is active. Add a deferred-promise test that starts A then B, settles A last, and asserts B remains active and busy state remains accurate.

**Test gap.** `src/App.test.tsx:806-831` covers one successful folder open and `src/App.test.tsx:833-860` covers one failed open. The helper for deferred promises already exists at `src/App.test.tsx:71-79`, but no test overlaps two project transitions or checks stale completion/error suppression.

### P1 — a project prop change temporarily pairs the new directory with the old project and preserves old session state

**Evidence.** `EditorWorkspace` owns its project in local state initialized once (`src/components/workspace/editor-workspace.tsx:3438-3442`). When `initialProject` changes, an effect later copies only the project and two preview fields (`src/components/workspace/editor-workspace.tsx:3718-3727`). React first renders all derived values and runs earlier/later effects with the **old local `project` and new `projectDir` props**. For example, canonical preview preparation can invoke the bridge with that mixed pair (`src/components/workspace/editor-workspace.tsx:3907-3945`). Project-scoped selection, history, Codex proposal/status, render report, filmstrips, session manifest, and other workspace state are not reset; representative owners are at `src/components/workspace/editor-workspace.tsx:3429-3503`, `src/components/workspace/editor-workspace.tsx:3556-3594`, and `src/components/workspace/editor-workspace.tsx:3661-3669`.

There is a directly observable persistence bug in timeline view state: the storage key changes with `projectDir`, but `timelineViewStates` is initialized only on mount, and the effect immediately writes the retained state under the new key (`src/components/workspace/editor-workspace.tsx:4036-4049`). Agent sessions and project templates similarly retain the previous value until a load succeeds; the session failure path does not clear the old manifest (`src/components/workspace/editor-workspace.tsx:4609-4623`), and an empty template result leaves the old project's templates in place (`src/components/workspace/editor-workspace.tsx:4634-4652`).

**Trigger.** Change `projectDir`/`initialProject` without unmounting the workspace. The concurrent-open bug above creates exactly this path: after one request has set `view="editor"`, a second completion updates the props at the same component position (`src/App.tsx:574-605`) with no React `key`.

**Impact.** Project B can inherit project A's timeline zoom/scroll/playhead, undo history, selection, render/Codex status, session metadata, or template catalog. More seriously, project-bound bridge effects can receive B's directory together with A's project snapshot during the transition render.

**Recommended fix.** Make project identity an explicit component/session boundary. The smallest safe containment is to key `EditorWorkspace` by a stable canonical `projectDir + project.id`, forcing a complete remount when identity changes while retaining the component across Settings for the same project. Longer term, place project-bound state in a reducer/store whose state carries the project identity and rejects mismatched actions. Do not use a post-render effect to synchronize a prop into the primary project state. Add a rerender test using distinct projects/directories that asserts no B-key localStorage write contains A state and no bridge call combines A's ID with B's directory.

**Test gap.** Existing `EditorWorkspace` rerender tests exercise configuration refresh and menu requests (`src/components/workspace/editor-workspace.test.tsx:15183-15221`, `src/components/workspace/editor-workspace.test.tsx:15422-15467`), but do not replace both project identity props. No test covers `video-creater.timeline-view-state:<project>` isolation.

### P1 — generated-asset polling can restart after workspace cleanup and run for five minutes

**Evidence.** Project changes/unmounts clear only timeout IDs currently present in `generationRefreshTimeoutsRef` (`src/components/workspace/editor-workspace.tsx:3708-3716`). `refreshLiveGeneratedAssetUntilTerminal` removes an ID before starting the next async project load, awaits that load, then schedules another timeout without checking component lifetime or project identity (`src/components/workspace/editor-workspace.tsx:5007-5044`). If cleanup occurs while `loadSplitProjectFromFolder` is in flight, there is no timeout in the map to clear. The old continuation can then call `setProject` and install a fresh timeout after cleanup has already run.

**Trigger.** Start a real generated-media workflow so polling begins (`src/components/workspace/editor-workspace.tsx:4935-4941` or `src/components/workspace/editor-workspace.tsx:4974-4979`), allow a poll load to be in flight, then navigate Home/unmount the workspace or switch project identity.

**Impact.** The abandoned editor continues one-second Tauri reads for up to 300 attempts. On an in-place project change it can also replace the visible project with the old project's refreshed snapshot. On unmount React discards the state update, but the self-scheduling IPC loop remains a resource leak.

**Recommended fix.** Introduce a workspace/project epoch or abort token captured by each poll. Check it immediately after every `await` and immediately before scheduling the next timeout. Invalidate it and clear timers on cleanup; where possible make the bridge operation abortable. A single managed polling hook/state machine would also prevent duplicate poll chains for the same job.

**Test gap.** Add a fake-timer test with a deferred `load_split_project_from_folder`: unmount or rerender to another project while it is pending, resolve it, advance time, and assert no state commit and no further load occur.

### P2 — native menu state has two asynchronous writers and no ordering token

**Evidence.** `App` publishes inactive home/settings menu state in a fire-and-forget effect (`src/App.tsx:165-180`), while `EditorWorkspace` independently publishes editor state in another fire-and-forget effect (`src/components/workspace/editor-workspace.tsx:3864-3899`). `syncNativeMenuState` sends only the state, with no view generation or sequence (`src/lib/native-menu.ts:83-85`), and the Rust command applies every received snapshot (`src-tauri/src/native_menu.rs:343-373`). Neither frontend writer invalidates an older in-flight call when ownership changes.

**Trigger.** Navigate from Editor to Home/Settings while an editor menu sync is delayed, then let that older invocation apply after the inactive snapshot.

**Impact.** Editor-only native commands can be re-enabled or show stale checked state while the app is on Home or Settings. The inverse ordering can leave editor commands disabled after entering the editor.

**Recommended fix.** Centralize menu-state publication in `App`, passing the editor's computed capabilities upward, or add a monotonically increasing view/menu revision that Rust rejects when stale. Test with deferred sync promises and reversed completion/application order.

**Test gap.** The current workspace menu test verifies emitted snapshots (`src/components/workspace/editor-workspace.test.tsx:15422-15467`), but no test covers cross-owner transition ordering.

### P2 — a transient preferences bootstrap failure leaves no recovery path

**Evidence.** Preferences load once on mount and stores a terminal error (`src/App.tsx:124-144`). While `appPreferences` is null, the whole app is replaced by the bootstrap status/error surface (`src/App.tsx:470-493`). The error branch exposes no Retry action and the effect has no retry policy.

**Trigger.** A transient Tauri IPC/startup failure from `loadAppPreferences`, followed by the bridge becoming available.

**Impact.** The user cannot reach Home, Settings, or diagnostics and must restart the app even though the dependency recovered.

**Recommended fix.** Model bootstrap as `loading | ready | failed` with a visible Retry button that starts a new request ID. Ignore stale attempts. A short bounded automatic retry is reasonable for startup bridge readiness, but the explicit recovery action should remain.

**Test gap.** Add a test that rejects the first preferences request, resolves the retry, and asserts Home becomes available. Current App tests generally mock either permanent bridge failure or immediate success; none exercises bootstrap recovery.

## Architecture direction

The fixes above point to three ownership boundaries:

1. `App` owns one project-navigation transaction with a request ID and the active project identity.
2. A keyed project session owns all editor state and async work; every completion carries and validates that identity/epoch.
3. One publisher owns native menu state for the current app view.

This can be introduced incrementally. The request guard and keyed workspace close the highest-risk cross-project paths first. Extracting async workflows into project-scoped hooks/reducers can then reduce the large number of independent state setters without a broad rewrite.

## Checks and positive evidence

- `pnpm exec vitest run src/App.test.tsx src/lib/settings/use-settings-operations.test.tsx` passed: 2 files, 41 tests.
- `useSettingsOperations` has sound late-listener cleanup and retry handling (`src/lib/settings/use-settings-operations.ts:115-245`), with focused coverage for the poll/listen gap, stale versions, cleanup, and recovery (`src/lib/settings/use-settings-operations.test.tsx:67-300` and subsequent tests). No change is recommended there from this review.
- Native event registrations correctly handle async registration completing after disposal in both App (`src/App.tsx:182-243`) and the webview drag/drop listener (`src/components/workspace/editor-workspace.tsx:3755-3784`). The ordering problem is in state publication, not listener cleanup.
