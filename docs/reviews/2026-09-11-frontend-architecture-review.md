# Frontend architecture review — 2026-09-11

Reviewed commit: `19b59f347304b38f3a0d4ced29a9496613afe02a`.

The frontend has substantial test coverage and useful pure timeline helpers, but project identity, async lifetimes, and keyboard ownership cross component boundaries without consistent coordination. Improve those boundaries incrementally before undertaking a broad component rewrite.

## Review method and evidence

Three Sol agents at medium effort reviewed state/async behavior, component interactions, and unused code. Astra at high effort performed two bounded critical passes: architecture/cleanup, then bug findings. The collaboration tool refused another thread even after a Sol completed, so Astra ran through the local Codex CLI in read-only, ephemeral sessions with the requested model explicitly selected. Astra did source review, not repeated baseline tests. No application code, dependencies, or lockfile were changed.

Supporting reports:

- [State and async findings](2026-09-11-frontend-state.md)
- [Interaction findings](2026-09-11-frontend-interactions.md)
- [Knip cleanup analysis](2026-09-11-frontend-unused.md)
- [Baseline checks and build observations](2026-09-11-frontend-checks.md)
- [Astra architecture/cleanup judgment](evidence/2026-09-11-astra-architecture.md)
- [Astra bug verification and corrections](evidence/2026-09-11-astra-findings.md)

The final priorities below supersede preliminary severity labels in the supporting reports. Source-confirmed behavior is distinguished from runtime reproduction; passing existing tests does not verify the proposed bug fixes.

## Prioritized bugs and fixes

**Eight findings survived critical source review: two P1 findings in the same project-transition chain and six P2 findings.** P1 means address first because active-project identity/state can become incorrect; P2 means a concrete interaction, lifecycle, accessibility, or recovery defect. All are source-supported, not newly reproduced in a running app.

### 1. P1 — overlapping project opens activate the wrong project

