# Frontend interaction architecture review

These are preliminary Sol findings. See the [Astra-reviewed synthesis](2026-09-11-frontend-architecture-review.md) for final priorities and corrections to reachability/impact claims.

Scope: `src/components/workspace` timeline, preview, and inspector surfaces plus `src/components/settings`. This is a read-only review; no app code was changed. The repository-wide baseline already reports 2,085 tests passing with lint and build passing in `docs/reviews/2026-09-11-frontend-checks.md`.

## Priority findings

### P1 — Source-frame arrow keys also seek the timeline

**Evidence.** `TimelineEditor` installs a document-level keydown listener (`src/components/workspace/timeline-editor.tsx:2595-2607`, registered at `:3061-3064`) and handles unmodified Left/Right by always seeking the timeline (`:2895-2908`). `PreviewPanel` independently installs another document-level listener (`src/components/workspace/preview-panel.tsx:674-714`) and, in source mode, handles the same keys by stepping the source (`:701-708`). The preview handler does not check `event.defaultPrevented`, and neither listener scopes the shortcut to a focused/active editor surface.

**Trigger and impact.** Open a source in the viewer while the timeline editor remains mounted, then press Left or Right outside an input. One keypress can change the source media `currentTime` and the canonical timeline playhead. Returning to Timeline therefore lands at an unexpected time; listener registration order cannot make this safe because the preview listener ignores the prevention flag.

**Fix.** Introduce one workspace shortcut owner that dispatches according to active viewer/focus context, or scope each shortcut handler to a focusable surface. As an immediate guard, make `PreviewPanel` return when `event.defaultPrevented` and make `TimelineEditor` skip transport keys while source mode owns transport. A shared keyboard command layer is preferable because it also makes shortcut priority explicit.

**Test gap.** Existing preview shortcut tests render `PreviewPanel` in isolation (`src/components/workspace/preview-panel.test.tsx:1551`, `:1699`). Add an integration test with both `TimelineEditor` and `PreviewPanel` mounted in source mode and assert that one ArrowRight changes only source time. Add the inverse timeline-mode assertion.

### P1 — “Canonical ready” can produce a silent blank preview

**Evidence.** The compositor removes every layer requiring canonical preparation whenever the `canonicalPreparation` object exists, including `{ status: "ready" }` (`src/components/workspace/timeline-preview-compositor.tsx:504-512`). It reports waiting layers only when status is not ready (`:532-535`) and treats the canvas as having no visible preview when those hidden layers have no overlays (`:630-634`, empty state at `:958-970`). The parent draws prepared frames only when `canonicalPreparedTimeline` resolves a matching sequence URL (`src/components/workspace/preview-panel.tsx:816-845`, render at `:1125-1147`).

**Trigger and impact.** Supply `canonicalPreviewPreparation={{status: "ready"}}` for a rich layer but omit, lag, or mismatch `canonicalPreparedTimeline` / `canonicalFrameSequences`. The direct media approximation is hidden, no canonical image is mounted, and no failed/pending issue is generated. The user sees “No timeline media at playhead” even though media exists.

**Fix.** Pass the set of canonical item IDs that actually have a frame for the current playhead into the compositor, and suppress a direct layer only when that item has a renderable prepared frame. Otherwise show the direct layer with an accuracy notice, or show a recoverable “prepared frame missing” issue. Do not infer frame availability from the preparation status alone.

**Test gap.** Current tests cover ready plus a valid sequence and pending preparation (`src/components/workspace/preview-panel.test.tsx:224-266`; `src/components/workspace/timeline-preview-compositor.test.tsx:890-925`). Add ready-with-no-sequence, ready-with-sequence-gap, and prepared-media-ID-mismatch cases.

### P2 — Timeline playback repeatedly calls `play()` for every active media layer

**Evidence.** Video and audio synchronization effects depend on `layer.sourceTimeSeconds` (`src/components/workspace/timeline-preview-compositor.tsx:64-69`, `:134-140`). A moving playhead changes that value on each render. `synchronizeTimelineMediaElement` avoids seeking when drift is small, but unconditionally calls `mediaElement.play()` whenever `isPlaying` is true (`:173-190`). Every active video and audio layer therefore creates another play request/promise at playback update frequency.

**Trigger and impact.** Play a timeline with stacked video and audio. At 30–60 playhead updates per second, each layer repeatedly invokes `play()` even while already playing. This adds promise and media-pipeline work on the hottest rendering path and scales with layer count, increasing the chance of preview jank.

**Fix.** Separate transport transitions from clock correction: call `play()` only when `isPlaying && mediaElement.paused`, pause only on a playing-to-paused transition, and perform bounded drift correction in a separate effect or scheduler. Keep the parent playhead as the clock but avoid making transport commands a side effect of every sampled frame.

