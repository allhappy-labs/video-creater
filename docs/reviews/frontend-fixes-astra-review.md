# Focused frontend architecture review

Date: 2026-09-11. Reviewed against `32e81fcc`, with HEAD `83876190456d7d6c16af4d2d9d6284599a8ead66` and the uncommitted inspector/tooling changes present during review. Source review only; integration owner runs verification. No implementation files changed by this reviewer.

## Remaining findings

1. **P2 — successful inspector applies can falsely block further editing.** [source-clip-inspector.tsx:764](../../src/components/workspace/source-clip-inspector.tsx#L764) places independently applied fields, including both audio and visual representations of the same fade properties, into one canonical draft. [use-inspector-draft-lifecycle.tsx:39](../../src/components/workspace/use-inspector-draft-lifecycle.tsx#L39) accepts a canonical update only when the entire draft equals the old or new canonical value. Select a video with no fades, change Visual fade in to 1, apply, and rerender the successful result: canonical audio and visual fade values both become 1, but the hidden audio draft remains 0. The inspector reports an external change and disables all applies. Editing two independent property groups and applying only one has the same problem. Reconcile drafts per apply operation, preserving other dirty groups; test successful apply/returned-project cycles as well as actual external conflicts. This finding has been sent to the inspector owner for correction.

2. **P2 — closing a middle source tab focuses a different source from the one selected.** [preview-panel.tsx:701](../../src/components/workspace/preview-panel.tsx#L701) focuses the removed tab's numeric position, while [editor-workspace.tsx:6746](../../src/components/workspace/editor-workspace.tsx#L6746) selects the previous source. Open A, B, C and close active B: A becomes selected, but C receives focus despite `tabIndex=-1`; subsequent arrow navigation is based on C rather than the displayed source. Focus the selected surviving tab after the parent updates. The existing two-source close test does not exercise this mismatch.

3. **P2 — the legacy screenshot runner can silently use an existing server.** [browser-visual-qa.mjs:370](../../scripts/browser-visual-qa.mjs#L370) accepts any HTTP 200 on the configured port while the newly spawned pnpm process is merely alive. If another app instance already occupies 4179, the probe can succeed before the new Vite process reports its strict-port failure, and screenshots then capture the existing checkout/runtime. Validate ownership/readiness of the spawned server and fail on an occupied default port; add an occupied-port regression. The new Playwright config separately uses `reuseExistingServer: false` and is not affected by this finding.

## Architecture judgment and limits

The navigation request identity, keyed project session, disposed poller, App-owned menu publication, active-view keyboard routing, Settings request supersession, and canonical-frame retry ownership are coherent improvements. No additional concrete blocker was found in those focused paths. The browser smoke gate uses an explicit fixture runtime and actual decoded media metadata; it is useful browser evidence, not native backend or packaged macOS verification. The narrow editor case deliberately runs at 900 px, so it does not establish editor usability at 390 px.

Changes requested for the three findings above. This report records the reviewed snapshot; subsequent corrections need focused verification before final approval.

## Follow-up disposition

The bounded follow-up reviewed only the three findings above: commit `7c8ff56645139ed0d3c21f3865de409eb537d014` and the current owned-Vite startup changes. **All three findings are resolved on source review; approved within this review scope.**

- **Inspector reconciliation — resolved.** Draft comparison and apply blocking now follow individual apply operations. The hidden audio fade draft can adopt returned values without conflicting with the visual fade draft; applying opacity preserves an independent dirty speed draft. Added regressions cover both scenarios and a subsequent real external speed conflict. Reload targets conflicting groups rather than discarding unrelated drafts.
- **Viewer close focus — resolved.** Deferred close focus now targets `activeViewerTabId`, matching the parent's selected surviving source. The added three-source/middle-close regression covers the original mismatch.
- **Screenshot server ownership — resolved.** `scripts/vite-visual-qa-server.mjs` writes the owned ready file only after `server.listen()` succeeds with `strictPort: true`. The parent waits for that file instead of accepting an unrelated HTTP response. An occupied-port regression verifies startup rejection; the reported successful Home browser run exercises the positive path.

The integration owner reports 154 focused inspector/preview tests and the broader 2,145-test suite passing, along with lint, build, policy, and Knip checks; the tooling owner reports the occupied-port and real Home checks passing. This follow-up inspected the changed code and regression assertions without independently rerunning tests. Subsequent media-fit/responsive layout work is outside this three-finding follow-up.