Start opening A, then use an enabled recent-project/sample control to open B. If A resolves last, it replaces B and clears shared busy state without checking whether it is still current. See [open handlers](../../src/App.tsx#L369) and [Home controls](../../src/components/workspace/project-home.tsx#L91).

**Fix:** one navigation transaction/request ID for every open/create/sample entry point; only the current request may commit project, errors, busy state, or navigation. **Regression:** deferred A/B responses resolved in both orders, including late failure and busy-state assertions.

### 2. P1 — in-place identity changes retain the previous project's editor state

The overlapping-open path mounts the workspace on the first completion, then updates identity props on the second without remounting. History/selection/view state survive, and old timeline view state can be persisted under the new directory. Ordinary sequential navigation through Home unmounts the workspace and is not shown to have this problem. See [mount boundary](../../src/App.tsx#L574), [partial reset](../../src/components/workspace/editor-workspace.tsx#L3718), and [view-state persistence](../../src/components/workspace/editor-workspace.tsx#L4036).

**Fix:** key the project session by normalized directory plus project ID, combined with async lifetime guards. Keep the same session across Settings for the same project. **Regression:** replace A with B and assert B's storage, history and bridge requests never contain A's state. A mixed project/directory preview call exists, but canonical disk corruption is **not established**: [Rust reloads the saved project](../../src-tauri/src/main.rs#L1575).

### 3. P2 — native menus can remain disabled after returning from Settings

Open Settings from the editor, then return without changing editor capabilities. App publishes inactive menu state on entry but nothing on return; the retained editor's publisher has no active-view dependency. Conversely, background updates can let the hidden editor republish active capabilities. See [App publisher](../../src/App.tsx#L165) and [editor publisher](../../src/components/workspace/editor-workspace.tsx#L3864).

**Fix:** one App-owned publisher derived from active view and editor capabilities; gate editor command dispatch by active view. **Regression:** Editor → Settings → Editor with unchanged capabilities, plus an update while hidden. Do not use reversed Promise settlement as proof of native application order; the original race explanation was not established.

### 4. P2 — internal Settings navigation masks newer external navigation

Use AI & Models' Configure action for a missing provider credential, then native Help → Advanced Settings. The still-mounted Settings instance prefers its old internal target over the new external request. The state persists until remount. See [target precedence](../../src/components/settings/settings.tsx#L261) and [external navigation](../../src/App.tsx#L304).

**Fix:** associate internal navigation with the external request version that created it; supersede it on every new external request. Independent counters cannot safely be compared as one sequence. **Regression:** configure provider, issue a new external request, then check category, focus and announcement.

### 5. P2 — source-frame arrows can also move the timeline playhead

Open playable source media, then play/pause it. The source listener is reinstalled after the timeline listener and does not respect `defaultPrevented`; an arrow can affect both playheads. This depends on listener order and is not inevitable every time source mode opens. See [preview listener](../../src/components/workspace/preview-panel.tsx#L674) and [timeline handling](../../src/components/workspace/timeline-editor.tsx#L2895).

**Fix:** route transport commands through one active-viewer owner and honor prevented events. **Regression:** mount both surfaces, toggle source playback, then assert arrows affect only the active viewer. This is unexpected playhead movement, not an established canonical edit.

### 6. P2 — generated-media polling survives workspace cleanup

Navigate Home while a generated-asset poll load is pending. Cleanup removes current timers, but the async continuation can create a new one afterward. The loop allows 300 attempts; total duration also includes IPC latency. See [cleanup](../../src/components/workspace/editor-workspace.tsx#L3708) and [poll continuation](../../src/components/workspace/editor-workspace.tsx#L5007).

**Fix:** invalidate a session epoch on cleanup and check it after awaits, before commits and before rescheduling. **Regression:** resolve a deferred load after unmount and advance fake timers; assert no subsequent loads. Cross-project replacement additionally depends on the identity-transition defect above.

### 7. P2 — failed preferences bootstrap has no retry

The initial preferences request can reject into an error surface that never attempts recovery. See [initial load](../../src/App.tsx#L124) and [error surface](../../src/App.tsx#L470).

**Fix:** expose Retry with a guarded request lifecycle. **Regression:** reject the first request, recover on retry, and assert Home becomes reachable. Failure frequency is unknown; the missing recovery action is explicit.

### 8. P2 — viewer tabs have incomplete keyboard/accessibility behavior

Viewer tabs declare tab roles without roving focus, linked panel semantics, or arrow/Home/End behavior. Previous/next buttons provide an alternative, so the surface is not wholly keyboard-inaccessible. See [viewer tabs](../../src/components/workspace/preview-panel.tsx#L951).

**Fix:** reuse [inspector tab behavior](../../src/components/workspace/contextual-inspector-tabs.tsx#L34), including focus after closing a source; prevent handled tab keys reaching transport listeners. **Regression:** keyboard traversal, close-active-tab focus, and tab/panel relationships with both editor surfaces mounted.

## Findings downgraded by Astra

- **Missing canonical frames: P3 hardening, not an established P1 bug.** The production parent derives ready status, timeline and sequences from one project-matched result; Rust rejects empty PNG sequences, and image-load errors have an error surface. No ordinary user trigger for the proposed missing-data combination was established. Add coverage validation and recovery without silently showing approximation as canonical; account for flattened composite IDs. [Parent contract](../../src/components/workspace/editor-workspace.tsx#L10562), [backend validation](../../src-tauri/src/main.rs#L1608).
- **Repeated media `play()` calls: P3 optimization.** The calls recur on source-time updates, but preview jank, dropped frames and meaningful runtime cost were not measured. Separate transport transitions/pending play attempts from drift correction and preserve metadata/recovery behavior. Profile target hardware before raising priority. [Synchronization](../../src/components/workspace/timeline-preview-compositor.tsx#L160).

## Suggested delivery sequence

1. Project-navigation request guards, keyed session identity and poll lifetime guards, each with adversarial ordering tests.
2. Active-view ownership for native menus/transport plus Settings request supersession and bootstrap Retry.
3. Viewer keyboard semantics with integrated shortcut tests.
4. Independent small Knip cleanup batches; move useful tests to active components.
5. Targeted component/controller extraction, error recovery and measured bundle/preview optimization.

Keep canonical project mutations and validation in Rust. These fixes do not require a frontend rewrite or a new global state library.

## Verification

| Check | Result |
| --- | --- |
| Dependencies | Installed successfully using `pnpm install --frozen-lockfile` |
| `pnpm lint` | Passed |
| Frontend Vitest suite | 106 files, 2,085 tests passed |
| Additional focused App/settings tests | 2 files, 41 tests passed |
| `pnpm build` | Passed; main app JS 977.81 kB / 253.06 kB gzip; chunk-size warning |
| `npx knip --no-progress` and scoped run | Completed with findings; exit 1 is expected for reported issues |
| Browser visual QA | Blocked by missing Playwright CLI wrapper |
| New bug reproductions / packaged macOS verification | Not performed |

## Separate cleanup: unused code and components

Knip 6.35.1 was run through `npx`; [raw output and exact commands](evidence/2026-09-11-knip.txt) are retained. The default scan counted four unused files, including the temporary review config; after excluding that artifact, three repository findings remain. Two are genuinely unused, and one is a PostCSS convention-loading false positive.

| Category | Recommendation |
| --- | --- |
| Unused files | Remove `src/components/ui/card.tsx` and `src/lib/provider-account.ts` in a cleanup change. Removing the latter frontend wrapper does not remove the native command. |
| Unused dependencies | Remove `@dnd-kit/core` and `dnd-timeline`; update the lockfile. |
| Dead functions | Remove `cancelModelDownload`, `captureCanonicalPreviewFrameInSplitProjectFolder`, and `renderWebmToSplitProjectFolder`; remove the stale cancellation mock entry. |
| Test-only components | `SkillsSettings` and `RenderQualityControl` have no production render sites. Check parity with the live Project Settings/export surfaces, then remove obsolete implementations while moving useful assertions to active-surface tests. Their existence does not prove missing product functionality. |
| Export surface | Default scan: 23 unused runtime exports and 112 exported types; scoped scan: 21 runtime exports. Many declarations are used locally. Remove unnecessary export modifiers only after checking test consumers; do not delete declarations wholesale. |
| Keep / classify | Keep PostCSS configuration and Autoprefixer. The 13 unlisted binaries are system prerequisites, not missing npm dependencies. Both default-preference aliases have consumers. |

This is low-priority maintenance, separate from correctness fixes. No speculative product approval gate is needed for technically verified unused private frontend code. Batch small removals, run type checks/focused tests/build, then add a stable Knip configuration with documented intentional exceptions.

## Architectural improvements

1. Establish one project-navigation transaction and a project-scoped session boundary. Every async completion should validate its session/request identity before committing state or scheduling work.
2. Give workspace keyboard commands one owner that routes by active surface and respects focus, visibility, and editable controls.
3. Keep preview transport transitions separate from playhead sampling and drift correction. Make prepared-frame availability explicit at the compositor boundary.
4. Extract feature hooks/controllers from EditorWorkspace, then view-only components; preserve the existing typed Rust command wrappers and pure timeline helpers. File length alone does not justify a new state library.
5. Add a recoverable render-error boundary. No current crashing input was demonstrated; test intentional render failure and reload from canonical project state.
6. Evaluate lazy loading for the editor and secondary settings surfaces. Preserve the current originating-view lifetimes during Settings/Health navigation. The measured bundle size warrants investigation, but startup and playback improvements require packaged-app measurements.

Large component sizes (EditorWorkspace 11,018 lines, MediaBin 7,095, TimelineEditor 5,868) support focused ownership extraction, not a wholesale rewrite. Existing settings-operation listener cleanup is a useful pattern to reuse.