**Test gap.** Add fake media-element tests that rerender several increasing `sourceTimeSeconds` values while `isPlaying` remains true and assert a single `play()` call, plus a drift-threshold seek assertion for both video and audio.

### P2 — Internal settings navigation permanently masks later external requests

**Evidence.** `SettingsContent` chooses `internalNavigation` whenever it exists (`src/components/settings/settings.tsx:261-266`). `openSettingsTarget` sets that state (`:744-748`), but no effect clears it when the `target` or `navigationRequestId` props change. Internal navigation is used when the models screen opens a provider (`:872-877`).

**Trigger and impact.** From AI & Models, choose Configure provider, which records an internal Integrations target. While Settings remains mounted, issue a new external request such as Advanced recovery with a newer `navigationRequestId`. The stale internal target still wins, so the requested category and focus destination do not open.

**Fix.** Treat internal navigation as a request with provenance and clear it when a newer external request ID arrives. A simpler model is to store one effective navigation request and have both internal and external callers feed the same reducer with monotonically increasing IDs.

**Test gap.** Existing typed-target tests rerender external requests without first creating internal navigation (`src/components/settings/settings.integration.test.tsx:408-434`, `:556-585`). Add a test that clicks Configure provider, rerenders with a newer external target, and verifies category, focus, and live announcement follow the external request.

### P2 accessibility — Viewer tabs do not implement tab keyboard semantics

**Evidence.** The viewer declares a `tablist` (`src/components/workspace/preview-panel.tsx:951-957`) and each Timeline/source control declares `role="tab"` (`:984-996`, `:1003-1019`), but all tabs retain their normal tab stop, none references a tabpanel with `aria-controls`, and there is no Left/Right/Home/End handler. The separate close button is visually fused into each source tab (`:1002-1035`) but has no grouping relationship. By comparison, `ContextualInspectorTabs` already implements roving `tabIndex`, arrow navigation, and a linked tabpanel (`src/components/workspace/contextual-inspector-tabs.tsx:34-47`, `:53-99`).

**Trigger and impact.** Keyboard and screen-reader users encounter every viewer tab in the Tab sequence and cannot use standard tablist arrow navigation. The active tab is announced, but its controlled panel relationship is absent. With many sources open, this makes a core editor surface slow and ambiguous to navigate.

**Fix.** Reuse/extract the tab behavior from `ContextualInspectorTabs`: selected tab gets `tabIndex=0`, inactive tabs `-1`, arrow/Home/End change and focus the active tab, and all tabs reference one labelled `tabpanel`. Model close as an adjacent action with an accessible group name, while retaining one roving tab target per source.

**Test gap.** Existing viewer-tab tests cover labels and previous/next buttons (`src/components/workspace/preview-panel.test.tsx:918-1018`, `:2069-2157`) but not tablist keyboard behavior or tab-to-panel relationships. Add keyboard navigation and axe/accessibility assertions.

## Architectural improvements after the defects

- Consolidate workspace keyboard commands. The timeline and preview currently own global document listeners independently, which makes collisions an emergent property rather than a typed routing decision.
- Give canonical preview availability one source of truth. `PreviewPanel` owns prepared frame sequences while `TimelinePreviewCompositor` independently decides which direct layers to suppress; a per-frame render plan should make those decisions together.
- Split timeline media synchronization into transport state and drift correction. This isolates expensive browser media commands from React’s playhead render frequency.
- Extract the proven tab primitive from `ContextualInspectorTabs` for viewer tabs and other editor tab strips, including closeable-tab behavior.

## Lower-priority cleanup opportunities

- Timeline grid lines and edit-point markers are individually exposed as `role="img"` with time labels (`src/components/workspace/timeline-editor.tsx:94-118`, `:4706-4714`). These are visual guides rather than meaningful images and can flood the accessibility tree. Mark the grid container or individual marks `aria-hidden`, then provide one concise timeline range/zoom description.
- Inspector resizing calls both local state and `onResize` on every raw pointer move (`src/components/workspace/inspector-dock.tsx:191-211`). Coalesce visual updates with `requestAnimationFrame` and persist the final width on pointer-up to keep storage or parent layout work off the pointer hot path.
- Inspector forms copy canonical item data into many local state fields and reset all drafts whenever the `item` object identity changes (for example `src/components/workspace/source-clip-inspector.tsx:840-953`, reset at `:988-1028`; similar patterns in `caption-inspector.tsx:233-241` and `text-overlay-inspector.tsx:34-55`). Introduce a reusable draft lifecycle with an explicit dirty state and conflict/reload behavior so unrelated project refreshes cannot silently erase edits.
